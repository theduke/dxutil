use std::rc::Rc;

use dioxus::prelude::*;
use futures::FutureExt;

use crate::{
    DynValidator, FieldMeta, FieldPath, FieldValidationContext, FieldValidator, FormError,
    FormErrorSource, FormNodeKind, FormNodeState, FormRoot, FormScope, ValidationPhase,
    ValidationStrategy, set_signal_if_changed,
};

pub type ValueGetter<Root, Value> = Rc<dyn Fn(&Root) -> Value>;
pub type ValueSetter<Root, Value> = Rc<dyn Fn(&mut Root, Value)>;
pub type DraftParser<Draft, Value> = Rc<dyn Fn(&Draft) -> std::result::Result<Value, FormError>>;

pub struct FieldSpec<Parent, Value, Draft = Value> {
    pub name: String,
    pub get: crate::ValueGetter<Parent, Value>,
    pub set: crate::ValueSetter<Parent, Value>,
    pub format: Rc<dyn Fn(&Value) -> Draft>,
    pub parse: crate::DraftParser<Draft, Value>,
    pub is_empty: Rc<dyn Fn(&Draft) -> bool>,
    pub validators: Vec<FieldValidator<Parent, Value>>,
    pub validation: ValidationStrategy,
}

impl<Parent, Value> FieldSpec<Parent, Value>
where
    Value: Clone + Default + PartialEq + 'static,
{
    pub fn new(
        name: impl Into<String>,
        get: impl Fn(&Parent) -> Value + 'static,
        set: impl Fn(&mut Parent, Value) + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            get: Rc::new(get),
            set: Rc::new(set),
            format: Rc::new(|value: &Value| value.clone()),
            parse: Rc::new(|value: &Value| Ok(value.clone())),
            is_empty: Rc::new(|value: &Value| value == &Value::default()),
            validators: Vec::new(),
            validation: ValidationStrategy::default(),
        }
    }
}

impl<Parent, Value, Draft> FieldSpec<Parent, Value, Draft> {
    pub fn validator(mut self, validator: FieldValidator<Parent, Value>) -> Self {
        self.validators.push(validator);
        self
    }

    pub fn is_empty(mut self, f: impl Fn(&Draft) -> bool + 'static) -> Self {
        self.is_empty = Rc::new(f);
        self
    }

    pub fn validation(mut self, strategy: ValidationStrategy) -> Self {
        self.validation = strategy;
        self
    }

    pub fn with_draft<NextDraft>(
        self,
        format: impl Fn(&Value) -> NextDraft + 'static,
        parse: impl Fn(&NextDraft) -> std::result::Result<Value, FormError> + 'static,
        is_empty: impl Fn(&NextDraft) -> bool + 'static,
    ) -> FieldSpec<Parent, Value, NextDraft> {
        FieldSpec {
            name: self.name,
            get: self.get,
            set: self.set,
            format: Rc::new(format),
            parse: Rc::new(parse),
            is_empty: Rc::new(is_empty),
            validators: self.validators,
            validation: self.validation,
        }
    }
}

pub struct FieldHandle<Value, Root = Value, Draft = Value>
where
    Root: Clone + PartialEq + 'static,
    Value: Clone + PartialEq + 'static,
    Draft: Clone + PartialEq + 'static,
{
    pub(crate) root: FormRoot<Root>,
    pub(crate) path: FieldPath,
    get: crate::ValueGetter<Root, Value>,
    set: crate::ValueSetter<Root, Value>,
    pure_set: crate::ValueSetter<Root, Value>,
    format: Rc<dyn Fn(&Value) -> Draft>,
    parse: crate::DraftParser<Draft, Value>,
    is_empty: Rc<dyn Fn(&Draft) -> bool>,
    value_signal: Signal<Value>,
    initial_value_signal: Signal<Value>,
    draft_signal: Signal<Draft>,
    initial_draft_signal: Signal<Draft>,
    meta_signal: Signal<FieldMeta>,
}

