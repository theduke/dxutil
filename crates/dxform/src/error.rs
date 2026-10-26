use std::fmt;

use crate::FieldPath;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormErrorSource {
    Parse,
    Validation,
    Submission,
    External,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormError {
    pub path: Option<FieldPath>,
    pub code: Option<String>,
    pub message: String,
    pub source: FormErrorSource,
}

impl fmt::Display for FormError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for FormError {}

impl FormError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            path: None,
            code: None,
            message: message.into(),
            source: FormErrorSource::Validation,
        }
    }

    pub fn field(path: impl Into<FieldPath>, message: impl Into<String>) -> Self {
        Self {
            path: Some(path.into()),
            code: None,
            message: message.into(),
            source: FormErrorSource::Validation,
        }
    }

    pub fn coded(
        path: impl Into<FieldPath>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            path: Some(path.into()),
            code: Some(code.into()),
            message: message.into(),
            source: FormErrorSource::Validation,
        }
    }

    pub fn with_source(mut self, source: FormErrorSource) -> Self {
        self.source = source;
        self
    }

    pub(crate) fn at_path(mut self, path: &FieldPath) -> Self {
        if self.path.is_none() {
            self.path = Some(path.clone());
        }
        self
    }
}
