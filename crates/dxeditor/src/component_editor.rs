//! The public block editor: synchronous Rust engine state and a wasm DOM adapter.
mod change_validation;
use crate::engine::{Command, EditorEngine, EngineInput, EngineOptions, EngineOutput, Profile};
use crate::{
    AttributeMap, ComponentDocumentV2, ComponentNode, EditorCatalog, EngineEvent, HistoryPolicy,
    InsertAt,
};
use dioxus::core::{current_scope_id, parent_scope};
use dioxus::html::HasFileData;
use dioxus::prelude::*;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_EDITOR: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, PartialEq)]
pub struct ComponentChange {
    pub document: ComponentDocumentV2,
    pub revision: u64,
    pub dirty_blocks: Vec<String>,
}
pub type ComponentEditorEvent = EngineEvent;

type OutputHandler = Rc<dyn Fn(Vec<EngineOutput>)>;
#[derive(Default)]
struct Runtime {
    engine: Option<Rc<RefCell<EditorEngine>>>,
    output: Option<OutputHandler>,
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    dom: Option<Rc<RefCell<crate::dom::DomHost>>>,
}

/// A retained handle becomes inert when its component unmounts. Snapshots read
/// directly from the engine and never require a browser round-trip.
#[derive(Clone)]
pub struct ComponentEditorHandle {
    mounted: Signal<bool>,
    revision: Signal<u64>,
    external_revision: Signal<u64>,
    dirty: Signal<bool>,
    island_states: Signal<BTreeMap<String, crate::IslandState>>,
    runtime: Rc<RefCell<Runtime>>,
}
impl PartialEq for ComponentEditorHandle {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.runtime, &other.runtime)
    }
}
impl ComponentEditorHandle {
    fn is_mounted(&self) -> bool {
        self.mounted.try_peek().is_ok_and(|mounted| *mounted)
    }
    fn engine(&self) -> Option<Rc<RefCell<EditorEngine>>> {
        if !self.is_mounted() {
            return None;
        }
        self.runtime.try_borrow().ok()?.engine.clone()
    }
    fn input(&self, input: EngineInput) {
        if !self.is_mounted() {
            return;
        }
        #[cfg(all(feature = "web", target_arch = "wasm32"))]
        {
            let dom = self.runtime.borrow().dom.clone();
            if let Some(dom) = dom {
                crate::dom::DomHost::dispatch(&dom, input);
                return;
            }
        }
        if let Some(engine) = self.engine() {
            let outputs = engine.borrow_mut().handle(input);
            self.publish(outputs);
        }
    }
    fn publish(&self, outputs: Vec<EngineOutput>) {
        let output = self.runtime.borrow().output.clone();
        if let Some(output) = output {
            output(outputs);
        }
    }
    pub(crate) fn run(&self, command: Command) {
        self.input(EngineInput::Command(command));
    }
    pub fn replace_document(&self, document: ComponentDocumentV2, policy: HistoryPolicy) -> u64 {
        let Some(engine) = self.engine() else {
            return self
                .external_revision
                .try_peek()
                .map(|v| *v)
                .unwrap_or_default();
        };
        let current_external = *self.external_revision.peek();
        let next = current_external.saturating_add(1);
        let queued = engine.borrow().is_composing();
        let mut outputs = engine.borrow_mut().replace_document(document, policy);
        if !accept_replacement(&mut outputs, queued) {
            self.publish(outputs);
            return current_external;
        }
        let mut external = self.external_revision;
        external.set(next);
        let mut dirty = self.dirty;
        dirty.set(false);
        #[cfg(all(feature = "web", target_arch = "wasm32"))]
        if let Some(dom) = self.runtime.borrow().dom.as_ref() {
            let _ = dom.borrow_mut().refresh(&outputs);
        }
        self.publish(outputs);
        next
    }
    pub fn current_revision(&self) -> Option<u64> {
        if !self.is_mounted() {
            return None;
        }
        Some(self.engine()?.borrow().revision())
    }
    pub async fn snapshot_document(&self) -> Option<ComponentDocumentV2> {
        Some(self.engine()?.borrow().document().clone())
    }
    pub fn node_content_attrs(&self, uid: &str) -> Option<AttributeMap> {
        let engine = self.engine()?;
        let engine = engine.borrow();
        crate::engine::model::block(engine.document(), uid)?
            .content
            .first()
            .map(|n| n.attrs.clone())
    }
    pub fn entity_picker_result(
        &self,
        request_id: u64,
        entity: Option<crate::EntityLinkCandidate>,
    ) {
        self.input(EngineInput::HostResponse(
            crate::engine::HostResponse::EntityPicker { request_id, entity },
        ));
    }
    pub fn entity_create_result(
        &self,
        request_id: u64,
        entity: Option<crate::EntityLinkCandidate>,
    ) {
        self.input(EngineInput::HostResponse(
            crate::engine::HostResponse::EntityCreate { request_id, entity },
        ));
    }
    pub fn convert_markdown_result(&self, request_id: u64, nodes: Option<Vec<ComponentNode>>) {
        self.input(EngineInput::HostResponse(
            crate::engine::HostResponse::Markdown { request_id, nodes },
        ));
    }
    pub fn media_url_result(&self, request_id: u64, url: Option<String>) {
        #[cfg(all(feature = "web", target_arch = "wasm32"))]
        if let Some(dom) = self.runtime.borrow().dom.as_ref() {
            dom.borrow_mut().media_url_result(request_id, url.clone());
        }
        self.input(EngineInput::HostResponse(
            crate::engine::HostResponse::MediaUrl { request_id, url },
        ));
    }
    pub fn undo(&self) {
        self.run(Command::Undo);
    }
    pub fn redo(&self) {
        self.run(Command::Redo);
    }
    pub fn select_node(&self, uid: String) {
        self.run(Command::SelectNode(uid));
    }
    pub fn remove_node(&self, uid: String) {
        self.run(Command::RemoveNode(uid));
    }
    pub fn replace_node(&self, uid: String, nodes: Vec<ComponentNode>) {
        self.run(Command::ReplaceNode { uid, nodes });
    }
    pub fn insert_blocks(
        &self,
        at: InsertAt,
        nodes: Vec<ComponentNode>,
        transient: BTreeMap<String, AttributeMap>,
    ) {
        self.run(Command::InsertBlocks { at, nodes });
        for (uid, attrs) in transient {
            self.set_transient_attrs(uid, attrs);
        }
    }
    pub fn update_node_attrs(&self, uid: String, attrs: AttributeMap, add_to_history: bool) {
        self.run(Command::UpdateNodeAttrs {
            uid,
            attrs,
            add_to_history,
        });
    }
    pub fn set_transient_attrs(&self, uid: String, attrs: AttributeMap) {
        if !self.is_mounted() {
            return;
        }
        if let Some(state) = self.island_states.peek().get(&uid) {
            let mut signal = state.transient;
            signal.set(attrs.clone());
        }
        self.run(Command::SetTransientAttrs { uid, attrs });
    }
    pub fn scroll_to_block(&self, uid: String, highlight: bool) {
        self.run(Command::ScrollToBlock { uid, highlight });
    }
    pub fn set_collapsed(&self, uids: Vec<String>) {
        self.run(Command::SetCollapsed(uids));
    }
    pub fn focus(&self) {
        self.run(Command::Focus);
    }
    pub fn focus_after_block(&self, uid: String) {
        self.run(Command::FocusAfterBlock(uid));
    }
    pub fn insert_text(&self, text: String) {
        self.run(Command::InsertText(text));
    }
    pub fn island_ready(&self, uid: String) {
        #[cfg(all(feature = "web", target_arch = "wasm32"))]
        if let Some(dom) = self.runtime.borrow().dom.as_ref() {
            dom.borrow_mut().island_ready(&uid);
        }
        #[cfg(not(all(feature = "web", target_arch = "wasm32")))]
        let _ = uid;
    }
}

