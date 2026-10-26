use dioxus::prelude::*;
use dxform::FieldHandle;

use super::{native_attributes, use_bound_control};

/// String-draft input for any committed value type, including foreign application types.
/// Input preserves invalid text; focus/blur and change validation follow the field strategy.
/// Caller event handlers run after binding. Prefer text + inputmode over type=number when
/// preserving partial numeric text matters. Native has input events but no focus/blur events.
#[component]
pub fn FieldInput<V, R>(
    field: FieldHandle<V, R, String>,
    oninput: Option<EventHandler<FormEvent>>,
    onchange: Option<EventHandler<FormEvent>>,
    onfocus: Option<EventHandler<FocusEvent>>,
    onblur: Option<EventHandler<FocusEvent>>,
    onkeydown: Option<EventHandler<KeyboardEvent>>,
    onmounted: Option<EventHandler<MountedEvent>>,
    #[props(extends = GlobalAttributes)]
    #[props(extends = input)]
    attributes: Vec<Attribute>,
) -> Element
where
    V: Clone + PartialEq + 'static,
    R: Clone + PartialEq + 'static,
{
    let (control, caller) = use_bound_control(&field, attributes);
    let attributes = native_attributes(&control, caller);
    let value = field.draft();
    let input = field.clone();
    let focus = field.clone();
    rsx! {
        input {
            class: "dx-input",
            value,
            oninput: move |event: FormEvent| {
                if !control.disabled && !control.read_only { input.handle_draft_change(event.value()); }
                if let Some(callback) = oninput { callback.call(event); }
            },
            onchange: move |event| { if let Some(callback) = onchange { callback.call(event); } },
            onfocus: move |event| {
                focus.handle_focus();
                if let Some(callback) = onfocus { callback.call(event); }
            },
            onblur: move |event| {
                field.handle_blur();
                if let Some(callback) = onblur { callback.call(event); }
            },
            onkeydown: move |event| { if let Some(callback) = onkeydown { callback.call(event); } },
            onmounted: move |event| { if let Some(callback) = onmounted { callback.call(event); } },
            ..attributes,
        }
    }
}

/// String-draft textarea for any committed value type, including foreign application types.
/// Input preserves invalid text; focus/blur and change validation follow the field strategy.
/// Caller event handlers run after binding. Prefer text + inputmode over type=number when
/// preserving partial numeric text matters. Native has input events but no focus/blur events.
#[component]
pub fn FieldTextarea<V, R>(
    field: FieldHandle<V, R, String>,
    #[props(default)] variant: crate::TextareaVariant,
    oninput: Option<EventHandler<FormEvent>>,
    onchange: Option<EventHandler<FormEvent>>,
    onfocus: Option<EventHandler<FocusEvent>>,
    onblur: Option<EventHandler<FocusEvent>>,
    onkeydown: Option<EventHandler<KeyboardEvent>>,
    onmounted: Option<EventHandler<MountedEvent>>,
    #[props(extends = GlobalAttributes)]
    #[props(extends = textarea)]
    attributes: Vec<Attribute>,
) -> Element
where
    V: Clone + PartialEq + 'static,
    R: Clone + PartialEq + 'static,
{
    let (control, caller) = use_bound_control(&field, attributes);
    let attributes = native_attributes(&control, caller);
    let value = field.draft();
    let input = field.clone();
    let focus = field.clone();
    rsx! {
        textarea {
            class: "dx-textarea",
            "data-slot": "textarea",
            "data-style": variant.class(),
            value,
            oninput: move |event: FormEvent| {
                if !control.disabled && !control.read_only { input.handle_draft_change(event.value()); }
                if let Some(callback) = oninput { callback.call(event); }
            },
            onchange: move |event| { if let Some(callback) = onchange { callback.call(event); } },
            onfocus: move |event| {
                focus.handle_focus();
                if let Some(callback) = onfocus { callback.call(event); }
            },
            onblur: move |event| {
                field.handle_blur();
                if let Some(callback) = onblur { callback.call(event); }
            },
            onkeydown: move |event| { if let Some(callback) = onkeydown { callback.call(event); } },
            onmounted: move |event| { if let Some(callback) = onmounted { callback.call(event); } },
            ..attributes,
        }
    }
}

/// Password draft binding with the shared presentation toggle. Caller input/focus/blur
/// handlers run after the binding; visibility can be controlled independently.
#[component]
pub fn FieldPasswordInput<V, R>(
    field: FieldHandle<V, R, String>,
    #[props(default)] visible: Option<bool>,
    #[props(default)] on_visible_change: Option<EventHandler<bool>>,
    #[props(default = "Show password".to_string())] toggle_label: String,
    oninput: Option<EventHandler<FormEvent>>,
    onfocus: Option<EventHandler<FocusEvent>>,
    onblur: Option<EventHandler<FocusEvent>>,
    #[props(extends = GlobalAttributes)]
    #[props(extends = input)]
    attributes: Vec<Attribute>,
) -> Element
where
    V: Clone + PartialEq + 'static,
    R: Clone + PartialEq + 'static,
{
    let (control, caller) = use_bound_control(&field, attributes);
    let mut attributes = native_attributes(&control, caller);
    // PasswordInput resolves the same context again. Preserve an explicit false override
    // until that resolver consumes it; the rendered control still omits false ARIA flags.
    if !control.invalid {
        attributes.push(super::super::attrs::attr("aria-invalid", "false"));
    }
    let value = field.draft();
    let input = field.clone();
    let focus = field.clone();
    rsx! {
        super::super::PasswordInput {
            value, visible, on_visible_change, toggle_label, attributes,
            oninput: move |event: FormEvent| {
                if !control.disabled && !control.read_only { input.handle_draft_change(event.value()); }
                if let Some(callback) = oninput { callback.call(event); }
            },
            onfocus: move |event| {
                focus.handle_focus();
                if let Some(callback) = onfocus { callback.call(event); }
            },
            onblur: move |event| {
                field.handle_blur();
                if let Some(callback) = onblur { callback.call(event); }
            },
        }
    }
}
