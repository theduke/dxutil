//! Dioxus-owned roots adopted by the Rust DOM renderer.
use crate::{AttributeMap, ComponentEditorHandle, ComponentId, ComponentNode};
use dioxus::prelude::*;
use std::{collections::BTreeMap, rc::Rc, time::Duration};

pub type IslandRenderer = Rc<dyn Fn(IslandContext) -> Element>;

#[derive(Clone, Default)]
pub struct IslandRegistry {
    renderers: BTreeMap<ComponentId, IslandRenderer>,
}
impl IslandRegistry {
    pub fn register(&mut self, kind: impl Into<ComponentId>, renderer: IslandRenderer) {
        self.renderers.insert(kind.into(), renderer);
    }
    pub fn renderer(&self, kind: &ComponentId) -> Option<IslandRenderer> {
        self.renderers.get(kind).cloned()
    }
}
impl PartialEq for IslandRegistry {
    fn eq(&self, other: &Self) -> bool {
        self.renderers.len() == other.renderers.len()
            && self.renderers.iter().all(|(key, value)| {
                other
                    .renderers
                    .get(key)
                    .is_some_and(|other| Rc::ptr_eq(value, other))
            })
    }
}

#[derive(Clone)]
pub struct IslandContext {
    pub uid: String,
    pub kind: ComponentId,
    pub attrs: ReadSignal<AttributeMap>,
    pub transient: ReadSignal<AttributeMap>,
    pub selected: ReadSignal<bool>,
    pub visible: ReadSignal<bool>,
    pub read_only: bool,
    pub handle: ComponentEditorHandle,
}
impl IslandContext {
    pub fn update_attrs(&self, attrs: AttributeMap) {
        self.handle.update_node_attrs(self.uid.clone(), attrs, true);
    }
    pub fn select(&self) {
        self.handle.select_node(self.uid.clone());
    }
    pub fn remove(&self) {
        self.handle.remove_node(self.uid.clone());
    }
    pub fn replace_with(&self, nodes: Vec<ComponentNode>) {
        self.handle.replace_node(self.uid.clone(), nodes);
    }
}

#[derive(Clone, PartialEq)]
pub struct IslandState {
    pub kind: ComponentId,
    pub attrs: Signal<AttributeMap>,
    pub transient: Signal<AttributeMap>,
    pub selected: Signal<bool>,
    pub visible: Signal<bool>,
    generation: u64,
    removal_pending: bool,
}

#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub(crate) fn sync_engine(
    engine: &crate::engine::EditorEngine,
    states: Signal<BTreeMap<String, IslandState>>,
) {
    sync_engine_changes(engine, states, None);
}

