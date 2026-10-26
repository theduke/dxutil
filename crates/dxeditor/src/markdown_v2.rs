use std::{
    collections::{BTreeMap, VecDeque},
    ops::Range,
};

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde_json::{Map, Value, json};

use crate::{
    EditorPayload,
    component_spec::{ComponentCatalog, FORMAT_MARKDOWN, validate_component_document},
    document::NodeId,
    document_v2::{
        COMPONENT_BLOCKQUOTE, COMPONENT_BULLET_LIST, COMPONENT_CODE_BLOCK, COMPONENT_HARD_BREAK,
        COMPONENT_HEADING_V2, COMPONENT_IMAGE, COMPONENT_LIST_ITEM_V2, COMPONENT_MENTION_V2,
        COMPONENT_OPAQUE_MARKDOWN_BLOCK, COMPONENT_OPAQUE_MARKDOWN_INLINE, COMPONENT_ORDERED_LIST,
        COMPONENT_PARAGRAPH_V2, COMPONENT_TABLE_CELL_V2, COMPONENT_TABLE_HEADER,
        COMPONENT_TABLE_ROW_V2, COMPONENT_TABLE_V2, COMPONENT_TASK_ITEM, COMPONENT_TASK_LIST,
        COMPONENT_TEXT_V2, COMPONENT_THEMATIC_BREAK, ComponentDocumentV2, ComponentMark,
        ComponentNode, UnknownComponentPolicy,
    },
    format::{
        DecodeOptions, DecodedDocument, DocumentFormat, EncodeOptions, EncodedPayload, Fidelity,
        FormatDescriptor, FormatError,
    },
};

/// Native Markdown codec for the v2 editor document. The legacy v1 Markdown codec remains a
/// compatibility API, but the interactive editor never projects through it.
pub struct MarkdownDocumentFormat {
    descriptor: FormatDescriptor,
}

impl MarkdownDocumentFormat {
    pub fn new() -> Self {
        let mut capabilities = BTreeMap::new();
        for kind in [
            COMPONENT_PARAGRAPH_V2,
            COMPONENT_HEADING_V2,
            COMPONENT_BLOCKQUOTE,
            COMPONENT_BULLET_LIST,
            COMPONENT_ORDERED_LIST,
            COMPONENT_TASK_LIST,
            COMPONENT_LIST_ITEM_V2,
            COMPONENT_TASK_ITEM,
            COMPONENT_CODE_BLOCK,
            COMPONENT_THEMATIC_BREAK,
            COMPONENT_TABLE_V2,
            COMPONENT_TABLE_ROW_V2,
            COMPONENT_TABLE_HEADER,
            COMPONENT_TABLE_CELL_V2,
            COMPONENT_TEXT_V2,
            COMPONENT_HARD_BREAK,
            COMPONENT_IMAGE,
            COMPONENT_MENTION_V2,
            "bold",
            "italic",
            "strike",
            "code",
            "link",
        ] {
            capabilities.insert(kind.into(), crate::FormatCapability::Native);
        }
        capabilities.insert(
            COMPONENT_OPAQUE_MARKDOWN_BLOCK.into(),
            crate::FormatCapability::Opaque,
        );
        capabilities.insert(
            COMPONENT_OPAQUE_MARKDOWN_INLINE.into(),
            crate::FormatCapability::Opaque,
        );
        Self {
            descriptor: FormatDescriptor {
                id: FORMAT_MARKDOWN.into(),
                media_type: "text/markdown".to_string(),
                version: 1,
                capabilities,
            },
        }
    }
}

impl Default for MarkdownDocumentFormat {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentFormat for MarkdownDocumentFormat {
    fn descriptor(&self) -> &FormatDescriptor {
        &self.descriptor
    }

    fn decode(
        &self,
        input: &EditorPayload,
        catalog: &ComponentCatalog,
        options: DecodeOptions,
    ) -> Result<DecodedDocument, FormatError> {
        if input.format != FORMAT_MARKDOWN && input.format != self.descriptor.media_type {
            return Err(FormatError::FormatMismatch {
                expected: FORMAT_MARKDOWN.to_string(),
                actual: input.format.clone(),
            });
        }
        let source = input
            .value
            .as_str()
            .ok_or_else(|| FormatError::InvalidPayload {
                format: FORMAT_MARKDOWN.to_string(),
                message: "expected a Markdown string".to_string(),
            })?;
        if source.len() > options.validation_limits.max_text_bytes {
            return Err(FormatError::InvalidPayload {
                format: FORMAT_MARKDOWN.to_string(),
                message: "input exceeds the configured text-size limit".to_string(),
            });
        }
        let document = MarkdownV2Parser::new(source).parse();
        validate_component_document(
            &document,
            catalog,
            &options.validation_limits,
            UnknownComponentPolicy::PreserveOpaque,
        )?;
        Ok(DecodedDocument {
            document,
            diagnostics: Vec::new(),
            fidelity: Fidelity::Semantic,
            source_state: None,
        })
    }

