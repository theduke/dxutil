# dxeditor_harness

An unpublished Dioxus web application for exercising the real public `dxeditor`
components. Playwright drives this app rather than a separate JavaScript editor
implementation. It provides reproducible fixtures, host callbacks, document
snapshots, and performance cases.

## Run locally

From the workspace root, enter `nix develop .#ui` when Nix is available:

```sh
make build-editor-harness
DXEDITOR_HARNESS_DIR="$PWD/target/dx/dxeditor_harness/debug/web/public" \
  DXEDITOR_PORT=4173 python3 crates/dxeditor/e2e/serve.py
```

Open `http://127.0.0.1:4173/blocks`. Dioxus CLI, the Rust WASM target, and a matching
wasm-bindgen CLI must be installed; see the [root setup guide](../../README.md).
For the Dioxus development server, use
`dx serve --web --package dxeditor_harness` from the workspace root.

A native `cargo run -p dxeditor_harness` only prints build instructions. It does
not start a desktop editor or the browser test server.

## Routes and fixtures

| Route | Exercises |
| --- | --- |
| `/blocks` | Full block-document editing |
| `/graph-node` | Restricted graph-node catalog |
| `/notes` | Markdown string editing |
| `/composer` | Text/chip composer, suggestions, and submission events |
| `/islands` | Host Dioxus components embedded in editor blocks |
| `/perf` | Large documents with embedded entity blocks |

Use `?fixture=table`, `image`, `quote`, `todo`, `task-children`, `toggle`, `all`,
`mention`, `deleted-mention`, or `no-provider` for focused block fixtures.
`/perf?blocks=10000` changes the performance document size; the count defaults to
2,000 and is capped at 20,000.

Harness controls expose undo/redo, read-only mode, revision acknowledgements,
document replacement with both history policies, text/chip insertion, snapshots,
and mock host entity creation/picker completion. `window.__dxeditorHarness` is a
test-only snapshot consumed by the Playwright helpers, not a public library API.

## Browser tests

```sh
make install-js
make test-editor-native test-editor-wasm test-editor-e2e
```

`test-editor-e2e` builds the app and lets Playwright start the Python SPA server.
The suite covers Chromium, Firefox, and WebKit. For a focused project after
building the harness:

```sh
cd crates/dxeditor/e2e
npx playwright test --project chromium
```

`DXEDITOR_HARNESS_DIR` overrides the public asset directory; `DXEDITOR_PORT`
changes the server port; `DXEDITOR_RESULTS_DIR` changes test output; and
`PLAYWRIGHT_CHROMIUM_PATH` overrides Chromium. Failure reports, screenshots, and
traces are written beneath `crates/dxeditor/e2e/` by default. See
[the browser suite guide](../dxeditor/e2e/README.md) for browser installation and
platform requirements.

Keep test fixtures and host controls here; editor behavior belongs in `dxeditor`.
