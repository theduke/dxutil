//! Tree-sitter highlighting and a read-only Dioxus code viewer.
//!
//! Core ranges use UTF-8 **byte** offsets. [`highlight_lines`] returns per-line
//! Unicode scalar (**char**) offsets, suitable for diff rendering, never UTF-16.
mod highlight;
mod languages;
mod session;
mod tokens;
#[cfg(feature = "viewer")]
mod view;

pub use highlight::{HighlightSpan, highlight_lines};
pub use languages::{Language, language_for_path, language_named, languages};
use serde::{Deserialize, Serialize};
pub use session::{HighlightSession, TextEdit};
pub use tokens::{TokenKind, TokenSpan};
#[cfg(feature = "viewer")]
pub use view::{CodeView, CodeViewProps};

/// One-based line and optional one-based Unicode scalar column.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LineTarget {
    pub line: u32,
    pub column: Option<u32>,
}

/// Selection over rendered lines. Line numbers are one-based.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CodeSelection {
    pub start_line: u32,
    pub end_line: u32,
    pub text: String,
}

#[derive(Debug, thiserror::Error)]
pub enum CodeError {
    #[error("Invalid UTF-8 byte range")]
    InvalidRange,
    #[error("Highlight query: {0}")]
    Query(String),
    #[error("Parser: {0}")]
    Parser(String),
}