    fn encode(
        &self,
        document: &ComponentDocumentV2,
        catalog: &ComponentCatalog,
        options: EncodeOptions,
    ) -> Result<EncodedPayload, FormatError> {
        validate_component_document(
            document,
            catalog,
            &options.validation_limits,
            UnknownComponentPolicy::PreserveOpaque,
        )?;
        let markdown = serialize_document(document)?;
        Ok(EncodedPayload {
            payload: EditorPayload::new(FORMAT_MARKDOWN, Value::String(markdown)),
            diagnostics: Vec::new(),
            fidelity: Fidelity::Semantic,
        })
    }
}

type RangedEvent = (Event<'static>, Range<usize>);

struct MarkdownV2Parser<'a> {
    source: &'a str,
    events: VecDeque<RangedEvent>,
    next_id: usize,
    task_markers: Vec<Option<bool>>,
}

impl<'a> MarkdownV2Parser<'a> {
    fn new(source: &'a str) -> Self {
        let options =
            Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH;
        Self {
            source,
            events: Parser::new_ext(source, options)
                .into_offset_iter()
                .map(|(event, range)| (event.into_static(), range))
                .collect(),
            next_id: 1,
            task_markers: Vec::new(),
        }
    }

    fn parse(mut self) -> ComponentDocumentV2 {
        let content = self.parse_blocks(None);
        ComponentDocumentV2::new(if content.is_empty() {
            vec![self.node(COMPONENT_PARAGRAPH_V2, Vec::new())]
        } else {
            content
        })
    }

    fn id(&mut self, prefix: &str) -> NodeId {
        let id = NodeId::new(format!("markdown-{prefix}-{}", self.next_id));
        self.next_id += 1;
        id
    }

    fn node(&mut self, kind: &str, content: Vec<ComponentNode>) -> ComponentNode {
        ComponentNode::container(kind, Some(self.id(kind)), content)
    }

    fn parse_blocks(&mut self, end: Option<TagEnd>) -> Vec<ComponentNode> {
        let mut blocks = Vec::new();
        loop {
            if matches!(self.events.front(), Some((Event::End(found), _)) if Some(*found) == end) {
                self.events.pop_front();
                break;
            }
            let Some((event, range)) = self.events.pop_front() else {
                break;
            };
            match event {
                Event::Start(Tag::Paragraph) => {
                    let inline = self.parse_inlines(TagEnd::Paragraph);
                    blocks.push(self.node(COMPONENT_PARAGRAPH_V2, inline));
                }
                Event::Start(Tag::Heading { level, .. }) => {
                    let inline = self.parse_inlines(TagEnd::Heading(level));
                    let mut node = self.node(COMPONENT_HEADING_V2, inline);
                    node.attrs
                        .insert("level".to_string(), json!(heading_level(level)));
                    blocks.push(node);
                }
                Event::Start(Tag::BlockQuote(kind)) => {
                    let content = self.parse_blocks(Some(TagEnd::BlockQuote(kind)));
                    blocks.push(self.node(COMPONENT_BLOCKQUOTE, content));
                }
                Event::Start(Tag::CodeBlock(kind)) => blocks.push(self.parse_code_block(kind)),
                Event::Start(Tag::List(start)) => blocks.extend(self.parse_list(start)),
                Event::Start(Tag::Table(alignments)) => blocks.push(self.parse_table(alignments)),
                Event::Start(Tag::HtmlBlock) => blocks.push(self.parse_html_block(range.start)),
                Event::Rule => blocks.push(self.node(COMPONENT_THEMATIC_BREAK, Vec::new())),
                Event::Text(text) | Event::Code(text) => {
                    let text = ComponentNode::text(text.into_string());
                    blocks.push(self.node(COMPONENT_PARAGRAPH_V2, vec![text]));
                }
                Event::Html(_) | Event::InlineHtml(_) => {
                    let raw = self
                        .source
                        .get(range.clone())
                        .unwrap_or_default()
                        .to_string();
                    blocks.push(self.opaque(COMPONENT_OPAQUE_MARKDOWN_BLOCK, raw, "html_block"));
                }
                Event::SoftBreak | Event::HardBreak => {
                    blocks.push(self.node(COMPONENT_PARAGRAPH_V2, Vec::new()));
                }
                Event::InlineMath(_) | Event::DisplayMath(_) | Event::FootnoteReference(_) => {
                    let raw = self.source.get(range).unwrap_or_default().to_string();
                    blocks.push(self.opaque(COMPONENT_OPAQUE_MARKDOWN_BLOCK, raw, "extension"));
                }
                Event::TaskListMarker(checked) => {
                    if let Some(marker) = self.task_markers.last_mut() {
                        *marker = Some(checked);
                    }
                }
                Event::Start(tag) => {
                    let end = tag.to_end();
                    let raw_start = range.start;
                    let raw_end = self.consume_through(end).unwrap_or(range.end);
                    let raw = self
                        .source
                        .get(raw_start..raw_end)
                        .unwrap_or_default()
                        .to_string();
                    blocks.push(self.opaque(COMPONENT_OPAQUE_MARKDOWN_BLOCK, raw, "unsupported"));
                }
                Event::End(_) => break,
            }
        }
        blocks
    }

