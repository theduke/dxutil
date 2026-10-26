use dioxus::prelude::*;
use dxform::{FieldHandle, FormError, FormRoot};

use super::super::{Field, FieldErrorList, FieldLabelling, FieldLayout};
use super::{ErrorVisibility, error_message, visible_field_errors};

/// Field shell deriving visible errors and interaction metadata from a typed dxform handle.
#[component]
pub fn FormField<V, R, D>(
    field: FieldHandle<V, R, D>,
    #[props(default)] id: Option<String>,
    #[props(default)] label: Option<String>,
    #[props(default)] description: Option<String>,
    #[props(default)] error_visibility: ErrorVisibility,
    #[props(default)] format_error: Option<Callback<FormError, String>>,
    #[props(default)] required: bool,
    #[props(default)] disabled: bool,
    #[props(default)] read_only: bool,
    #[props(default)] disable_while_submitting: bool,
    #[props(default)] layout: FieldLayout,
    #[props(default)] labelling: Option<FieldLabelling>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element
where
    V: Clone + PartialEq + 'static,
    R: Clone + PartialEq + 'static,
    D: Clone + PartialEq + 'static,
{
    let meta = field.meta();
    let form = field.form().meta();
    let errors: Vec<String> =
        visible_field_errors(&meta, &form, field.validation(), error_visibility)
            .into_iter()
            .map(|error| error_message(error, format_error))
            .collect();
    rsx! {
        Field {
            id, label, description, errors, required, read_only, layout, labelling,
            disabled: disabled || (disable_while_submitting && form.submitting),
            "data-dirty": meta.dirty,
            "data-touched": meta.touched,
            "data-validating": meta.validating,
            attributes,
            {children}
        }
    }
}

/// Visible errors at the context's errors ID (or an explicit ID) for custom layouts.
#[component]
pub fn FieldErrors<V, R, D>(
    field: FieldHandle<V, R, D>,
    #[props(default)] visibility: ErrorVisibility,
    #[props(default)] format_error: Option<Callback<FormError, String>>,
    #[props(default)] id: Option<String>,
) -> Element
where
    V: Clone + PartialEq + 'static,
    R: Clone + PartialEq + 'static,
    D: Clone + PartialEq + 'static,
{
    let messages: Vec<String> = visible_field_errors(
        &field.meta(),
        &field.form().meta(),
        field.validation(),
        visibility,
    )
    .into_iter()
    .map(|error| error_message(error, format_error))
    .collect();
    rsx! { FieldErrorList { messages, id } }
}

/// Always-mounted alert for form/root errors. Validation errors appear after a submit
/// attempt; submit errors are always visible. Enable `include_field_errors` for all paths.
#[component]
pub fn FormErrors<T>(
    form: FormRoot<T>,
    #[props(default)] include_field_errors: bool,
    #[props(default)] format_error: Option<Callback<FormError, String>>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element
where
    T: Clone + PartialEq + 'static,
{
    let meta = form.meta();
    let mut errors = Vec::new();
    for error in meta
        .errors
        .iter()
        .filter(|_| meta.submit_attempted)
        .chain(&meta.submit_errors)
    {
        if (include_field_errors || error.path.as_ref().is_none_or(|path| path.is_root()))
            && !errors.contains(error)
        {
            errors.push(error.clone());
        }
    }
    rsx! {
        div { class: "dx-form-errors", role: "alert", ..attributes,
            if !errors.is_empty() {
                ul { for error in errors { li { {error_message(error, format_error)} } } }
            }
        }
    }
}
