//! Framework-neutral specifications for the block document wire profile.
use serde_json::json;

use crate::{
    AttributeSpec, AttributeType, ComponentCatalog, ComponentCatalogError, ComponentKind,
    ComponentNode, ComponentSpec, ContentRule, IdentityPolicy, ValidationIssue,
    component_spec::{InsertGroup, InsertSpec},
};

pub const BLOCK_DOCUMENT_SCHEMA: &str = "semantic.block-document";
pub const COLOR_TOKENS: [&str; 9] = [
    "gray", "brown", "orange", "yellow", "green", "blue", "purple", "pink", "red",
];

fn kinds(names: &[&str], min: usize, max: Option<usize>) -> ContentRule {
    ContentRule::Kinds {
        kinds: names.iter().map(|name| (*name).into()).collect(),
        min,
        max,
    }
}

pub fn register_block_document_specs(
    catalog: &mut ComponentCatalog,
) -> Result<(), ComponentCatalogError> {
    let content_kinds = [
        "paragraph",
        "heading",
        "bulleted_list_item",
        "numbered_list_item",
        "todo",
        "toggle",
        "quote",
        "callout",
        "code_block",
        "divider",
        "table",
        "image",
        "file",
        "bookmark",
        "entity_embed",
        "unsupported",
        "graph",
        "details_break",
    ];
    let mut root = ComponentSpec::new(
        "document",
        "Document",
        ComponentKind::Block,
        None,
        kinds(&["block_children"], 1, Some(1)),
        "div",
    );
    root.identity = IdentityPolicy::None;
    catalog.register(root)?;
    let mut group = ComponentSpec::new(
        "block_children",
        "Children",
        ComponentKind::Block,
        None,
        kinds(&["block"], 1, None),
        "div",
    );
    group.identity = IdentityPolicy::None;
    catalog.register(group)?;
    let mut container = ComponentSpec::new(
        "block",
        "Block",
        ComponentKind::Block,
        None,
        kinds(
            &[&content_kinds[..], &["block_children"]].concat(),
            1,
            Some(2),
        ),
        "div",
    );
    container.behavior.draggable = true;
    catalog.register(container)?;
    for (index, kind) in content_kinds.iter().enumerate() {
        let rule = match *kind {
            "divider" | "file" | "bookmark" | "entity_embed" | "unsupported" | "graph"
            | "details_break" => ContentRule::Empty,
            "table" => kinds(&["table_row"], 1, None),
            "code_block" => kinds(&["text"], 0, None),
            _ => ContentRule::Group {
                group: "inline".into(),
                min: 0,
                max: None,
            },
        };
        let tag = match *kind {
            "paragraph" => "p",
            "heading" => "h1",
            "quote" => "blockquote",
            "code_block" => "pre",
            "divider" => "hr",
            "table" => "table",
            _ => "div",
        };
        let (label, description, shortcut) = match *kind {
            "paragraph" => ("Text", "Start writing with plain text.", None),
            "heading" => (
                "Heading",
                "Organize your document with a heading.",
                Some("# Space"),
            ),
            "bulleted_list_item" => (
                "Bulleted list",
                "Create a simple list of items.",
                Some("- Space"),
            ),
            "numbered_list_item" => (
                "Numbered list",
                "Create a list with numbered steps.",
                Some("1. Space"),
            ),
            "todo" => (
                "To-do list",
                "Keep track of things to do.",
                Some("[] Space"),
            ),
            "toggle" => ("Toggle list", "Show or hide a group of blocks.", None),
            "quote" => ("Quote", "Highlight a quotation.", Some("> Space")),
            "callout" => ("Callout", "Draw attention to an important idea.", None),
            "code_block" => (
                "Code block",
                "Write code with a language label.",
                Some("``` Space"),
            ),
            "divider" => (
                "Divider",
                "Separate sections with a line.",
                Some("--- Space"),
            ),
            "table" => ("Table", "Organize information in rows and columns.", None),
            "image" => ("Image", "Upload an image with a caption.", None),
            "file" => ("File", "Attach a file to your document.", None),
            "bookmark" => ("Web bookmark", "Save a link to a website.", None),
            "entity_embed" => (
                "Embed entity",
                "Display an entity inside this document.",
                None,
            ),
            "graph" => ("Graph / mind map", "Connect ideas in a visual map.", None),
            "details_break" => (
                "Details",
                "Separate always-visible content from details.",
                None,
            ),
            _ => ("Unsupported block", "", None),
        };
        let mut spec = ComponentSpec::new(
            *kind,
            label,
            ComponentKind::Block,
            Some("block_content"),
            rule,
            tag,
        );
        spec.identity = IdentityPolicy::None;
        spec.behavior.allows_children = index < 8;
        spec.insert = (*kind != "unsupported").then(|| InsertSpec {
            label: spec.label.clone(),
            description: description.into(),
            icon: kind.to_string(),
            keywords: if *kind == "graph" {
                vec!["graph".into(), "mind map".into(), "flowchart".into()]
            } else {
                vec![kind.to_string()]
            },
            group: match *kind {
                "image" | "file" | "bookmark" => InsertGroup::Media,
                "entity_embed" => InsertGroup::Semantic,
                "table" | "code_block" | "graph" => InsertGroup::Advanced,
                _ => InsertGroup::Basic,
            },
            shortcut: shortcut.map(str::to_string),
        });
        if index < 8 {
            for name in ["text_color", "background_color"] {
                spec = spec.attribute(name, AttributeSpec::optional(AttributeType::String));
            }
        }
        match *kind {
            "graph" => {
                spec = spec
                    .attribute("title", AttributeSpec::optional(AttributeType::String))
                    .attribute("view", AttributeSpec::optional(AttributeType::Object))
                    .attribute("frame", AttributeSpec::optional(AttributeType::Object))
                    .attribute("nodes", AttributeSpec::optional(AttributeType::Array))
                    .attribute("edges", AttributeSpec::optional(AttributeType::Array))
                    .attribute("child_order", AttributeSpec::optional(AttributeType::Array));
            }
            "heading" => {
                spec = spec.attribute(
                    "level",
                    AttributeSpec::required(AttributeType::Integer).bounded(1, 6),
                )
            }
            "numbered_list_item" => {
                spec = spec.attribute(
                    "start",
                    AttributeSpec::optional(AttributeType::Integer).bounded(1, i64::MAX),
                )
            }
            "todo" => {
                spec = spec.attribute("checked", AttributeSpec::required(AttributeType::Boolean))
            }
            "callout" => {
                spec = spec.attribute(
                    "icon",
                    AttributeSpec::optional(AttributeType::String).with_default(json!("💡")),
                )
            }
            "code_block" => {
                spec = spec
                    .attribute("language", AttributeSpec::optional(AttributeType::String))
                    .attribute("wrap", AttributeSpec::optional(AttributeType::Boolean));
            }
            "table" => {
                spec = spec
                    .attribute(
                        "header_row",
                        AttributeSpec::required(AttributeType::Boolean),
                    )
                    .attribute(
                        "header_column",
                        AttributeSpec::required(AttributeType::Boolean),
                    )
                    .attribute(
                        "column_widths",
                        AttributeSpec::optional(AttributeType::Array),
                    );
            }
            "image" => {
                spec = spec
                    .attribute("media", AttributeSpec::optional(AttributeType::Object))
                    .attribute("alt", AttributeSpec::optional(AttributeType::String))
                    .attribute("width", AttributeSpec::optional(AttributeType::Number))
                    .attribute("alignment", AttributeSpec::optional(AttributeType::String));
            }
            "file" => {
                spec = spec.attribute("media", AttributeSpec::optional(AttributeType::Object))
            }
            "bookmark" => {
                spec = spec
                    .attribute("url", AttributeSpec::required(AttributeType::String).url())
                    .attribute("preview", AttributeSpec::optional(AttributeType::Object))
            }
            "entity_embed" => {
                spec = spec
                    .attribute("entity", AttributeSpec::required(AttributeType::String))
                    .attribute("view", AttributeSpec::optional(AttributeType::String))
                    .attribute("width", AttributeSpec::optional(AttributeType::Number))
            }
            "unsupported" => {
                spec = spec.attribute("raw", AttributeSpec::required(AttributeType::Object))
            }
            _ => {}
        }
        catalog.register(spec)?;
    }
    catalog.register(ComponentSpec::new(
        "table_row",
        "Row",
        ComponentKind::Block,
        None,
        kinds(&["table_cell", "table_header_cell"], 1, None),
        "tr",
    ))?;
    for (kind, tag) in [("table_cell", "td"), ("table_header_cell", "th")] {
        let mut cell = ComponentSpec::new(
            kind,
            "Cell",
            ComponentKind::Block,
            None,
            ContentRule::Group {
                group: "inline".into(),
                min: 0,
                max: None,
            },
            tag,
        )
        .attribute("alignment", AttributeSpec::optional(AttributeType::String))
        .attribute(
            "background_color",
            AttributeSpec::optional(AttributeType::String),
        );
        cell.identity = IdentityPolicy::None;
        catalog.register(cell)?;
    }
    for (kind, tag) in [("text", "span"), ("hard_break", "br"), ("mention", "a")] {
        let mut node = ComponentSpec::new(
            kind,
            kind,
            ComponentKind::Inline,
            Some("inline"),
            ContentRule::Empty,
            tag,
        );
        node.identity = IdentityPolicy::None;
        if kind == "mention" {
            node.insert = Some(InsertSpec {
                label: "Mention".into(),
                description: "Link to an entity inline.".into(),
                icon: "mention".into(),
                keywords: vec!["entity".into(), "@".into()],
                group: InsertGroup::Semantic,
                shortcut: Some("@".into()),
            });
            node = node
                .attribute("entity_id", AttributeSpec::required(AttributeType::String))
                .attribute("label", AttributeSpec::optional(AttributeType::String));
        }
        catalog.register(node)?;
    }
    for kind in [
        "bold",
        "italic",
        "underline",
        "strike",
        "code",
        "highlight",
        "text_color",
        "link",
        "entity_link",
    ] {
        let mut mark = ComponentSpec::new(
            kind,
            kind,
            ComponentKind::Mark,
            None,
            ContentRule::Empty,
            "span",
        );
        if ["highlight", "text_color"].contains(&kind) {
            mark = mark.attribute("color", AttributeSpec::required(AttributeType::String));
        }
        if kind == "link" {
            mark = mark
                .attribute("href", AttributeSpec::required(AttributeType::String).url())
                .attribute("title", AttributeSpec::optional(AttributeType::String));
        }
        if kind == "entity_link" {
            mark = mark.attribute("entity_id", AttributeSpec::required(AttributeType::String));
        }
        catalog.register(mark)?;
    }
    catalog.validate()
}

