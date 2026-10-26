//! A small semantic HTML interchange layer. The DOM adapter supplies a parsed
//! tree; this module never interprets markup with regular expressions or executes
//! source HTML. Only explicitly supported elements, marks and URLs survive.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{ComponentMark, ComponentNode, document::NodeId, is_safe_media_url, is_safe_url};

use super::clipboard::{MAX_CLIPBOARD_DEPTH, MAX_CLIPBOARD_NODES};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum HtmlNode {
    Text(String),
    Element {
        tag: String,
        attrs: BTreeMap<String, String>,
        children: Vec<HtmlNode>,
    },
}

impl HtmlNode {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }
    pub fn element(tag: impl Into<String>, children: Vec<Self>) -> Self {
        Self::Element {
            tag: tag.into(),
            attrs: BTreeMap::new(),
            children,
        }
    }
    pub fn with_attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        if let Self::Element { attrs, .. } = &mut self {
            attrs.insert(name.into(), value.into());
        }
        self
    }
    pub fn text_content(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Element { tag, .. } if blocked(&tag.to_ascii_lowercase()) => String::new(),
            Self::Element { tag, .. } if tag.eq_ignore_ascii_case("br") => "\n".into(),
            Self::Element { children, .. } => children.iter().map(Self::text_content).collect(),
        }
    }
}

pub fn wrap_block(content: ComponentNode, children: Vec<ComponentNode>) -> ComponentNode {
    let mut nodes = vec![content];
    if !children.is_empty() {
        nodes.push(ComponentNode::container("block_children", None, children));
    }
    ComponentNode::container(
        "block",
        Some(NodeId::new(uuid::Uuid::now_v7().to_string())),
        nodes,
    )
}

fn content(kind: &str, inline: Vec<ComponentNode>) -> ComponentNode {
    ComponentNode::container(kind, None, inline)
}

fn blocked(tag: &str) -> bool {
    matches!(
        tag,
        "script"
            | "style"
            | "iframe"
            | "object"
            | "embed"
            | "form"
            | "input"
            | "textarea"
            | "select"
            | "button"
            | "svg"
            | "math"
            | "template"
            | "noscript"
            | "head"
            | "link"
            | "meta"
    )
}

fn is_block_tag(tag: &str) -> bool {
    matches!(
        tag,
        "p" | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "div"
            | "section"
            | "article"
            | "main"
            | "body"
            | "html"
            | "blockquote"
            | "aside"
            | "pre"
            | "ul"
            | "ol"
            | "li"
            | "table"
            | "figure"
            | "hr"
            | "img"
            | "details"
            | "summary"
    )
}

fn tree_within_limits(node: &HtmlNode, depth: usize, budget: &mut usize) -> bool {
    if depth > MAX_CLIPBOARD_DEPTH || *budget == 0 {
        return false;
    }
    *budget -= 1;
    match node {
        HtmlNode::Text(text) => text.len() <= super::clipboard::MAX_CLIPBOARD_TEXT,
        HtmlNode::Element { children, .. } => children
            .iter()
            .all(|child| tree_within_limits(child, depth + 1, budget)),
    }
}

/// Normalize lists, tables and media into the one block editing schema.
/// Dangerous subtrees and unsafe URLs are discarded before inline conversion.
pub fn html_to_blocks(root: &HtmlNode) -> Vec<ComponentNode> {
    let mut budget = MAX_CLIPBOARD_NODES;
    if !tree_within_limits(root, 0, &mut budget) {
        return Vec::new();
    }
    visit(root)
}

fn inline_children(children: &[HtmlNode], marks: &[ComponentMark]) -> Vec<ComponentNode> {
    let mut output = Vec::new();
    for child in children {
        inline(child, marks, &mut output);
    }
    coalesce_text(output)
}

fn coalesce_text(nodes: Vec<ComponentNode>) -> Vec<ComponentNode> {
    let mut output: Vec<ComponentNode> = Vec::new();
    for node in nodes {
        if node.kind.0 == "text" {
            if node.text.as_deref() == Some("") {
                continue;
            }
            if let Some(last) = output
                .last_mut()
                .filter(|last| last.kind.0 == "text" && last.marks == node.marks)
            {
                last.text
                    .get_or_insert_default()
                    .push_str(node.text.as_deref().unwrap_or_default());
                continue;
            }
        }
        output.push(node);
    }
    output
}

