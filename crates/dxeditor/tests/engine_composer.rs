use dxeditor::engine::{
    Command, EditorEngine, EditorSelection, EngineOptions, EngineOutput, InlineContainerRef,
    Profile, TextPoint, TextRange, model,
};
use dxeditor::{EditorCatalog, profiles::composer::*};
use serde_json::json;

fn fixture() -> ComposerValue {
    ComposerValue {
        segments: vec![
            ComposerSegment::Text("Hello 🌍 ".into()),
            ComposerSegment::Chip(ComposerChip {
                kind: "entity".into(),
                label: "A person".into(),
                payload: json!({"id":"entity-one","extra":[1,2]}),
            }),
            ComposerSegment::Text("\nsecond\n".into()),
        ],
    }
}
fn engine(value: &ComposerValue) -> EditorEngine {
    EditorEngine::new(
        composer_to_blocks(value),
        &EditorCatalog::composer(),
        Profile::Composer,
        EngineOptions::default(),
    )
    .unwrap()
}
#[test]
fn composer_round_trips_chips_newlines_and_unicode() {
    let value = fixture();
    let doc = composer_to_blocks(&value);
    assert_eq!(blocks_to_composer(&doc), value);
    assert_eq!(composer_plain_text(&value), "Hello 🌍 A person\nsecond\n");
    assert_eq!(model::children(&doc, None).unwrap().len(), 3);
    assert_eq!(
        blocks_to_composer(&composer_to_blocks(&ComposerValue::default())),
        ComposerValue::default()
    );
}
#[test]
fn composer_text_segments_canonicalize_without_extra_newlines() {
    let value = ComposerValue {
        segments: vec![
            ComposerSegment::Text("a".into()),
            ComposerSegment::Text("b\r\nc".into()),
            ComposerSegment::Text("".into()),
        ],
    };
    assert_eq!(
        blocks_to_composer(&composer_to_blocks(&value)),
        ComposerValue {
            segments: vec![ComposerSegment::Text("ab\nc".into())]
        }
    );
}
#[test]
fn controlled_update_keeps_ids_maps_selection_and_emits_no_change() {
    let value = fixture();
    let mut engine = engine(&value);
    let uid = model::children(engine.document(), None).unwrap()[0]
        .id
        .as_ref()
        .unwrap()
        .0
        .clone();
    engine.run(Command::SetSelection(EditorSelection::Text(TextRange {
        anchor: TextPoint::new(InlineContainerRef::Block(uid.clone()), 2),
        head: TextPoint::new(InlineContainerRef::Block(uid.clone()), 7),
    })));
    let mut changed = value.clone();
    changed.segments[0] = ComposerSegment::Text("Hey Hello 🌍 ".into());
    let output = apply_composer_value(&mut engine, &changed);
    assert!(
        !output
            .iter()
            .any(|output| matches!(output, EngineOutput::Changed { .. }))
    );
    assert_eq!(blocks_to_composer(engine.document()), changed);
    assert_eq!(
        model::children(engine.document(), None).unwrap()[0]
            .id
            .as_ref()
            .unwrap()
            .0,
        uid
    );
    let range = engine.selection().text_range().unwrap();
    assert_eq!(range.anchor.offset, 6);
    assert_eq!(range.head.offset, 11);
    assert!(apply_composer_value(&mut engine, &changed).is_empty());
}
#[test]
fn controlled_update_does_not_split_or_count_chip_label_as_characters() {
    let value = fixture();
    let mut engine = engine(&value);
    let uid = model::children(engine.document(), None).unwrap()[0]
        .id
        .as_ref()
        .unwrap()
        .0
        .clone();
    engine.run(Command::SetSelection(EditorSelection::caret(
        InlineContainerRef::Block(uid),
        9,
    )));
    let mut changed = value.clone();
    changed
        .segments
        .insert(0, ComposerSegment::Text("prefix ".into()));
    apply_composer_value(&mut engine, &changed);
    assert_eq!(engine.selection().text_range().unwrap().head.offset, 16);
    assert_eq!(
        blocks_to_composer(engine.document()),
        ComposerValue {
            segments: vec![
                ComposerSegment::Text("prefix Hello 🌍 ".into()),
                value.segments[1].clone(),
                value.segments[2].clone()
            ]
        }
    );
}

#[test]
fn select_all_includes_every_paragraph_and_chips_on_every_press() {
    let mut e = engine(&fixture());
    let order = model::block_order(e.document());
    let first = order.first().unwrap().clone();
    let last = order.last().unwrap().clone();
    let expected = EditorSelection::Text(TextRange {
        anchor: TextPoint::new(InlineContainerRef::Block(first), 0),
        head: TextPoint::new(InlineContainerRef::Block(last), 0),
    });
    for _ in 0..3 {
        e.run(Command::SelectAll);
        assert_eq!(e.selection(), &expected);
    }
    e.run(Command::InsertText("replacement".into()));
    assert_eq!(
        composer_plain_text(&blocks_to_composer(e.document())),
        "replacement"
    );
}
