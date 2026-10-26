use serde_json::{Map, Value, json};

use crate::{
    document::{
        BlockNode, COMPONENT_CODE, COMPONENT_DIVIDER, COMPONENT_HEADING, COMPONENT_LINK,
        COMPONENT_LIST, COMPONENT_LIST_ITEM, COMPONENT_MENTION, COMPONENT_PARAGRAPH,
        COMPONENT_QUOTE, COMPONENT_TABLE, COMPONENT_TEXT, EditorDocument, InlineNode, Mark,
        NodeContent, NodeId, TableCell, TableNode, TableRow,
    },
    document_v2::{
        COMPONENT_BLOCKQUOTE, COMPONENT_BULLET_LIST, COMPONENT_CODE_BLOCK, COMPONENT_DOCUMENT,
        COMPONENT_HEADING_V2, COMPONENT_LIST_ITEM_V2, COMPONENT_MENTION_V2, COMPONENT_ORDERED_LIST,
        COMPONENT_PARAGRAPH_V2, COMPONENT_TABLE_CELL_V2, COMPONENT_TABLE_ROW_V2,
        COMPONENT_TABLE_V2, COMPONENT_TEXT_V2, COMPONENT_THEMATIC_BREAK, ComponentDocumentV2,
        ComponentMark, ComponentNode, MARK_BOLD_V2, MARK_CODE_V2, MARK_ITALIC_V2, MARK_LINK_V2,
        UnknownComponentPolicy,
    },
};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MigrationError {
    #[error("unsupported legacy component '{component}' at {path}")]
    UnsupportedLegacyComponent { component: String, path: String },
    #[error("component '{component}' cannot be represented by document v1 at {path}")]
    UnsupportedV2Component { component: String, path: String },
    #[error("component '{component}' is missing a stable node ID at {path}")]
    MissingNodeId { component: String, path: String },
    #[error("invalid migration data at {path}: {message}")]
    InvalidData { path: String, message: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MigrationOptions {
    pub unknown_components: UnknownComponentPolicy,
}

impl Default for MigrationOptions {
    fn default() -> Self {
        Self {
            unknown_components: UnknownComponentPolicy::Reject,
        }
    }
}

pub fn migrate_v1_to_v2(document: &EditorDocument) -> Result<ComponentDocumentV2, MigrationError> {
    migrate_v1_to_v2_with(document, MigrationOptions::default())
}

pub fn migrate_v1_to_v2_with(
    document: &EditorDocument,
    options: MigrationOptions,
) -> Result<ComponentDocumentV2, MigrationError> {
    let mut content = Vec::with_capacity(document.blocks.len());
    for (index, block) in document.blocks.iter().enumerate() {
        content.push(migrate_block_to_v2(
            block,
            &format!("$.blocks[{index}]"),
            options,
        )?);
    }

    let mut migrated = ComponentDocumentV2::new(content);
    migrated.metadata = document.meta.clone();
    Ok(migrated)
}

pub fn migrate_v2_to_v1(document: &ComponentDocumentV2) -> Result<EditorDocument, MigrationError> {
    if document.root.kind.0 != COMPONENT_DOCUMENT {
        return Err(MigrationError::InvalidData {
            path: "$.root".to_string(),
            message: format!("expected '{COMPONENT_DOCUMENT}' root"),
        });
    }

    let mut blocks = Vec::with_capacity(document.root.content.len());
    for (index, block) in document.root.content.iter().enumerate() {
        blocks.push(migrate_block_to_v1(
            block,
            &format!("$.root.content[{index}]"),
        )?);
    }

    let mut migrated = EditorDocument::new(blocks);
    migrated.meta = document.metadata.clone();
    Ok(migrated)
}

fn migrate_block_to_v2(
    block: &BlockNode,
    path: &str,
    options: MigrationOptions,
) -> Result<ComponentNode, MigrationError> {
    let kind = match block.component.as_str() {
        COMPONENT_PARAGRAPH => COMPONENT_PARAGRAPH_V2,
        COMPONENT_HEADING => COMPONENT_HEADING_V2,
        COMPONENT_QUOTE => COMPONENT_BLOCKQUOTE,
        COMPONENT_CODE => COMPONENT_CODE_BLOCK,
        COMPONENT_LIST => legacy_list_kind(&block.attrs),
        COMPONENT_LIST_ITEM => COMPONENT_LIST_ITEM_V2,
        COMPONENT_DIVIDER => COMPONENT_THEMATIC_BREAK,
        COMPONENT_TABLE => COMPONENT_TABLE_V2,
        other => return preserve_or_reject_legacy_block(block, other, path, options),
    };

    let content = match &block.content {
        NodeContent::Inline(inline) if block.component == COMPONENT_QUOTE => {
            vec![ComponentNode::paragraph(
                NodeId::new(format!("{}-paragraph", block.id.0)),
                migrate_inline_to_v2(inline, path, options)?,
            )]
        }
        NodeContent::Inline(inline) if block.component == COMPONENT_LIST => {
            vec![ComponentNode::container(
                COMPONENT_LIST_ITEM_V2,
                Some(NodeId::new(format!("{}-item", block.id.0))),
                vec![ComponentNode::paragraph(
                    NodeId::new(format!("{}-paragraph", block.id.0)),
                    migrate_inline_to_v2(inline, path, options)?,
                )],
            )]
        }
        NodeContent::Inline(inline) => migrate_inline_to_v2(inline, path, options)?,
        NodeContent::Blocks(blocks) => blocks
            .iter()
            .enumerate()
            .map(|(index, child)| {
                migrate_block_to_v2(child, &format!("{path}.content[{index}]"), options)
            })
            .collect::<Result<Vec<_>, MigrationError>>()?,
        NodeContent::Table(table) => migrate_table_to_v2(table, path, options)?,
        NodeContent::Void => Vec::new(),
        NodeContent::Custom(value) => {
            if options.unknown_components == UnknownComponentPolicy::Reject {
                return Err(MigrationError::UnsupportedLegacyComponent {
                    component: block.component.clone(),
                    path: path.to_string(),
                });
            }
            vec![unknown_node(
                &format!("{}.custom", block.component),
                block.id.clone(),
                value.clone(),
                "",
            )]
        }
    };

    let mut attrs = block.attrs.clone();
    if block.component == COMPONENT_LIST {
        attrs.remove("ordered");
        attrs.remove("kind");
        if kind == COMPONENT_ORDERED_LIST && !attrs.contains_key("start") {
            attrs.insert("start".to_string(), json!(1));
        }
    }

    Ok(ComponentNode {
        kind: kind.into(),
        id: Some(block.id.clone()),
        attrs,
        content,
        text: None,
        marks: Vec::new(),
    })
}

fn legacy_list_kind(attrs: &Map<String, Value>) -> &'static str {
    let ordered = attrs.get("ordered").and_then(Value::as_bool) == Some(true)
        || attrs.get("kind").and_then(Value::as_str) == Some("ordered");
    if ordered {
        COMPONENT_ORDERED_LIST
    } else {
        COMPONENT_BULLET_LIST
    }
}

fn migrate_inline_to_v2(
    inline: &[InlineNode],
    path: &str,
    options: MigrationOptions,
) -> Result<Vec<ComponentNode>, MigrationError> {
    inline
        .iter()
        .enumerate()
        .map(|(index, inline)| {
            let inline_path = format!("{path}.inline[{index}]");
            let marks = inline
                .marks
                .iter()
                .map(|mark| migrate_mark_to_v2(mark, &inline_path, options))
                .collect::<Result<Vec<_>, MigrationError>>()?;

            match inline.component.as_str() {
                COMPONENT_TEXT => Ok(ComponentNode {
                    kind: COMPONENT_TEXT_V2.into(),
                    id: None,
                    attrs: inline.attrs.clone(),
                    content: Vec::new(),
                    text: Some(inline.text.clone()),
                    marks,
                }),
                COMPONENT_MENTION => {
                    let Some(entity_id) = inline.attrs.get("id").and_then(Value::as_str) else {
                        return Err(MigrationError::InvalidData {
                            path: inline_path,
                            message: "mention requires a string 'id' attribute".to_string(),
                        });
                    };
                    Ok(ComponentNode {
                        kind: COMPONENT_MENTION_V2.into(),
                        id: Some(inline.id.clone()),
                        attrs: Map::from_iter([
                            ("entity_id".to_string(), json!(entity_id)),
                            ("label".to_string(), json!(inline.text)),
                        ]),
                        content: Vec::new(),
                        text: None,
                        marks,
                    })
                }
                other if options.unknown_components != UnknownComponentPolicy::Reject => {
                    Ok(unknown_node(
                        other,
                        inline.id.clone(),
                        serde_json::to_value(inline).map_err(|error| {
                            MigrationError::InvalidData {
                                path: inline_path,
                                message: error.to_string(),
                            }
                        })?,
                        &inline.text,
                    ))
                }
                other => Err(MigrationError::UnsupportedLegacyComponent {
                    component: other.to_string(),
                    path: inline_path,
                }),
            }
        })
        .collect()
}

fn migrate_mark_to_v2(
    mark: &Mark,
    path: &str,
    options: MigrationOptions,
) -> Result<ComponentMark, MigrationError> {
    let kind = match mark.component.as_str() {
        crate::document::MARK_BOLD => MARK_BOLD_V2,
        crate::document::MARK_ITALIC => MARK_ITALIC_V2,
        crate::document::MARK_CODE => MARK_CODE_V2,
        COMPONENT_LINK => MARK_LINK_V2,
        other => {
            if options.unknown_components == UnknownComponentPolicy::Reject {
                return Err(MigrationError::UnsupportedLegacyComponent {
                    component: other.to_string(),
                    path: path.to_string(),
                });
            }
            return Err(MigrationError::InvalidData {
                path: path.to_string(),
                message: format!("unknown mark '{other}' cannot be safely preserved"),
            });
        }
    };
    Ok(ComponentMark {
        kind: kind.into(),
        attrs: mark.attrs.clone(),
    })
}

fn migrate_table_to_v2(
    table: &TableNode,
    path: &str,
    options: MigrationOptions,
) -> Result<Vec<ComponentNode>, MigrationError> {
    table
        .rows
        .iter()
        .enumerate()
        .map(|(row_index, row)| {
            let row_path = format!("{path}.rows[{row_index}]");
            let cells = row
                .cells
                .iter()
                .enumerate()
                .map(|(cell_index, cell)| {
                    let cell_path = format!("{row_path}.cells[{cell_index}]");
                    let content = cell
                        .blocks
                        .iter()
                        .enumerate()
                        .map(|(index, block)| {
                            migrate_block_to_v2(
                                block,
                                &format!("{cell_path}.blocks[{index}]"),
                                options,
                            )
                        })
                        .collect::<Result<Vec<_>, MigrationError>>()?;
                    Ok(ComponentNode {
                        kind: COMPONENT_TABLE_CELL_V2.into(),
                        id: Some(cell.id.clone()),
                        attrs: cell.attrs.clone(),
                        content,
                        text: None,
                        marks: Vec::new(),
                    })
                })
                .collect::<Result<Vec<_>, MigrationError>>()?;
            Ok(ComponentNode {
                kind: COMPONENT_TABLE_ROW_V2.into(),
                id: Some(row.id.clone()),
                attrs: row.attrs.clone(),
                content: cells,
                text: None,
                marks: Vec::new(),
            })
        })
        .collect()
}

fn preserve_or_reject_legacy_block(
    block: &BlockNode,
    component: &str,
    path: &str,
    options: MigrationOptions,
) -> Result<ComponentNode, MigrationError> {
    if options.unknown_components == UnknownComponentPolicy::Reject {
        return Err(MigrationError::UnsupportedLegacyComponent {
            component: component.to_string(),
            path: path.to_string(),
        });
    }
    let payload = serde_json::to_value(block).map_err(|error| MigrationError::InvalidData {
        path: path.to_string(),
        message: error.to_string(),
    })?;
    Ok(unknown_node(
        component,
        block.id.clone(),
        payload,
        &block.text_content(),
    ))
}

fn unknown_node(kind: &str, id: NodeId, payload: Value, fallback: &str) -> ComponentNode {
    ComponentNode {
        kind: crate::document_v2::COMPONENT_UNKNOWN.into(),
        id: Some(id),
        attrs: Map::from_iter([
            ("original_kind".to_string(), json!(kind)),
            ("fallback".to_string(), json!(fallback)),
            (
                "payload".to_string(),
                match payload {
                    Value::Object(payload) => Value::Object(payload),
                    other => json!({ "value": other }),
                },
            ),
        ]),
        content: Vec::new(),
        text: None,
        marks: Vec::new(),
    }
}

fn migrate_block_to_v1(node: &ComponentNode, path: &str) -> Result<BlockNode, MigrationError> {
    let id = required_id(node, path)?;
    let component = match node.kind.0.as_str() {
        COMPONENT_PARAGRAPH_V2 => COMPONENT_PARAGRAPH,
        COMPONENT_HEADING_V2 => COMPONENT_HEADING,
        COMPONENT_BLOCKQUOTE => COMPONENT_QUOTE,
        COMPONENT_CODE_BLOCK => COMPONENT_CODE,
        COMPONENT_BULLET_LIST | COMPONENT_ORDERED_LIST => COMPONENT_LIST,
        COMPONENT_LIST_ITEM_V2 => COMPONENT_LIST_ITEM,
        COMPONENT_THEMATIC_BREAK => COMPONENT_DIVIDER,
        COMPONENT_TABLE_V2 => COMPONENT_TABLE,
        other => {
            return Err(MigrationError::UnsupportedV2Component {
                component: other.to_string(),
                path: path.to_string(),
            });
        }
    };

    let mut attrs = node.attrs.clone();
    if node.kind.0 == COMPONENT_ORDERED_LIST {
        attrs.insert("ordered".to_string(), Value::Bool(true));
    } else if node.kind.0 == COMPONENT_BULLET_LIST {
        attrs.insert("ordered".to_string(), Value::Bool(false));
    }

    let content = match node.kind.0.as_str() {
        COMPONENT_PARAGRAPH_V2 | COMPONENT_HEADING_V2 | COMPONENT_CODE_BLOCK => {
            NodeContent::Inline(migrate_inline_to_v1(&node.content, path)?)
        }
        COMPONENT_BLOCKQUOTE
        | COMPONENT_BULLET_LIST
        | COMPONENT_ORDERED_LIST
        | COMPONENT_LIST_ITEM_V2 => NodeContent::Blocks(
            node.content
                .iter()
                .enumerate()
                .map(|(index, child)| {
                    migrate_block_to_v1(child, &format!("{path}.content[{index}]"))
                })
                .collect::<Result<Vec<_>, MigrationError>>()?,
        ),
        COMPONENT_TABLE_V2 => NodeContent::Table(migrate_table_to_v1(node, path)?),
        COMPONENT_THEMATIC_BREAK => NodeContent::Void,
        _ => unreachable!("component mapping and content mapping must stay aligned"),
    };

    Ok(BlockNode::new(id, component, attrs, content))
}

fn migrate_inline_to_v1(
    nodes: &[ComponentNode],
    path: &str,
) -> Result<Vec<InlineNode>, MigrationError> {
    nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let node_path = format!("{path}.content[{index}]");
            let marks = node
                .marks
                .iter()
                .map(|mark| migrate_mark_to_v1(mark, &node_path))
                .collect::<Result<Vec<_>, MigrationError>>()?;
            match node.kind.0.as_str() {
                COMPONENT_TEXT_V2 => Ok(InlineNode {
                    id: NodeId::new(format!("{}-text-{}", path_id(path), index + 1)),
                    component: COMPONENT_TEXT.to_string(),
                    attrs: node.attrs.clone(),
                    text: node.text.clone().unwrap_or_default(),
                    marks,
                }),
                COMPONENT_MENTION_V2 => {
                    let id = required_id(node, &node_path)?;
                    let entity_id = node
                        .attrs
                        .get("entity_id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| MigrationError::InvalidData {
                            path: node_path.clone(),
                            message: "mention requires 'entity_id'".to_string(),
                        })?;
                    let label = node
                        .attrs
                        .get("label")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    Ok(InlineNode {
                        id,
                        component: COMPONENT_MENTION.to_string(),
                        attrs: Map::from_iter([("id".to_string(), json!(entity_id))]),
                        text: label.to_string(),
                        marks,
                    })
                }
                other => Err(MigrationError::UnsupportedV2Component {
                    component: other.to_string(),
                    path: node_path,
                }),
            }
        })
        .collect()
}

