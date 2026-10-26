//! Public Markdown/payload editors and the retained read-only v1 document view.
use crate::{
    ComponentDocumentV2, EditorCatalog, EditorPayload,
    document::{
        BlockNode, COMPONENT_CODE, COMPONENT_DIVIDER, COMPONENT_HEADING, COMPONENT_LIST,
        COMPONENT_LIST_ITEM, COMPONENT_MENTION, COMPONENT_PARAGRAPH, COMPONENT_QUOTE,
        COMPONENT_TABLE, EditorDocument, InlineNode, Mark, NodeContent,
    },
    format::DecodeOptions,
};
use dioxus::prelude::*;
use serde_json::Value;

pub(crate) const DXEDITOR_STYLE: &str = r#"
.dxeditor {
  display: grid;
  width: min(100%, 980px);
  min-width: 0;
  max-width: 100%;
  overflow: hidden;
  border: 1px solid #cfd8e3;
  border-radius: 8px;
  background: #ffffff;
  color: #17202a;
}

.dxeditor__toolbar,
.dxeditor__status {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  padding: 8px;
  background: #f6f8fa;
}

.dxeditor__toolbar {
  border-bottom: 1px solid #dfe5ec;
}

.dxeditor__toolbar-group {
  display: inline-flex;
  flex-wrap: wrap;
  gap: 4px;
  padding-right: 8px;
  border-right: 1px solid #dfe5ec;
}

.dxeditor__toolbar-group:last-child {
  border-right: 0;
}

.dxeditor__button {
  min-width: 32px;
  min-height: 30px;
  border: 1px solid #cfd8e3;
  border-radius: 6px;
  background: #ffffff;
  color: #17202a;
  padding: 4px 8px;
  font: inherit;
  font-size: 12px;
  font-weight: 650;
  line-height: 1;
  cursor: pointer;
}

.dxeditor__button:hover {
  border-color: #176b87;
  color: #176b87;
}

.dxeditor__button:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.dxeditor__document {
  display: grid;
  min-width: 0;
  gap: 2px;
  min-height: 260px;
  padding: 22px 28px;
  background: #ffffff;
}

.dxeditor__block {
  min-width: 0;
  margin: 0;
  border: 0;
  border-radius: 4px;
  padding: 2px 0;
  outline: none;
  overflow-wrap: anywhere;
  line-height: 1.6;
  white-space: pre-wrap;
}

.dxeditor__block:hover {
  background: transparent;
}

.dxeditor__block:focus {
  box-shadow: inset 3px 0 0 #176b87;
  padding-left: 8px;
}

.dxeditor__block[data-component="heading"] {
  font-weight: 720;
  line-height: 1.25;
}

.dxeditor__block[data-level="1"] {
  font-size: 26px;
}

.dxeditor__block[data-level="2"] {
  font-size: 21px;
}

.dxeditor__block[data-component="quote"] {
  border-left: 3px solid #9bb5c1;
  border-radius: 0 4px 4px 0;
  color: #435367;
  padding-left: 12px;
}

.dxeditor__block[data-component="code"] {
  background: #f6f8fa;
  border-radius: 4px;
  padding: 8px 10px;
  font-family: ui-monospace, SFMono-Regular, Consolas, "Liberation Mono", Menlo, monospace;
  font-size: 13px;
}

.dxeditor__inline-code {
  border-radius: 4px;
  background: #eef2f6;
  padding: 0 4px;
  font-family: ui-monospace, SFMono-Regular, Consolas, "Liberation Mono", Menlo, monospace;
  font-size: 0.92em;
}

.dxeditor__mention {
  border-radius: 4px;
  background: #e9f2f5;
  color: #176b87;
  padding: 0 4px;
}

.dxeditor__divider {
  height: 1px;
  margin: 10px 0;
  border: 0;
  background: #cfd8e3;
}

.dxeditor__status {
  justify-content: space-between;
  border-top: 1px solid #dfe5ec;
  color: #617084;
  font-size: 12px;
}

.dxeditor[data-engine="rust"] {
  position: relative;
  overflow: visible;
  border: 0;
  border-radius: 0;
  box-shadow: none;
}

.dxeditor__editor-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  min-height: 38px;
  padding: 4px 8px;
  color: #617084;
}

.dxeditor__editor-actions-status {
  margin-left: auto;
  font-size: 12px;
}

.dxeditor__live-status {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}