/// Build the notes editing schema without admitting commands that cannot round-trip
/// through Markdown. This does not alter the persisted standard Markdown schema.
pub fn register_markdown_notes_specs(
    catalog: &mut ComponentCatalog,
) -> Result<(), ComponentCatalogError> {
    let mut all = ComponentCatalog::default();
    register_block_document_specs(&mut all)?;
    let allowed = [
        "document",
        "block_children",
        "block",
        "paragraph",
        "heading",
        "bulleted_list_item",
        "numbered_list_item",
        "todo",
        "quote",
        "code_block",
        "divider",
        "table",
        "table_row",
        "table_cell",
        "table_header_cell",
        "image",
        "unsupported",
        "text",
        "hard_break",
        "mention",
        "bold",
        "italic",
        "strike",
        "code",
        "link",
        "entity_link",
    ];
    for mut spec in all
        .specs()
        .filter(|spec| allowed.contains(&spec.id.0.as_str()))
        .cloned()
    {
        if spec.id.0 == "block"
            && let ContentRule::Kinds { kinds, .. } = &mut spec.content
        {
            kinds.retain(|kind| allowed.contains(&kind.0.as_str()));
            kinds.push("opaque_markdown".into());
        }
        // Standard Markdown assigns identities to its inline/content nodes.
        // Retaining them here makes the projection exactly reversible.
        if !["document", "block_children", "block", "table_row"].contains(&spec.id.0.as_str()) {
            spec.identity = IdentityPolicy::Optional;
        }
        spec.attributes.remove("text_color");
        spec.attributes.remove("background_color");
        if spec.id.0 == "table" {
            spec.attributes.remove("header_column");
            spec.attributes.remove("column_widths");
        }
        if spec.id.0 == "code_block" {
            spec = spec.attribute("info", AttributeSpec::optional(AttributeType::String));
        }
        if spec.id.0 == "image" {
            spec = spec.attribute("title", AttributeSpec::optional(AttributeType::String));
        }
        catalog.register(spec)?;
    }
    for (kind, group, tag) in [
        ("opaque_markdown", "block_content", "pre"),
        ("opaque_markdown_inline", "inline", "code"),
    ] {
        let mut spec = ComponentSpec::new(
            kind,
            "Preserved Markdown",
            if group == "inline" {
                ComponentKind::Atom
            } else {
                ComponentKind::Block
            },
            Some(group),
            ContentRule::Empty,
            tag,
        )
        .attribute("source", AttributeSpec::required(AttributeType::String))
        .attribute("fallback", AttributeSpec::required(AttributeType::String))
        .attribute("construct", AttributeSpec::optional(AttributeType::String));
        spec.identity = IdentityPolicy::Optional;
        spec.behavior.editable = false;
        catalog.register(spec)?;
    }
    catalog.validate()
}