/// An ordinary text transaction need only inspect dirty blocks and the mounted
/// island set. Full tree walks are reserved for structural renders and mount.
pub(crate) fn sync_engine_changes(
    engine: &crate::engine::EditorEngine,
    mut states: Signal<BTreeMap<String, IslandState>>,
    dirty: Option<&[String]>,
) {
    use crate::engine::EditorSelection;
    fn collect<'a>(node: &'a ComponentNode, islands: &mut Vec<(&'a str, &'a ComponentNode)>) {
        if node.kind.0 == "block"
            && let (Some(uid), Some(content)) = (node.id.as_ref(), node.content.first())
            && crate::engine::identity::is_island_kind(&content.kind.0)
        {
            islands.push((&uid.0, content));
        }
        for child in &node.content {
            collect(child, islands);
        }
    }
    let mut islands = Vec::new();
    if let Some(dirty) = dirty {
        for uid in dirty {
            if let Some(content) = engine
                .content(uid)
                .filter(|content| crate::engine::identity::is_island_kind(&content.kind.0))
            {
                islands.push((uid.as_str(), content));
            }
        }
    } else {
        collect(&engine.document().root, &mut islands);
    }
    if islands.is_empty() && states.peek().is_empty() {
        return;
    }
    let owner = states.origin_scope();
    let mut live = if dirty.is_some() {
        states.peek().keys().cloned().collect()
    } else {
        std::collections::BTreeSet::new()
    };
    if let Some(dirty) = dirty {
        for uid in dirty {
            live.remove(uid);
        }
    }
    let selected: std::collections::BTreeSet<String> = match engine.selection() {
        EditorSelection::Node(uid) => [uid.clone()].into_iter().collect(),
        EditorSelection::Blocks { anchor, head } => {
            let order = &engine.index.order;
            let a = order
                .iter()
                .position(|uid| uid == anchor)
                .unwrap_or_default();
            let b = order.iter().position(|uid| uid == head).unwrap_or(a);
            order[a.min(b)..=a.max(b)].iter().cloned().collect()
        }
        _ => Default::default(),
    };
    for (uid, content) in islands {
        live.insert(uid.to_string());
        let existing = states.peek().get(uid).cloned();
        if let Some(mut state) = existing {
            if *state.attrs.peek() != content.attrs {
                state.attrs.set(content.attrs.clone());
            }
            let transient = engine.transient(uid).cloned().unwrap_or_default();
            if *state.transient.peek() != transient {
                state.transient.set(transient);
            }
            let is_selected = selected.contains(uid);
            if *state.selected.peek() != is_selected {
                state.selected.set(is_selected);
            }
            // Only republish the map when a root returns during its grace period
            // or changes kind. Ordinary text edits leave the island VDOM alone.
            if state.removal_pending || state.kind != content.kind {
                state.removal_pending = false;
                state.generation += 1;
                state.kind = content.kind.clone();
                states.write().insert(uid.to_string(), state);
            }
        } else {
            states.write().insert(
                uid.to_string(),
                IslandState {
                    kind: content.kind.clone(),
                    attrs: Signal::new_in_scope(content.attrs.clone(), owner),
                    transient: Signal::new_in_scope(
                        engine.transient(uid).cloned().unwrap_or_default(),
                        owner,
                    ),
                    selected: Signal::new_in_scope(selected.contains(uid), owner),
                    visible: Signal::new_in_scope(false, owner),
                    generation: 0,
                    removal_pending: false,
                },
            );
        }
    }
    for (uid, state) in states.peek().iter() {
        let is_selected = selected.contains(uid);
        if *state.selected.peek() != is_selected {
            let mut signal = state.selected;
            signal.set(is_selected);
        }
    }
    let removed: Vec<_> = states
        .peek()
        .iter()
        .filter(|(uid, state)| !live.contains(*uid) && !state.removal_pending)
        .map(|(uid, _)| uid.clone())
        .collect();
    for uid in removed {
        let generation = {
            let mut map = states.write();
            let state = map.get_mut(&uid).expect("removed island exists");
            state.removal_pending = true;
            state.generation += 1;
            state.generation
        };
        spawn(async move {
            futures_timer::Delay::new(Duration::from_millis(250)).await;
            let should_remove = states.try_peek().is_ok_and(|map| {
                map.get(&uid)
                    .is_some_and(|state| state.removal_pending && state.generation == generation)
            });
            if should_remove {
                states.write().remove(&uid);
            }
        });
    }
}

#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub(crate) fn set_visibility(
    states: Signal<BTreeMap<String, IslandState>>,
    uid: &str,
    visible: bool,
) {
    if let Ok(map) = states.try_peek()
        && let Some(state) = map.get(uid)
    {
        let mut signal = state.visible;
        if *signal.peek() != visible {
            signal.set(visible);
        }
    }
}

#[component]
pub(crate) fn IslandParking(
    session_id: String,
    states: Signal<BTreeMap<String, IslandState>>,
    registry: IslandRegistry,
    read_only: bool,
    handle: ComponentEditorHandle,
) -> Element {
    rsx! {
        div { id: "dxeditor-islands-{session_id}", style: "position:absolute;width:0;height:0;overflow:hidden;",
            for (uid, state) in states.read().iter() {
                IslandRoot { key: "{uid}", uid: uid.clone(), session_id: session_id.clone(), state: state.clone(), registry: registry.clone(), read_only, handle: handle.clone() }
            }
        }
    }
}
#[component]
fn IslandRoot(
    uid: String,
    session_id: String,
    state: IslandState,
    registry: IslandRegistry,
    read_only: bool,
    handle: ComponentEditorHandle,
) -> Element {
    let renderer = registry.renderer(&state.kind);
    let context = IslandContext {
        uid: uid.clone(),
        kind: state.kind.clone(),
        attrs: state.attrs.into(),
        transient: state.transient.into(),
        selected: state.selected.into(),
        visible: state.visible.into(),
        read_only,
        handle: handle.clone(),
    };
    rsx! {
        div { id: "dxeditor-island-{session_id}-{uid}", class: "dxeditor-island-root", onmounted: move |_| handle.island_ready(uid.clone()),
            if let Some(renderer) = renderer { {renderer(context)} } else { div { class: "dxeditor-island-unsupported", "Unsupported block" } }
        }
    }
}
