use dioxus::prelude::*;
use dioxus_primitives::{dioxus_attributes::attributes, merge_attributes};

/// An inline loading circle. The label is announced to assistive technology.
#[component]
pub fn Spinner(
    #[props(default = "Loading".to_string())] label: String,
    #[props(default = "20".to_string())] size: String,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let base = attributes!(span {
        class: "dx-spinner",
        role: "status",
        aria_label: label,
        aria_busy: "true",
    });
    let merged = merge_attributes(vec![base, attributes]);

    rsx! {
        span { ..merged,
            span { aria_hidden: "true",
                dioxus_icons::lucide::LoaderCircle { class: "dx-spinner__circle", size }
            }
        }
    }
}

/// A loading circle centered horizontally and vertically in the available space.
/// Grows to fill a flex parent, with a minimum height for unconstrained containers.
#[component]
pub fn BlockSpinner(
    #[props(default = "Loading".to_string())] label: String,
    #[props(default = "32".to_string())] size: String,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let base = attributes!(div {
        class: "dx-block-spinner",
    });
    let merged = merge_attributes(vec![base, attributes]);

    rsx! {
        div { ..merged,
            Spinner { label, size }
        }
    }
}
