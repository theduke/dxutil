//! Shared IDs, monotonic clocks, and browser event listeners for Dioxus.
//!
//! IDs and clocks work on native targets. On wasm32, enable `web` for the
//! browser clock and RAII event listeners.

pub mod id;
pub mod time;
#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub mod web;
