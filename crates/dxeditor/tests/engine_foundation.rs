use dxeditor::engine::{
    Command, EditorEngine, EditorSelection, EngineInput, EngineOptions, EngineOutput,
    InlineContainerRef, KeyChord, Op, Profile, Transaction, TxKind, identity, input_rules,
    input_types::{self, InputDisposition},
    model, suggestion,
};
use dxeditor::{ComponentNode, EditorCatalog, HistoryPolicy, NodeId};
fn engine(text: &str) -> EditorEngine {
    let mut block = model::paragraph();
    block.content[0].content = if text.is_empty() {
        vec![]
    } else {
        vec![ComponentNode::text(text)]
    };
    EditorEngine::new(
        model::document(vec![block]),
        &EditorCatalog::block_document(),
        Profile::BlockDocument,
        EngineOptions::default(),
    )
    .unwrap()
}
fn uid(e: &EditorEngine) -> String {
    model::block_order(e.document())[0].clone()
}
fn type_text(e: &mut EditorEngine, text: &str) {
    e.handle(EngineInput::InsertText { text: text.into() });
}
fn plain(e: &EditorEngine) -> String {
    model::inline_text(model::inline(e.document(), &InlineContainerRef::Block(uid(e))).unwrap())
}
#[test]
fn dismissing_overlays_preserves_focus_selection_and_document() {
    for read_only in [false, true] {
        let mut e = engine("unchanged");
        e.set_read_only(read_only);
        e.set_focused(false);
        e.run(Command::OpenMenu("link".into()));
        e.set_entity_preview("entity-1".into(), None, true);
        let document = e.document().clone();
        let selection = e.selection().clone();

        let output = e.dismiss_overlays();

        assert!(
            !output
                .iter()
                .any(|o| matches!(o, EngineOutput::Focus { .. }))
        );
        assert!(e.ui_state().open_menu.is_none());
        assert!(e.ui_state().entity_preview.is_none());
        assert!(!e.ui_state().focused);
        assert_eq!(e.document(), &document);
        assert_eq!(e.selection(), &selection);
        assert!(!e.can_undo());
        assert!(e.dismiss_overlays().is_empty());
    }
}

#[test]
fn explicitly_closing_a_menu_restores_editor_focus() {
    let mut e = engine("unchanged");
    e.run(Command::OpenMenu("link".into()));
    let output = e.run(Command::CloseMenu);
    assert!(e.ui_state().open_menu.is_none());
    assert!(output.iter().any(|o| matches!(
        o,
        EngineOutput::Focus {
            target: dxeditor::engine::FocusTarget::Editor
        }
    )));
}

