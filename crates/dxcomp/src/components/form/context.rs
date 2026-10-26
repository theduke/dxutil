use dioxus::prelude::*;

use super::attrs::{CallerControlAttrs, attr, take_control_attrs};
use super::{FieldIds, merge_idrefs};

/// How a field's visible label names its control.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FieldLabelling {
    /// A native `<label for>` names the control.
    #[default]
    Native,
    /// A custom element with the label ID names the control through ARIA.
    LabelledBy,
    /// The control supplies its own accessible name, such as `aria-label`.
    None,
}

/// Presentation state shared by a field's descendants.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldState {
    /// IDs for the control and supporting content.
    pub ids: FieldIds,
    /// How the visible label names the control.
    pub labelling: FieldLabelling,
    /// Whether description content is rendered.
    pub has_description: bool,
    /// Whether visible errors are rendered at `ids.errors`.
    pub has_errors: bool,
    /// Whether a value is required.
    pub required: bool,
    /// Whether interaction is disabled.
    pub disabled: bool,
    /// Whether changes should be ignored while retaining focusability.
    pub read_only: bool,
}

impl FieldState {
    /// Native labelling with all state flags unset.
    pub fn new(ids: FieldIds) -> Self {
        Self {
            ids,
            labelling: FieldLabelling::Native,
            has_description: false,
            has_errors: false,
            required: false,
            disabled: false,
            read_only: false,
        }
    }

    /// Description then visible-error IDREFs, omitting content that is not present.
    pub fn describedby(&self) -> Option<String> {
        let description = if self.has_description {
            self.ids.description.as_str()
        } else {
            ""
        };
        let errors = if self.has_errors {
            self.ids.errors.as_str()
        } else {
            ""
        };
        merge_idrefs([description, errors])
    }
}

/// Reactive field context consumed by descendant controls.
#[derive(Clone, Copy, PartialEq)]
pub struct FieldContext {
    state: ReadSignal<FieldState>,
}

impl FieldContext {
    /// Read and subscribe to the current presentation state.
    pub fn state(&self) -> FieldState {
        self.state.read().clone()
    }
}

/// Provide reactive state to descendants. Call unconditionally on every render.
pub fn use_field_provider(state: FieldState) -> FieldContext {
    let state = use_memo(use_reactive((&state,), |(state,)| state));
    use_context_provider(|| FieldContext {
        state: state.into(),
    })
}

/// The nearest field context, when present.
pub fn use_field_context() -> Option<FieldContext> {
    try_use_context::<FieldContext>()
}

/// Provide field state without adding an element, including inside table layouts.
#[component]
pub fn FieldProvider(state: FieldState, children: Element) -> Element {
    use_field_provider(state);
    children
}

/// Resolved control IDs, ARIA references, and interaction flags.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldControl {
    /// Focusable element ID.
    pub id: String,
    /// Accessible-name IDREFs, if needed.
    pub labelledby: Option<String>,
    /// Description and visible-error IDREFs.
    pub describedby: Option<String>,
    /// Whether the control has visible errors.
    pub invalid: bool,
    /// Whether a value is required.
    pub required: bool,
    /// Whether interaction is disabled.
    pub disabled: bool,
    /// Whether changes should be ignored.
    pub read_only: bool,
}

impl FieldControl {
    /// Attributes for a custom control. Native `disabled`, `required`, and `readonly`
    /// remain the caller's responsibility because these do not apply to every element.
    pub fn aria_attributes(&self) -> Vec<Attribute> {
        let mut attributes = vec![attr("id", self.id.clone())];
        if let Some(value) = &self.labelledby {
            attributes.push(attr("aria-labelledby", value.clone()));
        }
        if let Some(value) = &self.describedby {
            attributes.push(attr("aria-describedby", value.clone()));
        }
        for (name, enabled) in [
            ("aria-invalid", self.invalid),
            ("aria-required", self.required),
            ("aria-disabled", self.disabled),
            ("aria-readonly", self.read_only),
        ] {
            if enabled {
                attributes.push(attr(name, "true"));
            }
        }
        attributes
    }
}

/// Resolve context for a custom editor and return unconsumed caller attributes.
/// Caller IDs override context; set the ID on `Field` to keep its label's `for` aligned.
/// Caller `aria-labelledby` replaces context, `aria-describedby` merges after context,
/// and explicit `aria-invalid` wins. Interaction flags are ORed with context flags.
pub fn use_field_control(attributes: Vec<Attribute>) -> (FieldControl, Vec<Attribute>) {
    let (control, caller) = use_control(attributes, false);
    (control, caller.rest)
}

pub(crate) fn use_control(
    attributes: Vec<Attribute>,
    fallback_invalid: bool,
) -> (FieldControl, CallerControlAttrs) {
    let caller = take_control_attrs(attributes);
    let fallback = dxutil::id::use_unique_id("dx-control");
    let state = use_field_context().map(|context| context.state());
    let control = FieldControl {
        id: caller
            .id
            .clone()
            .or_else(|| state.as_ref().map(|s| s.ids.control.clone()))
            .unwrap_or(fallback),
        labelledby: caller.labelledby.clone().or_else(|| {
            state
                .as_ref()
                .filter(|s| s.labelling == FieldLabelling::LabelledBy)
                .map(|s| s.ids.label.clone())
        }),
        describedby: merge_idrefs([
            state
                .as_ref()
                .and_then(FieldState::describedby)
                .unwrap_or_default(),
            caller.describedby.clone().unwrap_or_default(),
        ]),
        invalid: caller
            .invalid
            .unwrap_or_else(|| state.as_ref().map_or(fallback_invalid, |s| s.has_errors)),
        required: caller.required || state.as_ref().is_some_and(|s| s.required),
        disabled: caller.disabled || state.as_ref().is_some_and(|s| s.disabled),
        read_only: caller.read_only || state.as_ref().is_some_and(|s| s.read_only),
    };
    (control, caller)
}