impl<Value, Root, Draft> Clone for FieldHandle<Value, Root, Draft>
where
    Root: Clone + PartialEq + 'static,
    Value: Clone + PartialEq + 'static,
    Draft: Clone + PartialEq + 'static,
{
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            path: self.path.clone(),
            get: self.get.clone(),
            set: self.set.clone(),
            pure_set: self.pure_set.clone(),
            format: self.format.clone(),
            parse: self.parse.clone(),
            is_empty: self.is_empty.clone(),
            value_signal: self.value_signal,
            initial_value_signal: self.initial_value_signal,
            draft_signal: self.draft_signal,
            initial_draft_signal: self.initial_draft_signal,
            meta_signal: self.meta_signal,
        }
    }
}

impl<Value, Root, Draft> PartialEq for FieldHandle<Value, Root, Draft>
where
    Root: Clone + PartialEq + 'static,
    Value: Clone + PartialEq + 'static,
    Draft: Clone + PartialEq + 'static,
{
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root && self.path == other.path
    }
}

impl<Value, Root, Draft> FieldHandle<Value, Root, Draft>
where
    Root: Clone + PartialEq + 'static,
    Value: Clone + PartialEq + 'static,
    Draft: Clone + PartialEq + 'static,
{
    pub(crate) fn new<Parent>(
        scope: &FormScope<Parent, Root>,
        spec: FieldSpec<Parent, Value, Draft>,
    ) -> Self
    where
        Parent: Clone + PartialEq + 'static,
    {
        let path = scope.path.child(&spec.name);
        let parent_value_signal_for_child = scope.value_signal;
        let parent_initial_value_signal_for_child = scope.initial_value_signal;
        let parent_value_signal_for_set = scope.value_signal;
        let parent_set = scope.set.clone();
        let parent_pure_get_for_set = scope.pure_get.clone();
        let parent_pure_set = scope.pure_set.clone();
        let field_get = spec.get.clone();
        let field_initial_get = spec.get.clone();
        let field_set = spec.set.clone();
        let field_pure_set = spec.set.clone();
        let format_current = spec.format.clone();
        let format_initial = spec.format.clone();
        let get: crate::ValueGetter<Root, Value> = Rc::new(move |_root| {
            let parent = parent_value_signal_for_child.peek().clone();
            field_get(&parent)
        });
        let initial_get: crate::ValueGetter<Root, Value> = Rc::new(move |_root| {
            let parent = parent_initial_value_signal_for_child.peek().clone();
            field_initial_get(&parent)
        });
        let set: crate::ValueSetter<Root, Value> = Rc::new(move |root, value| {
            let mut parent = parent_value_signal_for_set.peek().clone();
            field_set(&mut parent, value);
            set_signal_if_changed(parent_value_signal_for_set, parent.clone());
            parent_set(root, parent);
        });
        let pure_set: crate::ValueSetter<Root, Value> = Rc::new(move |root, value| {
            let mut parent = parent_pure_get_for_set(root);
            field_pure_set(&mut parent, value);
            parent_pure_set(root, parent);
        });
        let validators = spec
            .validators
            .into_iter()
            .map(|validator| {
                let get = get.clone();
                let parent_value_signal = scope.value_signal;
                let path = path.clone();
                Rc::new(move |phase, root: Root| {
                    let validator = validator.clone();
                    let path = path.clone();
                    let parent = parent_value_signal.peek().clone();
                    let value = get(&root);
                    async move {
                        validator
                            .validate(FieldValidationContext {
                                phase,
                                path,
                                value,
                                parent,
                            })
                            .await
                    }
                    .boxed_local()
                }) as DynValidator<Root>
            })
            .collect::<Vec<_>>();
        let current = get(&scope.root.state.values.peek());
        let initial = initial_get(&scope.root.state.initial_values.peek());
        let current_draft = format_current(&current);
        let initial_draft = format_initial(&initial);
        let owner = scope.root.state.owner;
        let (value_signal, initial_value_signal, draft_signal, initial_draft_signal, meta_signal) =
            scope.root.with_registry(|registry| {
                let value_signal = registry.ensure_value_signal(&path, current.clone(), owner);
                let initial_value_signal =
                    registry.ensure_initial_value_signal(&path, initial.clone(), owner);
                let draft_signal =
                    registry.ensure_draft_signal(&path, current_draft.clone(), owner);
                let initial_draft_signal =
                    registry.ensure_initial_draft_signal(&path, initial_draft.clone(), owner);
                let node = registry.nodes.entry(path.clone()).or_insert_with(|| {
                    FormNodeState::new(FormNodeKind::Field, path.clone(), owner)
                });
                node.validators = validators;
                node.current_value_applier = Some({
                    let pure_set = pure_set.clone();
                    Rc::new(move |root: &mut Root| {
                        pure_set(root, value_signal.peek().clone());
                    })
                });
                node.value_refresher = Some({
                    let get = get.clone();
                    let format = spec.format.clone();
                    Rc::new(move |root: &Root| {
                        let value = get(root);
                        set_signal_if_changed(value_signal, value.clone());
                        set_signal_if_changed(draft_signal, format(&value));
                    })
                });
                node.initial_value_refresher = Some({
                    let initial_get = initial_get.clone();
                    let format = spec.format.clone();
                    Rc::new(move |root: &Root| {
                        let value = initial_get(root);
                        set_signal_if_changed(initial_value_signal, value.clone());
                        set_signal_if_changed(initial_draft_signal, format(&value));
                    })
                });
                (
                    value_signal,
                    initial_value_signal,
                    draft_signal,
                    initial_draft_signal,
                    node.field_meta_signal,
                )
            });
        let handle = Self {
            root: scope.root.clone(),
            path,
            get,
            set,
            pure_set,
            format: spec.format,
            parse: spec.parse,
            is_empty: spec.is_empty,
            value_signal,
            initial_value_signal,
            draft_signal,
            initial_draft_signal,
            meta_signal,
        };
        handle.refresh_node_state_local();
        handle
    }

    pub fn path(&self) -> FieldPath {
        self.path.clone()
    }

    pub fn value(&self) -> Value {
        self.value_signal.read().clone()
    }

    pub fn value_signal(&self) -> ReadSignal<Value> {
        self.value_signal.into()
    }

    pub fn draft(&self) -> Draft {
        self.draft_signal.read().clone()
    }

    pub fn draft_signal(&self) -> ReadSignal<Draft> {
        self.draft_signal.into()
    }

    pub fn scope(&self) -> FormScope<Value, Root> {
        let value_signal_for_get = self.value_signal;
        let value_signal_for_set = self.value_signal;
        let set = self.set.clone();
        let pure_set = self.pure_set.clone();
        let format = self.format.clone();
        let is_empty = self.is_empty.clone();
        let path = self.path.clone();
        let owner = self.root.state.owner;
        let meta_signal = self.root.with_registry(|registry| {
            registry
                .nodes
                .entry(path.clone())
                .or_insert_with(|| FormNodeState::new(FormNodeKind::Field, path.clone(), owner))
                .scope_meta_signal
        });
        FormScope {
            root: self.root.clone(),
            path: self.path.clone(),
            get: Rc::new(move |_root: &Root| value_signal_for_get.peek().clone()),
            initial_value_signal: self.initial_value_signal,
            set: Rc::new(move |root: &mut Root, value: Value| {
                set_signal_if_changed(value_signal_for_set, value.clone());
                set(root, value);
            }),
            pure_get: Rc::new(move |_root: &Root| value_signal_for_get.peek().clone()),
            pure_set,
            is_empty: Rc::new(move |value: &Value| is_empty(&format(value))),
            value_signal: self.value_signal,
            meta_signal,
        }
    }

    pub fn meta(&self) -> FieldMeta {
        self.meta_signal.read().clone()
    }

    pub fn meta_signal(&self) -> ReadSignal<FieldMeta> {
        self.meta_signal.into()
    }

    pub fn set_value(&self, value: Value) {
        let set = self.set.clone();
        let root_value = value.clone();
        let draft = (self.format)(&value);
        self.root
            .mutate_values_at(self.path.clone(), move |root| set(root, root_value));
        set_signal_if_changed(self.value_signal, value);
        set_signal_if_changed(self.draft_signal, draft);
        self.root.with_registry(|registry| {
            if let Some(node) = registry.nodes.get_mut(&self.path) {
                node.touched = true;
                node.parse_errors.clear();
            }
        });
        self.refresh_node_state();
    }

    pub fn set_draft(&self, draft: Draft) {
        set_signal_if_changed(self.draft_signal, draft.clone());
        match (self.parse)(&draft) {
            Ok(value) => {
                let set = self.set.clone();
                let root_value = value.clone();
                self.root
                    .mutate_values_at(self.path.clone(), move |root| set(root, root_value));
                set_signal_if_changed(self.value_signal, value);
                self.root.with_registry(|registry| {
                    if let Some(node) = registry.nodes.get_mut(&self.path) {
                        node.touched = true;
                        node.parse_errors.clear();
                    }
                });
            }
            Err(error) => {
                self.root.with_registry(|registry| {
                    if let Some(node) = registry.nodes.get_mut(&self.path) {
                        node.touched = true;
                        node.parse_errors = vec![
                            error
                                .with_source(FormErrorSource::Parse)
                                .at_path(&self.path),
                        ];
                    }
                });
            }
        }
        self.refresh_node_state();
    }

    pub fn update_value(&self, f: impl FnOnce(&mut Value) + 'static) {
        let mut value = self.value();
        f(&mut value);
        self.set_value(value);
    }

    pub fn mark_touched(&self) {
        self.root.with_registry(|registry| {
            if let Some(node) = registry.nodes.get_mut(&self.path) {
                node.touched = true;
            }
        });
        self.root.recompute_all_meta();
    }

    pub fn set_focused(&self, focused: bool) {
        self.root.with_registry(|registry| {
            if let Some(node) = registry.nodes.get_mut(&self.path) {
                node.focused = focused;
                if !focused {
                    node.touched = true;
                }
            }
        });
        self.root.recompute_all_meta();
    }

    pub fn reset(&self) {
        let initial = self.initial_value_signal.peek().clone();
        let initial_draft = self.initial_draft_signal.peek().clone();
        let set = self.set.clone();
        let root_value = initial.clone();
        self.root
            .mutate_values_at(self.path.clone(), move |root| set(root, root_value));
        set_signal_if_changed(self.value_signal, initial);
        set_signal_if_changed(self.draft_signal, initial_draft);
        self.root.with_registry(|registry| {
            if let Some(node) = registry.nodes.get_mut(&self.path) {
                node.touched = false;
                node.focused = false;
                node.parse_errors.clear();
                node.errors.clear();
                node.submit_errors.clear();
            }
        });
        self.refresh_node_state();
    }

    pub async fn validate(&self) -> std::result::Result<(), Vec<FormError>> {
        self.root
            .validate_path(self.path.clone(), ValidationPhase::Manual)
            .await
    }

    pub fn clear_errors(&self) {
        self.root
            .with_registry(|registry| registry.clear_errors_under(&self.path, false));
        self.root.recompute_all_meta();
    }

    pub fn clear_submit_errors(&self) {
        self.root
            .with_registry(|registry| registry.clear_errors_under(&self.path, true));
        self.root.recompute_all_meta();
    }

    pub(crate) fn refresh_node_state(&self) {
        self.refresh_node_state_local();
        self.root.recompute_all_meta();
    }

    fn refresh_node_state_local(&self) {
        let current = self.draft_signal.peek().clone();
        let initial = self.initial_draft_signal.peek().clone();
        let empty = (self.is_empty)(&current);
        self.root.with_registry(|registry| {
            let node = registry.nodes.entry(self.path.clone()).or_insert_with(|| {
                FormNodeState::new(
                    FormNodeKind::Field,
                    self.path.clone(),
                    self.root.state.owner,
                )
            });
            node.dirty = current != initial;
            node.empty = empty;
            if !node.parse_errors.is_empty() {
                node.validity = crate::Validity::Invalid;
            }
            node.sync_meta_signals();
        });
    }
}

pub fn use_field<Parent, Value, Root, Draft>(
    scope: FormScope<Parent, Root>,
    spec: impl FnOnce() -> FieldSpec<Parent, Value, Draft>,
) -> FieldHandle<Value, Root, Draft>
where
    Root: Clone + PartialEq + 'static,
    Parent: Clone + PartialEq + 'static,
    Value: Clone + PartialEq + 'static,
    Draft: Clone + PartialEq + 'static,
{
    scope.field(spec())
}