fn use_handle(
    document: &ComponentDocumentV2,
    catalog: &EditorCatalog,
    profile: Profile,
    options: EngineOptions,
    external: u64,
) -> (ComponentEditorHandle, Option<String>) {
    let owner = parent_scope().unwrap_or_else(current_scope_id);
    let mut mounted = use_hook(|| Signal::new_in_scope(true, owner));
    let initial =
        use_hook(
            move || match EditorEngine::new(document.clone(), catalog, profile, options) {
                Ok(engine) => (Some(Rc::new(RefCell::new(engine))), None),
                Err(error) => (None, Some(error.to_string())),
            },
        );
    let handle = ComponentEditorHandle {
        mounted,
        revision: use_hook(|| Signal::new_in_scope(0, owner)),
        external_revision: use_hook(|| Signal::new_in_scope(external, owner)),
        dirty: use_hook(|| Signal::new_in_scope(false, owner)),
        island_states: use_hook(|| Signal::new_in_scope(BTreeMap::new(), owner)),
        runtime: use_hook(|| {
            Rc::new(RefCell::new(Runtime {
                engine: initial.0.clone(),
                ..Runtime::default()
            }))
        }),
    };
    let runtime = handle.runtime.clone();
    use_drop(move || {
        if let Ok(mut value) = mounted.try_write() {
            *value = false;
        }
        let mut runtime = runtime.borrow_mut();
        #[cfg(all(feature = "web", target_arch = "wasm32"))]
        if let Some(dom) = runtime.dom.take() {
            dom.borrow_mut().destroy();
        }
        runtime.output = None;
        runtime.engine = None;
    });
    (handle, initial.1)
}