fn inline(node: &HtmlNode, marks: &[ComponentMark], output: &mut Vec<ComponentNode>) {
    match node {
        HtmlNode::Text(text) => {
            if !text.is_empty() {
                let mut node = ComponentNode::text(text.replace('\u{feff}', ""));
                node.marks = marks.to_vec();
                output.push(node);
            }
        }
        HtmlNode::Element {
            tag,
            attrs,
            children,
        } => {
            let tag = tag.to_ascii_lowercase();
            if blocked(&tag) || matches!(tag.as_str(), "ul" | "ol" | "img" | "table") {
                return;
            }
            if tag == "br" {
                output.push(ComponentNode::new("hard_break"));
                return;
            }
            if matches!(tag.as_str(), "p" | "div")
                && !output.is_empty()
                && output
                    .last()
                    .is_some_and(|node| node.kind.0 != "hard_break")
            {
                output.push(ComponentNode::new("hard_break"));
            }
            if let Some(kind) = attrs.get("data-composer-chip") {
                let label = attrs
                    .get("data-label")
                    .cloned()
                    .unwrap_or_else(|| node.text_content());
                let mut chip = ComponentNode::new("chip");
                chip.attrs
                    .insert("kind".into(), Value::String(kind.clone()));
                chip.attrs.insert("label".into(), Value::String(label));
                chip.attrs.insert(
                    "payload".into(),
                    Value::String(
                        attrs
                            .get("data-payload")
                            .cloned()
                            .unwrap_or_else(|| "null".into()),
                    ),
                );
                output.push(chip);
                return;
            }
            if let Some(id) = attrs.get("data-semantic-mention").or_else(|| {
                attrs
                    .get("data-entity-id")
                    .filter(|_| attrs.contains_key("data-mention"))
            }) {
                if !id.is_empty() && id.len() <= 1024 {
                    let mut mention = ComponentNode::new("mention");
                    mention
                        .attrs
                        .insert("entity_id".into(), Value::String(id.clone()));
                    mention.attrs.insert(
                        "label".into(),
                        Value::String(attrs.get("data-label").cloned().unwrap_or_else(|| {
                            node.text_content().trim_start_matches('@').to_owned()
                        })),
                    );
                    output.push(mention);
                    return;
                }
            }
            let mut next_marks = marks.to_vec();
            let mark = match tag.as_str() {
                "strong" | "b" => Some(ComponentMark::new("bold")),
                "em" | "i" => Some(ComponentMark::new("italic")),
                "u" => Some(ComponentMark::new("underline")),
                "s" | "del" | "strike" => Some(ComponentMark::new("strike")),
                "code" => Some(ComponentMark::new("code")),
                "mark" => {
                    let mut mark = ComponentMark::new("highlight");
                    mark.attrs.insert(
                        "color".into(),
                        Value::String(
                            safe_color(attrs.get("data-highlight").map(String::as_str))
                                .unwrap_or("yellow")
                                .into(),
                        ),
                    );
                    Some(mark)
                }
                "a" => attrs.get("href").and_then(|href| {
                    if let Some(id) = href.strip_prefix("semantic:entity:").filter(|id| {
                        !id.is_empty() && id.len() <= 1024 && !id.chars().any(char::is_control)
                    }) {
                        let mut mark = ComponentMark::new("entity_link");
                        mark.attrs
                            .insert("entity_id".into(), Value::String(id.into()));
                        Some(mark)
                    } else if is_safe_url(href) {
                        let mut mark = ComponentMark::new("link");
                        mark.attrs
                            .insert("href".into(), Value::String(href.clone()));
                        if let Some(title) = attrs.get("title") {
                            mark.attrs
                                .insert("title".into(), Value::String(title.clone()));
                        }
                        Some(mark)
                    } else {
                        None
                    }
                }),
                "span" => attrs
                    .get("data-color")
                    .and_then(|color| safe_color(Some(color)))
                    .map(|color| {
                        let mut mark = ComponentMark::new("text_color");
                        mark.attrs
                            .insert("color".into(), Value::String(color.into()));
                        mark
                    })
                    .or_else(|| {
                        attrs
                            .get("data-highlight")
                            .and_then(|color| safe_color(Some(color)))
                            .map(|color| {
                                let mut mark = ComponentMark::new("highlight");
                                mark.attrs
                                    .insert("color".into(), Value::String(color.into()));
                                mark
                            })
                    }),
                _ => None,
            };
            if let Some(mark) = mark {
                next_marks.retain(|existing| existing.kind != mark.kind);
                next_marks.push(mark);
            }
            for child in children {
                inline(child, &next_marks, output);
            }
        }
    }
}

