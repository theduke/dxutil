use crate::scope::FormAccessors;
use std::rc::Rc;

use dioxus::prelude::*;
use futures::FutureExt;

use crate::{
    DynValidator, FieldPath, FormNodeKind, FormNodeState, FormRoot, FormScope, ListItemKey,
    ListMeta, ListValidationContext, ListValidator, ValidationPhase, ValidationStrategy,
    set_signal_if_changed,
};

pub struct ListSpec<Parent, Item> {
    pub name: String,
    pub get: crate::ValueGetter<Parent, Vec<Item>>,
    pub set: crate::ValueSetter<Parent, Vec<Item>>,
    pub item_empty: Rc<dyn Fn(&Item) -> bool>,
    pub validators: Vec<ListValidator<Item>>,
    pub validation: ValidationStrategy,
}

impl<Parent, Item> ListSpec<Parent, Item>
where
    Item: Default + PartialEq + 'static,
{
    pub fn new(
        name: impl Into<String>,
        get: impl Fn(&Parent) -> Vec<Item> + 'static,
        set: impl Fn(&mut Parent, Vec<Item>) + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            get: Rc::new(get),
            set: Rc::new(set),
            item_empty: Rc::new(|item: &Item| item == &Item::default()),
            validators: Vec::new(),
            validation: ValidationStrategy::default(),
        }
    }
}

impl<Parent, Item> ListSpec<Parent, Item> {
    pub fn validator(mut self, validator: ListValidator<Item>) -> Self {
        self.validators.push(validator);
        self
    }

    pub fn item_empty(mut self, f: impl Fn(&Item) -> bool + 'static) -> Self {
        self.item_empty = Rc::new(f);
        self
    }

    pub fn validation(mut self, strategy: ValidationStrategy) -> Self {
        self.validation = strategy;
        self
    }
}

pub struct ListHandle<Item, Root = Item>
where
    Root: Clone + PartialEq + 'static,
    Item: Clone + PartialEq + 'static,
{
    pub(crate) root: FormRoot<Root>,
    pub(crate) path: FieldPath,
    get: crate::ValueGetter<Root, Vec<Item>>,
    set: crate::ValueSetter<Root, Vec<Item>>,
    pure_get: crate::ValueGetter<Root, Vec<Item>>,
    pure_set: crate::ValueSetter<Root, Vec<Item>>,
    item_empty: Rc<dyn Fn(&Item) -> bool>,
    value_signal: Signal<Vec<Item>>,
    initial_value_signal: Signal<Vec<Item>>,
    key_signal: Signal<Vec<ListItemKey>>,
    meta_signal: Signal<ListMeta>,
}

impl<Item, Root> Clone for ListHandle<Item, Root>
where
    Root: Clone + PartialEq + 'static,
    Item: Clone + PartialEq + 'static,
{
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            path: self.path.clone(),
            get: self.get.clone(),
            set: self.set.clone(),
            pure_get: self.pure_get.clone(),
            pure_set: self.pure_set.clone(),
            item_empty: self.item_empty.clone(),
            value_signal: self.value_signal,
            initial_value_signal: self.initial_value_signal,
            key_signal: self.key_signal,
            meta_signal: self.meta_signal,
        }
    }
}

impl<Item, Root> PartialEq for ListHandle<Item, Root>
where
    Root: Clone + PartialEq + 'static,
    Item: Clone + PartialEq + 'static,
{
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root && self.path == other.path
    }
}

