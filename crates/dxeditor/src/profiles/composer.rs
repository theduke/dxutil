//! Structured composer profile on the shared Rust engine. Chip payloads and
//! suggestion actions belong to the host application.
use crate::engine::{
    EditorEngine, EditorSelection, InlineContainerRef, TextPoint, TextRange, model,
};
use crate::{ComponentDocumentV2, ComponentNode, PastedFile, SuggestionItem, SuggestionRegistry};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ComposerSegment {
    Text(String),
    Chip(ComposerChip),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComposerChip {
    pub kind: String,
    pub label: String,
    pub payload: serde_json::Value,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ComposerValue {
    pub segments: Vec<ComposerSegment>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub enum SubmitKey {
    #[default]
    Enter,
    ModEnter,
}
#[derive(Clone, Debug, PartialEq)]
pub enum ComposerEvent {
    BackgroundSubmit,
    Submit { alternate: bool },
    HistoryPrevious,
    HistoryNext,
    Stash,
    PastedFiles(Vec<PastedFile>),
    PastedText { text: String },
    Dropped { mime: String, data: String },
    ChipActivated(ComposerChip),
    ChipRemoved(ComposerChip),
    SuggestionAccepted(SuggestionItem),
}

#[derive(Clone)]
pub struct ComposerHandle {
    run: Rc<dyn Fn(HandleCommand)>,
}
impl PartialEq for ComposerHandle {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.run, &other.run)
    }
}
#[derive(Clone)]
enum HandleCommand {
    Chip(ComposerChip),
    Text(String),
    Value(ComposerValue),
    Focus,
    Clear,
}
impl ComposerHandle {
    pub fn insert_chip(&self, chip: ComposerChip) {
        (self.run)(HandleCommand::Chip(chip));
    }
    pub fn insert_text(&self, text: String) {
        (self.run)(HandleCommand::Text(text));
    }
    pub fn focus(&self) {
        (self.run)(HandleCommand::Focus);
    }
    pub fn clear(&self) {
        (self.run)(HandleCommand::Clear);
    }
    pub fn set_value(&self, value: ComposerValue) {
        (self.run)(HandleCommand::Value(value));
    }
}

pub fn chip_node(chip: &ComposerChip) -> ComponentNode {
    let mut node = ComponentNode::new("chip");
    node.attrs
        .insert("kind".into(), serde_json::json!(chip.kind));
    node.attrs
        .insert("label".into(), serde_json::json!(chip.label));
    node.attrs.insert(
        "payload".into(),
        serde_json::json!(chip.payload.to_string()),
    );
    node
}
pub fn chip_from_node(node: &ComponentNode) -> Option<ComposerChip> {
    if node.kind.0 != "chip" {
        return None;
    }
    let payload = node.attrs.get("payload")?;
    Some(ComposerChip {
        kind: node.attrs.get("kind")?.as_str()?.into(),
        label: node.attrs.get("label")?.as_str()?.into(),
        payload: match payload {
            serde_json::Value::String(json) => serde_json::from_str(json).ok()?,
            other => other.clone(),
        },
    })
}

pub fn composer_to_blocks(value: &ComposerValue) -> ComponentDocumentV2 {
    let mut blocks = vec![model::paragraph()];
    for segment in &value.segments {
        match segment {
            ComposerSegment::Chip(chip) => blocks.last_mut().unwrap().content[0]
                .content
                .push(chip_node(chip)),
            ComposerSegment::Text(text) => {
                let normalized = crate::engine::clipboard::normalize_line_endings(text);
                for (index, line) in normalized.split('\n').enumerate() {
                    if index > 0 {
                        blocks.push(model::paragraph());
                    }
                    if !line.is_empty() {
                        blocks.last_mut().unwrap().content[0]
                            .content
                            .push(ComponentNode::text(line));
                    }
                }
            }
        }
    }
    for block in &mut blocks {
        block.content[0].content =
            crate::engine::text::compact(std::mem::take(&mut block.content[0].content));
    }
    model::document(blocks)
}

pub fn blocks_to_composer(document: &ComponentDocumentV2) -> ComposerValue {
    fn text(value: &mut ComposerValue, text: &str) {
        if text.is_empty() {
            return;
        }
        if let Some(ComposerSegment::Text(last)) = value.segments.last_mut() {
            last.push_str(text);
        } else {
            value.segments.push(ComposerSegment::Text(text.into()));
        }
    }
    let mut value = ComposerValue::default();
    if let Some(blocks) = model::children(document, None) {
        for (index, block) in blocks.iter().enumerate() {
            if index > 0 {
                text(&mut value, "\n");
            }
            for node in block
                .content
                .first()
                .map(|node| node.content.as_slice())
                .unwrap_or_default()
            {
                if let Some(chip) = chip_from_node(node) {
                    value.segments.push(ComposerSegment::Chip(chip));
                } else if node.kind.0 == "hard_break" {
                    text(&mut value, "\n");
                } else if let Some(content) = &node.text {
                    text(&mut value, content);
                }
            }
        }
    }
    value
}

pub fn composer_plain_text(value: &ComposerValue) -> String {
    value
        .segments
        .iter()
        .map(|segment| match segment {
            ComposerSegment::Text(text) => text.as_str(),
            ComposerSegment::Chip(chip) => chip.label.as_str(),
        })
        .collect()
}

#[derive(PartialEq)]
enum Unit {
    Text(char),
    Chip(ComposerChip),
}
fn units(value: &ComposerValue) -> Vec<Unit> {
    value
        .segments
        .iter()
        .flat_map(|segment| match segment {
            ComposerSegment::Text(text) => text.chars().map(Unit::Text).collect(),
            ComposerSegment::Chip(chip) => vec![Unit::Chip(chip.clone())],
        })
        .collect()
}
fn global_point(document: &ComponentDocumentV2, point: &TextPoint) -> usize {
    let mut offset = 0;
    if let Some(blocks) = model::children(document, None) {
        for block in blocks {
            if block
                .id
                .as_ref()
                .is_some_and(|id| point.container == InlineContainerRef::Block(id.0.clone()))
            {
                return offset + point.offset;
            }
            offset += block
                .content
                .first()
                .map_or(0, |node| model::inline_len(&node.content))
                + 1;
        }
    }
    offset.saturating_sub(1)
}
fn point_at(document: &ComponentDocumentV2, mut offset: usize) -> Option<TextPoint> {
    let blocks = model::children(document, None)?;
    for (index, block) in blocks.iter().enumerate() {
        let length = block
            .content
            .first()
            .map_or(0, |node| model::inline_len(&node.content));
        if offset <= length || index + 1 == blocks.len() {
            return Some(TextPoint::new(
                InlineContainerRef::Block(block.id.as_ref()?.0.clone()),
                offset.min(length),
            ));
        }
        offset = offset.saturating_sub(length + 1);
    }
    None
}

/// Controlled updates keep block identities and map both caret endpoints through
/// the smallest changed interval. Equal values do no work and emit no echo.
pub fn apply_composer_value(
    engine: &mut EditorEngine,
    value: &ComposerValue,
) -> Vec<crate::engine::EngineOutput> {
    let before = blocks_to_composer(engine.document());
    let mut document = composer_to_blocks(value);
    let next = blocks_to_composer(&document);
    if before == next {
        return Vec::new();
    }
    let old = units(&before);
    let new = units(&next);
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let map_offset = |offset: usize| {
        if offset < prefix {
            offset
        } else if offset >= old.len() - suffix {
            new.len() - suffix + offset.saturating_sub(old.len() - suffix)
        } else {
            new.len() - suffix
        }
    };
    let points = engine.selection().text_range().map(|range| {
        (
            map_offset(global_point(engine.document(), &range.anchor)),
            map_offset(global_point(engine.document(), &range.head)),
        )
    });
    if let (Some(old), Some(new)) = (
        model::children(engine.document(), None),
        model::children_mut(&mut document, None),
    ) {
        for (old, new) in old.iter().zip(new) {
            new.id = old.id.clone();
        }
    }
    let selection = points.and_then(|(anchor, head)| {
        Some(EditorSelection::Text(TextRange {
            anchor: point_at(&document, anchor)?,
            head: point_at(&document, head)?,
        }))
    });
    let composing = engine.is_composing();
    let mut output = engine.replace_document(document, crate::HistoryPolicy::Preserve);
    if composing {
        return output;
    }
    if let Some(selection) = selection {
        output.extend(engine.run(crate::engine::Command::SetSelection(selection)));
    }
    output
}

#[component]
pub fn ComposerEditor(
    value: ComposerValue,
    on_change: EventHandler<ComposerValue>,
    triggers: Vec<char>,
    suggestions: SuggestionRegistry,
    on_event: EventHandler<ComposerEvent>,
    #[props(default)] on_ready: EventHandler<ComposerHandle>,
    #[props(default)] placeholder: String,
    #[props(default = "Message".into())] aria_label: String,
    #[props(default)] submit_on: SubmitKey,
    #[props(default)] disabled: bool,
    #[props(default = 5000)] max_inline_paste_chars: usize,
) -> Element {
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    {
        web::editor(
            value,
            on_change,
            triggers,
            suggestions,
            on_event,
            on_ready,
            placeholder,
            aria_label,
            submit_on,
            disabled,
            max_inline_paste_chars,
        )
    }
    #[cfg(not(all(feature = "web", target_arch = "wasm32")))]
    {
        let _ = (triggers, suggestions, max_inline_paste_chars);
        native_editor(
            value,
            on_change,
            on_event,
            on_ready,
            placeholder,
            aria_label,
            submit_on,
            disabled,
        )
    }
}

#[cfg(not(all(feature = "web", target_arch = "wasm32")))]
fn native_editor(
    value: ComposerValue,
    on_change: EventHandler<ComposerValue>,
    on_event: EventHandler<ComposerEvent>,
    on_ready: EventHandler<ComposerHandle>,
    placeholder: String,
    aria_label: String,
    submit_on: SubmitKey,
    disabled: bool,
) -> Element {
    let mut current = use_signal(|| value.clone());
    let alive = use_hook(|| Rc::new(Cell::new(true)));
    let mut mounted = use_signal(|| None::<Rc<MountedData>>);
    let value_writer = current;
    use_effect(use_reactive((&value,), move |(value,)| {
        let mut writer = value_writer;
        if *writer.peek() != value {
            writer.set(value);
        }
    }));
    let is_disabled = use_signal(|| disabled);
    let mut disabled_writer = is_disabled;
    use_effect(use_reactive((&disabled,), move |(disabled,)| {
        disabled_writer.set(disabled)
    }));
    let ready_alive = alive.clone();
    let ready_runtime = dioxus::core::Runtime::current();
    let ready_scope = dioxus::core::current_scope_id();
    use_hook(move || {
        let handle = ComposerHandle {
            run: Rc::new(move |command| {
                if !ready_alive.get() {
                    return;
                }
                ready_runtime.in_scope(ready_scope, || match command {
                    HandleCommand::Focus => {
                        if let Some(node) = mounted.peek().clone() {
                            spawn(async move {
                                let _ = node.set_focus(true).await;
                            });
                        }
                    }
                    HandleCommand::Value(value) => {
                        let mut writer = current;
                        writer.set(value);
                    }
                    _ if *is_disabled.peek() => {}
                    HandleCommand::Clear => {
                        let mut writer = current;
                        let value = ComposerValue::default();
                        writer.set(value.clone());
                        on_change.call(value);
                    }
                    HandleCommand::Text(text) => {
                        let mut value = current.peek().clone();
                        value.segments.push(ComposerSegment::Text(text));
                        let mut writer = current;
                        writer.set(value.clone());
                        on_change.call(value);
                    }
                    HandleCommand::Chip(chip) => {
                        let mut value = current.peek().clone();
                        value.segments.push(ComposerSegment::Chip(chip));
                        let mut writer = current;
                        writer.set(value.clone());
                        on_change.call(value);
                    }
                });
            }),
        };
        on_ready.call(handle);
    });
    use_drop(move || alive.set(false));
    rsx! {
        style { {include_str!("composer.css")} }
        div { class: "dx-composer-editor dx-composer-editor--fallback",
            textarea {
                class: "dx-composer-content", value: composer_plain_text(&current.read()), placeholder, aria_label, disabled,
                onmounted: move |event| mounted.set(Some(event.data())),
                oninput: move |event| { let value = ComposerValue { segments: vec![ComposerSegment::Text(event.value())] }; current.set(value.clone()); on_change.call(value); },
                onkeydown: move |event| {
                    if disabled { return; }
                    let modifiers = event.modifiers();
                    let modified = modifiers.ctrl() || modifiers.meta();
                    if event.key() == Key::Enter && !modifiers.shift() && !modifiers.alt() && (submit_on == SubmitKey::Enter || modified) { event.prevent_default(); on_event.call(ComposerEvent::Submit { alternate: modified }); }
                    else if event.key() == Key::Enter && modified && modifiers.alt() { event.prevent_default(); on_event.call(ComposerEvent::BackgroundSubmit); }
                    else if matches!(event.key(), Key::Character(ref key) if key.eq_ignore_ascii_case("s")) && modified && modifiers.shift() { event.prevent_default(); on_event.call(ComposerEvent::Stash); }
                }
            }
        }
    }
}

#[cfg(all(feature = "web", target_arch = "wasm32"))]
mod web {
    use super::*;
    use crate::dom::{DomHost, Host};
    use crate::engine::{Command, ComposerAction, EngineOptions, EngineOutput, Profile};
    use crate::{EditorCatalog, SuggestionQueryContext};
    use dioxus::core::Task;
    use dioxus::web::WebEventExt;
    use std::cell::RefCell;

    type Runtime = Rc<RefCell<Option<Host>>>;

    #[allow(clippy::too_many_arguments)]
    pub fn editor(
        value: ComposerValue,
        on_change: EventHandler<ComposerValue>,
        triggers: Vec<char>,
        suggestions: SuggestionRegistry,
        on_event: EventHandler<ComposerEvent>,
        on_ready: EventHandler<ComposerHandle>,
        placeholder: String,
        aria_label: String,
        submit_on: SubmitKey,
        disabled: bool,
        max_inline_paste_chars: usize,
    ) -> Element {
        let engine = use_hook(|| {
            let mut engine = EditorEngine::new(
                composer_to_blocks(&value),
                &EditorCatalog::composer(),
                Profile::Composer,
                EngineOptions {
                    read_only: disabled,
                    ..Default::default()
                },
            )
            .expect("composer mapping produces a valid document");
            engine.set_composer_submit_on_enter(submit_on == SubmitKey::Enter);
            let _ = engine.set_suggestion_triggers(triggers.clone());
            Rc::new(RefCell::new(engine))
        });
        let runtime: Runtime = use_hook(|| Rc::new(RefCell::new(None)));
        let alive = use_hook(|| Rc::new(Cell::new(true)));
        let query_task = use_hook(|| Rc::new(RefCell::new(None::<Task>)));
        let state = use_signal(|| engine.borrow().ui_state());
        let mut providers = use_signal(|| suggestions.clone());
        use_effect(use_reactive((&suggestions,), move |(next,)| {
            providers.set(next)
        }));

        let component_runtime = dioxus::core::Runtime::current();
        let component_scope = dioxus::core::current_scope_id();
        let callback = {
            let callback_runtime = component_runtime.clone();
            let weak = Rc::downgrade(&runtime);
            let engine = Rc::downgrade(&engine);
            let query_task = query_task.clone();
            let alive = alive.clone();
            Rc::new(move |outputs: Vec<EngineOutput>| {
                if !alive.get() {
                    return;
                }
                callback_runtime.in_scope(component_scope, || {
                    for output in outputs {
                        match output {
                            EngineOutput::Changed { .. } => {
                                if let Some(engine) = engine.upgrade() {
                                    on_change.call(blocks_to_composer(engine.borrow().document()));
                                }
                            }
                            EngineOutput::Composer(action) => match action {
                                ComposerAction::Submit { alternate } => {
                                    on_event.call(ComposerEvent::Submit { alternate })
                                }
                                ComposerAction::BackgroundSubmit => {
                                    on_event.call(ComposerEvent::BackgroundSubmit)
                                }
                                ComposerAction::HistoryPrevious => {
                                    on_event.call(ComposerEvent::HistoryPrevious)
                                }
                                ComposerAction::HistoryNext => {
                                    on_event.call(ComposerEvent::HistoryNext)
                                }
                                ComposerAction::Stash => on_event.call(ComposerEvent::Stash),
                                ComposerAction::SuggestionAccepted(item) => {
                                    on_event.call(ComposerEvent::SuggestionAccepted(item))
                                }
                                ComposerAction::PlainPaste => {}
                            },
                            EngineOutput::UiState(next) => {
                                if next.suggestion.is_none() {
                                    if let Some(task) = query_task.borrow_mut().take() {
                                        task.cancel();
                                    }
                                }
                                if *state.peek() != next {
                                    let mut state = state;
                                    state.set(next);
                                }
                            }
                            EngineOutput::QuerySuggestions {
                                generation,
                                trigger,
                                query,
                            } => {
                                if let Some(task) = query_task.borrow_mut().take() {
                                    task.cancel();
                                }
                                let provider = trigger
                                    .chars()
                                    .next()
                                    .and_then(|trigger| providers.peek().provider(trigger));
                                let weak = weak.clone();
                                let alive = alive.clone();
                                let task = spawn(async move {
                                    futures_timer::Delay::new(std::time::Duration::from_millis(
                                        120,
                                    ))
                                    .await;
                                    let items = if let Some(provider) = provider {
                                        provider
                                            .query(SuggestionQueryContext {
                                                query,
                                                catalog: EditorCatalog::composer(),
                                            })
                                            .await
                                    } else {
                                        Vec::new()
                                    };
                                    if !alive.get() {
                                        return;
                                    }
                                    let Some(runtime) = weak.upgrade() else {
                                        return;
                                    };
                                    let Some(host) = runtime.borrow().clone() else {
                                        return;
                                    };
                                    let engine = host.borrow().engine.clone();
                                    let output =
                                        engine.borrow_mut().set_suggestion_items(generation, items);
                                    DomHost::publish(&host, output);
                                });
                                query_task.borrow_mut().replace(task);
                            }
                            _ => {}
                        }
                    }
                });
            }) as Rc<dyn Fn(Vec<EngineOutput>)>
        };

        let options_runtime = runtime.clone();
        let options_engine = engine.clone();
        use_effect(use_reactive(
            (
                &value,
                &placeholder,
                &aria_label,
                &triggers,
                &disabled,
                &submit_on,
                &max_inline_paste_chars,
            ),
            move |(
                value,
                placeholder,
                aria_label,
                triggers,
                disabled,
                submit_on,
                max_inline_paste_chars,
            )| {
                let output = {
                    let mut engine = options_engine.borrow_mut();
                    let mut output = apply_composer_value(&mut engine, &value);
                    output.extend(engine.set_read_only(disabled));
                    engine.set_composer_submit_on_enter(submit_on == SubmitKey::Enter);
                    output.extend(engine.set_suggestion_triggers(triggers));
                    output
                };
                if let Some(host) = options_runtime.borrow().clone() {
                    {
                        let mut host = host.borrow_mut();
                        host.set_placeholder(&placeholder);
                        host.set_aria_label(&aria_label);
                        host.set_max_inline_paste_chars(max_inline_paste_chars);
                    }
                    DomHost::publish(&host, output);
                }
            },
        ));
        let drop_runtime = runtime.clone();
        let drop_alive = alive.clone();
        use_drop(move || {
            drop_alive.set(false);
            if let Some(task) = query_task.borrow_mut().take() {
                task.cancel();
            }
            if let Some(host) = drop_runtime.borrow_mut().take() {
                host.borrow_mut().destroy();
            }
        });
        use_effect(move || {
            if let Some(suggestion) = &state.read().suggestion {
                if let Some(element) = web_sys::window()
                    .and_then(|window| window.document())
                    .and_then(|document| {
                        document.get_element_by_id(&format!(
                            "{}-{}",
                            suggestion.listbox_id, suggestion.active_index
                        ))
                    })
                {
                    element.scroll_into_view_with_bool(false);
                }
            }
        });
        let event_runtime = component_runtime.clone();
        let event_alive = alive.clone();
        let host_runtime = runtime.clone();
        let handle_runtime = Rc::downgrade(&runtime);
        let handle_alive = alive.clone();
        let handle = ComposerHandle {
            run: Rc::new(move |command| {
                if !handle_alive.get() {
                    return;
                }
                let Some(runtime) = handle_runtime.upgrade() else {
                    return;
                };
                let Some(host) = runtime.borrow().clone() else {
                    return;
                };
                match command {
                    HandleCommand::Chip(chip) => {
                        DomHost::run(&host, Command::InsertInline(vec![chip_node(&chip)]))
                    }
                    HandleCommand::Text(text) => DomHost::run(&host, Command::InsertText(text)),
                    HandleCommand::Focus => DomHost::run(&host, Command::Focus),
                    HandleCommand::Clear => {
                        DomHost::run(&host, Command::SelectAll);
                        DomHost::run(&host, Command::SelectAll);
                        DomHost::run(&host, Command::InsertText(String::new()));
                    }
                    HandleCommand::Value(value) => {
                        let engine = host.borrow().engine.clone();
                        let output = apply_composer_value(&mut engine.borrow_mut(), &value);
                        DomHost::publish(&host, output);
                    }
                }
            }),
        };
        let current = state.read().clone();
        let listbox = if !current.read_only && !current.composing {
            current.suggestion.clone()
        } else {
            None
        };
        let list_style =
            if current.geometry.viewport.width <= 640. && current.geometry.viewport.width > 0. {
                format!(
                    "left:{}px;top:{}px;width:{}px;max-height:{}px;",
                    current.geometry.viewport.x + 8.,
                    (current.geometry.viewport.y + current.geometry.viewport.height - 320.)
                        .max(current.geometry.viewport.y + 8.),
                    (current.geometry.viewport.width - 16.).max(0.),
                    (current.geometry.viewport.height - 16.).min(320.)
                )
            } else {
                let anchor = current.geometry.caret.unwrap_or_default();
                let viewport = current.geometry.viewport;
                let x = anchor
                    .x
                    .max(viewport.x + 8.)
                    .min((viewport.x + viewport.width - 360.).max(viewport.x + 8.));
                let y = if anchor.y + anchor.height + 328. > viewport.y + viewport.height {
                    (anchor.y - 320.).max(viewport.y + 8.)
                } else {
                    anchor.y + anchor.height + 6.
                };
                format!("left:{x}px;top:{y}px;")
            };
        rsx! {
            style { {include_str!("composer.css")} }
            div { class: "dx-composer-editor", "data-dxeditor-profile": "composer",
                div { "data-dxeditor-host": "", class: "dxeditor__document",
                    onmounted: move |event| {
                        let Some(element) = event.data().try_as_web_event() else { return; };
                        if let Some(previous) = host_runtime.borrow_mut().take() { previous.borrow_mut().destroy(); }
                        match DomHost::mount(element, engine.clone(), callback.clone()) {
                            Ok(host) => {
                                { let mut host = host.borrow_mut(); host.set_placeholder(&placeholder); host.set_aria_label(&aria_label); host.set_max_inline_paste_chars(max_inline_paste_chars); host.set_composer_event_callback(Rc::new({let event_runtime = event_runtime.clone(); let event_alive = event_alive.clone(); move |event| { if event_alive.get() { event_runtime.in_scope(component_scope, || on_event.call(event)); } }})); }
                                host_runtime.borrow_mut().replace(host);
                                on_ready.call(handle.clone());
                            }
                            Err(_) => {}
                        }
                    }
                }
                div { class: "dxeditor__overlays", "data-dxeditor-overlays": "",
                    if let Some(suggestion) = listbox {
                        div { id: suggestion.listbox_id.clone(), class: "dx-composer-suggestions", role: "listbox", aria_label: "Suggestions", style: list_style,
                            if suggestion.searching { div { role: "status", "Searching…" } }
                            else if suggestion.items.is_empty() { div { role: "status", "No suggestions" } }
                            for (index, item) in suggestion.items.iter().enumerate() {
                                if let Some(group) = item.data.get("group").and_then(serde_json::Value::as_str).filter(|group| index == 0 || suggestion.items[index - 1].data.get("group").and_then(serde_json::Value::as_str) != Some(*group)) {
                                    div { class: "dx-composer-group", role: "presentation", "{group}" }
                                }
                                div {
                                    id: format!("{}-{index}", suggestion.listbox_id), role: "option",
                                    aria_selected: index == suggestion.active_index,
                                    aria_disabled: item.data.get("disabled").and_then(serde_json::Value::as_bool).unwrap_or(false),
                                    onmousedown: move |event| event.prevent_default(),
                                    onclick: {
                                        let runtime = runtime.clone(); let disabled = item.data.get("disabled").and_then(serde_json::Value::as_bool).unwrap_or(false);
                                        move |_| {
                                            if disabled { return; }
                                            if let Some(host) = runtime.borrow().clone() {
                                                host.borrow().engine.borrow_mut().suggestion.active_index = index;
                                                DomHost::run(&host, Command::AcceptSuggestion);
                                            }
                                        }
                                    },
                                    strong { "{item.label}" }
                                    if let Some(description) = &item.description { small { "{description}" } }
                                }
                            }
                        }
                    }
                }
                div { class: "dxeditor__live", aria_live: "polite", role: "status" }
            }
        }
    }
}
