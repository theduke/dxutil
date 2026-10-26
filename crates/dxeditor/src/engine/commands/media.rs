//! Persisted image settings and upload placeholders. Upload progress and errors
//! are view state, held separately from the image's durable media reference.
use crate::engine::{EditorEngine, EngineOutput, model, selection::Uid};
use crate::{AttributeMap, ComponentNode, is_safe_media_url};
use serde_json::{Value, json};

pub const MAX_PASTED_FILE_BYTES: u64 = 25 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileMetadata {
    pub name: String,
    pub mime: String,
    pub size: u64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct FilePlaceholder {
    pub node: ComponentNode,
    pub uid: Uid,
    pub transient: AttributeMap,
}

pub fn file_placeholders(files: &[FileMetadata]) -> Vec<FilePlaceholder> {
    files.iter().map(|file| {
        let content = ComponentNode::new(if file.mime.starts_with("image/") { "image" } else { "file" });
        let node = model::block_node(content);
        let uid = node.id.as_ref().map(|id| id.0.clone()).unwrap_or_default();
        let mut upload = json!({"status": if file.size > MAX_PASTED_FILE_BYTES { "failed" } else { "uploading" }, "name":file.name, "size":file.size, "progress":0});
        if file.size > MAX_PASTED_FILE_BYTES { upload["error"] = json!("Too large to paste — use Upload"); }
        let mut transient = AttributeMap::new(); transient.insert("upload".into(), upload);
        FilePlaceholder { node, uid, transient }
    }).collect()
}

pub fn update(engine: &mut EditorEngine, uid: Uid, attrs: AttributeMap) -> Vec<EngineOutput> {
    if !engine
        .content(&uid)
        .is_some_and(|node| node.kind.0 == "image")
    {
        return Vec::new();
    }
    for (key, value) in &attrs {
        if value.is_null() {
            continue;
        }
        let valid = match key.as_str() {
            "width" => value
                .as_f64()
                .is_some_and(|width| width.is_finite() && (1.0..=100.0).contains(&width)),
            "alignment" => value
                .as_str()
                .is_some_and(|alignment| matches!(alignment, "start" | "center" | "end")),
            "alt" | "title" => value.is_string(),
            "media" => {
                value.is_object()
                    && (value.get("type").and_then(Value::as_str) != Some("url")
                        || value
                            .get("url")
                            .and_then(Value::as_str)
                            .is_some_and(is_safe_media_url))
            }
            _ => false,
        };
        if !valid {
            return Vec::new();
        }
    }
    super::islands::update(engine, uid, attrs, true)
}
