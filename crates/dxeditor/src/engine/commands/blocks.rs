use super::{BlockPlacement, Command};
use crate::engine::{
    EditorEngine, EngineOutput, FocusTarget, model,
    selection::{EditorSelection, InlineContainerRef, TextPoint, TextRange, Uid},
    text::{self, DeleteUnit, Direction},
    transaction::{Op, Transaction, TxKind},
};
use crate::{AttributeMap, ComponentMark, ComponentNode, InsertAt};

pub fn select(e: &mut EditorEngine, s: EditorSelection) -> Vec<EngineOutput> {
    e.selection = s.clone();
    vec![EngineOutput::ApplySelection(s)]
}
pub fn current_uid(e: &EditorEngine) -> Option<Uid> {
    match &e.selection {
        EditorSelection::Text(r) => owner(e, &r.head.container),
        EditorSelection::Blocks { head, .. } | EditorSelection::Node(head) => Some(head.clone()),
        EditorSelection::Cells { table, .. } => Some(table.clone()),
    }
}
pub fn owner(e: &EditorEngine, c: &InlineContainerRef) -> Option<Uid> {
    e.container_owner(c)
}

pub fn selected_uids(e: &EditorEngine) -> Vec<Uid> {
    if let EditorSelection::Blocks { anchor, head } = &e.selection {
        let order = model::block_order(&e.doc);
        if let (Some(a), Some(b)) = (
            order.iter().position(|x| x == anchor),
            order.iter().position(|x| x == head),
        ) {
            return order[a.min(b)..=a.max(b)].to_vec();
        }
    }
    current_uid(e).into_iter().collect()
}
fn tx(
    e: &mut EditorEngine,
    ops: Vec<Op>,
    selection: EditorSelection,
    kind: TxKind,
) -> Vec<EngineOutput> {
    let t = Transaction::new(ops, e.selection.clone(), selection, kind);
    e.transact(t).unwrap_or_else(|error| {
        vec![EngineOutput::Request(crate::EngineEvent::Error {
            code: "command".into(),
            message: error.to_string(),
            recoverable: true,
        })]
    })
}
pub fn set_content(
    e: &mut EditorEngine,
    uid: &str,
    after: ComponentNode,
    selection: EditorSelection,
    kind: TxKind,
) -> Vec<EngineOutput> {
    let Some(before) = e.content(uid).cloned() else {
        return vec![];
    };
    if before == after {
        return vec![];
    }
    tx(
        e,
        vec![Op::SetContent {
            uid: uid.into(),
            before,
            after,
        }],
        selection,
        kind,
    )
}
fn set_inline(
    e: &mut EditorEngine,
    c: &InlineContainerRef,
    nodes: Vec<ComponentNode>,
    selection: EditorSelection,
    kind: TxKind,
) -> Vec<EngineOutput> {
    let Some(uid) = owner(e, c) else {
        return vec![];
    };
    let Some(mut after) = e.content(&uid).cloned() else {
        return vec![];
    };
    match c {
        InlineContainerRef::Block(_) | InlineContainerRef::Caption(_) => after.content = nodes,
        InlineContainerRef::Cell { row, col } => {
            let Some(r) = model::find_mut(&mut after, row) else {
                return vec![];
            };
            let Some(cell) = r.content.get_mut(*col) else {
                return vec![];
            };
            cell.content = nodes;
        }
    }
    set_content(e, &uid, after, selection, kind)
}
fn selected_text(e: &EditorEngine) -> Option<(InlineContainerRef, usize, usize)> {
    let r = e.selection.text_range()?;
    if r.anchor.container != r.head.container {
        return None;
    }
    Some((
        r.head.container.clone(),
        r.anchor.offset.min(r.head.offset),
        r.anchor.offset.max(r.head.offset),
    ))
}
fn is_children_kind(kind: &str) -> bool {
    matches!(
        kind,
        "paragraph"
            | "heading"
            | "bulleted_list_item"
            | "numbered_list_item"
            | "todo"
            | "toggle"
            | "quote"
            | "callout"
    )
}