fn safe_color(value: Option<&str>) -> Option<&str> {
    value.filter(|color| crate::profiles::block_document::COLOR_TOKENS.contains(color))
}

fn visit(node: &HtmlNode) -> Vec<ComponentNode> {
    let HtmlNode::Element {
        tag,
        attrs,
        children,
    } = node
    else {
        return if node.text_content().trim().is_empty() {
            Vec::new()
        } else {
            vec![wrap_block(
                content("paragraph", vec![ComponentNode::text(node.text_content())]),
                Vec::new(),
            )]
        };
    };
    let tag = tag.to_ascii_lowercase();
    if blocked(&tag) {
        return Vec::new();
    }
    match tag.as_str() {
        "ul" | "ol" => {
            let ordered = tag == "ol";
            let task = attrs
                .get("data-type")
                .is_some_and(|kind| kind == "taskList");
            let mut items = Vec::new();
            for child in children {
                let HtmlNode::Element {
                    tag,
                    attrs: item_attrs,
                    children: item_children,
                } = child
                else {
                    continue;
                };
                if !tag.eq_ignore_ascii_case("li") {
                    continue;
                }
                let mut nested = Vec::new();
                let mut own = Vec::new();
                for child in item_children {
                    if matches!(child, HtmlNode::Element { tag, .. } if matches!(tag.to_ascii_lowercase().as_str(), "ul" | "ol"))
                    {
                        nested.extend(visit(child));
                    } else {
                        own.push(child.clone());
                    }
                }
                let mut item = content(
                    if task {
                        "todo"
                    } else if ordered {
                        "numbered_list_item"
                    } else {
                        "bulleted_list_item"
                    },
                    inline_children(&own, &[]),
                );
                if task {
                    item.attrs.insert(
                        "checked".into(),
                        json!(
                            item_attrs
                                .get("data-checked")
                                .is_some_and(|value| value == "true")
                        ),
                    );
                }
                if ordered && items.is_empty() {
                    item.attrs.insert(
                        "start".into(),
                        json!(
                            attrs
                                .get("start")
                                .and_then(|value| value.parse::<u64>().ok())
                                .filter(|n| *n > 0)
                                .unwrap_or(1)
                        ),
                    );
                }
                items.push(wrap_block(item, nested));
            }
            items
        }
        "table" => table(children),
        "img" => {
            let Some(src) = attrs.get("src").filter(|src| is_safe_media_url(src)) else {
                return Vec::new();
            };
            let mut image = ComponentNode::new("image");
            image
                .attrs
                .insert("media".into(), json!({"type":"url", "url":src}));
            if let Some(alt) = attrs.get("alt") {
                image.attrs.insert("alt".into(), json!(alt));
            }
            if let Some(title) = attrs.get("title") {
                image.attrs.insert("title".into(), json!(title));
            }
            vec![wrap_block(image, Vec::new())]
        }
        "figure" => {
            let image = children.iter().find(|child| matches!(child, HtmlNode::Element { tag, .. } if tag.eq_ignore_ascii_case("img")));
            let Some(image) = image else {
                return block_children(children);
            };
            let mut blocks = visit(image);
            if let (Some(block), Some(HtmlNode::Element { children, .. })) = (blocks.first_mut(), children.iter().find(|child| matches!(child, HtmlNode::Element { tag, .. } if tag.eq_ignore_ascii_case("figcaption")))) {
                block.content[0].content = inline_children(children, &[]);
            }
            blocks
        }
        "hr" => vec![wrap_block(ComponentNode::new("divider"), Vec::new())],
        "pre" => {
            let mut code = content(
                "code_block",
                if node.text_content().is_empty() {
                    Vec::new()
                } else {
                    vec![ComponentNode::text(node.text_content())]
                },
            );
            code.attrs.insert("wrap".into(), json!(false));
            let language = attrs.get("data-language").cloned().or_else(|| {
                children.iter().find_map(|child| {
                    let HtmlNode::Element { attrs, .. } = child else {
                        return None;
                    };
                    attrs
                        .get("class")?
                        .split_ascii_whitespace()
                        .find_map(|class| class.strip_prefix("language-").map(str::to_string))
                })
            });
            if let Some(language) = language {
                code.attrs.insert("language".into(), json!(language));
            }
            vec![wrap_block(code, Vec::new())]
        }
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let mut heading = content("heading", inline_children(children, &[]));
            heading
                .attrs
                .insert("level".into(), json!(tag.as_bytes()[1] - b'0'));
            vec![wrap_block(heading, Vec::new())]
        }
        "blockquote" => {
            let mut blocks = block_children(children);
            if blocks.is_empty() {
                return vec![wrap_block(content("quote", Vec::new()), Vec::new())];
            }
            let first = blocks.remove(0);
            let mut nested = first
                .content
                .get(1)
                .map(|group| group.content.clone())
                .unwrap_or_default();
            nested.extend(blocks);
            let quote = content(
                "quote",
                first
                    .content
                    .first()
                    .map(|node| node.content.clone())
                    .unwrap_or_default(),
            );
            vec![wrap_block(quote, nested)]
        }
        "p" | "summary" | "li" => paragraph_with_images(children),
        "div" | "aside"
            if attrs.get("data-block-kind").is_some_and(|kind| {
                matches!(
                    kind.as_str(),
                    "todo"
                        | "toggle"
                        | "callout"
                        | "quote"
                        | "bulleted_list_item"
                        | "numbered_list_item"
                )
            }) =>
        {
            let kind = attrs.get("data-block-kind").unwrap();
            let mut node = content(kind, inline_children(children, &[]));
            if kind == "todo" {
                node.attrs.insert(
                    "checked".into(),
                    json!(
                        attrs
                            .get("data-checked")
                            .is_some_and(|value| value == "true")
                    ),
                );
            }
            vec![wrap_block(node, Vec::new())]
        }
        "div" | "section" | "article" | "main" | "body" | "html" | "details" => {
            block_children(children)
        }
        _ => vec![wrap_block(
            content(
                "paragraph",
                inline_children(std::slice::from_ref(node), &[]),
            ),
            Vec::new(),
        )],
    }
}

