//! Lossless projection between the persisted standard document and the editor's
//! block containers. Only shallow structural shells live in metadata; editable
//! inline content is always read from the current document on the inverse pass.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::profiles::block_document::BLOCK_DOCUMENT_SCHEMA;
use crate::{ComponentDocumentV2, ComponentNode, DocumentSchemaId, NodeId};

const ORIGIN: &str = "dxeditor.markdown-origin";

#[derive(Default, Serialize, Deserialize)]
struct Origins {
    root: Option<ComponentNode>,
    schema: Option<DocumentSchemaId>,
    previous: Option<Value>,
    #[serde(default)]
    previous_present: bool,
    nodes: BTreeMap<String, Origin>,
    #[serde(default)]
    inline: BTreeMap<String, ComponentNode>,
}

#[derive(Default, Serialize, Deserialize)]
struct Origin {
    // A list container or the paragraph surrounding an image.
    outer: Option<ComponentNode>,
    // The first paragraph flattened into a list item or quote.
    paragraph: Option<ComponentNode>,
    // A quote can begin with another block, without an empty paragraph.
    #[serde(default)]
    no_first_paragraph: bool,
    // Table row identities and cell paragraph shells.
    #[serde(default)]
    rows: BTreeMap<String, ComponentNode>,
    #[serde(default)]
    cells: BTreeMap<String, ComponentNode>,
}

fn shell(node: &ComponentNode) -> ComponentNode {
    let mut node = node.clone();
    node.content.clear();
    node
}

fn fresh_id() -> NodeId {
    NodeId::new(uuid::Uuid::now_v7().to_string())
}

fn wrap(content: ComponentNode, children: Vec<ComponentNode>) -> ComponentNode {
    let mut content = vec![content];
    if !children.is_empty() {
        content.push(ComponentNode::container("block_children", None, children));
    }
    ComponentNode::container("block", Some(fresh_id()), content)
}

fn key(node: &ComponentNode) -> &str {
    node.id.as_ref().map(|id| id.0.as_str()).unwrap_or_default()
}

/// Convert a standard CDV2 document to the engine block schema. Images split
/// paragraphs into before/image/after blocks, as required by the editing schema.
pub fn to_blocks(standard: &ComponentDocumentV2) -> ComponentDocumentV2 {
    let mut origins = Origins {
        root: Some(shell(&standard.root)),
        schema: Some(standard.schema.clone()),
        previous: standard.metadata.get(ORIGIN).cloned(),
        previous_present: standard.metadata.contains_key(ORIGIN),
        ..Origins::default()
    };
    let mut blocks = forward(&standard.root.content, &mut origins);
    if blocks.is_empty() {
        blocks.push(wrap(ComponentNode::new("paragraph"), Vec::new()));
    }
    let mut metadata = standard.metadata.clone();
    metadata.insert(
        ORIGIN.into(),
        serde_json::to_value(origins).expect("structural shells serialize"),
    );
    ComponentDocumentV2 {
        schema: BLOCK_DOCUMENT_SCHEMA.into(),
        version: standard.version,
        root: ComponentNode::container(
            "document",
            None,
            vec![ComponentNode::container("block_children", None, blocks)],
        ),
        metadata,
    }
}

