use dxeditor::engine::{
    Command, EditorEngine, EditorSelection, EngineInput, EngineOptions, InlineContainerRef,
    Profile,
    commands::BlockPlacement,
    model,
    text::{self, DeleteUnit, Direction},
};
use dxeditor::{AttributeMap, ComponentNode, EditorCatalog};
fn p(s: &str) -> ComponentNode {
    let mut p = model::paragraph();
    if !s.is_empty() {
        p.content[0].content = vec![ComponentNode::text(s)];
    }
    p
}
fn engine(nodes: Vec<ComponentNode>) -> EditorEngine {
    EditorEngine::new(
        model::document(nodes),
        &EditorCatalog::block_document(),
        Profile::BlockDocument,
        EngineOptions::default(),
    )
    .unwrap()
}
fn uid(n: &ComponentNode) -> String {
    n.id.as_ref().unwrap().0.clone()
}
fn caret(e: &mut EditorEngine, id: &str, offset: usize) {
    e.handle(EngineInput::SelectionChanged(EditorSelection::caret(
        InlineContainerRef::Block(id.into()),
        offset,
    )));
}
fn root(e: &EditorEngine) -> &Vec<ComponentNode> {
    model::children(e.document(), None).unwrap()
}
#[test]
fn split_keeps_head_uid_and_marks() {
    let a = p("abc");
    let id = uid(&a);
    let mut e = engine(vec![a]);
    caret(&mut e, &id, 1);
    e.run(Command::SplitBlock);
    assert_eq!(uid(&root(&e)[0]), id);
    assert_eq!(root(&e)[0].text_content(), "a");
    assert_eq!(root(&e)[1].text_content(), "bc");
    e.run(Command::Undo);
    assert_eq!(root(&e).len(), 1);
    assert_eq!(root(&e)[0].text_content(), "abc");
}
#[test]
fn closed_toggle_split_is_visible_sibling() {
    let mut a = p("headtail");
    a.content[0].kind = "toggle".into();
    a.content.push(ComponentNode::container(
        "block_children",
        None,
        vec![p("kid")],
    ));
    let id = uid(&a);
    let mut e = engine(vec![a]);
    e.run(Command::SetCollapsed(vec![id.clone()]));
    caret(&mut e, &id, 4);
    e.run(Command::SplitBlock);
    assert_eq!(root(&e).len(), 2);
    assert_eq!(root(&e)[1].text_content(), "tail");
    assert_eq!(model::children(e.document(), Some(&id)).unwrap().len(), 1);
}
#[test]
fn open_toggle_tail_becomes_first_child() {
    let mut a = p("headtail");
    a.content[0].kind = "toggle".into();
    let id = uid(&a);
    let mut e = engine(vec![a]);
    caret(&mut e, &id, 4);
    e.run(Command::SplitBlock);
    let children = model::children(e.document(), Some(&id)).unwrap();
    assert_eq!(children[0].text_content(), "tail");
    assert_eq!(children[0].content[0].kind.0, "paragraph");
}
#[test]
fn heading_split_becomes_paragraph() {
    let mut a = p("title tail");
    a.content[0].kind = "heading".into();
    a.content[0].attrs.insert("level".into(), 2.into());
    let id = uid(&a);
    let mut e = engine(vec![a]);
    caret(&mut e, &id, 5);
    e.run(Command::SplitBlock);
    assert_eq!(root(&e)[1].content[0].kind.0, "paragraph");
}
#[test]
fn delete_grapheme_and_word() {
    let a = p("a👩‍👩‍👦 words");
    let id = uid(&a);
    let mut e = engine(vec![a]);
    caret(&mut e, &id, 6);
    e.run(Command::Delete {
        direction: Direction::Backward,
        unit: DeleteUnit::Grapheme,
    });
    assert_eq!(root(&e)[0].text_content(), "a words");
    caret(&mut e, &id, 7);
    e.run(Command::Delete {
        direction: Direction::Backward,
        unit: DeleteUnit::Word,
    });
    assert_eq!(root(&e)[0].text_content(), "a ");
}
#[test]
fn backspace_nonparagraph_retains_uid_children() {
    let mut a = p("todo");
    a.content[0].kind = "todo".into();
    a.content[0].attrs.insert("checked".into(), false.into());
    a.content.push(ComponentNode::container(
        "block_children",
        None,
        vec![p("kid")],
    ));
    let id = uid(&a);
    let mut e = engine(vec![a]);
    caret(&mut e, &id, 0);
    e.run(Command::Delete {
        direction: Direction::Backward,
        unit: DeleteUnit::Grapheme,
    });
    assert_eq!(root(&e)[0].content[0].kind.0, "paragraph");
    assert_eq!(uid(&root(&e)[0]), id);
    assert_eq!(model::children(e.document(), Some(&id)).unwrap().len(), 1);
}
#[test]
fn indent_outdent_preserves_following_siblings() {
    let a = p("parent");
    let aid = uid(&a);
    let b = p("child");
    let bid = uid(&b);
    let c = p("following");
    let mut e = engine(vec![a, b, c]);
    caret(&mut e, &bid, 0);
    e.run(Command::Indent);
    assert_eq!(
        model::children(e.document(), Some(&aid)).unwrap()[0].text_content(),
        "child"
    );
    e.run(Command::Outdent);
    assert_eq!(
        root(&e)
            .iter()
            .map(ComponentNode::text_content)
            .collect::<Vec<_>>(),
        vec!["parent", "child", "following"]
    );
    e.run(Command::Undo);
    assert!(model::children(e.document(), Some(&aid)).is_some());
}
#[test]
fn outdent_adopts_following_siblings() {
    let b = p("child");
    let bid = uid(&b);
    let mut a = p("parent");
    a.content.push(ComponentNode::container(
        "block_children",
        None,
        vec![b, p("following")],
    ));
    let mut e = engine(vec![a]);
    caret(&mut e, &bid, 0);
    e.run(Command::Outdent);
    assert_eq!(root(&e).len(), 2);
    assert_eq!(
        model::children(e.document(), Some(&bid)).unwrap()[0].text_content(),
        "following"
    );
    e.run(Command::Undo);
    assert_eq!(root(&e).len(), 1);
}
#[test]
fn move_down_and_undo() {
    let a = p("a");
    let id = uid(&a);
    let mut e = engine(vec![a, p("b"), p("c")]);
    caret(&mut e, &id, 0);
    e.run(Command::MoveDown);
    assert_eq!(root(&e)[1].text_content(), "a");
    e.run(Command::Undo);
    assert_eq!(root(&e)[0].text_content(), "a");
}
#[test]
fn dnd_true_move_retains_ids() {
    let a = p("a");
    let aid = uid(&a);
    let b = p("b");
    let bid = uid(&b);
    let mut e = engine(vec![a, b]);
    e.run(Command::MoveBlocks {
        uids: vec![aid.clone()],
        target: bid,
        placement: BlockPlacement::After,
    });
    assert_eq!(uid(&root(&e)[1]), aid);
}
#[test]
fn insert_selection_replaces_empty_else_after() {
    let a = p("original");
    let aid = uid(&a);
    let mut e = engine(vec![a]);
    caret(&mut e, &aid, 0);
    e.run(Command::InsertBlocks {
        at: dxeditor::InsertAt::Selection,
        nodes: vec![p("new")],
    });
    assert_eq!(root(&e)[0].text_content(), "original");
    assert_eq!(root(&e)[1].text_content(), "new");
    let a = p("");
    let id = uid(&a);
    let mut e = engine(vec![a]);
    caret(&mut e, &id, 0);
    e.run(Command::InsertBlocks {
        at: dxeditor::InsertAt::Selection,
        nodes: vec![p("new")],
    });
    assert_eq!(root(&e).len(), 1);
}
#[test]
fn duplicate_fresh_identity_undo() {
    let a = p("a");
    let id = uid(&a);
    let mut e = engine(vec![a]);
    caret(&mut e, &id, 0);
    e.run(Command::Duplicate);
    assert_ne!(uid(&root(&e)[0]), uid(&root(&e)[1]));
    e.run(Command::Undo);
    assert_eq!(root(&e).len(), 1);
}
#[test]
fn forward_join_parent_child_without_duplicates() {
    let mut a = p("a");
    let id = uid(&a);
    a.content.push(ComponentNode::container(
        "block_children",
        None,
        vec![p("b")],
    ));
    let mut e = engine(vec![a]);
    caret(&mut e, &id, 1);
    e.run(Command::Delete {
        direction: Direction::Forward,
        unit: DeleteUnit::Grapheme,
    });
    assert_eq!(root(&e)[0].content[0].text_content(), "ab");
    assert!(model::children(e.document(), Some(&id)).is_none());
    e.run(Command::Undo);
    assert_eq!(root(&e)[0].content[0].text_content(), "a");
    assert_eq!(model::children(e.document(), Some(&id)).unwrap().len(), 1);
}
#[test]
fn readonly_rejects_commands() {
    let a = p("a");
    let mut e = engine(vec![a]);
    e.set_read_only(true);
    e.run(Command::InsertText("bad".into()));
    assert_eq!(root(&e)[0].text_content(), "a");
}
#[test]
fn escape_enter_places_end() {
    let a = p("abc");
    let id = uid(&a);
    let mut e = engine(vec![a]);
    caret(&mut e, &id, 1);
    e.run(Command::SelectBlocks);
    e.run(Command::EditSelected);
    assert_eq!(
        e.selection(),
        &EditorSelection::caret(InlineContainerRef::Block(id), 3)
    );
}
#[test]
fn code_conversion_promotes_children() {
    let mut a = p("parent");
    let id = uid(&a);
    a.content.push(ComponentNode::container(
        "block_children",
        None,
        vec![p("kid")],
    ));
    let mut e = engine(vec![a]);
    caret(&mut e, &id, 0);
    e.run(Command::TurnInto {
        kind: "code_block".into(),
        attrs: AttributeMap::new(),
    });
    assert_eq!(root(&e).len(), 2);
    e.run(Command::Undo);
    assert_eq!(root(&e).len(), 1);
}
#[test]
fn marks_at_collapsed_caret_are_used() {
    let a = p("");
    let id = uid(&a);
    let mut e = engine(vec![a]);
    caret(&mut e, &id, 0);
    e.run(Command::ToggleMark(dxeditor::ComponentMark::new("bold")));
    e.run(Command::InsertText("bold".into()));
    assert_eq!(root(&e)[0].content[0].content[0].marks[0].kind.0, "bold");
    assert_eq!(text::len(&root(&e)[0].content[0].content), 4);
}
#[test]
fn selected_block_group_moves_as_one_undo() {
    let a = p("a");
    let b = p("b");
    let aid = uid(&a);
    let bid = uid(&b);
    let mut e = engine(vec![a, b, p("c")]);
    e.run(Command::SetSelection(EditorSelection::Blocks {
        anchor: aid,
        head: bid,
    }));
    let out = e.run(Command::MoveDown);
    assert_eq!(
        out.iter()
            .filter(|o| matches!(o, dxeditor::engine::EngineOutput::Changed { .. }))
            .count(),
        1
    );
    assert_eq!(
        root(&e)
            .iter()
            .map(ComponentNode::text_content)
            .collect::<Vec<_>>(),
        vec!["c", "a", "b"]
    );
    e.run(Command::Undo);
    assert_eq!(
        root(&e)
            .iter()
            .map(ComponentNode::text_content)
            .collect::<Vec<_>>(),
        vec!["a", "b", "c"]
    );
}
#[test]
fn explicit_empty_marks_do_not_reinherit() {
    let mut a = p("bold");
    a.content[0].content[0]
        .marks
        .push(dxeditor::ComponentMark::new("bold"));
    let id = uid(&a);
    let mut e = engine(vec![a]);
    caret(&mut e, &id, 4);
    e.run(Command::ToggleMark(dxeditor::ComponentMark::new("bold")));
    e.run(Command::InsertText(" plain".into()));
    let nodes = &root(&e)[0].content[0].content;
    assert_eq!(nodes.len(), 2);
    assert!(nodes[1].marks.is_empty());
}
#[test]
fn cross_block_replace_is_one_transaction() {
    let a = p("abc");
    let aid = uid(&a);
    let b = p("xyz");
    let bid = uid(&b);
    let mut e = engine(vec![a, b]);
    e.run(Command::SetSelection(EditorSelection::Text(
        dxeditor::engine::TextRange {
            anchor: dxeditor::engine::TextPoint::new(InlineContainerRef::Block(aid), 1),
            head: dxeditor::engine::TextPoint::new(InlineContainerRef::Block(bid), 2),
        },
    )));
    let out = e.run(Command::InsertText("!".into()));
    assert_eq!(
        out.iter()
            .filter(|o| matches!(o, dxeditor::engine::EngineOutput::Changed { .. }))
            .count(),
        1
    );
    assert_eq!(root(&e)[0].text_content(), "a!z");
    e.run(Command::Undo);
    assert_eq!(root(&e).len(), 2);
    assert_eq!(root(&e)[0].text_content(), "abc");
}

