use std::collections::VecDeque;

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde_json::{Map, Value};

use crate::document::{
    BlockNode, COMPONENT_CODE, COMPONENT_DIVIDER, COMPONENT_HEADING, COMPONENT_LINK,
    COMPONENT_LIST, COMPONENT_LIST_ITEM, COMPONENT_MENTION, COMPONENT_PARAGRAPH, COMPONENT_QUOTE,
    COMPONENT_TABLE, EditorDocument, InlineNode, Mark, NodeContent, TableCell, TableNode, TableRow,
};

const COMPONENT_IMAGE: &str = "image";
const COMPONENT_RAW_HTML: &str = "raw_html";
const COMPONENT_HARD_BREAK: &str = "hard_break";
const COMPONENT_TASK_MARKER: &str = "task_marker";
const MARK_STRIKETHROUGH: &str = "strikethrough";

struct MarkdownParser {
    events: VecDeque<Event<'static>>,
    block_index: usize,
    inline_index: usize,
}

impl MarkdownParser {
    fn new(markdown: &str) -> Self {
        let options =
            Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH;
        Self {
            events: Parser::new_ext(markdown, options)
                .map(Event::into_static)
                .collect(),
            block_index: 1,
            inline_index: 1,
        }
    }

    fn block_id(&mut self) -> String {
        let id = format!("block-{}", self.block_index);
        self.block_index += 1;
        id
    }

    fn inline_node(&mut self, component: &str, text: impl Into<String>) -> InlineNode {
        let node = InlineNode {
            id: format!("text-{}", self.inline_index).into(),
            component: component.to_string(),
            attrs: Map::new(),
            text: text.into(),
            marks: Vec::new(),
        };
        self.inline_index += 1;
        node
    }

    fn parse_blocks(&mut self, end: Option<TagEnd>) -> Vec<BlockNode> {
        let mut blocks = Vec::new();
        loop {
            if matches!(self.events.front(), Some(Event::End(found)) if Some(*found) == end) {
                self.events.pop_front();
                break;
            }
            let Some(event) = self.events.pop_front() else {
                break;
            };
            match event {
                Event::Start(Tag::Paragraph) => {
                    let inline = self.parse_inlines(TagEnd::Paragraph);
                    let id = self.block_id();
                    blocks.push(BlockNode::paragraph(id, inline));
                }
                Event::Start(Tag::Heading { level, .. }) => {
                    let inline = self.parse_inlines(TagEnd::Heading(level));
                    let id = self.block_id();
                    blocks.push(BlockNode::heading(id, heading_level(level), inline));
                }
                Event::Start(Tag::BlockQuote(kind)) => {
                    let content = self.parse_blocks(Some(TagEnd::BlockQuote(kind)));
                    let id = self.block_id();
                    blocks.push(BlockNode::new(
                        id,
                        COMPONENT_QUOTE,
                        Map::new(),
                        NodeContent::Blocks(content),
                    ));
                }
                Event::Start(Tag::CodeBlock(kind)) => blocks.push(self.parse_code_block(kind)),
                Event::Start(Tag::List(start)) => blocks.push(self.parse_list(start)),
                Event::Start(Tag::Table(alignments)) => {
                    blocks.push(self.parse_table(alignments));
                }
                Event::Start(Tag::HtmlBlock) => blocks.push(self.parse_html_block()),
                Event::Rule => {
                    let id = self.block_id();
                    blocks.push(BlockNode::new(
                        id,
                        COMPONENT_DIVIDER,
                        Map::new(),
                        NodeContent::Void,
                    ));
                }
                Event::Text(text) | Event::Code(text) => {
                    let node = self.inline_node("text", text.into_string());
                    let id = self.block_id();
                    blocks.push(BlockNode::paragraph(id, vec![node]));
                }
                Event::Html(html) | Event::InlineHtml(html) => {
                    let node = self.inline_node(COMPONENT_RAW_HTML, html.into_string());
                    let id = self.block_id();
                    blocks.push(BlockNode::new(
                        id,
                        COMPONENT_RAW_HTML,
                        Map::new(),
                        NodeContent::Inline(vec![node]),
                    ));
                }
                Event::InlineMath(math) => {
                    let node = self.inline_node("text", format!("${math}$"));
                    let id = self.block_id();
                    blocks.push(BlockNode::paragraph(id, vec![node]));
                }
                Event::DisplayMath(math) => {
                    let node = self.inline_node("text", format!("$$\n{math}\n$$"));
                    let id = self.block_id();
                    blocks.push(BlockNode::paragraph(id, vec![node]));
                }
                Event::FootnoteReference(label) => {
                    let node = self.inline_node("text", format!("[^{label}]"));
                    let id = self.block_id();
                    blocks.push(BlockNode::paragraph(id, vec![node]));
                }
                Event::SoftBreak | Event::HardBreak => {
                    let node = self.inline_node("text", "\n");
                    let id = self.block_id();
                    blocks.push(BlockNode::paragraph(id, vec![node]));
                }
                Event::TaskListMarker(checked) => {
                    let mut node = self.inline_node(COMPONENT_TASK_MARKER, "");
                    node.attrs
                        .insert("checked".to_string(), Value::Bool(checked));
                    let id = self.block_id();
                    blocks.push(BlockNode::paragraph(id, vec![node]));
                }
                Event::Start(tag) => blocks.push(self.preserve_unknown_block(tag)),
                Event::End(_) => break,
            }
        }
        blocks
    }