impl<Item, Root> ListHandle<Item, Root>
where
    Root: Clone + PartialEq + 'static,
    Item: Clone + PartialEq + 'static,
{
    pub(crate) fn new<Parent>(scope: &FormScope<Parent, Root>, spec: ListSpec<Parent, Item>) -> Self
    where
        Parent: Clone + PartialEq + 'static,
    {
        let path = scope.path.child(&spec.name);
        let parent_value_signal_for_child = scope.value_signal;
        let parent_initial_value_signal_for_child = scope.initial_value_signal;
        let parent_value_signal_for_set = scope.value_signal;
        let parent_set = scope.set.clone();
        let parent_pure_get_for_child = scope.pure_get.clone();
        let parent_pure_get_for_set = scope.pure_get.clone();
        let parent_pure_set = scope.pure_set.clone();
        let list_get = spec.get.clone();
        let list_initial_get = spec.get.clone();
        let list_pure_get = spec.get.clone();
        let list_set = spec.set.clone();
        let list_pure_set = spec.set.clone();
        let get: crate::ValueGetter<Root, Vec<Item>> = Rc::new(move |_root| {
            let parent = parent_value_signal_for_child.peek().clone();
            list_get(&parent)
        });
        let initial_get: crate::ValueGetter<Root, Vec<Item>> = Rc::new(move |_root| {
            let parent = parent_initial_value_signal_for_child.peek().clone();
            list_initial_get(&parent)
        });
        let set: crate::ValueSetter<Root, Vec<Item>> = Rc::new(move |root, value| {
            let mut parent = parent_value_signal_for_set.peek().clone();
            list_set(&mut parent, value);
            set_signal_if_changed(parent_value_signal_for_set, parent.clone());
            parent_set(root, parent);
        });
        let pure_get: crate::ValueGetter<Root, Vec<Item>> = Rc::new(move |root| {
            let parent = parent_pure_get_for_child(root);
            list_pure_get(&parent)
        });
        let pure_set: crate::ValueSetter<Root, Vec<Item>> = Rc::new(move |root, value| {
            let mut parent = parent_pure_get_for_set(root);
            list_pure_set(&mut parent, value);
            parent_pure_set(root, parent);
        });
        let validators = spec
            .validators
            .into_iter()
            .map(|validator| {
                let get = get.clone();
                let path = path.clone();
                Rc::new(move |phase, root: Root| {
                    let validator = validator.clone();
                    let path = path.clone();
                    let items = get(&root);
                    async move {
                        validator
                            .validate(ListValidationContext { phase, path, items })
                            .await
                    }
                    .boxed_local()
                }) as DynValidator<Root>
            })
            .collect::<Vec<_>>();
        let current = get(&scope.root.state.values.peek());
        let initial = initial_get(&scope.root.state.initial_values.peek());
        let has_keys = scope
            .root
            .state
            .registry
            .peek()
            .list_keys
            .contains_key(&path);
        let new_keys = if has_keys {
            Vec::new()
        } else {
            let len = current.len();
            scope.root.allocate_keys(len)
        };
        let initial_keys = if has_keys {
            scope
                .root
                .state
                .registry
                .peek()
                .list_keys
                .get(&path)
                .cloned()
                .unwrap_or_default()
        } else {
            new_keys.clone()
        };
        let owner = scope.root.state.owner;
        let (value_signal, initial_value_signal, key_signal, meta_signal) =
            scope.root.with_registry(|registry| {
                let value_signal = registry.ensure_value_signal(&path, current.clone(), owner);
                let initial_value_signal =
                    registry.ensure_initial_value_signal(&path, initial.clone(), owner);
                let key_signal = registry.ensure_list_key_signal(&path, initial_keys, owner);
                let node = registry
                    .nodes
                    .entry(path.clone())
                    .or_insert_with(|| FormNodeState::new(FormNodeKind::List, path.clone(), owner));
                node.validators = validators;
                node.current_value_applier = Some({
                    let pure_set = pure_set.clone();
                    Rc::new(move |root: &mut Root| {
                        pure_set(root, value_signal.peek().clone());
                    })
                });
                registry.list_keys.entry(path.clone()).or_insert(new_keys);
                node.value_refresher = Some({
                    let get = get.clone();
                    Rc::new(move |root: &Root| {
                        set_signal_if_changed(value_signal, get(root));
                    })
                });
                node.initial_value_refresher = Some({
                    let initial_get = initial_get.clone();
                    Rc::new(move |root: &Root| {
                        set_signal_if_changed(initial_value_signal, initial_get(root));
                    })
                });
                (
                    value_signal,
                    initial_value_signal,
                    key_signal,
                    node.list_meta_signal,
                )
            });
        let handle = Self {
            root: scope.root.clone(),
            path,
            get,
            set,
            pure_get,
            pure_set,
            item_empty: spec.item_empty,
            value_signal,
            initial_value_signal,
            key_signal,
            meta_signal,
        };
        handle.rebuild_item_nodes();
        handle.refresh_node_state_local();
        handle
    }

    pub fn path(&self) -> FieldPath {
        self.path.clone()
    }

    pub fn meta(&self) -> ListMeta {
        self.meta_signal.read().clone()
    }

    pub fn meta_signal(&self) -> ReadSignal<ListMeta> {
        self.meta_signal.into()
    }

    pub fn values(&self) -> Vec<Item> {
        self.value_signal.read().clone()
    }

    pub fn values_signal(&self) -> ReadSignal<Vec<Item>> {
        self.value_signal.into()
    }

    pub fn keys_signal(&self) -> ReadSignal<Vec<ListItemKey>> {
        self.key_signal.into()
    }

    pub fn items(&self) -> Vec<ListItemHandle<Item, Root>> {
        let keys = self.key_signal.read().clone();
        let values = self.value_signal.read().clone();
        keys.into_iter()
            .zip(values)
            .enumerate()
            .map(|(index, (key, value))| ListItemHandle {
                list: self.clone(),
                index,
                key,
                value,
            })
            .collect()
    }

    pub fn len(&self) -> usize {
        self.values().len()
    }

    pub fn is_empty(&self) -> bool {
        self.values().is_empty()
    }

    pub fn push(&self, value: Item) {
        self.insert(self.len(), value);
    }

    pub fn insert(&self, index: usize, value: Item) {
        let mut values = self.values();
        let index = index.min(values.len());
        values.insert(index, value);
        let set = self.set.clone();
        set_signal_if_changed(self.value_signal, values.clone());
        self.root
            .mutate_values_at(self.path.clone(), move |root| set(root, values));
        let key = self.root.allocate_key();
        self.root.with_registry(|registry| {
            registry
                .list_keys
                .entry(self.path.clone())
                .or_default()
                .insert(index, key);
        });
        let mut keys = self.key_signal.read().clone();
        keys.insert(index, key);
        set_signal_if_changed(self.key_signal, keys);
        self.refresh_node_state();
    }

    pub fn remove(&self, index: usize) -> Option<Item> {
        let mut values = self.values();
        if index >= values.len() {
            return None;
        }
        let removed = values.remove(index);
        let set = self.set.clone();
        set_signal_if_changed(self.value_signal, values.clone());
        self.root
            .mutate_values_at(self.path.clone(), move |root| set(root, values));
        let item_path = self.path.index(index);
        let mut keys = self.key_signal.read().clone();
        self.root.with_registry(|registry| {
            if let Some(keys) = registry.list_keys.get_mut(&self.path)
                && index < keys.len()
            {
                keys.remove(index);
            }
            registry.remove_descendants(&item_path);
        });
        if index < keys.len() {
            keys.remove(index);
        }
        set_signal_if_changed(self.key_signal, keys);
        self.rebuild_item_nodes();
        self.refresh_node_state();
        Some(removed)
    }

    pub fn clear(&self) {
        let set = self.set.clone();
        set_signal_if_changed(self.value_signal, Vec::new());
        self.root
            .mutate_values_at(self.path.clone(), move |root| set(root, Vec::new()));
        self.root.with_registry(|registry| {
            registry.list_keys.insert(self.path.clone(), Vec::new());
            let path = self.path.clone();
            registry
                .nodes
                .retain(|node_path, _| !node_path.starts_with(&path) || node_path == &path);
        });
        set_signal_if_changed(self.key_signal, Vec::new());
        self.refresh_node_state();
    }

    pub fn swap(&self, a: usize, b: usize) {
        let mut values = self.values();
        if a >= values.len() || b >= values.len() {
            return;
        }
        values.swap(a, b);
        let set = self.set.clone();
        set_signal_if_changed(self.value_signal, values.clone());
        self.root
            .mutate_values_at(self.path.clone(), move |root| set(root, values));
        let mut keys = self.key_signal.read().clone();
        self.root.with_registry(|registry| {
            if let Some(keys) = registry.list_keys.get_mut(&self.path)
                && a < keys.len()
                && b < keys.len()
            {
                keys.swap(a, b);
            }
        });
        if a < keys.len() && b < keys.len() {
            keys.swap(a, b);
            set_signal_if_changed(self.key_signal, keys);
        }
        self.rebuild_item_nodes();
        self.refresh_node_state();
    }

    pub fn move_item(&self, from: usize, to: usize) {
        let mut values = self.values();
        if from >= values.len() || to >= values.len() || from == to {
            return;
        }
        let value = values.remove(from);
        values.insert(to, value);
        let set = self.set.clone();
        set_signal_if_changed(self.value_signal, values.clone());
        self.root
            .mutate_values_at(self.path.clone(), move |root| set(root, values));
        let mut keys = self.key_signal.read().clone();
        self.root.with_registry(|registry| {
            if let Some(keys) = registry.list_keys.get_mut(&self.path)
                && from < keys.len()
                && to < keys.len()
            {
                let key = keys.remove(from);
                keys.insert(to, key);
            }
        });
        if from < keys.len() && to < keys.len() {
            let key = keys.remove(from);
            keys.insert(to, key);
            set_signal_if_changed(self.key_signal, keys);
        }
        self.rebuild_item_nodes();
        self.refresh_node_state();
    }

    pub fn reset(&self) {
        let initial = self.initial_value_signal.peek().clone();
        let set = self.set.clone();
        set_signal_if_changed(self.value_signal, initial.clone());
        self.root
            .mutate_values_at(self.path.clone(), move |root| set(root, initial));
        self.root.with_registry(|registry| {
            registry.list_keys.remove(&self.path);
        });
        set_signal_if_changed(self.key_signal, Vec::new());
        self.ensure_key_count();
        self.refresh_node_state();
    }

    pub async fn validate(&self) -> std::result::Result<(), Vec<crate::FormError>> {
        self.root
            .validate_path(self.path.clone(), ValidationPhase::Manual)
            .await
    }

    pub fn add_handler(&self, new_item: Rc<dyn Fn() -> Item>) -> EventHandler<MouseEvent> {
        let list = self.clone();
        EventHandler::new(move |_| list.push(new_item()))
    }

    pub fn clear_handler(&self) -> EventHandler<MouseEvent> {
        let list = self.clone();
        EventHandler::new(move |_| list.clear())
    }

    fn ensure_key_count(&self) {
        let len = self.values().len();
        let current_len = self
            .root
            .state
            .registry
            .peek()
            .list_keys
            .get(&self.path)
            .map_or(0, Vec::len);
        let missing = len.saturating_sub(current_len);
        let new_keys = self.root.allocate_keys(missing);
        self.root.with_registry(|registry| {
            let keys = registry.list_keys.entry(self.path.clone()).or_default();
            keys.truncate(len);
            keys.extend(new_keys);
            set_signal_if_changed(self.key_signal, keys.clone());
        });
        self.rebuild_item_nodes();
    }

    fn rebuild_item_nodes(&self) {
        let keys = self.key_signal.read().clone();
        self.root.with_registry(|registry| {
            for (index, _) in keys.iter().enumerate() {
                let path = self.path.index(index);
                registry.ensure_node(FormNodeKind::ListItem, path, self.root.state.owner);
            }
        });
    }

    fn refresh_node_state(&self) {
        self.refresh_node_state_local();
        self.root.recompute_all_meta();
    }

    fn refresh_node_state_local(&self) {
        let current = self.value_signal.peek().clone();
        let initial = self.initial_value_signal.peek().clone();
        self.root.with_registry(|registry| {
            let node = registry.nodes.entry(self.path.clone()).or_insert_with(|| {
                FormNodeState::new(FormNodeKind::List, self.path.clone(), self.root.state.owner)
            });
            node.dirty = current != initial;
            node.empty = current.is_empty() || current.iter().all(|item| (self.item_empty)(item));
            node.sync_meta_signals();
        });
    }
}

