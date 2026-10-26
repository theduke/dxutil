use super::transaction::{HistoryMode, Transaction, TxKind};
use std::collections::VecDeque;

#[derive(Clone, Debug)]
struct Entry {
    transaction: Transaction,
    timestamp_ms: u64,
}
#[derive(Clone, Debug)]
pub struct History {
    undo: VecDeque<Entry>,
    redo: Vec<Entry>,
    depth: usize,
    group_ms: u64,
    group_closed: bool,
}
impl History {
    pub fn new(depth: usize, group_ms: u64) -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
            depth,
            group_ms,
            group_closed: false,
        }
    }
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.group_closed = true;
    }
    pub fn close_group(&mut self) {
        self.group_closed = true;
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn record(&mut self, tx: Transaction, timestamp_ms: u64) {
        if tx.ops.is_empty() || tx.history == HistoryMode::Skip {
            return;
        }
        self.redo.clear();
        let can_group = !self.group_closed
            && self.undo.back().is_some_and(|entry| {
                matches!(tx.kind, TxKind::Typing | TxKind::Deleting)
                    && entry.transaction.kind == tx.kind
                    && timestamp_ms.saturating_sub(entry.timestamp_ms) <= self.group_ms
                    && entry.transaction.selection_after == tx.selection_before
                    && same_container(&tx)
                    && same_container(&entry.transaction)
            });
        if (can_group || tx.history == HistoryMode::Merge)
            && let Some(entry) = self.undo.back_mut()
        {
            for op in tx.ops {
                match (entry.transaction.ops.last_mut(), &op) {
                    (
                        Some(super::transaction::Op::SetContent { uid, after, .. }),
                        super::transaction::Op::SetContent {
                            uid: next,
                            before,
                            after: replacement,
                        },
                    ) if uid == next && after == before => *after = replacement.clone(),
                    (
                        Some(super::transaction::Op::SetBlockAttrs { uid, after, .. }),
                        super::transaction::Op::SetBlockAttrs {
                            uid: next,
                            before,
                            after: replacement,
                        },
                    ) if uid == next && after == before => *after = replacement.clone(),
                    _ => entry.transaction.ops.push(op),
                }
            }
            entry.transaction.selection_after = tx.selection_after;
            entry.timestamp_ms = timestamp_ms;
        } else if self.depth > 0 {
            self.undo.push_back(Entry {
                transaction: tx,
                timestamp_ms,
            });
            while self.undo.len() > self.depth {
                self.undo.pop_front();
            }
        }
        self.group_closed = false;
    }
    /// Returns the inverse transaction. Its successful application is acknowledged by `did_undo`.
    pub fn undo_transaction(&self) -> Option<Transaction> {
        self.undo.back().map(|entry| entry.transaction.invert())
    }
    pub fn redo_transaction(&self) -> Option<Transaction> {
        self.redo.last().map(|entry| entry.transaction.clone())
    }
    pub fn did_undo(&mut self) {
        if let Some(entry) = self.undo.pop_back() {
            self.redo.push(entry);
        }
        self.group_closed = true;
    }
    pub fn did_redo(&mut self) {
        if let Some(entry) = self.redo.pop() {
            self.undo.push_back(entry);
        }
        self.group_closed = true;
    }
}
fn same_container(tx: &Transaction) -> bool {
    matches!((&tx.selection_before,&tx.selection_after),(super::selection::EditorSelection::Text(a),super::selection::EditorSelection::Text(b)) if a.is_collapsed() && b.is_collapsed() && a.head.container==b.head.container)
}