fn forward(nodes: &[ComponentNode], origins: &mut Origins) -> Vec<ComponentNode> {
    let mut result = Vec::new();
    for node in nodes {
        match node.kind.0.as_str() {
            "bullet_list" | "ordered_list" | "task_list" => {
                for (index, item) in node.content.iter().enumerate() {
                    let first = item.content.first().filter(|n| n.kind.0 == "paragraph");
                    let mut content = shell(item);
                    content.kind = match node.kind.0.as_str() {
                        "ordered_list" => "numbered_list_item",
                        "task_list" => "todo",
                        _ => "bulleted_list_item",
                    }
                    .into();
                    let (inline, image_children) = split_first_paragraph(first, origins);
                    content.content = inline;
                    if node.kind.0 == "ordered_list" && index == 0 {
                        content.attrs.insert(
                            "start".into(),
                            node.attrs.get("start").cloned().unwrap_or(json!(1)),
                        );
                    }
                    let mut children = image_children;
                    children.extend(forward(
                        &item.content[usize::from(first.is_some())..],
                        origins,
                    ));
                    let block = wrap(content, children);
                    origins.nodes.insert(
                        key(&block).into(),
                        Origin {
                            outer: Some(shell(node)),
                            paragraph: first.map(shell),
                            no_first_paragraph: first.is_none(),
                            ..Origin::default()
                        },
                    );
                    result.push(block);
                }
            }
            "blockquote" => {
                let first = node.content.first().filter(|n| n.kind.0 == "paragraph");
                let mut quote = shell(node);
                quote.kind = "quote".into();
                let (inline, mut children) = split_first_paragraph(first, origins);
                quote.content = inline;
                children.extend(forward(
                    &node.content[usize::from(first.is_some())..],
                    origins,
                ));
                let block = wrap(quote, children);
                origins.nodes.insert(
                    key(&block).into(),
                    Origin {
                        paragraph: first.map(shell),
                        no_first_paragraph: first.is_none(),
                        ..Origin::default()
                    },
                );
                result.push(block);
            }
            "paragraph" | "heading" if node.content.iter().any(|n| n.kind.0 == "image") => {
                let mut run = Vec::new();
                let mut used_original_id = false;
                for inline in &node.content {
                    if inline.kind.0 == "image" {
                        if !run.is_empty() {
                            let mut text = shell(node);
                            if used_original_id {
                                text.id = Some(fresh_id());
                            }
                            text.content = std::mem::take(&mut run);
                            result.push(wrap(text, Vec::new()));
                        }
                        let mut image = inline.clone();
                        image.attrs.remove("src");
                        image.attrs.insert("media".into(), json!({"type": "url", "url": inline.attrs.get("src").and_then(Value::as_str).unwrap_or_default()}));
                        let block = wrap(image, Vec::new());
                        let outer = if node.content.len() == 1 {
                            shell(node)
                        } else {
                            ComponentNode::container("paragraph", Some(fresh_id()), Vec::new())
                        };
                        origins.nodes.insert(
                            key(&block).into(),
                            Origin {
                                outer: Some(outer),
                                ..Origin::default()
                            },
                        );
                        result.push(block);
                        used_original_id = true;
                    } else {
                        run.push(inline.clone());
                    }
                }
                if !run.is_empty() {
                    let mut text = shell(node);
                    if used_original_id {
                        text.id = Some(fresh_id());
                    }
                    text.content = run;
                    result.push(wrap(text, Vec::new()));
                }
            }
            "table" => {
                let mut table = node.clone();
                table.attrs.insert(
                    "header_row".into(),
                    json!(node.content.first().is_some_and(|row| {
                        row.content.iter().all(|cell| cell.kind.0 == "table_header")
                    })),
                );
                let mut origin = Origin::default();
                for row in &mut table.content {
                    let original_row = shell(row);
                    row.id = Some(fresh_id());
                    origin.rows.insert(key(row).into(), original_row);
                    for cell in &mut row.content {
                        if let Some(paragraph) =
                            cell.content.first().filter(|n| n.kind.0 == "paragraph")
                        {
                            origin.cells.insert(key(cell).into(), shell(paragraph));
                        }
                        cell.kind = if cell.kind.0 == "table_header" {
                            "table_header_cell"
                        } else {
                            "table_cell"
                        }
                        .into();
                        cell.content = cell
                            .content
                            .iter()
                            .flat_map(|p| p.content.clone())
                            .collect();
                        for inline in &mut cell.content {
                            if inline.kind.0 == "image" {
                                let original = inline.clone();
                                if inline.id.is_none() {
                                    inline.id = Some(fresh_id());
                                }
                                origins.inline.insert(key(inline).into(), original);
                                let alt = inline
                                    .attrs
                                    .get("alt")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default()
                                    .to_string();
                                let src = inline
                                    .attrs
                                    .get("src")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default();
                                inline.kind = "opaque_markdown_inline".into();
                                inline.attrs = serde_json::from_value(json!({
                                    "source": format!("![{alt}]({src})"),
                                    "fallback": alt,
                                    "construct": "table_image",
                                }))
                                .expect("object attributes");
                            }
                        }
                    }
                }
                let block = wrap(table, Vec::new());
                origins.nodes.insert(key(&block).into(), origin);
                result.push(block);
            }
            "unknown_component" => {
                let mut unsupported = ComponentNode::new("unsupported");
                unsupported.attrs.insert(
                    "raw".into(),
                    serde_json::to_value(node).expect("component serializes"),
                );
                result.push(wrap(unsupported, Vec::new()));
            }
            _ => {
                let mut content = node.clone();
                match node.kind.0.as_str() {
                    "thematic_break" => content.kind = "divider".into(),
                    "opaque_markdown_block" => content.kind = "opaque_markdown".into(),
                    "code_block" => {
                        let info = content
                            .attrs
                            .get("info")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string();
                        if let Some(language) = info.split_whitespace().next() {
                            content.attrs.insert("language".into(), json!(language));
                        }
                        content.attrs.insert("wrap".into(), json!(false));
                    }
                    _ => {}
                }
                result.push(wrap(content, Vec::new()));
            }
        }
    }
    result
}

