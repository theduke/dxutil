# Editor browser harness

A Dioxus web app for testing `dxeditor` components with reproducible fixtures,
embedded components, host callbacks, and large documents.

## Run

After [workspace setup](../../README.md#development), run from the repository root:

```sh
dx serve --web --package dxeditor_harness
```

To serve built assets with the test suite's Python server:

```sh
make build-editor-harness
DXEDITOR_HARNESS_DIR="$PWD/target/dx/dxeditor_harness/debug/web/public" \
  DXEDITOR_PORT=4173 python3 crates/dxeditor/e2e/serve.py
```

Open `http://127.0.0.1:4173/blocks` when using that server. The harness runs in a
browser; native `cargo run` prints build instructions.

## Routes

| Route | Exercises |
| --- | --- |
| `/blocks` | Block documents |
| `/graph-node` | Restricted graph-node catalog |
| `/notes` | Markdown notes |
| `/composer` | Text/chip composer and suggestions |
| `/islands` | Embedded Dioxus components |
| `/perf` | Large documents with embedded entity blocks |

Add `?fixture=table`, `image`, `quote`, `todo`, `task-children`, `toggle`, `all`,
`mention`, `deleted-mention`, or `no-provider` for focused fixtures.
`/perf?blocks=10000` changes the document size; the default is 2,000 and the limit
is 20,000.

Controls exercise undo/redo, read-only mode, external revisions, document
replacement, text/chip insertion, snapshots, and entity picker callbacks.

## Tests

```sh
make install-js
make test-editor-native test-editor-wasm test-editor-e2e
```

See [browser test setup and reports](../dxeditor/e2e/README.md).