pub fn run(e: &mut EditorEngine, cmd: Command) -> Vec<EngineOutput> {
    match cmd {
        Command::InsertText(value) => {
            let mut n = ComponentNode::text(value);
            n.marks = e.stored_marks.clone();
            if !e.stored_marks_explicit {
                if let Some((c, start, _)) = selected_text(e) {
                    n.marks =
                        text::marks_at(e.inline(&c).map(Vec::as_slice).unwrap_or_default(), start);
                }
            }
            insert_inline(e, vec![n], TxKind::Typing)
        }
        Command::InsertInline(nodes) => insert_inline(e, nodes, TxKind::Typing),
        Command::ReplaceRange { range, text } => {
            e.selection = EditorSelection::Text(range);
            run(e, Command::InsertText(text))
        }
        Command::HardBreak => {
            insert_inline(e, vec![ComponentNode::new("hard_break")], TxKind::Typing)
        }
        Command::Delete { direction, unit } => delete(e, direction, unit),
        Command::SplitBlock => split_block(e),
        Command::ToggleMark(mark) => mark_command(e, mark, None),
        Command::SetMark(mark) => mark_command(e, mark, Some(false)),
        Command::UnsetMark(kind) => mark_command(e, ComponentMark::new(kind), Some(true)),
        Command::TurnInto { kind, attrs } => turn_into(e, &kind, attrs),
        Command::SetColor { key, color } => {
            if !matches!(key.as_str(), "text_color" | "background_color") {
                return vec![];
            }
            let Some(uid) = current_uid(e) else {
                return vec![];
            };
            let Some(mut after) = e.content(&uid).cloned() else {
                return vec![];
            };
            if let Some(color) = color {
                if !crate::profiles::block_document::COLOR_TOKENS.contains(&color.as_str()) {
                    return vec![];
                }
                after.attrs.insert(key, color.into());
            } else {
                after.attrs.remove(&key);
            }
            set_content(e, &uid, after, e.selection.clone(), TxKind::Structural)
        }
        Command::ToggleChecked => {
            let Some(uid) = current_uid(e) else {
                return vec![];
            };
            let Some(mut after) = e.content(&uid).cloned() else {
                return vec![];
            };
            if after.kind.0 != "todo" {
                return vec![];
            }
            let checked = after
                .attrs
                .get("checked")
                .and_then(|x| x.as_bool())
                .unwrap_or(false);
            after.attrs.insert("checked".into(), (!checked).into());
            set_content(e, &uid, after, e.selection.clone(), TxKind::Structural)
        }
        Command::Indent => reparent(e, true),
        Command::Outdent => reparent(e, false),
        Command::MoveUp => move_relative(e, false),
        Command::MoveDown => move_relative(e, true),
        Command::Duplicate => duplicate(e),
        Command::DeleteBlocks => delete_blocks(e),
        Command::InsertBlocks { at, nodes } => insert_blocks(e, at, nodes),
        Command::MoveBlocks {
            uids,
            target,
            placement,
        } => move_blocks(e, uids, target, placement),
        Command::ExitCode => exit(e, "code_block"),
        Command::ExitQuote => exit(e, "quote"),
        Command::ExitTrailingQuote => {
            let Some(uid) = current_uid(e) else {
                return vec![];
            };
            if model::block_order(&e.doc).last() != Some(&uid)
                || model::children(&e.doc, Some(&uid)).is_some_and(|n| !n.is_empty())
            {
                return vec![];
            }
            let Some((c, _, end)) = selected_text(e) else {
                return vec![];
            };
            if end != e.inline(&c).map_or(0, |n| text::len(n)) {
                return vec![];
            }
            exit(e, "quote")
        }
        Command::SelectBlocks => {
            let Some(uid) = current_uid(e) else {
                return vec![];
            };
            select(
                e,
                EditorSelection::Blocks {
                    anchor: uid.clone(),
                    head: uid,
                },
            )
        }
        Command::SelectAll => select_all(e),
        Command::MoveSelection { direction, extend } => move_selection(e, direction, extend),
        Command::SetSelection(s) => select(e, s),
        Command::EditSelected => edit_selected(e),
        Command::Focus => vec![EngineOutput::Focus {
            target: FocusTarget::Editor,
        }],
        Command::FocusAfterBlock(uid) => focus_after(e, uid),
        Command::FocusBeforeBlock(uid) => focus_before(e, uid),
        _ => vec![],
    }
}

