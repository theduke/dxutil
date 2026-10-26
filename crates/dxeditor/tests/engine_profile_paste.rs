use dxeditor::engine::{
    Command, EditorEngine, EditorSelection, EngineInput, EngineOptions, InlineContainerRef,
    Profile, TextPoint, TextRange,
    clipboard::{ClipboardSlice, PasteContent},
    html::HtmlNode,
    model,
};
use dxeditor::{
    ComponentMark, ComponentNode, EditorCatalog, UnknownComponentPolicy, ValidationLimits,
    validate_component_document,
};
use serde_json::json;
fn new_engine(profile: Profile, text: &str) -> (EditorEngine, EditorCatalog) {
    let catalog = match profile {
        Profile::Composer => EditorCatalog::composer(),
        Profile::MarkdownNotes => EditorCatalog::markdown_notes(),
        _ => EditorCatalog::block_document(),
    };
    let block = model::block_node(ComponentNode::container(
        "paragraph",
        None,
        if text.is_empty() {
            vec![]
        } else {
            vec![ComponentNode::text(text)]
        },
    ));
    let engine = EditorEngine::new(
        model::document(vec![block]),
        &catalog,
        profile,
        EngineOptions {
            origin: Some("https://app.example".into()),
            ..Default::default()
        },
    )
    .unwrap();
    (engine, catalog)
}
fn range(engine: &mut EditorEngine) {
    let uid = model::block_order(engine.document())[0].clone();
    engine.run(Command::SetSelection(EditorSelection::Text(TextRange {
        anchor: TextPoint::new(InlineContainerRef::Block(uid.clone()), 0),
        head: TextPoint::new(InlineContainerRef::Block(uid), 5),
    })));
}
fn assert_valid(engine: &EditorEngine, catalog: &EditorCatalog) {
    validate_component_document(
        engine.document(),
        catalog.component_specs(),
        &ValidationLimits::default(),
        UnknownComponentPolicy::Reject,
    )
    .unwrap();
}
fn text_node(engine: &EditorEngine) -> &ComponentNode {
    &model::children(engine.document(), None).unwrap()[0].content[0].content[0]
}
fn paste_text(engine: &mut EditorEngine, text: &str) {
    engine.handle(EngineInput::Paste(PasteContent {
        text: Some(text.into()),
        ..Default::default()
    }));
}
#[test]
fn explicit_urls_link_selected_text_and_entities_use_semantic_marks() {
    for (url, kind, value) in [
        ("semantic:entity:person", "entity_link", "person"),
        ("/entities/person", "entity_link", "person"),
        ("https://app.example/documents/page", "entity_link", "page"),
        ("tel:+43123", "link", "tel:+43123"),
        ("mailto:a@example.com", "link", "mailto:a@example.com"),
        (
            "https://other.example/entities/person",
            "link",
            "https://other.example/entities/person",
        ),
    ] {
        let (mut engine, catalog) = new_engine(Profile::BlockDocument, "label");
        range(&mut engine);
        paste_text(&mut engine, url);
        assert_eq!(text_node(&engine).text.as_deref(), Some("label"), "{url}");
        let mark = &text_node(&engine).marks[0];
        assert_eq!(mark.kind.0, kind, "{url}");
        assert_eq!(
            mark.attrs[if kind == "entity_link" {
                "entity_id"
            } else {
                "href"
            }],
            value
        );
        assert_valid(&engine, &catalog);
    }
}
#[test]
fn collapsed_urls_insert_linked_text_while_plain_words_and_unsafe_urls_remain_text() {
    for url in [
        "tel:+43123",
        "semantic:entity:person",
        "#section",
        "../relative",
    ] {
        let (mut engine, catalog) = new_engine(Profile::BlockDocument, "");
        paste_text(&mut engine, url);
        assert!(!text_node(&engine).marks.is_empty());
        assert_valid(&engine, &catalog);
    }
    for text in ["ordinary", "javascript:alert(1)", "data:text/html,x"] {
        let (mut engine, catalog) = new_engine(Profile::BlockDocument, "label");
        range(&mut engine);
        paste_text(&mut engine, text);
        assert_eq!(text_node(&engine).text.as_deref(), Some(text));
        assert!(text_node(&engine).marks.is_empty());
        assert_valid(&engine, &catalog);
    }
}
#[test]
fn composer_html_and_urls_remain_unmarked_paragraphs() {
    for text in ["https://example.com", "semantic:entity:person", "tel:+123"] {
        let (mut engine, catalog) = new_engine(Profile::Composer, "");
        paste_text(&mut engine, text);
        assert!(text_node(&engine).marks.is_empty());
        assert_valid(&engine, &catalog);
    }
    let (mut engine, catalog) = new_engine(Profile::Composer, "");
    let html = HtmlNode::element(
        "body",
        vec![
            HtmlNode::element(
                "h2",
                vec![HtmlNode::element("strong", vec![HtmlNode::text("Heading")])],
            ),
            HtmlNode::element("p", vec![HtmlNode::text("body")]),
        ],
    );
    engine.handle(EngineInput::Paste(PasteContent {
        html: Some(html),
        ..Default::default()
    }));
    let blocks = model::children(engine.document(), None).unwrap();
    assert_eq!(blocks.len(), 2);
    assert!(blocks.iter().all(|block| {
        block.content[0].kind.0 == "paragraph"
            && block.content[0]
                .content
                .iter()
                .all(|node| node.marks.is_empty())
    }));
    assert_valid(&engine, &catalog);
}
#[test]
fn composer_structured_chip_paste_is_preserved_but_rich_marks_are_removed() {
    let (mut engine, catalog) = new_engine(Profile::Composer, "");
    let chip =
        dxeditor::profiles::composer::chip_node(&dxeditor::profiles::composer::ComposerChip {
            kind: "entity".into(),
            label: "Ada".into(),
            payload: json!({"id":"person"}),
        });
    let mut marked = ComponentNode::text("text");
    marked.marks.push(ComponentMark::new("underline"));
    let block = model::block_node(ComponentNode::container(
        "heading",
        None,
        vec![chip, marked],
    ));
    engine.handle(EngineInput::Paste(PasteContent {
        internal: Some(ClipboardSlice {
            content: vec![block],
            open_start: 1,
            open_end: 1,
        }),
        ..Default::default()
    }));
    assert_valid(&engine, &catalog);
    let paragraph = &model::children(engine.document(), None).unwrap()[0].content[0];
    assert_eq!(paragraph.kind.0, "paragraph");
    // Custom composer envelopes contain paragraph blocks. An unsupported rich
    // block is deliberately reduced to its readable text.
    assert!(paragraph.content.iter().all(|node| node.marks.is_empty()));
    let (mut engine, catalog) = new_engine(Profile::Composer, "");
    let value = dxeditor::profiles::composer::ComposerValue {
        segments: vec![dxeditor::profiles::composer::ComposerSegment::Chip(
            dxeditor::profiles::composer::ComposerChip {
                kind: "entity".into(),
                label: "Ada".into(),
                payload: json!("person"),
            },
        )],
    };
    let doc = dxeditor::profiles::composer::composer_to_blocks(&value);
    engine.handle(EngineInput::Paste(PasteContent {
        internal: Some(ClipboardSlice {
            content: model::children(&doc, None).unwrap().clone(),
            open_start: 1,
            open_end: 1,
        }),
        ..Default::default()
    }));
    assert_eq!(
        dxeditor::profiles::composer::blocks_to_composer(engine.document()),
        value
    );
    assert_valid(&engine, &catalog);
}
#[test]
fn notes_html_strips_unrepresentable_marks_and_table_attributes() {
    let (mut engine, catalog) = new_engine(Profile::MarkdownNotes, "");
    let html = HtmlNode::element(
        "body",
        vec![
            HtmlNode::element(
                "p",
                vec![
                    HtmlNode::element(
                        "u",
                        vec![HtmlNode::element("strong", vec![HtmlNode::text("Bold")])],
                    ),
                    HtmlNode::element("span", vec![HtmlNode::text("colored")])
                        .with_attr("data-color", "red"),
                ],
            ),
            HtmlNode::element(
                "table",
                vec![HtmlNode::element(
                    "tr",
                    vec![
                        HtmlNode::element(
                            "th",
                            vec![HtmlNode::element("mark", vec![HtmlNode::text("Header")])],
                        )
                        .with_attr("data-background-color", "blue"),
                    ],
                )],
            ),
        ],
    );
    engine.handle(EngineInput::Paste(PasteContent {
        html: Some(html),
        ..Default::default()
    }));
    assert_valid(&engine, &catalog);
    let serialized = serde_json::to_string(engine.document()).unwrap();
    for unsupported in [
        "underline",
        "highlight",
        "text_color",
        "background_color",
        "header_column",
    ] {
        assert!(
            !serialized.contains(unsupported),
            "{unsupported}: {serialized}"
        );
    }
    assert!(serialized.contains("bold"));
}