pub fn use_list<Parent, Item, Root>(
    scope: FormScope<Parent, Root>,
    spec: impl FnOnce() -> ListSpec<Parent, Item>,
) -> ListHandle<Item, Root>
where
    Root: Clone + PartialEq + 'static,
    Parent: Clone + PartialEq + 'static,
    Item: Clone + PartialEq + 'static,
{
    scope.list(spec())
}

pub struct ListItemHandle<Item, Root = Item>
where
    Root: Clone + PartialEq + 'static,
    Item: Clone + PartialEq + 'static,
{
    pub(crate) list: ListHandle<Item, Root>,
    index: usize,
    key: ListItemKey,
    value: Item,
}

pub fn use_list_item_scope<Item, Root>(item: ListItemHandle<Item, Root>) -> FormScope<Item, Root>
where
    Root: Clone + PartialEq + 'static,
    Item: Clone + PartialEq + 'static,
{
    item.scope()
}

impl<Item, Root> Clone for ListItemHandle<Item, Root>
where
    Root: Clone + PartialEq + 'static,
    Item: Clone + PartialEq + 'static,
{
    fn clone(&self) -> Self {
        Self {
            list: self.list.clone(),
            index: self.index,
            key: self.key,
            value: self.value.clone(),
        }
    }
}

impl<Item, Root> PartialEq for ListItemHandle<Item, Root>
where
    Root: Clone + PartialEq + 'static,
    Item: Clone + PartialEq + 'static,
{
    fn eq(&self, other: &Self) -> bool {
        self.list == other.list && self.key == other.key
    }
}

