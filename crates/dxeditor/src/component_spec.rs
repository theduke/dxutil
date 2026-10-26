use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::document_v2::{
    COMPONENT_BLOCKQUOTE, COMPONENT_BULLET_LIST, COMPONENT_CODE_BLOCK, COMPONENT_DOCUMENT,
    COMPONENT_DOCUMENT_SCHEMA, COMPONENT_DOCUMENT_VERSION, COMPONENT_HARD_BREAK,
    COMPONENT_HEADING_V2, COMPONENT_IMAGE, COMPONENT_LIST_ITEM_V2, COMPONENT_MENTION_V2,
    COMPONENT_OPAQUE_MARKDOWN_BLOCK, COMPONENT_OPAQUE_MARKDOWN_INLINE, COMPONENT_ORDERED_LIST,
    COMPONENT_PARAGRAPH_V2, COMPONENT_TABLE_CELL_V2, COMPONENT_TABLE_HEADER,
    COMPONENT_TABLE_ROW_V2, COMPONENT_TABLE_V2, COMPONENT_TASK_ITEM, COMPONENT_TASK_LIST,
    COMPONENT_TEXT_V2, COMPONENT_THEMATIC_BREAK, COMPONENT_UNKNOWN, ComponentDocumentV2,
    ComponentId, ComponentNode, DocumentValidationError, MARK_BOLD_V2, MARK_CODE_V2,
    MARK_ITALIC_V2, MARK_LINK_V2, MARK_STRIKE, UnknownComponentPolicy, ValidationIssue,
    ValidationLimits,
};

