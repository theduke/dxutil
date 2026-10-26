pub use component_editor::{
    ComponentChange, ComponentEditor, ComponentEditorEvent, ComponentEditorHandle,
};
pub use islands::{IslandContext, IslandRegistry, IslandRenderer, IslandState};
pub use protocol::InsertAt;
pub use protocol::PastedFile;
pub mod catalog;
pub mod codec;
pub mod component;
pub mod component_editor;
pub mod component_spec;
pub mod document;
pub mod document_v2;
pub mod engine;
pub mod entity_link;
#[cfg(not(all(feature = "web", target_arch = "wasm32")))]
mod fallback;
pub mod format;
pub mod islands;
#[cfg(feature = "markdown")]
pub mod markdown;
#[cfg(feature = "markdown")]
pub mod markdown_v2;
pub mod migrate;
pub mod profiles;
pub mod protocol;
mod render_v2;
pub mod suggestion;
pub mod ui;

pub use catalog::{EditorCatalog, EditorCatalogBuilder, EditorCatalogConfig};
pub use codec::EditorPayload;
pub use component::{DocumentView, Editor, MarkdownEditor};
pub use component_spec::{
    AttributeSpec, AttributeType, ClipboardPolicy, ComponentBehavior, ComponentCatalog,
    ComponentCatalogError, ComponentKind, ComponentSpec, ContentRule, DomDescriptor,
    FORMAT_MARKDOWN, FORMAT_PLAIN_TEXT, FORMAT_TYPED_DOCUMENT, FormatCapability, IdentityPolicy,
    PlainTextFallback, UrlRole, is_safe_media_url, is_safe_url, is_safe_url_for_role,
    register_standard_component_specs, validate_component_document,
};
pub use document::{
    BlockNode, EditorDocument, InlineNode, Mark, NodeContent, NodeId, TableCell, TableNode,
    TableRow,
};
pub use document_v2::{
    AttributeMap, COMPONENT_DOCUMENT_FORMAT, COMPONENT_DOCUMENT_SCHEMA, COMPONENT_DOCUMENT_VERSION,
    CodeBlockAttributes, ComponentDocumentV2, ComponentId, ComponentMark, ComponentNode,
    DocumentMetadata, DocumentSchemaId, DocumentValidationError, HeadingAttributes,
    ImageAttributes, LinkAttributes, MentionAttributes, NormalizationOptions,
    OpaqueMarkdownAttributes, OrderedListAttributes, TableAlignment, TableCellAttributes,
    TaskItemAttributes, UnknownComponentAttributes, UnknownComponentPolicy, ValidationIssue,
    ValidationLimits, normalize_component_document,
};
pub use entity_link::{
    EntityLinkCandidate, EntityLinkExtension, EntityLinkPreview, EntityLinkProvider,
    EntityPreviewField,
};
pub use format::{
    DecodeOptions, DecodedDocument, DiagnosticSeverity, DocumentFormat, DocumentFormatRegistry,
    EncodeOptions, EncodedPayload, Fidelity, FormatDescriptor, FormatDiagnostic, FormatError,
    FormatId, FormatSourceState, LegacyV1DocumentFormat, PlainTextDocumentFormat, SourceRange,
    TYPED_DOCUMENT_JSON_SCHEMA, TypedDocumentCodec, register_standard_document_formats,
};
#[cfg(feature = "markdown")]
pub use markdown_v2::MarkdownDocumentFormat;
pub use migrate::{
    MigrationError, MigrationOptions, migrate_v1_to_v2, migrate_v1_to_v2_with, migrate_v2_to_v1,
};
pub use protocol::{EngineEvent, HistoryPolicy, MentionSuggestion};
pub use suggestion::{
    SuggestionItem, SuggestionProvider, SuggestionQueryContext, SuggestionRegistry,
};

#[derive(Debug, thiserror::Error)]
pub enum EditorError {
    #[error("unknown codec format '{0}'")]
    UnknownFormat(String),
    #[error("invalid payload for format '{format}': {message}")]
    InvalidPayload { format: String, message: String },
    #[error("unknown command '{0}'")]
    UnknownCommand(String),
    #[error("unknown action '{0}'")]
    UnknownAction(String),
    #[error("transaction failed: {0}")]
    Transaction(String),
    #[error("{0}")]
    Message(String),
}

#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub mod dom;
