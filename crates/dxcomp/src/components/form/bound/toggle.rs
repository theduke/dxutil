use dioxus::prelude::*;
use dxform::FieldHandle;

use super::{native_attributes, use_bound_control};

/// Typed boolean binding for the styled checkbox. Read-only controls remain
/// focusable and ignore changes. Labels target the primitive's labelable button.
#[component]
pub fn FieldCheckbox<R>(
    field: FieldHandle<bool, R>,
    onfocus: Option<EventHandler<FocusEvent>>,
    onblur: Option<EventHandler<FocusEvent>>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element
where
    R: Clone + PartialEq + 'static,
{
    let (control, caller) = use_bound_control(&field, attributes);
    let checked = use_memo(use_reactive((&field,), |(field,)| {
        Some(if field.value() {
            dioxus_primitives::checkbox::CheckboxState::Checked
        } else {
            dioxus_primitives::checkbox::CheckboxState::Unchecked
        })
    }));
    let focus = field.clone();
    let blur = field.clone();
    let mut attributes = control.aria_attributes();
    attributes.push(dioxus::html::events::onfocus(move |event| {
        focus.handle_focus();
        if let Some(callback) = onfocus {
            callback.call(event);
        }
    }));
    attributes.push(dioxus::html::events::onblur(move |event| {
        blur.handle_blur();
        if let Some(callback) = onblur {
            callback.call(event);
        }
    }));
    attributes.extend(caller.rest);
    rsx! {
        crate::Checkbox {
            checked,
            disabled: control.disabled,
            required: control.required,
            name: caller.name.unwrap_or_default(),
            attributes,
            on_checked_change: move |value: dioxus_primitives::checkbox::CheckboxState| {
                if !control.read_only && !control.disabled { field.handle_value_change(value == dioxus_primitives::checkbox::CheckboxState::Checked); }
            },
        }
    }
}

/// Typed boolean binding for the styled switch. Read-only controls remain
/// focusable and ignore changes. Labels target the primitive's labelable button.
#[component]
pub fn FieldSwitch<R>(
    field: FieldHandle<bool, R>,
    onfocus: Option<EventHandler<FocusEvent>>,
    onblur: Option<EventHandler<FocusEvent>>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element
where
    R: Clone + PartialEq + 'static,
{
    let (control, caller) = use_bound_control(&field, attributes);
    let checked = use_memo(use_reactive((&field,), |(field,)| Some(field.value())));
    let focus = field.clone();
    let blur = field.clone();
    let mut attributes = control.aria_attributes();
    attributes.push(dioxus::html::events::onfocus(move |event| {
        focus.handle_focus();
        if let Some(callback) = onfocus {
            callback.call(event);
        }
    }));
    attributes.push(dioxus::html::events::onblur(move |event| {
        blur.handle_blur();
        if let Some(callback) = onblur {
            callback.call(event);
        }
    }));
    attributes.extend(caller.rest);
    rsx! {
        crate::Switch {
            checked,
            disabled: control.disabled,
            required: control.required,
            name: caller.name.unwrap_or_default(),
            attributes,
            on_checked_change: move |value: bool| {
                if !control.read_only && !control.disabled { field.handle_value_change(value); }
            },
        }
    }
}

/// Native checkbox using input events (also emitted by Native/Blitz).
/// Read-only clicks are prevented because native checkboxes ignore `readonly`.
#[component]
pub fn FieldNativeCheckbox<R>(
    field: FieldHandle<bool, R>,
    onfocus: Option<EventHandler<FocusEvent>>,
    onblur: Option<EventHandler<FocusEvent>>,
    #[props(extends = GlobalAttributes)]
    #[props(extends = input)]
    attributes: Vec<Attribute>,
) -> Element
where
    R: Clone + PartialEq + 'static,
{
    let (control, caller) = use_bound_control(&field, attributes);
    let mut attributes = native_attributes(&control, caller);
    attributes.retain(|attribute| attribute.name != "type" || attribute.namespace.is_some());
    let checked = field.value();
    let input = field.clone();
    let focus = field.clone();
    rsx! {
        input {
            class: "dx-native-checkbox",
            r#type: "checkbox",
            checked,
            oninput: move |event: FormEvent| {
                if !control.disabled && !control.read_only { input.handle_value_change(event.checked()); }
            },
            onclick: move |event| { if control.read_only { event.prevent_default(); } },
            onfocus: move |event| {
                focus.handle_focus();
                if let Some(callback) = onfocus { callback.call(event); }
            },
            onblur: move |event| {
                field.handle_blur();
                if let Some(callback) = onblur { callback.call(event); }
            },
            ..attributes,
        }
    }
}