#[test]
fn clearing_a_block_selection_leaves_a_genuinely_empty_paragraph() {
    let mut e = engine(vec![p("clear me"), p("and me")]);
    e.run(Command::SetSelection(EditorSelection::Blocks {
        anchor: uid(&root(&e)[0]),
        head: uid(&root(&e)[1]),
    }));
    e.run(Command::InsertText(String::new()));
    assert_eq!(root(&e).len(), 1);
    assert!(root(&e)[0].content[0].content.is_empty());
    e.run(Command::Undo);
    assert_eq!(root(&e).len(), 2);
    assert_eq!(root(&e)[0].text_content(), "clear me");
}

#[test]
fn select_all_selects_every_text_block_on_the_first_press_and_is_idempotent() {
    for lines in [&[""][..], &["🌍"][..], &["first", "second 🌍"][..]] {
        let mut e = engine(lines.iter().map(|line| p(line)).collect());
        let first = uid(&root(&e)[0]);
        let last = uid(root(&e).last().unwrap());
        let length = lines.last().unwrap().chars().count();
        caret(&mut e, &last, length);
        let before = e.document().clone();
        let expected = EditorSelection::Text(dxeditor::engine::TextRange {
            anchor: dxeditor::engine::TextPoint::new(InlineContainerRef::Block(first), 0),
            head: dxeditor::engine::TextPoint::new(InlineContainerRef::Block(last), length),
        });
        for _ in 0..3 {
            e.run(Command::SelectAll);
            assert_eq!(e.selection(), &expected);
        }
        e.run(Command::InsertText("replacement".into()));
        assert_eq!(root(&e).len(), 1);
        assert_eq!(root(&e)[0].text_content(), "replacement");
        e.run(Command::Undo);
        assert_eq!(e.document(), &before);
    }
}

#[test]
fn select_all_preserves_nested_and_nontext_blocks() {
    for (nested, collapsed) in [(true, false), (true, true), (false, false)] {
        let mut first = p("summary");
        if nested {
            first.content[0].kind = "toggle".into();
            first.content.push(ComponentNode::container(
                "block_children",
                None,
                vec![p("child")],
            ));
        } else {
            first = model::block_node(ComponentNode::new("divider"));
        }
        let first_id = uid(&first);
        let mut e = engine(vec![first, p("tail")]);
        if collapsed {
            e.run(Command::SetCollapsed(vec![first_id.clone()]));
        }
        let before = e.document().clone();
        let order = model::block_order(e.document());
        let expected = EditorSelection::Blocks {
            anchor: first_id,
            head: order.last().unwrap().clone(),
        };
        for _ in 0..3 {
            e.run(Command::SelectAll);
            assert_eq!(e.selection(), &expected);
        }
        e.run(Command::InsertText("replacement".into()));
        assert_eq!(root(&e).len(), 1);
        assert_eq!(root(&e)[0].text_content(), "replacement");
        e.run(Command::Undo);
        assert_eq!(e.document(), &before);
    }
}