    fn parse_inlines(&mut self, end: TagEnd) -> Vec<InlineNode> {
        let mut inline = Vec::new();
        let mut marks = Vec::new();
        while let Some(event) = self.events.pop_front() {
            match event {
                Event::End(found) if found == end => break,
                Event::Start(Tag::Strong) => marks.push(Mark::new("bold")),
                Event::End(TagEnd::Strong) => remove_mark(&mut marks, "bold"),
                Event::Start(Tag::Emphasis) => marks.push(Mark::new("italic")),
                Event::End(TagEnd::Emphasis) => remove_mark(&mut marks, "italic"),
                Event::Start(Tag::Strikethrough) => marks.push(Mark::new(MARK_STRIKETHROUGH)),
                Event::End(TagEnd::Strikethrough) => {
                    remove_mark(&mut marks, MARK_STRIKETHROUGH);
                }
                Event::Start(Tag::Link {
                    dest_url, title, ..
                }) => {
                    let mut mark = Mark::link(dest_url.into_string());
                    if !title.is_empty() {
                        mark.attrs
                            .insert("title".to_string(), Value::String(title.into_string()));
                    }
                    marks.push(mark);
                }
                Event::End(TagEnd::Link) => remove_mark(&mut marks, COMPONENT_LINK),
                Event::Start(Tag::Image {
                    dest_url, title, ..
                }) => {
                    let alt = self
                        .parse_inlines(TagEnd::Image)
                        .into_iter()
                        .map(|node| node.text)
                        .collect::<String>();
                    let mut node = self.inline_node(COMPONENT_IMAGE, alt);
                    node.attrs
                        .insert("src".to_string(), Value::String(dest_url.into_string()));
                    if !title.is_empty() {
                        node.attrs
                            .insert("title".to_string(), Value::String(title.into_string()));
                    }
                    node.marks.clone_from(&marks);
                    inline.push(node);
                }
                Event::Text(text) => self.push_marked_text(&mut inline, &marks, &text),
                Event::Code(text) => {
                    let mut code_marks = marks.clone();
                    code_marks.push(Mark::new("code"));
                    self.push_marked_text(&mut inline, &code_marks, &text);
                }
                Event::Html(html) | Event::InlineHtml(html) => {
                    let mut node = self.inline_node(COMPONENT_RAW_HTML, html.into_string());
                    node.marks.clone_from(&marks);
                    inline.push(node);
                }
                Event::SoftBreak => self.push_marked_text(&mut inline, &marks, "\n"),
                Event::HardBreak => {
                    let mut node = self.inline_node(COMPONENT_HARD_BREAK, "\n");
                    node.marks.clone_from(&marks);
                    inline.push(node);
                }
                Event::TaskListMarker(checked) => {
                    let mut node = self.inline_node(COMPONENT_TASK_MARKER, "");
                    node.attrs
                        .insert("checked".to_string(), Value::Bool(checked));
                    inline.push(node);
                }
                Event::InlineMath(math) => {
                    self.push_marked_text(&mut inline, &marks, &format!("${math}$"));
                }
                Event::DisplayMath(math) => {
                    self.push_marked_text(&mut inline, &marks, &format!("$$\n{math}\n$$"));
                }
                Event::FootnoteReference(label) => {
                    self.push_marked_text(&mut inline, &marks, &format!("[^{label}]"));
                }
                Event::Rule => self.push_marked_text(&mut inline, &marks, "---"),
                Event::Start(Tag::Superscript) => marks.push(Mark::new("superscript")),
                Event::End(TagEnd::Superscript) => remove_mark(&mut marks, "superscript"),
                Event::Start(Tag::Subscript) => marks.push(Mark::new("subscript")),
                Event::End(TagEnd::Subscript) => remove_mark(&mut marks, "subscript"),
                Event::Start(_) | Event::End(_) => {}
            }
        }
        inline
    }