.dxeditor[data-engine="rust"] .dxeditor__document {
  display: block;
  min-height: 160px;
  padding: 16px 20px 16px 36px;
}

.dxeditor-engine__content {
  min-width: 0;
  max-width: 100%;
  min-height: 128px;
  outline: none;
  line-height: 1.62;
  overflow-wrap: anywhere;
}

.dxeditor-engine__content > * {
  margin: 0.2rem 0;
}

.dxeditor-engine__content[contenteditable="true"] {
  cursor: text;
}

.dxeditor-engine__content ul,
.dxeditor-engine__content ol {
  padding-inline-start: 1.5rem;
  list-style-position: outside;
}

.dxeditor-engine__content ul {
  list-style-type: disc;
}

.dxeditor-engine__content ol {
  list-style-type: decimal;
}

.dxeditor-engine__content li > p {
  margin: 0;
}

.dxeditor__task-list {
  margin-inline-start: 0;
  padding-inline-start: 0;
  list-style: none;
}

.dxeditor__task-item {
  display: flex;
  align-items: flex-start;
  gap: 0.5rem;
}

.dxeditor__task-item > input[type="checkbox"] {
  flex: 0 0 auto;
  margin-top: 0.35em;
}


.dxeditor__task-item-content {
  flex: 1 1 auto;
  min-width: 0;
}

.dxeditor__task-item-content > :first-child {
  margin-top: 0;
}

.dxeditor__task-item-content > :last-child {
  margin-bottom: 0;
}

.dxeditor-engine__content h1,
.dxeditor-engine__content h2,
.dxeditor-engine__content h3 {
  line-height: 1.25;
  margin-top: 1.1em;
}

.dxeditor-engine__content blockquote {
  margin-left: 0;
  border-left: 3px solid #9bb5c1;
  padding-left: 12px;
  color: #435367;
}

.dxeditor-engine__content pre {
  overflow-x: auto;
  border-radius: 6px;
  background: #f6f8fa;
  padding: 12px;
}

.dxeditor-engine__content table {
  width: 100%;
  table-layout: fixed;
  border-collapse: collapse;
  overflow: hidden;
}

.dxeditor-engine__content td,
.dxeditor-engine__content th {
  min-width: 96px;
  border: 1px solid #cfd8e3;
  padding: 6px 8px;
  vertical-align: top;
}

.dxeditor-engine__content :is(td,th)[data-selected]::after {
  position: absolute;
  inset: 0;
  pointer-events: none;
  background: rgb(23 107 135 / 14%);
  content: "";
}

