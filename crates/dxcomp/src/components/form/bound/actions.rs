use dioxus::prelude::*;
use dxform::FormRoot;

use super::super::SubmitButton;
use super::super::attrs::{attr, take_control_attrs};
use crate::{ButtonSize, ButtonVariant};

/// Submit button bound to form pending state. With `submit_on_click`, uses type=button
/// and calls submit directly for Native or outside a form. Do not also attach an onsubmit
/// handler that submits the same form. Native does not dispatch Dioxus submit events.
#[component]
pub fn FormSubmitButton<T>(
    form: FormRoot<T>,
    #[props(default)] pending_label: Option<String>,
    #[props(default)] submit_on_click: bool,
    #[props(default)] variant: ButtonVariant,
    #[props(default)] size: ButtonSize,
    #[props(extends = GlobalAttributes)]
    #[props(extends = button)]
    attributes: Vec<Attribute>,
    children: Element,
) -> Element
where
    T: Clone + PartialEq + 'static,
{
    let pending = form.meta().submitting;
    let mut attributes = attributes;
    if submit_on_click {
        attributes.retain(|attribute| attribute.name != "type" || attribute.namespace.is_some());
        attributes.push(attr("type", "button"));
    }
    rsx! {
        SubmitButton {
            pending, pending_label, variant, size, attributes,
            onclick: move |_| {
                if submit_on_click && !form.meta().submitting {
                    let form = form.clone();
                    spawn(async move { let _ = form.submit().await; });
                }
            },
            {children}
        }
    }
}

/// Reset action disabled during submission and optionally while clean.
#[component]
pub fn FormResetButton<T>(
    form: FormRoot<T>,
    #[props(default)] disabled_when_clean: bool,
    #[props(default = ButtonVariant::Outline)] variant: ButtonVariant,
    #[props(default)] size: ButtonSize,
    #[props(extends = GlobalAttributes)]
    #[props(extends = button)]
    attributes: Vec<Attribute>,
    children: Element,
) -> Element
where
    T: Clone + PartialEq + 'static,
{
    let meta = form.meta();
    let caller = take_control_attrs(attributes);
    let disabled = caller.disabled || meta.submitting || (disabled_when_clean && !meta.dirty);
    let mut attributes = caller.rest;
    if let Some(id) = caller.id {
        attributes.push(attr("id", id));
    }
    if let Some(name) = caller.name {
        attributes.push(attr("name", name));
    }
    if let Some(value) = caller.labelledby {
        attributes.push(attr("aria-labelledby", value));
    }
    if let Some(value) = caller.describedby {
        attributes.push(attr("aria-describedby", value));
    }
    attributes.retain(|attribute| attribute.name != "type" || attribute.namespace.is_some());
    attributes.push(attr("type", "button"));
    attributes.push(attr("disabled", disabled));
    rsx! {
        crate::Button {
            variant, size, attributes,
            onclick: move |_| { if !disabled { form.reset(); } },
            {children}
        }
    }
}
