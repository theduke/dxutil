use crate::CodeError;
use std::{cell::RefCell, collections::HashMap, sync::Arc};
use tree_sitter::Query;

/// An enabled grammar and its lookup metadata.
#[derive(Debug)]
pub struct Language {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub extensions: &'static [&'static str],
    pub file_names: &'static [&'static str],
    source: fn() -> (tree_sitter::Language, &'static str, &'static str),
}

pub fn languages() -> &'static [Language] {
    LANGUAGES
}

/// Resolve enabled names and aliases, including Markdown fence info strings.
pub fn language_named(name: &str) -> Option<&'static Language> {
    let name = name
        .split_whitespace()
        .next()?
        .trim_start_matches("language-");
    LANGUAGES.iter().find(|l| {
        l.name.eq_ignore_ascii_case(name) || l.aliases.iter().any(|a| a.eq_ignore_ascii_case(name))
    })
}

/// Detect an enabled language from a file name or extension (case insensitive).
pub fn language_for_path(path: &str) -> Option<&'static str> {
    let name = path.rsplit(['/', '\\']).next()?;
    LANGUAGES
        .iter()
        .find(|l| {
            l.file_names.iter().any(|f| f.eq_ignore_ascii_case(name))
                || name.rsplit_once('.').is_some_and(|(_, ext)| {
                    l.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext))
                })
        })
        .map(|l| l.name)
}

pub(crate) struct Configuration {
    pub language: tree_sitter::Language,
    pub highlights: Query,
    pub injections: Query,
}
thread_local! {
    static CONFIGS: RefCell<HashMap<&'static str, Result<Arc<Configuration>, String>>> = RefCell::new(HashMap::new());
}
impl Language {
    // Queries are expensive to compile. Each thread compiles a grammar only once.
    pub(crate) fn configuration(&self) -> Result<Arc<Configuration>, CodeError> {
        CONFIGS.with(|cache| {
            cache
                .borrow_mut()
                .entry(self.name)
                .or_insert_with(|| {
                    let (language, highlights, injections) = (self.source)();
                    let highlights =
                        Query::new(&language, highlights).map_err(|e| e.to_string())?;
                    let injections =
                        Query::new(&language, injections).map_err(|e| e.to_string())?;
                    Ok(Arc::new(Configuration {
                        language,
                        highlights,
                        injections,
                    }))
                })
                .clone()
                .map_err(CodeError::Query)
        })
    }
}

