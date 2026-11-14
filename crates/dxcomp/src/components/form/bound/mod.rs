mod actions;
mod select;
mod shell;
mod text;
mod toggle;
mod visibility;

pub use actions::*;
pub use select::*;
pub use shell::*;
pub use text::*;
pub use toggle::*;
pub use visibility::*;

use dioxus::prelude::*;
use dxform::{FieldHandle, FormError};

use super::FieldControl;
use super::attrs::{CallerControlAttrs, native_attributes};
use super::context::use_control;

fn use_bound_control<V, R, D>(
    field: &FieldHandle<V, R, D>,
    attributes: Vec<Attribute>,
) -> (FieldControl, CallerControlAttrs)
where
    V: Clone + PartialEq + 'static,
    R: Clone + PartialEq + 'static,
    D: Clone + PartialEq + 'static,
{
    let invalid = !visible_field_errors(
        &field.meta(),
        &field.form().meta(),
        field.validation(),
        ErrorVisibility::Auto,
    )
    .is_empty();
    let (control, mut caller) = use_control(attributes, invalid);
    if caller.name.is_none() {
        caller.name = Some(field.path().to_string());
    }
    (control, caller)
}

fn error_message(error: FormError, formatter: Option<Callback<FormError, String>>) -> String {
    match formatter {
        Some(formatter) => formatter.call(error),
        None => error.message,
    }
}
