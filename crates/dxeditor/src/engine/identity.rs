//! Identity maintenance belongs to editing, never serialization.
use super::transaction::Op;
use crate::{AttributeMap, ComponentDocumentV2, ComponentNode, NodeId};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub fn new_uid() -> String {
    uuid::Uuid::now_v7().to_string()
}

pub fn owned_uids(value: &Value) -> BTreeSet<String> {
    fn visit(value: &Value, ids: &mut BTreeSet<String>) {
        match value {
            Value::Array(values) => {
                for value in values {
                    visit(value, ids);
                }
            }
            Value::Object(values) => {
                for (key, value) in values {
                    if matches!(key.as_str(), "uid" | "id")
                        && let Some(id) = value.as_str()
                        && uuid::Uuid::parse_str(id).is_ok()
                    {
                        ids.insert(id.to_string());
                    } else {
                        visit(value, ids);
                    }
                }
            }
            _ => {}
        }
    }
    let mut ids = BTreeSet::new();
    visit(value, &mut ids);
    ids
}
pub fn rekey_owned(value: &mut Value, ids: &BTreeSet<String>) {
    fn visit(value: &mut Value, remap: &BTreeMap<String, String>) {
        match value {
            Value::String(id) => {
                if let Some(fresh) = remap.get(id) {
                    *id = fresh.clone();
                }
            }
            Value::Array(values) => {
                for value in values {
                    visit(value, remap);
                }
            }
            Value::Object(values) => {
                for value in values.values_mut() {
                    visit(value, remap);
                }
            }
            _ => {}
        }
    }
    let remap = ids.iter().map(|id| (id.clone(), new_uid())).collect();
    visit(value, &remap);
}
/// A copied subtree receives fresh block/row IDs and all island-owned identities.
pub fn rekey(node: &mut ComponentNode) {
    if node.id.is_some() || matches!(node.kind.0.as_str(), "block" | "table_row") {
        node.id = Some(NodeId::new(new_uid()));
    }
    if is_island_kind(&node.kind.0) {
        let mut attrs = Value::Object(node.attrs.clone());
        let ids = owned_uids(&attrs);
        rekey_owned(&mut attrs, &ids);
        if let Value::Object(attrs) = attrs {
            node.attrs = attrs;
        }
    }
    for child in &mut node.content {
        rekey(child);
    }
}

/// Returns invertible repairs. Existing islands claim their owned IDs before newly inserted copies.
pub fn repairs(
    doc: &ComponentDocumentV2,
    existing_islands: &BTreeMap<String, AttributeMap>,
) -> Vec<Op> {
    fn ids(
        node: &ComponentNode,
        path: &mut Vec<usize>,
        seen: &mut BTreeSet<String>,
        ops: &mut Vec<Op>,
    ) {
        if node.id.is_some() || matches!(node.kind.0.as_str(), "block" | "table_row") {
            let required_uuid = matches!(node.kind.0.as_str(), "block" | "table_row");
            let valid = node.id.as_ref().is_some_and(|id| {
                !id.0.trim().is_empty()
                    && (!required_uuid || uuid::Uuid::parse_str(&id.0).is_ok())
                    && seen.insert(id.0.clone())
            });
            if !valid {
                let id = new_uid();
                seen.insert(id.clone());
                ops.push(Op::SetNodeId {
                    path: path.clone(),
                    before: node.id.clone(),
                    after: Some(NodeId::new(id)),
                });
            }
        }
        for (i, child) in node.content.iter().enumerate() {
            path.push(i);
            ids(child, path, seen, ops);
            path.pop();
        }
    }
    let mut ops = Vec::new();
    ids(&doc.root, &mut Vec::new(), &mut BTreeSet::new(), &mut ops);
    let repaired = ops
        .iter()
        .filter_map(|op| {
            if let Op::SetNodeId {
                path,
                after: Some(id),
                ..
            } = op
            {
                Some((path.clone(), id.0.clone()))
            } else {
                None
            }
        })
        .collect::<BTreeMap<_, _>>();
    fn collect_islands(
        node: &ComponentNode,
        path: &mut Vec<usize>,
        repaired: &BTreeMap<Vec<usize>, String>,
        islands: &mut Vec<(String, ComponentNode)>,
    ) {
        if node.kind.0 == "block"
            && let Some(uid) = repaired
                .get(path)
                .cloned()
                .or_else(|| node.id.as_ref().map(|id| id.0.clone()))
            && let Some(content) = node.content.first()
            && is_island_kind(&content.kind.0)
        {
            islands.push((uid, content.clone()));
        }
        for (i, child) in node.content.iter().enumerate() {
            path.push(i);
            collect_islands(child, path, repaired, islands);
            path.pop();
        }
    }
    let mut islands = Vec::new();
    collect_islands(&doc.root, &mut Vec::new(), &repaired, &mut islands);
    islands.sort_by_key(|(uid, content)| {
        !existing_islands
            .get(uid)
            .is_some_and(|attrs| attrs == &content.attrs)
    });
    let mut claimed = BTreeSet::new();
    for (uid, content) in islands {
        let mut attrs = Value::Object(content.attrs.clone());
        let ids = owned_uids(&attrs);
        if !ids.is_disjoint(&claimed) {
            rekey_owned(&mut attrs, &ids);
            let mut after = content.clone();
            if let Value::Object(attrs) = attrs {
                after.attrs = attrs;
            }
            claimed.extend(owned_uids(&Value::Object(after.attrs.clone())));
            ops.push(Op::SetContent {
                uid,
                before: content,
                after,
            });
        } else {
            claimed.extend(ids);
        }
    }
    ops
}

