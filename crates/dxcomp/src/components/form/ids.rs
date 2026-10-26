/// IDs shared by a control, its label, description, and visible errors.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FieldIds {
    /// The focusable control's ID.
    pub control: String,
    /// The visible label's ID.
    pub label: String,
    /// The description's ID.
    pub description: String,
    /// The error live region's ID.
    pub errors: String,
}

impl FieldIds {
    /// Use `base` for the control and append descriptive suffixes for related elements.
    pub fn new(base: impl Into<String>) -> Self {
        let control = base.into();
        Self {
            label: format!("{control}-label"),
            description: format!("{control}-description"),
            errors: format!("{control}-errors"),
            control,
        }
    }

    /// Deterministic, injectively encoded IDs for a path and a form-unique prefix.
    /// Use a different prefix for each form instance. Underscores are encoded too.
    pub fn from_path(prefix: &str, path: &str) -> Self {
        fn encode(text: &str) -> String {
            text.bytes()
                .map(|byte| {
                    if byte.is_ascii_alphanumeric() || byte == b'-' {
                        char::from(byte).to_string()
                    } else {
                        format!("_{byte:02x}")
                    }
                })
                .collect()
        }
        Self::new(format!("{}-{}", encode(prefix), encode(path)))
    }
}

/// Stable component IDs, recomputed from an explicit ID when it changes.
/// Generated IDs use a process-wide counter and are not hydration-safe; use an explicit
/// ID or [`FieldIds::from_path`] for server rendering followed by hydration.
pub fn use_field_ids(id: Option<String>) -> FieldIds {
    let fallback = dxutil::id::use_unique_id("dx-field");
    FieldIds::new(id.unwrap_or(fallback))
}
