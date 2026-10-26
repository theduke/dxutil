//! Reuse a successful document validation only for a precisely checked text edit.
//!
//! The engine's render plan is authoritative about which blocks changed. If a
//! single block retains every kind, identity, attribute, mark and child boundary,
//! all schema, identity, depth, table and attribute-budget checks remain valid.
//! Only the global text-byte total can change, so update that budget before
//! publishing. Every other change uses the ordinary whole-document validator.
use crate::{
    ComponentCatalog, ComponentDocumentV2, ComponentNode, DocumentValidationError,
    UnknownComponentPolicy, ValidationLimits, validate_component_document,
};

#[derive(Default)]
pub(super) struct ChangeValidator {
    last_block: Option<(String, ComponentNode)>,
    text_bytes: Option<usize>,
    schema: String,
    limits: ValidationLimits,
}
impl ChangeValidator {
    /// Only call for a document already accepted by the engine's full validator.
    pub fn seed_validated(&mut self, document: &ComponentDocumentV2, schema: &str) {
        self.schema = schema.into();
        self.text_bytes = Some(text_bytes(&document.root));
        self.last_block = None;
    }
    pub fn track_selection(&mut self, candidate: Option<(&str, &ComponentNode)>) {
        if self.text_bytes.is_some()
            && let Some((uid, block)) = candidate
            && self.last_block.as_ref().is_none_or(|(last, _)| last != uid)
        {
            self.last_block = Some((uid.into(), block.clone()));
        }
    }
    pub fn invalidate(&mut self) {
        self.last_block = None;
        self.text_bytes = None;
    }
    pub fn validate(
        &mut self,
        document: &ComponentDocumentV2,
        catalog: &ComponentCatalog,
        schema: &str,
        candidate: Option<(&str, &ComponentNode)>,
        text_edit: bool,
    ) -> Result<(), DocumentValidationError> {
        if self.schema != schema {
            self.invalidate();
            self.schema = schema.into();
        }
        if text_edit
            && let (Some((uid, after)), Some((before_uid, before)), Some(total)) =
                (candidate, self.last_block.as_ref(), self.text_bytes)
        {
            if uid == before_uid && same_structure(before, after) {
                let next_total = total
                    .saturating_sub(text_bytes(before))
                    .checked_add(text_bytes(after));
                if let Some(next_total) =
                    next_total.filter(|total| *total <= self.limits.max_text_bytes)
                {
                    self.text_bytes = Some(next_total);
                    self.last_block = Some((uid.into(), after.clone()));
                    return Ok(());
                }
            }
        }
        let result = validate_component_document(
            document,
            catalog,
            &self.limits,
            UnknownComponentPolicy::Reject,
        );
        if result.is_ok() {
            self.text_bytes = Some(text_bytes(&document.root));
            self.last_block = candidate.map(|(uid, block)| (uid.into(), block.clone()));
        } else {
            self.invalidate();
        }
        result
    }
}
fn same_structure(before: &ComponentNode, after: &ComponentNode) -> bool {
    before.kind == after.kind
        && before.id == after.id
        && before.attrs == after.attrs
        && before.marks == after.marks
        && before.content.len() == after.content.len()
        && (if before.kind.0 == "text" {
            before.text.is_some() && after.text.is_some()
        } else {
            before.text == after.text
        })
        && before
            .content
            .iter()
            .zip(&after.content)
            .all(|(before, after)| same_structure(before, after))
}
fn text_bytes(node: &ComponentNode) -> usize {
    node.text.as_ref().map_or(0, String::len) + node.content.iter().map(text_bytes).sum::<usize>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ComponentMark, EditorCatalog, engine::model};
    use serde_json::json;
    fn document() -> ComponentDocumentV2 {
        model::document(vec![
            model::block_node(ComponentNode::container(
                "paragraph",
                None,
                vec![ComponentNode::text("ab")],
            )),
            model::block_node(ComponentNode::container(
                "paragraph",
                None,
                vec![ComponentNode::text("other")],
            )),
        ])
    }
    fn validate(
        cache: &mut ChangeValidator,
        document: &ComponentDocumentV2,
        eligible: bool,
    ) -> Result<(), DocumentValidationError> {
        let catalog = EditorCatalog::block_document();
        let block = &document.root.content[0].content[0];
        cache.validate(
            document,
            catalog.component_specs(),
            "test-schema",
            Some((&block.id.as_ref().unwrap().0, block)),
            eligible,
        )
    }
    #[test]
    fn initially_validated_document_primes_first_edit_budgets() {
        let mut document = document();
        let mut cache = ChangeValidator::default();
        cache.seed_validated(&document, "test-schema");
        let block = &document.root.content[0].content[0];
        cache.track_selection(Some((&block.id.as_ref().unwrap().0, block)));
        document.root.content[0].content[0].content[0].content[0].text =
            Some("first typed text".into());
        validate(&mut cache, &document, true).unwrap();
        assert_eq!(cache.text_bytes, Some("first typed textother".len()));
    }
    #[test]
    fn repeated_unicode_text_edits_match_full_validation() {
        let mut doc = document();
        let mut cache = ChangeValidator::default();
        validate(&mut cache, &doc, false).unwrap();
        for text in ["abc", "日本語", "👨‍👩‍👧", "x"] {
            doc.root.content[0].content[0].content[0].content[0].text = Some(text.into());
            validate(&mut cache, &doc, true).unwrap();
            assert_eq!(cache.text_bytes, Some(text.len() + "other".len()));
        }
    }
    #[test]
    fn global_text_budget_includes_unchanged_blocks() {
        let mut cache = ChangeValidator {
            limits: ValidationLimits {
                max_text_bytes: 8,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut doc = document();
        validate(&mut cache, &doc, false).unwrap();
        doc.root.content[0].content[0].content[0].content[0].text = Some("four".into());
        assert!(validate(&mut cache, &doc, true).is_err());
        assert!(cache.text_bytes.is_none());
        doc.root.content[0].content[0].content[0].content[0].text = Some("a".into());
        validate(&mut cache, &doc, true).unwrap();
    }
    #[test]
    fn marks_attributes_and_identity_changes_get_full_validation() {
        let mut doc = document();
        let mut cache = ChangeValidator::default();
        validate(&mut cache, &doc, false).unwrap();
        doc.root.content[0].content[0].content[0].content[0]
            .marks
            .push(ComponentMark::new("unknown_mark"));
        assert!(validate(&mut cache, &doc, true).is_err());
        doc = document();
        validate(&mut cache, &doc, false).unwrap();
        doc.root.content[0].content[0].content[0]
            .attrs
            .insert("unknown".into(), json!(true));
        assert!(validate(&mut cache, &doc, true).is_err());
        doc = document();
        validate(&mut cache, &doc, false).unwrap();
        doc.root.content[0].content[0].id = doc.root.content[0].content[1].id.clone();
        assert!(validate(&mut cache, &doc, true).is_err());
    }
    #[test]
    fn structural_changes_and_replacement_invalidation_validate_whole_document() {
        let mut doc = document();
        let mut cache = ChangeValidator::default();
        validate(&mut cache, &doc, false).unwrap();
        doc.root.content[0].content[1].content[0].kind = "unknown".into();
        assert!(validate(&mut cache, &doc, false).is_err());
        doc = document();
        validate(&mut cache, &doc, false).unwrap();
        cache.invalidate();
        doc.root.content[0].content[1].content[0].kind = "unknown".into();
        assert!(validate(&mut cache, &doc, true).is_err());
    }
    #[test]
    fn changed_schema_invalidates_text_cache() {
        let mut doc = document();
        let mut cache = ChangeValidator::default();
        validate(&mut cache, &doc, false).unwrap();
        doc.root.content[0].content[1].id = doc.root.content[0].content[0].id.clone();
        let catalog = EditorCatalog::block_document();
        let block = &doc.root.content[0].content[0];
        assert!(
            cache
                .validate(
                    &doc,
                    catalog.component_specs(),
                    "different",
                    Some((&block.id.as_ref().unwrap().0, block)),
                    true
                )
                .is_err()
        );
    }
}
