// Adapted from DioxusLabs/dioxus-components at 02801f27e4b3e30606e5c77e435c9d955c708eb3.
// MIT / Apache-2.0; see LICENSE-MIT and LICENSE-APACHE in this directory.
//! Core types for the virtual list implementation.

/// A unique key for identifying items in the virtualizer.
pub(crate) type Key = usize;

/// A single virtualized item with computed position.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VirtualItem {
    key: Key,
    index: usize,
    start: u32,
    size: u32,
}

impl VirtualItem {
    pub(crate) fn new(key: Key, index: usize, start: u32, size: u32) -> Self {
        Self {
            key,
            index,
            start,
            size,
        }
    }

    pub(crate) fn key(&self) -> Key {
        self.key
    }

    pub(crate) fn index(&self) -> usize {
        self.index
    }

    pub(crate) fn start(&self) -> u32 {
        self.start
    }

    pub(crate) fn end(&self) -> u32 {
        self.start + self.size
    }

    pub(crate) fn size(&self) -> u32 {
        self.size
    }
}
