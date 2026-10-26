//! Small block-tree access helpers shared by commands and adapters.
use super::selection::{InlineContainerRef, Uid};
use crate::{ComponentDocumentV2, ComponentNode, NodeId};
use std::collections::BTreeMap;

pub type BlockPath = Vec<usize>;
#[derive(Clone, Debug, Default)]
pub struct BlockIndex {
    pub paths: BTreeMap<Uid, BlockPath>,
    pub rows: BTreeMap<Uid, (Uid, BlockPath)>,
    pub order: Vec<Uid>,
}
impl BlockIndex {
    pub fn new(doc: &ComponentDocumentV2) -> Self {
        fn visit(
            node: &ComponentNode,
            path: &mut BlockPath,
            index: &mut BlockIndex,
            owner: Option<&str>,
        ) {
            let owner = if node.kind.0 == "block" {
                node.id.as_ref().map(|id| id.0.as_str())
            } else {
                owner
            };
            if node.kind.0 == "block" {
                if let Some(id) = &node.id {
                    index.paths.insert(id.0.clone(), path.clone());
                    index.order.push(id.0.clone());
                }
            }
            if node.kind.0 == "table_row"
                && let (Some(id), Some(owner)) = (&node.id, owner)
            {
                index
                    .rows
                    .insert(id.0.clone(), (owner.to_string(), path.clone()));
            }
            for (i, child) in node.content.iter().enumerate() {
                path.push(i);
                visit(child, path, index, owner);
                path.pop();
            }
        }
        let mut index = Self::default();
        visit(&doc.root, &mut Vec::new(), &mut index, None);
        index
    }
    pub fn get<'a>(&self, doc: &'a ComponentDocumentV2, uid: &str) -> Option<&'a ComponentNode> {
        node_at(&doc.root, self.paths.get(uid)?)
    }
    pub fn refresh_table(&mut self, uid: &str, table: &ComponentNode) {
        self.rows.retain(|_, (owner, _)| owner != uid);
        if table.kind.0 != "table" {
            return;
        }
        let Some(block_path) = self.paths.get(uid) else {
            return;
        };
        for (i, row) in table.content.iter().enumerate() {
            if let Some(id) = &row.id {
                let mut path = block_path.clone();
                path.extend([0, i]);
                self.rows.insert(id.0.clone(), (uid.to_string(), path));
            }
        }
    }
}
pub fn inline_indexed<'a>(
    doc: &'a ComponentDocumentV2,
    index: &BlockIndex,
    container: &InlineContainerRef,
) -> Option<&'a Vec<ComponentNode>> {
    match container {
        InlineContainerRef::Block(uid) | InlineContainerRef::Caption(uid) => {
            Some(&index.get(doc, uid)?.content.first()?.content)
        }
        InlineContainerRef::Cell { row, col } => Some(
            &node_at(&doc.root, &index.rows.get(row)?.1)?
                .content
                .get(*col)?
                .content,
        ),
    }
}
pub fn node_at<'a>(mut node: &'a ComponentNode, path: &[usize]) -> Option<&'a ComponentNode> {
    for i in path {
        node = node.content.get(*i)?;
    }
    Some(node)
}
pub fn node_at_mut<'a>(
    mut node: &'a mut ComponentNode,
    path: &[usize],
) -> Option<&'a mut ComponentNode> {
    for i in path {
        node = node.content.get_mut(*i)?;
    }
    Some(node)
}
pub fn find<'a>(node: &'a ComponentNode, uid: &str) -> Option<&'a ComponentNode> {
    if node.id.as_ref().is_some_and(|id| id.0 == uid) {
        return Some(node);
    }
    node.content.iter().find_map(|n| find(n, uid))
}
pub fn find_mut<'a>(node: &'a mut ComponentNode, uid: &str) -> Option<&'a mut ComponentNode> {
    if node.id.as_ref().is_some_and(|id| id.0 == uid) {
        return Some(node);
    }
    node.content.iter_mut().find_map(|n| find_mut(n, uid))
}
pub fn block<'a>(doc: &'a ComponentDocumentV2, uid: &str) -> Option<&'a ComponentNode> {
    find(&doc.root, uid).filter(|n| n.kind.0 == "block")
}
pub fn block_mut<'a>(doc: &'a mut ComponentDocumentV2, uid: &str) -> Option<&'a mut ComponentNode> {
    find_mut(&mut doc.root, uid).filter(|n| n.kind.0 == "block")
}
pub fn content<'a>(doc: &'a ComponentDocumentV2, uid: &str) -> Option<&'a ComponentNode> {
    block(doc, uid)?.content.first()
}
pub fn content_mut<'a>(
    doc: &'a mut ComponentDocumentV2,
    uid: &str,
) -> Option<&'a mut ComponentNode> {
    block_mut(doc, uid)?.content.first_mut()
}
pub fn children<'a>(
    doc: &'a ComponentDocumentV2,
    parent: Option<&str>,
) -> Option<&'a Vec<ComponentNode>> {
    let owner = match parent {
        Some(uid) => block(doc, uid)?,
        None => &doc.root,
    };
    owner
        .content
        .iter()
        .find(|n| n.kind.0 == "block_children")
        .map(|n| &n.content)
}
pub fn children_mut<'a>(
    doc: &'a mut ComponentDocumentV2,
    parent: Option<&str>,
) -> Option<&'a mut Vec<ComponentNode>> {
    let owner = match parent {
        Some(uid) => block_mut(doc, uid)?,
        None => &mut doc.root,
    };
    if parent.is_some() && !owner.content.iter().any(|n| n.kind.0 == "block_children") {
        owner
            .content
            .push(ComponentNode::container("block_children", None, Vec::new()));
    }
    owner
        .content
        .iter_mut()
        .find(|n| n.kind.0 == "block_children")
        .map(|n| &mut n.content)
}
pub fn location(doc: &ComponentDocumentV2, uid: &str) -> Option<(Option<Uid>, usize)> {
    fn visit(
        nodes: &[ComponentNode],
        parent: Option<&str>,
        uid: &str,
    ) -> Option<(Option<Uid>, usize)> {
        for (i, node) in nodes.iter().enumerate() {
            let id = node.id.as_ref().map(|id| id.0.as_str());
            if id == Some(uid) {
                return Some((parent.map(str::to_owned), i));
            }
            if let Some(group) = node.content.iter().find(|n| n.kind.0 == "block_children") {
                if let Some(found) = visit(&group.content, id, uid) {
                    return Some(found);
                }
            }
        }
        None
    }
    visit(children(doc, None)?, None, uid)
}
pub fn block_order(doc: &ComponentDocumentV2) -> Vec<Uid> {
    BlockIndex::new(doc).order
}
pub fn inline<'a>(
    doc: &'a ComponentDocumentV2,
    container: &InlineContainerRef,
) -> Option<&'a Vec<ComponentNode>> {
    match container {
        InlineContainerRef::Block(uid) | InlineContainerRef::Caption(uid) => {
            Some(&content(doc, uid)?.content)
        }
        InlineContainerRef::Cell { row, col } => {
            Some(&find(&doc.root, row)?.content.get(*col)?.content)
        }
    }
}
pub fn inline_mut<'a>(
    doc: &'a mut ComponentDocumentV2,
    container: &InlineContainerRef,
) -> Option<&'a mut Vec<ComponentNode>> {
    match container {
        InlineContainerRef::Block(uid) | InlineContainerRef::Caption(uid) => {
            Some(&mut content_mut(doc, uid)?.content)
        }
        InlineContainerRef::Cell { row, col } => {
            Some(&mut find_mut(&mut doc.root, row)?.content.get_mut(*col)?.content)
        }
    }
}
pub fn inline_len(nodes: &[ComponentNode]) -> usize {
    nodes
        .iter()
        .map(|n| n.text.as_ref().map_or(1, |s| s.chars().count()))
        .sum()
}
pub fn inline_text(nodes: &[ComponentNode]) -> String {
    nodes
        .iter()
        .map(|n| {
            n.text.clone().unwrap_or_else(|| {
                if n.kind.0 == "hard_break" {
                    "\n".into()
                } else {
                    "\u{fffc}".into()
                }
            })
        })
        .collect()
}
pub fn paragraph() -> ComponentNode {
    block_node(ComponentNode::new("paragraph"))
}
pub fn block_node(content: ComponentNode) -> ComponentNode {
    ComponentNode::container(
        "block",
        Some(NodeId::new(super::identity::new_uid())),
        vec![content],
    )
}
pub fn document(blocks: Vec<ComponentNode>) -> ComponentDocumentV2 {
    let mut doc = ComponentDocumentV2::new(vec![ComponentNode::container(
        "block_children",
        None,
        blocks,
    )]);
    doc.schema = crate::profiles::block_document::BLOCK_DOCUMENT_SCHEMA.into();
    doc
}
pub fn is_text_kind(kind: &str) -> bool {
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
            | "code_block"
    )
}
pub fn first_caret(doc: &ComponentDocumentV2) -> Option<super::selection::EditorSelection> {
    fn visit(node: &ComponentNode) -> Option<super::selection::EditorSelection> {
        if node.kind.0 == "block"
            && let Some(uid) = &node.id
            && let Some(content) = node.content.first()
        {
            if is_text_kind(&content.kind.0) {
                return Some(super::selection::EditorSelection::caret(
                    InlineContainerRef::Block(uid.0.clone()),
                    0,
                ));
            }
            if content.kind.0 == "image" {
                return Some(super::selection::EditorSelection::caret(
                    InlineContainerRef::Caption(uid.0.clone()),
                    0,
                ));
            }
        }
        node.content.iter().find_map(visit)
    }
    visit(&doc.root)
}
