use std::{collections::BTreeMap, sync::Arc};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::{
    EditorPayload,
    component_spec::{
        ComponentCatalog, FORMAT_PLAIN_TEXT, FORMAT_TYPED_DOCUMENT, FormatCapability,
        validate_component_document,
    },
    document::EditorDocument,
    document_v2::{
        COMPONENT_DOCUMENT_FORMAT, COMPONENT_DOCUMENT_VERSION, COMPONENT_UNKNOWN,
        ComponentDocumentV2, ComponentId, ComponentNode, NormalizationOptions,
        UnknownComponentPolicy, ValidationLimits, normalize_component_document,
    },
    migrate::{MigrationError, MigrationOptions, migrate_v1_to_v2_with, migrate_v2_to_v1},
};

pub const TYPED_DOCUMENT_JSON_SCHEMA: &str =
    include_str!("../assets/component_document_v2.schema.json");

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FormatId(pub String);

impl FormatId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl From<&str> for FormatId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormatDescriptor {
    pub id: FormatId,
    pub media_type: String,
    pub version: u32,
    pub capabilities: BTreeMap<ComponentId, FormatCapability>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormatDiagnostic {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_range: Option<SourceRange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fidelity {
    Lossless,
    Semantic,
    PreservedOpaque,
    ConvertedWithWarning,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FormatSourceState {
    pub format: FormatId,
    pub state: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DecodedDocument {
    pub document: ComponentDocumentV2,
    pub diagnostics: Vec<FormatDiagnostic>,
    pub fidelity: Fidelity,
    pub source_state: Option<FormatSourceState>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EncodedPayload {
    pub payload: EditorPayload,
    pub diagnostics: Vec<FormatDiagnostic>,
    pub fidelity: Fidelity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodeOptions {
    pub unknown_components: UnknownComponentPolicy,
    pub validation_limits: ValidationLimits,
    pub repair_duplicate_ids: bool,
}

impl Default for DecodeOptions {
    fn default() -> Self {
        Self {
            unknown_components: UnknownComponentPolicy::Reject,
            validation_limits: ValidationLimits::default(),
            repair_duplicate_ids: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct EncodeOptions {
    pub validation_limits: ValidationLimits,
}

#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("payload format '{actual}' does not match codec format '{expected}'")]
    FormatMismatch { expected: String, actual: String },
    #[error("invalid '{format}' payload: {message}")]
    InvalidPayload { format: String, message: String },
    #[error(transparent)]
    Validation(#[from] crate::document_v2::DocumentValidationError),
    #[error(transparent)]
    Migration(#[from] MigrationError),
}

pub trait DocumentFormat: Send + Sync {
    fn descriptor(&self) -> &FormatDescriptor;

    fn decode(
        &self,
        input: &EditorPayload,
        catalog: &ComponentCatalog,
        options: DecodeOptions,
    ) -> Result<DecodedDocument, FormatError>;

    fn encode(
        &self,
        document: &ComponentDocumentV2,
        catalog: &ComponentCatalog,
        options: EncodeOptions,
    ) -> Result<EncodedPayload, FormatError>;
}

#[derive(Clone, Default)]
pub struct DocumentFormatRegistry {
    formats: BTreeMap<FormatId, Arc<dyn DocumentFormat>>,
    media_types: BTreeMap<String, FormatId>,
}

impl DocumentFormatRegistry {
    pub fn register(&mut self, format: Arc<dyn DocumentFormat>) {
        let descriptor = format.descriptor();
        self.media_types
            .insert(descriptor.media_type.clone(), descriptor.id.clone());
        self.formats.insert(descriptor.id.clone(), format);
    }

    pub fn format(&self, id_or_media_type: &str) -> Option<Arc<dyn DocumentFormat>> {
        let id = FormatId::from(id_or_media_type);
        self.formats.get(&id).cloned().or_else(|| {
            self.media_types
                .get(id_or_media_type)
                .and_then(|id| self.formats.get(id))
                .cloned()
        })
    }

    pub fn formats(&self) -> impl Iterator<Item = &FormatDescriptor> {
        self.formats.values().map(|format| format.descriptor())
    }

    pub fn decode(
        &self,
        input: &EditorPayload,
        catalog: &ComponentCatalog,
        options: DecodeOptions,
    ) -> Result<DecodedDocument, FormatError> {
        let format = self
            .format(&input.format)
            .ok_or_else(|| FormatError::InvalidPayload {
                format: input.format.clone(),
                message: "no document format is registered".to_string(),
            })?;
        format.decode(input, catalog, options)
    }

    pub fn encode(
        &self,
        document: &ComponentDocumentV2,
        format: &str,
        catalog: &ComponentCatalog,
        options: EncodeOptions,
    ) -> Result<EncodedPayload, FormatError> {
        let format = self
            .format(format)
            .ok_or_else(|| FormatError::InvalidPayload {
                format: format.to_string(),
                message: "no document format is registered".to_string(),
            })?;
        format.encode(document, catalog, options)
    }
}

pub fn register_standard_document_formats(registry: &mut DocumentFormatRegistry) {
    registry.register(Arc::new(TypedDocumentCodec::new()));
    #[cfg(feature = "markdown")]
    registry.register(Arc::new(crate::markdown_v2::MarkdownDocumentFormat::new()));
    registry.register(Arc::new(LegacyV1DocumentFormat::new()));
    registry.register(Arc::new(PlainTextDocumentFormat::new()));
}

pub struct TypedDocumentCodec {
    descriptor: FormatDescriptor,
}

impl TypedDocumentCodec {
    pub fn new() -> Self {
        Self {
            descriptor: FormatDescriptor {
                id: FORMAT_TYPED_DOCUMENT.into(),
                media_type: COMPONENT_DOCUMENT_FORMAT.to_string(),
                version: COMPONENT_DOCUMENT_VERSION,
                capabilities: BTreeMap::new(),
            },
        }
    }
}

impl Default for TypedDocumentCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentFormat for TypedDocumentCodec {
    fn descriptor(&self) -> &FormatDescriptor {
        &self.descriptor
    }

    fn decode(
        &self,
        input: &EditorPayload,
        catalog: &ComponentCatalog,
        options: DecodeOptions,
    ) -> Result<DecodedDocument, FormatError> {
        ensure_format(input, &self.descriptor)?;
        let mut document: ComponentDocumentV2 = serde_json::from_value(input.value.clone())
            .map_err(|error| invalid_payload(input, error.to_string()))?;
        let mut diagnostics = Vec::new();
        preserve_unknown_nodes(
            &mut document.root,
            catalog,
            options.unknown_components,
            &mut diagnostics,
            "$.root",
        )?;
        normalize_component_document(
            &mut document,
            catalog,
            &NormalizationOptions {
                repair_duplicate_ids: options.repair_duplicate_ids,
            },
        )?;
        validate_component_document(
            &document,
            catalog,
            &options.validation_limits,
            UnknownComponentPolicy::Reject,
        )?;

        let fidelity = if diagnostics.is_empty() {
            Fidelity::Lossless
        } else if options.unknown_components == UnknownComponentPolicy::ConvertWithWarning {
            Fidelity::ConvertedWithWarning
        } else {
            Fidelity::PreservedOpaque
        };
        Ok(DecodedDocument {
            document,
            diagnostics,
            fidelity,
            source_state: None,
        })
    }

    fn encode(
        &self,
        document: &ComponentDocumentV2,
        catalog: &ComponentCatalog,
        options: EncodeOptions,
    ) -> Result<EncodedPayload, FormatError> {
        let mut document = document.clone();
        normalize_component_document(&mut document, catalog, &NormalizationOptions::default())?;
        validate_component_document(
            &document,
            catalog,
            &options.validation_limits,
            UnknownComponentPolicy::Reject,
        )?;
        let value =
            serde_json::to_value(document).map_err(|error| FormatError::InvalidPayload {
                format: self.descriptor.media_type.clone(),
                message: error.to_string(),
            })?;
        Ok(EncodedPayload {
            payload: EditorPayload::new(self.descriptor.media_type.clone(), value),
            diagnostics: Vec::new(),
            fidelity: Fidelity::Lossless,
        })
    }
}

pub struct LegacyV1DocumentFormat {
    descriptor: FormatDescriptor,
}

impl LegacyV1DocumentFormat {
    pub fn new() -> Self {
        Self {
            descriptor: FormatDescriptor {
                id: crate::document::DOCUMENT_SCHEMA_V1.into(),
                media_type: crate::document::DOCUMENT_SCHEMA_V1.to_string(),
                version: 1,
                capabilities: BTreeMap::new(),
            },
        }
    }
}

impl Default for LegacyV1DocumentFormat {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentFormat for LegacyV1DocumentFormat {
    fn descriptor(&self) -> &FormatDescriptor {
        &self.descriptor
    }

    fn decode(
        &self,
        input: &EditorPayload,
        catalog: &ComponentCatalog,
        options: DecodeOptions,
    ) -> Result<DecodedDocument, FormatError> {
        ensure_format(input, &self.descriptor)?;
        let legacy: EditorDocument = serde_json::from_value(input.value.clone())
            .map_err(|error| invalid_payload(input, error.to_string()))?;
        let mut document = migrate_v1_to_v2_with(
            &legacy,
            MigrationOptions {
                unknown_components: options.unknown_components,
            },
        )?;
        normalize_component_document(
            &mut document,
            catalog,
            &NormalizationOptions {
                repair_duplicate_ids: options.repair_duplicate_ids,
            },
        )?;
        validate_component_document(
            &document,
            catalog,
            &options.validation_limits,
            options.unknown_components,
        )?;
        Ok(DecodedDocument {
            document,
            diagnostics: Vec::new(),
            fidelity: Fidelity::Lossless,
            source_state: None,
        })
    }

    fn encode(
        &self,
        document: &ComponentDocumentV2,
        catalog: &ComponentCatalog,
        options: EncodeOptions,
    ) -> Result<EncodedPayload, FormatError> {
        validate_component_document(
            document,
            catalog,
            &options.validation_limits,
            UnknownComponentPolicy::Reject,
        )?;
        let legacy = migrate_v2_to_v1(document)?;
        let value = serde_json::to_value(legacy).map_err(|error| FormatError::InvalidPayload {
            format: self.descriptor.media_type.clone(),
            message: error.to_string(),
        })?;
        Ok(EncodedPayload {
            payload: EditorPayload::new(self.descriptor.media_type.clone(), value),
            diagnostics: Vec::new(),
            fidelity: Fidelity::Lossless,
        })
    }
}

pub struct PlainTextDocumentFormat {
    descriptor: FormatDescriptor,
}

impl PlainTextDocumentFormat {
    pub fn new() -> Self {
        Self {
            descriptor: FormatDescriptor {
                id: FORMAT_PLAIN_TEXT.into(),
                media_type: FORMAT_PLAIN_TEXT.to_string(),
                version: 1,
                capabilities: BTreeMap::new(),
            },
        }
    }
}

impl Default for PlainTextDocumentFormat {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentFormat for PlainTextDocumentFormat {
    fn descriptor(&self) -> &FormatDescriptor {
        &self.descriptor
    }

    fn decode(
        &self,
        input: &EditorPayload,
        catalog: &ComponentCatalog,
        options: DecodeOptions,
    ) -> Result<DecodedDocument, FormatError> {
        ensure_format(input, &self.descriptor)?;
        let text = input
            .value
            .as_str()
            .ok_or_else(|| invalid_payload(input, "expected a string"))?;
        let mut document = ComponentDocumentV2::plain_text(text);
        normalize_component_document(&mut document, catalog, &NormalizationOptions::default())?;
        validate_component_document(
            &document,
            catalog,
            &options.validation_limits,
            UnknownComponentPolicy::Reject,
        )?;
        Ok(DecodedDocument {
            document,
            diagnostics: Vec::new(),
            fidelity: Fidelity::Semantic,
            source_state: Some(FormatSourceState {
                format: self.descriptor.id.clone(),
                state: json!({ "original": text }),
            }),
        })
    }

    fn encode(
        &self,
        document: &ComponentDocumentV2,
        catalog: &ComponentCatalog,
        options: EncodeOptions,
    ) -> Result<EncodedPayload, FormatError> {
        validate_component_document(
            document,
            catalog,
            &options.validation_limits,
            UnknownComponentPolicy::Reject,
        )?;
        Ok(EncodedPayload {
            payload: EditorPayload::new(
                self.descriptor.media_type.clone(),
                Value::String(document.text_content()),
            ),
            diagnostics: Vec::new(),
            fidelity: Fidelity::Semantic,
        })
    }
}

fn ensure_format(input: &EditorPayload, descriptor: &FormatDescriptor) -> Result<(), FormatError> {
    if input.format == descriptor.id.0 || input.format == descriptor.media_type {
        Ok(())
    } else {
        Err(FormatError::FormatMismatch {
            expected: descriptor.media_type.clone(),
            actual: input.format.clone(),
        })
    }
}

fn invalid_payload(input: &EditorPayload, message: impl Into<String>) -> FormatError {
    FormatError::InvalidPayload {
        format: input.format.clone(),
        message: message.into(),
    }
}

fn preserve_unknown_nodes(
    node: &mut ComponentNode,
    catalog: &ComponentCatalog,
    policy: UnknownComponentPolicy,
    diagnostics: &mut Vec<FormatDiagnostic>,
    path: &str,
) -> Result<(), FormatError> {
    if catalog.spec(&node.kind).is_none() {
        if policy == UnknownComponentPolicy::Reject {
            return Err(FormatError::InvalidPayload {
                format: COMPONENT_DOCUMENT_FORMAT.to_string(),
                message: format!("unknown component '{}' at {path}", node.kind.0),
            });
        }

        let fallback = node
            .attrs
            .get("fallback")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                (policy == UnknownComponentPolicy::ConvertWithWarning).then(|| node.text_content())
            })
            .filter(|fallback| !fallback.is_empty())
            .ok_or_else(|| FormatError::InvalidPayload {
                format: COMPONENT_DOCUMENT_FORMAT.to_string(),
                message: format!(
                    "unknown component '{}' at {path} has no safe text fallback",
                    node.kind.0
                ),
            })?;
        let original_kind = node.kind.0.clone();
        let payload =
            serde_json::to_value(&*node).map_err(|error| FormatError::InvalidPayload {
                format: COMPONENT_DOCUMENT_FORMAT.to_string(),
                message: error.to_string(),
            })?;
        let id = node.id.clone();
        *node = ComponentNode {
            kind: COMPONENT_UNKNOWN.into(),
            id,
            attrs: Map::from_iter([
                (
                    "original_kind".to_string(),
                    Value::String(original_kind.clone()),
                ),
                ("fallback".to_string(), Value::String(fallback)),
                (
                    "payload".to_string(),
                    match payload {
                        Value::Object(payload) => Value::Object(payload),
                        _ => unreachable!("component nodes serialize to JSON objects"),
                    },
                ),
            ]),
            content: Vec::new(),
            text: None,
            marks: Vec::new(),
        };
        diagnostics.push(FormatDiagnostic {
            code: "unknown_component_preserved".to_string(),
            severity: DiagnosticSeverity::Warning,
            message: format!("preserved unknown component '{original_kind}' as an opaque atom"),
            source_range: None,
            node_id: node.id.as_ref().map(|id| id.0.clone()),
            recovery: Some("Install the component extension before editing this atom".to_string()),
        });
        return Ok(());
    }

    for (index, child) in node.content.iter_mut().enumerate() {
        preserve_unknown_nodes(
            child,
            catalog,
            policy,
            diagnostics,
            &format!("{path}.content[{index}]"),
        )?;
    }

    for (index, mark) in node.marks.iter().enumerate() {
        if catalog.spec(&mark.kind).is_none() {
            return Err(FormatError::InvalidPayload {
                format: COMPONENT_DOCUMENT_FORMAT.to_string(),
                message: format!("unknown mark '{}' at {path}.marks[{index}]", mark.kind.0),
            });
        }
    }
    Ok(())
}
