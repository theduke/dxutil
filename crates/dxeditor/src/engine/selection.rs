//! Model positions use Unicode scalar/atom units, never DOM UTF-16 offsets.
use serde::{Deserialize, Serialize};

pub type Uid = String;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum InlineContainerRef {
    Block(Uid),
    Cell { row: Uid, col: usize },
    Caption(Uid),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextPoint {
    pub container: InlineContainerRef,
    pub offset: usize,
}
impl TextPoint {
    pub fn new(container: InlineContainerRef, offset: usize) -> Self {
        Self { container, offset }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextRange {
    pub anchor: TextPoint,
    pub head: TextPoint,
}
impl TextRange {
    pub fn caret(point: TextPoint) -> Self {
        Self {
            anchor: point.clone(),
            head: point,
        }
    }
    pub fn is_collapsed(&self) -> bool {
        self.anchor == self.head
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorSelection {
    Text(TextRange),
    Blocks {
        anchor: Uid,
        head: Uid,
    },
    Node(Uid),
    Cells {
        table: Uid,
        anchor: (usize, usize),
        head: (usize, usize),
    },
}
impl EditorSelection {
    pub fn caret(container: InlineContainerRef, offset: usize) -> Self {
        Self::Text(TextRange::caret(TextPoint::new(container, offset)))
    }
    pub fn text_range(&self) -> Option<&TextRange> {
        if let Self::Text(range) = self {
            Some(range)
        } else {
            None
        }
    }
    pub fn is_collapsed(&self) -> bool {
        matches!(self, Self::Text(range) if range.is_collapsed())
    }
}

impl EditorSelection {
    pub(crate) fn remap_ids(&mut self, ids: &std::collections::BTreeMap<String, String>) {
        fn remap(uid: &mut String, ids: &std::collections::BTreeMap<String, String>) {
            if let Some(next) = ids.get(uid) {
                *uid = next.clone();
            }
        }
        fn point(point: &mut TextPoint, ids: &std::collections::BTreeMap<String, String>) {
            match &mut point.container {
                InlineContainerRef::Block(uid) | InlineContainerRef::Caption(uid) => {
                    remap(uid, ids)
                }
                InlineContainerRef::Cell { row, .. } => remap(row, ids),
            }
        }
        match self {
            Self::Text(range) => {
                point(&mut range.anchor, ids);
                point(&mut range.head, ids)
            }
            Self::Node(uid) => remap(uid, ids),
            Self::Blocks { anchor, head } => {
                remap(anchor, ids);
                remap(head, ids)
            }
            Self::Cells { table, .. } => remap(table, ids),
        }
    }
}