pub fn is_island_kind(kind: &str) -> bool {
    matches!(
        kind,
        "graph" | "details_break" | "file" | "bookmark" | "entity_embed" | "unsupported"
    )
}

/// New subtrees are copies when their declared IDs are already owned by the
/// live document. Repair them before insertion, so a copy placed before its
/// source can never take the source's block or embedded graph identities.
pub fn prepare_insertions(doc: &ComponentDocumentV2, ops: &mut [Op]) -> BTreeMap<String, String> {
    if !ops.iter().any(|op| matches!(op, Op::InsertBlocks { .. })) {
        return BTreeMap::new();
    }
    fn ids(node: &ComponentNode, seen: &mut BTreeSet<String>) {
        if let Some(id) = &node.id {
            seen.insert(id.0.clone());
        }
        for child in &node.content {
            ids(child, seen);
        }
    }
    let mut seen = BTreeSet::new();
    ids(&doc.root, &mut seen);
    let mut removed = BTreeSet::new();
    let mut retained = BTreeSet::new();
    for op in ops.iter() {
        match op {
            Op::RemoveBlocks { blocks, .. } => {
                for block in blocks {
                    ids(block, &mut removed);
                }
            }
            Op::SetContent { before, after, .. } => {
                ids(before, &mut removed);
                ids(after, &mut retained);
            }
            Op::SetChildren { before, after, .. } => {
                if let Some(before) = before {
                    ids(before, &mut removed);
                }
                if let Some(after) = after {
                    ids(after, &mut retained);
                }
            }
            _ => {}
        }
    }
    seen.retain(|id| !removed.contains(id));
    seen.extend(retained);
    fn collision(
        node: &ComponentNode,
        seen: &BTreeSet<String>,
        local: &mut BTreeSet<String>,
    ) -> bool {
        let here = node
            .id
            .as_ref()
            .is_some_and(|id| seen.contains(&id.0) || !local.insert(id.0.clone()));
        here || node
            .content
            .iter()
            .any(|child| collision(child, seen, local))
    }
    fn mapping(
        before: &ComponentNode,
        after: &ComponentNode,
        result: &mut BTreeMap<String, String>,
    ) {
        if let (Some(before), Some(after)) = (&before.id, &after.id) {
            result.insert(before.0.clone(), after.0.clone());
        }
        for (before, after) in before.content.iter().zip(&after.content) {
            mapping(before, after, result);
        }
    }
    let mut remap = BTreeMap::new();
    for op in ops {
        if let Op::InsertBlocks { blocks, .. } = op {
            for block in blocks {
                if collision(block, &seen, &mut BTreeSet::new()) {
                    let before = block.clone();
                    rekey(block);
                    mapping(&before, block, &mut remap);
                }
                ids(block, &mut seen);
            }
        }
    }
    remap
}