pub const FORMAT_MARKDOWN: &str = "markdown";
pub const FORMAT_PLAIN_TEXT: &str = "plain_text";
pub const FORMAT_TYPED_DOCUMENT: &str = "typed_document";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentKind {
    Block,
    Inline,
    Atom,
    Mark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityPolicy {
    None,
    Optional,
    Required,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributeType {
    String,
    Boolean,
    Integer,
    Number,
    Object,
    Array,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UrlRole {
    Hyperlink,
    Media,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttributeSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_attribute: Option<String>,
    pub value_type: AttributeType,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum: Option<i64>,
    #[serde(default)]
    pub url: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url_role: Option<UrlRole>,
}

impl AttributeSpec {
    pub fn required(value_type: AttributeType) -> Self {
        Self {
            value_type,
            semantic_attribute: None,
            required: true,
            default: None,
            minimum: None,
            maximum: None,
            url: false,
            url_role: None,
        }
    }

    pub fn optional(value_type: AttributeType) -> Self {
        Self {
            value_type,
            semantic_attribute: None,
            required: false,
            default: None,
            minimum: None,
            maximum: None,
            url: false,
            url_role: None,
        }
    }

    pub fn with_default(mut self, value: Value) -> Self {
        self.default = Some(value);
        self
    }

    pub fn bounded(mut self, minimum: i64, maximum: i64) -> Self {
        self.minimum = Some(minimum);
        self.maximum = Some(maximum);
        self
    }

    pub fn url(mut self) -> Self {
        self.url = true;
        self.url_role = Some(UrlRole::Hyperlink);
        self
    }

    pub fn media_url(mut self) -> Self {
        self.url = true;
        self.url_role = Some(UrlRole::Media);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentRule {
    Empty,
    Group {
        group: String,
        min: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<usize>,
    },
    Kinds {
        kinds: Vec<ComponentId>,
        min: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<usize>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormatCapability {
    Native,
    Opaque,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlainTextFallback {
    TextContent,
    Attribute(String),
    Fixed(String),
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipboardPolicy {
    Structured,
    TextOnly,
    Opaque,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentBehavior {
    #[serde(default)]
    pub allows_children: bool,
    #[serde(default = "default_true")]
    pub editable: bool,
    #[serde(default = "default_true")]
    pub selectable: bool,
    #[serde(default)]
    pub draggable: bool,
    #[serde(default)]
    pub isolating: bool,
    #[serde(default)]
    pub defining: bool,
}

fn default_true() -> bool {
    true
}

impl Default for ComponentBehavior {
    fn default() -> Self {
        Self {
            allows_children: false,
            editable: true,
            selectable: true,
            draggable: false,
            isolating: false,
            defining: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomDescriptor {
    pub tag: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsertGroup {
    Basic,
    Media,
    Semantic,
    Advanced,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InsertSpec {
    pub label: String,
    pub description: String,
    pub icon: String,
    pub keywords: Vec<String>,
    pub group: InsertGroup,
    pub shortcut: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComponentSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insert: Option<InsertSpec>,
    pub id: ComponentId,
    pub component_version: u32,
    pub label: String,
    pub kind: ComponentKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    pub content: ContentRule,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, AttributeSpec>,
    pub identity: IdentityPolicy,
    pub behavior: ComponentBehavior,
    pub dom: DomDescriptor,
    pub clipboard: ClipboardPolicy,
    pub plain_text: PlainTextFallback,
    pub formats: BTreeMap<String, FormatCapability>,
}

impl ComponentSpec {
    pub fn new(
        id: impl Into<ComponentId>,
        label: impl Into<String>,
        kind: ComponentKind,
        group: Option<&str>,
        content: ContentRule,
        tag: impl Into<String>,
    ) -> Self {
        Self {
            semantic_class: None,
            insert: None,
            id: id.into(),
            component_version: 1,
            label: label.into(),
            kind,
            group: group.map(str::to_string),
            content,
            attributes: BTreeMap::new(),
            identity: if matches!(kind, ComponentKind::Block | ComponentKind::Atom) {
                IdentityPolicy::Required
            } else {
                IdentityPolicy::None
            },
            behavior: ComponentBehavior::default(),
            dom: DomDescriptor { tag: tag.into() },
            clipboard: ClipboardPolicy::Structured,
            plain_text: PlainTextFallback::TextContent,
            formats: standard_format_capabilities(FormatCapability::Native),
        }
    }

    pub fn attribute(mut self, name: impl Into<String>, spec: AttributeSpec) -> Self {
        self.attributes.insert(name.into(), spec);
        self
    }

    pub fn markdown_capability(mut self, capability: FormatCapability) -> Self {
        self.formats.insert(FORMAT_MARKDOWN.to_string(), capability);
        self
    }
}

fn standard_format_capabilities(markdown: FormatCapability) -> BTreeMap<String, FormatCapability> {
    BTreeMap::from([
        (FORMAT_MARKDOWN.to_string(), markdown),
        (FORMAT_PLAIN_TEXT.to_string(), FormatCapability::Native),
        (FORMAT_TYPED_DOCUMENT.to_string(), FormatCapability::Native),
    ])
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ComponentCatalogError {
    #[error("component '{0}' is already registered")]
    DuplicateComponent(String),
    #[error("component '{component}' has an invalid specification: {message}")]
    InvalidSpec { component: String, message: String },
}

#[derive(Clone, Debug, Default)]
pub struct ComponentCatalog {
    specs: BTreeMap<ComponentId, ComponentSpec>,
}

impl ComponentCatalog {
    pub fn standard() -> Result<Self, ComponentCatalogError> {
        let mut catalog = Self::default();
        register_standard_component_specs(&mut catalog)?;
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn register(&mut self, spec: ComponentSpec) -> Result<(), ComponentCatalogError> {
        validate_spec(&spec)?;
        if self.specs.contains_key(&spec.id) {
            return Err(ComponentCatalogError::DuplicateComponent(spec.id.0));
        }
        self.specs.insert(spec.id.clone(), spec);
        Ok(())
    }

    pub fn spec(&self, id: &ComponentId) -> Option<&ComponentSpec> {
        self.specs.get(id)
    }

    pub fn spec_by_id(&self, id: &str) -> Option<&ComponentSpec> {
        self.specs.get(&ComponentId::from(id))
    }

    pub fn specs(&self) -> impl Iterator<Item = &ComponentSpec> {
        self.specs.values()
    }

    pub fn validate(&self) -> Result<(), ComponentCatalogError> {
        let groups = self
            .specs
            .values()
            .filter_map(|spec| spec.group.as_deref())
            .flat_map(str::split_ascii_whitespace)
            .collect::<BTreeSet<_>>();

        for spec in self.specs.values() {
            match &spec.content {
                ContentRule::Group { group, .. } if !groups.contains(group.as_str()) => {
                    return Err(invalid_spec(
                        spec,
                        &format!("content expression references unknown group '{group}'"),
                    ));
                }
                ContentRule::Kinds { kinds, .. } => {
                    if let Some(kind) = kinds.iter().find(|kind| !self.specs.contains_key(*kind)) {
                        return Err(invalid_spec(
                            spec,
                            &format!(
                                "content expression references unknown component '{}'",
                                kind.0
                            ),
                        ));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn schema_fingerprint(&self) -> String {
        let canonical = serde_json::to_vec(&self.specs)
            .expect("component specifications are always JSON serializable");
        let digest = Sha256::digest(canonical);
        let mut fingerprint = String::with_capacity(digest.len() * 2);
        for byte in digest {
            use std::fmt::Write;
            write!(&mut fingerprint, "{byte:02x}").expect("writing to a String cannot fail");
        }
        fingerprint
    }
}

fn validate_spec(spec: &ComponentSpec) -> Result<(), ComponentCatalogError> {
    if spec.id.0.trim().is_empty() {
        return Err(invalid_spec(spec, "component ID must not be empty"));
    }
    if spec.component_version == 0 {
        return Err(invalid_spec(spec, "component version must be non-zero"));
    }
    if !is_safe_tag(&spec.dom.tag) {
        return Err(invalid_spec(
            spec,
            "DOM tag is not in the safe semantic allowlist",
        ));
    }
    for format in [FORMAT_MARKDOWN, FORMAT_PLAIN_TEXT, FORMAT_TYPED_DOCUMENT] {
        if !spec.formats.contains_key(format) {
            return Err(invalid_spec(
                spec,
                &format!("missing declared '{format}' format capability"),
            ));
        }
    }
    if spec.kind == ComponentKind::Mark && !matches!(spec.content, ContentRule::Empty) {
        return Err(invalid_spec(spec, "marks cannot contain child nodes"));
    }
    Ok(())
}

fn invalid_spec(spec: &ComponentSpec, message: &str) -> ComponentCatalogError {
    ComponentCatalogError::InvalidSpec {
        component: spec.id.0.clone(),
        message: message.to_string(),
    }
}

fn is_safe_tag(tag: &str) -> bool {
    matches!(
        tag,
        "article"
            | "div"
            | "p"
            | "h1"
            | "blockquote"
            | "ul"
            | "ol"
            | "li"
            | "pre"
            | "hr"
            | "table"
            | "tr"
            | "th"
            | "td"
            | "span"
            | "br"
            | "img"
            | "strong"
            | "em"
            | "s"
            | "code"
            | "a"
    )
}

pub fn validate_component_document(
    document: &ComponentDocumentV2,
    catalog: &ComponentCatalog,
    limits: &ValidationLimits,
    unknown_policy: UnknownComponentPolicy,
) -> Result<(), DocumentValidationError> {
    let mut validator = Validator {
        catalog,
        limits,
        unknown_policy,
        issues: Vec::new(),
        node_count: 0,
        text_bytes: 0,
        attribute_bytes: 0,
        ids: BTreeSet::new(),
    };

    if document.schema.0 != COMPONENT_DOCUMENT_SCHEMA
        && document.schema.0 != crate::profiles::block_document::BLOCK_DOCUMENT_SCHEMA
    {
        validator.issue(
            "unknown_schema",
            "$",
            format!("unsupported schema '{}'", document.schema.0),
        );
    }
    if document.version != COMPONENT_DOCUMENT_VERSION {
        validator.issue(
            "unknown_version",
            "$",
            format!("unsupported document version {}", document.version),
        );
    }
    if document.root.kind.0 != COMPONENT_DOCUMENT {
        validator.issue(
            "invalid_root",
            "$.root",
            format!("expected root kind '{COMPONENT_DOCUMENT}'"),
        );
    }

    validator.validate_node(&document.root, "$.root", 0);
    validator.validate_table_shapes(&document.root, "$.root");
    if document.schema.0 == crate::profiles::block_document::BLOCK_DOCUMENT_SCHEMA {
        crate::profiles::block_document::validate_structure(
            &document.root,
            catalog,
            "$.root",
            &mut validator.issues,
        );
    }

    if validator.issues.is_empty() {
        Ok(())
    } else {
        Err(DocumentValidationError::new(validator.issues))
    }
}

struct Validator<'a> {
    catalog: &'a ComponentCatalog,
    limits: &'a ValidationLimits,
    unknown_policy: UnknownComponentPolicy,
    issues: Vec<ValidationIssue>,
    node_count: usize,
    text_bytes: usize,
    attribute_bytes: usize,
    ids: BTreeSet<String>,
}

impl Validator<'_> {
    fn validate_node(&mut self, node: &ComponentNode, path: &str, depth: usize) {
        self.node_count += 1;
        if self.node_count > self.limits.max_nodes {
            self.issue("node_limit", path, "document exceeds the node-count limit");
            return;
        }
        if depth > self.limits.max_depth {
            self.issue(
                "depth_limit",
                path,
                "document exceeds the nesting-depth limit",
            );
            return;
        }

        if let Some(text) = &node.text {
            self.text_bytes = self.text_bytes.saturating_add(text.len());
            if self.text_bytes > self.limits.max_text_bytes {
                self.issue("text_limit", path, "document exceeds the text-size limit");
            }
        }
        self.attribute_bytes = self.attribute_bytes.saturating_add(
            serde_json::to_vec(&node.attrs)
                .map(|bytes| bytes.len())
                .unwrap_or(self.limits.max_attribute_bytes.saturating_add(1)),
        );
        if self.attribute_bytes > self.limits.max_attribute_bytes {
            self.issue(
                "attribute_limit",
                path,
                "document exceeds the attribute-size limit",
            );
        }

        let Some(spec) = self.catalog.spec(&node.kind) else {
            if self.unknown_policy == UnknownComponentPolicy::Reject {
                self.issue(
                    "unknown_component",
                    path,
                    format!("component '{}' is not registered", node.kind.0),
                );
            } else if !has_safe_unknown_fallback(node) {
                self.issue(
                    "unsafe_unknown_component",
                    path,
                    "unknown components require a string 'fallback' attribute",
                );
            }
            return;
        };

        self.validate_identity(node, spec, path);
        self.validate_shape(node, spec, path);
        self.validate_attributes(&node.attrs, &spec.attributes, path);

        for (mark_index, mark) in node.marks.iter().enumerate() {
            let mark_path = format!("{path}.marks[{mark_index}]");
            let Some(mark_spec) = self.catalog.spec(&mark.kind) else {
                self.issue(
                    "unknown_mark",
                    &mark_path,
                    format!("mark '{}' is not registered", mark.kind.0),
                );
                continue;
            };
            if mark_spec.kind != ComponentKind::Mark {
                self.issue(
                    "invalid_mark",
                    &mark_path,
                    "referenced component is not a mark",
                );
            }
            self.validate_attributes(&mark.attrs, &mark_spec.attributes, &mark_path);
        }

        for (index, child) in node.content.iter().enumerate() {
            self.validate_node(child, &format!("{path}.content[{index}]"), depth + 1);
        }
    }

    fn validate_identity(&mut self, node: &ComponentNode, spec: &ComponentSpec, path: &str) {
        match (spec.identity, &node.id) {
            (IdentityPolicy::Required, None) => {
                self.issue(
                    "missing_node_id",
                    path,
                    "component requires a stable node ID",
                );
            }
            (IdentityPolicy::None, Some(_)) => {
                self.issue(
                    "unexpected_node_id",
                    path,
                    "component must not have a node ID",
                );
            }
            (_, Some(id)) if id.0.trim().is_empty() => {
                self.issue("empty_node_id", path, "node ID must not be empty");
            }
            _ => {}
        }
        if let Some(id) = &node.id
            && !id.0.trim().is_empty()
            && !self.ids.insert(id.0.clone())
        {
            self.issue(
                "duplicate_node_id",
                path,
                format!("duplicate node ID '{}'", id.0),
            );
        }
    }

    fn validate_shape(&mut self, node: &ComponentNode, spec: &ComponentSpec, path: &str) {
        if node.kind.0 == COMPONENT_TEXT_V2 {
            if node.text.is_none() {
                self.issue("missing_text", path, "text node requires a text value");
            }
        } else if node.text.is_some() {
            self.issue(
                "unexpected_text",
                path,
                "only text nodes may have a text value",
            );
        }

        if !matches!(spec.kind, ComponentKind::Inline | ComponentKind::Atom)
            && !node.marks.is_empty()
        {
            self.issue("unexpected_marks", path, "block nodes cannot carry marks");
        }

        match &spec.content {
            ContentRule::Empty if !node.content.is_empty() => {
                self.issue(
                    "unexpected_content",
                    path,
                    "component cannot contain child nodes",
                );
            }
            ContentRule::Group { group, min, max } => {
                self.validate_content_count(node, *min, *max, path);
                for child in &node.content {
                    let in_group = self
                        .catalog
                        .spec(&child.kind)
                        .and_then(|child_spec| child_spec.group.as_deref())
                        .is_some_and(|groups| {
                            groups.split_ascii_whitespace().any(|item| item == group)
                        });
                    if !in_group {
                        self.issue(
                            "invalid_child",
                            path,
                            format!("child '{}' is not in group '{group}'", child.kind.0),
                        );
                    }
                }
            }
            ContentRule::Kinds { kinds, min, max } => {
                self.validate_content_count(node, *min, *max, path);
                for child in &node.content {
                    if !kinds.contains(&child.kind) {
                        self.issue(
                            "invalid_child",
                            path,
                            format!("child '{}' is not allowed", child.kind.0),
                        );
                    }
                }
            }
            ContentRule::Empty => {}
        }
    }

    fn validate_content_count(
        &mut self,
        node: &ComponentNode,
        min: usize,
        max: Option<usize>,
        path: &str,
    ) {
        if node.content.len() < min || max.is_some_and(|max| node.content.len() > max) {
            self.issue(
                "invalid_content_count",
                path,
                format!("component has {} child node(s)", node.content.len()),
            );
        }
    }

    fn validate_attributes(
        &mut self,
        attrs: &Map<String, Value>,
        specs: &BTreeMap<String, AttributeSpec>,
        path: &str,
    ) {
        for key in attrs.keys() {
            if !specs.contains_key(key) {
                self.issue(
                    "unknown_attribute",
                    path,
                    format!("attribute '{key}' is not declared"),
                );
            }
        }
        for (name, spec) in specs {
            let Some(value) = attrs.get(name) else {
                if spec.required && spec.default.is_none() {
                    self.issue(
                        "missing_attribute",
                        path,
                        format!("required attribute '{name}' is missing"),
                    );
                }
                continue;
            };
            if !attribute_matches(value, &spec.value_type) {
                self.issue(
                    "attribute_type",
                    path,
                    format!("attribute '{name}' has the wrong type"),
                );
                continue;
            }
            if let Some(integer) = value.as_i64()
                && (spec.minimum.is_some_and(|minimum| integer < minimum)
                    || spec.maximum.is_some_and(|maximum| integer > maximum))
            {
                self.issue(
                    "attribute_range",
                    path,
                    format!("attribute '{name}' is outside its allowed range"),
                );
            }
            if spec.url
                && let Some(url) = value.as_str()
                && !is_safe_url_for_role(url, spec.url_role.unwrap_or(UrlRole::Hyperlink))
            {
                self.issue(
                    "unsafe_url",
                    path,
                    format!("attribute '{name}' contains a forbidden URL"),
                );
            }
        }
    }

    fn validate_table_shapes(&mut self, node: &ComponentNode, path: &str) {
        if node.kind.0 == COMPONENT_TABLE_V2 {
            self.validate_table_geometry(node, path);
        }
        for (index, child) in node.content.iter().enumerate() {
            self.validate_table_shapes(child, &format!("{path}.content[{index}]"));
        }
    }

    fn validate_table_geometry(&mut self, table: &ComponentNode, path: &str) {
        let row_count = table.content.len();
        if row_count == 0 || row_count > self.limits.max_table_rows {
            self.issue(
                "table_row_limit",
                path,
                "table has an invalid number of rows",
            );
            return;
        }

        let mut occupied = vec![vec![false; self.limits.max_table_columns]; row_count];
        let mut effective_width = 0usize;
        let mut physical_cells = 0usize;
        let mut span_work = 0usize;

        for (row_index, row) in table.content.iter().enumerate() {
            let mut column = 0usize;
            for (cell_index, cell) in row.content.iter().enumerate() {
                physical_cells = physical_cells.saturating_add(1);
                while column < self.limits.max_table_columns && occupied[row_index][column] {
                    column += 1;
                }
                let colspan = cell
                    .attrs
                    .get("colspan")
                    .and_then(Value::as_u64)
                    .unwrap_or(1) as usize;
                let rowspan = cell
                    .attrs
                    .get("rowspan")
                    .and_then(Value::as_u64)
                    .unwrap_or(1) as usize;
                let cell_path = format!("{path}.content[{row_index}].content[{cell_index}]");
                if colspan == 0 || rowspan == 0 {
                    self.issue(
                        "invalid_table_span",
                        &cell_path,
                        "table spans must be positive",
                    );
                    continue;
                }
                if column.saturating_add(colspan) > self.limits.max_table_columns
                    || row_index.saturating_add(rowspan) > row_count
                {
                    self.issue(
                        "table_span_bounds",
                        &cell_path,
                        "table span exceeds table bounds",
                    );
                    continue;
                }
                span_work = span_work.saturating_add(colspan.saturating_mul(rowspan));
                if span_work > self.limits.max_table_span_work {
                    self.issue(
                        "table_span_work_limit",
                        path,
                        "table spans exceed the validation work limit",
                    );
                    return;
                }
                let mut overlaps = false;
                for row in occupied.iter().skip(row_index).take(rowspan) {
                    overlaps |= row[column..column + colspan].iter().any(|value| *value);
                }
                if overlaps {
                    self.issue("overlapping_table_span", &cell_path, "table cells overlap");
                    continue;
                }
                for row in occupied.iter_mut().skip(row_index).take(rowspan) {
                    row[column..column + colspan].fill(true);
                }
                if let Some(widths) = cell.attrs.get("colwidth") {
                    let valid = widths.as_array().is_some_and(|widths| {
                        widths.len() == colspan
                            && widths.iter().all(|width| {
                                width
                                    .as_u64()
                                    .is_some_and(|width| width > 0 && width <= 100_000)
                            })
                    });
                    if !valid {
                        self.issue(
                            "invalid_colwidth",
                            &cell_path,
                            "colwidth must contain one positive width per spanned column",
                        );
                    }
                }
                column += colspan;
                effective_width = effective_width.max(column);
            }
        }

        if physical_cells > self.limits.max_table_cells {
            self.issue(
                "table_cell_limit",
                path,
                "table exceeds the cell-count limit",
            );
            return;
        }
        if effective_width == 0 || effective_width > self.limits.max_table_columns {
            self.issue(
                "table_column_limit",
                path,
                "table has an invalid effective width",
            );
            return;
        }
        for (row_index, row) in occupied.iter().enumerate() {
            if row[..effective_width].iter().any(|value| !value) {
                self.issue(
                    "table_map_hole",
                    &format!("{path}.content[{row_index}]"),
                    "table geometry contains a hole or inconsistent effective width",
                );
            }
        }
    }

    fn issue(&mut self, code: &'static str, path: &str, message: impl Into<String>) {
        self.issues.push(ValidationIssue {
            code,
            path: path.to_string(),
            message: message.into(),
        });
    }
}

fn attribute_matches(value: &Value, value_type: &AttributeType) -> bool {
    match value_type {
        AttributeType::String => value.is_string(),
        AttributeType::Boolean => value.is_boolean(),
        AttributeType::Integer => value.as_i64().is_some() || value.as_u64().is_some(),
        AttributeType::Number => value.is_number(),
        AttributeType::Object => value.is_object(),
        AttributeType::Array => value.is_array(),
    }
}

fn has_safe_unknown_fallback(node: &ComponentNode) -> bool {
    node.attrs.get("fallback").is_some_and(Value::is_string)
        && node.text.is_none()
        && node.content.is_empty()
}

pub fn is_safe_url(url: &str) -> bool {
    is_safe_url_for_role(url, UrlRole::Hyperlink)
}

pub fn is_safe_media_url(url: &str) -> bool {
    is_safe_url_for_role(url, UrlRole::Media)
}

pub fn is_safe_url_for_role(url: &str, role: UrlRole) -> bool {
    if url.is_empty()
        || url != url.trim()
        || url
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
        || url.contains('\\')
    {
        return false;
    }
    let scheme = url
        .split_once(':')
        .map(|(scheme, _)| scheme.to_ascii_lowercase());
    match role {
        UrlRole::Media => url::Url::parse(url).is_ok_and(|parsed| {
            matches!(parsed.scheme(), "http" | "https") && parsed.host_str().is_some()
        }),
        UrlRole::Hyperlink => {
            if scheme.is_none() {
                return true;
            }
            url::Url::parse(url).is_ok_and(|parsed| match parsed.scheme() {
                "http" | "https" => parsed.host_str().is_some(),
                "mailto" | "tel" => !parsed.path().is_empty(),
                _ => false,
            })
        }
    }
}

pub fn register_standard_component_specs(
    catalog: &mut ComponentCatalog,
) -> Result<(), ComponentCatalogError> {
    let inline = || ContentRule::Group {
        group: "inline".to_string(),
        min: 0,
        max: None,
    };
    let blocks = |min| ContentRule::Group {
        group: "block".to_string(),
        min,
        max: None,
    };
    let empty = || ContentRule::Empty;

    let mut root = ComponentSpec::new(
        COMPONENT_DOCUMENT,
        "Document",
        ComponentKind::Block,
        None,
        blocks(1),
        "article",
    );
    root.identity = IdentityPolicy::None;
    root.behavior.editable = false;
    catalog.register(root)?;

    catalog.register(ComponentSpec::new(
        COMPONENT_PARAGRAPH_V2,
        "Paragraph",
        ComponentKind::Block,
        Some("block"),
        inline(),
        "p",
    ))?;
    catalog.register(
        ComponentSpec::new(
            COMPONENT_HEADING_V2,
            "Heading",
            ComponentKind::Block,
            Some("block"),
            inline(),
            "h1",
        )
        .attribute(
            "level",
            AttributeSpec::required(AttributeType::Integer)
                .with_default(json!(1))
                .bounded(1, 6),
        ),
    )?;
    catalog.register(ComponentSpec::new(
        COMPONENT_BLOCKQUOTE,
        "Blockquote",
        ComponentKind::Block,
        Some("block"),
        blocks(1),
        "blockquote",
    ))?;

    let list_items = ContentRule::Kinds {
        kinds: vec![COMPONENT_LIST_ITEM_V2.into()],
        min: 1,
        max: None,
    };
    catalog.register(ComponentSpec::new(
        COMPONENT_BULLET_LIST,
        "Bullet List",
        ComponentKind::Block,
        Some("block"),
        list_items.clone(),
        "ul",
    ))?;
    catalog.register(
        ComponentSpec::new(
            COMPONENT_ORDERED_LIST,
            "Ordered List",
            ComponentKind::Block,
            Some("block"),
            list_items,
            "ol",
        )
        .attribute(
            "start",
            AttributeSpec::optional(AttributeType::Integer)
                .with_default(json!(1))
                .bounded(1, i64::MAX),
        ),
    )?;
    catalog.register(ComponentSpec::new(
        COMPONENT_TASK_LIST,
        "Task List",
        ComponentKind::Block,
        Some("block"),
        ContentRule::Kinds {
            kinds: vec![COMPONENT_TASK_ITEM.into()],
            min: 1,
            max: None,
        },
        "ul",
    ))?;
    catalog.register(ComponentSpec::new(
        COMPONENT_LIST_ITEM_V2,
        "List Item",
        ComponentKind::Block,
        None,
        blocks(1),
        "li",
    ))?;
    catalog.register(
        ComponentSpec::new(
            COMPONENT_TASK_ITEM,
            "Task Item",
            ComponentKind::Block,
            None,
            blocks(1),
            "li",
        )
        .attribute(
            "checked",
            AttributeSpec::optional(AttributeType::Boolean).with_default(json!(false)),
        ),
    )?;

    catalog.register(
        ComponentSpec::new(
            COMPONENT_CODE_BLOCK,
            "Code Block",
            ComponentKind::Block,
            Some("block"),
            ContentRule::Kinds {
                kinds: vec![COMPONENT_TEXT_V2.into()],
                min: 0,
                max: Some(1),
            },
            "pre",
        )
        .attribute(
            "info",
            AttributeSpec::optional(AttributeType::String).with_default(json!("")),
        ),
    )?;
    catalog.register(ComponentSpec::new(
        COMPONENT_THEMATIC_BREAK,
        "Thematic Break",
        ComponentKind::Atom,
        Some("block"),
        empty(),
        "hr",
    ))?;

    catalog.register(ComponentSpec::new(
        COMPONENT_TABLE_V2,
        "Table",
        ComponentKind::Block,
        Some("block"),
        ContentRule::Kinds {
            kinds: vec![COMPONENT_TABLE_ROW_V2.into()],
            min: 1,
            max: None,
        },
        "table",
    ))?;
    catalog.register(ComponentSpec::new(
        COMPONENT_TABLE_ROW_V2,
        "Table Row",
        ComponentKind::Block,
        None,
        ContentRule::Kinds {
            kinds: vec![
                COMPONENT_TABLE_HEADER.into(),
                COMPONENT_TABLE_CELL_V2.into(),
            ],
            min: 1,
            max: None,
        },
        "tr",
    ))?;
    for (id, label, tag) in [
        (COMPONENT_TABLE_HEADER, "Table Header", "th"),
        (COMPONENT_TABLE_CELL_V2, "Table Cell", "td"),
    ] {
        catalog.register(
            ComponentSpec::new(id, label, ComponentKind::Block, None, blocks(1), tag)
                .attribute(
                    "colspan",
                    AttributeSpec::optional(AttributeType::Integer)
                        .with_default(json!(1))
                        .bounded(1, 1000),
                )
                .attribute(
                    "rowspan",
                    AttributeSpec::optional(AttributeType::Integer)
                        .with_default(json!(1))
                        .bounded(1, 1000),
                )
                .attribute("alignment", AttributeSpec::optional(AttributeType::String))
                .attribute("colwidth", AttributeSpec::optional(AttributeType::Array)),
        )?;
    }

    let mut text = ComponentSpec::new(
        COMPONENT_TEXT_V2,
        "Text",
        ComponentKind::Inline,
        Some("inline"),
        empty(),
        "span",
    );
    text.identity = IdentityPolicy::None;
    catalog.register(text)?;
    catalog.register(ComponentSpec::new(
        COMPONENT_HARD_BREAK,
        "Hard Break",
        ComponentKind::Atom,
        Some("inline"),
        empty(),
        "br",
    ))?;
    catalog.register(
        ComponentSpec::new(
            COMPONENT_IMAGE,
            "Image",
            ComponentKind::Atom,
            Some("inline"),
            empty(),
            "img",
        )
        .attribute(
            "src",
            AttributeSpec::required(AttributeType::String).media_url(),
        )
        .attribute(
            "alt",
            AttributeSpec::optional(AttributeType::String).with_default(json!("")),
        )
        .attribute("title", AttributeSpec::optional(AttributeType::String)),
    )?;
    catalog.register(
        ComponentSpec::new(
            COMPONENT_MENTION_V2,
            "Mention",
            ComponentKind::Atom,
            Some("inline"),
            empty(),
            "span",
        )
        .attribute("entity_id", AttributeSpec::required(AttributeType::String))
        .attribute("label", AttributeSpec::required(AttributeType::String)),
    )?;

    for (id, label, group) in [
        (
            COMPONENT_OPAQUE_MARKDOWN_BLOCK,
            "Opaque Markdown Block",
            "block",
        ),
        (
            COMPONENT_OPAQUE_MARKDOWN_INLINE,
            "Opaque Markdown Inline",
            "inline",
        ),
    ] {
        let mut spec =
            ComponentSpec::new(id, label, ComponentKind::Atom, Some(group), empty(), "span")
                .attribute("source", AttributeSpec::required(AttributeType::String))
                .attribute("fallback", AttributeSpec::required(AttributeType::String))
                .attribute("construct", AttributeSpec::optional(AttributeType::String))
                .markdown_capability(FormatCapability::Opaque);
        spec.clipboard = ClipboardPolicy::Opaque;
        spec.behavior.editable = false;
        catalog.register(spec)?;
    }

    let mut unknown = ComponentSpec::new(
        COMPONENT_UNKNOWN,
        "Unknown Component",
        ComponentKind::Atom,
        Some("block inline"),
        empty(),
        "span",
    )
    .attribute(
        "original_kind",
        AttributeSpec::required(AttributeType::String),
    )
    .attribute("fallback", AttributeSpec::required(AttributeType::String))
    .attribute("payload", AttributeSpec::required(AttributeType::Object))
    .markdown_capability(FormatCapability::Unsupported);
    unknown.identity = IdentityPolicy::Optional;
    unknown.clipboard = ClipboardPolicy::Opaque;
    unknown.behavior.editable = false;
    catalog.register(unknown)?;

    for (id, label, tag) in [
        (MARK_BOLD_V2, "Bold", "strong"),
        (MARK_ITALIC_V2, "Italic", "em"),
        (MARK_STRIKE, "Strikethrough", "s"),
        (MARK_CODE_V2, "Inline Code", "code"),
    ] {
        let mut spec = ComponentSpec::new(id, label, ComponentKind::Mark, None, empty(), tag);
        spec.identity = IdentityPolicy::None;
        catalog.register(spec)?;
    }
    let mut link = ComponentSpec::new(
        MARK_LINK_V2,
        "Link",
        ComponentKind::Mark,
        None,
        empty(),
        "a",
    )
    .attribute("href", AttributeSpec::required(AttributeType::String).url())
    .attribute("title", AttributeSpec::optional(AttributeType::String));
    link.identity = IdentityPolicy::None;
    catalog.register(link)?;

    Ok(())
}