#[component]
pub fn ComponentEditor(
    document: ComponentDocumentV2,
    #[props(default = EditorCatalog::block_document())] catalog: EditorCatalog,
    #[props(default)] external_revision: Option<u64>,
    #[props(default)] read_only: bool,
    aria_label: String,
    #[props(default)] islands: crate::IslandRegistry,
    #[props(default)] entity_links: Option<crate::EntityLinkExtension>,
    #[props(default)] host_entity_picker: bool,
    on_change: EventHandler<ComponentChange>,
    #[props(default)] on_ready: EventHandler<ComponentEditorHandle>,
    #[props(default)] on_event: EventHandler<ComponentEditorEvent>,
    #[props(default)] on_invalid_change: EventHandler<String>,
    #[props(default)] ondrop: Option<EventHandler<DragEvent>>,
    #[props(default)] ondragover: Option<EventHandler<DragEvent>>,
    #[props(default)] onblur: Option<EventHandler<FocusEvent>>,
    #[props(default)] onfocus: Option<EventHandler<FocusEvent>>,
    #[props(default)] onkeydown: Option<EventHandler<KeyboardEvent>>,
) -> Element {
    let session = use_hook(|| {
        format!(
            "rust-editor-{}",
            NEXT_EDITOR.fetch_add(1, Ordering::Relaxed)
        )
    });
    let profile = if catalog
        .component_specs()
        .spec_by_id("opaque_markdown")
        .is_some()
    {
        Profile::MarkdownNotes
    } else if catalog.component_specs().spec_by_id("graph").is_none() {
        Profile::GraphNode
    } else {
        Profile::BlockDocument
    };
    let (handle, initial_error) = use_handle(
        &document,
        &catalog,
        profile,
        EngineOptions {
            read_only,
            host_entity_picker,
            ..EngineOptions::default()
        },
        external_revision.unwrap_or_default(),
    );
    let mut error = use_signal(|| initial_error.unwrap_or_default());
    let mut ready = use_signal(|| false);
    let ready_once = use_hook(|| Rc::new(Cell::new(false)));
    let initial_ui = handle
        .engine()
        .map(|engine| engine.borrow().ui_state())
        .unwrap_or_else(|| {
            EditorEngine::new(
                crate::engine::model::document(vec![crate::engine::model::paragraph()]),
                &EditorCatalog::block_document(),
                Profile::BlockDocument,
                EngineOptions::default(),
            )
            .expect("empty editor schema is valid")
            .ui_state()
        });
    #[allow(unused_mut)]
    let mut ui_state = use_signal(|| initial_ui);
    let validation_catalog = catalog.clone();
    let validation_schema = catalog.schema_fingerprint();
    let validation_cache = use_hook(|| {
        let mut cache = change_validation::ChangeValidator::default();
        if let Some(engine) = handle.engine() {
            cache.seed_validated(engine.borrow().document(), &validation_schema);
        }
        Rc::new(RefCell::new(cache))
    });
    let output_handle = handle.clone();
    let provider = entity_links.as_ref().map(|links| links.provider().clone());
    let query_generation = use_hook(|| Rc::new(Cell::new(0u64)));
    let preview_generation = use_hook(|| Rc::new(Cell::new(0u64)));
    let preview_cache = use_hook(|| {
        Rc::new(RefCell::new(BTreeMap::<
            String,
            Option<crate::EntityLinkPreview>,
        >::new()))
    });
    let preview_inflight = use_hook(|| Rc::new(RefCell::new(None::<String>)));
    let callback_runtime = dioxus::core::Runtime::current();
    let callback_scope = current_scope_id();
    let callback: OutputHandler = Rc::new(move |outputs| {
        callback_runtime.in_scope(callback_scope, || {
            let mut error = error;
            let Some(engine) = output_handle.engine() else {
                return;
            };
            let structural = outputs.iter().any(|output| matches!(output, EngineOutput::Render(plan) if plan.full || plan.structure_changed));
            let text_edit = !structural && outputs.iter().any(|output| matches!(output, EngineOutput::Render(_))) && !outputs.iter().any(|output| matches!(output, EngineOutput::ChangedSnapshot { .. }));
            let has_change = outputs.iter().any(|output| matches!(output, EngineOutput::Changed { .. } | EngineOutput::ChangedSnapshot { .. }));
            if structural {
                let mut cache = validation_cache.borrow_mut(); cache.invalidate();
                // Revision zero is the fully validated initial engine document.
                let engine = engine.borrow();
                if !has_change && engine.revision() == 0 && engine.schema().fingerprint == validation_schema { cache.seed_validated(engine.document(), &validation_schema); }
            }
            if !has_change {
                let engine = engine.borrow();
                let uid = crate::engine::commands::blocks::current_uid(&engine);
                let candidate = uid.as_deref().and_then(|uid| engine.block(uid).map(|block| (uid, block)));
                validation_cache.borrow_mut().track_selection(candidate);
            }
            if outputs.iter().any(|output| matches!(output, EngineOutput::Render(_) | EngineOutput::ApplySelection(_) | EngineOutput::Changed { .. } | EngineOutput::ChangedSnapshot { .. })) {
                let dirty: std::collections::BTreeSet<_> = outputs.iter().flat_map(|output| match output { EngineOutput::Render(plan) => plan.dirty.clone(), EngineOutput::Changed { dirty_blocks, .. } | EngineOutput::ChangedSnapshot { dirty_blocks, .. } => dirty_blocks.clone(), _ => Vec::new() }).collect();
                let dirty: Vec<_> = dirty.into_iter().collect();
                profile_change("dxeditor.islands", || crate::islands::sync_engine_changes(&engine.borrow(), output_handle.island_states, (!structural).then_some(dirty.as_slice())));
            }
            for output in outputs {
                let (output, snapshot) = match output {
                    EngineOutput::ChangedSnapshot {
                        revision,
                        dirty_blocks,
                        document,
                    } => (
                        EngineOutput::Changed {
                            revision,
                            dirty_blocks,
                        },
                        Some(document),
                    ),
                    output => (output, None),
                };
                match output {
                    EngineOutput::Changed {
                        revision: next,
                        dirty_blocks,
                    } => {
                        let (document, validation) = if let Some(document) = snapshot {
                            let validation = profile_change("dxeditor.validate", || validation_cache.borrow_mut().validate(&document, validation_catalog.component_specs(), &validation_schema, None, false));
                            (Some(document), validation)
                        } else {
                            let engine = engine.borrow();
                            let candidate = if dirty_blocks.len() == 1 { engine.block(&dirty_blocks[0]).map(|block| (dirty_blocks[0].as_str(), block)) } else { None };
                            let validation = profile_change("dxeditor.validate", || validation_cache.borrow_mut().validate(engine.document(), validation_catalog.component_specs(), &validation_schema, candidate, text_edit));
                            (validation.is_ok().then(|| profile_change("dxeditor.snapshot", || engine.document().clone())), validation)
                        };
                        let mut revision = output_handle.revision;
                        revision.set(next);
                        let mut dirty = output_handle.dirty;
                        dirty.set(true);
                        match validation {
                            Ok(()) => {
                                if !error.peek().is_empty() {
                                    error.set(String::new());
                                }
                                profile_change("dxeditor.on_change", || {
                                    on_change.call(ComponentChange {
                                        document: document.expect("validated document"),
                                        revision: next,
                                        dirty_blocks,
                                    })
                                });
                            }
                            Err(problem) => {
                                let message = format!("{problem}: {:?}", problem.issues);
                                error.set(message.clone());
                                on_invalid_change.call(message);
                            }
                        }
                    }
                    EngineOutput::UiState(mut state) => {
                        let mut ui_state = ui_state;
                        if state.geometry.scale_x == 0.0 {
                            state.geometry = ui_state.peek().geometry.clone();
                        }
                        if *ui_state.peek() != state {
                            ui_state.set(state);
                        }
                    }
                    EngineOutput::Request(event) => {
                        if let EngineEvent::Error { code, message, .. } = &event {
                            error.set(message.clone());
                            if code == "invalid_external_document" { on_invalid_change.call(message.clone()); }
                        }
                        if let EngineEvent::ConvertMarkdownRequest { request_id, .. } = &event {
                            let request_id = *request_id;
                            let timeout_handle = output_handle.clone();
                            spawn(async move {
                                futures_timer::Delay::new(std::time::Duration::from_secs(2)).await;
                                timeout_handle.convert_markdown_result(request_id, None);
                            });
                        }
                        on_event.call(event);
                    }
                    EngineOutput::EntityOpen { entity_id } => {
                        if let Some(provider) = &provider {
                            provider.open(entity_id);
                        }
                    }
                    EngineOutput::QueryEntityPreview { entity_id } => {
                        let Some(provider) = provider.clone() else {
                            continue;
                        };
                        let cached = preview_cache.borrow().get(&entity_id).cloned();
                        if let Some(preview) = cached {
                            let outputs = engine
                                .borrow_mut()
                                .set_entity_preview(entity_id, preview, false);
                            output_handle.publish(outputs);
                            continue;
                        }
                        if preview_inflight.borrow().as_ref() == Some(&entity_id) {
                            continue;
                        }
                        *preview_inflight.borrow_mut() = Some(entity_id.clone());
                        let generation = preview_generation.get().saturating_add(1);
                        preview_generation.set(generation);
                        let preview_generation = preview_generation.clone();
                        let preview_cache = preview_cache.clone();
                        let preview_inflight = preview_inflight.clone();
                        let handle = output_handle.clone();
                        let output =
                            engine
                                .borrow_mut()
                                .set_entity_preview(entity_id.clone(), None, true);
                        handle.publish(output);
                        spawn(async move {
                            let preview = provider.preview(entity_id.clone()).await;
                            if preview_generation.get() != generation {
                                return;
                            }
                            *preview_inflight.borrow_mut() = None;
                            preview_cache
                                .borrow_mut()
                                .insert(entity_id.clone(), preview.clone());
                            if let Some(engine) = handle.engine() {
                                let outputs = engine
                                    .borrow_mut()
                                    .set_entity_preview(entity_id, preview, false);
                                #[cfg(all(feature = "web", target_arch = "wasm32"))]
                                if let Some(dom) = handle.runtime.borrow().dom.as_ref() {
                                    let _ = dom.borrow_mut().refresh(&outputs);
                                }
                                handle.publish(outputs);
                            }
                        });
                    }
                    EngineOutput::QuerySuggestions {
                        generation,
                        trigger,
                        query,
                    } => {
                        query_generation.set(generation);
                        let provider = provider.clone();
                        let output_handle = output_handle.clone();
                        let query_generation = query_generation.clone();
                        spawn(async move {
                            futures_timer::Delay::new(std::time::Duration::from_millis(120)).await;
                            if query_generation.get() != generation || !output_handle.is_mounted() {
                                return;
                            }
                            let candidates = match provider {
                                Some(provider) => provider.search_for_trigger(query, trigger).await,
                                None => Vec::new(),
                            };
                            if query_generation.get() != generation {
                                return;
                            }
                            let items = candidates
                                .into_iter()
                                .map(|candidate| crate::SuggestionItem {
                                    id: candidate.id.clone(),
                                    label: candidate.label.clone(),
                                    description: candidate.detail.clone(),
                                    icon: None,
                                    data: serde_json::json!({"entity": candidate}),
                                    priority: 0,
                                })
                                .collect();
                            if let Some(engine) = output_handle.engine() {
                                let outputs =
                                    engine.borrow_mut().set_suggestion_items(generation, items);
                                #[cfg(all(feature = "web", target_arch = "wasm32"))]
                                if let Some(dom) = output_handle.runtime.borrow().dom.as_ref() {
                                    let _ = dom.borrow_mut().refresh(&outputs);
                                }
                                output_handle.publish(outputs);
                            }
                        });
                    }
                    _ => {}
                }
            }
        })
    });
    handle.runtime.borrow_mut().output = Some(callback.clone());
    let controlled_handle = handle.clone();
    use_effect(use_reactive(
        (&document, &external_revision),
        move |(document, external_revision)| {
            let Some(engine) = controlled_handle.engine() else {
                return;
            };
            let next = external_revision.unwrap_or_default();
            let current_external = *controlled_handle.external_revision.peek();
            if external_revision.is_some() && next <= current_external {
                return;
            }
            if &document == engine.borrow().document() {
                let mut dirty = controlled_handle.dirty;
                dirty.set(false);
                let mut revision = controlled_handle.external_revision;
                revision.set(next);
            } else if *controlled_handle.dirty.peek() {
                error.set("Document changed externally while local edits are pending".into());
            } else {
                let queued = engine.borrow().is_composing();
                let mut outputs = engine
                    .borrow_mut()
                    .replace_document(document, HistoryPolicy::Reset);
                if !accept_replacement(&mut outputs, queued) {
                    controlled_handle.publish(outputs);
                    return;
                }
                let mut external = controlled_handle.external_revision;
                external.set(next);
                #[cfg(all(feature = "web", target_arch = "wasm32"))]
                if let Some(dom) = controlled_handle.runtime.borrow().dom.as_ref() {
                    let _ = dom.borrow_mut().refresh(&outputs);
                }
                controlled_handle.publish(outputs);
            }
        },
    ));
    let options_handle = handle.clone();
    let has_entity_links = entity_links.is_some() || host_entity_picker;
    use_effect(use_reactive(
        (&read_only, &aria_label, &has_entity_links),
        move |(read_only, aria_label, has_entity_links)| {
            let mut outputs = Vec::new();
            if let Some(engine) = options_handle.engine() {
                let mut engine = engine.borrow_mut();
                outputs.extend(engine.set_read_only(read_only));
                let mut triggers = vec!["/".to_string()];
                if has_entity_links {
                    triggers.extend(["@".to_string(), "[[".to_string()]);
                }
                outputs.extend(engine.set_triggers(triggers));
                outputs.push(EngineOutput::Render(crate::engine::RenderPlan {
                    full: true,
                    structure_changed: false,
                    dirty: Vec::new(),
                }));
            }
            #[cfg(all(feature = "web", target_arch = "wasm32"))]
            if let Some(dom) = options_handle.runtime.borrow().dom.as_ref() {
                let mut dom = dom.borrow_mut();
                dom.set_aria_label(&aria_label);
                let _ = dom.refresh(&outputs);
            }
            options_handle.publish(outputs);
            #[cfg(not(all(feature = "web", target_arch = "wasm32")))]
            let _ = aria_label;
        },
    ));
    let mount_handle = handle.clone();
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    let mount_label = aria_label.clone();
    #[cfg(not(all(feature = "web", target_arch = "wasm32")))]
    {
        let handle = handle.clone();
        let ready_once = ready_once.clone();
        use_effect(move || {
            if !ready_once.replace(true) {
                ready.set(true);
                on_ready.call(handle.clone());
            }
        });
    }
    rsx! {
        style { {crate::component::DXEDITOR_STYLE} }
        style { {include_str!("profiles/block_document.css")} }
        div { class: "dxeditor dxeditor--blocks", "data-engine": "rust", "data-dxeditor-session": "{session}", "data-dxeditor-ready": ready().to_string(),
            ondragover: move |event: DragEvent| {
                if !read_only {
                    if !event.data().files().is_empty() { event.prevent_default(); }
                    if let Some(callback) = ondragover { callback.call(event); }
                }
            },
            ondrop: move |event| { if !read_only && let Some(callback) = ondrop { callback.call(event); } },
            div { class: "dxeditor__document", "data-dxeditor-host": "{session}", "data-native-file-drop": ondrop.is_some().to_string(),
                onfocus: move |event| { if let Some(callback) = onfocus { callback.call(event); } },
                onblur: move |event| { if let Some(callback) = onblur { callback.call(event); } },
                onkeydown: move |event| { if let Some(callback) = onkeydown { callback.call(event); } },
                onmounted: move |event| {
                    #[cfg(all(feature = "web", target_arch = "wasm32"))]
                    {
                        use dioxus::web::WebEventExt;
                        let Some(engine) = mount_handle.engine() else { return; };
                        let Some(element) = event.data().try_as_web_event() else { return; };
                        let callback = mount_handle.runtime.borrow().output.clone().expect("output handler installed");
                        let old_dom = mount_handle.runtime.borrow_mut().dom.take();
                        if let Some(old_dom) = old_dom { old_dom.borrow_mut().destroy(); }
                        match crate::dom::DomHost::mount(element, engine.clone(), callback) {
                            Ok(dom) => {
                                dom.borrow_mut().set_aria_label(&mount_label);
                                let states = mount_handle.island_states;
                                let visibility_runtime = dioxus::core::Runtime::current();
                                let visibility_scope = current_scope_id();
                                dom.borrow_mut().set_island_visibility_callback(Rc::new(move |uid, visible| visibility_runtime.in_scope(visibility_scope, || crate::islands::set_visibility(states, &uid, visible))));
                                mount_handle.runtime.borrow_mut().dom = Some(dom);
                                crate::islands::sync_engine(&engine.borrow(), mount_handle.island_states);
                                ready.set(true);
                                if !ready_once.replace(true) { on_ready.call(mount_handle.clone()); }
                            }
                            Err(problem) => error.set(format!("Could not mount editor: {problem:?}")),
                        }
                    }
                    #[cfg(not(all(feature = "web", target_arch = "wasm32")))]
                    let _ = (&event, &mount_handle, &ready_once);
                },
                {native_document(&document, entity_links.clone(), read_only)}
            }
            div { class: "dxeditor__overlays", "data-dxeditor-overlays": "",
                {editor_surfaces(ui_state, handle.clone(), catalog.clone())}
            }
            crate::islands::IslandParking { session_id: session.clone(), states: handle.island_states, registry: islands, read_only, handle }
            span { class: "dxeditor__live-status", role: "status", aria_live: "polite", "{error}" }
        }
    }
}

