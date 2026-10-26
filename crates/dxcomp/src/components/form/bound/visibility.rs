use dxform::{FieldHandle, FieldMeta, FormError, FormMeta, ValidationStrategy};

use super::super::FieldState;

/// When parse and validation errors become visible. Submit errors remain visible except
/// under `Never`. Touched means interacted with, including input before the first blur.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ErrorVisibility {
    /// Touched and unfocused, after submit, or validating on change.
    #[default]
    Auto,
    /// After any interaction.
    Touched,
    /// After the first submit attempt.
    AfterSubmit,
    /// Immediately.
    Always,
    /// Caller owns error rendering.
    Never,
}

/// Visible node-local errors, followed by deduplicated submit errors.
/// Native has no focus events, so `Auto` behaves like `Touched`. Form reset preserves
/// `submit_attempted`. Parent field metadata does not aggregate descendant errors.
pub fn visible_field_errors(
    meta: &FieldMeta,
    form: &FormMeta,
    strategy: ValidationStrategy,
    visibility: ErrorVisibility,
) -> Vec<FormError> {
    if visibility == ErrorVisibility::Never {
        return Vec::new();
    }
    let show = match visibility {
        ErrorVisibility::Auto => {
            meta.touched && (!meta.focused || form.submit_attempted || strategy.on_change)
        }
        ErrorVisibility::Touched => meta.touched,
        ErrorVisibility::AfterSubmit => form.submit_attempted,
        ErrorVisibility::Always => true,
        ErrorVisibility::Never => false,
    };
    let mut errors = if show {
        meta.errors.clone()
    } else {
        Vec::new()
    };
    for error in &meta.submit_errors {
        if !errors.contains(error) {
            errors.push(error.clone());
        }
    }
    errors
}

/// Subscribe to field and form metadata and derive state for a custom layout.
/// Render the returned errors at `base.ids.errors`, or use `FieldErrors` with the same visibility.
pub fn bound_field_state<V, R, D>(
    field: &FieldHandle<V, R, D>,
    mut base: FieldState,
    visibility: ErrorVisibility,
) -> (FieldState, Vec<FormError>)
where
    V: Clone + PartialEq + 'static,
    R: Clone + PartialEq + 'static,
    D: Clone + PartialEq + 'static,
{
    let errors = visible_field_errors(
        &field.meta(),
        &field.form().meta(),
        field.validation(),
        visibility,
    );
    base.has_errors = !errors.is_empty();
    (base, errors)
}