fn split_first_paragraph(
    first: Option<&ComponentNode>,
    origins: &mut Origins,
) -> (Vec<ComponentNode>, Vec<ComponentNode>) {
    let Some(first) = first else {
        return (Vec::new(), Vec::new());
    };
    if !first.content.iter().any(|n| n.kind.0 == "image") {
        return (first.content.clone(), Vec::new());
    }
    let mut blocks = forward(std::slice::from_ref(first), origins);
    let inline = if blocks
        .first()
        .and_then(|block| block.content.first())
        .is_some_and(|content| content.kind.0 == "paragraph")
    {
        blocks.remove(0).content.remove(0).content
    } else {
        Vec::new()
    };
    (inline, blocks)
}

/// Convert the current editing blocks back to standard CDV2. Structural source
/// shells restore codec identities without overwriting edited content.
pub fn to_standard(blocks: &ComponentDocumentV2) -> ComponentDocumentV2 {
    let origins: Origins = blocks
        .metadata
        .get(ORIGIN)
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default();
    let group = blocks
        .root
        .content
        .iter()
        .find(|n| n.kind.0 == "block_children");
    let mut root = origins
        .root
        .clone()
        .unwrap_or_else(|| ComponentNode::new("document"));
    root.content = backward(
        group.map(|n| n.content.as_slice()).unwrap_or_default(),
        &origins,
    );
    restore_inline_images(&mut root, &origins);
    ensure_standard_ids(&mut root);
    let mut metadata = blocks.metadata.clone();
    metadata.remove(ORIGIN);
    if origins.previous_present {
        metadata.insert(ORIGIN.into(), origins.previous.unwrap_or(Value::Null));
    }
    ComponentDocumentV2 {
        schema: origins
            .schema
            .unwrap_or_else(|| crate::COMPONENT_DOCUMENT_SCHEMA.into()),
        version: blocks.version,
        root,
        metadata,
    }
}

// The standard codec requires IDs for block nodes and inline atoms, while
// newly authored engine content has identity only on its block container.
fn ensure_standard_ids(node: &mut ComponentNode) {
    if node.id.is_none()
        && !["document", "text", "unknown_component"].contains(&node.kind.0.as_str())
    {
        node.id = Some(fresh_id());
    }
    for child in &mut node.content {
        ensure_standard_ids(child);
    }
}

fn restore_inline_images(node: &mut ComponentNode, origins: &Origins) {
    if node.kind.0 == "opaque_markdown_inline"
        && node.attrs.get("construct").and_then(Value::as_str) == Some("table_image")
        && let Some(original) = origins.inline.get(key(node))
    {
        *node = original.clone();
    }
    for child in &mut node.content {
        restore_inline_images(child, origins);
    }
}

fn paragraph(inlines: Vec<ComponentNode>, original: Option<&ComponentNode>) -> ComponentNode {
    let mut node = original
        .cloned()
        .unwrap_or_else(|| ComponentNode::container("paragraph", Some(fresh_id()), Vec::new()));
    node.content = inlines;
    node
}