fn paragraph_with_images(children: &[HtmlNode]) -> Vec<ComponentNode> {
    let mut blocks = Vec::new();
    let mut pending = Vec::new();
    for child in children {
        if matches!(child, HtmlNode::Element { tag, .. } if tag.eq_ignore_ascii_case("img")) {
            if !pending.is_empty() {
                blocks.push(wrap_block(
                    content(
                        "paragraph",
                        inline_children(&std::mem::take(&mut pending), &[]),
                    ),
                    Vec::new(),
                ));
            }
            blocks.extend(visit(child));
        } else {
            pending.push(child.clone());
        }
    }
    if !pending.is_empty() || blocks.is_empty() {
        blocks.push(wrap_block(
            content("paragraph", inline_children(&pending, &[])),
            Vec::new(),
        ));
    }
    blocks
}

fn block_children(children: &[HtmlNode]) -> Vec<ComponentNode> {
    let mut blocks = Vec::new();
    let mut pending = Vec::new();
    let flush = |pending: &mut Vec<HtmlNode>, blocks: &mut Vec<ComponentNode>| {
        if !pending.is_empty() && pending.iter().any(|node| {
            !node.text_content().trim().is_empty()
                || matches!(node, HtmlNode::Element { tag, .. } if tag.eq_ignore_ascii_case("br"))
        }) {
            blocks.extend(paragraph_with_images(pending));
        }
        pending.clear();
    };
    for child in children {
        match child {
            HtmlNode::Element { tag, .. } if blocked(&tag.to_ascii_lowercase()) => {}
            HtmlNode::Element { tag, .. } if is_block_tag(&tag.to_ascii_lowercase()) => {
                flush(&mut pending, &mut blocks);
                blocks.extend(visit(child));
            }
            _ => pending.push(child.clone()),
        }
    }
    flush(&mut pending, &mut blocks);
    blocks
}

