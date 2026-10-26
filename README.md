# dxutil

Reusable Dioxus libraries for Rust applications.

| Crate | Purpose |
| --- | --- |
| [`dxcomp`](crates/dxcomp/README.md) | Prestyled components, popup positioning, and virtual lists |
| [`dxeditor`](crates/dxeditor/README.md) | Rust rich-text editor, Markdown, and browser DOM integration |
| [`dxgraph`](crates/dxgraph/README.md) | Graph canvas, layouts, selection, and gestures |
| [`dxform`](crates/dxform/README.md) | Headless form state, validation, and submission |
| [`dxterm`](crates/dxterm/README.md) | xterm.js terminal component |
| [`dxcode`](crates/dxcode/README.md) | CodeMirror code component and syntax highlighting |
| [`dxeditor_harness`](crates/dxeditor_harness/README.md) | Unpublished editor browser test application |

There is no `dxutil` library crate yet. Shared functionality will be extracted
when concrete consumers justify it.

## Development

Requires Rust (MSRV 1.90), the `wasm32-unknown-unknown` target, Node.js 22,
Python 3, and Make. Use the pinned Nix environment when Nix is available:

```sh
nix develop .#ui
make install-js
make fmt check test check-headless check-wasm docs
make test-js check-bundles
```

Without Nix, `rustup show` installs the toolchain declared in
`rust-toolchain.toml`; install Node.js, Python and Make separately.
Native Linux builds may require OpenSSL development files and pkg-config.
The optional desktop check additionally needs GTK 3, WebKitGTK 4.1,
libxdo, and librsvg development packages:

```sh
make check-desktop
```

## Browser tests

The editor suite tests the real Rust editor in Chromium, Firefox and WebKit.
For a non-Nix environment, install matching CLI tools and browser dependencies:

```sh
cargo install dioxus-cli --version 0.7.9 --locked
cargo install wasm-bindgen-cli --version 0.2.121 --locked
cd crates/dxeditor/e2e
npm ci
npx playwright install --with-deps chromium firefox webkit
cd ../../..
make test-editor-native test-editor-wasm test-editor-e2e
```

The wasm test runner also requires Chrome/Chromium and a matching chromedriver on
PATH. Set `CHROMEDRIVER` and `WASM_BINDGEN_TEST_WEBDRIVER_JSON` when overriding
these. The wasm-bindgen CLI version must match `wasm-bindgen` in `Cargo.lock`.
See [editor browser tests](crates/dxeditor/e2e/README.md) for suite details.

`dxcode` and `dxterm` embed committed bundles, so Rust-only consumers do not need
Node.js. After editing their TypeScript, run `make build-js` and commit `web/dist/`.
Their current JavaScript global names and Semantic-compatible editor entity URLs
are retained for compatibility.

## CI

CI runs formatting, Clippy, documentation, native tests on Linux/macOS/Windows,
headless and WASM feature checks, a Linux desktop compile check, JavaScript tests
and bundle reproducibility, and editor WASM/browser tests. Actions are pinned to
commit hashes. Browser failures retain reports and traces. Configure branch
protection to require the stable **CI required** check. Dependabot maintains Rust,
npm, and GitHub Actions dependencies; security scanning audits `Cargo.lock`.

## Origin and licensing

Copied from [Semantic](https://github.com/semantiverse/semantic), commit
`f43fa65b83edfad3d5985cef9cc43e759f6ebcba`, on 2026-10-26. The source copies remain
in Semantic. Crate behavior is preserved; initial setup fixes a few private documentation links
and updates Vitest to its patched release.

MIT licensed. `dxcomp` contains components ported from DioxusLabs and a virtualizer
with its original MIT/Apache notices retained alongside its source. JavaScript
bundles retain upstream license notices.
