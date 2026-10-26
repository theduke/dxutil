use super::mapping::ZW;
use crate::engine::{EditorEngine, InlineContainerRef, RenderPlan, model};
use crate::{ComponentMark, ComponentNode};
use std::collections::{BTreeMap, BTreeSet};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, Element, Node};

#[derive(Default)]
pub struct Renderer {
    pub blocks: BTreeMap<String, Element>,
    cache: BTreeMap<String, ComponentNode>,
}
fn element(doc: &Document, tag: &str, class: &str) -> Result<Element, JsValue> {
    let el = doc.create_element(tag)?;
    if !class.is_empty() {
        el.set_class_name(class);
    }
    Ok(el)
}
fn append(parent: &Element, child: &Element) -> Result<(), JsValue> {
    parent.append_child(child)?;
    Ok(())
}
fn attrs(el: &Element, node: &ComponentNode) -> Result<(), JsValue> {
    for (key, value) in &node.attrs {
        if matches!(
            key.as_str(),
            "text_color" | "background_color" | "alignment"
        ) {
            if let Some(v) = value.as_str() {
                el.set_attribute(&format!("data-{}", key.replace('_', "-")), v)?;
            }
        }
    }
    Ok(())
}
fn bind(el: &Element, c: InlineContainerRef) -> Result<(), JsValue> {
    el.set_attribute("data-inline-container", &serde_json::to_string(&c).unwrap())
}
impl Renderer {
    pub fn invalidate(&mut self, uid: &str) {
        self.cache.remove(uid);
    }
    pub fn point(&self, root: &Element, point: &crate::engine::TextPoint) -> Option<(Node, u32)> {
        let content = match &point.container {
            InlineContainerRef::Block(uid) | InlineContainerRef::Caption(uid) => {
                self.blocks.get(uid)?.first_element_child()?
            }
            InlineContainerRef::Cell { .. } => return super::mapping::to_dom(root, point),
        };
        let inline = if content.has_attribute("data-inline-container") {
            content
        } else {
            content.query_selector("[data-inline-container]").ok()??
        };
        super::mapping::locate(inline.as_ref(), point.offset)
    }

