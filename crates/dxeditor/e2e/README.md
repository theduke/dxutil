# Editor browser tests

Playwright tests for rich-text editing, Markdown notes, composers, accessibility,
clipboard input, embedded components, and large documents. The suite runs the
[editor harness](../../dxeditor_harness/README.md) in Chromium, Firefox, and WebKit.

## Setup

Run from the repository root. On Linux with Nix:

```sh
nix develop .#ui
make install-js
make test-editor-native test-editor-wasm test-editor-e2e
```

For other environments, start with [workspace setup](../../../README.md#development)
and install the browser build tools:

```sh
cargo install dioxus-cli --version 0.7.9 --locked
cargo install wasm-bindgen-cli --version 0.2.121 --locked
make install-js
cd crates/dxeditor/e2e
npx playwright install --with-deps chromium firefox webkit
cd ../../..
```

The wasm-bindgen CLI must match `wasm-bindgen` in `Cargo.lock`. WASM tests also
need Chrome/Chromium and a matching chromedriver on PATH. The Linux Nix shell
supplies these tools and browsers; on macOS, install Playwright browsers even
when using Nix.

## Run

From the repository root:

```sh
make test-editor-native test-editor-wasm test-editor-e2e
```

For one browser or test file, build the harness first:

```sh
make build-editor-harness
cd crates/dxeditor/e2e
npx playwright test --project chromium
npx playwright test editor.spec.ts --project chromium
npm run typecheck
```

Playwright starts the Python server automatically. Chromium runs the CDP
composition and performance cases; Firefox and WebKit run the applicable cases.

## Configuration and reports

| Variable | Purpose |
| --- | --- |
| `DXEDITOR_HARNESS_DIR` | Built public asset directory |
| `DXEDITOR_PORT` | Test server port; default `4173` |
| `DXEDITOR_RESULTS_DIR` | Results directory, relative to this directory |
| `PLAYWRIGHT_CHROMIUM_PATH` | Custom Chromium executable |
| `CHROMEDRIVER` | Driver executable for WASM tests |
| `WASM_BINDGEN_TEST_WEBDRIVER_JSON` | WebDriver capabilities file for WASM tests |

Failures retain screenshots and traces. Results are written to `test-results/`
and the HTML report to `playwright-report/` by default. CI uploads both in the
`editor-browser-results` artifact. Performance tests attach their measurements;
compare timings using the same machine and build profile.