fn backward(blocks: &[ComponentNode], origins: &Origins) -> Vec<ComponentNode> {
    let mut result: Vec<ComponentNode> = Vec::new();
    let mut last_list_source: Option<String> = None;
    for block in blocks {
        let Some(content) = block.content.first() else {
            continue;
        };
        let origin = origins.nodes.get(key(block));
        let children = block
            .content
            .iter()
            .find(|n| n.kind.0 == "block_children")
            .map(|n| backward(&n.content, origins))
            .unwrap_or_default();
        let mut node = content.clone();
        match content.kind.0.as_str() {
            "bulleted_list_item" | "numbered_list_item" | "todo" => {
                let list_kind = match content.kind.0.as_str() {
                    "numbered_list_item" => "ordered_list",
                    "todo" => "task_list",
                    _ => "bullet_list",
                };
                node.kind = if list_kind == "task_list" {
                    "task_item"
                } else {
                    "list_item"
                }
                .into();
                node.attrs.remove("start");
                node.content =
                    if origin.is_some_and(|o| o.no_first_paragraph) && content.content.is_empty() {
                        Vec::new()
                    } else {
                        vec![paragraph(
                            content.content.clone(),
                            origin.and_then(|o| o.paragraph.as_ref()),
                        )]
                    };
                node.content.extend(children);
                let source = origin.and_then(|o| o.outer.as_ref());
                let source_key = source.map(|n| key(n).to_string());
                // Keep separately authored list runs distinct when their source
                // identity is known; freshly inserted adjacent items coalesce.
                let merge = result.last().is_some_and(|n| n.kind.0 == list_kind)
                    && (source_key.is_none()
                        || last_list_source.is_none()
                        || source_key == last_list_source)
                    && !content.attrs.contains_key("start");
                if merge {
                    result.last_mut().unwrap().content.push(node);
                } else {
                    let mut list = source.cloned().unwrap_or_else(|| {
                        ComponentNode::container(list_kind, Some(fresh_id()), Vec::new())
                    });
                    list.kind = list_kind.into();
                    if list_kind == "ordered_list" {
                        list.attrs.insert(
                            "start".into(),
                            content.attrs.get("start").cloned().unwrap_or(json!(1)),
                        );
                    }
                    list.content.push(node);
                    result.push(list);
                }
                last_list_source = source_key;
                continue;
            }
            "quote" => {
                node.kind = "blockquote".into();
                node.content =
                    if origin.is_some_and(|o| o.no_first_paragraph) && content.content.is_empty() {
                        Vec::new()
                    } else {
                        vec![paragraph(
                            content.content.clone(),
                            origin.and_then(|o| o.paragraph.as_ref()),
                        )]
                    };
                node.content.extend(children);
            }
            "divider" => node.kind = "thematic_break".into(),
            "opaque_markdown" => node.kind = "opaque_markdown_block".into(),
            "code_block" => {
                let language = node
                    .attrs
                    .remove("language")
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_default();
                node.attrs.remove("wrap");
                // Preserve complete info strings until the language is changed.
                let previous = node
                    .attrs
                    .get("info")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if previous.split_whitespace().next().unwrap_or_default() != language {
                    if language.is_empty() {
                        node.attrs.remove("info");
                    } else {
                        node.attrs.insert("info".into(), json!(language));
                    }
                }
            }
            "image" => {
                let media = node.attrs.remove("media");
                let url = media
                    .as_ref()
                    .and_then(|m| m.get("url"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                node.attrs.insert("src".into(), json!(url));
                node.attrs.remove("width");
                node.attrs.remove("alignment");
                node.content.clear();
                node = paragraph(vec![node], origin.and_then(|o| o.outer.as_ref()));
            }
            "table" => {
                node.attrs.remove("header_row");
                node.attrs.remove("header_column");
                node.attrs.remove("column_widths");
                for row in &mut node.content {
                    if let Some(original) = origin.and_then(|o| o.rows.get(key(row))) {
                        row.id = original.id.clone();
                    }
                    for cell in &mut row.content {
                        let first = origin.and_then(|o| o.cells.get(key(cell)));
                        cell.kind = if cell.kind.0 == "table_header_cell" {
                            "table_header"
                        } else {
                            "table_cell"
                        }
                        .into();
                        cell.attrs.remove("background_color");
                        cell.content = vec![paragraph(std::mem::take(&mut cell.content), first)];
                    }
                }
            }
            "unsupported" => {
                if let Some(raw) = node
                    .attrs
                    .get("raw")
                    .and_then(|v| serde_json::from_value::<ComponentNode>(v.clone()).ok())
                {
                    node = raw;
                }
            }
            _ => {}
        }
        convert_entity_marks(&mut node);
        result.push(node);
        last_list_source = None;
    }
    result
}

fn convert_entity_marks(node: &mut ComponentNode) {
    for mark in &mut node.marks {
        if mark.kind.0 == "entity_link" {
            let entity = mark
                .attrs
                .remove("entity_id")
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default();
            mark.kind = "link".into();
            mark.attrs
                .insert("href".into(), json!(format!("semantic:entity:{entity}")));
        }
    }
    for child in &mut node.content {
        convert_entity_marks(child);
    }
}