pub fn register_composer_specs(
    catalog: &mut ComponentCatalog,
) -> Result<(), ComponentCatalogError> {
    let mut all = ComponentCatalog::default();
    register_block_document_specs(&mut all)?;
    for mut spec in all
        .specs()
        .filter(|spec| {
            [
                "document",
                "block_children",
                "block",
                "paragraph",
                "text",
                "hard_break",
            ]
            .contains(&spec.id.0.as_str())
        })
        .cloned()
    {
        if spec.id.0 == "block" {
            spec.content = kinds(&["paragraph"], 1, Some(1));
        }
        if spec.id.0 == "paragraph" {
            spec.attributes.clear();
            spec.behavior.allows_children = false;
        }
        spec.insert = None;
        catalog.register(spec)?;
    }
    let mut chip = ComponentSpec::new(
        "chip",
        "Chip",
        ComponentKind::Atom,
        Some("inline"),
        ContentRule::Empty,
        "span",
    )
    .attribute("kind", AttributeSpec::required(AttributeType::String))
    .attribute("label", AttributeSpec::required(AttributeType::String))
    .attribute("payload", AttributeSpec::required(AttributeType::String));
    chip.identity = IdentityPolicy::None;
    chip.behavior.editable = false;
    catalog.register(chip)?;
    catalog.validate()
}