    fn push_marked_text(&mut self, inline: &mut Vec<InlineNode>, marks: &[Mark], text: &str) {
        if text.is_empty() {
            return;
        }
        let link = marks
            .iter()
            .find(|mark| mark.component == COMPONENT_LINK)
            .and_then(|mark| mark.attrs.get("href"))
            .and_then(Value::as_str);
        if let Some(entity_id) = link.and_then(|href| href.strip_prefix("semantic:entity:"))
            && let Some(label) = text.strip_prefix('@')
        {
            let mut node =
                InlineNode::mention(format!("mention-{}", self.inline_index), entity_id, label);
            self.inline_index += 1;
            node.marks.extend(
                marks
                    .iter()
                    .filter(|mark| mark.component != COMPONENT_LINK)
                    .cloned(),
            );
            inline.push(node);
            return;
        }

        let mut node = self.inline_node("text", text);
        node.marks.extend_from_slice(marks);
        inline.push(node);
    }

    fn parse_code_block(&mut self, kind: CodeBlockKind<'static>) -> BlockNode {
        let mut text = String::new();
        while let Some(event) = self.events.pop_front() {
            match event {
                Event::End(TagEnd::CodeBlock) => break,
                Event::Text(value)
                | Event::Code(value)
                | Event::Html(value)
                | Event::InlineHtml(value) => text.push_str(&value),
                Event::SoftBreak | Event::HardBreak => text.push('\n'),
                Event::InlineMath(value) | Event::DisplayMath(value) => text.push_str(&value),
                Event::FootnoteReference(value) => {
                    text.push_str("[^");
                    text.push_str(&value);
                    text.push(']');
                }
                Event::TaskListMarker(checked) => {
                    text.push_str(if checked { "[x] " } else { "[ ] " });
                }
                Event::Rule => text.push_str("---"),
                Event::Start(_) | Event::End(_) => {}
            }
        }
        let text = text.strip_suffix('\n').unwrap_or(&text).to_string();
        let mut attrs = Map::new();
        if let CodeBlockKind::Fenced(info) = kind
            && let Some(language) = info
                .split_whitespace()
                .next()
                .filter(|value| !value.is_empty())
        {
            attrs.insert("language".to_string(), Value::String(language.to_string()));
        }
        let node = self.inline_node("text", text);
        let id = self.block_id();
        BlockNode::new(id, COMPONENT_CODE, attrs, NodeContent::Inline(vec![node]))
    }

    fn parse_html_block(&mut self) -> BlockNode {
        let mut html = String::new();
        while let Some(event) = self.events.pop_front() {
            match event {
                Event::End(TagEnd::HtmlBlock) => break,
                Event::Html(value) | Event::InlineHtml(value) | Event::Text(value) => {
                    html.push_str(&value);
                }
                Event::SoftBreak | Event::HardBreak => html.push('\n'),
                Event::Code(value) | Event::InlineMath(value) | Event::DisplayMath(value) => {
                    html.push_str(&value);
                }
                Event::FootnoteReference(value) => html.push_str(&format!("[^{value}]")),
                Event::TaskListMarker(checked) => {
                    html.push_str(if checked { "[x] " } else { "[ ] " });
                }
                Event::Rule => html.push_str("---"),
                Event::Start(_) | Event::End(_) => {}
            }
        }
        let node = self.inline_node(COMPONENT_RAW_HTML, html);
        let id = self.block_id();
        BlockNode::new(
            id,
            COMPONENT_RAW_HTML,
            Map::new(),
            NodeContent::Inline(vec![node]),
        )
    }

