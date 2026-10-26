use std::{any::Any, collections::BTreeMap, rc::Rc};

use dioxus::prelude::*;
use futures::future::LocalBoxFuture;

use crate::{FieldMeta, FieldPath, FormError, ListMeta, ScopeMeta, ValidationPhase, Validity};

pub type ListItemKey = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormNodeKind {
    Field,
    Scope,
    List,
    ListItem,
}

pub(crate) type DynValidator<Root> =
    Rc<dyn Fn(ValidationPhase, Root) -> LocalBoxFuture<'static, Vec<FormError>>>;
pub(crate) type ValueApplier<Root> = Rc<dyn Fn(&mut Root)>;
pub(crate) type ValueRefresher<Root> = Rc<dyn Fn(&Root)>;

pub(crate) struct FormNodeState<Root> {
    pub kind: FormNodeKind,
    pub path: FieldPath,
    pub touched: bool,
    pub focused: bool,
    pub dirty: bool,
    pub empty: bool,
    pub validating: bool,
    pub validity: Validity,
    pub parse_errors: Vec<FormError>,
    pub errors: Vec<FormError>,
    pub submit_errors: Vec<FormError>,
    pub validation_epoch: u64,
    pub validators: Vec<DynValidator<Root>>,
    pub current_value_applier: Option<ValueApplier<Root>>,
    pub field_meta_signal: Signal<FieldMeta>,
    pub scope_meta_signal: Signal<ScopeMeta>,
    pub list_meta_signal: Signal<ListMeta>,
    pub value_refresher: Option<ValueRefresher<Root>>,
    pub initial_value_refresher: Option<ValueRefresher<Root>>,
}

impl<Root> Clone for FormNodeState<Root> {
    fn clone(&self) -> Self {
        Self {
            kind: self.kind,
            path: self.path.clone(),
            touched: self.touched,
            focused: self.focused,
            dirty: self.dirty,
            empty: self.empty,
            validating: self.validating,
            validity: self.validity,
            parse_errors: self.parse_errors.clone(),
            errors: self.errors.clone(),
            submit_errors: self.submit_errors.clone(),
            validation_epoch: self.validation_epoch,
            validators: self.validators.clone(),
            current_value_applier: self.current_value_applier.clone(),
            field_meta_signal: self.field_meta_signal,
            scope_meta_signal: self.scope_meta_signal,
            list_meta_signal: self.list_meta_signal,
            value_refresher: self.value_refresher.clone(),
            initial_value_refresher: self.initial_value_refresher.clone(),
        }
    }
}

impl<Root> FormNodeState<Root> {
    pub fn new(kind: FormNodeKind, path: FieldPath, owner: ScopeId) -> Self {
        let state = Self {
            kind,
            path: path.clone(),
            touched: false,
            focused: false,
            dirty: false,
            empty: true,
            validating: false,
            validity: Validity::NotValidated,
            parse_errors: Vec::new(),
            errors: Vec::new(),
            submit_errors: Vec::new(),
            validation_epoch: 0,
            validators: Vec::new(),
            current_value_applier: None,
            field_meta_signal: Signal::new_in_scope(FieldMeta::new(path.clone()), owner),
            scope_meta_signal: Signal::new_in_scope(ScopeMeta::new(path.clone()), owner),
            list_meta_signal: Signal::new_in_scope(ListMeta::new(path), owner),
            value_refresher: None,
            initial_value_refresher: None,
        };
        state.sync_meta_signals();
        state
    }

    pub fn field_meta(&self) -> FieldMeta {
        FieldMeta {
            path: self.path.clone(),
            touched: self.touched,
            focused: self.focused,
            dirty: self.dirty,
            empty: self.empty,
            validating: self.validating,
            validity: self.validity,
            errors: combined_errors(&self.parse_errors, &self.errors),
            submit_errors: self.submit_errors.clone(),
        }
    }

    pub fn scope_meta(&self) -> ScopeMeta {
        ScopeMeta {
            path: self.path.clone(),
            touched: self.touched,
            dirty: self.dirty,
            empty: self.empty,
            validating: self.validating,
            validity: self.validity,
            errors: combined_errors(&self.parse_errors, &self.errors),
            submit_errors: self.submit_errors.clone(),
        }
    }

    pub fn list_meta(&self) -> ListMeta {
        ListMeta {
            path: self.path.clone(),
            touched: self.touched,
            dirty: self.dirty,
            empty: self.empty,
            validating: self.validating,
            validity: self.validity,
            errors: combined_errors(&self.parse_errors, &self.errors),
            submit_errors: self.submit_errors.clone(),
        }
    }

    pub fn sync_meta_signals(&self) {
        set_signal_if_changed(self.field_meta_signal, self.field_meta());
        set_signal_if_changed(self.scope_meta_signal, self.scope_meta());
        set_signal_if_changed(self.list_meta_signal, self.list_meta());
    }
}

fn combined_errors(parse_errors: &[FormError], validation_errors: &[FormError]) -> Vec<FormError> {
    parse_errors
        .iter()
        .chain(validation_errors.iter())
        .cloned()
        .collect()
}

pub(crate) struct FormRegistry<Root> {
    pub nodes: BTreeMap<FieldPath, FormNodeState<Root>>,
    pub list_keys: BTreeMap<FieldPath, Vec<ListItemKey>>,
    value_signals: BTreeMap<FieldPath, Rc<dyn Any>>,
    initial_value_signals: BTreeMap<FieldPath, Rc<dyn Any>>,
    draft_signals: BTreeMap<FieldPath, Rc<dyn Any>>,
    initial_draft_signals: BTreeMap<FieldPath, Rc<dyn Any>>,
    list_key_signals: BTreeMap<FieldPath, Signal<Vec<ListItemKey>>>,
}

