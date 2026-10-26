use dioxus::prelude::*;
use dioxus_icons::lucide::{Eye, EyeOff};

use super::context::use_control;

/// Controlled text with an optional controlled visibility toggle (initially hidden).
/// The toggle's constant label and `aria-pressed` communicate its state. Read-only
/// inputs still allow toggling visibility. Pass autocomplete through input attributes.
#[component]
pub fn PasswordInput(
    value: String,
    oninput: EventHandler<FormEvent>,
    onfocus: Option<EventHandler<FocusEvent>>,
    onblur: Option<EventHandler<FocusEvent>>,
    #[props(default)] visible: Option<bool>,
    #[props(default)] on_visible_change: Option<EventHandler<bool>>,
    #[props(default = "Show password".to_string())] toggle_label: String,
    #[props(extends = GlobalAttributes)]
    #[props(extends = input)]
    attributes: Vec<Attribute>,
) -> Element {
    let mut local_visible = use_signal(|| false);
    let visible = visible.unwrap_or(local_visible());
    let (control, caller) = use_control(attributes, false);
    let aria = control.aria_attributes();
    // Visibility owns the type; a caller cannot accidentally turn this into another control.
    let rest: Vec<_> = caller
        .rest
        .into_iter()
        .filter(|attribute| {
            attribute.namespace.is_some() || !matches!(attribute.name, "type" | "value")
        })
        .collect();
    rsx! {
        div { class: "dx-password-input",
            crate::Input {
                r#type: if visible { "text" } else { "password" },
                value,
                name: caller.name,
                disabled: control.disabled,
                required: control.required,
                readonly: control.read_only,
                attributes: [aria, rest].concat(),
                oninput,
                onfocus,
                onblur,
            }
            button {
                r#type: "button",
                class: "dx-button dx-password-toggle",
                "data-style": "ghost",
                "data-size": "icon-sm",
                aria_controls: control.id,
                aria_pressed: visible.to_string(),
                aria_label: toggle_label,
                disabled: control.disabled,
                onclick: move |_| {
                    local_visible.set(!visible);
                    if let Some(callback) = on_visible_change { callback.call(!visible); }
                },
                span { aria_hidden: "true",
                    if visible { EyeOff { size: "18" } } else { Eye { size: "18" } }
                }
            }
        }
    }
}
