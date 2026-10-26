//! Host-owned block updates and transient view state. No transient attribute is
//! ever copied into the persisted CDV2 document or history.
use crate::engine::{
    EditorEngine, EngineOutput, model,
    selection::{EditorSelection, Uid},
    transaction::{HistoryMode, Op, RenderPlan, Transaction, TxKind},
};
use crate::{AttributeMap, ComponentNode};
use std::collections::BTreeSet;

pub fn update(
    engine: &mut EditorEngine,
    uid: Uid,
    attrs: AttributeMap,
    add_to_history: bool,
) -> Vec<EngineOutput> {
    if engine.options.read_only {
        return Vec::new();
    }
    let Some(before) = engine.content(&uid).cloned() else {
        return Vec::new();
    };
    let mut after = before.clone();
    for (key, value) in attrs {
        if value.is_null() {
            after.attrs.remove(&key);
        } else {
            after.attrs.insert(key, value);
        }
    }
    if after == before {
        return Vec::new();
    }
    let mut tx = Transaction::new(
        vec![Op::SetContent { uid, before, after }],
        engine.selection.clone(),
        engine.selection.clone(),
        TxKind::Host,
    );
    if !add_to_history {
        tx.history = if engine.can_undo() {
            HistoryMode::Merge
        } else {
            HistoryMode::Skip
        };
    }
    engine.transact(tx).unwrap_or_default()
}

pub fn transient(engine: &mut EditorEngine, uid: Uid, attrs: AttributeMap) -> Vec<EngineOutput> {
    if engine.block(&uid).is_none() {
        return Vec::new();
    }
    if attrs.is_empty() {
        engine.transient.remove(&uid);
    } else if engine.transient.get(&uid) == Some(&attrs) {
        return Vec::new();
    } else {
        engine.transient.insert(uid.clone(), attrs);
    }
    vec![EngineOutput::Render(RenderPlan {
        dirty: vec![uid],
        structure_changed: false,
        full: false,
    })]
}

pub fn replace(
    engine: &mut EditorEngine,
    uid: Uid,
    mut nodes: Vec<ComponentNode>,
) -> Vec<EngineOutput> {
    if engine.options.read_only {
        return Vec::new();
    }
    let Some((parent, index)) = engine.location(&uid) else {
        return Vec::new();
    };
    let Some(before) = engine.block(&uid).cloned() else {
        return Vec::new();
    };
    for node in &mut nodes {
        if node.kind.0 != "block" {
            *node = model::block_node(node.clone());
        }
    }
    let selection = nodes
        .last()
        .and_then(|node| node.id.as_ref())
        .map(|id| EditorSelection::Node(id.0.clone()))
        .unwrap_or_else(|| engine.selection.clone());
    let mut ops = vec![Op::RemoveBlocks {
        parent: parent.clone(),
        index,
        blocks: vec![before],
    }];
    if !nodes.is_empty() {
        ops.push(Op::InsertBlocks {
            parent,
            index,
            blocks: nodes,
        });
    }
    let tx = Transaction::new(ops, engine.selection.clone(), selection, TxKind::Host);
    engine.transact(tx).unwrap_or_default()
}

pub fn set_collapsed(engine: &mut EditorEngine, uids: Vec<Uid>) -> Vec<EngineOutput> {
    let next: BTreeSet<_> = uids
        .into_iter()
        .filter(|uid| engine.block(uid).is_some())
        .collect();
    if next == engine.collapsed {
        return Vec::new();
    }
    let dirty = next
        .symmetric_difference(&engine.collapsed)
        .cloned()
        .collect();
    engine.collapsed = next;
    vec![EngineOutput::Render(RenderPlan {
        dirty,
        structure_changed: true,
        full: false,
    })]
}

pub fn toggle_collapsed(engine: &mut EditorEngine, uid: Uid) -> Vec<EngineOutput> {
    if engine.options.read_only || engine.block(&uid).is_none() {
        return Vec::new();
    }
    let collapsed = if engine.collapsed.remove(&uid) {
        false
    } else {
        engine.collapsed.insert(uid.clone());
        true
    };
    vec![
        EngineOutput::Render(RenderPlan {
            dirty: vec![uid.clone()],
            structure_changed: true,
            full: false,
        }),
        EngineOutput::Request(crate::EngineEvent::ToggleCollapsed { uid, collapsed }),
    ]
}