    fn parse_inlines(&mut self, end: TagEnd) -> Vec<ComponentNode> {
        let mut inline = Vec::new();
        let mut marks = Vec::<ComponentMark>::new();
        while let Some((event, range)) = self.events.pop_front() {
            match event {
                Event::End(found) if found == end => break,
                Event::Start(Tag::Strong) => marks.push(ComponentMark::new("bold")),
                Event::End(TagEnd::Strong) => remove_mark(&mut marks, "bold"),
                Event::Start(Tag::Emphasis) => marks.push(ComponentMark::new("italic")),
                Event::End(TagEnd::Emphasis) => remove_mark(&mut marks, "italic"),
                Event::Start(Tag::Strikethrough) => marks.push(ComponentMark::new("strike")),
                Event::End(TagEnd::Strikethrough) => remove_mark(&mut marks, "strike"),
                Event::Start(Tag::Link {
                    dest_url, title, ..
                }) => {
                    let mut attrs = Map::new();
                    attrs.insert("href".to_string(), json!(dest_url.as_ref()));
                    if !title.is_empty() {
                        attrs.insert("title".to_string(), json!(title.as_ref()));
                    }
                    marks.push(ComponentMark {
                        kind: "link".into(),
                        attrs,
                    });
                }
                Event::End(TagEnd::Link) => remove_mark(&mut marks, "link"),
                Event::Start(Tag::Image {
                    dest_url, title, ..
                }) => {
                    let alt = self.collect_plain_inline(TagEnd::Image);
                    let mut image = self.node(COMPONENT_IMAGE, Vec::new());
                    image
                        .attrs
                        .insert("src".to_string(), json!(dest_url.as_ref()));
                    image.attrs.insert("alt".to_string(), json!(alt));
                    if !title.is_empty() {
                        image
                            .attrs
                            .insert("title".to_string(), json!(title.as_ref()));
                    }
                    inline.push(image);
                }
                Event::Text(text) => self.push_text(&mut inline, text.as_ref(), &marks),
                Event::Code(text) => {
                    let mut code_marks = marks.clone();
                    code_marks.push(ComponentMark::new("code"));
                    self.push_text(&mut inline, text.as_ref(), &code_marks);
                }
                Event::SoftBreak => self.push_text(&mut inline, "\n", &marks),
                Event::HardBreak => {
                    let mut node = self.node(COMPONENT_HARD_BREAK, Vec::new());
                    node.marks.clone_from(&marks);
                    inline.push(node);
                }
                Event::Html(_) | Event::InlineHtml(_) => {
                    let raw = self.source.get(range).unwrap_or_default().to_string();
                    let mut node =
                        self.opaque(COMPONENT_OPAQUE_MARKDOWN_INLINE, raw, "html_inline");
                    node.marks.clone_from(&marks);
                    inline.push(node);
                }
                Event::TaskListMarker(checked) => {
                    if let Some(marker) = self.task_markers.last_mut() {
                        *marker = Some(checked);
                    }
                }
                Event::InlineMath(_) | Event::DisplayMath(_) | Event::FootnoteReference(_) => {
                    let raw = self.source.get(range).unwrap_or_default().to_string();
                    let mut node = self.opaque(COMPONENT_OPAQUE_MARKDOWN_INLINE, raw, "extension");
                    node.marks.clone_from(&marks);
                    inline.push(node);
                }
                Event::Rule => self.push_text(&mut inline, "---", &marks),
                Event::Start(tag) => {
                    let tag_end = tag.to_end();
                    let raw_start = range.start;
                    let raw_end = self.consume_through(tag_end).unwrap_or(range.end);
                    let raw = self
                        .source
                        .get(raw_start..raw_end)
                        .unwrap_or_default()
                        .to_string();
                    inline.push(self.opaque(COMPONENT_OPAQUE_MARKDOWN_INLINE, raw, "unsupported"));
                }
                Event::End(_) => {}
            }
        }
        inline
    }