    fn parse_list(&mut self, start: Option<u64>) -> BlockNode {
        let mut items = Vec::new();
        loop {
            match self.events.front() {
                Some(Event::End(TagEnd::List(_))) => {
                    self.events.pop_front();
                    break;
                }
                Some(Event::Start(Tag::Item)) => {
                    self.events.pop_front();
                    let mut blocks = self.parse_blocks(Some(TagEnd::Item));
                    let checked = take_task_marker(&mut blocks);
                    let mut attrs = Map::new();
                    if let Some(checked) = checked {
                        attrs.insert("checked".to_string(), Value::Bool(checked));
                    }
                    let id = self.block_id();
                    items.push(BlockNode::new(
                        id,
                        COMPONENT_LIST_ITEM,
                        attrs,
                        NodeContent::Blocks(blocks),
                    ));
                }
                None => break,
                _ => {
                    self.events.pop_front();
                }
            }
        }
        let mut attrs = Map::new();
        attrs.insert("ordered".to_string(), Value::Bool(start.is_some()));
        if let Some(start) = start {
            attrs.insert("start".to_string(), Value::from(start));
        }
        let id = self.block_id();
        BlockNode::new(id, COMPONENT_LIST, attrs, NodeContent::Blocks(items))
    }

    fn parse_table(&mut self, alignments: Vec<Alignment>) -> BlockNode {
        let mut rows = Vec::new();
        let mut in_header = false;
        loop {
            match self.events.pop_front() {
                Some(Event::Start(Tag::TableHead)) => {
                    rows.push(self.parse_table_cells(TagEnd::TableHead, true, &alignments));
                    in_header = false;
                }
                Some(Event::End(TagEnd::TableHead)) => in_header = false,
                Some(Event::Start(Tag::TableRow)) => {
                    rows.push(self.parse_table_cells(TagEnd::TableRow, in_header, &alignments));
                }
                Some(Event::End(TagEnd::Table)) | None => break,
                Some(_) => {}
            }
        }
        let mut attrs = Map::new();
        attrs.insert(
            "alignments".to_string(),
            Value::Array(
                alignments
                    .iter()
                    .map(|alignment| Value::String(alignment_name(*alignment).to_string()))
                    .collect(),
            ),
        );
        let id = self.block_id();
        BlockNode::new(
            id,
            COMPONENT_TABLE,
            attrs,
            NodeContent::Table(TableNode { rows }),
        )
    }

    fn parse_table_cells(
        &mut self,
        end: TagEnd,
        header: bool,
        alignments: &[Alignment],
    ) -> TableRow {
        let mut cells = Vec::new();
        loop {
            match self.events.pop_front() {
                Some(Event::Start(Tag::TableCell)) => {
                    let inline = self.parse_inlines(TagEnd::TableCell);
                    let column = cells.len();
                    let mut attrs = Map::new();
                    attrs.insert("header".to_string(), Value::Bool(header));
                    if let Some(alignment) = alignments.get(column) {
                        attrs.insert(
                            "alignment".to_string(),
                            Value::String(alignment_name(*alignment).to_string()),
                        );
                    }
                    let paragraph_id = self.block_id();
                    cells.push(TableCell {
                        id: format!("cell-{}-{}", self.block_index, column + 1).into(),
                        attrs,
                        blocks: vec![BlockNode::paragraph(paragraph_id, inline)],
                    });
                }
                Some(Event::End(found)) if found == end => break,
                None => break,
                Some(_) => {}
            }
        }
        let mut attrs = Map::new();
        attrs.insert("header".to_string(), Value::Bool(header));
        TableRow {
            id: format!("row-{}", self.block_index).into(),
            attrs,
            cells,
        }
    }

