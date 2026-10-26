#![cfg(feature = "markdown")]
use dxeditor::engine::markdown_blocks::{to_blocks, to_standard};
use dxeditor::{
    ComponentDocumentV2, DecodeOptions, DocumentFormat, EditorCatalog, EditorPayload,
    EncodeOptions, MarkdownDocumentFormat, UnknownComponentPolicy, ValidationLimits,
    validate_component_document,
};

fn decode(source: &str) -> ComponentDocumentV2 {
    MarkdownDocumentFormat::new()
        .decode(
            &EditorPayload::new("markdown", serde_json::json!(source)),
            EditorCatalog::default().component_specs(),
            DecodeOptions::default(),
        )
        .unwrap()
        .document
}

fn encode(document: &ComponentDocumentV2) -> String {
    MarkdownDocumentFormat::new()
        .encode(
            document,
            EditorCatalog::default().component_specs(),
            EncodeOptions::default(),
        )
        .unwrap()
        .payload
        .value
        .as_str()
        .unwrap()
        .into()
}

const CORPUS: &[&str] = &[
    "",
    "hello **bold** and *italic* and ~~strike~~ and `code`",
    "first  \nsecond",
    "# h1\n\n###### h6\n\n---",
    "- parent\n  - child\n    3. nested\n    4. next\n- sibling",
    "- [x] done\n  - [ ] subtask\n- [ ] pending",
    "3. third\n4. fourth\n\n   another paragraph",
    "> quote\n>\n> - list\n>   - child\n>\n> > nested quote",
    "> # heading first\n>\n> quote paragraph",
    "```rust edition=2024\nfn main() {}\n```",
    "```\nplain code\n```",
    "| left | center | right |\n| :--- | :---: | ---: |\n| **a** | b | c |",
    "![alt](https://example.com/img.png \"title\")",
    "| image |\n| --- |\n| ![cell](https://example.com/cell.png \"title\") |",
    "before\n\n<section data-x=\"1\">raw</section>\n\nafter",
    "text <kbd>opaque</kbd> and [@Ada](semantic:entity:user-1)",
    "before[^a]\n\n[^a]: opaque footnote",
];

#[test]
fn notes_transform_restores_standard_tree_and_canonical_markdown() {
    for source in CORPUS {
        let standard = decode(source);
        let editing = to_blocks(&standard);
        validate_component_document(
            &editing,
            EditorCatalog::markdown_notes().component_specs(),
            &ValidationLimits::default(),
            UnknownComponentPolicy::Reject,
        )
        .unwrap_or_else(|error| panic!("{source:?}: {:?}", error.issues));
        let restored = to_standard(&editing);
        assert_eq!(restored, standard, "{source:?}");
        assert_eq!(encode(&restored), encode(&standard), "{source:?}");
    }
}

#[test]
fn inline_images_are_split_and_canonicalized_without_losing_neighbor_text() {
    let source = "before ![alt](https://example.com/a.png) after";
    let editing = to_blocks(&decode(source));
    let blocks = &editing.root.content[0].content;
    assert_eq!(blocks.len(), 3);
    assert_eq!(blocks[0].content[0].text_content(), "before ");
    assert_eq!(blocks[1].content[0].kind.0, "image");
    assert_eq!(blocks[2].content[0].text_content(), " after");
    let canonical = encode(&to_standard(&editing));
    assert!(canonical.contains("before "));
    assert!(canonical.contains("![alt](https://example.com/a.png)"));
    assert!(canonical.contains(" after"));
}

#[test]
fn source_shells_preserve_identity_without_overwriting_neighbor_edits() {
    let mut document = decode("- first\n- second\n\n<section>opaque</section>");
    document
        .metadata
        .insert("app".into(), serde_json::json!({"custom": true}));
    document.metadata.insert(
        "dxeditor.markdown-origin".into(),
        serde_json::json!("existing"),
    );
    let mut editing = to_blocks(&document);
    editing.root.content[0].content[0].content[0].content[0].text = Some("changed".into());
    let restored = to_standard(&editing);
    assert_eq!(restored.metadata, document.metadata);
    assert!(encode(&restored).contains("- changed"));
    assert!(encode(&restored).contains("<section>opaque</section>"));
}

#[test]
fn catalogs_admit_only_profile_content() {
    let notes = EditorCatalog::markdown_notes();
    for kind in [
        "opaque_markdown",
        "opaque_markdown_inline",
        "table",
        "image",
        "entity_link",
    ] {
        assert!(notes.component_specs().spec_by_id(kind).is_some(), "{kind}");
    }
    for kind in [
        "toggle",
        "callout",
        "graph",
        "underline",
        "highlight",
        "text_color",
    ] {
        assert!(notes.component_specs().spec_by_id(kind).is_none(), "{kind}");
    }
    let composer = EditorCatalog::composer();
    assert!(composer.component_specs().spec_by_id("chip").is_some());
    assert!(composer.component_specs().spec_by_id("bold").is_none());
    assert!(composer.component_specs().spec_by_id("heading").is_none());
}

#[test]
fn engine_normalization_and_new_inline_atoms_remain_persistable() {
    use dxeditor::engine::{Command, EditorEngine, EngineOptions, Profile};
    let mut engine = EditorEngine::new(
        to_blocks(&decode("<section>opaque</section>")),
        &EditorCatalog::markdown_notes(),
        Profile::MarkdownNotes,
        EngineOptions::default(),
    )
    .unwrap();
    engine.run(Command::InsertText("new".into()));
    engine.run(Command::HardBreak);
    let standard = to_standard(engine.document());
    assert!(encode(&standard).contains("new"));
}

#[test]
fn inline_images_inside_quotes_and_lists_remain_valid_notes_blocks() {
    for source in [
        "> before ![alt](https://example.com/a.png) after",
        "- before ![alt](https://example.com/a.png) after",
    ] {
        let editing = to_blocks(&decode(source));
        validate_component_document(
            &editing,
            EditorCatalog::markdown_notes().component_specs(),
            &ValidationLimits::default(),
            UnknownComponentPolicy::Reject,
        )
        .unwrap();
        let restored = encode(&to_standard(&editing));
        assert!(restored.contains("before"));
        assert!(restored.contains("![alt](https://example.com/a.png)"));
        assert!(restored.contains("after"));
    }
}

#[test]
fn a_preexisting_null_origin_metadata_value_is_restored() {
    let mut standard = decode("text");
    standard
        .metadata
        .insert("dxeditor.markdown-origin".into(), serde_json::Value::Null);
    assert_eq!(to_standard(&to_blocks(&standard)), standard);
}
