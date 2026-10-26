use dioxus::prelude::*;

use super::attrs::{attr, take_control_attrs};
use crate::{ButtonSize, ButtonVariant};

/// Submit action with a busy state and the shared Spinner. Keep application status
/// announcements in a separate live region; the decorative spinner is hidden from AT.
#[component]
pub fn SubmitButton(
    #[props(default)] pending: bool,
    #[props(default)] pending_label: Option<String>,
    #[props(default)] variant: ButtonVariant,
    #[props(default)] size: ButtonSize,
    onclick: Option<EventHandler<MouseEvent>>,
    #[props(extends = GlobalAttributes)]
    #[props(extends = button)]
    attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let caller = take_control_attrs(attributes);
    let mut attributes = vec![attr("type", "submit")];
    if let Some(id) = caller.id {
        attributes.push(attr("id", id));
    }
    if let Some(name) = caller.name {
        attributes.push(attr("name", name));
    }
    if let Some(labelledby) = caller.labelledby {
        attributes.push(attr("aria-labelledby", labelledby));
    }
    if let Some(describedby) = caller.describedby {
        attributes.push(attr("aria-describedby", describedby));
    }
    attributes.extend(caller.rest);
    attributes.push(attr("disabled", pending || caller.disabled));
    attributes.push(attr("aria-busy", pending.to_string()));
    rsx! {
        crate::Button { variant, size, attributes, onclick,
            if pending {
                crate::Spinner { size: "16", aria_hidden: "true" }
                if let Some(label) = pending_label { "{label}" } else { {children} }
            } else {
                {children}
            }
        }
    }
}
