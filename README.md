# dxutil

[![CI](https://github.com/theduke/dxutil/actions/workflows/ci.yml/badge.svg)](https://github.com/theduke/dxutil/actions/workflows/ci.yml)

Dioxus components and libraries for Rust applications.

Explore all six crates in the [component playground](crates/playground/README.md).
Run `make serve-playground` for the web app or
`make serve-playground-desktop` for a Dioxus desktop webview window.

| Crate | Purpose |
| --- | --- |
| [`dxutil`](crates/dxutil/README.md) | Shared IDs, monotonic clocks, and browser event listeners |
| [`dxcomp`](crates/dxcomp/README.md) | Styled components, menus, dialogs, and virtual lists |
| [`dxeditor`](crates/dxeditor/README.md) | Rich-text documents, Markdown notes, and message composers |
| [`dxgraph`](crates/dxgraph/README.md) | Interactive graph canvas and layout algorithms |
| [`dxform`](crates/dxform/README.md) | Typed form state, validation, and submission |
| [`dxterm`](crates/dxterm/README.md) | xterm.js terminal component |
| [`dxcode`](crates/dxcode/README.md) | Tree-sitter highlighting and a read-only Rust code viewer |

## Use

Add the crates you need as Git dependencies. For example, in a Dioxus web app:

```toml
[dependencies]
dioxus = { version = "0.7.9", features = ["web"] }
dxcomp = { git = "https://github.com/theduke/dxutil", features = ["web"] }
```

Each crate's README includes usage examples, styling requirements, and platform
support. `dxterm` includes its JavaScript bundle. `dxcode` links its selected
grammars statically; see its README for C compiler and wasm requirements.

## Development

```sh
git clone https://github.com/theduke/dxutil.git
cd dxutil
nix develop .#ui
make check lint test
```

Without Nix, install stable Rust, Node.js 22, Python 3, and Make. The
`rust-toolchain.toml` installs Rust components and the WASM target through rustup.
Linux desktop builds also need GTK 3, WebKitGTK 4.1, libxdo, librsvg, OpenSSL
headers, and pkg-config. `dxcode` grammars need a C compiler; wasm builds need
a wasm-capable clang (LLVM clang, not Apple clang).

Run commands from the repository root:

| Command | Checks |
| --- | --- |
| `make check` | Rust workspace and all targets |
| `make lint` | Clippy |
| `make test` | Native tests and doctests |
| `make fmt` | Formatting; use `cargo fmt --all` to apply it |
| `make check-headless check-wasm check-desktop` | Feature and renderer configurations |
| `make test-code-native test-code-wasm` | Highlighter golden tests and real WASM runtime tests |
| `make measure-code-sizes` | Core WASM sizes for each grammar feature set |
| `make docs` | API documentation |
| `make install-js test-js check-bundles` | JavaScript tests, test types, and bundle reproducibility |
| `make build-playground test-playground` | Release web playground and browser interactions |
| `make check-playground check-playground-desktop` | Playground WASM and desktop compatibility |

After editing `dxterm` TypeScript, regenerate its bundle with
`make build-js` and commit the resulting `web/dist/` files.

The [editor browser suite](crates/dxeditor/e2e/README.md) tests Chromium, Firefox,
and WebKit through the [editor harness](crates/dxeditor_harness/README.md).
CI uses stable Rust and runs native checks on Linux, macOS, and Windows.

## License

[MIT](LICENSE). Third-party components retain their own notices, including the
[virtualizer's MIT/Apache-2.0 licenses](crates/dxcomp/src/components/virtual_list/).