    fn preserve_unknown_block(&mut self, tag: Tag<'static>) -> BlockNode {
        let end = tag.to_end();
        let inline = self.parse_inlines(end);
        let id = self.block_id();
        BlockNode::new(
            id,
            "markdown_unknown",
            Map::new(),
            NodeContent::Inline(inline),
        )
    }
}

pub fn parse_markdown(markdown: &str) -> EditorDocument {
    let mut parser = MarkdownParser::new(markdown);
    let blocks = parser.parse_blocks(None);
    EditorDocument::new(blocks)
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn alignment_name(alignment: Alignment) -> &'static str {
    match alignment {
        Alignment::None => "none",
        Alignment::Left => "left",
        Alignment::Center => "center",
        Alignment::Right => "right",
    }
}

fn remove_mark(marks: &mut Vec<Mark>, component: &str) {
    if let Some(index) = marks.iter().rposition(|mark| mark.component == component) {
        marks.remove(index);
    }
}

fn take_task_marker(blocks: &mut Vec<BlockNode>) -> Option<bool> {
    let NodeContent::Inline(inline) = &mut blocks.first_mut()?.content else {
        return None;
    };
    let marker = inline.first()?;
    if marker.component != COMPONENT_TASK_MARKER {
        return None;
    }
    let checked = marker.attrs.get("checked").and_then(Value::as_bool)?;
    inline.remove(0);
    if inline.is_empty() {
        blocks.remove(0);
    }
    Some(checked)
}

