//! Direct Rust host events and insertion options. Persisted document formats do
//! not depend on this API; there is no browser message protocol.
use crate::AttributeMap;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HistoryPolicy {
    Preserve,
    #[default]
    Reset,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EngineEvent {
    ConvertMarkdownRequest {
        request_id: u64,
        markdown: String,
    },
    ToggleCollapsed {
        uid: String,
        collapsed: bool,
    },
    CopyBlockLink {
        uid: String,
    },
    MoveToRequest {
        uids: Vec<String>,
    },
    EntityPickerRequest {
        request_id: u64,
        mode: String,
    },
    EntityCreateRequest {
        request_id: u64,
        query: String,
        trigger: String,
        #[serde(default)]
        uids: Vec<String>,
        #[serde(default)]
        checked: Option<bool>,
    },
    FilesPasted {
        items: Vec<PastedFile>,
    },
    UploadRetry {
        uid: String,
    },
    MediaUrlQuery {
        request_id: u64,
        media: AttributeMap,
    },
    FilePickerRequest {
        uid: String,
        accept: String,
    },
    MediaOpen {
        uid: String,
        media: Option<AttributeMap>,
    },
    Error {
        code: String,
        message: String,
        recoverable: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MentionSuggestion {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum InsertAt {
    Coords {
        x: f64,
        y: f64,
    },
    AfterBlock {
        uid: String,
    },
    #[default]
    Selection,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PastedFile {
    pub uid: String,
    pub name: String,
    pub mime: String,
    pub size: u64,
    pub data_base64: String,
}
