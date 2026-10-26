use dxeditor::document_v2::{
    COMPONENT_DOCUMENT_FORMAT, COMPONENT_HEADING_V2, COMPONENT_IMAGE, COMPONENT_PARAGRAPH_V2,
    COMPONENT_TABLE_CELL_V2, COMPONENT_TABLE_ROW_V2, COMPONENT_TABLE_V2, COMPONENT_UNKNOWN,
    HeadingAttributes, MARK_BOLD_V2,
};
use dxeditor::{
    AttributeSpec, AttributeType, ComponentCatalog, ComponentCatalogError, ComponentDocumentV2,
    ComponentKind, ComponentNode, ComponentSpec, ContentRule, DecodeOptions, DocumentFormat,
    EditorCatalog, EditorDocument, EditorPayload, EncodeOptions, FormatCapability, IdentityPolicy,
    NormalizationOptions, PlainTextFallback, TypedDocumentCodec, UnknownComponentPolicy,
    ValidationLimits, migrate_v1_to_v2, migrate_v2_to_v1, normalize_component_document,
    validate_component_document,
};
use dxeditor::{BlockNode, InlineNode, Mark, NodeContent, NodeId};
use serde_json::{Map, Value, json};

#[test]
fn standard_document_validates_and_typed_attributes_round_trip() {
    let catalog = ComponentCatalog::standard().unwrap();
    let heading = ComponentNode::container(
        COMPONENT_HEADING_V2,
        Some(NodeId::new("heading-a")),
        vec![ComponentNode::text("Title")],
    )
    .with_attrs(&HeadingAttributes { level: 2 })
    .unwrap();
    let document = ComponentDocumentV2::new(vec![heading.clone()]);

    validate_component_document(
        &document,
        &catalog,
        &ValidationLimits::default(),
        UnknownComponentPolicy::Reject,
    )
    .unwrap();
    assert_eq!(
        heading.typed_attrs::<HeadingAttributes>().unwrap(),
        HeadingAttributes { level: 2 }
    );
}

#[test]
fn normalization_is_idempotent_and_repairs_only_when_requested() {
    let catalog = ComponentCatalog::standard().unwrap();
    let mut heading = ComponentNode::container(
        COMPONENT_HEADING_V2,
        Some(NodeId::new("duplicate")),
        vec![ComponentNode::text("a"), ComponentNode::text("b")],
    );
    let paragraph =
        ComponentNode::paragraph(NodeId::new("duplicate"), vec![ComponentNode::text("body")]);
    let mut document = ComponentDocumentV2::new(vec![heading.clone(), paragraph]);

    assert!(
        normalize_component_document(&mut document, &catalog, &NormalizationOptions::default())
            .is_err()
    );

    heading.id = Some(NodeId::new("duplicate"));
    let paragraph =
        ComponentNode::paragraph(NodeId::new("duplicate"), vec![ComponentNode::text("body")]);
    let mut document = ComponentDocumentV2::new(vec![heading, paragraph]);
    normalize_component_document(
        &mut document,
        &catalog,
        &NormalizationOptions {
            repair_duplicate_ids: true,
        },
    )
    .unwrap();
    let normalized = document.clone();
    normalize_component_document(
        &mut document,
        &catalog,
        &NormalizationOptions {
            repair_duplicate_ids: true,
        },
    )
    .unwrap();

    assert_eq!(document, normalized);
    assert_eq!(document.root.content[0].attrs["level"], json!(1));
    assert_eq!(document.root.content[0].content.len(), 1);
    assert_eq!(
        document.root.content[0].content[0].text.as_deref(),
        Some("ab")
    );
    assert_eq!(
        document.root.content[1].id.as_ref().unwrap().0,
        "duplicate~2"
    );
}

#[test]
fn validation_rejects_unsafe_urls_and_invalid_tree_shapes() {
    let catalog = ComponentCatalog::standard().unwrap();
    let image = ComponentNode {
        kind: COMPONENT_IMAGE.into(),
        id: Some(NodeId::new("image-a")),
        attrs: Map::from_iter([
            ("src".to_string(), json!("javascript:alert(1)")),
            ("alt".to_string(), json!("bad")),
        ]),
        content: vec![ComponentNode::text("not allowed")],
        text: None,
        marks: Vec::new(),
    };
    let document = ComponentDocumentV2::new(vec![image]);
    let error = validate_component_document(
        &document,
        &catalog,
        &ValidationLimits::default(),
        UnknownComponentPolicy::Reject,
    )
    .unwrap_err();

    assert!(error.issues.iter().any(|issue| issue.code == "unsafe_url"));
    assert!(
        error
            .issues
            .iter()
            .any(|issue| issue.code == "unexpected_content")
    );
}

#[test]
fn url_roles_distinguish_navigation_from_media_sources() {
    use dxeditor::{is_safe_media_url, is_safe_url};

    let fixtures: Vec<Value> =
        serde_json::from_str(include_str!("../assets/url_policy_cases.json")).unwrap();
    for fixture in fixtures {
        let value = fixture["value"].as_str().unwrap();
        assert_eq!(
            is_safe_url(value),
            fixture["hyperlink"].as_bool().unwrap(),
            "hyperlink: {value}"
        );
        assert_eq!(
            is_safe_media_url(value),
            fixture["media"].as_bool().unwrap(),
            "media: {value}"
        );
    }
}

