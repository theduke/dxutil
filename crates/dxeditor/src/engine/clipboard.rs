//! Clipboard interchange independent of the browser and of the editor runtime.
//!
//! Internal data is deliberately a versioned CDV2 slice, rather than an editing
//! engine's private JSON representation. Untrusted clipboard data is bounded and
//! checked before it reaches a transaction.
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    ComponentNode,
    component_spec::{is_safe_media_url, is_safe_url},
    profiles::composer::ComposerValue,
};

use super::html::HtmlNode;

pub const INTERNAL_CLIPBOARD_MIME: &str = "application/x-semantic-dxeditor+json";
pub const COMPOSER_CLIPBOARD_MIME: &str = "application/x-semantic-composer+json";
pub const INTERNAL_CLIPBOARD_VERSION: u32 = 3;
pub const INTERNAL_CLIPBOARD_SCHEMA: &str = "semantic.component-slice";
pub const MAX_CLIPBOARD_BYTES: usize = 4_000_000;
pub const MAX_CLIPBOARD_NODES: usize = 10_000;
pub const MAX_CLIPBOARD_DEPTH: usize = 32;
pub const MAX_CLIPBOARD_TEXT: usize = 1_000_000;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ClipboardSlice {
    #[serde(default)]
    pub content: Vec<ComponentNode>,
    #[serde(default, rename = "openStart", alias = "open_start")]
    pub open_start: usize,
    #[serde(default, rename = "openEnd", alias = "open_end")]
    pub open_end: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PasteContent {
    pub internal: Option<ClipboardSlice>,
    pub html: Option<HtmlNode>,
    pub text: Option<String>,
    pub plain_mode: bool,
}

#[derive(Serialize, Deserialize)]
struct ClipboardEnvelope {
    version: u32,
    schema: String,
    #[serde(rename = "schemaFingerprint")]
    schema_fingerprint: String,
    slice: ClipboardSlice,
}

#[derive(Debug, thiserror::Error)]
pub enum ClipboardError {
    #[error("clipboard content is too large or contains unsupported or unsafe nodes")]
    InvalidContent,
    #[error("clipboard serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Remove structural identities, keeping entity references and island-owned
/// identities intact. The transaction identity repair pass rekeys copied islands
/// consistently with all their references if the original is still present.
pub fn strip_copied_identities(node: &mut ComponentNode) {
    node.id = None;
    for name in ["id", "uid", "semanticId"] {
        node.attrs.remove(name);
    }
    // Embedded opaque CDV2 nodes need new identities too. They do not participate
    // in the live block index, so strip their ids rather than leaking duplicates.
    for name in ["originalNode", "original_node", "raw"] {
        if let Some(value) = node.attrs.get_mut(name) {
            strip_embedded_identities(value);
        }
    }
    for child in &mut node.content {
        strip_copied_identities(child);
    }
}

fn strip_embedded_identities(value: &mut Value) {
    match value {
        Value::Array(values) => values.iter_mut().for_each(strip_embedded_identities),
        Value::Object(object) => {
            if object.get("id").is_some_and(Value::is_string) {
                object.insert("id".into(), Value::String(uuid::Uuid::now_v7().to_string()));
            }
            for child in object.values_mut() {
                strip_embedded_identities(child);
            }
        }
        _ => {}
    }
}

pub fn encode_internal_clipboard(
    slice: &ClipboardSlice,
    schema_fingerprint: &str,
) -> Result<String, ClipboardError> {
    if !valid_slice(slice) {
        return Err(ClipboardError::InvalidContent);
    }
    let mut slice = slice.clone();
    slice.content.iter_mut().for_each(strip_copied_identities);
    let encoded = serde_json::to_string(&ClipboardEnvelope {
        version: INTERNAL_CLIPBOARD_VERSION,
        schema: INTERNAL_CLIPBOARD_SCHEMA.into(),
        schema_fingerprint: schema_fingerprint.into(),
        slice,
    })?;
    if encoded.len() > MAX_CLIPBOARD_BYTES {
        return Err(ClipboardError::InvalidContent);
    }
    Ok(encoded)
}

pub fn decode_internal_clipboard(raw: &str, schema_fingerprint: &str) -> Option<ClipboardSlice> {
    if raw.is_empty() || raw.len() > MAX_CLIPBOARD_BYTES {
        return None;
    }
    let envelope: ClipboardEnvelope = serde_json::from_str(raw).ok()?;
    if envelope.version != INTERNAL_CLIPBOARD_VERSION
        || envelope.schema != INTERNAL_CLIPBOARD_SCHEMA
        || envelope.schema_fingerprint != schema_fingerprint
        || !valid_slice(&envelope.slice)
    {
        return None;
    }
    Some(envelope.slice)
}

fn valid_slice(slice: &ClipboardSlice) -> bool {
    // The openness cannot exceed the depth of a legal tree.
    if slice.open_start > MAX_CLIPBOARD_DEPTH || slice.open_end > MAX_CLIPBOARD_DEPTH {
        return false;
    }
    let mut remaining = MAX_CLIPBOARD_NODES;
    slice
        .content
        .iter()
        .all(|node| valid_node(node, 0, &mut remaining))
}

fn valid_node(node: &ComponentNode, depth: usize, remaining: &mut usize) -> bool {
    if depth > MAX_CLIPBOARD_DEPTH || *remaining == 0 {
        return false;
    }
    *remaining -= 1;
    if !matches!(
        node.kind.0.as_str(),
        "document"
            | "block_children"
            | "block"
            | "paragraph"
            | "heading"
            | "bulleted_list_item"
            | "numbered_list_item"
            | "todo"
            | "toggle"
            | "quote"
            | "callout"
            | "code_block"
            | "divider"
            | "table"
            | "table_row"
            | "table_cell"
            | "table_header_cell"
            | "image"
            | "file"
            | "bookmark"
            | "entity_embed"
            | "unsupported"
            | "graph"
            | "details_break"
            | "text"
            | "hard_break"
            | "mention"
            | "chip"
            | "opaque_markdown"
            | "opaque_markdown_inline"
            | "opaque_markdown_block"
            | "unknown_component"
            | "blockquote"
            | "bullet_list"
            | "ordered_list"
            | "task_list"
            | "list_item"
            | "task_item"
            | "thematic_break"
            | "table_header"
    ) || node
        .text
        .as_ref()
        .is_some_and(|text| text.len() > MAX_CLIPBOARD_TEXT)
        || (node.kind.0 == "text" && (node.text.is_none() || !node.content.is_empty()))
        || (node.kind.0 != "text" && node.text.is_some())
    {
        return false;
    }
    if node.kind.0 == "image" {
        if let Some(src) = node.attrs.get("src") {
            if !src.as_str().is_some_and(is_safe_media_url) {
                return false;
            }
        }
        if let Some(media) = node.attrs.get("media") {
            if !media.is_object()
                || (media.get("type").and_then(Value::as_str) == Some("url")
                    && !media
                        .get("url")
                        .and_then(Value::as_str)
                        .is_some_and(is_safe_media_url))
            {
                return false;
            }
        }
    }
    for mark in &node.marks {
        if !matches!(
            mark.kind.0.as_str(),
            "bold"
                | "italic"
                | "underline"
                | "strike"
                | "code"
                | "link"
                | "highlight"
                | "text_color"
                | "entity_link"
        ) {
            return false;
        }
        if mark.kind.0 == "link"
            && !mark
                .attrs
                .get("href")
                .and_then(Value::as_str)
                .is_some_and(is_safe_url)
        {
            return false;
        }
        if mark.kind.0 == "entity_link"
            && !mark
                .attrs
                .get("entity_id")
                .and_then(Value::as_str)
                .is_some_and(valid_entity_id)
        {
            return false;
        }
    }
    node.content
        .iter()
        .all(|child| valid_node(child, depth + 1, remaining))
}

/// RFC-4180 style tab-separated fields with strict editor table limits. A tab is
/// required so ordinary multiline text never accidentally becomes a table.
pub fn parse_tsv_grid(text: &str) -> Option<Vec<Vec<String>>> {
    if !text.contains('\t') || text.len() > MAX_CLIPBOARD_TEXT {
        return None;
    }
    let mut rows = vec![Vec::new()];
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' if quoted || field.is_empty() => quoted = !quoted,
            '\t' if !quoted => {
                rows.last_mut()?.push(std::mem::take(&mut field));
                if rows.last()?.len() >= 100 {
                    return None;
                }
            }
            '\n' | '\r' if !quoted => {
                if ch == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                rows.last_mut()?.push(std::mem::take(&mut field));
                rows.push(Vec::new());
                if rows.len() > 101 {
                    return None;
                }
            }
            _ => field.push(ch),
        }
    }
    if quoted {
        return None;
    }
    rows.last_mut()?.push(field);
    if rows.len() > 1
        && rows
            .last()
            .is_some_and(|row| row.len() == 1 && row[0].is_empty())
    {
        rows.pop();
    }
    let width = rows.iter().map(Vec::len).max()?;
    if width == 0 || width > 100 || rows.len() > 100 || rows.len() * width > 10_000 {
        return None;
    }
    for row in &mut rows {
        row.resize(width, String::new());
    }
    Some(rows)
}