fn migrate_mark_to_v1(mark: &ComponentMark, path: &str) -> Result<Mark, MigrationError> {
    let component = match mark.kind.0.as_str() {
        MARK_BOLD_V2 => crate::document::MARK_BOLD,
        MARK_ITALIC_V2 => crate::document::MARK_ITALIC,
        MARK_CODE_V2 => crate::document::MARK_CODE,
        MARK_LINK_V2 => COMPONENT_LINK,
        other => {
            return Err(MigrationError::UnsupportedV2Component {
                component: other.to_string(),
                path: path.to_string(),
            });
        }
    };
    Ok(Mark {
        component: component.to_string(),
        attrs: mark.attrs.clone(),
    })
}

fn migrate_table_to_v1(node: &ComponentNode, path: &str) -> Result<TableNode, MigrationError> {
    let rows = node
        .content
        .iter()
        .enumerate()
        .map(|(row_index, row)| {
            let row_path = format!("{path}.content[{row_index}]");
            if row.kind.0 != COMPONENT_TABLE_ROW_V2 {
                return Err(MigrationError::InvalidData {
                    path: row_path,
                    message: "table child is not a row".to_string(),
                });
            }
            let cells = row
                .content
                .iter()
                .enumerate()
                .map(|(cell_index, cell)| {
                    let cell_path = format!("{row_path}.content[{cell_index}]");
                    if cell.kind.0 != COMPONENT_TABLE_CELL_V2 {
                        return Err(MigrationError::UnsupportedV2Component {
                            component: cell.kind.0.clone(),
                            path: cell_path,
                        });
                    }
                    let blocks = cell
                        .content
                        .iter()
                        .enumerate()
                        .map(|(index, block)| {
                            migrate_block_to_v1(block, &format!("{cell_path}.content[{index}]"))
                        })
                        .collect::<Result<Vec<_>, MigrationError>>()?;
                    Ok(TableCell {
                        id: required_id(cell, &cell_path)?,
                        attrs: cell.attrs.clone(),
                        blocks,
                    })
                })
                .collect::<Result<Vec<_>, MigrationError>>()?;
            Ok(TableRow {
                id: required_id(row, &row_path)?,
                attrs: row.attrs.clone(),
                cells,
            })
        })
        .collect::<Result<Vec<_>, MigrationError>>()?;
    Ok(TableNode { rows })
}

fn required_id(node: &ComponentNode, path: &str) -> Result<NodeId, MigrationError> {
    node.id
        .clone()
        .ok_or_else(|| MigrationError::MissingNodeId {
            component: node.kind.0.clone(),
            path: path.to_string(),
        })
}

fn path_id(path: &str) -> String {
    path.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect()
}
