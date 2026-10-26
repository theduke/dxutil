use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::document::NodeId;

pub const COMPONENT_DOCUMENT_SCHEMA: &str = "semantic.component-document";
pub const COMPONENT_DOCUMENT_VERSION: u32 = 2;
pub const COMPONENT_DOCUMENT_FORMAT: &str = "application/vnd.semantic.component-document+json";

pub const COMPONENT_DOCUMENT: &str = "document";
pub const COMPONENT_PARAGRAPH_V2: &str = "paragraph";
pub const COMPONENT_HEADING_V2: &str = "heading";
pub const COMPONENT_BLOCKQUOTE: &str = "blockquote";
pub const COMPONENT_BULLET_LIST: &str = "bullet_list";
pub const COMPONENT_ORDERED_LIST: &str = "ordered_list";
pub const COMPONENT_TASK_LIST: &str = "task_list";
pub const COMPONENT_LIST_ITEM_V2: &str = "list_item";
pub const COMPONENT_TASK_ITEM: &str = "task_item";
pub const COMPONENT_CODE_BLOCK: &str = "code_block";
pub const COMPONENT_THEMATIC_BREAK: &str = "thematic_break";
pub const COMPONENT_TABLE_V2: &str = "table";
pub const COMPONENT_TABLE_ROW_V2: &str = "table_row";
pub const COMPONENT_TABLE_HEADER: &str = "table_header";
pub const COMPONENT_TABLE_CELL_V2: &str = "table_cell";
pub const COMPONENT_TEXT_V2: &str = "text";
pub const COMPONENT_HARD_BREAK: &str = "hard_break";
pub const COMPONENT_IMAGE: &str = "image";
pub const COMPONENT_MENTION_V2: &str = "mention";
pub const COMPONENT_OPAQUE_MARKDOWN_BLOCK: &str = "opaque_markdown_block";
pub const COMPONENT_OPAQUE_MARKDOWN_INLINE: &str = "opaque_markdown_inline";
pub const COMPONENT_UNKNOWN: &str = "unknown_component";
pub const MARK_BOLD_V2: &str = "bold";
pub const MARK_ITALIC_V2: &str = "italic";
pub const MARK_STRIKE: &str = "strike";
pub const MARK_CODE_V2: &str = "code";
pub const MARK_LINK_V2: &str = "link";

pub type AttributeMap = Map<String, Value>;
pub type DocumentMetadata = Map<String, Value>;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DocumentSchemaId(pub String);

impl DocumentSchemaId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl From<&str> for DocumentSchemaId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ComponentId(pub String);

impl ComponentId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl From<&str> for ComponentId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for ComponentId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComponentDocumentV2 {
    pub schema: DocumentSchemaId,
    pub version: u32,
    pub root: ComponentNode,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub metadata: DocumentMetadata,
}

impl ComponentDocumentV2 {
    pub fn new(content: Vec<ComponentNode>) -> Self {
        Self {
            schema: COMPONENT_DOCUMENT_SCHEMA.into(),
            version: COMPONENT_DOCUMENT_VERSION,
            root: ComponentNode::container(COMPONENT_DOCUMENT, None, content),
            metadata: Map::new(),
        }
    }

    pub fn empty() -> Self {
        Self::new(vec![ComponentNode::paragraph(
            NodeId::new("paragraph-1"),
            Vec::new(),
        )])
    }

    pub fn plain_text(text: impl Into<String>) -> Self {
        Self::new(vec![ComponentNode::paragraph(
            NodeId::new("paragraph-1"),
            vec![ComponentNode::text(text)],
        )])
    }

    pub fn text_content(&self) -> String {
        self.root.text_content()
    }
}

impl Default for ComponentDocumentV2 {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComponentNode {
    pub kind: ComponentId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<NodeId>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub attrs: AttributeMap,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<ComponentNode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marks: Vec<ComponentMark>,
}

impl ComponentNode {
    pub fn new(kind: impl Into<ComponentId>) -> Self {
        Self {
            kind: kind.into(),
            id: None,
            attrs: Map::new(),
            content: Vec::new(),
            text: None,
            marks: Vec::new(),
        }
    }