pub fn serialize_markdown(document: &EditorDocument) -> String {
    document
        .blocks
        .iter()
        .map(serialize_block)
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn serialize_block(block: &BlockNode) -> String {
    match block.component.as_str() {
        COMPONENT_HEADING => {
            let level = block
                .attrs
                .get("level")
                .and_then(Value::as_u64)
                .unwrap_or(1)
                .clamp(1, 6) as usize;
            format!(
                "{} {}",
                "#".repeat(level),
                serialize_inline_content(&block.content)
            )
        }
        COMPONENT_QUOTE => serialize_inline_content(&block.content)
            .lines()
            .map(|line| format!("> {line}"))
            .collect::<Vec<_>>()
            .join("\n"),
        COMPONENT_CODE => {
            let language = block
                .attrs
                .get("language")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let content = raw_text_content(&block.content);
            let longest = longest_backtick_run(&content);
            let fence = "`".repeat((longest + 1).max(3));
            format!("{fence}{language}\n{content}\n{fence}")
        }
        COMPONENT_DIVIDER => "---".to_string(),
        COMPONENT_LIST => serialize_list(block),
        COMPONENT_TABLE => serialize_table(block),
        COMPONENT_RAW_HTML => raw_text_content(&block.content),
        COMPONENT_PARAGRAPH => serialize_inline_content(&block.content),
        _ => serialize_inline_content(&block.content),
    }
}

fn serialize_list(block: &BlockNode) -> String {
    let NodeContent::Blocks(items) = &block.content else {
        return serialize_inline_content(&block.content);
    };
    let ordered = block
        .attrs
        .get("ordered")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let start = block
        .attrs
        .get("start")
        .and_then(Value::as_u64)
        .unwrap_or(1);
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let marker = if ordered {
                format!("{}. ", start + index as u64)
            } else {
                "- ".to_string()
            };
            let task = item
                .attrs
                .get("checked")
                .and_then(Value::as_bool)
                .map(|checked| if checked { "[x] " } else { "[ ] " })
                .unwrap_or_default();
            let content = serialize_inline_content(&item.content);
            let mut lines = content.lines();
            let first = lines.next().unwrap_or_default();
            let mut output = format!("{marker}{task}{first}");
            for line in lines {
                output.push('\n');
                if !line.is_empty() {
                    output.push_str("    ");
                    output.push_str(line);
                }
            }
            output
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn serialize_table(block: &BlockNode) -> String {
    let NodeContent::Table(table) = &block.content else {
        return serialize_inline_content(&block.content);
    };
    let Some(header) = table.rows.first() else {
        return String::new();
    };
    let row_text = |row: &TableRow| {
        format!(
            "| {} |",
            row.cells
                .iter()
                .map(|cell| {
                    cell.blocks
                        .iter()
                        .map(serialize_block)
                        .collect::<Vec<_>>()
                        .join(" ")
                        .replace('\n', "<br>")
                })
                .collect::<Vec<_>>()
                .join(" | ")
        )
    };
    let delimiter = format!(
        "| {} |",
        header
            .cells
            .iter()
            .map(|cell| {
                match cell
                    .attrs
                    .get("alignment")
                    .and_then(Value::as_str)
                    .unwrap_or("none")
                {
                    "left" => ":---",
                    "center" => ":---:",
                    "right" => "---:",
                    _ => "---",
                }
            })
            .collect::<Vec<_>>()
            .join(" | ")
    );
    std::iter::once(row_text(header))
        .chain(std::iter::once(delimiter))
        .chain(table.rows.iter().skip(1).map(row_text))
        .collect::<Vec<_>>()
        .join("\n")
}

fn raw_text_content(content: &NodeContent) -> String {
    match content {
        NodeContent::Inline(inline) => inline.iter().map(|node| node.text.as_str()).collect(),
        NodeContent::Blocks(blocks) => blocks
            .iter()
            .map(serialize_block)
            .collect::<Vec<_>>()
            .join("\n\n"),
        NodeContent::Table(_) => serialize_inline_content(content),
        NodeContent::Custom(value) => match value {
            Value::String(value) => value.clone(),
            other => other.to_string(),
        },
        NodeContent::Void => String::new(),
    }
}

fn serialize_inline_content(content: &NodeContent) -> String {
    match content {
        NodeContent::Inline(inline) => inline.iter().map(serialize_inline).collect(),
        NodeContent::Blocks(blocks) => blocks
            .iter()
            .map(serialize_block)
            .collect::<Vec<_>>()
            .join("\n\n"),
        NodeContent::Table(table) => serialize_table(&BlockNode::new(
            "table",
            COMPONENT_TABLE,
            Map::new(),
            NodeContent::Table(table.clone()),
        )),
        NodeContent::Custom(value) => match value {
            Value::String(value) => value.clone(),
            other => other.to_string(),
        },
        NodeContent::Void => String::new(),
    }
}

fn serialize_inline(inline: &InlineNode) -> String {
    if inline.component == COMPONENT_MENTION {
        let id = inline
            .attrs
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        return format!(
            "[@{}](semantic:entity:{})",
            escape_text(&inline.text),
            escape_destination(id)
        );
    }
    if inline.component == COMPONENT_RAW_HTML {
        return inline.text.clone();
    }
    if inline.component == COMPONENT_HARD_BREAK {
        return "  \n".to_string();
    }
    if inline.component == COMPONENT_IMAGE {
        let source = inline
            .attrs
            .get("src")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let title = inline
            .attrs
            .get("title")
            .and_then(Value::as_str)
            .map(|title| format!(" \"{}\"", escape_title(title)))
            .unwrap_or_default();
        return format!(
            "![{}]({}{})",
            escape_text(&inline.text),
            escape_destination(source),
            title
        );
    }
    if inline.component == COMPONENT_TASK_MARKER {
        return if inline
            .attrs
            .get("checked")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            "[x] ".to_string()
        } else {
            "[ ] ".to_string()
        };
    }

    let has_code = inline.marks.iter().any(|mark| mark.component == "code");
    let mut text = if has_code {
        code_span(&inline.text)
    } else {
        escape_text(&inline.text)
    };
    for mark in inline.marks.iter().rev() {
        match mark.component.as_str() {
            "bold" => text = format!("**{text}**"),
            "italic" => text = format!("*{text}*"),
            MARK_STRIKETHROUGH => text = format!("~~{text}~~"),
            "superscript" => text = format!("<sup>{text}</sup>"),
            "subscript" => text = format!("<sub>{text}</sub>"),
            "code" => {}
            COMPONENT_LINK => {
                if let Some(href) = mark.attrs.get("href").and_then(Value::as_str) {
                    let title = mark
                        .attrs
                        .get("title")
                        .and_then(Value::as_str)
                        .map(|title| format!(" \"{}\"", escape_title(title)))
                        .unwrap_or_default();
                    text = format!("[{text}]({}{})", escape_destination(href), title);
                }
            }
            _ => {}
        }
    }
    text
}

fn code_span(text: &str) -> String {
    let fence = "`".repeat(longest_backtick_run(text) + 1);
    let padding = if text.starts_with('`') || text.ends_with('`') {
        " "
    } else {
        ""
    };
    format!("{fence}{padding}{text}{padding}{fence}")
}

fn longest_backtick_run(text: &str) -> usize {
    text.split(|character| character != '`')
        .map(str::len)
        .max()
        .unwrap_or(0)
}

fn escape_text(text: &str) -> String {
    text.chars()
        .flat_map(|character| {
            if character == '\\' || character.is_ascii_punctuation() {
                vec!['\\', character]
            } else {
                vec![character]
            }
        })
        .collect()
}

fn escape_destination(destination: &str) -> String {
    destination.replace('\\', "\\\\").replace(')', "\\)")
}

fn escape_title(title: &str) -> String {
    title.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_markdown_is_encode_decode_idempotent() {
        let markdown = "# Heading *with emphasis*\n\nA **bold** paragraph with `code` and [a link](https://example.com).\ncontinued here\n\n> quoted **text**\n\n```rust\nlet value = `raw`;\n```\n\n---";
        let encoded = serialize_markdown(&parse_markdown(markdown));
        assert_eq!(serialize_markdown(&parse_markdown(&encoded)), encoded);
    }

    #[test]
    fn text_metacharacters_round_trip_as_text() {
        let samples = [
            "*literal*",
            "_literal_",
            "`literal`",
            "\\path",
            "# heading",
            "- item",
            "+ item",
            "[label]",
            "a | b",
            "1. item",
        ];
        for sample in samples {
            let document = EditorDocument::plain_text(sample);
            let encoded = serialize_markdown(&document);
            assert_eq!(
                parse_markdown(&encoded).text_content(),
                sample,
                "encoded as {encoded:?}"
            );
        }
    }

    #[test]
    fn groups_multiline_paragraph_and_quote_blocks() {
        let document = parse_markdown("first\nsecond\n\n> one\n> two");
        assert_eq!(document.blocks.len(), 2);
        assert_eq!(document.blocks[0].text_content(), "first\nsecond");
        assert_eq!(document.blocks[1].component, COMPONENT_QUOTE);
        assert_eq!(document.blocks[1].text_content(), "one\ntwo");
    }

    #[test]
    fn lists_and_task_state_round_trip_structurally() {
        let markdown = "- [x] done\n- [ ] pending\n  - nested\n\n3. third\n4. fourth";
        let document = parse_markdown(markdown);
        assert_eq!(document.blocks.len(), 2);
        assert_eq!(document.blocks[0].component, COMPONENT_LIST);
        assert_eq!(document.blocks[1].attrs.get("start"), Some(&Value::from(3)));
        let encoded = serialize_markdown(&document);
        assert!(encoded.contains("- [x] done"));
        assert!(encoded.contains("- [ ] pending"));
        assert!(encoded.contains("3. third"));
        assert_eq!(serialize_markdown(&parse_markdown(&encoded)), encoded);
    }

    #[test]
    fn gfm_tables_and_strikethrough_round_trip() {
        let markdown = "| left | center | right |\n| :--- | :---: | ---: |\n| a | ~~old~~ | c |";
        let document = parse_markdown(markdown);
        assert_eq!(document.blocks[0].component, COMPONENT_TABLE);
        let encoded = serialize_markdown(&document);
        assert!(encoded.contains(":---:"));
        assert!(encoded.contains("~~old~~"));
        assert_eq!(serialize_markdown(&parse_markdown(&encoded)), encoded);
    }

    #[test]
    fn images_and_raw_html_are_preserved() {
        let markdown = "before ![alt](image.png \"caption\") <kbd>key</kbd>\n\n<section data-x=\"1\">raw</section>";
        let encoded = serialize_markdown(&parse_markdown(markdown));
        assert!(encoded.contains("![alt](image.png \"caption\")"));
        assert!(encoded.contains("<kbd>key</kbd>"));
        assert!(encoded.contains("<section data-x=\"1\">raw</section>"));
        assert_eq!(serialize_markdown(&parse_markdown(&encoded)), encoded);
    }
}
