# dxcode

A Dioxus code surface backed by CodeMirror 6, with controlled text, read-only mode,
selection/save callbacks, line navigation, and a syntax-highlighting helper.

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
                on_save: move |_| { /* Persist the current source value. */ },
            }
        }
    }
}
```

The parent must provide a definite height. `CodeView` injects its styles and
keeps its CodeMirror instance synchronized with `value`, `path`, and `read_only`.
The `on_save` callback handles Command/Ctrl-S. Optional `on_selection` reports a
`CodeSelection` containing selected text and one-based start/end line numbers.

Pass `goto: Some(LineTarget { line, column })` to scroll to a location and focus
the editor. Lines and columns are one-based; omitted columns start at column one.
Changing the target triggers navigation. Columns follow CodeMirror's UTF-16 text
positions.

## Languages and tokens

`language_for_path` detects Rust, JavaScript/TypeScript (including JSX/TSX), JSON,
CSS, HTML, Markdown, Python, Go, Java, C/C++, SQL, YAML, XML/SVG, shell, and TOML.
Unknown paths retain plain-text editing.

`highlight_lines(language, lines).await` returns token spans for each line using
CodeMirror's language parsers. It requires an active Dioxus JavaScript evaluation
context, even though its return value is a Rust data structure. Calls accept at
most 5,000 lines and return `CodeError` on failure. Returned `TokenSpan` start/end
offsets count Rust Unicode scalar values within each line; the end is exclusive.
Token classes follow Lezer's class highlighter, such as `tok-keyword` and
`tok-string`.

Override `--code-background`, `--code-text`, `--code-muted`, `--code-selection`,
and token color variables in your app CSS to match your theme.

## Runtime and assets

There are no crate feature flags. Editing requires a DOM-capable Dioxus renderer
with JavaScript evaluation support. The initial HTML includes a plain-text,
numbered fallback of up to 500 lines; the JavaScript editor hides that fallback
when it mounts. This fallback does not make the interactive editor available to
renderers without browser DOM support.

The committed `web/dist/code.iife.js` bundle is embedded by Rust. Consumers do not
need Node.js, and the bundle's existing `window.SemanticCode` global is retained
for compatibility. Each view is destroyed on unmount.

## Development

From the workspace root, enter `nix develop .#ui` when Nix is available:

```sh
cargo check -p dxcode --all-targets --locked --quiet --message-format=short
cargo test -p dxcode --locked --quiet --message-format=short
npm ci --prefix crates/dxcode/web
npm test --prefix crates/dxcode/web
npm run build --prefix crates/dxcode/web
```

The build enforces a 1,200,000-byte JavaScript bundle budget. Commit regenerated
`web/dist/` files with TypeScript or dependency changes. Workspace commands
`make test-js check-bundles` run JavaScript validation and check bundle consistency.
