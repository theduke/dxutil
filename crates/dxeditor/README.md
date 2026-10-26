# dxeditor

A Rust rich-text editor for Dioxus, with structured block documents, Markdown
notes, message composers, undo/redo, tables, suggestions, and embedded components.

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

Editor components include their styles.

## Editor APIs

| Component | Value and use |
| --- | --- |
| `MarkdownEditor` | Markdown string |
| `Editor` | `EditorPayload` with an explicit catalog and output format |
| `ComponentEditor` | Structured `ComponentDocumentV2` block document |
| `profiles::composer::ComposerEditor` | Message text and chips, suggestions, and submission |
| `DocumentView` | Read-only `EditorDocument` rendering |

`ComponentEditor` defaults to `EditorCatalog::block_document()`. Use
`EditorCatalog::graph_node()` for rich node content without nested graphs, or
extend a catalog with your own component specifications. `IslandRegistry` mounts
your Dioxus components inside blocks. `EntityLinkExtension` supplies entity search,
preview, and open callbacks; entity links use `semantic:entity:<id>` URLs.

`on_ready` provides a `ComponentEditorHandle` for commands, undo/redo, insertion,
snapshots, and document replacement. `on_change` emits a validated
`ComponentChange` containing the document, revision, and dirty block IDs.
`on_invalid_change` reports invalid edits. Use `external_revision` to acknowledge
controlled updates and `HistoryPolicy` to choose replacement undo behavior.

## Features and platforms

| Feature | Default | Purpose |
| --- | --- | --- |
| `markdown` | Yes | Markdown parsing and codecs |
| `web` | Yes | Web renderer and browser API bindings |

Full rich-text editing requires `web` and a WASM build. Native targets render
block documents read-only; notes and composer use textarea fallbacks. The document
model, editing engine, and codecs can run natively. Disable default features for
headless use.

See [the design guide](docs/design.md) for document structure, transactions, and
custom component integration.

## Tests

After [workspace setup](../../README.md#development), run:

```sh
make test-editor-native test-editor-wasm test-editor-e2e
```

The [browser test guide](e2e/README.md) covers required tools, individual browser
projects, and reports.