    fn push_text(&mut self, output: &mut Vec<ComponentNode>, text: &str, marks: &[ComponentMark]) {
        if text.is_empty() {
            return;
        }
        if let Some(link) = marks.iter().find(|mark| mark.kind.0 == "link")
            && let Some(entity_id) = link
                .attrs
                .get("href")
                .and_then(Value::as_str)
                .and_then(|href| href.strip_prefix("semantic:entity:"))
            && let Some(label) = text.strip_prefix('@')
        {
            let mut node = ComponentNode::new(COMPONENT_MENTION_V2);
            node.id = Some(self.id("mention"));
            node.attrs.insert("entity_id".to_string(), json!(entity_id));
            node.attrs.insert("label".to_string(), json!(label));
            node.marks = marks
                .iter()
                .filter(|mark| mark.kind.0 != "link")
                .cloned()
                .collect();
            output.push(node);
            return;
        }
        let mut node = ComponentNode::text(text);
        node.marks = marks.to_vec();
        output.push(node);
    }

    fn collect_plain_inline(&mut self, end: TagEnd) -> String {
        let mut value = String::new();
        while let Some((event, _)) = self.events.pop_front() {
            match event {
                Event::End(found) if found == end => break,
                Event::Text(text) | Event::Code(text) => value.push_str(&text),
                Event::SoftBreak | Event::HardBreak => value.push('\n'),
                _ => {}
            }
        }
        value
    }

    fn parse_code_block(&mut self, kind: CodeBlockKind<'static>) -> ComponentNode {
        let mut value = String::new();
        while let Some((event, _)) = self.events.pop_front() {
            match event {
                Event::End(TagEnd::CodeBlock) => break,
                Event::Text(text)
                | Event::Code(text)
                | Event::Html(text)
                | Event::InlineHtml(text) => value.push_str(&text),
                Event::SoftBreak | Event::HardBreak => value.push('\n'),
                _ => {}
            }
        }
        if value.ends_with('\n') {
            value.pop();
        }
        let mut node = self.node(COMPONENT_CODE_BLOCK, vec![ComponentNode::text(value)]);
        if let CodeBlockKind::Fenced(info) = kind
            && !info.is_empty()
        {
            node.attrs.insert("info".to_string(), json!(info.as_ref()));
        }
        node
    }

    fn parse_list(&mut self, start: Option<u64>) -> Vec<ComponentNode> {
        let mut items = Vec::new();
        loop {
            match self.events.front() {
                Some((Event::End(TagEnd::List(_)), _)) => {
                    self.events.pop_front();
                    break;
                }
                Some((Event::Start(Tag::Item), _)) => {
                    self.events.pop_front();
                    self.task_markers.push(None);
                    let content = self.parse_blocks(Some(TagEnd::Item));
                    let checked = self.task_markers.pop().flatten();
                    let content = if content.is_empty() {
                        vec![self.node(COMPONENT_PARAGRAPH_V2, Vec::new())]
                    } else {
                        content
                    };
                    let mut item = self.node(
                        if checked.is_some() {
                            COMPONENT_TASK_ITEM
                        } else {
                            COMPONENT_LIST_ITEM_V2
                        },
                        content,
                    );
                    if let Some(checked) = checked {
                        item.attrs.insert("checked".to_string(), json!(checked));
                    }
                    items.push(item);
                }
                None => break,
                _ => {
                    self.events.pop_front();
                }
            }
        }
        let mut runs: Vec<ComponentNode> = Vec::new();
        for (index, item) in items.into_iter().enumerate() {
            let kind = if item.kind.0 == COMPONENT_TASK_ITEM {
                COMPONENT_TASK_LIST
            } else if start.is_some() {
                COMPONENT_ORDERED_LIST
            } else {
                COMPONENT_BULLET_LIST
            };
            if runs.last().is_none_or(|run| run.kind.0 != kind) {
                let mut run = self.node(kind, Vec::new());
                if kind == COMPONENT_ORDERED_LIST {
                    run.attrs.insert(
                        "start".to_string(),
                        json!(start.unwrap_or(1) + index as u64),
                    );
                }
                runs.push(run);
            }
            runs.last_mut().unwrap().content.push(item);
        }
        runs
    }

    fn parse_table(&mut self, alignments: Vec<Alignment>) -> ComponentNode {
        let mut rows = Vec::new();
        loop {
            match self.events.pop_front() {
                Some((Event::Start(Tag::TableHead), _)) => {
                    rows.push(self.parse_table_row(TagEnd::TableHead, true, &alignments))
                }
                Some((Event::Start(Tag::TableRow), _)) => {
                    rows.push(self.parse_table_row(TagEnd::TableRow, false, &alignments))
                }
                Some((Event::End(TagEnd::Table), _)) | None => break,
                _ => {}
            }
        }
        self.node(COMPONENT_TABLE_V2, rows)
    }