fn table(children: &[HtmlNode]) -> Vec<ComponentNode> {
    fn rows<'a>(nodes: &'a [HtmlNode], output: &mut Vec<&'a HtmlNode>) {
        for node in nodes {
            if let HtmlNode::Element { tag, children, .. } = node {
                if tag.eq_ignore_ascii_case("tr") {
                    output.push(node);
                } else if matches!(
                    tag.to_ascii_lowercase().as_str(),
                    "thead" | "tbody" | "tfoot"
                ) {
                    rows(children, output);
                }
            }
        }
    }
    let mut source_rows = Vec::new();
    rows(children, &mut source_rows);
    if source_rows.is_empty() || source_rows.len() > 100 {
        return Vec::new();
    }
    let cells = |row: &HtmlNode| {
        match row {
        HtmlNode::Element { children, .. } => children.iter().filter(|cell| matches!(cell, HtmlNode::Element { tag, .. } if tag.eq_ignore_ascii_case("td") || tag.eq_ignore_ascii_case("th"))).cloned().collect::<Vec<_>>(),
        _ => Vec::new(),
    }
    };
    let width = source_rows
        .iter()
        .map(|row| cells(row).len())
        .max()
        .unwrap_or(0);
    if width == 0 || width > 100 {
        return Vec::new();
    }
    let header_row = cells(source_rows[0]).iter().all(
        |cell| matches!(cell, HtmlNode::Element { tag, .. } if tag.eq_ignore_ascii_case("th")),
    );
    let mut node = ComponentNode::new("table");
    node.attrs.insert("header_row".into(), json!(header_row));
    node.attrs.insert("header_column".into(), json!(false));
    for row in source_rows {
        let source_cells = cells(row);
        let mut output_cells = Vec::new();
        for column in 0..width {
            let (kind, inline, attrs) = match source_cells.get(column) {
                Some(HtmlNode::Element {
                    tag,
                    attrs,
                    children,
                }) => (
                    if tag.eq_ignore_ascii_case("th") {
                        "table_header_cell"
                    } else {
                        "table_cell"
                    },
                    inline_children(children, &[]),
                    Some(attrs),
                ),
                _ => ("table_cell", Vec::new(), None),
            };
            let mut cell = content(kind, inline);
            if let Some(attrs) = attrs {
                let alignment = attrs.get("align").map(String::as_str).or_else(|| {
                    attrs.get("style").and_then(|style| {
                        style.split(';').find_map(|property| {
                            let (name, value) = property.split_once(':')?;
                            name.trim()
                                .eq_ignore_ascii_case("text-align")
                                .then_some(value.trim())
                        })
                    })
                });
                if let Some(alignment) =
                    alignment.filter(|value| matches!(*value, "left" | "center" | "right"))
                {
                    cell.attrs.insert("alignment".into(), json!(alignment));
                }
                if let Some(color) =
                    safe_color(attrs.get("data-background-color").map(String::as_str))
                {
                    cell.attrs.insert("background_color".into(), json!(color));
                }
            }
            output_cells.push(cell);
        }
        node.content.push(ComponentNode::container(
            "table_row",
            Some(NodeId::new(uuid::Uuid::now_v7().to_string())),
            output_cells,
        ));
    }
    vec![wrap_block(node, Vec::new())]
}

