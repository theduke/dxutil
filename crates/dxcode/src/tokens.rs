use serde::{Deserialize, Serialize};

/// Stable CSS token vocabulary. More specific query captures map to these kinds.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TokenKind {
    Keyword,
    Operator,
    String,
    Number,
    Comment,
    Type,
    Name,
    Punctuation,
    Invalid,
    Heading,
    Emphasis,
    Link,
}

impl TokenKind {
    pub const fn class(self) -> &'static str {
        match self {
            Self::Keyword => "tok-keyword",
            Self::Operator => "tok-operator",
            Self::String => "tok-string",
            Self::Number => "tok-number",
            Self::Comment => "tok-comment",
            Self::Type => "tok-type",
            Self::Name => "tok-name",
            Self::Punctuation => "tok-punctuation",
            Self::Invalid => "tok-invalid",
            Self::Heading => "tok-heading",
            Self::Emphasis => "tok-emphasis",
            Self::Link => "tok-link",
        }
    }

    pub(crate) fn capture(name: &str) -> Option<Self> {
        let first = name.split('.').next()?;
        Some(match first {
            "keyword" | "conditional" | "repeat" | "include" | "exception" => Self::Keyword,
            "operator" => Self::Operator,
            "string" | "character" | "escape" => Self::String,
            "number" | "float" | "boolean" | "constant" => Self::Number,
            "comment" => Self::Comment,
            "type" | "constructor" | "namespace" | "module" => Self::Type,
            "variable" | "function" | "property" | "attribute" | "tag" | "label" => Self::Name,
            "punctuation" => Self::Punctuation,
            "error" => Self::Invalid,
            "markup" | "text" => {
                if name.contains("heading") || name.contains("title") {
                    Self::Heading
                } else if name.contains("link") || name.contains("uri") {
                    Self::Link
                } else if name.contains("raw") || name.contains("literal") {
                    Self::String
                } else if name.contains("strong")
                    || name.contains("emphasis")
                    || name.contains("italic")
                    || name.contains("bold")
                {
                    Self::Emphasis
                } else {
                    return None;
                }
            }
            _ => return None,
        })
    }
}

/// Half-open Unicode scalar offsets within a line (excluding CRLF/LF).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TokenSpan {
    pub start: u32,
    pub end: u32,
    pub class: String,
}