pub fn insert_inline(
    e: &mut EditorEngine,
    nodes: Vec<ComponentNode>,
    kind: TxKind,
) -> Vec<EngineOutput> {
    let nodes = text::compact(nodes);
    if let Some((c, start, end)) = selected_text(e) {
        let Some(inline) = e.inline(&c) else {
            return vec![];
        };
        let at = start + text::len(&nodes);
        let after = text::replace(inline, start, end, nodes);
        return set_inline(e, &c, after, EditorSelection::caret(c.clone(), at), kind);
    }
    // Cross-container text ranges collapse at the join before insertion.
    if matches!(&e.selection, EditorSelection::Text(_)) {
        let Some(range) = e.selection.text_range() else {
            return vec![];
        };
        if owner(e, &range.anchor.container) == owner(e, &range.head.container) {
            return vec![];
        }
        return e.batch_commands(
            vec![
                Command::Delete {
                    direction: Direction::Backward,
                    unit: DeleteUnit::Selection,
                },
                Command::InsertInline(nodes),
            ],
            kind,
        );
    }
    let uids = selected_uids(e);
    let Some(first) = uids.first() else {
        return vec![];
    };
    let Some((parent, index)) = e.location(first) else {
        return vec![];
    };
    let roots = top_level_selection(e, &uids);
    let mut ops = Vec::new();
    for uid in roots.iter().rev() {
        if let Some((p, i)) = e.location(uid) {
            ops.push(Op::RemoveBlocks {
                parent: p,
                index: i,
                blocks: vec![e.block(uid).unwrap().clone()],
            });
        }
    }
    let mut node = model::paragraph();
    node.content[0].content = nodes;
    let uid = node.id.as_ref().unwrap().0.clone();
    let at = text::len(&node.content[0].content);
    ops.push(Op::InsertBlocks {
        parent,
        index,
        blocks: vec![node],
    });
    tx(
        e,
        ops,
        EditorSelection::caret(InlineContainerRef::Block(uid), at),
        kind,
    )
}
fn delete(e: &mut EditorEngine, direction: Direction, unit: DeleteUnit) -> Vec<EngineOutput> {
    if let DeleteUnit::TargetRange(range) = unit {
        e.selection = EditorSelection::Text(range);
        return delete(e, direction, DeleteUnit::Selection);
    }
    if let Some((c, start, end)) = selected_text(e) {
        if start != end {
            return set_inline(
                e,
                &c,
                text::replace(e.inline(&c).unwrap(), start, end, vec![]),
                EditorSelection::caret(c.clone(), start),
                TxKind::Deleting,
            );
        }
        if matches!(unit, DeleteUnit::Selection) {
            return vec![];
        }
        let inline = e.inline(&c).unwrap();
        let boundary = text::deletion_boundary(inline, start, direction, &unit);
        if start != boundary {
            return set_inline(
                e,
                &c,
                text::replace(inline, start.min(boundary), start.max(boundary), vec![]),
                EditorSelection::caret(c.clone(), start.min(boundary)),
                TxKind::Deleting,
            );
        }
        if matches!(c, InlineContainerRef::Block(_)) {
            return merge(e, direction);
        }
        return vec![];
    }
    if matches!(&e.selection, EditorSelection::Text(_)) {
        return delete_cross_range(e);
    }
    delete_blocks(e)
}
fn delete_cross_range(e: &mut EditorEngine) -> Vec<EngineOutput> {
    let Some(range) = e.selection.text_range().cloned() else {
        return vec![];
    };
    let (Some(a), Some(b)) = (
        owner(e, &range.anchor.container),
        owner(e, &range.head.container),
    ) else {
        return vec![];
    };
    let order = model::block_order(&e.doc);
    let (Some(ai), Some(bi)) = (
        order.iter().position(|x| x == &a),
        order.iter().position(|x| x == &b),
    ) else {
        return vec![];
    };
    let (start, end, si, ei) = if ai <= bi {
        (range.anchor, range.head, ai, bi)
    } else {
        (range.head, range.anchor, bi, ai)
    };
    let Some(kept) = owner(e, &start.container) else {
        return vec![];
    };
    let (Some(first), Some(last)) = (e.inline(&start.container), e.inline(&end.container)) else {
        return vec![];
    };
    let mut joined = text::split(first, start.offset).0;
    joined.extend(text::split(last, end.offset).1);
    let Some(before) = e.content(&kept).cloned() else {
        return vec![];
    };
    let mut after = before.clone();
    after.content = text::compact(joined);
    let mut ops = vec![Op::SetContent {
        uid: kept,
        before,
        after,
    }];
    let removed = if si < ei {
        order[si + 1..=ei].to_vec()
    } else {
        Vec::new()
    };
    for uid in top_level_selection(e, &removed).iter().rev() {
        if let Some((parent, index)) = e.location(uid) {
            ops.push(Op::RemoveBlocks {
                parent,
                index,
                blocks: vec![e.block(uid).unwrap().clone()],
            });
        }
    }
    tx(
        e,
        ops,
        EditorSelection::caret(start.container, start.offset),
        TxKind::Deleting,
    )
}
fn mark_command(
    e: &mut EditorEngine,
    mark: ComponentMark,
    remove: Option<bool>,
) -> Vec<EngineOutput> {
    if !e.schema.allows_mark(&mark.kind.0) {
        return vec![];
    }
    if let Some(uid) = current_uid(e) {
        if e.content(&uid).is_some_and(|n| n.kind.0 == "code_block") {
            return vec![];
        }
    }
    let Some((c, mut start, mut end)) = selected_text(e) else {
        return mark_cross_range(e, mark, remove);
    };
    let inline = e.inline(&c).unwrap();
    if start == end && matches!(mark.kind.0.as_str(), "link" | "entity_link") {
        let active = text::marks_at(inline, start);
        if let Some(existing) = active.iter().find(|m| m.kind == mark.kind) {
            let mut position = 0;
            let mut first = None;
            for node in inline {
                let next = position + text::node_len(node);
                if node.marks.contains(existing) {
                    if first.is_none() {
                        first = Some(position);
                    }
                } else if let Some(from) = first.take() {
                    if start >= from && start <= position {
                        start = from;
                        end = position;
                        break;
                    }
                }
                position = next;
            }
            if let Some(from) = first {
                if start >= from && start <= position {
                    start = from;
                    end = position;
                }
            }
        }
    }
    if start == end {
        let mut marks = if e.stored_marks_explicit {
            e.stored_marks.clone()
        } else {
            text::marks_at(inline, start)
        };
        let remove = remove.unwrap_or_else(|| marks.iter().any(|m| m.kind == mark.kind));
        marks.retain(|m| m.kind != mark.kind);
        if !remove {
            marks.push(mark);
        }
        e.stored_marks = marks;
        e.stored_marks_explicit = true;
        return vec![];
    }
    let remove = remove.unwrap_or_else(|| {
        text::slice(inline, start, end)
            .iter()
            .filter(|n| n.kind.0 == "text")
            .all(|n| n.marks.iter().any(|m| m.kind == mark.kind))
    });
    let nodes = text::change_mark(inline, start, end, &mark, remove);
    set_inline(e, &c, nodes, e.selection.clone(), TxKind::Structural)
}
fn mark_cross_range(
    e: &mut EditorEngine,
    mark: ComponentMark,
    remove: Option<bool>,
) -> Vec<EngineOutput> {
    let Some(range) = e.selection.text_range().cloned() else {
        return vec![];
    };
    let (Some(a), Some(b)) = (
        owner(e, &range.anchor.container),
        owner(e, &range.head.container),
    ) else {
        return vec![];
    };
    let order = model::block_order(&e.doc);
    let (Some(ai), Some(bi)) = (
        order.iter().position(|u| u == &a),
        order.iter().position(|u| u == &b),
    ) else {
        return vec![];
    };
    let (start, end, from, to) = if ai <= bi {
        (range.anchor, range.head, ai, bi)
    } else {
        (range.head, range.anchor, bi, ai)
    };
    let mut ranges = Vec::new();
    for (i, uid) in order[from..=to].iter().enumerate() {
        if let Some(content) = e
            .content(uid)
            .filter(|n| model::is_text_kind(&n.kind.0) && n.kind.0 != "code_block")
        {
            let a = if i == 0 { start.offset } else { 0 };
            let b = if from + i == to {
                end.offset
            } else {
                text::len(&content.content)
            };
            ranges.push((uid.clone(), content.clone(), a, b));
        }
    }
    let remove = remove.unwrap_or_else(|| {
        ranges.iter().all(|(_, n, a, b)| {
            text::slice(&n.content, *a, *b)
                .iter()
                .filter(|n| n.kind.0 == "text")
                .all(|n| n.marks.iter().any(|m| m.kind == mark.kind))
        })
    });
    let ops = ranges
        .into_iter()
        .map(|(uid, before, a, b)| {
            let mut after = before.clone();
            after.content = text::change_mark(&before.content, a, b, &mark, remove);
            Op::SetContent { uid, before, after }
        })
        .collect();
    tx(e, ops, e.selection.clone(), TxKind::Structural)
}
fn split_block(e: &mut EditorEngine) -> Vec<EngineOutput> {
    if matches!(
        e.selection,
        EditorSelection::Node(_) | EditorSelection::Blocks { .. }
    ) {
        return edit_selected(e);
    }
    let Some((c, start, end)) = selected_text(e) else {
        return vec![];
    };
    if start != end {
        return e.batch_commands(
            vec![
                Command::Delete {
                    direction: Direction::Backward,
                    unit: DeleteUnit::Selection,
                },
                Command::SplitBlock,
            ],
            TxKind::Structural,
        );
    }
    if matches!(c, InlineContainerRef::Cell { .. }) {
        return run(e, Command::HardBreak);
    }
    let Some(uid) = current_uid(e) else {
        return vec![];
    };
    let Some(before) = e.content(&uid).cloned() else {
        return vec![];
    };
    if before.kind.0 == "code_block" {
        let plain = text::plain(&before.content);
        if start == plain.chars().count() && plain.ends_with("\n\n") {
            return exit(e, "code_block");
        }
        return run(e, Command::InsertText("\n".into()));
    }
    if !model::is_text_kind(&before.kind.0) {
        return vec![];
    }
    if before.content.is_empty()
        && matches!(
            before.kind.0.as_str(),
            "bulleted_list_item" | "numbered_list_item" | "todo" | "quote" | "callout"
        )
    {
        if e.location(&uid).is_some_and(|(p, _)| p.is_some()) {
            return reparent(e, false);
        }
        return turn_into(e, "paragraph", AttributeMap::new());
    }
    let (head, tail) = text::split(&before.content, start);
    let mut after = before.clone();
    after.content = head;
    let mut next = before.clone();
    next.content = tail;
    if matches!(
        next.kind.0.as_str(),
        "heading" | "quote" | "callout" | "toggle"
    ) {
        next.kind = "paragraph".into();
        next.attrs.clear();
    }
    if next.kind.0 == "todo" {
        next.attrs.insert("checked".into(), false.into());
    }
    let next = model::block_node(next);
    let next_uid = next.id.as_ref().unwrap().0.clone();
    let Some((mut parent, mut index)) = e.location(&uid) else {
        return vec![];
    };
    index += 1;
    if !e.collapsed.contains(&uid)
        && (before.kind.0 == "toggle"
            || model::children(&e.doc, Some(&uid)).is_some_and(|x| !x.is_empty()))
    {
        parent = Some(uid.clone());
        index = 0;
    }
    tx(
        e,
        vec![
            Op::SetContent { uid, before, after },
            Op::InsertBlocks {
                parent,
                index,
                blocks: vec![next],
            },
        ],
        EditorSelection::caret(InlineContainerRef::Block(next_uid), 0),
        TxKind::Structural,
    )
}
fn turn_into(e: &mut EditorEngine, kind: &str, attrs: AttributeMap) -> Vec<EngineOutput> {
    if !e.schema.allows_kind(kind) || !model::is_text_kind(kind) {
        return vec![];
    }
    let uids = selected_uids(e);
    let mut ops = vec![];
    for uid in uids {
        let Some(before) = e.content(&uid).cloned() else {
            continue;
        };
        if !model::is_text_kind(&before.kind.0) {
            continue;
        }
        let mut after = ComponentNode::new(kind);
        after.attrs = attrs.clone();
        if kind == "heading" {
            after.attrs.entry("level".to_string()).or_insert(1.into());
        }
        if kind == "todo" {
            after
                .attrs
                .entry("checked".to_string())
                .or_insert(false.into());
        }
        if let Some(spec) = e.schema.kinds.get(kind) {
            for (key, value) in &spec.attributes {
                if let Some(default) = &value.default {
                    after
                        .attrs
                        .entry(key.clone())
                        .or_insert_with(|| default.clone());
                }
            }
        }
        after.content = if kind == "code_block" {
            let value = text::plain(&before.content);
            if value.is_empty() {
                vec![]
            } else {
                vec![ComponentNode::text(value)]
            }
        } else {
            before.content.clone()
        };
        ops.push(Op::SetContent {
            uid: uid.clone(),
            before,
            after,
        });
        if !is_children_kind(kind) {
            let kids = model::children(&e.doc, Some(&uid))
                .cloned()
                .unwrap_or_default();
            if !kids.is_empty() {
                let (parent, index) = e.location(&uid).unwrap();
                ops.push(Op::RemoveBlocks {
                    parent: Some(uid),
                    index: 0,
                    blocks: kids.clone(),
                });
                ops.push(Op::InsertBlocks {
                    parent,
                    index: index + 1,
                    blocks: kids,
                });
            }
        }
    }
    tx(e, ops, e.selection.clone(), TxKind::Structural)
}
fn merge(e: &mut EditorEngine, direction: Direction) -> Vec<EngineOutput> {
    let Some(uid) = current_uid(e) else {
        return vec![];
    };
    let Some(before) = e.content(&uid).cloned() else {
        return vec![];
    };
    if direction == Direction::Backward {
        let (parent, index) = e.location(&uid).unwrap();
        if index > 0 {
            let prev = &model::children(&e.doc, parent.as_deref()).unwrap()[index - 1];
            if prev
                .content
                .first()
                .is_some_and(|c| c.kind.0 == "details_break")
            {
                return tx(
                    e,
                    vec![Op::RemoveBlocks {
                        parent,
                        index: index - 1,
                        blocks: vec![prev.clone()],
                    }],
                    e.selection.clone(),
                    TxKind::Deleting,
                );
            }
        }
        if before.kind.0 != "paragraph" {
            return turn_into(e, "paragraph", AttributeMap::new());
        }
        if parent.is_some() {
            return reparent(e, false);
        }
    }
    let order = model::block_order(&e.doc);
    let Some(index) = order.iter().position(|x| x == &uid) else {
        return vec![];
    };
    let adjacent = match direction {
        Direction::Backward => index.checked_sub(1),
        Direction::Forward => Some(index + 1),
    }
    .and_then(|i| order.get(i))
    .cloned();
    let Some(adjacent) = adjacent else {
        return vec![];
    };
    let Some(other) = e.content(&adjacent).cloned() else {
        return vec![];
    };
    if !model::is_text_kind(&other.kind.0) {
        return select(e, EditorSelection::Node(adjacent));
    }
    let (keep, remove, mut after, append) = if direction == Direction::Backward {
        (adjacent.clone(), uid.clone(), other.clone(), before.content)
    } else {
        (uid.clone(), adjacent.clone(), before.clone(), other.content)
    };
    let offset = text::len(&after.content);
    after.content.extend(append);
    after.content = text::compact(after.content);
    let kept_before = e.content(&keep).unwrap().clone();
    let mut ops = vec![Op::SetContent {
        uid: keep.clone(),
        before: kept_before,
        after,
    }];
    let kids = model::children(&e.doc, Some(&remove))
        .cloned()
        .unwrap_or_default();
    // Detach descendant before removing its parent; this prevents Delete-forward duplicating children.
    if !kids.is_empty() {
        ops.push(Op::RemoveBlocks {
            parent: Some(remove.clone()),
            index: 0,
            blocks: kids.clone(),
        });
        let count = model::children(&e.doc, Some(&keep)).map_or(0, Vec::len);
        ops.push(Op::InsertBlocks {
            parent: Some(keep.clone()),
            index: count,
            blocks: kids.clone(),
        });
    }
    let (parent, index) = e.location(&remove).unwrap();
    let mut removed = e.block(&remove).unwrap().clone();
    if !kids.is_empty() {
        removed.content.retain(|n| n.kind.0 != "block_children");
    }
    ops.push(Op::RemoveBlocks {
        parent,
        index,
        blocks: vec![removed],
    });
    tx(
        e,
        ops,
        EditorSelection::caret(InlineContainerRef::Block(keep), offset),
        TxKind::Deleting,
    )
}
fn top_level_selection(e: &EditorEngine, uids: &[Uid]) -> Vec<Uid> {
    uids.iter()
        .filter(|uid| {
            let mut at = e.location(uid).and_then(|(p, _)| p);
            while let Some(parent) = at {
                if uids.contains(&parent) {
                    return false;
                }
                at = e.location(&parent).and_then(|(p, _)| p);
            }
            true
        })
        .cloned()
        .collect()
}
fn delete_blocks(e: &mut EditorEngine) -> Vec<EngineOutput> {
    let uids = top_level_selection(e, &selected_uids(e));
    let mut ops = vec![];
    for uid in uids.iter().rev() {
        if let Some((parent, index)) = e.location(uid) {
            ops.push(Op::RemoveBlocks {
                parent,
                index,
                blocks: vec![e.block(uid).unwrap().clone()],
            });
        }
    }
    let order = model::block_order(&e.doc);
    let next = order.iter().find(|u| !uids.contains(u)).cloned();
    let selection = next
        .map(|uid| EditorSelection::caret(InlineContainerRef::Block(uid), 0))
        .unwrap_or_else(|| e.selection.clone());
    tx(e, ops, selection, TxKind::Structural)
}
/// Structural selections are planned against a temporary tree and committed once.
fn plan_op(doc: &mut crate::ComponentDocumentV2, ops: &mut Vec<Op>, op: Op) -> bool {
    if op.apply(doc).is_ok() {
        ops.push(op);
        true
    } else {
        false
    }
}
fn reparent(e: &mut EditorEngine, indent: bool) -> Vec<EngineOutput> {
    let mut uids = top_level_selection(e, &selected_uids(e));
    if !indent {
        uids.reverse();
    }
    let mut doc = e.doc.clone();
    let mut ops = Vec::new();
    for uid in uids {
        let Some((parent, index)) = model::location(&doc, &uid) else {
            continue;
        };
        if indent {
            if index == 0 {
                continue;
            }
            let prev = &model::children(&doc, parent.as_deref()).unwrap()[index - 1];
            if !prev
                .content
                .first()
                .is_some_and(|c| is_children_kind(&c.kind.0))
            {
                continue;
            }
            let target = prev.id.as_ref().unwrap().0.clone();
            let count = model::children(&doc, Some(&target)).map_or(0, Vec::len);
            plan_op(
                &mut doc,
                &mut ops,
                Op::MoveBlocks {
                    from: (parent, index, 1),
                    to: (Some(target), count),
                },
            );
        } else if let Some(parent_uid) = parent {
            let Some((grand, pi)) = model::location(&doc, &parent_uid) else {
                continue;
            };
            let following = model::children(&doc, Some(&parent_uid)).unwrap().len() - index - 1;
            if following > 0
                && !model::content(&doc, &uid).is_some_and(|c| is_children_kind(&c.kind.0))
            {
                continue;
            }
            if following > 0 {
                let count = model::children(&doc, Some(&uid)).map_or(0, Vec::len);
                plan_op(
                    &mut doc,
                    &mut ops,
                    Op::MoveBlocks {
                        from: (Some(parent_uid.clone()), index + 1, following),
                        to: (Some(uid.clone()), count),
                    },
                );
            }
            plan_op(
                &mut doc,
                &mut ops,
                Op::MoveBlocks {
                    from: (Some(parent_uid), index, 1),
                    to: (grand, pi + 1),
                },
            );
        }
    }
    tx(e, ops, e.selection.clone(), TxKind::Structural)
}
fn move_relative(e: &mut EditorEngine, down: bool) -> Vec<EngineOutput> {
    let mut uids = top_level_selection(e, &selected_uids(e));
    if down {
        uids.reverse();
    }
    let mut doc = e.doc.clone();
    let mut ops = vec![];
    for uid in uids {
        let Some((parent, index)) = model::location(&doc, &uid) else {
            continue;
        };
        let siblings = model::children(&doc, parent.as_deref()).unwrap();
        let to = if down && index + 1 < siblings.len() {
            Some((parent.clone(), index + 1))
        } else if !down && index > 0 {
            Some((parent.clone(), index - 1))
        } else {
            parent
                .as_ref()
                .and_then(|p| model::location(&doc, p))
                .map(|(p, i)| (p, i + usize::from(down)))
        };
        if let Some(to) = to {
            plan_op(
                &mut doc,
                &mut ops,
                Op::MoveBlocks {
                    from: (parent, index, 1),
                    to,
                },
            );
        }
    }
    tx(e, ops, e.selection.clone(), TxKind::Structural)
}
fn duplicate(e: &mut EditorEngine) -> Vec<EngineOutput> {
    let uids = top_level_selection(e, &selected_uids(e));
    let mut ops = vec![];
    for uid in uids.iter().rev() {
        let Some((parent, index)) = e.location(uid) else {
            continue;
        };
        let mut node = e.block(uid).unwrap().clone();
        fn fresh(n: &mut ComponentNode) {
            if n.id.is_some() {
                n.id = Some(crate::NodeId::new(crate::engine::identity::new_uid()));
            }
            for c in &mut n.content {
                fresh(c);
            }
        }
        fresh(&mut node);
        ops.push(Op::InsertBlocks {
            parent,
            index: index + 1,
            blocks: vec![node],
        });
    }
    tx(e, ops, e.selection.clone(), TxKind::Structural)
}
pub fn insert_blocks(
    e: &mut EditorEngine,
    at: InsertAt,
    mut nodes: Vec<ComponentNode>,
) -> Vec<EngineOutput> {
    if nodes.is_empty() {
        return vec![];
    }
    for node in &mut nodes {
        if node.kind.0 != "block" {
            *node = model::block_node(node.clone());
        } else if node.id.is_none() {
            node.id = Some(crate::NodeId::new(crate::engine::identity::new_uid()));
        }
    }
    let target = match at {
        InsertAt::AfterBlock { uid } => Some((uid, false)),
        _ => current_uid(e).map(|u| (u, true)),
    };
    let (parent, index, replace) = if let Some((uid, selection)) = target {
        let Some((p, i)) = e.location(&uid) else {
            return vec![];
        };
        let empty = selection
            && e.content(&uid)
                .is_some_and(|c| c.kind.0 == "paragraph" && c.content.is_empty())
            && !model::children(&e.doc, Some(&uid)).is_some_and(|c| !c.is_empty());
        (
            p,
            i + usize::from(!empty),
            if empty { e.block(&uid).cloned() } else { None },
        )
    } else {
        (
            None,
            model::children(&e.doc, None).map_or(0, Vec::len),
            None,
        )
    };
    let last = nodes
        .last()
        .and_then(|n| n.id.as_ref())
        .map(|id| id.0.clone())
        .unwrap();
    let mut ops = vec![];
    if let Some(node) = replace {
        ops.push(Op::RemoveBlocks {
            parent: parent.clone(),
            index,
            blocks: vec![node],
        });
    }
    ops.push(Op::InsertBlocks {
        parent,
        index,
        blocks: nodes,
    });
    tx(e, ops, EditorSelection::Node(last), TxKind::Structural)
}
fn move_blocks(
    e: &mut EditorEngine,
    uids: Vec<Uid>,
    target: Uid,
    placement: BlockPlacement,
) -> Vec<EngineOutput> {
    let roots = top_level_selection(e, &uids);
    if roots.contains(&target)
        || roots.iter().any(|u| {
            e.block(u)
                .is_some_and(|b| model::find(b, &target).is_some())
        })
    {
        return vec![];
    }
    let iter: Vec<_> = if placement == BlockPlacement::Before {
        roots
    } else {
        roots.into_iter().rev().collect()
    };
    let mut doc = e.doc.clone();
    let mut ops = vec![];
    for uid in iter {
        let Some((from, index)) = model::location(&doc, &uid) else {
            continue;
        };
        let to = if placement == BlockPlacement::Inside {
            if !model::content(&doc, &target).is_some_and(|c| is_children_kind(&c.kind.0)) {
                continue;
            }
            Some((Some(target.clone()), 0))
        } else {
            model::location(&doc, &target)
                .map(|(p, i)| (p, i + usize::from(placement == BlockPlacement::After)))
        };
        if let Some(mut to) = to {
            if to.0 == from && to.1 > index {
                to.1 -= 1;
            }
            plan_op(
                &mut doc,
                &mut ops,
                Op::MoveBlocks {
                    from: (from, index, 1),
                    to,
                },
            );
        }
    }
    tx(e, ops, e.selection.clone(), TxKind::Structural)
}
fn exit(e: &mut EditorEngine, kind: &str) -> Vec<EngineOutput> {
    let Some(uid) = current_uid(e) else {
        return vec![];
    };
    let Some(before) = e.content(&uid).cloned() else {
        return vec![];
    };
    if before.kind.0 != kind {
        return vec![];
    }
    let mut ops = vec![];
    if kind == "code_block" {
        let value = text::plain(&before.content);
        if let Some(value) = value.strip_suffix("\n\n") {
            let mut after = before.clone();
            after.content = if value.is_empty() {
                vec![]
            } else {
                vec![ComponentNode::text(value)]
            };
            ops.push(Op::SetContent {
                uid: uid.clone(),
                before,
                after,
            });
        }
    }
    let (parent, index) = e.location(&uid).unwrap();
    let node = model::paragraph();
    let next = node.id.as_ref().unwrap().0.clone();
    ops.push(Op::InsertBlocks {
        parent,
        index: index + 1,
        blocks: vec![node],
    });
    tx(
        e,
        ops,
        EditorSelection::caret(InlineContainerRef::Block(next), 0),
        TxKind::Structural,
    )
}
fn select_all(e: &mut EditorEngine) -> Vec<EngineOutput> {
    if let Some((c, start, end)) = selected_text(e) {
        let length = e.inline(&c).map_or(0, |n| text::len(n));
        if start != 0 || end != length {
            return select(
                e,
                EditorSelection::Text(TextRange {
                    anchor: TextPoint::new(c.clone(), 0),
                    head: TextPoint::new(c, length),
                }),
            );
        }
    }
    let order = model::block_order(&e.doc);
    let (Some(first), Some(last)) = (order.first(), order.last()) else {
        return vec![];
    };
    select(
        e,
        EditorSelection::Blocks {
            anchor: first.clone(),
            head: last.clone(),
        },
    )
}
fn move_selection(e: &mut EditorEngine, direction: Direction, extend: bool) -> Vec<EngineOutput> {
    let Some(uid) = current_uid(e) else {
        return vec![];
    };
    let order = model::block_order(&e.doc);
    let Some(i) = order.iter().position(|x| x == &uid) else {
        return vec![];
    };
    let next = match direction {
        Direction::Backward => i.checked_sub(1),
        Direction::Forward => Some(i + 1),
    }
    .and_then(|i| order.get(i))
    .cloned();
    let Some(head) = next else { return vec![] };
    let anchor = if extend {
        if let EditorSelection::Blocks { anchor, .. } = &e.selection {
            anchor.clone()
        } else {
            uid
        }
    } else {
        head.clone()
    };
    select(e, EditorSelection::Blocks { anchor, head })
}
fn edit_selected(e: &mut EditorEngine) -> Vec<EngineOutput> {
    let Some(uid) = current_uid(e) else {
        return vec![];
    };
    let Some(node) = e.content(&uid) else {
        return vec![];
    };
    if model::is_text_kind(&node.kind.0) {
        return select(
            e,
            EditorSelection::caret(InlineContainerRef::Block(uid), text::len(&node.content)),
        );
    }
    if node.kind.0 == "image" && e.profile != crate::engine::Profile::MarkdownNotes {
        return select(
            e,
            EditorSelection::caret(InlineContainerRef::Caption(uid), text::len(&node.content)),
        );
    }
    if crate::engine::identity::is_island_kind(&node.kind.0) {
        vec![EngineOutput::Focus {
            target: FocusTarget::Island(uid),
        }]
    } else {
        focus_after(e, uid)
    }
}
fn focus_after(e: &mut EditorEngine, uid: Uid) -> Vec<EngineOutput> {
    let order = model::block_order(&e.doc);
    if let Some(next) = order
        .iter()
        .position(|x| x == &uid)
        .and_then(|i| order.get(i + 1))
    {
        if let Some(content) = e.content(next) {
            if model::is_text_kind(&content.kind.0) {
                let mut out = select(
                    e,
                    EditorSelection::caret(InlineContainerRef::Block(next.clone()), 0),
                );
                out.push(EngineOutput::Focus {
                    target: FocusTarget::Editor,
                });
                return out;
            }
        }
    }
    let mut out = insert_blocks(e, InsertAt::AfterBlock { uid }, vec![model::paragraph()]);
    out.extend(edit_selected(e));
    out.push(EngineOutput::Focus {
        target: FocusTarget::Editor,
    });
    out
}

fn focus_before(e: &mut EditorEngine, uid: Uid) -> Vec<EngineOutput> {
    let order = model::block_order(&e.doc);
    if let Some(previous) = order
        .iter()
        .position(|x| x == &uid)
        .and_then(|i| i.checked_sub(1))
        .and_then(|i| order.get(i))
    {
        if let Some(content) = e.content(previous) {
            if model::is_text_kind(&content.kind.0) {
                let offset = text::len(&content.content);
                let mut out = select(
                    e,
                    EditorSelection::caret(InlineContainerRef::Block(previous.clone()), offset),
                );
                out.push(EngineOutput::Focus {
                    target: FocusTarget::Editor,
                });
                return out;
            }
        }
    }
    let Some((parent, index)) = e.location(&uid) else {
        return vec![];
    };
    let node = model::paragraph();
    let next = node.id.as_ref().unwrap().0.clone();
    let mut out = tx(
        e,
        vec![Op::InsertBlocks {
            parent,
            index,
            blocks: vec![node],
        }],
        EditorSelection::caret(InlineContainerRef::Block(next), 0),
        TxKind::Structural,
    );
    out.push(EngineOutput::Focus {
        target: FocusTarget::Editor,
    });
    out
}
