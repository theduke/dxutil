use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const DOCUMENT_SCHEMA_V1: &str = "dxeditor.document.v1";

pub const COMPONENT_PARAGRAPH: &str = "paragraph";
pub const COMPONENT_HEADING: &str = "heading";
pub const COMPONENT_QUOTE: &str = "quote";
pub const COMPONENT_CODE: &str = "code";
pub const COMPONENT_LIST: &str = "list";
pub const COMPONENT_LIST_ITEM: &str = "list_item";
pub const COMPONENT_DIVIDER: &str = "divider";
pub const COMPONENT_TEXT: &str = "text";
pub const COMPONENT_LINK: &str = "link";
pub const COMPONENT_MENTION: &str = "mention";
pub const COMPONENT_TABLE: &str = "table";
pub const COMPONENT_TABLE_ROW: &str = "table_row";
pub const COMPONENT_TABLE_CELL: &str = "table_cell";

pub const MARK_BOLD: &str = "bold";
pub const MARK_ITALIC: &str = "italic";
pub const MARK_CODE: &str = "code";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl NodeId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl From<&str> for NodeId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for NodeId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EditorDocument {
    pub schema: String,
    pub blocks: Vec<BlockNode>,
    pub meta: Map<String, Value>,
}

impl EditorDocument {
    pub fn new(blocks: Vec<BlockNode>) -> Self {
        Self {
            schema: DOCUMENT_SCHEMA_V1.to_string(),
            blocks,
            meta: Map::new(),
        }
    }

    pub fn empty() -> Self {
        Self::new(vec![BlockNode::paragraph("block-1", Vec::new())])
    }

    pub fn plain_text(text: impl Into<String>) -> Self {
        Self::new(vec![BlockNode::paragraph(
            "block-1",
            vec![InlineNode::text("text-1", text)],
        )])
    }

    pub fn text_content(&self) -> String {
        self.blocks
            .iter()
            .map(BlockNode::text_content)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Default for EditorDocument {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlockNode {
    pub id: NodeId,
    pub component: String,
    pub attrs: Map<String, Value>,
    pub content: NodeContent,
}

impl BlockNode {
    pub fn new(
        id: impl Into<NodeId>,
        component: impl Into<String>,
        attrs: Map<String, Value>,
        content: NodeContent,
    ) -> Self {
        Self {
            id: id.into(),
            component: component.into(),
            attrs,
            content,
        }
    }

    pub fn paragraph(id: impl Into<NodeId>, inline: Vec<InlineNode>) -> Self {
        Self::new(
            id,
            COMPONENT_PARAGRAPH,
            Map::new(),
            NodeContent::Inline(inline),
        )
    }

    pub fn heading(id: impl Into<NodeId>, level: u8, inline: Vec<InlineNode>) -> Self {
        let mut attrs = Map::new();
        attrs.insert("level".to_string(), Value::from(level));
        Self::new(id, COMPONENT_HEADING, attrs, NodeContent::Inline(inline))
    }

    pub fn text_content(&self) -> String {
        match &self.content {
            NodeContent::Inline(inline) => inline.iter().map(InlineNode::text_content).collect(),
            NodeContent::Blocks(blocks) => blocks
                .iter()
                .map(BlockNode::text_content)
                .collect::<Vec<_>>()
                .join("\n"),
            NodeContent::Table(table) => table
                .rows
                .iter()
                .map(|row| {
                    row.cells
                        .iter()
                        .map(|cell| {
                            cell.blocks
                                .iter()
                                .map(BlockNode::text_content)
                                .collect::<Vec<_>>()
                                .join(" ")
                        })
                        .collect::<Vec<_>>()
                        .join("\t")
                })
                .collect::<Vec<_>>()
                .join("\n"),
            NodeContent::Void => String::new(),
            NodeContent::Custom(_) => String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum NodeContent {
    Inline(Vec<InlineNode>),
    Blocks(Vec<BlockNode>),
    Table(TableNode),
    Void,
    Custom(Value),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InlineNode {
    pub id: NodeId,
    pub component: String,
    pub attrs: Map<String, Value>,
    pub text: String,
    pub marks: Vec<Mark>,
}

impl InlineNode {
    pub fn text(id: impl Into<NodeId>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            component: COMPONENT_TEXT.to_string(),
            attrs: Map::new(),
            text: text.into(),
            marks: Vec::new(),
        }
    }

    pub fn mention(
        id: impl Into<NodeId>,
        entity_id: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        let mut attrs = Map::new();
        attrs.insert("id".to_string(), Value::String(entity_id.into()));
        Self {
            id: id.into(),
            component: COMPONENT_MENTION.to_string(),
            attrs,
            text: label.into(),
            marks: Vec::new(),
        }
    }

    pub fn with_mark(mut self, mark: Mark) -> Self {
        self.marks.push(mark);
        self
    }

    pub fn text_content(&self) -> String {
        self.text.clone()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mark {
    pub component: String,
    pub attrs: Map<String, Value>,
}

impl Mark {
    pub fn new(component: impl Into<String>) -> Self {
        Self {
            component: component.into(),
            attrs: Map::new(),
        }
    }

    pub fn link(href: impl Into<String>) -> Self {
        let mut attrs = Map::new();
        attrs.insert("href".to_string(), Value::String(href.into()));
        Self {
            component: COMPONENT_LINK.to_string(),
            attrs,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableNode {
    pub rows: Vec<TableRow>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableRow {
    pub id: NodeId,
    pub attrs: Map<String, Value>,
    pub cells: Vec<TableCell>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableCell {
    pub id: NodeId,
    pub attrs: Map<String, Value>,
    pub blocks: Vec<BlockNode>,
}