    fn parse_table_row(
        &mut self,
        end: TagEnd,
        header: bool,
        alignments: &[Alignment],
    ) -> ComponentNode {
        let mut cells = Vec::new();
        loop {
            match self.events.pop_front() {
                Some((Event::Start(Tag::TableCell), _)) => {
                    let inline = self.parse_inlines(TagEnd::TableCell);
                    let paragraph = self.node(COMPONENT_PARAGRAPH_V2, inline);
                    let mut cell = self.node(
                        if header {
                            COMPONENT_TABLE_HEADER
                        } else {
                            COMPONENT_TABLE_CELL_V2
                        },
                        vec![paragraph],
                    );
                    if let Some(alignment) = alignments.get(cells.len()).and_then(alignment_name) {
                        cell.attrs.insert("alignment".to_string(), json!(alignment));
                    }
                    cells.push(cell);
                }
                Some((Event::End(found), _)) if found == end => break,
                None => break,
                _ => {}
            }
        }
        self.node(COMPONENT_TABLE_ROW_V2, cells)
    }

    fn parse_html_block(&mut self, start: usize) -> ComponentNode {
        let mut end = start;
        while let Some((event, range)) = self.events.pop_front() {
            end = range.end;
            if event == Event::End(TagEnd::HtmlBlock) {
                break;
            }
        }
        let raw = self.source.get(start..end).unwrap_or_default().to_string();
        self.opaque(COMPONENT_OPAQUE_MARKDOWN_BLOCK, raw, "html_block")
    }

    fn consume_through(&mut self, end: TagEnd) -> Option<usize> {
        while let Some((event, range)) = self.events.pop_front() {
            if event == Event::End(end) {
                return Some(range.end);
            }
        }
        None
    }