impl<Item, Root> ListItemHandle<Item, Root>
where
    Root: Clone + PartialEq + 'static,
    Item: Clone + PartialEq + 'static,
{
    pub fn key(&self) -> u64 {
        self.key
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn path(&self) -> FieldPath {
        self.list.path.index(self.index)
    }

    pub fn scope(&self) -> FormScope<Item, Root> {
        let value_signal_for_get = self.list.value_signal;
        let value_signal_for_set = self.list.value_signal;
        let initial_value_signal_for_get = self.list.initial_value_signal;
        let list_set = self.list.set.clone();
        let list_pure_get_for_item = self.list.pure_get.clone();
        let list_pure_get_for_set = self.list.pure_get.clone();
        let list_pure_set = self.list.pure_set.clone();
        let index = self.index;
        let fallback = self.value.clone();
        let initial_fallback = self.value.clone();
        let get = Rc::new(move |_root: &Root| {
            value_signal_for_get
                .peek()
                .get(index)
                .cloned()
                .unwrap_or_else(|| fallback.clone())
        });
        let initial_get = Rc::new(move |_root: &Root| {
            initial_value_signal_for_get
                .peek()
                .get(index)
                .cloned()
                .unwrap_or_else(|| initial_fallback.clone())
        });
        let set = Rc::new(move |root: &mut Root, item: Item| {
            let mut items = value_signal_for_set.peek().clone();
            if index < items.len() {
                items[index] = item;
                set_signal_if_changed(value_signal_for_set, items.clone());
                list_set(root, items);
            }
        });
        let pure_get = Rc::new(move |root: &Root| list_pure_get_for_item(root)[index].clone());
        let pure_set = Rc::new(move |root: &mut Root, item: Item| {
            let mut items = list_pure_get_for_set(root);
            if index < items.len() {
                items[index] = item;
                list_pure_set(root, items);
            }
        });
        FormScope::new_with_accessors(
            self.list.root.clone(),
            self.path(),
            FormAccessors {
                get,
                initial_get,
                set,
                pure_get,
                pure_set,
                is_empty: self.list.item_empty.clone(),
            },
        )
    }

    pub fn remove(&self) {
        self.list.remove(self.index);
    }

    pub fn remove_handler(&self) -> EventHandler<MouseEvent> {
        let item = self.clone();
        EventHandler::new(move |_| item.remove())
    }
}

pub fn provide_list_item<Item, Root>(item: ListItemHandle<Item, Root>) -> ListItemHandle<Item, Root>
where
    Root: Clone + PartialEq + 'static,
    Item: Clone + PartialEq + 'static,
{
    provide_context(item)
}

pub fn use_list_item<Item, Root>() -> ListItemHandle<Item, Root>
where
    Root: Clone + PartialEq + 'static,
    Item: Clone + PartialEq + 'static,
{
    use_context()
}

#[allow(non_snake_case)]
pub fn ListAddButton<Item, Root>(
    list: ListHandle<Item, Root>,
    new_item: Rc<dyn Fn() -> Item>,
    disabled: Option<bool>,
    children: Element,
) -> Element
where
    Root: Clone + PartialEq + 'static,
    Item: Clone + PartialEq + 'static,
{
    rsx! {
        button {
            r#type: "button",
            disabled: disabled.unwrap_or(false),
            onclick: list.add_handler(new_item),
            {children}
        }
    }
}

#[allow(non_snake_case)]
pub fn ListClearButton<Item, Root>(
    list: ListHandle<Item, Root>,
    disabled: Option<bool>,
    children: Element,
) -> Element
where
    Root: Clone + PartialEq + 'static,
    Item: Clone + PartialEq + 'static,
{
    rsx! {
        button {
            r#type: "button",
            disabled: disabled.unwrap_or(false),
            onclick: list.clear_handler(),
            {children}
        }
    }
}

#[allow(non_snake_case)]
pub fn ListItemRemoveButton<Item, Root>(
    item: ListItemHandle<Item, Root>,
    disabled: Option<bool>,
    children: Element,
) -> Element
where
    Root: Clone + PartialEq + 'static,
    Item: Clone + PartialEq + 'static,
{
    rsx! {
        button {
            r#type: "button",
            disabled: disabled.unwrap_or(false),
            onclick: item.remove_handler(),
            {children}
        }
    }
}
