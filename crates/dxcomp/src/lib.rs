//! Prestyled Dioxus component wrappers ported from `DioxusLabs/dioxus-components`.
//!
//! The component modules expose the generated component API that `dx component
//! add` would place in an app, bundled here as a reusable crate.

use dioxus::prelude::*;

pub mod components;
mod floating;

pub use components::*;
pub use dioxus_primitives as primitives;

/// The bundled CSS used by the generated Dioxus components.
///
/// Apps should render [`Stylesheet`] once near the root to install the shared
/// variables, base styles, and component styles.
pub const CSS: &str = include_str!("../assets/dxcomp.css");

/// The shared theme CSS used by the generated Dioxus components.
#[deprecated(note = "use CSS instead")]
pub const THEME_CSS: &str = CSS;

/// Render the bundled shared stylesheet required by the prestyled components.
#[component]
pub fn Stylesheet() -> Element {
    rsx! {
        style { {CSS} }
    }
}

/// Render the bundled shared theme stylesheet.
#[component]
pub fn ThemeStylesheet() -> Element {
    rsx! {
        Stylesheet {}
    }
}