    fn opaque(&mut self, kind: &str, source: String, construct: &str) -> ComponentNode {
        let mut node = self.node(kind, Vec::new());
        node.attrs.insert("source".to_string(), json!(source));
        node.attrs.insert("fallback".to_string(), json!(source));
        node.attrs.insert("construct".to_string(), json!(construct));
        node
    }
}

fn remove_mark(marks: &mut Vec<ComponentMark>, kind: &str) {
    if let Some(index) = marks.iter().rposition(|mark| mark.kind.0 == kind) {
        marks.remove(index);
    }
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

fn alignment_name(alignment: &Alignment) -> Option<&'static str> {
    match alignment {
        Alignment::None => None,
        Alignment::Left => Some("left"),
        Alignment::Center => Some("center"),
        Alignment::Right => Some("right"),
    }
}

fn serialize_document(document: &ComponentDocumentV2) -> Result<String, FormatError> {
    document
        .root
        .content
        .iter()
        .map(serialize_block)
        .collect::<Result<Vec<_>, _>>()
        .map(|blocks| blocks.join("\n\n"))
}

fn serialize_block(node: &ComponentNode) -> Result<String, FormatError> {
    match node.kind.0.as_str() {
        COMPONENT_PARAGRAPH_V2 => serialize_inline(&node.content),
        COMPONENT_HEADING_V2 => Ok(format!(
            "{} {}",
            "#".repeat(
                node.attrs
                    .get("level")
                    .and_then(Value::as_u64)
                    .unwrap_or(1)
                    .clamp(1, 6) as usize
            ),
            serialize_inline(&node.content)?
        )),
        COMPONENT_BLOCKQUOTE => Ok(serialize_blocks(&node.content)?
            .lines()
            .map(|line| format!("> {line}"))
            .collect::<Vec<_>>()
            .join("\n")),
        COMPONENT_CODE_BLOCK => {
            let value = node.text_content();
            let fence = "`".repeat(longest_backtick_run(&value).saturating_add(1).max(3));
            Ok(format!(
                "{fence}{}\n{value}\n{fence}",
                node.attrs
                    .get("info")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
            ))
        }
        COMPONENT_THEMATIC_BREAK => Ok("---".to_string()),
        COMPONENT_BULLET_LIST | COMPONENT_ORDERED_LIST | COMPONENT_TASK_LIST => {
            serialize_list(node)
        }
        COMPONENT_TABLE_V2 => serialize_table(node),
        COMPONENT_OPAQUE_MARKDOWN_BLOCK => Ok(required_source(node)?),
        other => Err(unsupported(other, node)),
    }
}

fn serialize_blocks(nodes: &[ComponentNode]) -> Result<String, FormatError> {
    nodes
        .iter()
        .map(serialize_block)
        .collect::<Result<Vec<_>, _>>()
        .map(|blocks| blocks.join("\n\n"))
}

fn serialize_list(node: &ComponentNode) -> Result<String, FormatError> {
    let start = node.attrs.get("start").and_then(Value::as_u64).unwrap_or(1);
    node.content
        .iter()
        .enumerate()
        .map(|(index, item)| {
            if !matches!(
                item.kind.0.as_str(),
                COMPONENT_LIST_ITEM_V2 | COMPONENT_TASK_ITEM
            ) {
                return Err(unsupported(&item.kind.0, item));
            }
            let marker = if node.kind.0 == COMPONENT_ORDERED_LIST {
                format!("{}. ", start + index as u64)
            } else {
                "- ".to_string()
            };
            let task = if item.kind.0 == COMPONENT_TASK_ITEM {
                if item
                    .attrs
                    .get("checked")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    "[x] "
                } else {
                    "[ ] "
                }
            } else {
                ""
            };
            let body = serialize_blocks(&item.content)?;
            let mut lines = body.lines();
            let mut result = format!("{marker}{task}{}", lines.next().unwrap_or_default());
            for line in lines {
                result.push('\n');
                if !line.is_empty() {
                    result.push_str("    ");
                    result.push_str(line);
                }
            }
            Ok(result)
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|items| items.join("\n"))
}

fn serialize_table(node: &ComponentNode) -> Result<String, FormatError> {
    let Some(header) = node.content.first() else {
        return Ok(String::new());
    };
    if header
        .content
        .iter()
        .any(|cell| cell.kind.0 != COMPONENT_TABLE_HEADER)
    {
        return Err(FormatError::InvalidPayload {
            format: FORMAT_MARKDOWN.to_string(),
            message: "Markdown tables require a header row".to_string(),
        });
    }
    for row in &node.content {
        for cell in &row.content {
            let colspan = cell
                .attrs
                .get("colspan")
                .and_then(Value::as_u64)
                .unwrap_or(1);
            let rowspan = cell
                .attrs
                .get("rowspan")
                .and_then(Value::as_u64)
                .unwrap_or(1);
            if colspan != 1 || rowspan != 1 || cell.attrs.contains_key("colwidth") {
                return Err(FormatError::InvalidPayload {
                    format: FORMAT_MARKDOWN.to_string(),
                    message: format!(
                        "table cell '{}' uses spans or widths that Markdown cannot persist",
                        cell.id
                            .as_ref()
                            .map(|id| id.0.as_str())
                            .unwrap_or("<missing>")
                    ),
                });
            }
        }
    }
    let row = |row: &ComponentNode| -> Result<String, FormatError> {
        row.content
            .iter()
            .map(|cell| {
                serialize_blocks(&cell.content)
                    .map(|value| value.replace('|', "\\|").replace('\n', "<br>"))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|cells| format!("| {} |", cells.join(" | ")))
    };
    let delimiter = format!(
        "| {} |",
        header
            .content
            .iter()
            .map(
                |cell| match cell.attrs.get("alignment").and_then(Value::as_str) {
                    Some("left") => ":---",
                    Some("center") => ":---:",
                    Some("right") => "---:",
                    _ => "---",
                }
            )
            .collect::<Vec<_>>()
            .join(" | ")
    );
    std::iter::once(row(header))
        .chain(std::iter::once(Ok(delimiter)))
        .chain(node.content.iter().skip(1).map(row))
        .collect::<Result<Vec<_>, _>>()
        .map(|rows| rows.join("\n"))
}

fn serialize_inline(nodes: &[ComponentNode]) -> Result<String, FormatError> {
    nodes.iter().map(serialize_inline_node).collect()
}

fn serialize_inline_node(node: &ComponentNode) -> Result<String, FormatError> {
    match node.kind.0.as_str() {
        COMPONENT_TEXT_V2 => {
            let has_code = node.marks.iter().any(|mark| mark.kind.0 == "code");
            let mut text = if has_code {
                code_span(node.text.as_deref().unwrap_or_default())
            } else {
                escape_text(node.text.as_deref().unwrap_or_default())
            };
            for mark in node.marks.iter().rev() {
                match mark.kind.0.as_str() {
                    "bold" => text = format!("**{text}**"),
                    "italic" => text = format!("*{text}*"),
                    "strike" => text = format!("~~{text}~~"),
                    "code" => {}
                    "link" => {
                        if let Some(href) = mark.attrs.get("href").and_then(Value::as_str) {
                            text = format!(
                                "[{text}]({}{})",
                                escape_destination(href),
                                mark.attrs
                                    .get("title")
                                    .and_then(Value::as_str)
                                    .map(|title| format!(" \"{}\"", escape_title(title)))
                                    .unwrap_or_default()
                            );
                        }
                    }
                    other => return Err(unsupported(other, node)),
                }
            }
            Ok(text)
        }
        COMPONENT_HARD_BREAK => Ok("  \n".to_string()),
        COMPONENT_IMAGE => Ok(format!(
            "![{}]({}{})",
            escape_text(
                node.attrs
                    .get("alt")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
            ),
            escape_destination(
                node.attrs
                    .get("src")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
            ),
            node.attrs
                .get("title")
                .and_then(Value::as_str)
                .map(|title| format!(" \"{}\"", escape_title(title)))
                .unwrap_or_default()
        )),
        COMPONENT_MENTION_V2 => Ok(format!(
            "[@{}](semantic:entity:{})",
            escape_text(
                node.attrs
                    .get("label")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
            ),
            escape_destination(
                node.attrs
                    .get("entity_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
            )
        )),
        COMPONENT_OPAQUE_MARKDOWN_INLINE => Ok(required_source(node)?),
        other => Err(unsupported(other, node)),
    }
}

fn required_source(node: &ComponentNode) -> Result<String, FormatError> {
    node.attrs
        .get("source")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| FormatError::InvalidPayload {
            format: FORMAT_MARKDOWN.to_string(),
            message: format!("opaque node '{}' has no source", node.kind.0),
        })
}

fn unsupported(kind: &str, node: &ComponentNode) -> FormatError {
    FormatError::InvalidPayload {
        format: FORMAT_MARKDOWN.to_string(),
        message: format!(
            "component '{kind}' at node '{}' is not representable as Markdown",
            node.id
                .as_ref()
                .map(|id| id.0.as_str())
                .unwrap_or("<identity-free>")
        ),
    }
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
    use crate::component_spec::ComponentCatalog;

    fn catalog() -> ComponentCatalog {
        ComponentCatalog::standard().unwrap()
    }

    fn decode(source: &str) -> ComponentDocumentV2 {
        MarkdownDocumentFormat::new()
            .decode(
                &EditorPayload::new(FORMAT_MARKDOWN, json!(source)),
                &catalog(),
                DecodeOptions::default(),
            )
            .unwrap()
            .document
    }

    fn encode(document: &ComponentDocumentV2) -> String {
        MarkdownDocumentFormat::new()
            .encode(document, &catalog(), EncodeOptions::default())
            .unwrap()
            .payload
            .value
            .as_str()
            .unwrap()
            .to_string()
    }

    #[test]
    fn supported_profile_is_semantically_stable() {
        let source = "# Heading *em*\n\n- [x] done\n- [ ] pending\n\n3. third\n4. fourth\n\n~~old~~ ![alt](https://example.com/a.png \"title\")  \nnext\n\n```rust edition=2024\nfn main() {}\n```\n\n| left | right |\n| :--- | ---: |\n| a | b |";
        let first = encode(&decode(source));
        let second = encode(&decode(&first));
        assert_eq!(second, first);
    }

    #[test]
    fn multiline_paragraph_round_trips_as_one_semantic_block() {
        let mut hard_break = ComponentNode::new(COMPONENT_HARD_BREAK);
        hard_break.id = Some(NodeId::new("break-1"));
        let document = ComponentDocumentV2::new(vec![ComponentNode::paragraph(
            NodeId::new("paragraph-1"),
            vec![
                ComponentNode::text("first"),
                hard_break,
                ComponentNode::text("second"),
            ],
        )]);

        assert_eq!(document.text_content(), "first\nsecond");
        let markdown = encode(&document);
        assert_eq!(markdown, "first  \nsecond");

        let decoded = decode(&markdown);
        assert_eq!(decoded.root.content.len(), 1);
        assert_eq!(decoded.root.content[0].kind.0, COMPONENT_PARAGRAPH_V2);
        assert_eq!(decoded.root.content[0].content.len(), 3);
        assert_eq!(
            decoded.root.content[0].content[1].kind.0,
            COMPONENT_HARD_BREAK
        );
        assert_eq!(decoded.text_content(), "first\nsecond");
        assert_eq!(encode(&decoded), markdown);
    }

    #[test]
    fn opaque_html_source_survives_neighbor_edits() {
        let source = "before\n\n<section data-x=\"1\">raw</section>\n\nafter";
        let mut document = decode(source);
        document.root.content[0].content[0].text = Some("changed".to_string());
        assert!(encode(&document).contains("<section data-x=\"1\">raw</section>"));
    }

    #[test]
    fn markdown_rejects_non_persistable_table_geometry() {
        let mut document = decode("| a |\n| --- |\n| b |");
        document.root.content[0].content[1].content[0]
            .attrs
            .insert("rowspan".to_string(), json!(2));
        assert!(
            MarkdownDocumentFormat::new()
                .encode(&document, &catalog(), EncodeOptions::default())
                .is_err()
        );
    }

    #[test]
    fn profile_maps_standard_kinds_and_unique_mentions_directly_to_v2() {
        let document = decode(
            "> quote with [link](https://example.com \"title\") and <kbd>raw</kbd>\n\n- bullet\n\n[@Ada](semantic:entity:user-1) and [@Ada](semantic:entity:user-1)",
        );
        let mut kinds = Vec::new();
        let mut ids = Vec::new();
        fn visit(node: &ComponentNode, kinds: &mut Vec<String>, ids: &mut Vec<String>) {
            kinds.push(node.kind.0.clone());
            if let Some(id) = &node.id {
                ids.push(id.0.clone());
            }
            for child in &node.content {
                visit(child, kinds, ids);
            }
        }
        visit(&document.root, &mut kinds, &mut ids);
        for expected in [
            COMPONENT_BLOCKQUOTE,
            COMPONENT_BULLET_LIST,
            COMPONENT_MENTION_V2,
            COMPONENT_OPAQUE_MARKDOWN_INLINE,
        ] {
            assert!(
                kinds.iter().any(|kind| kind == expected),
                "missing {expected}: {kinds:?}"
            );
        }
        let unique = ids.iter().collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique.len(), ids.len());
    }

    #[test]
    fn fenced_code_preserves_the_complete_info_string() {
        let document = decode("```rust edition=2024\nfn main() {}\n```");
        assert_eq!(
            document.root.content[0].attrs.get("info"),
            Some(&json!("rust edition=2024"))
        );
        assert!(encode(&document).starts_with("```rust edition=2024"));
    }

    #[test]
    fn nested_and_loose_lists_canonicalize_stably() {
        let source =
            "- parent\n  - nested\n\n- second\n\n  continuation\n\n3. ordered\n   1. inner";
        let first = encode(&decode(source));
        let second = encode(&decode(&first));
        assert_eq!(second, first, "{first}");
        assert!(first.contains("nested"));
        assert!(first.contains("continuation"));
    }

    #[test]
    fn mixed_bullet_and_task_runs_round_trip_without_converting_bullets() {
        for source in [
            "- ordinary\n- [x] finished\n- [ ] pending\n- ordinary again",
            "- [ ] pending\n- [x] finished\n- ordinary\n- [ ] another task",
        ] {
            let document = decode(source);
            let kinds = document
                .root
                .content
                .iter()
                .map(|list| list.kind.0.as_str())
                .collect::<Vec<_>>();
            assert_eq!(
                kinds,
                if source.starts_with("- ordinary") {
                    vec![
                        COMPONENT_BULLET_LIST,
                        COMPONENT_TASK_LIST,
                        COMPONENT_BULLET_LIST,
                    ]
                } else {
                    vec![
                        COMPONENT_TASK_LIST,
                        COMPONENT_BULLET_LIST,
                        COMPONENT_TASK_LIST,
                    ]
                }
            );
            let first = encode(&document);
            let reopened = decode(&first);
            assert_eq!(encode(&reopened), first);
            assert_eq!(
                reopened
                    .root
                    .content
                    .iter()
                    .map(|list| list.kind.0.as_str())
                    .collect::<Vec<_>>(),
                kinds
            );
            for list in &reopened.root.content {
                for item in &list.content {
                    if list.kind.0 == COMPONENT_TASK_LIST {
                        assert_eq!(item.kind.0, COMPONENT_TASK_ITEM);
                        assert_eq!(
                            item.attrs.get("checked").and_then(Value::as_bool),
                            Some(item.text_content().contains("finished"))
                        );
                    } else {
                        assert_eq!(item.kind.0, COMPONENT_LIST_ITEM_V2);
                        assert!(!item.attrs.contains_key("checked"));
                    }
                }
            }
        }
    }

    #[test]
    fn nested_mixed_list_runs_round_trip() {
        let source =
            "- parent\n  - ordinary child\n  - [x] done child\n  - ordinary again\n- sibling";
        let first = encode(&decode(source));
        let reopened = decode(&first);
        assert_eq!(encode(&reopened), first);

        let parent = &reopened.root.content[0].content[0];
        assert_eq!(parent.kind.0, COMPONENT_LIST_ITEM_V2);
        assert_eq!(
            parent
                .content
                .iter()
                .skip(1)
                .map(|list| list.kind.0.as_str())
                .collect::<Vec<_>>(),
            vec![
                COMPONENT_BULLET_LIST,
                COMPONENT_TASK_LIST,
                COMPONENT_BULLET_LIST
            ]
        );
        assert_eq!(
            parent.content[2].content[0].attrs.get("checked"),
            Some(&json!(true))
        );
    }

    #[test]
    fn decode_limits_fail_before_unbounded_parsing() {
        let error = MarkdownDocumentFormat::new()
            .decode(
                &EditorPayload::new(FORMAT_MARKDOWN, json!("oversized")),
                &catalog(),
                DecodeOptions {
                    validation_limits: crate::ValidationLimits {
                        max_text_bytes: 3,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .unwrap_err();
        assert!(error.to_string().contains("text-size limit"));
    }
}
