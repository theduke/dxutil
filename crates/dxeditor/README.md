# dxeditor

A rich-text editor implemented in Rust for Dioxus. A platform-neutral editing
engine owns documents, transactions, selections, history, and validation. On
browser WASM, a `web-sys` adapter manages the editable DOM; Dioxus renders the host,
menus, and application-provided component islands.

## Use

```toml
[dependencies]
dioxus = { version = "0.7.9", features = ["web"] }
dxeditor = { git = "https://github.com/theduke/dxutil" }
```

For Markdown notes, use a controlled string:

```rust
use dioxus::prelude::*;
use dxeditor::MarkdownEditor;

fn App() -> Element {
    let mut notes = use_signal(|| "# Notes\n\nStart writing.".to_string());
    rsx! {
        MarkdownEditor {
            value: notes(),
            on_change: move |value| notes.set(value),
        }
    }
}
```

Editor components inject their own styles. The Markdown example uses the default
`markdown` and `web` features. No Node.js build is required to use the editor.

## Choose an editor

| API | Value and intended use |
| --- | --- |
| `MarkdownEditor` | Markdown `String` with `on_change` |
| `Editor` | `EditorPayload` plus a catalog and explicit output format |
| `ComponentEditor` | Structured `ComponentDocumentV2` block documents |
| `profiles::composer::ComposerEditor` | Message text and chips, suggestions, and submission events |
| `DocumentView` | Read-only rendering of the legacy `EditorDocument` model |

`ComponentEditor` defaults to `EditorCatalog::block_document()`. Use
`EditorCatalog::graph_node()` for rich node content without nested graphs, or
extend a catalog with your own component specifications. `IslandRegistry` mounts
host Dioxus components inside blocks; `EntityLinkExtension` supplies entity search,
preview, and open callbacks.

The `on_ready` callback provides a `ComponentEditorHandle` for commands, undo/redo,
selection-aware insertions, snapshots, and external document replacement. Valid
local changes emit `ComponentChange`, including the document, revision, and dirty
block IDs. Invalid changes call `on_invalid_change`. Controlled hosts can use
`external_revision` to acknowledge updates and distinguish external replacements.
Choose an explicit `HistoryPolicy` when replacing a document through the handle.

Existing Semantic-compatible document schema identifiers and
`semantic:entity:` links are preserved. The crate does not depend on Semantic's
database or application crates.

## Features and platforms

| Feature | Default | Purpose |
| --- | --- | --- |
| `markdown` | Yes | Markdown parsing and codecs |
| `web` | Yes | Dioxus web support and browser API bindings |

Full rich-text interaction requires both the `web` feature and a `wasm32` build.
On native targets, block documents render read-only with an editing notice;
notes and composer use textarea fallbacks. The editing engine and codecs remain
native-testable. Disable default features for consumers that only need the
headless document/engine APIs.

Input handles Unicode scalar model offsets, UTF-16 DOM offsets, grapheme deletion,
composition, clipboard formats, tables, and host media requests. The detailed
ownership and persistence rules are in [the design document](docs/design.md).

## Development and tests

From the workspace root, use `nix develop .#ui` when Nix is available:

```sh
make test-editor-native test-editor-wasm test-editor-e2e
make check-headless check-wasm
cargo fmt --all
```

The native suite covers the engine, history, formats, and clipboard policy. WASM
tests exercise DOM behavior through `wasm-bindgen-test`. Playwright runs the
[editor harness](../dxeditor_harness/README.md) in Chromium, Firefox, and WebKit.
See [browser setup and diagnostics](e2e/README.md) and the
[root development guide](../../README.md) for required tools.

Browser implementation belongs in `src/dom/`; test-only TypeScript belongs in
`e2e/`. Keep both the native and browser suites when changing engine or DOM code.
