use dioxus::prelude::*;
use dxform::FieldHandle;

use super::super::{FieldLabelling, merge_idrefs, use_field_context};
use super::{native_attributes, use_bound_control};

/// Typed select binding with accessible attributes applied to the focusable trigger.
/// Requires web or a desktop webview for floating positioning; Native is unsupported.
/// Optional values use `T = Option<X>` and an explicit `None` option.
/// Set each option's `text_value` when `T` is not a string type.
#[component]
pub fn FieldSelect<T, R>(
    field: FieldHandle<T, R>,
    #[props(default)] placeholder: Option<String>,
    onfocus: Option<EventHandler<FocusEvent>>,
    onblur: Option<EventHandler<FocusEvent>>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element
where
    T: Clone + PartialEq + 'static,
    R: Clone + PartialEq + 'static,
{
    let (mut control, caller) = use_bound_control(&field, attributes);
    let state = use_field_context().map(|context| context.state());
    if caller.labelledby.is_none() {
        if let Some(state) = state {
            if state.labelling != FieldLabelling::None {
                control.labelledby = merge_idrefs([state.ids.label, control.id.clone()]);
            }
        }
    }
    let value = use_memo(use_reactive((&field,), |(field,)| Some(field.value())));
    let mut open = use_signal(|| false);
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
        if !open() {
            blur.handle_blur();
        }
        if let Some(callback) = onblur {
            callback.call(event);
        }
    }));
    attributes.extend(caller.rest);
    let floating = crate::floating::use_floating(".dx-select-trigger", ".dx-select-list");
    rsx! {
        dioxus_primitives::select::Select::<T> {
            value: Some(ReadSignal::from(value)),
            disabled: control.disabled,
            name: caller.name.unwrap_or_default(),
            on_open_change: move |value| open.set(value),
            on_value_change: move |value: Option<T>| {
                if !control.disabled && !control.read_only {
                    if let Some(value) = value {
                        field.handle_value_change(value);
                    }
                }
            },
            class: "dx-select",
            "data-dx-floating": floating,
            crate::select::SelectContent { trigger_attributes: attributes, placeholder, children }
        }
    }
}

/// One typed option in a native select, represented in HTML by its index.
#[derive(Clone, Debug, PartialEq)]
pub struct SelectChoice<T> {
    /// Value committed when selected.
    pub value: T,
    /// Human-readable option text.
    pub label: String,
    /// Whether selection is disabled.
    pub disabled: bool,
}

impl<T> SelectChoice<T> {
    /// An enabled choice.
    pub fn new(value: T, label: impl Into<String>) -> Self {
        Self {
            value,
            label: label.into(),
            disabled: false,
        }
    }

    /// Set whether this choice can be selected.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// Native select without JavaScript. Unsupported on Native/Blitz (no select widget).
/// Unmatched values show a disabled placeholder instead of silently selecting option zero.
/// Read-only ignores changes and keeps the controlled selection.
#[component]
pub fn FieldNativeSelect<T, R>(
    field: FieldHandle<T, R>,
    options: Vec<SelectChoice<T>>,
    #[props(default)] placeholder: Option<String>,
    onfocus: Option<EventHandler<FocusEvent>>,
    onblur: Option<EventHandler<FocusEvent>>,
    #[props(extends = GlobalAttributes)]
    #[props(extends = select)]
    attributes: Vec<Attribute>,
) -> Element
where
    T: Clone + PartialEq + 'static,
    R: Clone + PartialEq + 'static,
{
    let (control, caller) = use_bound_control(&field, attributes);
    let attributes = native_attributes(&control, caller);
    let current = field.value();
    let selected = options.iter().position(|choice| choice.value == current);
    let value = selected.map(|index| index.to_string()).unwrap_or_default();
    let choices = options.clone();
    let rerender = dioxus::core::schedule_update();
    let input = field.clone();
    let focus = field.clone();
    rsx! {
        select {
            class: "dx-native-select",
            value,
            oninput: move |event: FormEvent| {
                if control.read_only {
                    // A select has no native readonly flag. Reapply its controlled value.
                    rerender();
                    return;
                }
                if !control.disabled {
                    if let Ok(index) = event.value().parse::<usize>() {
                        if let Some(choice) = choices.get(index).filter(|choice| !choice.disabled) {
                            input.handle_value_change(choice.value.clone());
                        }
                    }
                }
            },
            onfocus: move |event| {
                focus.handle_focus();
                if let Some(callback) = onfocus { callback.call(event); }
            },
            onblur: move |event| {
                field.handle_blur();
                if let Some(callback) = onblur { callback.call(event); }
            },
            ..attributes,
            if selected.is_none() {
                option { value: "", disabled: true, selected: true, "{placeholder.as_deref().unwrap_or_default()}" }
            }
            for (index, choice) in options.into_iter().enumerate() {
                option {
                    key: "{index}",
                    value: "{index}",
                    selected: selected == Some(index),
                    disabled: choice.disabled,
                    "{choice.label}"
                }
            }
        }
    }
}
