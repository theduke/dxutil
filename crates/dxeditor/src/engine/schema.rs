use crate::{ComponentCatalog, ComponentKind, ComponentSpec, EditorCatalog};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct EditingSchema {
    pub catalog: ComponentCatalog,
    pub kinds: BTreeMap<String, ComponentSpec>,
    pub marks: BTreeSet<String>,
    pub fingerprint: String,
}
impl EditingSchema {
    pub fn new(catalog: &EditorCatalog) -> Self {
        let specs = catalog.component_specs();
        Self {
            catalog: specs.clone(),
            kinds: specs.specs().map(|s| (s.id.0.clone(), s.clone())).collect(),
            marks: specs
                .specs()
                .filter(|s| s.kind == ComponentKind::Mark)
                .map(|s| s.id.0.clone())
                .collect(),
            fingerprint: catalog.schema_fingerprint(),
        }
    }
    pub fn allows_children(&self, kind: &str) -> bool {
        self.kinds
            .get(kind)
            .is_some_and(|s| s.behavior.allows_children)
    }
    pub fn allows_kind(&self, kind: &str) -> bool {
        self.has_kind(kind)
    }
    pub fn allows_mark(&self, kind: &str) -> bool {
        self.marks.contains(kind)
    }
    pub fn has_kind(&self, kind: &str) -> bool {
        self.kinds.contains_key(kind)
    }
    pub fn is_atom(&self, kind: &str) -> bool {
        self.kinds
            .get(kind)
            .is_some_and(|s| matches!(s.kind, ComponentKind::Atom))
    }
}