    pub fn render(
        &mut self,
        root: &Element,
        e: &EditorEngine,
        plan: &RenderPlan,
        composing: Option<&InlineContainerRef>,
    ) -> Result<(), JsValue> {
        let doc = root.owner_document().unwrap();
        let group = if let Some(g) = root.first_element_child() {
            g
        } else {
            let g = element(&doc, "div", "dx-block-group")?;
            append(root, &g)?;
            g
        };
        if plan.full || plan.structure_changed || self.blocks.is_empty() {
            let mut live = BTreeSet::new();
            self.group(
                &doc,
                &group,
                model::children(e.document(), None)
                    .map(Vec::as_slice)
                    .unwrap_or_default(),
                e,
                composing,
                &mut live,
            )?;
            self.blocks.retain(|uid, _| live.contains(uid));
            self.cache.retain(|uid, _| live.contains(uid));
        } else {
            for uid in &plan.dirty {
                if let (Some(block), Some(el)) = (e.block(uid), self.blocks.get(uid).cloned()) {
                    self.patch(&doc, &el, block, e, composing)?;
                }
            }
        }
        let empty = model::children(e.document(), None).is_some_and(|bs| {
            bs.len() == 1
                && bs[0]
                    .content
                    .first()
                    .is_some_and(|n| n.kind.0 == "paragraph" && n.content.is_empty())
        });
        let value = if empty { "true" } else { "false" };
        if root.get_attribute("data-empty").as_deref() != Some(value) {
            root.set_attribute("data-empty", value)?;
        }
        Ok(())
    }
    fn group(
        &mut self,
        doc: &Document,
        group: &Element,
        nodes: &[ComponentNode],
        e: &EditorEngine,
        composing: Option<&InlineContainerRef>,
        live: &mut BTreeSet<String>,
    ) -> Result<(), JsValue> {
        let mut number = 0u64;
        let mut previous_numbered = false;
        for (index, block) in nodes.iter().enumerate() {
            let Some(uid) = block.id.as_ref().map(|id| id.0.clone()) else {
                continue;
            };
            live.insert(uid.clone());
            let el = if let Some(el) = self.blocks.get(&uid) {
                el.clone()
            } else {
                let el = element(doc, "div", "dx-block")?;
                el.set_attribute("data-uid", &uid)?;
                self.blocks.insert(uid.clone(), el.clone());
                el
            };
            let at = group.children().item(index as u32);
            if at.as_ref() != Some(&el) {
                group.insert_before(&el, at.as_ref().map(|e| e.as_ref()))?;
            }
            self.patch(doc, &el, block, e, composing)?;
            let kind = block
                .content
                .first()
                .map(|c| c.kind.0.as_str())
                .unwrap_or_default();
            if kind == "numbered_list_item" {
                let start = block.content[0].attrs.get("start").and_then(|v| v.as_u64());
                number = start.unwrap_or(if previous_numbered { number + 1 } else { 1 });
                el.set_attribute("data-list", "numbered")?;
                el.set_attribute("data-number", &format!("{number}."))?;
            } else if kind == "bulleted_list_item" {
                el.set_attribute("data-list", "bullet")?;
                el.remove_attribute("data-number")?;
            } else {
                el.remove_attribute("data-list")?;
                el.remove_attribute("data-number")?;
            }
            previous_numbered = kind == "numbered_list_item";
            let nested = block.content.iter().find(|c| c.kind.0 == "block_children");
            let old = el.query_selector(":scope > [data-children]")?;
            if let Some(children) = nested {
                let group = if let Some(g) = old {
                    g
                } else {
                    let g = element(doc, "div", "dx-block-group")?;
                    g.set_attribute("data-children", "")?;
                    append(&el, &g)?;
                    g
                };
                group.set_id(&format!("dx-toggle-children-{uid}"));
                if e.is_collapsed(&uid) {
                    group.set_attribute("hidden", "")?;
                } else {
                    group.remove_attribute("hidden")?;
                }
                self.group(doc, &group, &children.content, e, composing, live)?;
            } else if let Some(old) = old {
                park_islands(&old);
                old.remove();
            }
        }
        while group.children().length() > nodes.len() as u32 {
            if let Some(last) = group.last_element_child() {
                park_islands(&last);
                last.remove();
            } else {
                break;
            }
        }
        Ok(())
    }
    fn patch(
        &mut self,
        doc: &Document,
        block_el: &Element,
        block: &ComponentNode,
        e: &EditorEngine,
        composing: Option<&InlineContainerRef>,
    ) -> Result<(), JsValue> {
        let Some(uid) = block.id.as_ref().map(|id| id.0.clone()) else {
            return Ok(());
        };
        let Some(content) = block.content.first() else {
            return Ok(());
        };
        if block_el.get_attribute("data-block-kind").as_deref() != Some(&content.kind.0) {
            block_el.set_attribute("data-block-kind", &content.kind.0)?;
        }
        if let Some(composing) = composing {
            let belongs = match composing {
                InlineContainerRef::Block(id) | InlineContainerRef::Caption(id) => id == &uid,
                InlineContainerRef::Cell { row, .. } => model::find(content, row).is_some(),
            };
            if belongs {
                return Ok(());
            }
        }
        let old = self.cache.get(&uid);
        let existing = block_el.first_element_child();
        if old == Some(content) && existing.is_some() {
            self.update_controls(block_el, content, e, &uid)?;
            return Ok(());
        }
        // The common typing case patches CharacterData and keeps selection/text node identity.
        if let (Some(old), Some(existing)) = (old, existing.as_ref()) {
            if old.kind == content.kind
                && old.attrs == content.attrs
                && old.content.len() == 1
                && content.content.len() == 1
                && old.content[0].kind.0 == "text"
                && content.content[0].kind.0 == "text"
                && old.content[0].marks == content.content[0].marks
            {
                let inline_element = if existing.has_attribute("data-inline-container") {
                    Some(existing.clone())
                } else {
                    existing.query_selector("[data-inline-container]")?
                };
                if let Some(target) = inline_element.and_then(|el| single_text(el.as_ref())) {
                    if let Some(text) = target.dyn_ref::<web_sys::Text>() {
                        let value = content.content[0].text.as_deref().unwrap_or_default();
                        if text.data() != value {
                            text.set_data(value);
                        }
                        self.cache.insert(uid.clone(), content.clone());
                        self.update_controls(block_el, content, e, &uid)?;
                        return Ok(());
                    }
                }
            }
        }
        // Island roots are Dioxus-owned; changing attrs must never detach their dialogs/inputs.
        if crate::engine::identity::is_island_kind(&content.kind.0)
            && old.is_some_and(|n| n.kind == content.kind)
            && existing.is_some()
        {
            self.cache.insert(uid, content.clone());
            return Ok(());
        }
        let rendered = self.content(doc, content, &uid, e)?;
        if let Some(old) = existing.filter(|el| !el.has_attribute("data-children")) {
            park_islands(&old);
            block_el.replace_child(&rendered, &old)?;
        } else {
            block_el.insert_before(&rendered, block_el.first_child().as_ref())?;
        }
        self.cache.insert(uid.clone(), content.clone());
        self.update_controls(block_el, content, e, &uid)?;
        Ok(())
    }
    fn update_controls(
        &self,
        block: &Element,
        n: &ComponentNode,
        e: &EditorEngine,
        uid: &str,
    ) -> Result<(), JsValue> {
        if n.kind.0 == "code_block" {
            if let Some(inline) = block.query_selector("[data-inline-container]")? {
                let trailing = inline
                    .last_element_child()
                    .filter(|element| element.tag_name() == "BR");
                if n.text_content().ends_with('\n') {
                    if trailing.is_none() {
                        if let Some(document) = inline.owner_document() {
                            append(&inline, &element(&document, "br", "")?)?;
                        }
                    }
                } else if !n.content.is_empty() {
                    if let Some(trailing) = trailing {
                        trailing.remove();
                    }
                }
            }
        }
        if n.kind.0 == "todo" {
            let checked = n
                .attrs
                .get("checked")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            block.set_attribute("data-checked", if checked { "true" } else { "false" })?;
            if let Some(input) = block
                .query_selector("input[type=checkbox]")?
                .and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok())
            {
                input.set_checked(checked);
                input.set_disabled(e.read_only());
                input.set_attribute(
                    "aria-label",
                    &format!(
                        "Mark {} {}",
                        n.text_content(),
                        if checked { "incomplete" } else { "complete" }
                    ),
                )?;
            }
        }
        if n.kind.0 == "toggle" {
            if let Some(button) = block.query_selector("button[data-toggle]")? {
                button.set_attribute(
                    "aria-expanded",
                    if e.is_collapsed(uid) { "false" } else { "true" },
                )?;
                button.set_text_content(Some(if e.is_collapsed(uid) { "▸" } else { "▾" }));
            }
        }
        if let Some(container) = block.query_selector("[data-inline-container]")? {
            let empty = n.content.is_empty();
            if empty {
                let hint = match n.kind.0.as_str() {
                    "heading" => format!(
                        "Heading {}",
                        n.attrs.get("level").and_then(|v| v.as_u64()).unwrap_or(1)
                    ),
                    "image" => "Write a caption…".into(),
                    "paragraph" => {
                        if e.profile() == crate::engine::Profile::Composer {
                            "Message…".into()
                        } else {
                            "Type '/' for commands".into()
                        }
                    }
                    "todo" => "To-do".into(),
                    other => other.replace('_', " "),
                };
                container.set_attribute("data-placeholder", &hint)?;
            } else {
                container.remove_attribute("data-placeholder")?;
            }
        }
        Ok(())
    }
    fn content(
        &self,
        doc: &Document,
        n: &ComponentNode,
        uid: &str,
        e: &EditorEngine,
    ) -> Result<Element, JsValue> {
        let kind = n.kind.0.as_str();
        let el = match kind {
            "table" => {
                let wrap = element(doc, "div", "dx-table-wrapper")?;
                let table = element(doc, "table", "")?;
                let cols = element(doc, "colgroup", "")?;
                if let Some(widths) = n.attrs.get("column_widths").and_then(|v| v.as_array()) {
                    for width in widths {
                        let col = element(doc, "col", "")?;
                        if let Some(width) = width.as_f64() {
                            col.set_attribute("style", &format!("width:{width}px"))?;
                        }
                        append(&cols, &col)?;
                    }
                }
                append(&table, &cols)?;
                let body = element(doc, "tbody", "")?;
                for (ri, row) in n.content.iter().enumerate() {
                    let tr = element(doc, "tr", "")?;
                    let row_uid = row.id.as_ref().map(|i| i.0.as_str()).unwrap_or_default();
                    tr.set_attribute("data-uid", row_uid)?;
                    for (ci, cell) in row.content.iter().enumerate() {
                        let header = (ri == 0
                            && n.attrs
                                .get("header_row")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false))
                            || (ci == 0
                                && n.attrs
                                    .get("header_column")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false));
                        let td = element(
                            doc,
                            if header || cell.kind.0 == "table_header_cell" {
                                "th"
                            } else {
                                "td"
                            },
                            "",
                        )?;
                        attrs(&td, cell)?;
                        if let Some(align) = cell.attrs.get("alignment").and_then(|v| v.as_str()) {
                            if matches!(align, "left" | "center" | "right") {
                                td.set_attribute("style", &format!("text-align:{align}"))?;
                            }
                        }
                        bind(
                            &td,
                            InlineContainerRef::Cell {
                                row: row_uid.into(),
                                col: ci,
                            },
                        )?;
                        render_inline(doc, &td, &cell.content)?;
                        append(&tr, &td)?;
                    }
                    append(&body, &tr)?;
                }
                append(&table, &body)?;
                append(&wrap, &table)?;
                wrap
            }
            "image" => {
                let figure = element(doc, "figure", "dx-doc-image")?;
                attrs(&figure, n)?;
                if let Some(w) = n.attrs.get("width").and_then(|v| v.as_f64()) {
                    figure.set_attribute("style", &format!("width:{}%", w.clamp(25., 100.)))?;
                }
                let img = element(doc, "img", "")?;
                img.set_attribute("contenteditable", "false")?;
                img.set_attribute(
                    "alt",
                    n.attrs
                        .get("alt")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default(),
                )?;
                if let Some(url) = n
                    .attrs
                    .get("media")
                    .and_then(|m| m.get("url"))
                    .and_then(|v| v.as_str())
                    .filter(|u| crate::is_safe_media_url(u))
                {
                    img.set_attribute("src", url)?;
                }
                append(&figure, &img)?;
                let status = element(doc, "div", "dx-doc-image-upload")?;
                status.set_attribute("contenteditable", "false")?;
                if n.attrs.get("media").is_none() {
                    status.set_text_content(Some("Upload interrupted"));
                }
                append(&figure, &status)?;
                let caption = element(doc, "figcaption", "dx-doc-image-caption")?;
                if e.profile() == crate::engine::Profile::MarkdownNotes {
                    caption.set_attribute("contenteditable", "false")?;
                    caption.set_attribute("hidden", "")?;
                } else {
                    bind(&caption, InlineContainerRef::Caption(uid.into()))?;
                    render_inline(doc, &caption, &n.content)?;
                }
                append(&figure, &caption)?;
                figure
            }
            "divider" => {
                let el = element(doc, "hr", "")?;
                el.set_attribute("contenteditable", "false")?;
                el
            }
            "opaque_markdown" => {
                let el = element(doc, "pre", "dxeditor-engine__opaque")?;
                el.set_attribute("contenteditable", "false")?;
                el.set_text_content(Some(
                    n.attrs
                        .get("source")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unsupported Markdown"),
                ));
                el
            }
            kind if crate::engine::identity::is_island_kind(kind) => {
                let el = element(doc, "div", "dxeditor-island")?;
                el.set_attribute("contenteditable", "false")?;
                el.set_attribute("data-island-uid", uid)?;
                el.set_text_content(Some("Loading block…"));
                el
            }
            _ => {
                let tag = match kind {
                    "heading" => match n.attrs.get("level").and_then(|v| v.as_u64()).unwrap_or(1) {
                        1 => "h1",
                        2 => "h2",
                        3 => "h3",
                        4 => "h4",
                        5 => "h5",
                        _ => "h6",
                    },
                    "paragraph" => "p",
                    "quote" => "blockquote",
                    "callout" => "aside",
                    "code_block" => "pre",
                    _ => "div",
                };
                let outer = element(
                    doc,
                    tag,
                    if matches!(kind, "todo" | "toggle") {
                        "dx-block-control-content"
                    } else {
                        ""
                    },
                )?;
                let text = if matches!(kind, "todo" | "toggle") {
                    let label = element(
                        doc,
                        if kind == "todo" { "label" } else { "span" },
                        "dx-block-native-control",
                    )?;
                    label.set_attribute("contenteditable", "false")?;
                    let control =
                        element(doc, if kind == "todo" { "input" } else { "button" }, "")?;
                    if kind == "todo" {
                        control.set_attribute("type", "checkbox")?;
                        control.set_attribute("data-todo", uid)?;
                    } else {
                        control.set_attribute("type", "button")?;
                        control.set_attribute("data-toggle", uid)?;
                        control.set_attribute("aria-label", "Toggle children")?;
                        control
                            .set_attribute("aria-controls", &format!("dx-toggle-children-{uid}"))?;
                    }
                    append(&label, &control)?;
                    append(&outer, &label)?;
                    let text = element(doc, "div", "dx-block-control-text")?;
                    append(&outer, &text)?;
                    text
                } else if kind == "code_block" {
                    let code = element(doc, "code", "")?;
                    if let Some(lang) = n.attrs.get("language").and_then(|v| v.as_str()) {
                        code.set_attribute("data-language", lang)?;
                    }
                    append(&outer, &code)?;
                    code
                } else {
                    outer.clone()
                };
                bind(&text, InlineContainerRef::Block(uid.into()))?;
                render_inline(doc, &text, &n.content)?;
                if kind == "code_block" && n.text_content().ends_with('\n') {
                    append(&text, &element(doc, "br", "")?)?;
                }
                outer
            }
        };
        el.set_attribute("data-content-kind", kind)?;
        attrs(&el, n)?;
        let _ = e;
        Ok(el)
    }
}
fn single_text(node: &Node) -> Option<Node> {
    fn collect(node: &Node, found: &mut Option<Node>) -> bool {
        if node.node_type() == Node::TEXT_NODE {
            if found.is_some() {
                return false;
            }
            *found = Some(node.clone());
            return true;
        }
        let children = node.child_nodes();
        (0..children.length()).all(|index| {
            children
                .item(index)
                .is_some_and(|child| collect(&child, found))
        })
    }
    let mut found = None;
    if collect(node, &mut found) {
        found
    } else {
        None
    }
}
fn park_islands(element: &Element) {
    super::islands::park_descendants(element);
}
fn mark(doc: &Document, m: &ComponentMark) -> Result<Option<Element>, JsValue> {
    let tag = match m.kind.0.as_str() {
        "bold" => "strong",
        "italic" => "em",
        "underline" => "u",
        "strike" => "s",
        "code" => "code",
        "highlight" | "text_color" => "span",
        "link" | "entity_link" => "a",
        _ => return Ok(None),
    };
    let el = element(doc, tag, "")?;
    match m.kind.0.as_str() {
        "highlight" | "text_color" => {
            if let Some(color) = m
                .attrs
                .get("color")
                .and_then(|v| v.as_str())
                .filter(|c| crate::profiles::block_document::COLOR_TOKENS.contains(c))
            {
                el.set_attribute(
                    if m.kind.0 == "highlight" {
                        "data-highlight"
                    } else {
                        "data-color"
                    },
                    color,
                )?;
            }
        }
        "link" => {
            if let Some(href) = m
                .attrs
                .get("href")
                .and_then(|v| v.as_str())
                .filter(|u| crate::is_safe_url(u))
            {
                el.set_attribute("href", href)?;
            }
            if let Some(title) = m.attrs.get("title").and_then(|v| v.as_str()) {
                el.set_attribute("title", title)?;
            }
        }
        "entity_link" => {
            if let Some(id) = m.attrs.get("entity_id").and_then(|v| v.as_str()) {
                el.set_attribute("href", &format!("semantic:entity:{id}"))?;
                el.set_attribute("data-entity-id", id)?;
                el.set_class_name("dxeditor-engine__entity-link");
            }
        }
        _ => {}
    }
    Ok(Some(el))
}
pub fn render_inline(
    doc: &Document,
    parent: &Element,
    nodes: &[ComponentNode],
) -> Result<(), JsValue> {
    if nodes.is_empty() {
        append(parent, &element(doc, "br", "")?)?;
        return Ok(());
    }
    for (i, n) in nodes.iter().enumerate() {
        if n.kind.0 == "text" {
            let mut at = parent.clone();
            for m in &n.marks {
                if let Some(wrapper) = mark(doc, m)? {
                    append(&at, &wrapper)?;
                    at = wrapper;
                }
            }
            at.append_child(&doc.create_text_node(n.text.as_deref().unwrap_or_default()))?;
            continue;
        }
        if i == 0 || nodes[i - 1].kind.0 != "text" {
            parent.append_child(&doc.create_text_node(ZW))?;
        }
        let atom = element(
            doc,
            match n.kind.0.as_str() {
                "hard_break" => "br",
                "mention" => "a",
                "opaque_markdown_inline" => "code",
                _ => "span",
            },
            "",
        )?;
        atom.set_attribute("data-inline-atom", &n.kind.0)?;
        if n.kind.0 != "hard_break" {
            atom.set_attribute("contenteditable", "false")?;
            atom.set_attribute("tabindex", "0")?;
        }
        match n.kind.0.as_str() {
            "mention" => {
                let id = n
                    .attrs
                    .get("entity_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                atom.set_attribute("data-semantic-mention", id)?;
                atom.set_attribute("href", &format!("semantic:entity:{id}"))?;
                atom.set_class_name("dxeditor-engine__mention dxeditor-engine__entity-link");
                atom.set_text_content(Some(&format!(
                    "@{}",
                    n.attrs.get("label").and_then(|v| v.as_str()).unwrap_or(id)
                )));
            }
            "chip" => {
                atom.set_class_name("dx-composer-chip");
                atom.set_attribute(
                    "data-composer-chip",
                    &serde_json::to_string(&n.attrs).unwrap(),
                )?;
                let button = element(doc, "button", "")?;
                button.set_attribute("type", "button")?;
                button.set_attribute("data-chip-activate", "")?;
                button.set_attribute(
                    "aria-label",
                    &format!(
                        "Open {}",
                        n.attrs
                            .get("label")
                            .and_then(|v| v.as_str())
                            .unwrap_or("chip")
                    ),
                )?;
                button.set_text_content(Some(
                    n.attrs
                        .get("label")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default(),
                ));
                append(&atom, &button)?;
                let remove = element(doc, "button", "")?;
                remove.set_attribute("type", "button")?;
                remove.set_attribute("data-chip-remove", "")?;
                remove.set_attribute(
                    "aria-label",
                    &format!(
                        "Remove {}",
                        n.attrs
                            .get("label")
                            .and_then(|v| v.as_str())
                            .unwrap_or("chip")
                    ),
                )?;
                remove.set_text_content(Some("×"));
                append(&atom, &remove)?;
            }
            "opaque_markdown_inline" => {
                atom.set_class_name("dxeditor-engine__opaque-inline");
                atom.set_text_content(Some(
                    n.attrs
                        .get("source")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unsupported"),
                ));
            }
            _ => {}
        }
        append(parent, &atom)?;
        if i + 1 == nodes.len() || nodes[i + 1].kind.0 != "text" {
            parent.append_child(&doc.create_text_node(ZW))?;
        }
    }
    Ok(())
}