pub fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn inline_html(nodes: &[ComponentNode]) -> String {
    nodes.iter().map(|node| {
        let mut html = match node.kind.0.as_str() {
            "text" => escape_html(node.text.as_deref().unwrap_or_default()),
            "hard_break" => "<br>".into(),
            "mention" => {
                let id = node.attrs.get("entity_id").and_then(Value::as_str).unwrap_or_default();
                let label = node.attrs.get("label").and_then(Value::as_str).unwrap_or(id);
                format!("<a href=\"semantic:entity:{}\" data-semantic-mention=\"{}\" data-label=\"{}\">@{}</a>", escape_html(id), escape_html(id), escape_html(label), escape_html(label))
            }
            "chip" => {
                let label = node.attrs.get("label").and_then(Value::as_str).unwrap_or_default();
                let kind = node.attrs.get("kind").and_then(Value::as_str).unwrap_or_default();
                let payload = node.attrs.get("payload").map(|value| match value { Value::String(value) => value.clone(), _ => value.to_string() }).unwrap_or_else(|| "null".into());
                format!("<span data-composer-chip=\"{}\" data-label=\"{}\" data-payload=\"{}\">{}</span>", escape_html(kind), escape_html(label), escape_html(&payload), escape_html(label))
            }
            "opaque_markdown_inline" => escape_html(node.attrs.get("source").and_then(Value::as_str).unwrap_or_default()),
            _ => inline_html(&node.content),
        };
        for mark in node.marks.iter().rev() {
            html = match mark.kind.0.as_str() {
                "bold" => format!("<strong>{html}</strong>"), "italic" => format!("<em>{html}</em>"),
                "underline" => format!("<u>{html}</u>"), "strike" => format!("<s>{html}</s>"), "code" => format!("<code>{html}</code>"),
                "link" => if let Some(href) = mark.attrs.get("href").and_then(Value::as_str).filter(|href| is_safe_url(href)) {
                    let title = mark.attrs.get("title").and_then(Value::as_str).map(|title| format!(" title=\"{}\"", escape_html(title))).unwrap_or_default();
                    format!("<a href=\"{}\"{title}>{html}</a>", escape_html(href))
                } else { html },
                "entity_link" => if let Some(id) = mark.attrs.get("entity_id").and_then(Value::as_str) { format!("<a href=\"semantic:entity:{}\">{html}</a>", escape_html(id)) } else { html },
                "highlight" | "text_color" => if let Some(color) = safe_color(mark.attrs.get("color").and_then(Value::as_str)) {
                    let attr = if mark.kind.0 == "highlight" { "data-highlight" } else { "data-color" };
                    format!("<span {attr}=\"{color}\">{html}</span>")
                } else { html },
                _ => html,
            };
        }
        html
    }).collect()
}

fn block_content(node: &ComponentNode) -> (&ComponentNode, &[ComponentNode]) {
    if node.kind.0 == "block" {
        let own = node.content.first().unwrap_or(node);
        let children = node
            .content
            .get(1)
            .filter(|group| group.kind.0 == "block_children")
            .map(|group| group.content.as_slice())
            .unwrap_or_default();
        (own, children)
    } else {
        (node, &[])
    }
}