#[test]
fn operations_invert_exactly_including_nested_group_creation() {
    let a = model::paragraph();
    let b = model::paragraph();
    let aid = a.id.as_ref().unwrap().0.clone();
    let original = model::document(vec![a, b]);
    let selection = EditorSelection::caret(InlineContainerRef::Block(aid.clone()), 0);
    let tx = Transaction::new(
        vec![Op::MoveBlocks {
            from: (None, 1, 1),
            to: (Some(aid), 0),
        }],
        selection.clone(),
        selection,
        TxKind::Structural,
    );
    let mut doc = original.clone();
    tx.apply(&mut doc).unwrap();
    tx.invert().apply(&mut doc).unwrap();
    assert_eq!(doc, original);
}
#[test]
fn failed_transaction_rolls_back_applied_ops() {
    let mut e = engine("before");
    let original = e.document().clone();
    let uid = uid(&e);
    let before = model::content(&original, &uid).unwrap().clone();
    let mut after = before.clone();
    after.content = vec![ComponentNode::text("after")];
    let tx = Transaction::new(
        vec![
            Op::SetContent {
                uid: uid.clone(),
                before,
                after,
            },
            Op::RemoveBlocks {
                parent: None,
                index: 99,
                blocks: vec![model::paragraph()],
            },
        ],
        e.selection().clone(),
        e.selection().clone(),
        TxKind::Command,
    );
    assert!(e.transact(tx).is_err());
    assert_eq!(e.document(), &original);
}
#[test]
fn normalization_identity_and_copy_owned_refs() {
    let mut a = model::paragraph();
    a.id = Some(NodeId::new("same"));
    let b = a.clone();
    let mut doc = model::document(vec![a, b, model::block_node(ComponentNode::new("divider"))]);
    for op in identity::repairs(&doc, &Default::default()) {
        op.apply(&mut doc).unwrap();
    }
    let e = EditorEngine::new(
        doc,
        &EditorCatalog::block_document(),
        Profile::BlockDocument,
        EngineOptions::default(),
    )
    .unwrap();
    let order = model::block_order(e.document());
    let unique = order.iter().collect::<std::collections::BTreeSet<_>>();
    assert_eq!(order.len(), unique.len());
    assert_eq!(
        model::content(e.document(), order.last().unwrap())
            .unwrap()
            .kind
            .0,
        "paragraph"
    );
    let id = identity::new_uid();
    assert_eq!(uuid::Uuid::parse_str(&id).unwrap().get_version_num(), 7);
    let mut value =
        serde_json::json!({"nodes":[{"uid":id}],"edges":[{"source":id}],"entity_id":"person"});
    let ids = identity::owned_uids(&value);
    identity::rekey_owned(&mut value, &ids);
    assert_eq!(value["nodes"][0]["uid"], value["edges"][0]["source"]);
    assert_ne!(value["nodes"][0]["uid"], id);
    assert_eq!(value["entity_id"], "person");
}
#[test]
fn history_groups_adjacent_typing_and_breaks_at_window() {
    let mut e = engine("");
    e.set_time_ms(100);
    type_text(&mut e, "a");
    e.set_time_ms(200);
    type_text(&mut e, "b");
    e.run(Command::Undo);
    assert_eq!(plain(&e), "");
    e.run(Command::Redo);
    assert_eq!(plain(&e), "ab");
    e.set_time_ms(900);
    type_text(&mut e, "c");
    e.run(Command::Undo);
    assert_eq!(plain(&e), "ab");
}
#[test]
fn replacing_document_never_emits_change_and_reset_clears_history() {
    let mut e = engine("");
    type_text(&mut e, "a");
    let doc = e.document().clone();
    let output = e.replace_document(doc.clone(), HistoryPolicy::Preserve);
    assert!(
        !output
            .iter()
            .any(|o| matches!(o, EngineOutput::Changed { .. }))
    );
    assert!(e.can_undo());
    e.replace_document(doc, HistoryPolicy::Reset);
    assert!(!e.can_undo());
}
#[test]
fn composition_readback_keeps_unchanged_marks_and_emits_one_change() {
    let mut e = engine("ab");
    let id = uid(&e);
    e.handle(EngineInput::CompositionStart);
    let output = e.handle(EngineInput::CompositionEnd {
        container: InlineContainerRef::Block(id),
        dom_text: "a日本b".into(),
        dom_caret: Some(3),
    });
    assert_eq!(plain(&e), "a日本b");
    assert_eq!(
        output
            .iter()
            .filter(|o| matches!(o, EngineOutput::Changed { .. }))
            .count(),
        1
    );
    e.run(Command::Undo);
    assert_eq!(plain(&e), "ab");
}
#[test]
fn input_rule_corpus() {
    for (text, kind) in [
        ("# ", "heading"),
        ("###### ", "heading"),
        ("- ", "bulleted_list_item"),
        ("12. ", "numbered_list_item"),
        ("[] ", "todo"),
        ("[ ] ", "todo"),
        ("[x] ", "todo"),
        ("[X] ", "todo"),
        ("> ", "quote"),
        ("```rust ", "code_block"),
        ("---", "divider"),
    ] {
        assert_eq!(input_rules::block_rule(text, false).unwrap().0, kind);
    }
    assert!(input_rules::block_rule("####### ", false).is_none());
    assert_eq!(
        input_rules::block_rule("```c++", true).unwrap().0,
        "code_block"
    );
}
#[test]
fn rules_own_history_step_and_backspace_undo() {
    let mut e = engine("");
    type_text(&mut e, "#");
    type_text(&mut e, " ");
    assert_eq!(
        model::content(e.document(), &uid(&e)).unwrap().kind.0,
        "heading"
    );
    e.handle(EngineInput::Delete {
        direction: dxeditor::engine::text::Direction::Backward,
        unit: dxeditor::engine::text::DeleteUnit::Grapheme,
    });
    assert_eq!(
        model::content(e.document(), &uid(&e)).unwrap().kind.0,
        "paragraph"
    );
    assert_eq!(plain(&e), "# ");
}
#[test]
fn inline_rules_preserve_tail_and_marks() {
    for (input, expected) in [
        ("**bold**", "bold"),
        ("__bold__", "bold"),
        ("*italic*", "italic"),
        ("_italic_", "italic"),
        ("~~strike~~", "strike"),
        ("`code`", "code"),
    ] {
        let mut e = engine("");
        type_text(&mut e, input);
        let content = model::content(e.document(), &uid(&e)).unwrap();
        assert_eq!(content.content.len(), 1, "{input}");
        assert!(
            content.content[0]
                .marks
                .iter()
                .any(|m| m.kind.0 == expected),
            "{input}"
        );
    }
}
#[test]
fn suggestions_match_corpus_and_dismiss_until_query_changes() {
    let c = InlineContainerRef::Block("a".into());
    let aliases = vec!["Mind Map".into(), "Heading 1".into()];
    for text in ["/mind", "/mind ", "/mind m", "Text /mind map"] {
        assert!(
            suggestion::detect(text, c.clone(), &["/".into()], false, &aliases).is_some(),
            "{text}"
        );
    }
    for text in [
        "/graph ",
        "/mind  map",
        "Text/mind map",
        "/mind\nmap",
        "/mind/map",
    ] {
        assert!(
            suggestion::detect(text, c.clone(), &["/".into()], false, &aliases).is_none(),
            "{text}"
        );
    }
    let matched = suggestion::detect("@other thread", c, &["@".into()], true, &[]).unwrap();
    assert_eq!(matched.query, "other thread");
    let mut state = suggestion::SuggestionState::default();
    state.update(Some(matched.clone()));
    state.dismiss();
    assert!(!state.update(Some(matched)));
    assert!(state.active.is_none());
    let mut e = engine("");
    type_text(&mut e, "/");
    assert!(e.suggestion().active.is_some());
    e.run(Command::DismissSuggestion);
    e.handle(EngineInput::SelectionChanged(e.selection().clone()));
    assert!(e.suggestion().active.is_none());
    type_text(&mut e, "h");
    assert!(e.suggestion().active.is_some());
}
#[test]
fn keymap_does_not_swallow_normal_arrow_or_composition() {
    let e = engine("plain");
    assert!(dxeditor::engine::keymap::resolve(&e, &KeyChord::new("ArrowDown")).is_none());
    let mut key = KeyChord::new("b");
    key.ctrl = true;
    assert!(matches!(
        dxeditor::engine::keymap::resolve(&e, &key),
        Some(dxeditor::engine::keymap::KeyAction::Command(
            Command::ToggleMark(_)
        ))
    ));
    key.composing = true;
    assert!(dxeditor::engine::keymap::resolve(&e, &key).is_none());
}
#[test]
fn input_types_cover_autocorrect_composition_and_deletion() {
    assert!(matches!(
        input_types::translate("insertCompositionText", Some("日"), None, false),
        InputDisposition::AllowComposition
    ));
    assert!(matches!(
        input_types::translate("insertParagraph", None, None, true),
        InputDisposition::Input(EngineInput::InsertLineBreak)
    ));
    assert!(matches!(
        input_types::translate("deleteWordForward", None, None, false),
        InputDisposition::Input(EngineInput::Delete {
            unit: dxeditor::engine::text::DeleteUnit::Word,
            ..
        })
    ));
    assert!(matches!(
        input_types::translate("unknown", None, None, false),
        InputDisposition::Ignore
    ));
}
#[test]
fn whole_internal_clipboard_preserves_block_boundaries_and_single_undo() {
    let mut e = engine("original");
    let original = e.document().clone();
    let id = uid(&e);
    e.run(Command::SetSelection(EditorSelection::caret(
        InlineContainerRef::Block(id.clone()),
        4,
    )));
    let mut pasted = model::paragraph();
    pasted.content[0].content = vec![ComponentNode::text("pasted")];
    let output = e.handle(EngineInput::Paste(
        dxeditor::engine::clipboard::PasteContent {
            internal: Some(dxeditor::engine::clipboard::ClipboardSlice {
                content: vec![pasted],
                open_start: 0,
                open_end: 0,
            }),
            ..Default::default()
        },
    ));
    assert_eq!(model::children(e.document(), None).unwrap().len(), 2);
    assert_eq!(
        model::content(e.document(), &id).unwrap().content[0]
            .text
            .as_deref(),
        Some("original")
    );
    assert_eq!(
        output
            .iter()
            .filter(|o| matches!(o, EngineOutput::Changed { .. }))
            .count(),
        1
    );
    e.run(Command::Undo);
    assert_eq!(e.document(), &original);
}
#[test]
fn html_table_paste_preserves_rich_inline_cells() {
    use dxeditor::engine::html::HtmlNode as H;
    let mut e = engine("");
    e.run(Command::Table(
        dxeditor::engine::commands::TableCommand::Insert {
            rows: 1,
            columns: 1,
            header_row: false,
        },
    ));
    let html = H::element(
        "table",
        vec![H::element(
            "tr",
            vec![
                H::element("td", vec![H::element("strong", vec![H::text("Bold")])]),
                H::element("td", vec![H::text("other")]),
            ],
        )],
    );
    let output = e.handle(EngineInput::Paste(
        dxeditor::engine::clipboard::PasteContent {
            html: Some(html),
            ..Default::default()
        },
    ));
    let table = model::block_order(e.document())
        .into_iter()
        .find_map(|id| model::content(e.document(), &id).filter(|n| n.kind.0 == "table"))
        .unwrap();
    assert_eq!(table.content[0].content.len(), 2);
    assert!(
        table.content[0].content[0].content[0]
            .marks
            .iter()
            .any(|m| m.kind.0 == "bold")
    );
    assert_eq!(
        output
            .iter()
            .filter(|o| matches!(o, EngineOutput::Changed { .. }))
            .count(),
        1
    );
}
#[test]
fn selected_text_html_paste_consumes_selection_once() {
    use dxeditor::engine::html::HtmlNode as H;
    let mut e = engine("before selected after");
    let id = uid(&e);
    e.run(Command::SetSelection(EditorSelection::Text(
        dxeditor::engine::TextRange {
            anchor: dxeditor::engine::TextPoint::new(InlineContainerRef::Block(id.clone()), 7),
            head: dxeditor::engine::TextPoint::new(InlineContainerRef::Block(id), 15),
        },
    )));
    let before = e.document().clone();
    let html = H::element("h1", vec![H::text("replacement")]);
    let output = e.handle(EngineInput::Paste(
        dxeditor::engine::clipboard::PasteContent {
            html: Some(html),
            ..Default::default()
        },
    ));
    assert!(!e.document().text_content().contains("selected"));
    assert!(e.document().text_content().contains("replacement"));
    assert_eq!(
        output
            .iter()
            .filter(|o| matches!(o, EngineOutput::Changed { .. }))
            .count(),
        1
    );
    e.run(Command::Undo);
    assert_eq!(e.document(), &before);
}
#[test]
fn host_picker_tracks_original_uid_when_other_blocks_inserted() {
    let mut block = model::paragraph();
    block.content[0].content = vec![ComponentNode::text("text")];
    let id = block.id.as_ref().unwrap().0.clone();
    let mut e = EditorEngine::new(
        model::document(vec![block]),
        &EditorCatalog::block_document(),
        Profile::BlockDocument,
        EngineOptions {
            host_entity_picker: true,
            ..Default::default()
        },
    )
    .unwrap();
    e.run(Command::SetSelection(EditorSelection::caret(
        InlineContainerRef::Block(id.clone()),
        4,
    )));
    e.run(Command::Request(
        dxeditor::EngineEvent::EntityPickerRequest {
            request_id: 55,
            mode: "mention".into(),
        },
    ));
    e.run(Command::InsertBlocks {
        at: dxeditor::InsertAt::AfterBlock { uid: id.clone() },
        nodes: vec![model::paragraph()],
    });
    e.handle(EngineInput::HostResponse(
        dxeditor::engine::HostResponse::EntityPicker {
            request_id: 55,
            entity: Some(dxeditor::EntityLinkCandidate {
                id: "ada".into(),
                label: "Ada".into(),
                detail: None,
            }),
        },
    ));
    assert_eq!(
        model::content(e.document(), &id)
            .unwrap()
            .content
            .last()
            .unwrap()
            .kind
            .0,
        "mention"
    );
    let output = e.handle(EngineInput::HostResponse(
        dxeditor::engine::HostResponse::EntityPicker {
            request_id: 55,
            entity: Some(dxeditor::EntityLinkCandidate {
                id: "again".into(),
                label: "Again".into(),
                detail: None,
            }),
        },
    ));
    assert!(output.is_empty());
}
#[test]
fn inline_entity_picker_preserves_selected_text_for_link_mode() {
    let mut e = engine("Ada Lovelace");
    let id = uid(&e);
    e.run(Command::SetSelection(EditorSelection::Text(
        dxeditor::engine::TextRange {
            anchor: dxeditor::engine::TextPoint::new(InlineContainerRef::Block(id.clone()), 0),
            head: dxeditor::engine::TextPoint::new(InlineContainerRef::Block(id.clone()), 12),
        },
    )));
    e.request_entity_picker("link".into());
    let generation = e.suggestion().generation;
    assert!(e.ui_state().suggestion.unwrap().manual);
    e.set_suggestion_items(
        generation,
        vec![dxeditor::SuggestionItem {
            id: "ada".into(),
            label: "Ada".into(),
            description: None,
            icon: None,
            data: serde_json::json!({"entity":{"id":"ada","label":"Ada"}}),
            priority: 0,
        }],
    );
    e.run(Command::AcceptSuggestion);
    assert_eq!(plain(&e), "Ada Lovelace");
    assert!(
        model::content(e.document(), &id).unwrap().content[0]
            .marks
            .iter()
            .any(|m| m.kind.0 == "entity_link")
    );
}
#[test]
fn semantic_task_conversion_retains_uid_and_promotes_children_one_undo() {
    let mut todo = model::block_node(ComponentNode::new("todo"));
    todo.content[0]
        .attrs
        .insert("checked".into(), serde_json::json!(false));
    todo.content[0].content = vec![ComponentNode::text("Do something")];
    let id = todo.id.as_ref().unwrap().0.clone();
    let child = model::paragraph();
    let child_id = child.id.as_ref().unwrap().0.clone();
    todo.content.push(ComponentNode::container(
        "block_children",
        None,
        vec![child],
    ));
    let mut e = EditorEngine::new(
        model::document(vec![todo]),
        &EditorCatalog::block_document(),
        Profile::BlockDocument,
        EngineOptions::default(),
    )
    .unwrap();
    let original = e.document().clone();
    e.run(Command::SelectNode(id.clone()));
    e.run(Command::Request(
        dxeditor::EngineEvent::EntityCreateRequest {
            request_id: 9,
            query: String::new(),
            trigger: "task".into(),
            uids: vec![id.clone()],
            checked: Some(false),
        },
    ));
    let out = e.handle(EngineInput::HostResponse(
        dxeditor::engine::HostResponse::EntityCreate {
            request_id: 9,
            entity: Some(dxeditor::EntityLinkCandidate {
                id: "task-id".into(),
                label: "Task".into(),
                detail: None,
            }),
        },
    ));
    assert_eq!(
        model::content(e.document(), &id).unwrap().kind.0,
        "entity_embed"
    );
    assert_eq!(model::location(e.document(), &child_id).unwrap().0, None);
    assert_eq!(
        out.iter()
            .filter(|o| matches!(o, EngineOutput::Changed { .. }))
            .count(),
        1
    );
    e.run(Command::Undo);
    assert_eq!(e.document(), &original);
}
#[test]
fn row_reorder_keeps_identity_and_undo() {
    let mut e = engine("");
    e.run(Command::Table(
        dxeditor::engine::commands::TableCommand::Insert {
            rows: 3,
            columns: 2,
            header_row: true,
        },
    ));
    let table_id = dxeditor::engine::commands::blocks::current_uid(&e).unwrap();
    let before = model::content(e.document(), &table_id).unwrap().clone();
    let ids = before
        .content
        .iter()
        .map(|r| r.id.clone())
        .collect::<Vec<_>>();
    e.run(Command::Table(
        dxeditor::engine::commands::TableCommand::MoveRow { from: 0, to: 2 },
    ));
    let table = model::content(e.document(), &table_id).unwrap();
    assert_eq!(table.content[2].id, ids[0]);
    assert_eq!(table.content[0].id, ids[1]);
    e.run(Command::Undo);
    assert_eq!(model::content(e.document(), &table_id).unwrap(), &before);
}
#[test]
fn input_rule_notifications_carry_the_document_for_their_revision() {
    let mut e = engine("");
    type_text(&mut e, "#");
    let output = e.handle(EngineInput::InsertText { text: " ".into() });
    let earlier = output
        .iter()
        .find_map(|o| {
            if let EngineOutput::ChangedSnapshot {
                revision, document, ..
            } = o
            {
                Some((*revision, document))
            } else {
                None
            }
        })
        .expect("pre-rule snapshot");
    assert_eq!(
        model::content(earlier.1, &uid(&e)).unwrap().kind.0,
        "paragraph"
    );
    assert_eq!(
        model::content(earlier.1, &uid(&e)).unwrap().content[0]
            .text
            .as_deref(),
        Some("# ")
    );
    let final_revision = output
        .iter()
        .find_map(|o| {
            if let EngineOutput::Changed { revision, .. } = o {
                Some(*revision)
            } else {
                None
            }
        })
        .unwrap();
    assert!(earlier.0 < final_revision);
    assert_eq!(final_revision, e.revision());
    assert_eq!(
        model::content(e.document(), &uid(&e)).unwrap().kind.0,
        "heading"
    );
}
#[test]
fn slash_selection_replacement_is_one_history_entry() {
    let mut e = engine("");
    type_text(&mut e, "/heading");
    let item = e
        .suggestion()
        .items
        .iter()
        .position(|item| item.id == "heading:1")
        .unwrap();
    e.run(Command::SuggestionFirst);
    e.run(Command::SuggestionNext(item as i32));
    let original = e.document().clone();
    let output = e.run(Command::AcceptSuggestion);
    assert_eq!(
        output
            .iter()
            .filter(|o| matches!(o, EngineOutput::Changed { .. }))
            .count(),
        1
    );
    assert_eq!(
        model::content(e.document(), &uid(&e)).unwrap().kind.0,
        "heading"
    );
    e.run(Command::Undo);
    assert_eq!(e.document(), &original);
}
#[test]
fn disabled_composer_hint_does_not_swallow_submit_or_tab() {
    let mut e = EditorEngine::new(
        model::document(vec![model::paragraph()]),
        &EditorCatalog::composer(),
        Profile::Composer,
        EngineOptions::default(),
    )
    .unwrap();
    e.set_triggers(vec!["/".into()]);
    type_text(&mut e, "/");
    let generation = e.suggestion().generation;
    e.set_suggestion_items(
        generation,
        vec![dxeditor::SuggestionItem {
            id: "disabled".into(),
            label: "Hint".into(),
            description: None,
            icon: None,
            data: serde_json::json!({"disabled":true}),
            priority: 0,
        }],
    );
    assert!(matches!(
        dxeditor::engine::keymap::resolve(&e, &KeyChord::new("Enter")),
        Some(dxeditor::engine::keymap::KeyAction::Submit { .. })
    ));
    assert!(dxeditor::engine::keymap::resolve(&e, &KeyChord::new("Tab")).is_none());
}
#[test]
fn preserve_replacement_folds_updated_content_and_keeps_undo_redo_valid() {
    let mut e = engine("before");
    let original = e.document().clone();
    let id = uid(&e);
    type_text(&mut e, "local");
    let mut replacement = e.document().clone();
    let content = model::content_mut(&mut replacement, &id).unwrap();
    content
        .attrs
        .insert("text_color".into(), serde_json::json!("blue"));
    content.content.push(ComponentNode::text(" external"));
    let output = e.replace_document(replacement.clone(), HistoryPolicy::Preserve);
    assert!(!output.iter().any(|o| matches!(
        o,
        EngineOutput::Changed { .. } | EngineOutput::ChangedSnapshot { .. }
    )));
    let output = e.run(Command::Undo);
    assert!(!output.iter().any(|o| matches!(
        o,
        EngineOutput::Request(dxeditor::EngineEvent::Error { .. })
    )));
    assert_eq!(e.document(), &original);
    e.run(Command::Redo);
    assert_eq!(e.document(), &replacement);
}
#[test]
fn preserve_structural_replacement_does_not_leave_stale_history_targets() {
    let mut e = engine("");
    let original = e.document().clone();
    type_text(&mut e, "local");
    let mut block = model::paragraph();
    block.content[0].content = vec![ComponentNode::text("remote")];
    let replacement = model::document(vec![block]);
    e.replace_document(replacement.clone(), HistoryPolicy::Preserve);
    e.run(Command::Undo);
    assert_eq!(e.document(), &original);
    e.run(Command::Redo);
    assert_eq!(e.document(), &replacement);
}
#[test]
fn bare_code_rule_has_no_invalid_null_language() {
    let mut e = engine("");
    type_text(&mut e, "```");
    let output = e.handle(EngineInput::InsertParagraph);
    assert!(
        output
            .iter()
            .any(|o| matches!(o, EngineOutput::Changed { .. }))
    );
    let content = model::content(e.document(), &uid(&e)).unwrap();
    assert_eq!(content.kind.0, "code_block");
    assert!(!content.attrs.contains_key("language"));
    dxeditor::validate_component_document(
        e.document(),
        EditorCatalog::block_document().component_specs(),
        &dxeditor::ValidationLimits::default(),
        dxeditor::UnknownComponentPolicy::Reject,
    )
    .unwrap();
}
#[test]
fn notes_image_capabilities_keep_url_alt_title_and_reject_unpersistable_width() {
    let catalog = EditorCatalog::markdown_notes();
    let mut e = EditorEngine::new(
        model::document(vec![model::paragraph()]),
        &catalog,
        Profile::MarkdownNotes,
        EngineOptions::default(),
    )
    .unwrap();
    let mut image = ComponentNode::new("image");
    image.attrs=serde_json::from_value(serde_json::json!({"media":{"type":"url","url":"https://example.com/image.png"},"alt":"Description","title":"Title"})).unwrap();
    let block = model::block_node(image);
    let id = block.id.as_ref().unwrap().0.clone();
    e.run(Command::InsertBlocks {
        at: dxeditor::InsertAt::Selection,
        nodes: vec![block],
    });
    assert_eq!(
        e.availability(&Command::Image {
            uid: id.clone(),
            attrs: serde_json::from_value(serde_json::json!({"width":50})).unwrap()
        }),
        dxeditor::engine::CommandAvailability::Hidden
    );
    e.run(Command::Image {
        uid: id.clone(),
        attrs: serde_json::from_value(serde_json::json!({"alt":"Changed","title":"Changed title"}))
            .unwrap(),
    });
    let standard = dxeditor::engine::markdown_blocks::to_standard(e.document());
    fn find(node: &ComponentNode) -> Option<&ComponentNode> {
        if node.kind.0 == "image" {
            Some(node)
        } else {
            node.content.iter().find_map(find)
        }
    }
    let projected = find(&standard.root).unwrap();
    assert_eq!(projected.attrs["src"], "https://example.com/image.png");
    assert_eq!(projected.attrs["alt"], "Changed");
    assert_eq!(projected.attrs["title"], "Changed title");
}
#[test]
fn raw_duplicate_insert_keeps_original_owned_identity_and_rekeys_copy_before_source() {
    let owned = identity::new_uid();
    let mut graph = ComponentNode::new("graph");
    graph.attrs = serde_json::from_value(
        serde_json::json!({"nodes":[{"uid":owned}],"edges":[{"source":owned}]}),
    )
    .unwrap();
    let original = model::block_node(graph);
    let original_id = original.id.as_ref().unwrap().0.clone();
    let mut e = EditorEngine::new(
        model::document(vec![original.clone()]),
        &EditorCatalog::block_document(),
        Profile::BlockDocument,
        EngineOptions::default(),
    )
    .unwrap();
    let before = e.document().clone();
    let selection = e.selection().clone();
    e.transact(Transaction::new(
        vec![Op::InsertBlocks {
            parent: None,
            index: 0,
            blocks: vec![original],
        }],
        selection,
        EditorSelection::Node(original_id.clone()),
        TxKind::Paste,
    ))
    .unwrap();
    let blocks = model::children(e.document(), None).unwrap();
    let copy_id = blocks[0].id.as_ref().unwrap().0.clone();
    assert_ne!(copy_id, original_id);
    assert_eq!(
        e.content(&original_id).unwrap().attrs["nodes"][0]["uid"],
        owned
    );
    let copy = e.content(&copy_id).unwrap();
    assert_ne!(copy.attrs["nodes"][0]["uid"], owned);
    assert_eq!(
        copy.attrs["nodes"][0]["uid"],
        copy.attrs["edges"][0]["source"]
    );
    assert_eq!(e.selection(), &EditorSelection::Node(copy_id));
    e.run(Command::Undo);
    assert_eq!(e.document(), &before);
}
#[test]
fn raw_table_copy_receives_fresh_valid_row_ids() {
    let mut e = engine("");
    e.run(Command::Table(
        dxeditor::engine::commands::TableCommand::Insert {
            rows: 2,
            columns: 2,
            header_row: true,
        },
    ));
    let id = dxeditor::engine::commands::blocks::current_uid(&e).unwrap();
    let copy = e.block(&id).unwrap().clone();
    let original_rows = e
        .content(&id)
        .unwrap()
        .content
        .iter()
        .map(|r| r.id.as_ref().unwrap().0.clone())
        .collect::<Vec<_>>();
    let selection = e.selection().clone();
    e.transact(Transaction::new(
        vec![Op::InsertBlocks {
            parent: None,
            index: 0,
            blocks: vec![copy],
        }],
        selection.clone(),
        selection,
        TxKind::Paste,
    ))
    .unwrap();
    let copied = &model::children(e.document(), None).unwrap()[0].content[0];
    for row in &copied.content {
        let id = &row.id.as_ref().unwrap().0;
        assert!(!original_rows.contains(id));
        assert_eq!(uuid::Uuid::parse_str(id).unwrap().get_version_num(), 7);
    }
    dxeditor::validate_component_document(
        e.document(),
        EditorCatalog::block_document().component_specs(),
        &dxeditor::ValidationLimits::default(),
        dxeditor::UnknownComponentPolicy::Reject,
    )
    .unwrap();
}
#[test]
fn empty_root_normalization_preserves_document_metadata() {
    let mut document = model::document(Vec::new());
    document.root.content.clear();
    document.metadata.insert(
        "source".into(),
        serde_json::json!({"owner":"keep","revision":8}),
    );
    let metadata = document.metadata.clone();
    let e = EditorEngine::new(
        document,
        &EditorCatalog::block_document(),
        Profile::BlockDocument,
        EngineOptions::default(),
    )
    .unwrap();
    assert_eq!(e.document().metadata, metadata);
    assert_eq!(model::children(e.document(), None).unwrap().len(), 1);
}
#[test]
fn long_typing_group_coalesces_content_ops_without_losing_undo_redo() {
    use dxeditor::engine::history::History;
    let mut document = model::document(vec![model::paragraph()]);
    let original = document.clone();
    let id = model::block_order(&document)[0].clone();
    let mut history = History::new(200, 500);
    for i in 0..100 {
        let before = model::content(&document, &id).unwrap().clone();
        let mut after = before.clone();
        after.content = vec![ComponentNode::text("a".repeat(i + 1))];
        let tx = Transaction::new(
            vec![Op::SetContent {
                uid: id.clone(),
                before,
                after,
            }],
            EditorSelection::caret(InlineContainerRef::Block(id.clone()), i),
            EditorSelection::caret(InlineContainerRef::Block(id.clone()), i + 1),
            TxKind::Typing,
        );
        tx.apply(&mut document).unwrap();
        history.record(tx, (i * 10) as u64);
    }
    let final_document = document.clone();
    let undo = history.undo_transaction().unwrap();
    assert_eq!(undo.ops.len(), 1);
    undo.apply(&mut document).unwrap();
    assert_eq!(document, original);
    undo.invert().apply(&mut document).unwrap();
    assert_eq!(document, final_document);
}
#[test]
fn invalid_replacement_during_composition_is_rejected_without_overwriting_live_content() {
    let mut e = engine("before");
    let id = uid(&e);
    let mut invalid = e.document().clone();
    invalid.version = 999;
    e.handle(EngineInput::CompositionStart);
    let output = e.replace_document(invalid, HistoryPolicy::Reset);
    assert!(output.iter().any(|o| matches!(
        o,
        EngineOutput::Request(dxeditor::EngineEvent::Error { .. })
    )));
    assert_eq!(plain(&e), "before");
    e.handle(EngineInput::CompositionEnd {
        container: InlineContainerRef::Block(id),
        dom_text: "before日本".into(),
        dom_caret: Some(8),
    });
    assert_eq!(plain(&e), "before日本");
}