.dxeditor-engine__content .dx-table-wrapper {
  min-width: 0;
  max-width: 100%;
  overflow-x: auto;
}

         .dxeditor-engine__mention {
  border-radius: 4px;
  background: #e9f2f5;
  color: #176b87;
  padding: 1px 4px;
         }

         .dxeditor-engine__entity-link {
           color: var(--dxeditor-entity-link-color, #176b87);
           font-weight: 600;
           text-decoration-color: color-mix(in srgb, var(--dxeditor-entity-link-color, #176b87) 55%, transparent);
           text-underline-offset: 0.14em;
           cursor: pointer;
         }

.dxeditor-engine__opaque {
  border: 1px dashed #a8b4c2;
  white-space: pre-wrap;
}

.dxeditor__overlays {
  position: absolute;
  inset: 0;
  z-index: 20;
  pointer-events: none;
}

.dxeditor-engine__surface {
  position: absolute;
  display: flex;
  gap: 2px;
  max-width: min(520px, calc(100vw - 24px));
  border: 1px solid #d8dee7;
  border-radius: 7px;
  background: #fff;
  box-shadow: 0 6px 24px rgb(23 32 42 / 16%);
  padding: 4px;
  pointer-events: auto;
}

.dxeditor-engine__surface[hidden] {
  display: none;
}

.dxeditor-engine__button {
  min-width: 30px;
  min-height: 30px;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: inherit;
  padding: 4px 8px;
  font: inherit;
  cursor: pointer;
}

.dxeditor-engine__button:hover,
.dxeditor-engine__button:focus-visible,
.dxeditor-engine__button[role="option"][aria-selected="true"] {
  background: #eef2f6;
  outline: 2px solid transparent;
}

.dxeditor-engine__button:disabled {
  color: #a8b4c2;
  cursor: default;
}

.dxeditor-engine__slash {
  flex-direction: column;
  width: 230px;
  max-height: 300px;
  overflow-y: auto;
}

.dxeditor-engine__block-menu {
  flex-direction: column;
  min-width: 160px;
}

.dxeditor-engine__block-menu .dxeditor-engine__button {
  text-align: left;
}

.dxeditor-engine__link-popover {
  flex-direction: column;
  width: min(320px, calc(100vw - 24px));
  padding: 8px;
}

.dxeditor-engine__link-popover label {
  display: grid;
  gap: 4px;
  color: #435367;
  font-size: 12px;
}

.dxeditor-engine__link-popover input {
  min-height: 34px;
  border: 1px solid #a8b4c2;
  border-radius: 5px;
  padding: 4px 8px;
  font: inherit;
}

         .dxeditor-engine__link-actions {
  display: flex;
  justify-content: flex-end;
  gap: 4px;
         }

         .dxeditor-engine__entity-popover {
           flex-direction: column;
           width: min(360px, calc(100vw - 24px));
           padding: 8px;
         }

         .dxeditor-engine__entity-popover > input {
           min-height: 36px;
           border: 1px solid #a8b4c2;
           border-radius: 5px;
           padding: 5px 8px;
           font: inherit;
         }

         .dxeditor-engine__entity-results {
           display: grid;
           max-height: 280px;
           overflow-y: auto;
         }

         .dxeditor-engine__entity-results .dxeditor-engine__button {
           text-align: left;
         }

         .dxeditor-engine__entity-preview {
           display: grid;
           width: min(340px, calc(100vw - 24px));
           gap: 5px;
           padding: 10px 12px;
         }

         .dxeditor-engine__entity-preview-detail {
           color: #617084;
           font-size: 12px;
         }

         .dxeditor-engine__entity-preview dl {
           display: grid;
           grid-template-columns: minmax(70px, auto) 1fr;
           gap: 3px 10px;
           margin: 4px 0 0;
           font-size: 12px;
         }

         .dxeditor-engine__entity-preview dt {
           color: #617084;
         }

         .dxeditor-engine__entity-preview dd {
           min-width: 0;
           margin: 0;
           overflow-wrap: anywhere;
         }

         .dxeditor__read-only-entity-link {
           position: relative;
         }

         .dxeditor__read-only-entity-preview {
           position: absolute;
           z-index: 20;
           top: calc(100% + 5px);
           left: 0;
           display: grid;
           width: min(320px, calc(100vw - 24px));
           gap: 4px;
           border: 1px solid #d8dee7;
           border-radius: 7px;
           background: #fff;
           box-shadow: 0 6px 24px rgb(23 32 42 / 16%);
           padding: 10px 12px;
           color: #17202a;
         }

         .dxeditor__read-only-entity-preview dl {
           display: grid;
           grid-template-columns: minmax(70px, auto) 1fr;
           gap: 3px 10px;
           margin: 4px 0 0;
           font-size: 12px;
         }

         .dxeditor__read-only-entity-preview dd {
           margin: 0;
         }

.dxeditor-engine__field-error {
  min-height: 1em;
  color: #b42318;
  font-size: 12px;
}

.dxeditor-engine__slash .dxeditor-engine__button {
  text-align: left;
}

.dxeditor-engine__block-controls {
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  border: 0;
  background: transparent;
  box-shadow: none;
  padding: 0;
}

.dxeditor-engine__block-controls[data-placement="inline"] {
  border-radius: 5px;
  background: rgb(255 255 255 / 92%);
  box-shadow: 0 1px 6px rgb(23 32 42 / 14%);
}

.dxeditor-engine__block-controls::after,
.dxeditor-engine__table-row-handle::after {
  position: absolute;
  inset-block: 0;
  left: 100%;
  width: 6px;
  content: "";
}

.dxeditor-engine__block-controls .dxeditor-engine__button {
  width: 22px;
  height: 22px;
  min-width: 22px;
  min-height: 22px;
  padding: 0;
  font-size: 16px;
  line-height: 1;
}

.dxeditor-engine__table-controls,
.dxeditor-engine__table-row-controls {
  position: absolute;
  inset: 0;
  pointer-events: none;
}

.dxeditor-engine__table-controls[hidden],
.dxeditor-engine__table-row-controls[hidden] {
  display: none;
}

.dxeditor-engine__table-controls .dxeditor-engine__button,
.dxeditor-engine__table-row-handle {
  position: absolute;
  z-index: 2;
  box-sizing: border-box;
  border: 1px solid #d8dee7;
  background: #fff;
  box-shadow: 0 2px 8px rgb(23 32 42 / 12%);
  pointer-events: auto;
}

.dxeditor-engine__table-add-row {
  min-height: 24px;
  border-radius: 5px;
  padding: 0;
}

.dxeditor-engine__table-add-column {
  min-width: 24px;
  border-radius: 5px;
  padding: 0;
}

.dxeditor-engine__table-row-handle {
  min-width: 28px;
  min-height: 28px;
  padding: 2px 4px;
  cursor: grab;
}

.dxeditor-engine__table-row-handle:active,
.dxeditor-engine__table-row-handle[aria-grabbed="true"] {
  cursor: grabbing;
}

.dxeditor-engine__table-row-handle[data-drop-position="before"] {
  border-top: 3px solid #176b87;
}

.dxeditor-engine__table-row-handle[data-drop-position="after"] {
  border-bottom: 3px solid #176b87;
}

.dxeditor-engine__table-row-menu .dxeditor-engine__button {
  text-align: left;
}

.dxeditor__error {
  border: 1px solid #b42318;
  border-radius: 6px;
  background: #fff5f4;
  padding: 12px;
  color: #7a271a;
}

@media (max-width: 640px) {
  .dxeditor[data-engine="rust"] .dxeditor__document {
    padding: 12px 14px 12px 32px;
  }

  .dxeditor-engine__block-controls .dxeditor-engine__button {
    width: 20px;
    height: 20px;
    min-width: 20px;
    min-height: 20px;
    font-size: 14px;
  }
}
"#;

#[component]
pub fn Editor(
    value: EditorPayload,
    output_format: String,
    catalog: EditorCatalog,
    on_change: EventHandler<EditorPayload>,
    #[props(default)] onfocus: Option<EventHandler<FocusEvent>>,
    #[props(default)] onblur: Option<EventHandler<FocusEvent>>,
    #[props(default)] readonly: bool,
    #[props(default)] external_revision: Option<u64>,
    #[props(default = "Document editor".to_string())] aria_label: String,
    #[props(default)] entity_links: Option<crate::EntityLinkExtension>,
) -> Element {
    let input_catalog = catalog.clone();
    let decoded = use_memo(use_reactive((&value,), move |(value,)| {
        decode_editor_payload(&input_catalog, &value)
    }));
    let decoded = decoded.read().clone();
    let source_document = decoded.as_ref().ok().cloned().unwrap_or_default();
    let projection = use_memo(use_reactive((&source_document,), move |(document,)| {
        crate::engine::markdown_blocks::to_blocks(&document)
    }));
    #[allow(unused_mut, unused_variables)]
    let mut error = use_signal(String::new);
    #[allow(unused_mut)]
    let mut last_emitted = use_signal(|| None::<EditorPayload>);
    let last_projection =
        use_hook(|| std::rc::Rc::new(std::cell::RefCell::new(None::<ComponentDocumentV2>)));
    let Some(standard) = decoded.as_ref().ok() else {
        let message = decoded.as_ref().err().cloned().unwrap_or_default();
        return rsx! { div { role: "alert", class: "dxeditor__error", "{message}" } };
    };
    if readonly {
        return rsx! {
            style { {DXEDITOR_STYLE} }
            div { class: "dxeditor", crate::render_v2::ReadOnlyDocumentV2 { document: standard.clone(), entity_links } }
        };
    }
    #[cfg(not(all(feature = "web", target_arch = "wasm32")))]
    {
        let _ = (
            external_revision,
            entity_links,
            last_emitted,
            standard,
            projection,
            last_projection,
        );
        let raw = value
            .value
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| value.value.to_string());
        return rsx! {
            style { {DXEDITOR_STYLE} }
            textarea {
                class: "dxeditor__textarea", aria_label, value: raw,
                onfocus: move |event| { if let Some(callback) = onfocus { callback.call(event); } },
                onblur: move |event| { if let Some(callback) = onblur { callback.call(event); } },
                oninput: move |event| {
                    let payload = EditorPayload::new(output_format.clone(), Value::String(event.value()));
                    on_change.call(payload);
                },
            }
        };
    }
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    {
        let blocks = if last_emitted.peek().as_ref() == Some(&value) {
            last_projection
                .borrow()
                .clone()
                .unwrap_or_else(|| projection.read().clone())
        } else {
            projection.read().clone()
        };
        // The input catalog describes persisted standard nodes. The editing
        // schema has block containers and permits only persistable commands.
        let mut editing_catalog = EditorCatalog::markdown_notes();
        *editing_catalog.suggestions_mut() = catalog.suggestions().clone();
        rsx! {
            crate::ComponentEditor {
                document: blocks,
                catalog: editing_catalog,
                external_revision,
                read_only: readonly,
                aria_label,
                entity_links,
                onfocus,
                onblur,
                on_invalid_change: move |message| error.set(message),
                on_change: move |change: crate::ComponentChange| {
                    let standard = crate::engine::markdown_blocks::to_standard(&change.document);
                    match encode_editor_payload(&catalog, &standard, &output_format) {
                        Ok(payload) => {
                            *last_projection.borrow_mut() = Some(change.document);
                            if last_emitted.peek().as_ref() != Some(&payload) {
                                last_emitted.set(Some(payload.clone()));
                                on_change.call(payload);
                            }
                            if !error.peek().is_empty() { error.set(String::new()); }
                        }
                        Err(message) => error.set(message),
                    }
                },
            }
            if !error().is_empty() { span { role: "status", "{error}" } }
        }
    }
}

fn decode_editor_payload(
    catalog: &EditorCatalog,
    payload: &EditorPayload,
) -> Result<ComponentDocumentV2, String> {
    catalog
        .document_formats()
        .decode(
            payload,
            catalog.component_specs(),
            DecodeOptions {
                unknown_components: crate::UnknownComponentPolicy::PreserveOpaque,
                ..DecodeOptions::default()
            },
        )
        .map(|decoded| decoded.document)
        .map_err(|error| error.to_string())
}

#[cfg(any(test, all(feature = "web", target_arch = "wasm32")))]
fn encode_editor_payload(
    catalog: &EditorCatalog,
    document: &ComponentDocumentV2,
    format: &str,
) -> Result<EditorPayload, String> {
    catalog
        .document_formats()
        .encode(
            document,
            format,
            catalog.component_specs(),
            crate::EncodeOptions::default(),
        )
        .map(|encoded| encoded.payload)
        .map_err(|error| match error {
            crate::FormatError::Validation(problem) => format!("{problem}: {:?}", problem.issues),
            other => other.to_string(),
        })
}

#[component]
pub fn MarkdownEditor(
    value: String,
    on_change: EventHandler<String>,
    #[props(default)] onfocus: Option<EventHandler<FocusEvent>>,
    #[props(default)] onblur: Option<EventHandler<FocusEvent>>,
    #[props(default)] catalog: EditorCatalog,
    #[props(default)] readonly: bool,
    #[props(default)] external_revision: Option<u64>,
    #[props(default)] entity_links: Option<crate::EntityLinkExtension>,
) -> Element {
    rsx! { Editor {
        value: EditorPayload::new("markdown", Value::String(value)),
        output_format: "markdown".to_string(), catalog, readonly, external_revision,
        entity_links, onfocus, onblur,
        on_change: move |payload: EditorPayload| {
            if let Some(value) = payload.value.as_str() { on_change.call(value.to_string()); }
        },
    } }
}

#[component]
pub fn DocumentView(document: EditorDocument, #[props(default)] catalog: EditorCatalog) -> Element {
    let _ = catalog;
    rsx! { ReadOnlyDocument { document } }
}

#[component]
fn ReadOnlyDocument(document: EditorDocument) -> Element {
    rsx! {
        article { class: "dxeditor__document dxeditor__read-only",
            for block in document.blocks {
                ReadOnlyBlock { block }
            }
        }
    }
}

#[component]
fn ReadOnlyBlock(block: BlockNode) -> Element {
    let component = block.component.as_str();
    let level = block
        .attrs
        .get("level")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .clamp(1, 6);
    match component {
        COMPONENT_HEADING => match level {
            1 => rsx! { h1 { ReadOnlyContent { content: block.content } } },
            2 => rsx! { h2 { ReadOnlyContent { content: block.content } } },
            3 => rsx! { h3 { ReadOnlyContent { content: block.content } } },
            4 => rsx! { h4 { ReadOnlyContent { content: block.content } } },
            5 => rsx! { h5 { ReadOnlyContent { content: block.content } } },
            _ => rsx! { h6 { ReadOnlyContent { content: block.content } } },
        },
        COMPONENT_QUOTE => rsx! { blockquote { ReadOnlyContent { content: block.content } } },
        COMPONENT_CODE => rsx! { pre { code { "{block.text_content()}" } } },
        COMPONENT_DIVIDER => rsx! { hr {} },
        COMPONENT_LIST => {
            let ordered = block
                .attrs
                .get("ordered")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let task = block
                .attrs
                .get("task")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let NodeContent::Blocks(items) = block.content else {
                return rsx! {};
            };
            if ordered {
                rsx! { ol { for item in items { ReadOnlyListItem { block: item } } } }
            } else {
                rsx! { ul {
                    class: if task { "dxeditor__task-list" } else { "" },
                    for item in items { ReadOnlyListItem { block: item } }
                } }
            }
        }
        COMPONENT_TABLE => {
            let NodeContent::Table(table) = block.content else {
                return rsx! {};
            };
            rsx! {
                div { class: "dx-table-wrapper",
                    table {
                        tbody {
                            for row in table.rows {
                                tr {
                                    for cell in row.cells {
                                        td {
                                            for cell_block in cell.blocks {
                                                ReadOnlyBlock { block: cell_block }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        "opaque_markdown" | "opaque_markdown_block" => {
            let source = block
                .attrs
                .get("source")
                .and_then(Value::as_str)
                .unwrap_or_default();
            rsx! { pre { class: "dxeditor-engine__opaque", code { "{source}" } } }
        }
        COMPONENT_PARAGRAPH | COMPONENT_LIST_ITEM => {
            rsx! { p { ReadOnlyContent { content: block.content } } }
        }
        _ => rsx! { div { class: "dxeditor__fallback", "{block.text_content()}" } },
    }
}

#[component]
fn ReadOnlyListItem(block: BlockNode) -> Element {
    let checked = block.attrs.get("checked").and_then(Value::as_bool);
    rsx! {
        li {
            class: if checked.is_some() { "dxeditor__task-item" } else { "" },
            if let Some(checked) = checked {
                input { r#type: "checkbox", checked, disabled: true, aria_label: "Task complete" }
                div { class: "dxeditor__task-item-content",
                    ReadOnlyContent { content: block.content }
                }
            } else {
                ReadOnlyContent { content: block.content }
            }
        }
    }
}

#[component]
fn ReadOnlyContent(content: NodeContent) -> Element {
    match content {
        NodeContent::Inline(inline) => rsx! {
            for node in inline { ReadOnlyInline { node } }
        },
        NodeContent::Blocks(blocks) => rsx! {
            for block in blocks { ReadOnlyBlock { block } }
        },
        NodeContent::Table(_) | NodeContent::Void | NodeContent::Custom(_) => rsx! {},
    }
}

#[component]
fn ReadOnlyInline(node: InlineNode) -> Element {
    if node.component == COMPONENT_MENTION {
        return rsx! { span { class: "dxeditor-engine__mention", "@{node.text}" } };
    }
    render_marked_text(node.text, &node.marks, 0)
}

fn render_marked_text(text: String, marks: &[Mark], index: usize) -> Element {
    let Some(mark) = marks.get(index) else {
        return rsx! { "{text}" };
    };
    let child = render_marked_text(text, marks, index + 1);
    match mark.component.as_str() {
        "bold" => rsx! { strong { {child} } },
        "italic" => rsx! { em { {child} } },
        "strike" => rsx! { del { {child} } },
        "code" => rsx! { code { {child} } },
        "link" => {
            let href = mark
                .attrs
                .get("href")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if safe_link_url(href) {
                rsx! { a { href, rel: "noopener noreferrer", {child} } }
            } else {
                child
            }
        }
        _ => child,
    }
}

fn safe_link_url(url: &str) -> bool {
    crate::component_spec::is_safe_url(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_payloads_remain_decodable_through_the_format_registry() {
        let payload = EditorPayload::new(
            crate::document::DOCUMENT_SCHEMA_V1,
            serde_json::to_value(EditorDocument::plain_text("hello")).unwrap(),
        );
        let catalog = EditorCatalog::default();
        let document = decode_editor_payload(&catalog, &payload).unwrap();
        assert_eq!(document.text_content(), "hello");
        let encoded =
            encode_editor_payload(&catalog, &document, crate::document::DOCUMENT_SCHEMA_V1)
                .unwrap();
        let restored: EditorDocument = serde_json::from_value(encoded.value).unwrap();
        assert_eq!(restored.text_content(), "hello");
    }
}