#[test]
fn catalog_fingerprint_is_deterministic_and_registration_is_checked() {
    fn custom_spec(id: &str) -> ComponentSpec {
        let mut spec = ComponentSpec::new(
            id,
            id,
            ComponentKind::Atom,
            Some("inline"),
            ContentRule::Empty,
            "span",
        );
        spec.identity = IdentityPolicy::Optional;
        spec.plain_text = PlainTextFallback::Fixed(id.to_string());
        spec
    }

    let mut first = ComponentCatalog::default();
    first.register(custom_spec("a")).unwrap();
    first.register(custom_spec("b")).unwrap();
    let mut second = ComponentCatalog::default();
    second.register(custom_spec("b")).unwrap();
    second.register(custom_spec("a")).unwrap();

    assert_eq!(first.schema_fingerprint(), second.schema_fingerprint());
    assert_eq!(first.schema_fingerprint().len(), 64);
    assert!(matches!(
        first.register(custom_spec("a")),
        Err(ComponentCatalogError::DuplicateComponent(_))
    ));

    let unsafe_spec = ComponentSpec::new(
        "unsafe",
        "Unsafe",
        ComponentKind::Atom,
        Some("inline"),
        ContentRule::Empty,
        "script",
    );
    assert!(matches!(
        first.register(unsafe_spec),
        Err(ComponentCatalogError::InvalidSpec { .. })
    ));
}

#[test]
fn typed_codec_validates_and_round_trips() {
    let catalog = ComponentCatalog::standard().unwrap();
    let codec = TypedDocumentCodec::new();
    let document = ComponentDocumentV2::plain_text("hello");

    let encoded = codec
        .encode(&document, &catalog, EncodeOptions::default())
        .unwrap();
    assert_eq!(encoded.payload.format, COMPONENT_DOCUMENT_FORMAT);
    let decoded = codec
        .decode(&encoded.payload, &catalog, DecodeOptions::default())
        .unwrap();

    assert_eq!(decoded.document, document);
}

#[test]
fn typed_codec_preserves_unknown_nodes_only_with_an_explicit_policy_and_fallback() {
    let catalog = ComponentCatalog::standard().unwrap();
    let codec = TypedDocumentCodec::new();
    let unknown = ComponentNode {
        kind: "future_widget".into(),
        id: Some(NodeId::new("future-a")),
        attrs: Map::from_iter([("fallback".to_string(), json!("Future widget"))]),
        content: Vec::new(),
        text: None,
        marks: Vec::new(),
    };
    let payload = EditorPayload::new(
        COMPONENT_DOCUMENT_FORMAT,
        serde_json::to_value(ComponentDocumentV2::new(vec![unknown])).unwrap(),
    );

    assert!(
        codec
            .decode(&payload, &catalog, DecodeOptions::default())
            .is_err()
    );
    let decoded = codec
        .decode(
            &payload,
            &catalog,
            DecodeOptions {
                unknown_components: UnknownComponentPolicy::PreserveOpaque,
                ..DecodeOptions::default()
            },
        )
        .unwrap();
    assert_eq!(decoded.document.root.content[0].kind.0, COMPONENT_UNKNOWN);
    assert_eq!(decoded.diagnostics[0].code, "unknown_component_preserved");
}

#[test]
fn legacy_migration_is_semantically_stable() {
    let legacy = EditorDocument::new(vec![BlockNode::new(
        "heading-a",
        dxeditor::document::COMPONENT_HEADING,
        Map::from_iter([("level".to_string(), json!(2))]),
        NodeContent::Inline(vec![
            InlineNode::text("text-a", "Title").with_mark(Mark::new(dxeditor::document::MARK_BOLD)),
        ]),
    )]);

    let migrated = migrate_v1_to_v2(&legacy).unwrap();
    let restored = migrate_v2_to_v1(&migrated).unwrap();
    let migrated_again = migrate_v1_to_v2(&restored).unwrap();

    assert_eq!(migrated.text_content(), "Title");
    assert_eq!(migrated.root.content[0].kind.0, COMPONENT_HEADING_V2);
    assert_eq!(
        migrated.root.content[0].content[0].marks[0].kind.0,
        MARK_BOLD_V2
    );
    assert_eq!(migrated_again, migrated);
}

#[test]
fn editor_catalog_exposes_legacy_and_standard_document_formats() {
    let catalog = EditorCatalog::default();
    assert!(
        catalog
            .document_formats()
            .format("dxeditor.document.v1")
            .is_some()
    );
    assert!(
        catalog
            .document_formats()
            .format(COMPONENT_DOCUMENT_FORMAT)
            .is_some()
    );
    assert_eq!(catalog.schema_fingerprint().len(), 64);

    let payload = EditorPayload::new("plain_text", Value::String("body".to_string()));
    let decoded = catalog
        .document_formats()
        .decode(
            &payload,
            catalog.component_specs(),
            DecodeOptions::default(),
        )
        .unwrap();
    assert_eq!(decoded.document.text_content(), "body");
}

