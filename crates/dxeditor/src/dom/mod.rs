//! Browser adapter. The engine owns no browser objects and remains native-testable.
mod clipboard;
mod composer;
pub mod geometry;
mod input;
pub mod islands;
mod listener;
pub mod mapping;
pub mod media;
pub mod render;
pub mod selection_sync;
use crate::engine::{
    Command, EditorEngine, EngineInput, EngineOutput, FocusTarget, InlineContainerRef, RenderPlan,
};
use listener::Listener;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{Element, Event, MutationObserver, MutationObserverInit};
pub type Host = Rc<RefCell<DomHost>>;
pub struct DomHost {
    pub root: Element,
    pub engine: Rc<RefCell<EditorEngine>>,
    pub(crate) renderer: render::Renderer,
    pub(crate) composing: Option<InlineContainerRef>,
    pub(crate) shift: bool,
    pub(crate) last_composition: f64,
    pub(crate) safari: bool,
    pub(crate) plain_paste: bool,
    pub(crate) max_inline_paste_chars: usize,
    pub(crate) drag_uids: Vec<String>,
    pub(crate) drop_target: Option<(String, crate::engine::commands::BlockPlacement)>,
    listeners: Vec<Listener>,
    observer: Option<MutationObserver>,
    observer_callback: Option<Closure<dyn FnMut(js_sys::Array, MutationObserver)>>,
    resize: Option<web_sys::ResizeObserver>,
    resize_callback: Option<Closure<dyn FnMut(js_sys::Array, web_sys::ResizeObserver)>>,
    session: String,
    callback: Rc<dyn Fn(Vec<EngineOutput>)>,
    pub(crate) composer_callback: Option<Rc<dyn Fn(crate::profiles::composer::ComposerEvent)>>,
    visibility_callback: Option<Rc<dyn Fn(String, bool)>>,
    intersection: Option<web_sys::IntersectionObserver>,
    intersection_callback: Option<Closure<dyn FnMut(js_sys::Array, web_sys::IntersectionObserver)>>,
    media: media::Media,
    pending: Vec<EngineOutput>,
    destroyed: bool,
    pub(crate) native_file_drop: bool,
    pub(crate) drag_indicator: Option<Element>,
    pub(crate) drag_ghost: Option<Element>,
    pub(crate) cell_drag: Option<(String, (usize, usize))>,
    pub(crate) table_drag: Option<(String, usize)>,
    frame: Option<i32>,
    last_geometry: crate::engine::ui_state::Geometry,
}
impl DomHost {
    pub fn mount(
        host: Element,
        engine: Rc<RefCell<EditorEngine>>,
        callback: Rc<dyn Fn(Vec<EngineOutput>)>,
    ) -> Result<Host, JsValue> {
        if let Some(window) = web_sys::window() {
            engine.borrow_mut().options.origin = window.location().origin().ok();
            let platform = window
                .navigator()
                .platform()
                .unwrap_or_default()
                .to_lowercase();
            engine.borrow_mut().options.mac = platform.contains("mac")
                || platform.contains("iphone")
                || platform.contains("ipad");
        }
        if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
            if let Some(raw) = storage.get_item("dxeditor-recent-blocks").ok().flatten() {
                if let Ok(ids) = serde_json::from_str(&raw) {
                    engine.borrow_mut().set_recent_blocks(ids);
                }
            }
            if let Some(raw) = storage.get_item("dxeditor-recent-mentions").ok().flatten() {
                if let Ok(candidates) = serde_json::from_str(&raw) {
                    engine.borrow_mut().set_recent_mentions(candidates);
                }
            }
            if let Some(color) = storage.get_item("dxeditor-last-highlight").ok().flatten() {
                engine.borrow_mut().set_last_highlight(color);
            }
        }
        let doc = host
            .owner_document()
            .ok_or_else(|| JsValue::from_str("Editor host has no document"))?;
        let root = doc.create_element("div")?;
        let composer = engine.borrow().profile() == crate::engine::Profile::Composer;
        root.set_class_name(if composer {
            "dxeditor-engine__content dx-composer-content"
        } else {
            "dxeditor-engine__content dxeditor-engine__blocks"
        });
        root.set_attribute(
            "contenteditable",
            if engine.borrow().read_only() {
                "false"
            } else {
                "true"
            },
        )?;
        root.set_attribute("role", "textbox")?;
        root.set_attribute("aria-multiline", "true")?;
        root.set_attribute("aria-label", "Document editor")?;
        root.set_attribute("translate", "no")?;
        root.set_attribute("spellcheck", "true")?;
        root.set_attribute("tabindex", "0")?;
        let session = host
            .closest("[data-dxeditor-session]")?
            .and_then(|e| e.get_attribute("data-dxeditor-session"))
            .unwrap_or_default();
        let native_file_drop =
            host.get_attribute("data-native-file-drop").as_deref() == Some("true");
        host.set_text_content(None);
        host.append_child(&root)?;
        let result = Rc::new(RefCell::new(Self {
            root,
            engine,
            renderer: render::Renderer::default(),
            composing: None,
            shift: false,
            last_composition: -1000.,
            safari: web_sys::window()
                .and_then(|w| w.navigator().user_agent().ok())
                .is_some_and(|s| {
                    s.contains("Safari")
                        && !s.contains("Chrome")
                        && !s.contains("Chromium")
                        && !s.contains("Android")
                }),
            plain_paste: false,
            max_inline_paste_chars: 5000,
            drag_uids: vec![],
            drop_target: None,
            listeners: vec![],
            observer: None,
            observer_callback: None,
            resize: None,
            resize_callback: None,
            session,
            callback,
            composer_callback: None,
            visibility_callback: None,
            intersection: None,
            intersection_callback: None,
            media: media::Media::default(),
            pending: vec![],
            destroyed: false,
            native_file_drop,
            drag_indicator: None,
            drag_ghost: None,
            cell_drag: None,
            table_drag: None,
            frame: None,
            last_geometry: crate::engine::ui_state::Geometry::default(),
        }));
        {
            let mut h = result.borrow_mut();
            let plan = RenderPlan {
                full: true,
                structure_changed: true,
                dirty: vec![],
            };
            let root = h.root.clone();
            let engine = h.engine.clone();
            h.renderer.render(&root, &engine.borrow(), &plan, None)?;
        }
        input::install(&result)?;
        clipboard::install(&result)?;
        media::install(&result)?;
        Self::observe(&result)?;
        {
            let mut h = result.borrow_mut();
            islands::adopt_all(&h.root, &h.session);
            let root = h.root.clone();
            let engine = h.engine.clone();
            let requests = h.media.refresh(&root, &engine.borrow(), None);
            h.pending.extend(requests);
        }
        let initial = result.borrow().engine.borrow().ui_state();
        Self::publish(&result, vec![EngineOutput::UiState(initial)]);
        Ok(result)
    }
    pub fn dispatch(host: &Host, input: EngineInput) {
        let measured = now_ms();
        let output = {
            let Ok(h) = host.try_borrow() else { return };
            if h.destroyed {
                return;
            }
            let mut e = h.engine.borrow_mut();
            if let Some(now) = web_sys::window()
                .and_then(|w| w.performance())
                .map(|p| p.now())
            {
                e.set_time_ms(now as u64);
            }
            if let EngineInput::Command(Command::InsertBlocks {
                at: crate::InsertAt::Coords { x, y },
                mut nodes,
            }) = input
            {
                let nearest = h
                    .renderer
                    .blocks
                    .iter()
                    .filter_map(|(uid, el)| {
                        let rect = el.get_bounding_client_rect();
                        (rect.height() > 0.).then_some((uid, rect))
                    })
                    .min_by(|(_, a), (_, b)| {
                        let da = if y < a.top() {
                            a.top() - y
                        } else if y > a.bottom() {
                            y - a.bottom()
                        } else {
                            0.
                        };
                        let db = if y < b.top() {
                            b.top() - y
                        } else if y > b.bottom() {
                            y - b.bottom()
                        } else {
                            0.
                        };
                        da.total_cmp(&db)
                    });
                if let Some((uid, rect)) = nearest {
                    let (parent, index) =
                        crate::engine::model::location(e.document(), uid).unwrap();
                    for node in &mut nodes {
                        if node.kind.0 != "block" {
                            *node = crate::engine::model::block_node(node.clone());
                        }
                    }
                    let selection = nodes
                        .last()
                        .and_then(|n| n.id.as_ref())
                        .map(|id| crate::engine::EditorSelection::Node(id.0.clone()))
                        .unwrap_or_else(|| e.selection().clone());
                    let tx = crate::engine::Transaction::new(
                        vec![crate::engine::Op::InsertBlocks {
                            parent,
                            index: index + usize::from(y >= rect.y() + rect.height() / 2.),
                            blocks: nodes,
                        }],
                        e.selection().clone(),
                        selection,
                        crate::engine::TxKind::Paste,
                    );
                    let _ = x;
                    e.transact(tx).unwrap_or_default()
                } else {
                    e.run(Command::InsertBlocks {
                        at: crate::InsertAt::Selection,
                        nodes,
                    })
                }
            } else {
                e.handle(input)
            }
        };
        let elapsed = now_ms() - measured;
        if host.borrow().root.has_attribute("data-measure-timings") {
            let _ = host
                .borrow()
                .root
                .set_attribute("data-perf-engine-ms", &elapsed.to_string());
        }
        Self::publish(host, output);
    }
    pub fn run(host: &Host, command: Command) {
        Self::dispatch(host, EngineInput::Command(command));
    }
    pub fn publish(host: &Host, mut output: Vec<EngineOutput>) {
        let measured = now_ms();
        let callback = {
            let Ok(mut h) = host.try_borrow_mut() else {
                return;
            };
            if h.destroyed {
                return;
            }
            if let Err(err) = h.refresh(&output) {
                output.push(EngineOutput::Request(crate::EngineEvent::Error {
                    code: "dom".into(),
                    message: format!("{err:?}"),
                    recoverable: true,
                }));
            }
            output.append(&mut h.pending);
            h.reconcile_focus();
            let mut state = h.engine.borrow().ui_state();
            state.geometry = h.last_geometry.clone();
            output.retain(|o| !matches!(o, EngineOutput::UiState(_)));
            output.push(EngineOutput::UiState(state));
            h.callback.clone()
        };
        let render_elapsed = now_ms() - measured;
        let root = host.borrow().root.clone();
        if root.has_attribute("data-measure-timings") {
            let _ = root.set_attribute("data-perf-render-ms", &render_elapsed.to_string());
        }
        let callback_started = now_ms();
        let media_requested = output.iter().any(|o| {
            matches!(
                o,
                EngineOutput::Request(crate::EngineEvent::MediaUrlQuery { .. })
            )
        });
        callback(output);
        if root.has_attribute("data-measure-timings") {
            let _ = root.set_attribute(
                "data-perf-callback-ms",
                &(now_ms() - callback_started).to_string(),
            );
        }
        if media_requested {
            media::schedule_timeout(host);
        }
        Self::schedule_geometry(host);
    }
    pub(crate) fn owns_focus(&self, element: &Element) -> bool {
        if element
            .closest(".dxeditor-island")
            .ok()
            .flatten()
            .is_some_and(|island| self.root.contains(Some(&island)))
        {
            return false;
        }
        if self.root.contains(Some(element)) {
            return true;
        }
        let wrapper = self
            .root
            .closest(".dxeditor,.dx-composer-editor")
            .ok()
            .flatten();
        let owner = element
            .closest(".dxeditor,.dx-composer-editor")
            .ok()
            .flatten();
        wrapper.is_some()&&wrapper==owner&&element.closest("[data-dxeditor-overlays],.dxeditor-engine__surface,.dxeditor-gutter,.dx-composer-suggestions").ok().flatten().is_some()
    }
    fn reconcile_focus(&self) {
        let focused = self
            .root
            .owner_document()
            .and_then(|document| document.active_element())
            .is_some_and(|element| self.owns_focus(&element));
        self.engine.borrow_mut().focused = focused;
    }
    fn schedule_geometry(host: &Host) {
        let mut h = host.borrow_mut();
        if h.destroyed || h.frame.is_some() {
            return;
        }
        let weak = Rc::downgrade(host);
        let callback = Closure::once_into_js(move || {
            let Some(host) = weak.upgrade() else { return };
            let (callback, state) = {
                let Ok(mut h) = host.try_borrow_mut() else {
                    return;
                };
                h.frame = None;
                if h.destroyed {
                    return;
                }
                h.reconcile_focus();
                let geometry = geometry::measure(&h.root, &h.engine.borrow());
                h.last_geometry = geometry.clone();
                let mut state = h.engine.borrow().ui_state();
                state.geometry = geometry;
                (h.callback.clone(), state)
            };
            callback(vec![EngineOutput::UiState(state)]);
        });
        if let Some(window) = web_sys::window() {
            h.frame = window
                .request_animation_frame(callback.unchecked_ref())
                .ok();
        }
    }
    pub fn refresh(&mut self, output: &[EngineOutput]) -> Result<(), JsValue> {
        if self.destroyed {
            return Ok(());
        }
        let editable = if self.engine.borrow().read_only() {
            "false"
        } else {
            "true"
        };
        if self.root.get_attribute("contenteditable").as_deref() != Some(editable) {
            self.root.set_attribute("contenteditable", editable)?;
        }
        let readonly = if editable == "false" { "true" } else { "false" };
        if self.root.get_attribute("aria-readonly").as_deref() != Some(readonly) {
            self.root.set_attribute("aria-readonly", readonly)?;
        }
        let patch_started = now_ms();
        for item in output {
            match item{EngineOutput::Render(plan)=>{self.renderer.render(&self.root,&self.engine.borrow(),plan,self.composing.as_ref())?;if plan.structure_changed||plan.full{if let Some(observer)=&self.intersection{if let Ok(nodes)=self.root.query_selector_all("[data-island-uid]"){for i in 0..nodes.length(){if let Some(el)=nodes.item(i).and_then(|n|n.dyn_into::<Element>().ok()){observer.observe(&el);}}}}}},
   EngineOutput::RememberSuggestion{trigger,item}=>{remember(trigger,item);},EngineOutput::Focus{target}=>match target{FocusTarget::Editor=>{self.engine.borrow_mut().set_focused(true);if let Some(h)=self.root.dyn_ref::<web_sys::HtmlElement>(){h.focus()?;}},FocusTarget::Island(uid)|FocusTarget::Block(uid)=>{if let Some(el)=self.root.query_selector(&format!("[data-uid='{uid}'] button,[data-uid='{uid}'] input,[data-uid='{uid}'] [tabindex='0']"))?.and_then(|e|e.dyn_into::<web_sys::HtmlElement>().ok()){el.focus()?;}},FocusTarget::AfterBlock(_)=>{}},EngineOutput::Scroll{uid,highlight}=>{if let Some(el)=self.renderer.blocks.get(uid){el.scroll_into_view();if *highlight{el.set_attribute("data-highlight","")?;let el=el.clone();let callback=Closure::once_into_js(move||{let _=el.remove_attribute("data-highlight");});if let Some(w)=web_sys::window(){let _=w.set_timeout_with_callback_and_timeout_and_arguments_0(callback.unchecked_ref(),1600);}}}},_=>{}}
        }
        if self.root.has_attribute("data-measure-timings") {
            let _ = self.root.set_attribute(
                "data-perf-patch-ms",
                &(now_ms() - patch_started).to_string(),
            );
        }
        let media_started = now_ms();
        let structural = output
            .iter()
            .any(|o| matches!(o,EngineOutput::Render(p) if p.full||p.structure_changed));
        if structural {
            islands::adopt_all(&self.root, &self.session);
            self.pending
                .extend(self.media.refresh(&self.root, &self.engine.borrow(), None));
        } else {
            let dirty: Vec<_> = output
                .iter()
                .filter_map(|o| {
                    if let EngineOutput::Render(p) = o {
                        Some(p.dirty.clone())
                    } else {
                        None
                    }
                })
                .flatten()
                .collect();
            if !dirty.is_empty() {
                self.pending.extend(self.media.refresh(
                    &self.root,
                    &self.engine.borrow(),
                    Some(&dirty),
                ));
                for uid in &dirty {
                    if let Some(block) = self.renderer.blocks.get(uid) {
                        if let Some(group) = block
                            .query_selector(":scope > [data-children]")
                            .ok()
                            .flatten()
                        {
                            if self.engine.borrow().is_collapsed(uid) {
                                let _ = group.set_attribute("hidden", "");
                            } else {
                                let _ = group.remove_attribute("hidden");
                            }
                        }
                    }
                }
            }
        }
        if self.root.has_attribute("data-measure-timings") {
            let _ = self.root.set_attribute(
                "data-perf-media-ms",
                &(now_ms() - media_started).to_string(),
            );
        }
        let selection_started = now_ms();
        if self.composing.is_none()
            && output.iter().any(|o| {
                matches!(
                    o,
                    EngineOutput::ApplySelection(_) | EngineOutput::Focus { .. }
                )
            })
        {
            selection_sync::write_with(&self.root, &self.engine.borrow(), |point| {
                self.renderer.point(&self.root, point)
            });
        }
        if self.root.has_attribute("data-measure-timings") {
            let _ = self.root.set_attribute(
                "data-perf-selection-ms",
                &(now_ms() - selection_started).to_string(),
            );
        }
        let state = self.engine.borrow().ui_state();
        if let Some(preview) = &state.entity_preview {
            if !preview.loading {
                if let Ok(elements) = self
                    .root
                    .query_selector_all("[data-semantic-mention],[data-entity-id]")
                {
                    for i in 0..elements.length() {
                        if let Some(el) =
                            elements.item(i).and_then(|n| n.dyn_into::<Element>().ok())
                        {
                            if el
                                .get_attribute("data-semantic-mention")
                                .or_else(|| el.get_attribute("data-entity-id"))
                                .as_ref()
                                == Some(&preview.entity_id)
                            {
                                if preview.preview.is_none() {
                                    let _ = el.set_attribute("data-deleted", "");
                                    let _ = el.remove_attribute("href");
                                } else {
                                    let _ = el.remove_attribute("data-deleted");
                                    let _ = el.set_attribute(
                                        "href",
                                        &format!("semantic:entity:{}", preview.entity_id),
                                    );
                                    if el.has_attribute("data-semantic-mention") {
                                        if let Some(value) = &preview.preview {
                                            let label = format!("@{}", value.label);
                                            if el.text_content().as_deref() != Some(&label) {
                                                el.set_text_content(Some(&label));
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
        if let Some(s) = state.suggestion {
            self.root.set_attribute("aria-haspopup", "listbox")?;
            self.root.set_attribute("aria-controls", &s.listbox_id)?;
            self.root.set_attribute(
                "aria-activedescendant",
                &format!("{}-{}", s.listbox_id, s.active_index),
            )?;
        } else {
            self.root.remove_attribute("aria-haspopup")?;
            self.root.remove_attribute("aria-controls")?;
            self.root.remove_attribute("aria-activedescendant")?;
        }
        if let Some(observer) = &self.observer {
            observer.take_records();
        }
        Ok(())
    }
    pub fn set_aria_label(&mut self, label: &str) {
        let _ = self.root.set_attribute("aria-label", label);
    }
    pub fn set_placeholder(&mut self, text: &str) {
        let _ = self.root.set_attribute("data-placeholder", text);
    }
    pub fn set_max_inline_paste_chars(&mut self, n: usize) {
        self.max_inline_paste_chars = n;
    }
    pub fn set_composer_event_callback(
        &mut self,
        callback: Rc<dyn Fn(crate::profiles::composer::ComposerEvent)>,
    ) {
        self.composer_callback = Some(callback);
    }
    pub fn set_island_visibility_callback(&mut self, callback: Rc<dyn Fn(String, bool)>) {
        self.visibility_callback = Some(callback);
    }
    pub fn media_url_result(&mut self, id: u64, url: Option<String>) {
        self.media.result(id, url);
        self.pending
            .extend(self.media.refresh(&self.root, &self.engine.borrow(), None));
    }
    pub fn island_ready(&mut self, uid: &str) {
        islands::adopt(&self.root, &self.session, uid);
    }
    pub fn destroy(&mut self) {
        if self.destroyed {
            return;
        }
        self.destroyed = true;
        if let Some(id) = self.frame.take() {
            if let Some(window) = web_sys::window() {
                let _ = window.cancel_animation_frame(id);
            }
        }
        self.listeners.clear();
        if let Some(o) = self.observer.take() {
            o.disconnect();
        }
        if let Some(o) = self.intersection.take() {
            o.disconnect();
        }
        if let Some(observer) = self.resize.take() {
            observer.disconnect();
        }
        self.resize_callback.take();
        self.observer_callback.take();
        self.intersection_callback.take();
        islands::park_descendants(&self.root);
        self.root.remove();
        if let Some(ghost) = self.drag_ghost.take() {
            ghost.remove();
        }
        if let Some(indicator) = self.drag_indicator.take() {
            indicator.remove();
        }
    }
    pub(crate) fn listen(
        host: &Host,
        target: &web_sys::EventTarget,
        name: &str,
        capture: bool,
        mut callback: impl FnMut(&Host, Event) + 'static,
    ) -> Result<(), JsValue> {
        let weak = Rc::downgrade(host);
        let listener = Listener::new(target, name, capture, move |event| {
            if let Some(host) = weak.upgrade() {
                let available = host.try_borrow_mut().is_ok();
                let engine = host.try_borrow().ok().map(|h| h.engine.clone());
                let engine_available = engine.as_ref().is_some_and(|e| e.try_borrow_mut().is_ok());
                if available && engine_available {
                    callback(&host, event);
                }
            }
        })?;
        host.borrow_mut().listeners.push(listener);
        Ok(())
    }
    fn observe(host: &Host) -> Result<(), JsValue> {
        let weak = Rc::downgrade(host);
        let callback = Closure::wrap(
            Box::new(move |records: js_sys::Array, _: MutationObserver| {
                let Some(h) = weak.upgrade() else { return };
                let mut changed = Vec::new();
                let mut foreign = false;
                let mut invalid = false;
                {
                    let Ok(host) = h.try_borrow() else { return };
                    if host.composing.is_some() || host.destroyed {
                        return;
                    }
                    for record in records
                        .iter()
                        .filter_map(|r| r.dyn_into::<web_sys::MutationRecord>().ok())
                    {
                        if let Some(target) = record.target() {
                            if target
                                .dyn_ref::<Element>()
                                .cloned()
                                .or_else(|| target.parent_element())
                                .and_then(|e| e.closest(".dxeditor-island").ok().flatten())
                                .is_some()
                            {
                                continue;
                            }
                            foreign = true;
                            if let Some((el, c)) = mapping::container(&target, &host.root) {
                                let children = el.child_nodes();
                                let simple = (0..children.length()).all(|i| {
                                    children.item(i).is_some_and(|n| {
                                        n.node_type() == web_sys::Node::TEXT_NODE
                                            || n.dyn_ref::<Element>().is_some_and(|e| {
                                                e.tag_name() == "BR"
                                                    && !e.has_attribute("data-inline-atom")
                                            })
                                    })
                                });
                                if simple {
                                    changed.push((c, mapping::read_text(&el)));
                                } else {
                                    invalid = true;
                                }
                            } else {
                                invalid = true;
                            }
                        }
                    }
                }
                if !foreign {
                    Self::schedule_geometry(&h);
                    return;
                }
                for (c, text) in changed {
                    DomHost::dispatch(
                        &h,
                        EngineInput::CompositionEnd {
                            container: c,
                            dom_text: text,
                            dom_caret: None,
                        },
                    );
                }
                if invalid {
                    if let Ok(mut host) = h.try_borrow_mut() {
                        host.renderer = render::Renderer::default();
                    }
                    let selection = h.borrow().engine.borrow().selection().clone();
                    DomHost::publish(
                        &h,
                        vec![
                            EngineOutput::Render(RenderPlan {
                                full: true,
                                structure_changed: true,
                                dirty: vec![],
                            }),
                            EngineOutput::ApplySelection(selection),
                        ],
                    );
                }
            }) as Box<dyn FnMut(js_sys::Array, MutationObserver)>,
        );
        let observer = MutationObserver::new(callback.as_ref().unchecked_ref())?;
        let options = MutationObserverInit::new();
        options.set_child_list(true);
        options.set_subtree(true);
        options.set_character_data(true);
        observer.observe_with_options(&host.borrow().root, &options)?;
        {
            let mut h = host.borrow_mut();
            h.observer = Some(observer);
            h.observer_callback = Some(callback);
        }
        let weak = Rc::downgrade(host);
        let resized = Closure::wrap(
            Box::new(move |_: js_sys::Array, _: web_sys::ResizeObserver| {
                if let Some(host) = weak.upgrade() {
                    let available = host.try_borrow_mut().is_ok();
                    if available {
                        DomHost::schedule_geometry(&host);
                    }
                }
            }) as Box<dyn FnMut(js_sys::Array, web_sys::ResizeObserver)>,
        );
        let resize = web_sys::ResizeObserver::new(resized.as_ref().unchecked_ref())?;
        resize.observe(&host.borrow().root);
        {
            let mut h = host.borrow_mut();
            h.resize = Some(resize);
            h.resize_callback = Some(resized);
        }
        let weak = Rc::downgrade(host);
        let callback = Closure::wrap(Box::new(
            move |records: js_sys::Array, _: web_sys::IntersectionObserver| {
                let Some(h) = weak.upgrade() else { return };
                let Ok(host) = h.try_borrow() else { return };
                let Some(callback) = &host.visibility_callback else {
                    return;
                };
                for record in records
                    .iter()
                    .filter_map(|r| r.dyn_into::<web_sys::IntersectionObserverEntry>().ok())
                {
                    if let Some(uid) = record.target().get_attribute("data-island-uid") {
                        callback(uid, record.is_intersecting());
                    }
                }
            },
        )
            as Box<dyn FnMut(js_sys::Array, web_sys::IntersectionObserver)>);
        let options = web_sys::IntersectionObserverInit::new();
        options.set_root_margin("200px");
        let observer = web_sys::IntersectionObserver::new_with_options(
            callback.as_ref().unchecked_ref(),
            &options,
        )?;
        {
            let mut h = host.borrow_mut();
            if let Ok(list) = h.root.query_selector_all("[data-island-uid]") {
                for i in 0..list.length() {
                    if let Some(el) = list.item(i).and_then(|n| n.dyn_into::<Element>().ok()) {
                        observer.observe(&el);
                    }
                }
            }
            h.intersection = Some(observer);
            h.intersection_callback = Some(callback);
        }
        Ok(())
    }
}
impl Drop for DomHost {
    fn drop(&mut self) {
        self.destroy();
    }
}
pub(crate) fn target_element(event: &Event) -> Option<Element> {
    let target = event.target()?;
    target.dyn_ref::<Element>().cloned().or_else(|| {
        target
            .dyn_ref::<web_sys::Node>()
            .and_then(|node| node.parent_element())
    })
}
pub(crate) fn inside_island(event: &Event, root: &Element) -> bool {
    target_element(event)
        .and_then(|e| e.closest(".dxeditor-island").ok().flatten())
        .is_some_and(|e| root.contains(Some(&e)))
}

fn remember(trigger: &str, item: &crate::SuggestionItem) {
    let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) else {
        return;
    };
    if trigger == "/" {
        let mut ids: Vec<String> = storage
            .get_item("dxeditor-recent-blocks")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        ids.retain(|id| id != &item.id);
        ids.insert(0, item.id.clone());
        ids.truncate(5);
        if let Ok(raw) = serde_json::to_string(&ids) {
            let _ = storage.set_item("dxeditor-recent-blocks", &raw);
        }
    } else {
        let candidate: Option<crate::EntityLinkCandidate> = item
            .data
            .get("entity")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok());
        if let Some(candidate) = candidate {
            let mut items: Vec<crate::EntityLinkCandidate> = storage
                .get_item("dxeditor-recent-mentions")
                .ok()
                .flatten()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            items.retain(|c| c.id != candidate.id);
            items.insert(0, candidate);
            items.truncate(5);
            if let Ok(raw) = serde_json::to_string(&items) {
                let _ = storage.set_item("dxeditor-recent-mentions", &raw);
            }
        }
    }
}

fn now_ms() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map_or(0., |performance| performance.now())
}