pub fn blocks_to_html(blocks: &[ComponentNode]) -> String {
    let mut output = String::new();
    let mut index = 0;
    while index < blocks.len() {
        let (node, children) = block_content(&blocks[index]);
        if matches!(
            node.kind.0.as_str(),
            "bulleted_list_item" | "numbered_list_item" | "todo"
        ) {
            let kind = node.kind.0.as_str();
            let ordered = kind == "numbered_list_item";
            let tag = if ordered { "ol" } else { "ul" };
            let attrs = if ordered {
                format!(
                    " start=\"{}\"",
                    node.attrs.get("start").and_then(Value::as_u64).unwrap_or(1)
                )
            } else if kind == "todo" {
                " data-type=\"taskList\"".into()
            } else {
                String::new()
            };
            output.push_str(&format!("<{tag}{attrs}>"));
            while index < blocks.len() {
                let (item, children) = block_content(&blocks[index]);
                if item.kind.0 != kind {
                    break;
                }
                let attrs = if kind == "todo" {
                    format!(
                        " data-type=\"taskItem\" data-checked=\"{}\"",
                        item.attrs
                            .get("checked")
                            .and_then(Value::as_bool)
                            .unwrap_or(false)
                    )
                } else {
                    String::new()
                };
                output.push_str(&format!(
                    "<li{attrs}>{}{}</li>",
                    inline_html(&item.content),
                    blocks_to_html(children)
                ));
                index += 1;
            }
            output.push_str(&format!("</{tag}>"));
            continue;
        }
        let inline = inline_html(&node.content);
        let nested = blocks_to_html(children);
        let html = match node.kind.0.as_str() {
            "paragraph" => format!("<p>{inline}</p>{nested}"),
            "heading" => {
                let level = node
                    .attrs
                    .get("level")
                    .and_then(Value::as_u64)
                    .unwrap_or(1)
                    .clamp(1, 6);
                format!("<h{level}>{inline}</h{level}>{nested}")
            }
            "quote" | "blockquote" => format!("<blockquote><p>{inline}</p>{nested}</blockquote>"),
            "callout" | "toggle" => format!(
                "<div data-block-kind=\"{}\">{inline}</div>{nested}",
                node.kind.0
            ),
            "code_block" => {
                let language = node
                    .attrs
                    .get("language")
                    .and_then(Value::as_str)
                    .map(|language| format!(" data-language=\"{}\"", escape_html(language)))
                    .unwrap_or_default();
                format!(
                    "<pre{language}><code>{}</code></pre>{nested}",
                    escape_html(&node.text_content())
                )
            }
            "divider" | "thematic_break" => format!("<hr>{nested}"),
            "image" => {
                let src = node
                    .attrs
                    .get("media")
                    .filter(|media| media.get("type").and_then(Value::as_str) == Some("url"))
                    .and_then(|media| media.get("url"))
                    .and_then(Value::as_str)
                    .or_else(|| node.attrs.get("src").and_then(Value::as_str));
                if let Some(src) = src.filter(|src| is_safe_media_url(src)) {
                    let alt = node
                        .attrs
                        .get("alt")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let title = node
                        .attrs
                        .get("title")
                        .and_then(Value::as_str)
                        .map(|title| format!(" title=\"{}\"", escape_html(title)))
                        .unwrap_or_default();
                    format!(
                        "<figure><img src=\"{}\" alt=\"{}\"{title}><figcaption>{inline}</figcaption></figure>{nested}",
                        escape_html(src),
                        escape_html(alt)
                    )
                } else {
                    format!(
                        "<p>{}</p>{nested}",
                        escape_html(
                            node.attrs
                                .get("alt")
                                .and_then(Value::as_str)
                                .unwrap_or("Image")
                        )
                    )
                }
            }
            "table" => {
                let mut table = String::from("<table><tbody>");
                for row in &node.content {
                    table.push_str("<tr>");
                    for cell in &row.content {
                        let tag =
                            if matches!(cell.kind.0.as_str(), "table_header_cell" | "table_header")
                            {
                                "th"
                            } else {
                                "td"
                            };
                        let align = cell
                            .attrs
                            .get("alignment")
                            .and_then(Value::as_str)
                            .filter(|align| matches!(*align, "left" | "center" | "right"))
                            .map(|align| format!(" style=\"text-align:{align}\""))
                            .unwrap_or_default();
                        let color =
                            safe_color(cell.attrs.get("background_color").and_then(Value::as_str))
                                .map(|color| format!(" data-background-color=\"{color}\""))
                                .unwrap_or_default();
                        table.push_str(&format!(
                            "<{tag}{align}{color}>{}</{tag}>",
                            inline_html(&cell.content)
                        ));
                    }
                    table.push_str("</tr>");
                }
                table.push_str("</tbody></table>");
                table.push_str(&nested);
                table
            }
            "block_children" | "document" => blocks_to_html(&node.content),
            "opaque_markdown" | "opaque_markdown_block" => format!(
                "<pre>{}</pre>{nested}",
                escape_html(
                    node.attrs
                        .get("source")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                )
            ),
            "text" | "hard_break" | "mention" | "chip" | "opaque_markdown_inline" => {
                inline_html(std::slice::from_ref(node))
            }
            _ => format!(
                "<p>{}</p>{nested}",
                escape_html(
                    node.attrs
                        .get("title")
                        .or_else(|| node.attrs.get("label"))
                        .and_then(Value::as_str)
                        .unwrap_or(&node.kind.0)
                )
            ),
        };
        output.push_str(&html);
        index += 1;
    }
    output
}

/// A second carrier keeps internal clipboard data available in applications that
/// strip custom MIME types. Attribute escaping is essential: this is JSON, never
/// executable HTML.
pub fn blocks_to_html_with_slice(blocks: &[ComponentNode], envelope: &str) -> String {
    format!(
        "<div data-semantic-slice=\"{}\">{}</div>",
        escape_html(envelope),
        blocks_to_html(blocks)
    )
}

fn inline_plain(nodes: &[ComponentNode]) -> String {
    nodes
        .iter()
        .map(|node| match node.kind.0.as_str() {
            "mention" | "chip" => node
                .attrs
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            "opaque_markdown_inline" => node
                .attrs
                .get("source")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            "hard_break" => "\n".into(),
            "text" => node.text.clone().unwrap_or_default(),
            _ => inline_plain(&node.content),
        })
        .collect()
}

