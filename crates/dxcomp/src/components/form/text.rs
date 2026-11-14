use dioxus::prelude::*;

use super::{attrs::native_attributes, context::use_control};

/// Field-aware native text control with application-owned state, without dxform.
/// Resolves label, description, errors and interaction flags from the nearest Field.
/// The caller owns parsing/validation; value cannot be replaced through attributes.
/// Input/change callbacks are suppressed while disabled or read-only.
#[component]
pub fn TextInput(
    value: String,
    oninput: EventHandler<FormEvent>,
    onchange: Option<EventHandler<FormEvent>>,
    oninvalid: Option<EventHandler<FormEvent>>,
    onselect: Option<EventHandler<SelectionEvent>>,
    onselectionchange: Option<EventHandler<SelectionEvent>>,
    onfocus: Option<EventHandler<FocusEvent>>,
    onblur: Option<EventHandler<FocusEvent>>,
    onfocusin: Option<EventHandler<FocusEvent>>,
    onfocusout: Option<EventHandler<FocusEvent>>,
    onkeydown: Option<EventHandler<KeyboardEvent>>,
    onkeypress: Option<EventHandler<KeyboardEvent>>,
    onkeyup: Option<EventHandler<KeyboardEvent>>,
    oncompositionstart: Option<EventHandler<CompositionEvent>>,
    oncompositionupdate: Option<EventHandler<CompositionEvent>>,
    oncompositionend: Option<EventHandler<CompositionEvent>>,
    oncopy: Option<EventHandler<ClipboardEvent>>,
    oncut: Option<EventHandler<ClipboardEvent>>,
    onpaste: Option<EventHandler<ClipboardEvent>>,
    onmounted: Option<EventHandler<MountedEvent>>,
    #[props(extends = GlobalAttributes)]
    #[props(extends = input)]
    attributes: Vec<Attribute>,
) -> Element {
    let (control, caller) = use_control(attributes, false);
    let editable = !control.disabled && !control.read_only;
    let attributes = native_attributes(&control, caller);
    rsx! {
        input {
            class: "dx-input",
            value,
            oninput: move |event| { if editable { oninput.call(event); } },
            onchange: move |event| { if editable { if let Some(callback) = onchange { callback.call(event); } } },
            oninvalid: move |event| { if let Some(callback) = oninvalid { callback.call(event); } },
            onselect: move |event| { if let Some(callback) = onselect { callback.call(event); } },
            onselectionchange: move |event| { if let Some(callback) = onselectionchange { callback.call(event); } },
            onfocus: move |event| { if let Some(callback) = onfocus { callback.call(event); } },
            onblur: move |event| { if let Some(callback) = onblur { callback.call(event); } },
            onfocusin: move |event| { if let Some(callback) = onfocusin { callback.call(event); } },
            onfocusout: move |event| { if let Some(callback) = onfocusout { callback.call(event); } },
            onkeydown: move |event| { if let Some(callback) = onkeydown { callback.call(event); } },
            onkeypress: move |event| { if let Some(callback) = onkeypress { callback.call(event); } },
            onkeyup: move |event| { if let Some(callback) = onkeyup { callback.call(event); } },
            oncompositionstart: move |event| { if let Some(callback) = oncompositionstart { callback.call(event); } },
            oncompositionupdate: move |event| { if let Some(callback) = oncompositionupdate { callback.call(event); } },
            oncompositionend: move |event| { if let Some(callback) = oncompositionend { callback.call(event); } },
            oncopy: move |event| { if let Some(callback) = oncopy { callback.call(event); } },
            oncut: move |event| { if let Some(callback) = oncut { callback.call(event); } },
            onpaste: move |event| { if let Some(callback) = onpaste { callback.call(event); } },
            onmounted: move |event| { if let Some(callback) = onmounted { callback.call(event); } },
            ..attributes,
        }
    }
}

/// Field-aware native text control with application-owned state, without dxform.
/// Resolves label, description, errors and interaction flags from the nearest Field.
/// The caller owns parsing/validation; value cannot be replaced through attributes.
/// Input/change callbacks are suppressed while disabled or read-only.
#[component]
pub fn Textarea(
    value: String,
    oninput: EventHandler<FormEvent>,
    #[props(default)] variant: crate::TextareaVariant,
    onchange: Option<EventHandler<FormEvent>>,
    oninvalid: Option<EventHandler<FormEvent>>,
    onselect: Option<EventHandler<SelectionEvent>>,
    onselectionchange: Option<EventHandler<SelectionEvent>>,
    onfocus: Option<EventHandler<FocusEvent>>,
    onblur: Option<EventHandler<FocusEvent>>,
    onfocusin: Option<EventHandler<FocusEvent>>,
    onfocusout: Option<EventHandler<FocusEvent>>,
    onkeydown: Option<EventHandler<KeyboardEvent>>,
    onkeypress: Option<EventHandler<KeyboardEvent>>,
    onkeyup: Option<EventHandler<KeyboardEvent>>,
    oncompositionstart: Option<EventHandler<CompositionEvent>>,
    oncompositionupdate: Option<EventHandler<CompositionEvent>>,
    oncompositionend: Option<EventHandler<CompositionEvent>>,
    oncopy: Option<EventHandler<ClipboardEvent>>,
    oncut: Option<EventHandler<ClipboardEvent>>,
    onpaste: Option<EventHandler<ClipboardEvent>>,
    onmounted: Option<EventHandler<MountedEvent>>,
    #[props(extends = GlobalAttributes)]
    #[props(extends = textarea)]
    attributes: Vec<Attribute>,
) -> Element {
    let (control, caller) = use_control(attributes, false);
    let editable = !control.disabled && !control.read_only;
    let attributes = native_attributes(&control, caller);
    rsx! {
        textarea {
            class: "dx-textarea",
            "data-slot": "textarea",
            "data-style": variant.class(),
            value,
            oninput: move |event| { if editable { oninput.call(event); } },
            onchange: move |event| { if editable { if let Some(callback) = onchange { callback.call(event); } } },
            oninvalid: move |event| { if let Some(callback) = oninvalid { callback.call(event); } },
            onselect: move |event| { if let Some(callback) = onselect { callback.call(event); } },
            onselectionchange: move |event| { if let Some(callback) = onselectionchange { callback.call(event); } },
            onfocus: move |event| { if let Some(callback) = onfocus { callback.call(event); } },
            onblur: move |event| { if let Some(callback) = onblur { callback.call(event); } },
            onfocusin: move |event| { if let Some(callback) = onfocusin { callback.call(event); } },
            onfocusout: move |event| { if let Some(callback) = onfocusout { callback.call(event); } },
            onkeydown: move |event| { if let Some(callback) = onkeydown { callback.call(event); } },
            onkeypress: move |event| { if let Some(callback) = onkeypress { callback.call(event); } },
            onkeyup: move |event| { if let Some(callback) = onkeyup { callback.call(event); } },
            oncompositionstart: move |event| { if let Some(callback) = oncompositionstart { callback.call(event); } },
            oncompositionupdate: move |event| { if let Some(callback) = oncompositionupdate { callback.call(event); } },
            oncompositionend: move |event| { if let Some(callback) = oncompositionend { callback.call(event); } },
            oncopy: move |event| { if let Some(callback) = oncopy { callback.call(event); } },
            oncut: move |event| { if let Some(callback) = oncut { callback.call(event); } },
            onpaste: move |event| { if let Some(callback) = onpaste { callback.call(event); } },
            onmounted: move |event| { if let Some(callback) = onmounted { callback.call(event); } },
            ..attributes,
        }
    }
}