/// One block per line, preserving empty lines and normalising all platform line endings.
pub fn plain_blocks(text: &str) -> Vec<ComponentNode> {
    normalize_line_endings(text)
        .split('\n')
        .map(|line| {
            super::html::wrap_block(
                ComponentNode::container(
                    "paragraph",
                    None,
                    if line.is_empty() {
                        Vec::new()
                    } else {
                        vec![ComponentNode::text(line)]
                    },
                ),
                Vec::new(),
            )
        })
        .collect()
}

pub fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

pub fn looks_like_markdown(text: &str) -> bool {
    let normalized = normalize_line_endings(text);
    normalized.contains('\n')
        && normalized.lines().any(|line| {
            let hashes = line.bytes().take_while(|byte| *byte == b'#').count();
            (hashes > 0 && hashes <= 6 && line.as_bytes().get(hashes) == Some(&b' '))
                || ["- ", "* ", "+ ", "> ", "```"]
                    .iter()
                    .any(|prefix| line.starts_with(prefix))
                || line.split_once(". ").is_some_and(|(number, _)| {
                    !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
                })
        })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClassifiedUrl {
    Entity { id: String, href: String },
    External(String),
}

fn valid_entity_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 1024 && !id.chars().any(char::is_control)
}

/// Resolve application entity URLs only against the actual page origin; URLs to
/// another host that happen to contain `/entities/` remain ordinary links.
pub fn classify_url(text: &str, origin: Option<&str>) -> Option<ClassifiedUrl> {
    if text != text.trim() || text.chars().any(|ch| ch.is_control() || ch.is_whitespace()) {
        return None;
    }
    if let Some(id) = text
        .strip_prefix("semantic:entity:")
        .filter(|id| valid_entity_id(id))
    {
        return Some(ClassifiedUrl::Entity {
            id: id.into(),
            href: text.into(),
        });
    }
    if !is_safe_url(text) {
        return None;
    }
    if let Some(origin) = origin.and_then(|origin| url::Url::parse(origin).ok()) {
        if let Ok(url) = origin.join(text) {
            if url.origin() == origin.origin() {
                if let Some(id) = url
                    .path()
                    .strip_prefix("/entities/")
                    .or_else(|| url.path().strip_prefix("/documents/"))
                {
                    if id.contains('/') {
                        return Some(ClassifiedUrl::External(text.into()));
                    }
                    let id = decode_path_segment(id)?;
                    if valid_entity_id(&id) {
                        return Some(ClassifiedUrl::Entity {
                            href: format!("semantic:entity:{id}"),
                            id,
                        });
                    }
                }
            }
        }
    }
    Some(ClassifiedUrl::External(text.into()))
}

