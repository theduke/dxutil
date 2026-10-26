use crate::{FieldPath, FormError};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Validity {
    #[default]
    NotValidated,
    Validating,
    Valid,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldMeta {
    pub path: FieldPath,
    pub touched: bool,
    pub focused: bool,
    pub dirty: bool,
    pub empty: bool,
    pub validating: bool,
    pub validity: Validity,
    pub errors: Vec<FormError>,
    pub submit_errors: Vec<FormError>,
}

impl FieldMeta {
    pub fn new(path: FieldPath) -> Self {
        Self {
            path,
            touched: false,
            focused: false,
            dirty: false,
            empty: true,
            validating: false,
            validity: Validity::NotValidated,
            errors: Vec::new(),
            submit_errors: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScopeMeta {
    pub path: FieldPath,
    pub touched: bool,
    pub dirty: bool,
    pub empty: bool,
    pub validating: bool,
    pub validity: Validity,
    pub errors: Vec<FormError>,
    pub submit_errors: Vec<FormError>,
}

impl ScopeMeta {
    pub fn new(path: FieldPath) -> Self {
        Self {
            path,
            touched: false,
            dirty: false,
            empty: true,
            validating: false,
            validity: Validity::NotValidated,
            errors: Vec::new(),
            submit_errors: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListMeta {
    pub path: FieldPath,
    pub touched: bool,
    pub dirty: bool,
    pub empty: bool,
    pub validating: bool,
    pub validity: Validity,
    pub errors: Vec<FormError>,
    pub submit_errors: Vec<FormError>,
}

impl ListMeta {
    pub fn new(path: FieldPath) -> Self {
        Self {
            path,
            touched: false,
            dirty: false,
            empty: true,
            validating: false,
            validity: Validity::NotValidated,
            errors: Vec::new(),
            submit_errors: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormMeta {
    pub touched: bool,
    pub dirty: bool,
    pub empty: bool,
    pub validating: bool,
    pub validity: Validity,
    pub errors: Vec<FormError>,
    pub submit_errors: Vec<FormError>,
    pub submitting: bool,
    pub submit_count: u64,
    pub submit_attempted: bool,
    pub submit_succeeded: bool,
    pub submit_failed: bool,
    pub dirty_since_last_submit: bool,
}

impl Default for FormMeta {
    fn default() -> Self {
        Self {
            touched: false,
            dirty: false,
            empty: true,
            validating: false,
            validity: Validity::NotValidated,
            errors: Vec::new(),
            submit_errors: Vec::new(),
            submitting: false,
            submit_count: 0,
            submit_attempted: false,
            submit_succeeded: false,
            submit_failed: false,
            dirty_since_last_submit: false,
        }
    }
}