static LANGUAGES: &[Language] = &[
    #[cfg(feature = "rust")]
    Language {
        name: "rust",
        aliases: &["rs"],
        extensions: &["rs"],
        file_names: &[],
        source: || {
            (
                arborium_rust::language().into(),
                arborium_rust::HIGHLIGHTS_QUERY,
                arborium_rust::INJECTIONS_QUERY,
            )
        },
    },
    #[cfg(feature = "javascript")]
    Language {
        name: "javascript",
        aliases: &["js", "jsx", "node"],
        extensions: &["js", "jsx", "mjs", "cjs"],
        file_names: &[],
        source: || {
            (
                arborium_javascript::language().into(),
                arborium_javascript::HIGHLIGHTS_QUERY,
                arborium_javascript::INJECTIONS_QUERY,
            )
        },
    },
    #[cfg(feature = "typescript")]
    Language {
        name: "typescript",
        aliases: &["ts"],
        extensions: &["ts", "mts", "cts"],
        file_names: &[],
        source: || {
            (
                arborium_typescript::language().into(),
                &arborium_typescript::HIGHLIGHTS_QUERY,
                arborium_typescript::INJECTIONS_QUERY,
            )
        },
    },
    #[cfg(feature = "tsx")]
    Language {
        name: "tsx",
        aliases: &[],
        extensions: &["tsx"],
        file_names: &[],
        source: || {
            (
                arborium_tsx::language().into(),
                &arborium_tsx::HIGHLIGHTS_QUERY,
                arborium_tsx::INJECTIONS_QUERY,
            )
        },
    },
    #[cfg(feature = "json")]
    Language {
        name: "json",
        aliases: &["jsonc"],
        extensions: &["json", "jsonc"],
        file_names: &[],
        source: || {
            (
                arborium_json::language().into(),
                arborium_json::HIGHLIGHTS_QUERY,
                arborium_json::INJECTIONS_QUERY,
            )
        },
    },
    #[cfg(feature = "toml")]
    Language {
        name: "toml",
        aliases: &[],
        extensions: &["toml"],
        file_names: &["Cargo.lock"],
        source: || {
            (
                arborium_toml::language().into(),
                arborium_toml::HIGHLIGHTS_QUERY,
                arborium_toml::INJECTIONS_QUERY,
            )
        },
    },
    #[cfg(feature = "yaml")]
    Language {
        name: "yaml",
        aliases: &["yml"],
        extensions: &["yaml", "yml"],
        file_names: &[],
        source: || {
            (
                arborium_yaml::language().into(),
                arborium_yaml::HIGHLIGHTS_QUERY,
                arborium_yaml::INJECTIONS_QUERY,
            )
        },
    },
    #[cfg(feature = "markdown")]
    Language {
        name: "markdown",
        aliases: &["md", "commonmark"],
        extensions: &["md", "markdown"],
        file_names: &[],
        source: || {
            (
                tree_sitter_md::LANGUAGE.into(),
                tree_sitter_md::HIGHLIGHT_QUERY_BLOCK,
                tree_sitter_md::INJECTION_QUERY_BLOCK,
            )
        },
    },
    #[cfg(feature = "html")]
    Language {
        name: "html",
        aliases: &["htm"],
        extensions: &["html", "htm"],
        file_names: &[],
        source: || {
            (
                arborium_html::language().into(),
                arborium_html::HIGHLIGHTS_QUERY,
                arborium_html::INJECTIONS_QUERY,
            )
        },
    },
    #[cfg(feature = "css")]
    Language {
        name: "css",
        aliases: &[],
        extensions: &["css"],
        file_names: &[],
        source: || {
            (
                arborium_css::language().into(),
                arborium_css::HIGHLIGHTS_QUERY,
                arborium_css::INJECTIONS_QUERY,
            )
        },
    },
    #[cfg(feature = "python")]
    Language {
        name: "python",
        aliases: &["py", "python3"],
        extensions: &["py", "pyi"],
        file_names: &[],
        source: || {
            (
                arborium_python::language().into(),
                arborium_python::HIGHLIGHTS_QUERY,
                arborium_python::INJECTIONS_QUERY,
            )
        },
    },
    #[cfg(feature = "go")]
    Language {
        name: "go",
        aliases: &["golang"],
        extensions: &["go"],
        file_names: &[],
        source: || {
            (
                arborium_go::language().into(),
                arborium_go::HIGHLIGHTS_QUERY,
                arborium_go::INJECTIONS_QUERY,
            )
        },
    },
    #[cfg(feature = "bash")]
    Language {
        name: "bash",
        aliases: &["sh", "shell", "zsh"],
        extensions: &["sh", "bash", "zsh"],
        file_names: &[".bashrc", ".zshrc", ".profile", ".bash_profile"],
        source: || {
            (
                arborium_bash::language().into(),
                arborium_bash::HIGHLIGHTS_QUERY,
                arborium_bash::INJECTIONS_QUERY,
            )
        },
    },
    #[cfg(feature = "sql")]
    Language {
        name: "sql",
        aliases: &[],
        extensions: &["sql"],
        file_names: &[],
        source: || {
            (
                arborium_sql::language().into(),
                arborium_sql::HIGHLIGHTS_QUERY,
                arborium_sql::INJECTIONS_QUERY,
            )
        },
    },
    #[cfg(feature = "c")]
    Language {
        name: "c",
        aliases: &[],
        extensions: &["c", "h"],
        file_names: &[],
        source: || {
            (
                arborium_c::language().into(),
                arborium_c::HIGHLIGHTS_QUERY,
                arborium_c::INJECTIONS_QUERY,
            )
        },
    },
    #[cfg(feature = "cpp")]
    Language {
        name: "cpp",
        aliases: &["c++", "cc", "cxx"],
        extensions: &["cpp", "cc", "cxx", "hpp", "hh", "hxx"],
        file_names: &[],
        source: || {
            (
                arborium_cpp::language().into(),
                &arborium_cpp::HIGHLIGHTS_QUERY,
                arborium_cpp::INJECTIONS_QUERY,
            )
        },
    },
    #[cfg(feature = "markdown")]
    Language {
        name: "markdown_inline",
        aliases: &["markdown-inline"],
        extensions: &[],
        file_names: &[],
        source: || {
            (
                tree_sitter_md::INLINE_LANGUAGE.into(),
                tree_sitter_md::HIGHLIGHT_QUERY_INLINE,
                tree_sitter_md::INJECTION_QUERY_INLINE,
            )
        },
    },
];