fn decode_path_segment(segment: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(segment.len());
    let mut source = segment.bytes();
    while let Some(byte) = source.next() {
        if byte == b'%' {
            let high = (source.next()? as char).to_digit(16)? as u8;
            let low = (source.next()? as char).to_digit(16)? as u8;
            bytes.push((high << 4) | low);
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes).ok()
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComposerClipboard {
    pub value: ComposerValue,
    /// Raw attachment metadata belongs to the host; the editor only routes it.
    pub attachments: Option<Vec<Value>>,
}

pub fn encode_composer_clipboard(value: &ComposerValue) -> Result<String, ClipboardError> {
    let encoded = serde_json::to_string(value)?;
    if encoded.len() > MAX_CLIPBOARD_BYTES || value.segments.len() > MAX_CLIPBOARD_NODES {
        return Err(ClipboardError::InvalidContent);
    }
    Ok(encoded)
}

pub fn decode_composer_clipboard(raw: &str) -> Option<ComposerClipboard> {
    if raw.is_empty() || raw.len() > MAX_CLIPBOARD_BYTES {
        return None;
    }
    let json: Value = serde_json::from_str(raw).ok()?;
    let value: ComposerValue = serde_json::from_value(json.clone()).ok()?;
    if value.segments.len() > MAX_CLIPBOARD_NODES {
        return None;
    }
    let attachments = match json.get("attachments") {
        Some(Value::Array(items)) => Some(items.clone()),
        Some(_) => return None,
        None => None,
    };
    Some(ComposerClipboard { value, attachments })
}

pub use super::html::{blocks_to_html, blocks_to_plain_text, html_to_blocks};
