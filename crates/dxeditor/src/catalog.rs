use crate::{
    component_spec::{ComponentCatalog, ComponentCatalogError, register_standard_component_specs},
    format::{DocumentFormatRegistry, register_standard_document_formats},
    suggestion::SuggestionRegistry,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorCatalogConfig {
    pub register_defaults: bool,
}

impl Default for EditorCatalogConfig {
    fn default() -> Self {
        Self {
            register_defaults: true,
        }
    }
}

#[derive(Clone)]
pub struct EditorCatalog {
    document_formats: DocumentFormatRegistry,
    component_specs: ComponentCatalog,
    suggestions: SuggestionRegistry,
}

impl EditorCatalog {
    pub fn block_document() -> Self {
        let mut catalog = Self {
            component_specs: ComponentCatalog::default(),
            ..Self::default()
        };
        crate::profiles::block_document::register_block_document_specs(
            &mut catalog.component_specs,
        )
        .expect("block document specifications must be valid");
        catalog
    }

    /// Rich node content without nested graphs or host-owned insert flows.
    pub fn graph_node() -> Self {
        let mut catalog = Self::block_document();
        let specs = catalog.component_specs.specs().cloned().collect::<Vec<_>>();
        catalog.component_specs = ComponentCatalog::default();
        for mut spec in specs {
            // A graph node may retain opaque host-owned content, but a graph
            // itself is never valid content within another graph node.
            if spec.id.0 == "graph" {
                continue;
            }
            if ["file", "bookmark", "entity_embed", "unsupported"].contains(&spec.id.0.as_str()) {
                spec.insert = None;
            }
            catalog
                .component_specs
                .register(spec)
                .expect("graph node specifications are valid");
        }
        catalog
    }

    /// The block editing schema restricted to constructs that Markdown can persist.
    pub fn markdown_notes() -> Self {
        let mut catalog = Self {
            component_specs: ComponentCatalog::default(),
            ..Self::default()
        };
        crate::profiles::block_document::register_markdown_notes_specs(
            &mut catalog.component_specs,
        )
        .expect("Markdown editing specifications must be valid");
        catalog
    }

    /// The shared block engine with paragraph, text, hard-break and chip content only.
    pub fn composer() -> Self {
        let mut catalog = Self {
            component_specs: ComponentCatalog::default(),
            ..Self::default()
        };
        crate::profiles::block_document::register_composer_specs(&mut catalog.component_specs)
            .expect("composer specifications must be valid");
        catalog
    }

    pub fn builder() -> EditorCatalogBuilder {
        EditorCatalogBuilder::new()
    }

    pub fn document_formats(&self) -> &DocumentFormatRegistry {
        &self.document_formats
    }

    pub fn document_formats_mut(&mut self) -> &mut DocumentFormatRegistry {
        &mut self.document_formats
    }

    pub fn component_specs(&self) -> &ComponentCatalog {
        &self.component_specs
    }

    pub fn component_specs_mut(&mut self) -> &mut ComponentCatalog {
        &mut self.component_specs
    }

    pub fn schema_fingerprint(&self) -> String {
        self.component_specs.schema_fingerprint()
    }

    pub fn suggestions(&self) -> &SuggestionRegistry {
        &self.suggestions
    }

    pub fn suggestions_mut(&mut self) -> &mut SuggestionRegistry {
        &mut self.suggestions
    }
}

impl Default for EditorCatalog {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl PartialEq for EditorCatalog {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

pub struct EditorCatalogBuilder {
    catalog: EditorCatalog,
    config: EditorCatalogConfig,
}

impl EditorCatalogBuilder {
    pub fn new() -> Self {
        Self {
            catalog: EditorCatalog {
                document_formats: DocumentFormatRegistry::default(),
                component_specs: ComponentCatalog::default(),
                suggestions: SuggestionRegistry::default(),
            },
            config: EditorCatalogConfig::default(),
        }
    }

    pub fn with_config(mut self, config: EditorCatalogConfig) -> Self {
        self.config = config;
        self
    }

    pub fn build(self) -> EditorCatalog {
        self.try_build()
            .expect("standard editor catalog specifications must be valid")
    }

    pub fn try_build(mut self) -> Result<EditorCatalog, ComponentCatalogError> {
        if self.config.register_defaults {
            register_standard_document_formats(&mut self.catalog.document_formats);
            register_standard_component_specs(&mut self.catalog.component_specs)?;
        }
        self.catalog.component_specs.validate()?;
        Ok(self.catalog)
    }
}

impl Default for EditorCatalogBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod graph_catalog_tests {
    use super::EditorCatalog;
    use crate::{
        ComponentDocumentV2, UnknownComponentPolicy, ValidationLimits, validate_component_document,
    };
    use serde_json::json;

    fn document(kind: &str) -> ComponentDocumentV2 {
        serde_json::from_value(json!({
            "schema": "semantic.block-document",
            "version": 2,
            "root": { "kind": "document", "content": [{
                "kind": "block_children", "content": [{
                    "kind": "block", "id": "019a5555-5555-7555-8555-555555555555",
                    "content": [{ "kind": kind }]
                }]
            }] }
        }))
        .expect("valid component fixture")
    }

    #[test]
    fn graph_profile_uses_generic_island_and_exposes_mind_map_insert() {
        let catalog = EditorCatalog::block_document();
        let graph = catalog.component_specs().spec_by_id("graph").unwrap();
        assert!(
            graph
                .insert
                .as_ref()
                .unwrap()
                .keywords
                .iter()
                .any(|word| word == "mind map")
        );
        assert!(
            validate_component_document(
                &document("graph"),
                catalog.component_specs(),
                &ValidationLimits::default(),
                UnknownComponentPolicy::Reject
            )
            .is_ok()
        );
    }

    #[test]
    fn node_catalog_rejects_nested_graphs_and_preserves_opaque_placeholders() {
        let catalog = EditorCatalog::graph_node();
        assert!(
            validate_component_document(
                &document("graph"),
                catalog.component_specs(),
                &ValidationLimits::default(),
                UnknownComponentPolicy::Reject
            )
            .is_err()
        );
        for kind in ["file", "bookmark", "entity_embed", "unsupported"] {
            let spec = catalog.component_specs().spec_by_id(kind).unwrap();
            assert!(spec.insert.is_none(), "host-owned insertion: {kind}");
        }
        assert!(
            validate_component_document(
                &document("details_break"),
                catalog.component_specs(),
                &ValidationLimits::default(),
                UnknownComponentPolicy::Reject
            )
            .is_ok()
        );
    }
}
