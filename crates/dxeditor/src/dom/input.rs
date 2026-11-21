use super::{
    DomHost, Host, inside_island, is_form_control, mapping, selection_sync, target_element,
};
use crate::engine::{
    Command, EditorSelection, EngineInput, EngineOutput, RenderPlan, TextPoint, TextRange,
    input_types::{self, InputDisposition},
    keymap::{self, KeyChord},
};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Element, InputEvent, KeyboardEvent};
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
    if let Some(selection) = selection {
        let h = host.borrow();
        h.engine
            .borrow_mut()
            .handle(EngineInput::SelectionChanged(selection));
    }
}
fn target_range(host: &Host, event: &InputEvent) -> Option<TextRange> {
    let range = event
        .get_target_ranges()
        .get(0)
        .dyn_into::<web_sys::StaticRange>()
        .ok()?;
    let h = host.borrow();
    Some(TextRange {
        anchor: mapping::from_dom(&h.root, &range.start_container(), range.start_offset())?,
        head: mapping::from_dom(&h.root, &range.end_container(), range.end_offset())?,
    })
}
fn read_back(host: &Host) {
    let value = {
        let h = host.borrow();
        let container = h.composing.clone().or_else(|| {
            h.engine
                .borrow()
                .selection()
                .text_range()
                .map(|r| r.head.container.clone())
        });
        container.and_then(|container| {
            let point = TextPoint::new(container.clone(), 0);
            let (node, _) = mapping::to_dom(&h.root, &point)?;
            let (el, _) = mapping::container(&node, &h.root)?;
            let text = mapping::read_text(&el);
            let caret =
                selection_sync::read(&h.root).and_then(|s| s.text_range().map(|r| r.head.offset));
            Some((container, text, caret))
        })
    };
    if let Some((container, dom_text, dom_caret)) = value {
        DomHost::dispatch(
            host,
            EngineInput::CompositionEnd {
                container,
                dom_text,
                dom_caret,
            },
        );
    }
}
pub fn install(host: &Host) -> Result<(), JsValue> {
    let root = host.borrow().root.clone();
    let doc = root.owner_document().unwrap();
    let window = web_sys::window().unwrap();
    DomHost::listen(host, root.as_ref(), "beforeinput", false, |host, event| {
        if inside_island(&event, &host.borrow().root)
            || target_element(&event).as_ref().is_some_and(is_form_control)
        {
            return;
        }
        let Some(input) = event.dyn_ref::<InputEvent>() else {
            return;
        };
        if host.borrow().engine.borrow().read_only() {
            if event.cancelable() {
                event.prevent_default();
            }
            return;
        }
        if input.is_composing() || host.borrow().composing.is_some() {
            return;
        }
        sync(host);
        if super::composer::beforeinput(host, input) {
            return;
        }
        let disposition = input_types::translate(
            &input.input_type(),
            input.data().as_deref(),
            target_range(host, input),
            host.borrow().shift,
        );
        match disposition {
            InputDisposition::AllowComposition => {}
            InputDisposition::Input(input) => {
                if event.cancelable() {
                    event.prevent_default();
                    DomHost::dispatch(host, input);
                }
            }
            InputDisposition::Command(command) => {
                if event.cancelable() {
                    event.prevent_default();
                    DomHost::run(host, command);
                }
            }
            InputDisposition::HandledElsewhere | InputDisposition::Ignore => {
                if event.cancelable() {
                    event.prevent_default();
                }
            }
        }
    })?;
    DomHost::listen(host, root.as_ref(), "input", false, |host, event| {
        if inside_island(&event, &host.borrow().root) {
            return;
        }
        let composing = event
            .dyn_ref::<InputEvent>()
            .is_some_and(|e| e.is_composing());
        if composing {
            if host.borrow().composing.is_none() {
                sync(host);
                let c = host
                    .borrow()
                    .engine
                    .borrow()
                    .selection()
                    .text_range()
                    .map(|r| r.head.container.clone());
                host.borrow_mut().composing = c;
                DomHost::dispatch(host, EngineInput::CompositionStart);
            }
            return;
        }
        if host.borrow().composing.is_none() {
            read_back(host);
        }
    })?;
    DomHost::listen(
        host,
        root.as_ref(),
        "compositionstart",
        false,
        |host, event| {
            if inside_island(&event, &host.borrow().root) {
                return;
            }
            sync(host);
            let c = host
                .borrow()
                .engine
                .borrow()
                .selection()
                .text_range()
                .map(|r| r.head.container.clone());
            host.borrow_mut().composing = c;
            DomHost::dispatch(host, EngineInput::CompositionStart);
        },
    )?;
    DomHost::listen(
        host,
        root.as_ref(),
        "compositionend",
        false,
        |host, event| {
            if inside_island(&event, &host.borrow().root) {
                return;
            }
            host.borrow_mut().last_composition = dxutil::time::now_ms();
            read_back(host);
            host.borrow_mut().composing = None;
            let uid = host
                .borrow()
                .engine
                .borrow()
                .selection()
                .text_range()
                .and_then(|r| {
                    crate::engine::commands::blocks::owner(
                        &host.borrow().engine.borrow(),
                        &r.head.container,
                    )
                });
            if let Some(uid) = uid {
                host.borrow_mut().renderer.invalidate(&uid);
                let selection = host.borrow().engine.borrow().selection().clone();
                DomHost::publish(
                    host,
                    vec![
                        EngineOutput::Render(RenderPlan {
                            dirty: vec![uid],
                            structure_changed: false,
                            full: false,
                        }),
                        EngineOutput::ApplySelection(selection),
                    ],
                );
            }
        },
    )?;
    DomHost::listen(host, root.as_ref(), "keydown", false, |host, event| {
        let Some(key) = event.dyn_ref::<KeyboardEvent>() else {
            return;
        };
        // Form controls inside the content (a code block's language picker)
        // handle their own keys.
        if target_element(&event).as_ref().is_some_and(is_form_control)
            && !inside_island(&event, &host.borrow().root)
        {
            return;
        }
        let root = host.borrow().root.clone();
        if inside_island(&event, &root) {
            if key.key() == "Escape" {
                if let Some(uid) = target_element(&event)
                    .and_then(|el| el.closest("[data-island-uid]").ok().flatten())
                    .and_then(|el| el.get_attribute("data-island-uid"))
                {
                    event.prevent_default();
                    event.stop_propagation();
                    DomHost::run(host, Command::SelectNode(uid));
                    DomHost::run(host, Command::Focus);
                }
            }
            return;
        }
        host.borrow_mut().shift = key.shift_key();
        if key.is_composing() || key.key_code() == 229 || host.borrow().composing.is_some() {
            return;
        }
        let now = dxutil::time::now_ms();
        if key.key() == "Enter"
            && host.borrow().safari
            && now - host.borrow().last_composition < 500.
        {
            event.prevent_default();
            return;
        }
        sync(host);
        if host.borrow().engine.borrow().read_only() {
            if matches!(key.key().as_str(), "Enter" | "Backspace" | "Delete" | "Tab")
                || ((key.ctrl_key() || key.meta_key())
                    && !matches!(key.key().to_lowercase().as_str(), "a" | "c"))
            {
                event.prevent_default();
            }
            return;
        }
        let bare = !key.ctrl_key() && !key.meta_key() && !key.alt_key() && !key.shift_key();
        if bare && matches!(key.key().as_str(), "Tab" | "Enter") {
            let configured = root
                .closest("[data-dxeditor-paragraph-shortcuts]")
                .ok()
                .flatten()
                .and_then(|e| e.get_attribute("data-dxeditor-paragraph-shortcuts"))
                .is_some_and(|v| v.split_whitespace().any(|k| k == key.key().to_lowercase()));
            if configured {
                if keymap::host_paragraph_shortcut_eligible(&host.borrow().engine.borrow()) {
                    event.prevent_default();
                    return;
                }
                event.stop_propagation();
            }
        }
        if key.key() == "Escape"
            && (!host.borrow().drag_uids.is_empty() || host.borrow().table_drag.is_some())
        {
            event.prevent_default();
            super::clipboard::cancel_drag(host);
            return;
        }
        let chord = KeyChord {
            key: key.key(),
            ctrl: key.ctrl_key(),
            meta: key.meta_key(),
            alt: key.alt_key(),
            shift: key.shift_key(),
            composing: false,
        };
        let action = keymap::resolve(&host.borrow().engine.borrow(), &chord);
        if action.is_some() {
            event.prevent_default();
            if host.borrow().engine.borrow().suggestion().active.is_some() {
                event.stop_propagation();
            }
            if matches!(action, Some(keymap::KeyAction::PlainPaste)) {
                host.borrow_mut().plain_paste = true;
            }
            DomHost::dispatch(host, EngineInput::Key(chord));
        }
    })?;
    DomHost::listen(host, root.as_ref(), "keyup", false, |host, event| {
        if let Some(key) = event.dyn_ref::<KeyboardEvent>() {
            host.borrow_mut().shift = key.shift_key();
        }
    })?;
    DomHost::listen(host, doc.as_ref(), "selectionchange", false, |host, _| {
        let current = {
            let h = host.borrow();
            if h.composing.is_some()
                || !h.drag_uids.is_empty()
                || h.root
                    .class_list()
                    .contains("dxeditor-engine--hide-selection")
            {
                return;
            }
            selection_sync::read(&h.root)
        };
        if let Some(selection) = current {
            if host.borrow().engine.borrow().selection() != &selection {
                DomHost::dispatch(host, EngineInput::SelectionChanged(selection));
            }
        }
    })?;
    let focus_owner = root
        .closest(".dxeditor,.dx-composer-editor")?
        .unwrap_or_else(|| root.clone());
    for (name, focused) in [("focusin", true), ("focusout", false)] {
        DomHost::listen(
            host,
            focus_owner.as_ref(),
            name,
            false,
            move |host, event| {
                if inside_island(&event, &host.borrow().root) {
                    return;
                }
                if !focused {
                    let moving_inside = event
                        .dyn_ref::<web_sys::FocusEvent>()
                        .and_then(|event| event.related_target())
                        .and_then(|target| target.dyn_into::<Element>().ok())
                        .is_some_and(|target| host.borrow().owns_focus(&target));
                    if moving_inside {
                        return;
                    }
                }
                let output = host.borrow().engine.borrow_mut().set_focused(focused);
                DomHost::publish(host, output);
            },
        )?;
    }
    DomHost::listen(host, root.as_ref(), "click", false, |host, event| {
        let root = host.borrow().root.clone();
        if inside_island(&event, &root) {
            return;
        }
        let Some(target) = target_element(&event) else {
            return;
        };
        if let Some(uid) = target
            .closest("[data-todo]")
            .ok()
            .flatten()
            .and_then(|e| e.get_attribute("data-todo"))
        {
            event.prevent_default();
            if !host.borrow().engine.borrow().read_only() {
                // Ticking a to-do leaves the selection where it was rather
                // than selecting the whole block.
                let previous = host.borrow().engine.borrow().selection().clone();
                DomHost::run(
                    host,
                    Command::SetSelection(EditorSelection::Node(uid.clone())),
                );
                DomHost::run(host, Command::ToggleChecked);
                let restored = if matches!(previous, EditorSelection::Text(_)) {
                    previous
                } else {
                    restore_caret_at_end(host, uid)
                };
                DomHost::run(host, Command::SetSelection(restored));
                focus_in_place(host);
            } else {
                DomHost::publish(
                    host,
                    vec![EngineOutput::Render(RenderPlan {
                        full: true,
                        structure_changed: true,
                        dirty: vec![],
                    })],
                );
            }
            return;
        }
        if let Some(uid) = target
            .closest("[data-toggle]")
            .ok()
            .flatten()
            .and_then(|e| e.get_attribute("data-toggle"))
        {
            event.prevent_default();
            DomHost::run(host, Command::ToggleCollapsed(uid));
            // Keyboard focus stays in the text rather than on the arrow.
            focus_in_place(host);
            return;
        }
        if let Some(chip) = target.closest("[data-composer-chip]").ok().flatten() {
            event.prevent_default();
            let raw = chip.get_attribute("data-composer-chip").unwrap_or_default();
            if let Ok(attrs) = serde_json::from_str::<crate::AttributeMap>(&raw) {
                let value = crate::profiles::composer::ComposerChip {
                    kind: attrs
                        .get("kind")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .into(),
                    label: attrs
                        .get("label")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .into(),
                    payload: attrs
                        .get("payload")
                        .and_then(|v| v.as_str())
                        .and_then(|s| serde_json::from_str(s).ok())
                        .unwrap_or(serde_json::Value::Null),
                };
                let remove = target
                    .closest("[data-chip-remove]")
                    .ok()
                    .flatten()
                    .is_some();
                if remove {
                    if host.borrow().engine.borrow().read_only() {
                        return;
                    }
                    if let Some(parent) = chip.parent_node() {
                        let index = (0..parent.child_nodes().length())
                            .find(|&i| parent.child_nodes().item(i).as_ref() == Some(chip.as_ref()))
                            .unwrap_or(0);
                        if let (Some(start), Some(end)) = (
                            mapping::from_dom(&root, &parent, index),
                            mapping::from_dom(&root, &parent, index + 1),
                        ) {
                            DomHost::run(
                                host,
                                Command::ReplaceRange {
                                    range: TextRange {
                                        anchor: start,
                                        head: end,
                                    },
                                    text: String::new(),
                                },
                            );
                        }
                    }
                }
                let callback = host.borrow().composer_callback.clone();
                if let Some(callback) = callback {
                    callback(if remove {
                        crate::profiles::composer::ComposerEvent::ChipRemoved(value)
                    } else {
                        crate::profiles::composer::ComposerEvent::ChipActivated(value)
                    });
                }
            }
            return;
        }
        if let Some(link) = target.closest("a[href]").ok().flatten() {
            let href = link.get_attribute("href").unwrap_or_default();
            if let Some(id) = href.strip_prefix("semantic:entity:") {
                event.prevent_default();
                if !link.has_attribute("data-deleted") {
                    DomHost::publish(
                        host,
                        vec![EngineOutput::EntityOpen {
                            entity_id: id.into(),
                        }],
                    );
                }
            } else if let Some(win) = web_sys::window() {
                event.prevent_default();
                // While editing, a plain click places the caret in the link (its
                // popover offers Open); a modified click opens it directly.
                let modified = event
                    .dyn_ref::<web_sys::MouseEvent>()
                    .is_some_and(|mouse| mouse.ctrl_key() || mouse.meta_key());
                let editable = !host.borrow().engine.borrow().read_only();
                if crate::is_safe_url(&href) && (modified || !editable) {
                    let _ = win.open_with_url_and_target(&href, "_blank");
                }
            }
            return;
        }
        if target.tag_name() == "IMG" || target.class_list().contains("dx-doc-image-upload") {
            if let Some(block) = target.closest(".dx-block[data-uid]").ok().flatten() {
                if let Some(uid) = block.get_attribute("data-uid") {
                    DomHost::run(host, Command::SelectNode(uid.clone()));
                    DomHost::run(host, Command::Focus);
                    // While editing, a click selects the image for its toolbar;
                    // a double click (or any click when reading) opens it.
                    if host.borrow().engine.borrow().read_only() {
                        open_media(host, uid);
                    }
                }
            }
        }
    })?;
    DomHost::listen(host, root.as_ref(), "dblclick", false, |host, event| {
        if inside_island(&event, &host.borrow().root) {
            return;
        }
        if let Some(uid) = target_element(&event)
            .filter(|target| target.tag_name() == "IMG")
            .and_then(|target| target.closest(".dx-block[data-uid]").ok().flatten())
            .and_then(|block| block.get_attribute("data-uid"))
        {
            event.prevent_default();
            open_media(host, uid);
        }
    })?;
    DomHost::listen(host, root.as_ref(), "change", false, |host, event| {
        if inside_island(&event, &host.borrow().root) {
            return;
        }
        let Some(select) = target_element(&event)
            .and_then(|target| target.dyn_into::<web_sys::HtmlSelectElement>().ok())
        else {
            return;
        };
        let Some(uid) = select.get_attribute("data-code-language") else {
            return;
        };
        let value = select.value();
        let mut attrs = crate::AttributeMap::new();
        attrs.insert(
            "language".into(),
            if value.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::Value::String(value)
            },
        );
        DomHost::run(
            host,
            Command::UpdateNodeAttrs {
                uid,
                attrs,
                add_to_history: true,
            },
        );
    })?;
    DomHost::listen(host, root.as_ref(), "pointerdown", false, |host, event| {
        if inside_island(&event, &host.borrow().root) {
            return;
        }
        host.borrow_mut().cell_drag = cell_at(&event);
        if let Some(target) = target_element(&event) {
            if let Some(content) = target
                .closest("[data-content-kind='divider'],[data-content-kind='opaque_markdown']")
                .ok()
                .flatten()
            {
                if let Some(uid) = content
                    .closest(".dx-block[data-uid]")
                    .ok()
                    .flatten()
                    .and_then(|el| el.get_attribute("data-uid"))
                {
                    event.prevent_default();
                    DomHost::run(host, Command::SelectNode(uid));
                    return;
                }
            }
        }
        if let Some(el) = target_element(&event) {
            if el
                .closest("[data-inline-container]")
                .ok()
                .flatten()
                .is_some()
            {
                let _ = host
                    .borrow()
                    .root
                    .class_list()
                    .remove_1("dxeditor-engine--hide-selection");
            }
        }
        if let Some(mouse) = event.dyn_ref::<web_sys::MouseEvent>() {
            if !mouse.shift_key() && !target_element(&event).as_ref().is_some_and(is_form_control) {
                if let Some(inline) = target_element(&event)
                    .and_then(|el| {
                        el.closest("[data-content-kind='code_block']")
                            .ok()
                            .flatten()
                    })
                    .and_then(|pre| pre.query_selector("[data-inline-container]").ok().flatten())
                {
                    let root = host.borrow().root.clone();
                    if let Some(container) =
                        inline
                            .get_attribute("data-inline-container")
                            .and_then(|raw| {
                                serde_json::from_str::<crate::engine::InlineContainerRef>(&raw).ok()
                            })
                    {
                        let point = super::geometry::caret_from_point(
                            &root,
                            mouse.client_x() as f64,
                            mouse.client_y() as f64,
                        )
                        .filter(|point| point.container == container)
                        .unwrap_or_else(|| {
                            crate::engine::TextPoint::new(
                                container,
                                mapping::read_text(&inline).chars().count(),
                            )
                        });
                        event.prevent_default();
                        DomHost::run(
                            host,
                            Command::SetSelection(EditorSelection::caret(
                                point.container,
                                point.offset,
                            )),
                        );
                        DomHost::run(host, Command::Focus);
                    }
                }
            }
        }
        if let Some(mouse) = event.dyn_ref::<web_sys::MouseEvent>() {
            if mouse.shift_key() {
                if let Some(uid) = target_element(&event)
                    .and_then(|e| e.closest(".dx-block[data-uid]").ok().flatten())
                    .and_then(|e| e.get_attribute("data-uid"))
                {
                    let selection = host.borrow().engine.borrow().selection().clone();
                    if let EditorSelection::Blocks { anchor, .. } = selection {
                        event.prevent_default();
                        DomHost::run(
                            host,
                            Command::SetSelection(EditorSelection::Blocks { anchor, head: uid }),
                        );
                    }
                }
            }
        }
    })?;
    DomHost::listen(host, root.as_ref(), "pointermove", false, |host, event| {
        if !host.borrow().drag_uids.is_empty() {
            return;
        }
        let inside = inside_island(&event, &host.borrow().root);
        let cell = if inside { None } else { cell_at(&event) };
        let anchor = host.borrow().cell_drag.clone();
        if event
            .dyn_ref::<web_sys::MouseEvent>()
            .is_some_and(|e| e.buttons() == 1)
        {
            if let (Some((table, anchor)), Some((target, head))) = (anchor, cell.clone()) {
                if table == target && anchor != head {
                    event.prevent_default();
                    DomHost::run(
                        host,
                        Command::SetSelection(EditorSelection::Cells {
                            table,
                            anchor,
                            head,
                        }),
                    );
                    return;
                }
            }
        }
        // Inside an island (a card, a graph) the island's own block is hovered.
        let uid = target_element(&event)
            .map(|target| {
                target
                    .closest(".dxeditor-island")
                    .ok()
                    .flatten()
                    .unwrap_or(target)
            })
            .and_then(|e| e.closest(".dx-block[data-uid]").ok().flatten())
            .and_then(|e| e.get_attribute("data-uid"));
        let cell_changed = {
            let mut host = host.borrow_mut();
            let changed = host.hovered_cell != cell;
            host.hovered_cell = cell;
            changed
        };
        let output = host.borrow().engine.borrow_mut().set_hover_block(uid);
        if cell_changed || !output.is_empty() {
            DomHost::publish(host, output);
        }
    })?;
    // A click below the last block continues writing there: at the end of a
    // text block, or in a new paragraph after a non-text block (an embed).
    DomHost::listen(
        host,
        focus_owner.as_ref(),
        "pointerdown",
        false,
        |host, event| {
            let root = host.borrow().root.clone();
            let Some(mouse) = event.dyn_ref::<web_sys::MouseEvent>() else {
                return;
            };
            if mouse.button() != 0
                || host.borrow().engine.borrow().read_only()
                || host.borrow().engine.borrow().profile() != crate::engine::Profile::BlockDocument
                || target_element(&event).is_some_and(|target| {
                    target != root && root.contains(Some(&target)) || target.closest(".dxeditor__overlays,.dxeditor-engine__surface,.dxeditor-gutter,.dxeditor-table-edges").ok().flatten().is_some()
                })
            {
                return;
            }
            let Some(last) = root
                .query_selector(":scope > .dx-block-group > .dx-block:last-child")
                .ok()
                .flatten()
            else {
                return;
            };
            let bounds = last.get_bounding_client_rect();
            let y = f64::from(mouse.client_y());
            let x = f64::from(mouse.client_x());
            if y <= bounds.bottom() || x < bounds.left() || x > bounds.right() {
                return;
            }
            let Some(uid) = last.get_attribute("data-uid") else {
                return;
            };
            event.prevent_default();
            // The end of a text block, else a new paragraph after the block.
            let text_end = {
                let host = host.borrow();
                let engine = host.engine.borrow();
                let container = crate::engine::InlineContainerRef::Block(uid.clone());
                engine
                    .content(&uid)
                    .filter(|content| crate::engine::model::is_text_kind(&content.kind.0))
                    .map(|_| {
                        let length = engine
                            .inline(&container)
                            .map_or(0, |nodes| crate::engine::text::len(nodes));
                        EditorSelection::caret(container, length)
                    })
            };
            match text_end {
                Some(caret) => {
                    DomHost::run(host, Command::SetSelection(caret));
                    DomHost::run(host, Command::Focus);
                }
                None => DomHost::run(host, Command::FocusAfterBlock(uid)),
            }
        },
    )?;
    // The gutter controls sit in the editor's margin, outside the content.
    // Moving along that margin hovers the block at the pointer's height.
    DomHost::listen(
        host,
        focus_owner.as_ref(),
        "pointermove",
        false,
        |host, event| {
            let root = host.borrow().root.clone();
            if !host.borrow().drag_uids.is_empty()
                || target_element(&event).is_some_and(|target| {
                    root.contains(Some(&target))
                        || target
                            .closest(".dxeditor__overlays,.dxeditor-engine__surface")
                            .ok()
                            .flatten()
                            .is_some()
                })
            {
                return;
            }
            let Some(mouse) = event.dyn_ref::<web_sys::MouseEvent>() else {
                return;
            };
            if mouse.buttons() != 0 {
                return;
            }
            let bounds = root.get_bounding_client_rect();
            let y = f64::from(mouse.client_y());
            if y < bounds.top() || y > bounds.bottom() {
                return;
            }
            let Some(document) = root.owner_document() else {
                return;
            };
            let uid = document
                .element_from_point((bounds.left() + bounds.width() / 2.) as f32, y as f32)
                .filter(|element| root.contains(Some(element)))
                .and_then(|element| element.closest(".dx-block[data-uid]").ok().flatten())
                .and_then(|block| block.get_attribute("data-uid"));
            if uid.is_some() {
                let output = host.borrow().engine.borrow_mut().set_hover_block(uid);
                DomHost::publish(host, output);
            }
        },
    )?;
    DomHost::listen(
        host,
        focus_owner.as_ref(),
        "pointerleave",
        false,
        |host, _| {
            if !host.borrow().drag_uids.is_empty() {
                return;
            }
            host.borrow_mut().hovered_cell = None;
            let output = host.borrow().engine.borrow_mut().set_hover_block(None);
            DomHost::publish(host, output);
        },
    )?;
    DomHost::listen(host, doc.as_ref(), "pointerup", false, |host, _| {
        host.borrow_mut().cell_drag = None;
    })?;
    for name in ["mouseover", "focusin"] {
        DomHost::listen(host, root.as_ref(), name, false, |host, event| {
            if inside_island(&event, &host.borrow().root) {
                return;
            }
            if let Some(el) = target_element(&event).and_then(|e| {
                e.closest("[data-semantic-mention],[data-entity-id]")
                    .ok()
                    .flatten()
            }) {
                if let Some(entity_id) = el
                    .get_attribute("data-semantic-mention")
                    .or_else(|| el.get_attribute("data-entity-id"))
                {
                    DomHost::publish(host, vec![EngineOutput::QueryEntityPreview { entity_id }]);
                }
            }
        })?;
    }
    for name in ["resize", "scroll"] {
        DomHost::listen(host, window.as_ref(), name, true, |host, _| {
            if host.borrow().engine.borrow().ui_state().focused {
                DomHost::publish(host, vec![]);
            }
        })?;
    }
    if let Some(v) = window.visual_viewport() {
        for name in ["resize", "scroll"] {
            DomHost::listen(host, v.as_ref(), name, false, |host, _| {
                if host.borrow().engine.borrow().ui_state().focused {
                    DomHost::publish(host, vec![]);
                }
            })?;
        }
    }
    // Pointer and keyboard navigation away from any editor dismiss its overlays
    // without stealing focus from another editor, popover, or host control.
    for name in ["pointerdown", "focusin"] {
        DomHost::listen(host, doc.as_ref(), name, false, |host, event| {
            if target_element(&event).is_some_and(|el| !host.borrow().owns_focus(&el)) {
                let output = host.borrow().engine.borrow_mut().dismiss_overlays();
                if !output.is_empty() {
                    DomHost::publish(host, output);
                }
            }
        })?;
    }
    Ok(())
}