#[test]
fn json_schema_artifact_and_component_capabilities_are_present() {
    let schema: Value = serde_json::from_str(dxeditor::TYPED_DOCUMENT_JSON_SCHEMA).unwrap();
    assert_eq!(schema["properties"]["version"]["const"], json!(2));

    let catalog = ComponentCatalog::standard().unwrap();
    for spec in catalog.specs() {
        assert!(spec.formats.contains_key(dxeditor::FORMAT_MARKDOWN));
        assert!(spec.formats.contains_key(dxeditor::FORMAT_PLAIN_TEXT));
        assert!(spec.formats.contains_key(dxeditor::FORMAT_TYPED_DOCUMENT));
        assert_ne!(
            spec.formats[dxeditor::FORMAT_TYPED_DOCUMENT],
            FormatCapability::Unsupported
        );
    }
}

#[test]
fn validation_limits_are_enforced() {
    let catalog = ComponentCatalog::standard().unwrap();
    let document = ComponentDocumentV2::plain_text("too long");
    let error = validate_component_document(
        &document,
        &catalog,
        &ValidationLimits {
            max_text_bytes: 3,
            ..ValidationLimits::default()
        },
        UnknownComponentPolicy::Reject,
    )
    .unwrap_err();

    assert!(error.issues.iter().any(|issue| issue.code == "text_limit"));
}

fn table_cell(id: &str, paragraph_id: &str) -> ComponentNode {
    ComponentNode::container(
        COMPONENT_TABLE_CELL_V2,
        Some(NodeId::new(id)),
        vec![ComponentNode::container(
            COMPONENT_PARAGRAPH_V2,
            Some(NodeId::new(paragraph_id)),
            Vec::new(),
        )],
    )
}

#[test]
fn table_validation_uses_effective_span_geometry() {
    let catalog = ComponentCatalog::standard().unwrap();
    let mut spanning = table_cell("cell-a", "paragraph-a");
    spanning.attrs.insert("rowspan".to_string(), json!(2));
    let table = ComponentNode::container(
        COMPONENT_TABLE_V2,
        Some(NodeId::new("table")),
        vec![
            ComponentNode::container(
                COMPONENT_TABLE_ROW_V2,
                Some(NodeId::new("row-a")),
                vec![spanning, table_cell("cell-b", "paragraph-b")],
            ),
            ComponentNode::container(
                COMPONENT_TABLE_ROW_V2,
                Some(NodeId::new("row-b")),
                vec![table_cell("cell-c", "paragraph-c")],
            ),
        ],
    );
    validate_component_document(
        &ComponentDocumentV2::new(vec![table]),
        &catalog,
        &ValidationLimits::default(),
        UnknownComponentPolicy::Reject,
    )
    .unwrap();
}

#[test]
fn table_validation_rejects_holes_bad_widths_and_out_of_bounds_spans() {
    let catalog = ComponentCatalog::standard().unwrap();
    let mut wide = table_cell("cell-a", "paragraph-a");
    wide.attrs.insert("colspan".to_string(), json!(2));
    wide.attrs.insert("colwidth".to_string(), json!([120]));
    let mut out_of_bounds = table_cell("cell-c", "paragraph-c");
    out_of_bounds.attrs.insert("rowspan".to_string(), json!(2));
    let table = ComponentNode::container(
        COMPONENT_TABLE_V2,
        Some(NodeId::new("table")),
        vec![
            ComponentNode::container(
                COMPONENT_TABLE_ROW_V2,
                Some(NodeId::new("row-a")),
                vec![wide],
            ),
            ComponentNode::container(
                COMPONENT_TABLE_ROW_V2,
                Some(NodeId::new("row-b")),
                vec![out_of_bounds],
            ),
        ],
    );
    let error = validate_component_document(
        &ComponentDocumentV2::new(vec![table]),
        &catalog,
        &ValidationLimits::default(),
        UnknownComponentPolicy::Reject,
    )
    .unwrap_err();
    assert!(
        error
            .issues
            .iter()
            .any(|issue| issue.code == "invalid_colwidth")
    );
    assert!(
        error
            .issues
            .iter()
            .any(|issue| issue.code == "table_span_bounds")
    );
    assert!(
        error
            .issues
            .iter()
            .any(|issue| issue.code == "table_map_hole")
    );
}

#[test]
fn malformed_custom_spec_can_declare_bounded_typed_attributes() {
    let spec = ComponentSpec::new(
        "rating",
        "Rating",
        ComponentKind::Atom,
        Some("inline"),
        ContentRule::Empty,
        "span",
    )
    .attribute(
        "value",
        AttributeSpec::required(AttributeType::Integer).bounded(1, 5),
    )
    .markdown_capability(FormatCapability::Opaque);
    let mut catalog = ComponentCatalog::default();
    catalog.register(spec).unwrap();

    assert_eq!(catalog.spec_by_id("rating").unwrap().component_version, 1);
}
