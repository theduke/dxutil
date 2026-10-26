//! Synchronous clipboard access is confined to trusted browser event handlers.
use super::{DomHost, Host, inside_island, selection_sync, target_element};
use crate::engine::{
    Command, EditorSelection, EngineInput, EngineOutput,
    clipboard::{self, ClipboardSlice, PasteContent},
    commands::{self, BlockPlacement},
    html::{self, HtmlNode},
    model, text,
};
use std::collections::BTreeMap;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{ClipboardEvent, DataTransfer, DragEvent, Element, Node};
fn sync(host: &Host) {
    let selection = {
        let h = host.borrow();
        if h.root
            .class_list()
            .contains("dxeditor-engine--hide-selection")
        {
            return;
        }
        selection_sync::read(&h.root)
    };
    if let Some(s) = selection {
        host.borrow()
            .engine
            .borrow_mut()
            .handle(EngineInput::SelectionChanged(s));
    }
}
fn slice(host: &Host) -> Option<ClipboardSlice> {
    let h = host.borrow();
    let e = h.engine.borrow();
    match e.selection() {
        EditorSelection::Text(r) if r.is_collapsed() => None,
        EditorSelection::Text(r) if r.anchor.container == r.head.container => {
            let source = model::inline(e.document(), &r.head.container)?;
            let selected = text::slice(
                source,
                r.anchor.offset.min(r.head.offset),
                r.anchor.offset.max(r.head.offset),
            );
            let mut block = model::paragraph();
            block.content[0].content = selected;
            Some(ClipboardSlice {
                content: vec![block],
                open_start: 1,
                open_end: 1,
            })
        }
        EditorSelection::Text(r) => {
            let a = commands::blocks::owner(&e, &r.anchor.container)?;
            let b = commands::blocks::owner(&e, &r.head.container)?;
            let order = model::block_order(e.document());
            let ai = order.iter().position(|u| u == &a)?;
            let bi = order.iter().position(|u| u == &b)?;
            if ai == bi {
                return None;
            }
            let (start, end, from, to) = if ai < bi {
                (&r.anchor, &r.head, ai, bi)
            } else {
                (&r.head, &r.anchor, bi, ai)
            };
            let mut content: Vec<_> = order[from..=to]
                .iter()
                .filter_map(|uid| model::block(e.document(), uid).cloned())
                .collect();
            for node in &mut content {
                node.content.retain(|n| n.kind.0 != "block_children");
            }
            if let Some(first) = content.first_mut().and_then(|b| b.content.first_mut()) {
                first.content = text::split(&first.content, start.offset).1;
            }
            if let Some(last) = content.last_mut().and_then(|b| b.content.first_mut()) {
                last.content = text::split(&last.content, end.offset).0;
            }
            Some(ClipboardSlice {
                content,
                open_start: 1,
                open_end: 1,
            })
        }
        EditorSelection::Cells {
            table,
            anchor,
            head,
        } => {
            let mut block = model::block(e.document(), table)?.clone();
            let table = block.content.first_mut()?;
            let (r0, r1) = (anchor.0.min(head.0), anchor.0.max(head.0));
            let (c0, c1) = (anchor.1.min(head.1), anchor.1.max(head.1));
            table.content = table.content.get(r0..=r1)?.to_vec();
            for row in &mut table.content {
                row.content = row.content.get(c0..=c1)?.to_vec();
            }
            table.attrs.insert(
                "header_row".into(),
                serde_json::json!(
                    r0 == 0
                        && table
                            .attrs
                            .get("header_row")
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(false)
                ),
            );
            table.attrs.insert(
                "header_column".into(),
                serde_json::json!(
                    c0 == 0
                        && table
                            .attrs
                            .get("header_column")
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(false)
                ),
            );
            if let Some(widths) = table
                .attrs
                .get_mut("column_widths")
                .and_then(serde_json::Value::as_array_mut)
            {
                *widths = widths.get(c0..=c1).unwrap_or_default().to_vec();
            }
            Some(ClipboardSlice {
                content: vec![block],
                open_start: 0,
                open_end: 0,
            })
        }
        _ => {
            let ids = commands::blocks::selected_uids(&e);
            let mut roots = Vec::new();
            for id in ids {
                if !roots
                    .iter()
                    .any(|b: &crate::ComponentNode| model::find(b, &id).is_some())
                {
                    if let Some(block) = model::block(e.document(), &id) {
                        roots.push(block.clone());
                    }
                }
            }
            Some(ClipboardSlice {
                content: roots,
                open_start: 0,
                open_end: 0,
            })
        }
    }
}
fn copy(host: &Host, data: &DataTransfer) -> bool {
    let Some(slice) = slice(host) else {
        return false;
    };
    let fingerprint = host.borrow().engine.borrow().schema().fingerprint.clone();
    let Ok(envelope) = clipboard::encode_internal_clipboard(&slice, &fingerprint) else {
        return false;
    };
    let html = html::blocks_to_html_with_slice(&slice.content, &envelope);
    let plain = clipboard::blocks_to_plain_text(&slice.content);
    let success = data
        .set_data(clipboard::INTERNAL_CLIPBOARD_MIME, &envelope)
        .is_ok()
        && data.set_data("text/html", &html).is_ok()
        && data.set_data("text/plain", &plain).is_ok();
    if host.borrow().engine.borrow().profile() == crate::engine::Profile::Composer {
        let value = crate::profiles::composer::blocks_to_composer(&model::document(slice.content));
        if let Ok(raw) = clipboard::encode_composer_clipboard(&value) {
            let _ = data.set_data(clipboard::COMPOSER_CLIPBOARD_MIME, &raw);
        }
    }
    success
}
fn cut_selection(host: &Host) {
    let removed = if host.borrow().engine.borrow().profile() == crate::engine::Profile::Composer {
        fn chips(
            node: &crate::ComponentNode,
            output: &mut Vec<crate::profiles::composer::ComposerChip>,
        ) {
            if let Some(chip) = crate::profiles::composer::chip_from_node(node) {
                output.push(chip);
            }
            for child in &node.content {
                chips(child, output);
            }
        }
        let mut removed = Vec::new();
        if let Some(slice) = slice(host) {
            for node in &slice.content {
                chips(node, &mut removed);
            }
        }
        removed
    } else {
        Vec::new()
    };
    let cells = matches!(
        host.borrow().engine.borrow().selection(),
        EditorSelection::Cells { .. }
    );
    DomHost::run(
        host,
        if cells {
            Command::Table(commands::TableCommand::Clear)
        } else {
            Command::Delete {
                direction: text::Direction::Backward,
                unit: text::DeleteUnit::Selection,
            }
        },
    );
    for chip in removed {
        emit_composer(
            host,
            crate::profiles::composer::ComposerEvent::ChipRemoved(chip),
        );
    }
}
fn dom_html(node: &Node, depth: usize, budget: &mut usize) -> Option<HtmlNode> {
    if depth > clipboard::MAX_CLIPBOARD_DEPTH || *budget == 0 {
        return None;
    }
    *budget -= 1;
    if node.node_type() == Node::TEXT_NODE {
        return Some(HtmlNode::Text(node.node_value().unwrap_or_default()));
    }
    let Some(el) = node.dyn_ref::<Element>() else {
        return Some(HtmlNode::Text(String::new()));
    };
    let mut attrs = BTreeMap::new();
    for name in [
        "href",
        "src",
        "alt",
        "title",
        "start",
        "class",
        "style",
        "data-semantic-mention",
        "data-entity-id",
        "data-semantic-slice",
        "data-type",
        "data-checked",
        "checked",
        "align",
        "data-color",
        "data-highlight",
        "data-block-kind",
        "data-background-color",
        "data-language",
        "data-label",
        "data-composer-chip",
        "data-payload",
    ] {
        if let Some(v) = el.get_attribute(name) {
            attrs.insert(name.into(), v);
        }
    }
    let children = el.child_nodes();
    Some(HtmlNode::Element {
        tag: el.tag_name().to_lowercase(),
        attrs,
        children: (0..children.length())
            .filter_map(|index| children.item(index))
            .map(|node| dom_html(&node, depth + 1, budget))
            .collect::<Option<Vec<_>>>()?,
    })
}
fn parse_html(raw: &str) -> Option<HtmlNode> {
    if raw.len() > clipboard::MAX_CLIPBOARD_BYTES {
        return None;
    }
    let doc = web_sys::DomParser::new()
        .ok()?
        .parse_from_string(raw, web_sys::SupportedType::TextHtml)
        .ok()?;
    let mut budget = clipboard::MAX_CLIPBOARD_NODES;
    doc.body()
        .and_then(|body| dom_html(body.as_ref(), 0, &mut budget))
}
fn paste(host: &Host, data: &DataTransfer) {
    if host.borrow().engine.borrow().read_only() {
        return;
    }
    let code = {
        let current = host.borrow();
        let engine = current.engine.borrow();
        commands::blocks::current_uid(&engine)
            .and_then(|uid| model::content(engine.document(), &uid))
            .is_some_and(|node| node.kind.0 == "code_block")
    };
    if code {
        DomHost::run(
            host,
            Command::InsertText(data.get_data("text/plain").unwrap_or_default()),
        );
        return;
    }
    let composer = host.borrow().engine.borrow().profile() == crate::engine::Profile::Composer;
    let plain_mode = {
        let mut h = host.borrow_mut();
        std::mem::take(&mut h.plain_paste)
    };
    let raw = data
        .get_data(clipboard::COMPOSER_CLIPBOARD_MIME)
        .unwrap_or_default();
    if composer && !plain_mode && !raw.is_empty() {
        if let Some(decoded) = clipboard::decode_composer_clipboard(&raw) {
            if decoded.attachments.as_ref().is_some_and(|a| !a.is_empty()) {
                emit_composer(
                    host,
                    crate::profiles::composer::ComposerEvent::Dropped {
                        mime: clipboard::COMPOSER_CLIPBOARD_MIME.into(),
                        data: raw,
                    },
                );
            }
            let doc = crate::profiles::composer::composer_to_blocks(&decoded.value);
            let content = model::children(&doc, None).cloned().unwrap_or_default();
            DomHost::dispatch(
                host,
                EngineInput::Paste(PasteContent {
                    internal: Some(ClipboardSlice {
                        content,
                        open_start: 1,
                        open_end: 1,
                    }),
                    ..PasteContent::default()
                }),
            );
            return;
        }
    }
    if let Some(files) = data
        .files()
        .filter(|files| files.length() > 0 && (!composer || !plain_mode))
    {
        let files = (0..files.length()).filter_map(|i| files.item(i)).collect();
        upload_files(host, files, crate::InsertAt::Selection);
        return;
    }
    let text = data.get_data("text/plain").unwrap_or_default();
    if composer && !plain_mode && text.chars().count() > host.borrow().max_inline_paste_chars {
        emit_composer(
            host,
            crate::profiles::composer::ComposerEvent::PastedText { text },
        );
        return;
    }
    if composer {
        DomHost::dispatch(
            host,
            EngineInput::Paste(PasteContent {
                text: Some(text),
                plain_mode: true,
                ..Default::default()
            }),
        );
        return;
    }
    let fingerprint = host.borrow().engine.borrow().schema().fingerprint.clone();
    let internal = if plain_mode {
        None
    } else {
        clipboard::decode_internal_clipboard(
            &data
                .get_data(clipboard::INTERNAL_CLIPBOARD_MIME)
                .unwrap_or_default(),
            &fingerprint,
        )
    };
    let html_raw = if plain_mode {
        String::new()
    } else {
        data.get_data("text/html").unwrap_or_default()
    };
    let parsed = if html_raw.is_empty() {
        None
    } else {
        parse_html(&html_raw)
    };
    fn carrier(node: &HtmlNode) -> Option<&str> {
        if let HtmlNode::Element {
            attrs, children, ..
        } = node
        {
            attrs
                .get("data-semantic-slice")
                .map(String::as_str)
                .or_else(|| children.iter().find_map(carrier))
        } else {
            None
        }
    }
    let internal = internal.or_else(|| {
        parsed
            .as_ref()
            .and_then(carrier)
            .and_then(|raw| clipboard::decode_internal_clipboard(raw, &fingerprint))
    });
    DomHost::dispatch(
        host,
        EngineInput::Paste(PasteContent {
            internal,
            html: parsed,
            text: Some(text),
            plain_mode,
        }),
    );
}
fn emit_composer(host: &Host, event: crate::profiles::composer::ComposerEvent) {
    let cb = host.borrow().composer_callback.clone();
    if let Some(cb) = cb {
        cb(event);
    }
}
fn base64(bytes: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let a = c[0] as usize;
        let b = c.get(1).copied().unwrap_or(0) as usize;
        let d = c.get(2).copied().unwrap_or(0) as usize;
        out.push(T[a >> 2] as char);
        out.push(T[((a & 3) << 4) | (b >> 4)] as char);
        out.push(if c.len() > 1 {
            T[((b & 15) << 2) | (d >> 6)] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 { T[d & 63] as char } else { '=' });
    }
    out
}
fn upload_files(host: &Host, files: Vec<web_sys::File>, at: crate::InsertAt) {
    let composer = host.borrow().engine.borrow().profile() == crate::engine::Profile::Composer;
    let metadata: Vec<_> = files
        .iter()
        .map(|f| commands::media::FileMetadata {
            name: f.name(),
            mime: f.type_(),
            size: f.size() as u64,
        })
        .collect();
    let mut placeholders = commands::media::file_placeholders(&metadata);
    if !composer {
        for (file, placeholder) in files.iter().zip(&mut placeholders) {
            if file.type_().starts_with("image/")
                && file.size() as u64 <= commands::media::MAX_PASTED_FILE_BYTES
            {
                if let Ok(url) = web_sys::Url::create_object_url_with_blob(file.as_ref()) {
                    if let Some(upload) = placeholder
                        .transient
                        .get_mut("upload")
                        .and_then(serde_json::Value::as_object_mut)
                    {
                        upload.insert("preview_url".into(), serde_json::json!(url));
                    }
                }
            }
        }
    }
    if !composer {
        DomHost::run(
            host,
            Command::InsertBlocks {
                at,
                nodes: placeholders.iter().map(|p| p.node.clone()).collect(),
            },
        );
        for p in &placeholders {
            DomHost::run(
                host,
                Command::SetTransientAttrs {
                    uid: p.uid.clone(),
                    attrs: p.transient.clone(),
                },
            );
        }
    }
    let weak = std::rc::Rc::downgrade(host);
    wasm_bindgen_futures::spawn_local(async move {
        let mut items = vec![];
        for (file, p) in files.into_iter().zip(placeholders) {
            if !composer && file.size() as u64 > commands::media::MAX_PASTED_FILE_BYTES {
                continue;
            }
            match wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await {
                Ok(buffer) => {
                    let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
                    items.push(crate::PastedFile {
                        uid: p.uid,
                        name: file.name(),
                        mime: if file.type_().is_empty() {
                            "application/octet-stream".into()
                        } else {
                            file.type_()
                        },
                        size: file.size() as u64,
                        data_base64: base64(&bytes),
                    });
                }
                Err(_) => {
                    if let Some(host) = weak.upgrade() {
                        let mut attrs = crate::AttributeMap::new();
                        attrs.insert(
                            "upload".into(),
                            serde_json::json!({"status":"failed","error":"Could not read file"}),
                        );
                        DomHost::run(&host, Command::SetTransientAttrs { uid: p.uid, attrs });
                    }
                }
            }
        }
        if let Some(host) = weak.upgrade() {
            if composer {
                emit_composer(
                    &host,
                    crate::profiles::composer::ComposerEvent::PastedFiles(items),
                );
            } else if !items.is_empty() {
                DomHost::publish(
                    &host,
                    vec![EngineOutput::Request(crate::EngineEvent::FilesPasted {
                        items,
                    })],
                );
            }
        }
    });
}
pub fn cancel_drag(host: &Host) {
    let mut h = host.borrow_mut();
    h.drag_uids.clear();
    h.table_drag = None;
    h.drop_target = None;
    if let Some(ghost) = h.drag_ghost.take() {
        ghost.remove();
    }
    if let Some(el) = h.drag_indicator.take() {
        el.remove();
    }
}
pub fn install(host: &Host) -> Result<(), JsValue> {
    let root = host.borrow().root.clone();
    for (cut, name) in [(false, "copy"), (true, "cut")] {
        DomHost::listen(host, root.as_ref(), name, false, move |host, event| {
            if inside_island(&event, &host.borrow().root) {
                return;
            }
            if cut && host.borrow().engine.borrow().read_only() {
                event.prevent_default();
                return;
            }
            sync(host);
            if let Some(data) = event
                .dyn_ref::<ClipboardEvent>()
                .and_then(|e| e.clipboard_data())
            {
                if copy(host, &data) {
                    event.prevent_default();
                    if cut {
                        cut_selection(host);
                    }
                }
            }
        })?;
    }
    DomHost::listen(host, root.as_ref(), "paste", false, |host, event| {
        if inside_island(&event, &host.borrow().root) {
            return;
        }
        if let Some(data) = event
            .dyn_ref::<ClipboardEvent>()
            .and_then(|e| e.clipboard_data())
        {
            event.prevent_default();
            sync(host);
            paste(host, &data);
        }
    })?;
    // The handle belongs to Dioxus overlays, so the dragstart listener is on the wrapper.
    let wrapper = root
        .closest(".dxeditor,.dx-composer-editor")?
        .unwrap_or_else(|| root.clone());
    DomHost::listen(host, wrapper.as_ref(), "dragstart", false, |host, event| {
        if inside_island(&event, &host.borrow().root) {
            return;
        }
        if let Some(handle) = target_element(&event)
            .and_then(|element| element.closest("[data-table-row-drag]").ok().flatten())
        {
            if host.borrow().engine.borrow().read_only() {
                event.prevent_default();
                return;
            }
            if let (Some(table), Some(from)) = (
                handle.get_attribute("data-table-uid"),
                handle
                    .get_attribute("data-table-row-drag")
                    .and_then(|index| index.parse::<usize>().ok()),
            ) {
                host.borrow_mut().table_drag = Some((table.clone(), from));
                if let Some(data) = event
                    .dyn_ref::<DragEvent>()
                    .and_then(|event| event.data_transfer())
                {
                    data.set_effect_allowed("move");
                    let _ = data.set_data(
                        "application/x-semantic-table-row",
                        &serde_json::json!({"table":table,"from":from}).to_string(),
                    );
                }
            }
            return;
        }
        let handle = target_element(&event).and_then(|e| {
            e.closest("[data-block-drag],[data-dxeditor-drag]")
                .ok()
                .flatten()
        });
        let Some(handle) = handle else {
            event.prevent_default();
            return;
        };
        if host.borrow().engine.borrow().read_only() {
            event.prevent_default();
            return;
        }
        let Some(uid) = handle
            .get_attribute("data-block-drag")
            .or_else(|| handle.get_attribute("data-dxeditor-drag"))
        else {
            return;
        };
        let ids = {
            let h = host.borrow();
            let e = h.engine.borrow();
            let ids = commands::blocks::selected_uids(&e);
            if ids.contains(&uid) { ids } else { vec![uid] }
        };
        host.borrow_mut().drag_uids = ids.clone();
        if let Some(data) = event.dyn_ref::<DragEvent>().and_then(|e| e.data_transfer()) {
            data.set_effect_allowed("move");
            let _ = data.set_data(
                "application/x-semantic-blocks",
                &serde_json::to_string(&ids).unwrap(),
            );
            let source = host.borrow().renderer.blocks.get(&ids[0]).cloned();
            if let Some(source) = source {
                if let Ok(ghost) = source
                    .clone_node_with_deep(true)
                    .and_then(|node| node.dyn_into::<Element>().map_err(Into::into))
                {
                    let _ = ghost.remove_attribute("id");
                    if let Ok(nodes) = ghost.query_selector_all("[id]") {
                        for i in 0..nodes.length() {
                            if let Some(element) = nodes
                                .item(i)
                                .and_then(|node| node.dyn_into::<Element>().ok())
                            {
                                let _ = element.remove_attribute("id");
                            }
                        }
                    }
                    let _ = ghost.class_list().add_1("dx-block-drag-ghost");
                    let _ = ghost.set_attribute("aria-hidden", "true");
                    let _=ghost.set_attribute("style","position:fixed;left:-10000px;top:0;max-height:120px;width:min(640px,80vw);overflow:hidden;opacity:.6;pointer-events:none");
                    if let Some(body) = ghost.owner_document().and_then(|doc| doc.body()) {
                        let _ = body.append_child(&ghost);
                        data.set_drag_image(&ghost, 10, 10);
                        host.borrow_mut().drag_ghost = Some(ghost);
                    }
                } else {
                    data.set_drag_image(&source, 10, 10);
                }
            }
        }
    })?;
    DomHost::listen(host, root.as_ref(), "dragover", false, |host, event| {
        if inside_island(&event, &host.borrow().root) {
            return;
        }
        let Some(drag) = event.dyn_ref::<DragEvent>() else {
            return;
        };
        if host.borrow().engine.borrow().read_only() {
            return;
        }
        event.prevent_default();
        let row_drag = host.borrow().table_drag.clone();
        if let Some((table, _)) = row_drag {
            if let Some((_, _, rect, after)) = row_drop_target(host, &table, drag.client_y() as f64)
            {
                show_indicator(
                    host,
                    rect.x(),
                    if after { rect.bottom() } else { rect.top() },
                    rect.width(),
                );
            }
            autoscroll(drag.client_y() as f64);
            return;
        }
        let block =
            target_element(&event).and_then(|e| e.closest(".dx-block[data-uid]").ok().flatten());
        if let Some(block) = block {
            let Some(uid) = block.get_attribute("data-uid") else {
                return;
            };
            let r = block.get_bounding_client_rect();
            let inside = drag.client_x() as f64 > r.x() + r.width() / 2.
                && crate::engine::model::content(host.borrow().engine.borrow().document(), &uid)
                    .is_some_and(|n| {
                        host.borrow()
                            .engine
                            .borrow()
                            .schema()
                            .allows_children(&n.kind.0)
                    });
            let placement = if inside {
                BlockPlacement::Inside
            } else if (drag.client_y() as f64) < r.y() + r.height() / 2. {
                BlockPlacement::Before
            } else {
                BlockPlacement::After
            };
            let mut h = host.borrow_mut();
            h.drop_target = Some((uid, placement));
            let doc = block.owner_document().unwrap();
            let indicator = h.drag_indicator.get_or_insert_with(|| {
                let el = doc.create_element("div").unwrap();
                el.set_class_name("dx-block-drop-indicator");
                let _ = doc.body().unwrap().append_child(&el);
                el
            });
            let y = if placement == BlockPlacement::After {
                r.bottom()
            } else {
                r.top()
            };
            let _=indicator.set_attribute("style",&format!("position:fixed;pointer-events:none;z-index:10000;left:{}px;top:{y}px;width:{}px;height:2px;background:var(--accent,#5684e9)",r.x()+if inside{24.}else{0.},r.width()-if inside{24.}else{0.}));
        }
        if let Some(w) = web_sys::window() {
            let height = w
                .inner_height()
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(800.);
            let y = drag.client_y() as f64;
            if y < 48. {
                w.scroll_by_with_x_and_y(0., -16.);
            } else if y > height - 48. {
                w.scroll_by_with_x_and_y(0., 16.);
            }
        }
    })?;
    DomHost::listen(host, root.as_ref(), "drop", false, |host, event| {
        if inside_island(&event, &host.borrow().root) {
            return;
        }
        let Some(drag) = event.dyn_ref::<DragEvent>() else {
            return;
        };
        let Some(data) = drag.data_transfer() else {
            return;
        };
        event.prevent_default();
        if host.borrow().engine.borrow().read_only() {
            return;
        }
        if let Some(files) = data.files().filter(|f| f.length() > 0) {
            if !host.borrow().native_file_drop {
                event.stop_propagation();
                upload_files(
                    host,
                    (0..files.length()).filter_map(|i| files.item(i)).collect(),
                    crate::InsertAt::Coords {
                        x: drag.client_x() as f64,
                        y: drag.client_y() as f64,
                    },
                );
            }
            cancel_drag(host);
            return;
        }
        let row_drag = host.borrow().table_drag.clone().or_else(|| {
            let raw = data.get_data("application/x-semantic-table-row").ok()?;
            let json: serde_json::Value = serde_json::from_str(&raw).ok()?;
            Some((
                json.get("table")?.as_str()?.to_owned(),
                usize::try_from(json.get("from")?.as_u64()?).ok()?,
            ))
        });
        if let Some((table, from)) = row_drag {
            let target = row_drop_target(host, &table, drag.client_y() as f64);
            cancel_drag(host);
            if let Some((row, count, _, after)) = target {
                let boundary = row + usize::from(after);
                let to = boundary
                    .saturating_sub(usize::from(from < boundary))
                    .min(count.saturating_sub(1));
                DomHost::run(host, Command::SetSelection(EditorSelection::Node(table)));
                DomHost::run(
                    host,
                    Command::Table(commands::TableCommand::MoveRow { from, to }),
                );
            }
            return;
        }
        let custom = data
            .get_data("application/x-semantic-thread")
            .unwrap_or_default();
        if !custom.is_empty()
            && host.borrow().engine.borrow().profile() == crate::engine::Profile::Composer
        {
            let point = {
                let current = host.borrow();
                super::geometry::caret_from_point(
                    &current.root,
                    drag.client_x() as f64,
                    drag.client_y() as f64,
                )
            };
            if let Some(point) = point {
                DomHost::run(
                    host,
                    Command::SetSelection(EditorSelection::caret(point.container, point.offset)),
                );
            }
            emit_composer(
                host,
                crate::profiles::composer::ComposerEvent::Dropped {
                    mime: "application/x-semantic-thread".into(),
                    data: custom,
                },
            );
            return;
        }
        let ids = host.borrow().drag_uids.clone();
        let target = host.borrow().drop_target.clone();
        cancel_drag(host);
        if !ids.is_empty() {
            if let Some((target, placement)) = target {
                DomHost::run(
                    host,
                    Command::MoveBlocks {
                        uids: ids,
                        target,
                        placement,
                    },
                );
            }
        } else {
            let point = {
                let current = host.borrow();
                super::geometry::caret_from_point(
                    &current.root,
                    drag.client_x() as f64,
                    drag.client_y() as f64,
                )
            };
            if let Some(point) = point {
                DomHost::run(
                    host,
                    Command::SetSelection(EditorSelection::caret(point.container, point.offset)),
                );
            }
            paste(host, &data);
        }
    })?;
    DomHost::listen(host, wrapper.as_ref(), "dragend", false, |host, _| {
        cancel_drag(host)
    })?;
    Ok(())
}