fn native_document(
    document: &ComponentDocumentV2,
    entity_links: Option<crate::EntityLinkExtension>,
    read_only: bool,
) -> Element {
    #[cfg(not(all(feature = "web", target_arch = "wasm32")))]
    return rsx! { crate::fallback::BlockDocumentFallback {document: document.clone(), entity_links, read_only} };
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    {
        let _ = (document, entity_links, read_only);
        rsx! {}
    }
}

fn editor_surfaces(
    state: Signal<crate::engine::ui_state::EditorUiState>,
    handle: ComponentEditorHandle,
    catalog: EditorCatalog,
) -> Element {
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    {
        let handle = crate::ui::EngineHandle::new(Rc::new(move |command| handle.run(command)));
        let state: ReadSignal<crate::engine::ui_state::EditorUiState> = state.into();
        rsx! { crate::ui::EditorSurfaces {state, handle, catalog} }
    }
    #[cfg(not(all(feature = "web", target_arch = "wasm32")))]
    {
        let _ = (state, handle, catalog);
        rsx! {}
    }
}

// Debug builds expose only the most recent phase duration to browser DevTools.
// Keeping one entry per phase avoids retaining a timeline while users type.
fn profile_change<T>(name: &str, action: impl FnOnce() -> T) -> T {
    #[cfg(all(debug_assertions, feature = "web", target_arch = "wasm32"))]
    let performance = web_sys::window().and_then(|window| window.performance());
    #[cfg(all(debug_assertions, feature = "web", target_arch = "wasm32"))]
    if let Some(performance) = &performance {
        let _ = performance.mark(&format!("{name}.start"));
    }
    let value = action();
    #[cfg(all(debug_assertions, feature = "web", target_arch = "wasm32"))]
    if let Some(performance) = performance {
        performance.clear_measures_with_measure_name(name);
        let _ = performance.measure_with_start_mark(name, &format!("{name}.start"));
        performance.clear_marks_with_mark_name(&format!("{name}.start"));
    }
    #[cfg(not(all(debug_assertions, feature = "web", target_arch = "wasm32")))]
    let _ = name;
    value
}

