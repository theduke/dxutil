//! Shared virtualized code surface and syntax tokens.
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
const ENGINE: &str = include_str!("../web/dist/code.iife.js");
const CSS: &str = include_str!("code.css");
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LineTarget {
    pub line: u32,
    pub column: Option<u32>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CodeSelection {
    pub start_line: u32,
    pub end_line: u32,
    pub text: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TokenSpan {
    pub start: u32,
    pub end: u32,
    pub class: String,
}
#[derive(Debug, thiserror::Error)]
pub enum CodeError {
    #[error("Highlight at most 5000 lines per call")]
    TooManyLines,
    #[error("Code engine: {0}")]
    Engine(String),
}
pub fn language_for_path(path: &str) -> Option<&'static str> {
    let name = path.rsplit('/').next()?.to_ascii_lowercase();
    if matches!(name.as_str(), ".bashrc" | ".zshrc" | ".profile") {
        return Some("shell");
    }
    Some(match name.rsplit('.').next()? {
        "rs" => "rust",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" => "typescript",
        "jsx" => "jsx",
        "tsx" => "tsx",
        "json" => "json",
        "css" => "css",
        "html" | "htm" => "html",
        "md" => "markdown",
        "py" => "python",
        "go" => "go",
        "java" => "java",
        "c" | "h" | "cpp" | "hpp" | "cc" => "cpp",
        "sql" => "sql",
        "yaml" | "yml" => "yaml",
        "xml" | "svg" => "xml",
        "sh" | "bash" | "zsh" => "shell",
        "toml" => "toml",
        _ => return None,
    })
}
fn bootstrap() -> String {
    format!("if (!window.SemanticCode) {{ {ENGINE}\nwindow.SemanticCode = SemanticCode; }}\n")
}
fn char_offset(line: &str, offset: u32) -> u32 {
    let mut utf16 = 0;
    let mut chars = 0;
    for c in line.chars() {
        if utf16 >= offset {
            break;
        }
        utf16 += c.len_utf16() as u32;
        chars += 1;
    }
    chars
}
pub async fn highlight_lines(
    language: &str,
    lines: Vec<String>,
) -> Result<Vec<Vec<TokenSpan>>, CodeError> {
    if lines.len() > 5000 {
        return Err(CodeError::TooManyLines);
    }
    let script = format!(
        "{}dioxus.send(window.SemanticCode.highlightLines({}, {}));",
        bootstrap(),
        serde_json::to_string(language).unwrap(),
        serde_json::to_string(&lines).unwrap()
    );
    let mut eval = document::eval(&script);
    let mut spans: Vec<Vec<TokenSpan>> = eval
        .recv()
        .await
        .map_err(|e| CodeError::Engine(e.to_string()))?;
    for (line, tokens) in lines.iter().zip(&mut spans) {
        for token in tokens {
            token.start = char_offset(line, token.start);
            token.end = char_offset(line, token.end);
        }
    }
    Ok(spans)
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Event {
    Change {
        value: String,
    },
    Selection {
        start_line: u32,
        end_line: u32,
        text: String,
    },
    Save,
}
#[component]
pub fn CodeView(
    value: String,
    path: Option<String>,
    #[props(default)] read_only: bool,
    #[props(default)] goto: Option<LineTarget>,
    #[props(default)] on_change: Option<EventHandler<String>>,
    #[props(default)] on_selection: Option<EventHandler<CodeSelection>>,
    #[props(default)] on_save: Option<EventHandler<()>>,
) -> Element {
    let id = use_hook(|| {
        format!(
            "dxcode-{}",
            NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        )
    });
    let mut mounted = use_signal(|| false);
    let options = serde_json::json!({"value":value,"path":path,"read_only":read_only,"goto":goto});
    let effect_id = id.clone();
    use_effect(use_reactive((&options,), move |(options,)| {
        let id = serde_json::to_string(&effect_id).unwrap();
        if mounted() {
            let script = format!("window.SemanticCode?.update({id}, {options});");
            let _ = document::eval(&script);
        } else {
            let script = format!(
                "{}window.SemanticCode.mount({id}, document.getElementById({id}), {options}, event => dioxus.send(event));",
                bootstrap()
            );
            mounted.set(true);
            spawn(async move {
                let mut eval = document::eval(&script);
                while let Ok(event) = eval.recv::<Event>().await {
                    match event {
                        Event::Change { value } => {
                            if let Some(handler) = on_change {
                                handler.call(value)
                            }
                        }
                        Event::Selection {
                            start_line,
                            end_line,
                            text,
                        } => {
                            if let Some(handler) = on_selection {
                                handler.call(CodeSelection {
                                    start_line,
                                    end_line,
                                    text,
                                })
                            }
                        }
                        Event::Save => {
                            if let Some(handler) = on_save {
                                handler.call(())
                            }
                        }
                    }
                }
            });
        }
    }));
    let drop_id = id.clone();
    use_drop(move || {
        let _ = document::eval(&format!(
            "window.SemanticCode?.destroy({});",
            serde_json::to_string(&drop_id).unwrap()
        ));
    });
    rsx! {style {"{CSS}"} div {class:"dxcode", pre {class:"dxcode-fallback", for (index,line) in value.lines().take(500).enumerate() {span {class:"dxcode-line", span {class:"dxcode-number", "{index + 1}"} "{line}\n"}}} div {id, class:"dxcode-host"}}}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn languages_and_unicode_offsets() {
        assert_eq!(language_for_path("src/app.TSX"), Some("tsx"));
        assert_eq!(language_for_path(".bashrc"), Some("shell"));
        assert_eq!(language_for_path("log.txt"), None);
        assert_eq!(char_offset("a😀b", 3), 2);
        assert_eq!(char_offset("a😀b", 4), 3);
    }
}