fn cell_at(event: &web_sys::Event) -> Option<(String, (usize, usize))> {
    let cell = target_element(event)?.closest("td,th").ok()??;
    let row = cell.parent_element()?;
    let table = row.closest("table").ok()??;
    let uid = table
        .closest(".dx-block[data-uid]")
        .ok()??
        .get_attribute("data-uid")?;
    let r = row.get_attribute("data-table-row-index")?.parse().ok()?;
    let c = cell
        .get_attribute("data-table-column-index")?
        .parse()
        .ok()?;
    Some((uid, (r, c)))
}

/// A caret at the end of block `uid`'s text.
/// Returns focus to the text after a click on a control inside it (a to-do's
/// box, a toggle's arrow) without scrolling to the caret, which may lie
/// outside the viewport.
fn focus_in_place(host: &Host) {
    DomHost::run(host, Command::Focus);
    host.borrow_mut().scroll_selection = false;
}

fn restore_caret_at_end(host: &Host, uid: String) -> EditorSelection {
    let container = crate::engine::InlineContainerRef::Block(uid);
    let length = host
        .borrow()
        .engine
        .borrow()
        .inline(&container)
        .map_or(0, |nodes| crate::engine::text::len(nodes));
    EditorSelection::caret(container, length)
}

/// Asks the host to show block `uid`'s media.
fn open_media(host: &Host, uid: String) {
    let media = crate::engine::model::content(host.borrow().engine.borrow().document(), &uid)
        .and_then(|n| n.attrs.get("media"))
        .and_then(|v| v.as_object())
        .cloned();
    DomHost::publish(
        host,
        vec![EngineOutput::Request(crate::EngineEvent::MediaOpen {
            uid,
            media,
        })],
    );
}