    pub fn container(
        kind: impl Into<ComponentId>,
        id: Option<NodeId>,
        content: Vec<ComponentNode>,
    ) -> Self {
        Self {
            id,
            content,
            ..Self::new(kind)
        }
    }

    pub fn paragraph(id: NodeId, content: Vec<ComponentNode>) -> Self {
        Self::container(COMPONENT_PARAGRAPH_V2, Some(id), content)
    }

    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: Some(text.into()),
            ..Self::new(COMPONENT_TEXT_V2)
        }
    }

    pub fn with_attrs<T>(mut self, attrs: &T) -> Result<Self, serde_json::Error>
    where
        T: Serialize,
    {
        let Value::Object(attrs) = serde_json::to_value(attrs)? else {
            unreachable!("serializing an attribute struct must produce an object")
        };
        self.attrs = attrs;
        Ok(self)
    }

    pub fn typed_attrs<T>(&self) -> Result<T, serde_json::Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        serde_json::from_value(Value::Object(self.attrs.clone()))
    }

    pub fn text_content(&self) -> String {
        if let Some(text) = &self.text {
            return text.clone();
        }
        if self.kind.0 == COMPONENT_HARD_BREAK {
            return "\n".to_string();
        }

        let separator = if self.kind.0 == COMPONENT_DOCUMENT
            || self.kind.0 == COMPONENT_BLOCKQUOTE
            || self.kind.0.ends_with("list")
            || self.kind.0 == COMPONENT_LIST_ITEM_V2
            || self.kind.0 == COMPONENT_TASK_ITEM
            || self.kind.0 == COMPONENT_TABLE_V2
            || self.kind.0 == COMPONENT_TABLE_ROW_V2
        {
            "\n"
        } else if self.kind.0 == COMPONENT_TABLE_CELL_V2 || self.kind.0 == COMPONENT_TABLE_HEADER {
            " "
        } else {
            ""
        };

        self.content
            .iter()
            .map(ComponentNode::text_content)
            .collect::<Vec<_>>()
            .join(separator)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComponentMark {
    pub kind: ComponentId,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub attrs: AttributeMap,
}

impl ComponentMark {
    pub fn new(kind: impl Into<ComponentId>) -> Self {
        Self {
            kind: kind.into(),
            attrs: Map::new(),
        }
    }

    pub fn with_attrs<T>(mut self, attrs: &T) -> Result<Self, serde_json::Error>
    where
        T: Serialize,
    {
        let Value::Object(attrs) = serde_json::to_value(attrs)? else {
            unreachable!("serializing an attribute struct must produce an object")
        };
        self.attrs = attrs;
        Ok(self)
    }

    pub fn typed_attrs<T>(&self) -> Result<T, serde_json::Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        serde_json::from_value(Value::Object(self.attrs.clone()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeadingAttributes {
    pub level: u8,
}

impl Default for HeadingAttributes {
    fn default() -> Self {
        Self { level: 1 }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderedListAttributes {
    #[serde(default = "default_list_start")]
    pub start: u64,
}

impl Default for OrderedListAttributes {
    fn default() -> Self {
        Self { start: 1 }
    }
}

fn default_list_start() -> u64 {
    1
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskItemAttributes {
    #[serde(default)]
    pub checked: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeBlockAttributes {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub info: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableCellAttributes {
    #[serde(default = "default_span")]
    pub colspan: u32,
    #[serde(default = "default_span")]
    pub rowspan: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alignment: Option<TableAlignment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colwidth: Option<Vec<u32>>,
}

impl Default for TableCellAttributes {
    fn default() -> Self {
        Self {
            colspan: 1,
            rowspan: 1,
            alignment: None,
            colwidth: None,
        }
    }
}

fn default_span() -> u32 {
    1
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TableAlignment {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageAttributes {
    pub src: String,
    #[serde(default)]
    pub alt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MentionAttributes {
    pub entity_id: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkAttributes {
    pub href: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpaqueMarkdownAttributes {
    pub source: String,
    pub fallback: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub construct: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UnknownComponentAttributes {
    pub original_kind: String,
    pub fallback: String,
    pub payload: Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationLimits {
    pub max_depth: usize,
    pub max_nodes: usize,
    pub max_text_bytes: usize,
    pub max_attribute_bytes: usize,
    pub max_table_rows: usize,
    pub max_table_columns: usize,
    pub max_table_cells: usize,
    pub max_table_span_work: usize,
}

impl Default for ValidationLimits {
    fn default() -> Self {
        Self {
            max_depth: 64,
            max_nodes: 100_000,
            max_text_bytes: 10 * 1024 * 1024,
            max_attribute_bytes: 2 * 1024 * 1024,
            max_table_rows: 1_000,
            max_table_columns: 1_000,
            max_table_cells: 100_000,
            max_table_span_work: 1_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UnknownComponentPolicy {
    #[default]
    Reject,
    PreserveOpaque,
    ConvertWithWarning,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationIssue {
    pub code: &'static str,
    pub path: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("document validation failed with {issue_count} issue(s)")]
pub struct DocumentValidationError {
    pub issue_count: usize,
    pub issues: Vec<ValidationIssue>,
}

impl DocumentValidationError {
    pub(crate) fn new(issues: Vec<ValidationIssue>) -> Self {
        Self {
            issue_count: issues.len(),
            issues,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NormalizationOptions {
    pub repair_duplicate_ids: bool,
}

pub fn normalize_component_document(
    document: &mut ComponentDocumentV2,
    catalog: &crate::component_spec::ComponentCatalog,
    options: &NormalizationOptions,
) -> Result<(), DocumentValidationError> {
    let mut seen_ids = BTreeSet::new();
    let mut repairs = BTreeMap::<String, usize>::new();
    normalize_node(
        &mut document.root,
        catalog,
        options,
        &mut seen_ids,
        &mut repairs,
    )?;
    Ok(())
}

fn normalize_node(
    node: &mut ComponentNode,
    catalog: &crate::component_spec::ComponentCatalog,
    options: &NormalizationOptions,
    seen_ids: &mut BTreeSet<String>,
    repairs: &mut BTreeMap<String, usize>,
) -> Result<(), DocumentValidationError> {
    if let Some(spec) = catalog.spec(&node.kind) {
        for (name, attribute) in &spec.attributes {
            if !node.attrs.contains_key(name)
                && let Some(default) = &attribute.default
            {
                node.attrs.insert(name.clone(), default.clone());
            }
        }
    }

    if let Some(id) = &mut node.id
        && !seen_ids.insert(id.0.clone())
    {
        if !options.repair_duplicate_ids {
            return Err(DocumentValidationError::new(vec![ValidationIssue {
                code: "duplicate_node_id",
                path: "$".to_string(),
                message: format!("duplicate node ID '{}'", id.0),
            }]));
        }

        let base = id.0.clone();
        let suffix = repairs.entry(base.clone()).or_insert(1);
        loop {
            *suffix += 1;
            let candidate = format!("{base}~{suffix}");
            if seen_ids.insert(candidate.clone()) {
                id.0 = candidate;
                break;
            }
        }
    }

    for child in &mut node.content {
        normalize_node(child, catalog, options, seen_ids, repairs)?;
    }

    if node.kind.0 != COMPONENT_CODE_BLOCK {
        let mut normalized = Vec::with_capacity(node.content.len());
        for child in std::mem::take(&mut node.content) {
            if let Some(previous) = normalized.last_mut()
                && mergeable_text_nodes(previous, &child)
            {
                previous
                    .text
                    .get_or_insert_with(String::new)
                    .push_str(child.text.as_deref().unwrap_or_default());
                continue;
            }
            normalized.push(child);
        }
        node.content = normalized;
    }

    Ok(())
}

fn mergeable_text_nodes(left: &ComponentNode, right: &ComponentNode) -> bool {
    left.kind.0 == COMPONENT_TEXT_V2
        && right.kind.0 == COMPONENT_TEXT_V2
        && left.id.is_none()
        && right.id.is_none()
        && left.attrs == right.attrs
        && left.marks == right.marks
        && left.content.is_empty()
        && right.content.is_empty()
        && left.text.is_some()
        && right.text.is_some()
}
