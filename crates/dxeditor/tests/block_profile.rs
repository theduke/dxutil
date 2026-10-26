use dxeditor::profiles::block_document::BLOCK_DOCUMENT_SCHEMA;
use dxeditor::{
    ComponentDocumentV2, ComponentNode, EditorCatalog, UnknownComponentPolicy, ValidationLimits,
    validate_component_document,
};

fn fixture() -> ComponentDocumentV2 {
    let mut doc = ComponentDocumentV2::new(vec![ComponentNode::container(
        "block_children",
        None,
        vec![ComponentNode::container(
            "block",
            Some(dxeditor::NodeId(
                "019a0ea0-0000-7000-8000-000000000001".into(),
            )),
            vec![ComponentNode::container(
                "paragraph",
                None,
                vec![ComponentNode::text("Hello")],
            )],
        )],
    )]);
    doc.schema = BLOCK_DOCUMENT_SCHEMA.into();
    doc
}
#[test]
fn validates_profile_and_rejects_non_uuid_identity() {
    let catalog = EditorCatalog::block_document();
    let mut doc = fixture();
    validate_component_document(
        &doc,
        catalog.component_specs(),
        &ValidationLimits::default(),
        UnknownComponentPolicy::Reject,
    )
    .unwrap();
    doc.root.content[0].content[0].id = Some(dxeditor::NodeId("bad".into()));
    assert!(
        validate_component_document(
            &doc,
            catalog.component_specs(),
            &ValidationLimits::default(),
            UnknownComponentPolicy::Reject
        )
        .is_err()
    );
}
#[test]
fn block_catalog_exposes_supported_content_and_identity_rules() {
    let catalog = EditorCatalog::block_document();
    assert!(catalog.component_specs().spec_by_id("graph").is_some());
    assert!(catalog.component_specs().spec_by_id("table").is_some());
    assert_eq!(catalog.schema_fingerprint().len(), 64);
}
