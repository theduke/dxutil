//! Native components retain useful viewing/editing without browser editor APIs.
use crate::{ComponentDocumentV2, ComponentNode, EntityLinkExtension};
use dioxus::prelude::*;

#[component]
pub(crate) fn BlockDocumentFallback(
    document: ComponentDocumentV2,
    entity_links: Option<EntityLinkExtension>,
    read_only: bool,
) -> Element {
    let mut standard = crate::engine::markdown_blocks::to_standard(&document);
    describe_islands(&mut standard.root);
    rsx! {
        crate::render_v2::ReadOnlyDocumentV2 {document: standard, entity_links}
        if !read_only { p { class: "dxeditor__native-notice", role: "note", "Editing requires the web build." } }
    }
}

fn describe_islands(node: &mut ComponentNode) {
    if matches!(
        node.kind.0.as_str(),
        "graph" | "details_break" | "file" | "bookmark" | "entity_embed" | "unsupported"
    ) {
        let label = node
            .attrs
            .get("title")
            .or_else(|| node.attrs.get("url"))
            .or_else(|| node.attrs.get("entity"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&node.kind.0)
            .to_string();
        node.attrs
            .insert("fallback".into(), serde_json::Value::String(label));
    }
    for child in &mut node.content {
        describe_islands(child);
    }
}
