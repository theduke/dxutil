# dxcode

A CodeMirror 6 editor for Dioxus, with controlled text, syntax highlighting,
read-only mode, selection callbacks, and line navigation.

## Use

```toml
[dependencies]
dioxus = { version = "0.7.9", features = ["web"] }
dxcode = { git = "https://github.com/theduke/dxutil" }
```

```rust
use dioxus::prelude::*;
use dxcode::CodeView;

fn App() -> Element {
    let mut source = use_signal(|| "fn main() {}\n".to_string());
    rsx! {
        div { style: "height: 360px;",
            CodeView {
                value: source(),
                path: Some("src/main.rs".to_string()),
                on_change: move |value| source.set(value),
                on_save: move |_| { /* Persist the current value. */ },
            }
        }
    }
}
```

Give the parent a definite height. The component includes its styles and requires
a DOM-capable Dioxus renderer with JavaScript evaluation support.

## Editing and navigation

Keep `value` synchronized through `on_change`. Set `read_only` to disable editing.
`path` selects the language; unknown extensions use plain text. `on_save` handles
Command/Ctrl-S, and `on_selection` receives the selected text and one-based line
numbers in a `CodeSelection`.

Pass `goto: Some(LineTarget { line, column })` to focus and scroll to a location.
Lines and columns are one-based; omitted columns start at column one. Columns
use UTF-16 positions. Navigation runs when the target changes.

Supported languages include Rust, JavaScript/TypeScript and JSX/TSX, JSON, CSS,
HTML, Markdown, Python, Go, Java, C/C++, SQL, YAML, XML, shell, and TOML.
`language_for_path` exposes filename-based detection.

## Token highlighting and styles

`highlight_lines(language, lines).await` returns `TokenSpan` values for up to
5,000 lines. It requires a Dioxus JavaScript evaluation context. Token start/end
offsets count Unicode scalar values within each line, with an exclusive end.
Classes use Lezer names such as `tok-keyword` and `tok-string`.

Override `--code-background`, `--code-text`, `--code-muted`, `--code-selection`,
and token color variables in your app CSS. Static HTML includes a numbered
plain-text preview of up to 500 lines until the interactive editor mounts.

## Development

See [workspace setup](../../README.md#development). Run JavaScript tests and
rebuild the embedded bundle from the repository root:

```sh
npm ci --prefix crates/dxcode/web
npm test --prefix crates/dxcode/web
npm run build --prefix crates/dxcode/web
```

The build enforces a 1,200,000-byte JavaScript bundle limit.