pub(crate) fn validate_structure(
    node: &ComponentNode,
    catalog: &ComponentCatalog,
    path: &str,
    issues: &mut Vec<ValidationIssue>,
) {
    let mut issue = |code: &'static str, message: &str| {
        issues.push(ValidationIssue {
            code,
            path: path.into(),
            message: message.into(),
        })
    };
    if ["block", "table_row"].contains(&node.kind.0.as_str())
        && node
            .id
            .as_ref()
            .is_none_or(|id| uuid::Uuid::parse_str(&id.0).is_err())
    {
        issue("invalid_uuid", "block and row identities must be UUIDs");
    }
    if node.kind.0 == "block" {
        let content = node.content.first();
        if content.is_none_or(|n| {
            catalog.spec(&n.kind).and_then(|s| s.group.as_deref()) != Some("block_content")
        }) {
            issue(
                "invalid_block_content",
                "a block must begin with exactly one content node",
            );
        }
        if let Some(children) = node.content.get(1)
            && (children.kind.0 != "block_children"
                || content
                    .and_then(|n| catalog.spec(&n.kind))
                    .is_none_or(|s| !s.behavior.allows_children))
        {
            issue("invalid_block_children", "this block cannot have children");
        }
    }
    for name in ["color", "text_color", "background_color"] {
        if node.attrs.get(name).is_some_and(|v| {
            v.as_str()
                .is_none_or(|token| !COLOR_TOKENS.contains(&token))
        }) {
            issue("invalid_color", "expected a theme colour token");
        }
        for mark in &node.marks {
            if mark.attrs.get(name).is_some_and(|v| {
                v.as_str()
                    .is_none_or(|token| !COLOR_TOKENS.contains(&token))
            }) {
                issue("invalid_color", "expected a theme colour token");
            }
        }
    }
    for (index, child) in node.content.iter().enumerate() {
        validate_structure(child, catalog, &format!("{path}.content[{index}]"), issues);
    }
}
