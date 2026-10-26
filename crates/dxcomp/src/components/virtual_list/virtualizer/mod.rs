// Adapted from DioxusLabs/dioxus-components at 02801f27e4b3e30606e5c77e435c9d955c708eb3.
// MIT / Apache-2.0; see LICENSE-MIT and LICENSE-APACHE in this directory.
//! Virtual list implementation using Dioxus Store for fine-grained reactivity.
//!
//! This module provides the core algorithms needed for efficient list virtualization:
//!
//! - Computing item positions from measured or estimated sizes
//! - Calculating the visible range using binary search
//! - Handling scroll position corrections when items resize

mod engine;
pub(crate) mod types;
mod utils;

pub(crate) use engine::{
    VirtualizerState, VirtualizerStateStoreExt, compute_measurements, get_total_size,
    get_virtual_items, resize_item, set_scroll_offset, set_viewport_size,
};
