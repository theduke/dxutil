use super::{
    EngineError, model,
    selection::{EditorSelection, Uid},
};
use crate::{AttributeMap, ComponentDocumentV2, ComponentNode, DocumentMetadata, NodeId};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TxKind {
    Typing,
    Deleting,
    Composition,
    InputRule,
    Command,
    Paste,
    Structural,
    Host,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HistoryMode {
    #[default]
    Push,
    Merge,
    Skip,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    SetContent {
        uid: Uid,
        before: ComponentNode,
        after: ComponentNode,
    },
    SetBlockAttrs {
        uid: Uid,
        before: AttributeMap,
        after: AttributeMap,
    },
    InsertBlocks {
        parent: Option<Uid>,
        index: usize,
        blocks: Vec<ComponentNode>,
    },
    RemoveBlocks {
        parent: Option<Uid>,
        index: usize,
        blocks: Vec<ComponentNode>,
    },
    /// `to.1` is an index in the target list after removing the source range.
    MoveBlocks {
        from: (Option<Uid>, usize, usize),
        to: (Option<Uid>, usize),
    },
    SetMetadata {
        before: DocumentMetadata,
        after: DocumentMetadata,
    },
    SetNodeId {
        path: Vec<usize>,
        before: Option<NodeId>,
        after: Option<NodeId>,
    },
    SetChildren {
        uid: Uid,
        before: Option<ComponentNode>,
        after: Option<ComponentNode>,
    },
}
impl Op {
    pub fn invert(&self) -> Self {
        match self {
            Self::SetContent { uid, before, after } => Self::SetContent {
                uid: uid.clone(),
                before: after.clone(),
                after: before.clone(),
            },
            Self::SetBlockAttrs { uid, before, after } => Self::SetBlockAttrs {
                uid: uid.clone(),
                before: after.clone(),
                after: before.clone(),
            },
            Self::InsertBlocks {
                parent,
                index,
                blocks,
            } => Self::RemoveBlocks {
                parent: parent.clone(),
                index: *index,
                blocks: blocks.clone(),
            },
            Self::RemoveBlocks {
                parent,
                index,
                blocks,
            } => Self::InsertBlocks {
                parent: parent.clone(),
                index: *index,
                blocks: blocks.clone(),
            },
            Self::MoveBlocks { from, to } => Self::MoveBlocks {
                from: (to.0.clone(), to.1, from.2),
                to: (from.0.clone(), from.1),
            },
            Self::SetMetadata { before, after } => Self::SetMetadata {
                before: after.clone(),
                after: before.clone(),
            },
            Self::SetNodeId {
                path,
                before,
                after,
            } => Self::SetNodeId {
                path: path.clone(),
                before: after.clone(),
                after: before.clone(),
            },
            Self::SetChildren { uid, before, after } => Self::SetChildren {
                uid: uid.clone(),
                before: after.clone(),
                after: before.clone(),
            },
        }
    }
    pub fn apply(&self, doc: &mut ComponentDocumentV2) -> Result<(), EngineError> {
        let invalid = || {
            EngineError::InvalidOperation(format!("operation target is missing or stale: {self:?}"))
        };
        match self {
            Self::SetContent { uid, before, after } => {
                let node = model::content_mut(doc, uid).ok_or_else(invalid)?;
                if node != before {
                    return Err(invalid());
                }
                *node = after.clone();
            }
            Self::SetBlockAttrs { uid, before, after } => {
                let node = model::block_mut(doc, uid).ok_or_else(invalid)?;
                if &node.attrs != before {
                    return Err(invalid());
                }
                node.attrs = after.clone();
            }
            Self::InsertBlocks {
                parent,
                index,
                blocks,
            } => {
                let list = model::children_mut(doc, parent.as_deref()).ok_or_else(invalid)?;
                if *index > list.len() {
                    return Err(invalid());
                }
                list.splice(*index..*index, blocks.clone());
            }
            Self::RemoveBlocks {
                parent,
                index,
                blocks,
            } => {
                let list = model::children_mut(doc, parent.as_deref()).ok_or_else(invalid)?;
                let end = index.checked_add(blocks.len()).ok_or_else(invalid)?;
                if list.get(*index..end) != Some(blocks.as_slice()) {
                    return Err(invalid());
                }
                list.drain(*index..end);
                cleanup_empty_group(doc, parent.as_deref());
            }
            Self::MoveBlocks { from, to } => {
                let source = model::children(doc, from.0.as_deref()).ok_or_else(invalid)?;
                let end = from.1.checked_add(from.2).ok_or_else(invalid)?;
                let moved = source.get(from.1..end).ok_or_else(invalid)?.to_vec();
                // Moving a node into its own descendants would create an invalid tree.
                if to
                    .0
                    .as_ref()
                    .is_some_and(|target| moved.iter().any(|n| model::find(n, target).is_some()))
                {
                    return Err(invalid());
                }
                let target_len = model::children(doc, to.0.as_deref()).map_or(0, Vec::len);
                let final_len = target_len.saturating_sub(if from.0 == to.0 { from.2 } else { 0 });
                if to.1 > final_len {
                    return Err(invalid());
                }
                model::children_mut(doc, from.0.as_deref())
                    .ok_or_else(invalid)?
                    .drain(from.1..end);
                model::children_mut(doc, to.0.as_deref())
                    .ok_or_else(invalid)?
                    .splice(to.1..to.1, moved);
                cleanup_empty_group(doc, from.0.as_deref());
            }
            Self::SetMetadata { before, after } => {
                if &doc.metadata != before {
                    return Err(invalid());
                }
                doc.metadata = after.clone();
            }
            Self::SetNodeId {
                path,
                before,
                after,
            } => {
                let node = model::node_at_mut(&mut doc.root, path).ok_or_else(invalid)?;
                if &node.id != before {
                    return Err(invalid());
                }
                node.id = after.clone();
            }
            Self::SetChildren { uid, before, after } => {
                let node = model::block_mut(doc, uid).ok_or_else(invalid)?;
                let position = node
                    .content
                    .iter()
                    .position(|n| n.kind.0 == "block_children");
                if position.map(|i| &node.content[i]) != before.as_ref() {
                    return Err(invalid());
                }
                if let Some(i) = position {
                    node.content.remove(i);
                }
                if let Some(group) = after {
                    node.content.push(group.clone());
                }
            }
        }
        Ok(())
    }
    fn dirty(&self, dirty: &mut BTreeSet<Uid>, structure: &mut bool) {
        match self {
            Self::SetContent { uid, .. } | Self::SetBlockAttrs { uid, .. } => {
                dirty.insert(uid.clone());
            }
            Self::SetChildren { uid, .. } => {
                dirty.insert(uid.clone());
                *structure = true;
            }
            Self::InsertBlocks { parent, blocks, .. }
            | Self::RemoveBlocks { parent, blocks, .. } => {
                if let Some(uid) = parent {
                    dirty.insert(uid.clone());
                }
                for b in blocks {
                    if let Some(uid) = &b.id {
                        dirty.insert(uid.0.clone());
                    }
                }
                *structure = true;
            }
            Self::MoveBlocks { from, to } => {
                dirty.extend(from.0.clone());
                dirty.extend(to.0.clone());
                *structure = true;
            }
            Self::SetNodeId { .. } | Self::SetMetadata { .. } => {
                *structure = true;
            }
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Transaction {
    pub ops: Vec<Op>,
    pub selection_before: EditorSelection,
    pub selection_after: EditorSelection,
    pub kind: TxKind,
    pub history: HistoryMode,
}
impl Transaction {
    pub fn new(
        ops: Vec<Op>,
        selection_before: EditorSelection,
        selection_after: EditorSelection,
        kind: TxKind,
    ) -> Self {
        Self {
            ops,
            selection_before,
            selection_after,
            kind,
            history: HistoryMode::Push,
        }
    }
    pub fn invert(&self) -> Self {
        Self {
            ops: self.ops.iter().rev().map(Op::invert).collect(),
            selection_before: self.selection_after.clone(),
            selection_after: self.selection_before.clone(),
            kind: self.kind,
            history: self.history,
        }
    }
    pub fn apply(&self, doc: &mut ComponentDocumentV2) -> Result<RenderPlan, EngineError> {
        let mut done = Vec::new();
        let mut dirty = BTreeSet::new();
        let mut structure = false;
        for op in &self.ops {
            if let Err(error) = op.apply(doc) {
                for prior in done.into_iter().rev() {
                    let _: Result<(), EngineError> = Op::invert(prior).apply(doc);
                }
                return Err(error);
            }
            done.push(op);
            op.dirty(&mut dirty, &mut structure);
        }
        Ok(RenderPlan {
            dirty: dirty.into_iter().collect(),
            structure_changed: structure,
            full: false,
        })
    }
    /// Content-only transactions cannot invalidate block paths. Use the runtime
    /// index instead of walking the entire tree for each keystroke.
    pub fn apply_indexed(
        &self,
        doc: &mut ComponentDocumentV2,
        index: &model::BlockIndex,
    ) -> Result<RenderPlan, EngineError> {
        if !self.ops.iter().all(|op| {
            matches!(
                op,
                Op::SetContent { .. } | Op::SetBlockAttrs { .. } | Op::SetMetadata { .. }
            )
        }) {
            return self.apply(doc);
        }
        fn apply(
            op: &Op,
            doc: &mut ComponentDocumentV2,
            index: &model::BlockIndex,
        ) -> Result<(), EngineError> {
            let invalid = || {
                EngineError::InvalidOperation("indexed operation target is missing or stale".into())
            };
            match op {
                Op::SetContent { uid, before, after } => {
                    let path = index.paths.get(uid).ok_or_else(invalid)?;
                    let node = model::node_at_mut(&mut doc.root, path)
                        .and_then(|node| node.content.first_mut())
                        .ok_or_else(invalid)?;
                    if node != before {
                        return Err(invalid());
                    }
                    *node = after.clone();
                }
                Op::SetBlockAttrs { uid, before, after } => {
                    let node = model::node_at_mut(
                        &mut doc.root,
                        index.paths.get(uid).ok_or_else(invalid)?,
                    )
                    .ok_or_else(invalid)?;
                    if &node.attrs != before {
                        return Err(invalid());
                    }
                    node.attrs = after.clone();
                }
                _ => op.apply(doc)?,
            }
            Ok(())
        }
        let mut done: Vec<&Op> = Vec::new();
        let mut dirty = BTreeSet::new();
        let mut structure = false;
        for op in &self.ops {
            if let Err(error) = apply(op, doc, index) {
                for done in done.into_iter().rev() {
                    let _ = apply(&done.invert(), doc, index);
                }
                return Err(error);
            }
            done.push(op);
            op.dirty(&mut dirty, &mut structure);
        }
        Ok(RenderPlan {
            dirty: dirty.into_iter().collect(),
            structure_changed: structure,
            full: false,
        })
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RenderPlan {
    pub dirty: Vec<Uid>,
    pub structure_changed: bool,
    pub full: bool,
}

fn cleanup_empty_group(doc: &mut ComponentDocumentV2, parent: Option<&str>) {
    if let Some(uid) = parent
        && let Some(node) = model::block_mut(doc, uid)
    {
        node.content
            .retain(|n| n.kind.0 != "block_children" || !n.content.is_empty());
    }
}