fn row_drop_target(
    host: &Host,
    table: &str,
    y: f64,
) -> Option<(usize, usize, web_sys::DomRect, bool)> {
    let current = host.borrow();
    let table = current
        .root
        .query_selector(&format!(".dx-block[data-uid='{table}'] table"))
        .ok()??;
    let rows = table.query_selector_all("tr").ok()?;
    let count = rows.length() as usize;
    (0..count)
        .filter_map(|index| {
            let row = rows.item(index as u32)?.dyn_into::<Element>().ok()?;
            let rect = row.get_bounding_client_rect();
            let distance = if y < rect.top() {
                rect.top() - y
            } else if y > rect.bottom() {
                y - rect.bottom()
            } else {
                0.
            };
            Some((index, rect, distance))
        })
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(index, rect, _)| {
            let after = y >= rect.y() + rect.height() / 2.;
            (index, count, rect, after)
        })
}
fn show_indicator(host: &Host, x: f64, y: f64, width: f64) {
    let mut current = host.borrow_mut();
    if current.drag_indicator.is_none() {
        if let Some(document) = current.root.owner_document() {
            if let (Ok(element), Some(body)) = (document.create_element("div"), document.body()) {
                element.set_class_name("dx-block-drop-indicator");
                let _ = body.append_child(&element);
                current.drag_indicator = Some(element);
            }
        }
    }
    if let Some(indicator) = &current.drag_indicator {
        let _ = indicator.set_attribute("style", &format!("position:fixed;pointer-events:none;z-index:10000;left:{x}px;top:{y}px;width:{width}px;height:2px;background:var(--accent,#5684e9)"));
    }
}
fn autoscroll(y: f64) {
    if let Some(window) = web_sys::window() {
        let height = window
            .inner_height()
            .ok()
            .and_then(|value| value.as_f64())
            .unwrap_or(800.);
        if y < 48. {
            window.scroll_by_with_x_and_y(0., -16.);
        } else if y > height - 48. {
            window.scroll_by_with_x_and_y(0., 16.);
        }
    }
}