impl<Root> Default for FormRegistry<Root> {
    fn default() -> Self {
        Self {
            nodes: BTreeMap::new(),
            list_keys: BTreeMap::new(),
            value_signals: BTreeMap::new(),
            initial_value_signals: BTreeMap::new(),
            draft_signals: BTreeMap::new(),
            initial_draft_signals: BTreeMap::new(),
            list_key_signals: BTreeMap::new(),
        }
    }
}

impl<Root> Clone for FormRegistry<Root> {
    fn clone(&self) -> Self {
        Self {
            nodes: self.nodes.clone(),
            list_keys: self.list_keys.clone(),
            value_signals: self.value_signals.clone(),
            initial_value_signals: self.initial_value_signals.clone(),
            draft_signals: self.draft_signals.clone(),
            initial_draft_signals: self.initial_draft_signals.clone(),
            list_key_signals: self.list_key_signals.clone(),
        }
    }
}

impl<Root> FormRegistry<Root> {
    pub fn ensure_node(&mut self, kind: FormNodeKind, path: FieldPath, owner: ScopeId) {
        self.nodes
            .entry(path.clone())
            .or_insert_with(|| FormNodeState::new(kind, path, owner));
    }

    pub fn ensure_value_signal<Value>(
        &mut self,
        path: &FieldPath,
        value: Value,
        owner: ScopeId,
    ) -> Signal<Value>
    where
        Value: Clone + PartialEq + 'static,
    {
        ensure_typed_signal(&mut self.value_signals, path, value, owner)
    }

    pub fn ensure_initial_value_signal<Value>(
        &mut self,
        path: &FieldPath,
        value: Value,
        owner: ScopeId,
    ) -> Signal<Value>
    where
        Value: Clone + PartialEq + 'static,
    {
        ensure_typed_signal(&mut self.initial_value_signals, path, value, owner)
    }

    pub fn ensure_draft_signal<Draft>(
        &mut self,
        path: &FieldPath,
        value: Draft,
        owner: ScopeId,
    ) -> Signal<Draft>
    where
        Draft: Clone + PartialEq + 'static,
    {
        ensure_typed_signal(&mut self.draft_signals, path, value, owner)
    }

    pub fn ensure_initial_draft_signal<Draft>(
        &mut self,
        path: &FieldPath,
        value: Draft,
        owner: ScopeId,
    ) -> Signal<Draft>
    where
        Draft: Clone + PartialEq + 'static,
    {
        ensure_typed_signal(&mut self.initial_draft_signals, path, value, owner)
    }

    pub fn ensure_list_key_signal(
        &mut self,
        path: &FieldPath,
        keys: Vec<ListItemKey>,
        owner: ScopeId,
    ) -> Signal<Vec<ListItemKey>> {
        *self
            .list_key_signals
            .entry(path.clone())
            .or_insert_with(|| Signal::new_in_scope(keys, owner))
    }

    pub fn remove_descendants(&mut self, path: &FieldPath) {
        self.nodes
            .retain(|node_path, _| !node_path.starts_with(path));
        self.list_keys
            .retain(|list_path, _| !list_path.starts_with(path));
        self.value_signals
            .retain(|node_path, _| !node_path.starts_with(path));
        self.initial_value_signals
            .retain(|node_path, _| !node_path.starts_with(path));
        self.draft_signals
            .retain(|node_path, _| !node_path.starts_with(path));
        self.initial_draft_signals
            .retain(|node_path, _| !node_path.starts_with(path));
        self.list_key_signals
            .retain(|list_path, _| !list_path.starts_with(path));
    }

    pub fn clear_errors_under(&mut self, path: &FieldPath, submit: bool) {
        for node in self
            .nodes
            .values_mut()
            .filter(|node| node.path.starts_with(path))
        {
            if submit {
                node.submit_errors.clear();
            } else {
                node.parse_errors.clear();
                node.errors.clear();
            }
            if node.parse_errors.is_empty() && node.errors.is_empty() {
                node.validity = Validity::NotValidated;
            }
            node.sync_meta_signals();
        }
    }

    pub fn touch_under(&mut self, path: &FieldPath) {
        for node in self
            .nodes
            .values_mut()
            .filter(|node| node.path.starts_with(path))
        {
            node.touched = true;
            node.sync_meta_signals();
        }
    }
}

pub(crate) fn set_signal_if_changed<T>(signal: Signal<T>, value: T)
where
    T: Clone + PartialEq + 'static,
{
    if *signal.peek() != value {
        let mut signal = signal;
        signal.set(value);
    }
}

fn ensure_typed_signal<Value>(
    signals: &mut BTreeMap<FieldPath, Rc<dyn Any>>,
    path: &FieldPath,
    value: Value,
    owner: ScopeId,
) -> Signal<Value>
where
    Value: Clone + PartialEq + 'static,
{
    if let Some(existing) = signals.get(path)
        && let Ok(signal) = existing.clone().downcast::<Signal<Value>>()
    {
        return *signal;
    }

    let signal = Signal::new_in_scope(value, owner);
    signals.insert(path.clone(), Rc::new(signal));
    signal
}