pub fn blocks_to_plain_text(blocks: &[ComponentNode]) -> String {
    let mut lines = Vec::new();
    for block in blocks {
        let (node, children) = block_content(block);
        match node.kind.0.as_str() {
            "document" | "block_children" => lines.push(blocks_to_plain_text(&node.content)),
            "table" => lines.push(
                node.content
                    .iter()
                    .map(|row| {
                        row.content
                            .iter()
                            .map(|cell| inline_plain(&cell.content))
                            .collect::<Vec<_>>()
                            .join("\t")
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            "image" => lines.push(if node.content.is_empty() {
                node.attrs
                    .get("alt")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            } else {
                inline_plain(&node.content)
            }),
            "opaque_markdown" | "opaque_markdown_block" => lines.push(
                node.attrs
                    .get("source")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            ),
            "text" | "hard_break" | "mention" | "chip" | "opaque_markdown_inline" => {
                lines.push(inline_plain(std::slice::from_ref(node)))
            }
            "divider" | "details_break" => lines.push(String::new()),
            _ => lines.push(inline_plain(&node.content)),
        }
        if !children.is_empty() {
            lines.push(blocks_to_plain_text(children));
        }
    }
    lines.join("\n")
}

/// Import semantic HTML into the active editing catalog. Clipboard HTML can
/// contain richer marks and attributes than Markdown or composer can persist.
/// Unsupported blocks retain their readable text; supported constructs keep only
/// the attributes and marks declared by this catalog.
pub fn restrict_to_catalog(
    blocks: Vec<ComponentNode>,
    catalog: &crate::ComponentCatalog,
    allows_mark: &dyn Fn(&str) -> bool,
) -> Vec<ComponentNode> {
    fn trim(
        node: &mut ComponentNode,
        catalog: &crate::ComponentCatalog,
        allows_mark: &dyn Fn(&str) -> bool,
    ) {
        let Some(spec) = catalog.spec(&node.kind) else {
            let text = node
                .attrs
                .get("label")
                .or_else(|| node.attrs.get("source"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| node.text_content());
            *node = ComponentNode::text(text);
            return;
        };
        node.attrs
            .retain(|key, _| spec.attributes.contains_key(key));
        if spec.identity == crate::IdentityPolicy::None {
            node.id = None;
        }
        node.marks.retain_mut(|mark| {
            let Some(spec) = catalog.spec(&mark.kind).filter(|spec| {
                spec.kind == crate::ComponentKind::Mark && allows_mark(&mark.kind.0)
            }) else {
                return false;
            };
            mark.attrs
                .retain(|key, _| spec.attributes.contains_key(key));
            true
        });
        for child in &mut node.content {
            trim(child, catalog, allows_mark);
        }
    }
    let mut result = Vec::new();
    for mut block in blocks {
        if block.kind.0 != "block" {
            block = wrap_block(block, Vec::new());
        }
        let children = block
            .content
            .iter()
            .find(|node| node.kind.0 == "block_children")
            .map(|node| node.content.clone())
            .unwrap_or_default();
        let mut node = block
            .content
            .first()
            .cloned()
            .unwrap_or_else(|| ComponentNode::new("paragraph"));
        if catalog.spec(&node.kind).is_none() {
            let text = blocks_to_plain_text(std::slice::from_ref(&node));
            node = ComponentNode::container(
                "paragraph",
                None,
                if text.is_empty() {
                    Vec::new()
                } else {
                    vec![ComponentNode::text(text)]
                },
            );
        }
        let allows_children = catalog
            .spec(&node.kind)
            .is_some_and(|spec| spec.behavior.allows_children);
        trim(&mut node, catalog, allows_mark);
        block.content = vec![node];
        if let Some(spec) = catalog.spec(&block.kind) {
            block
                .attrs
                .retain(|key, _| spec.attributes.contains_key(key));
        }
        let children = restrict_to_catalog(children, catalog, allows_mark);
        if allows_children && !children.is_empty() {
            block
                .content
                .push(ComponentNode::container("block_children", None, children));
            result.push(block);
        } else {
            result.push(block);
            result.extend(children);
        }
    }
    result
}