// A rejected external replacement must not acknowledge local changes or move the
// host revision forward. Valid replacements queued during IME emit no render yet.
fn accept_replacement(outputs: &mut [EngineOutput], queued: bool) -> bool {
    if outputs
        .iter()
        .any(|output| matches!(output, EngineOutput::Render(_)))
    {
        return true;
    }
    let mut rejected = false;
    for output in outputs {
        if let EngineOutput::Request(EngineEvent::Error { code, .. }) = output {
            *code = "invalid_external_document".into();
            rejected = true;
        }
    }
    queued && !rejected
}
#[cfg(test)]
mod replacement_tests {
    use super::*;
    #[test]
    fn rejected_replacements_are_distinct_from_accepted_composition_queue() {
        let error = EngineOutput::Request(EngineEvent::Error {
            code: "editor_engine".into(),
            message: "invalid document".into(),
            recoverable: true,
        });
        for queued in [false, true] {
            let mut outputs = vec![error.clone()];
            assert!(!accept_replacement(&mut outputs, queued));
            assert!(
                matches!(&outputs[0], EngineOutput::Request(EngineEvent::Error { code, .. }) if code == "invalid_external_document")
            );
        }
        assert!(accept_replacement(&mut [], true));
        assert!(!accept_replacement(&mut [], false));
        assert!(accept_replacement(
            &mut [EngineOutput::Render(Default::default())],
            false
        ));
    }
}
