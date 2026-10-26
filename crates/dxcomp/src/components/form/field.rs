use dioxus::prelude::*;

use super::{FieldLabelling, FieldState, use_field_context, use_field_ids, use_field_provider};

/// Placement of the label relative to the control.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FieldLayout {
    /// Label, control, description, then errors.
    #[default]
    Stacked,
    /// Control and label on a row, followed by description and errors.
    Inline,
}

/// Field shell usable with manual state or custom editors, independently of dxform.
/// For a rich label in children, set `labelling: FieldLabelling::Native` explicitly.
#[component]
pub fn Field(
    #[props(default)] id: Option<String>,
    #[props(default)] label: Option<String>,
    #[props(default)] description: Option<String>,
    #[props(default)] errors: Vec<String>,
    #[props(default)] required: bool,
    #[props(default)] disabled: bool,
    #[props(default)] read_only: bool,
    #[props(default)] layout: FieldLayout,
    #[props(default)] labelling: Option<FieldLabelling>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let state = FieldState {
        ids: use_field_ids(id),
        labelling: labelling.unwrap_or(if label.is_some() {
            FieldLabelling::Native
        } else {
            FieldLabelling::None
        }),
        has_description: description.is_some(),
        has_errors: !errors.is_empty(),
        required,
        disabled,
        read_only,
    };
    let invalid = state.has_errors;
    use_field_provider(state);
    rsx! {
        div {
            class: "dx-field",
            "data-layout": if layout == FieldLayout::Inline { "inline" } else { "stacked" },
            "data-invalid": invalid,
            "data-disabled": disabled,
            "data-required": required,
            ..attributes,
            if layout == FieldLayout::Inline {
                div { class: "dx-field-row",
                    {children}
                    if let Some(label) = label { FieldLabel { "{label}" } }
                }
            } else {
                if let Some(label) = label { FieldLabel { "{label}" } }
                {children}
            }
            if let Some(description) = description { FieldDescription { "{description}" } }
            FieldErrorList { messages: errors }
        }
    }
}

/// Native label linked to the context control, including an accessible required marker.
/// Outside a field context this renders a plain label.
#[component]
pub fn FieldLabel(
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let state = use_field_context().map(|context| context.state());
    rsx! {
        label {
            class: "dx-label dx-field-label",
            id: state.as_ref().map(|s| s.ids.label.clone()),
            r#for: state.as_ref().map(|s| s.ids.control.clone()),
            ..attributes,
            {children}
            if state.as_ref().is_some_and(|s| s.required) {
                span { class: "dx-field-required", aria_hidden: "true", "*" }
            }
        }
    }
}

/// Description at the current field's description ID.
#[component]
pub fn FieldDescription(
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let id = use_field_context().map(|context| context.state().ids.description);
    rsx! { p { class: "dx-field-description", id, ..attributes, {children} } }
}

/// Always-mounted polite error live region. The field references it only when errors are visible.
#[component]
pub fn FieldErrorList(
    messages: Vec<String>,
    #[props(default)] id: Option<String>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let context = use_field_context();
    let id = id.or_else(|| context.map(|context| context.state().ids.errors));
    rsx! {
        div { class: "dx-field-errors", id, aria_live: "polite", ..attributes,
            if !messages.is_empty() {
                ul { for message in messages { li { "{message}" } } }
            }
        }
    }
}
