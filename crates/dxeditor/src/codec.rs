//! Persisted payload wrapper. Format codecs live in `format`.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EditorPayload {
    pub format: String,
    pub value: Value,
}

impl EditorPayload {
    pub fn new(format: impl Into<String>, value: Value) -> Self {
        Self {
            format: format.into(),
            value,
        }
    }
}
