use dioxus::prelude::*;

use super::form::attrs::{attr, take_control_attrs};
use crate::{ButtonSize, ButtonVariant};

/// Controlled action with shared pending feedback. Defaults to `type="button"`.
/// The caller owns the operation and its errors; the spinner is decorative.
/// Announce application outcomes in a separate live region.
#[component]
pub fn ActionButton(
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
    rsx! {
        PendingButton { default_type: "button", pending, pending_label, variant, size, onclick, attributes, {children} }
    }
}

#[component]
pub(crate) fn PendingButton(
    default_type: &'static str,
    pending: bool,
    pending_label: Option<String>,
    variant: ButtonVariant,
    size: ButtonSize,
    onclick: Option<EventHandler<MouseEvent>>,
    attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let caller = take_control_attrs(attributes);
    let disabled = pending || caller.disabled;
    let mut attributes = vec![attr("type", default_type)];
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
    attributes.push(attr("disabled", disabled));
    attributes.push(attr("aria-busy", pending.to_string()));
    rsx! {
        crate::Button { variant, size, attributes,
            onclick: move |event: MouseEvent| {
                if disabled { event.prevent_default(); return; }
                if let Some(callback) = onclick { callback.call(event); }
            },
            if pending {
                crate::Spinner { size: "16", aria_hidden: "true" }
                if let Some(label) = pending_label { "{label}" } else { {children} }
            } else {
                {children}
            }
        }
    }
}
