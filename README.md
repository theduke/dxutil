# dxutil

[![CI](https://github.com/theduke/dxutil/actions/workflows/ci.yml/badge.svg)](https://github.com/theduke/dxutil/actions/workflows/ci.yml)

Dioxus components and libraries for Rust applications.

Explore the UI crates in the [component playground](https://theduke.github.io/dxutil/).
Run `make serve-playground` for the web app or
`make serve-playground-desktop` for a Dioxus desktop webview window.

| Crate | Purpose | crates.io |
| --- | --- | --- |
| [`dxutil`](crates/dxutil/README.md) | Shared IDs, monotonic clocks, and browser event listeners | [![crates.io](https://img.shields.io/crates/v/dxutil.svg)](https://crates.io/crates/dxutil) |
| [`dxcomp`](crates/dxcomp/README.md) | Styled components, menus, dialogs, and virtual lists | [![crates.io](https://img.shields.io/crates/v/dxcomp.svg)](https://crates.io/crates/dxcomp) |
| [`dxeditor`](crates/dxeditor/README.md) | Rich-text documents, Markdown notes, and message composers | [![crates.io](https://img.shields.io/crates/v/dxeditor.svg)](https://crates.io/crates/dxeditor) |
| [`dxgraph`](crates/dxgraph/README.md) | Interactive graph canvas and layout algorithms | [![crates.io](https://img.shields.io/crates/v/dxgraph.svg)](https://crates.io/crates/dxgraph) |
| [`dxform`](crates/dxform/README.md) | Typed form state, validation, and submission | [![crates.io](https://img.shields.io/crates/v/dxform.svg)](https://crates.io/crates/dxform) |
| [`dxterm`](crates/dxterm/README.md) | Rust terminal component for Dioxus | [![crates.io](https://img.shields.io/crates/v/dxterm.svg)](https://crates.io/crates/dxterm) |
| [`dxterm_core`](crates/dxterm_core/README.md) | Headless terminal model and input encoding | [![crates.io](https://img.shields.io/crates/v/dxterm_core.svg)](https://crates.io/crates/dxterm_core) |
| [`dxhighlight`](crates/dxhighlight/README.md) | Tree-sitter highlighting and a read-only Rust code viewer | [![crates.io](https://img.shields.io/crates/v/dxhighlight.svg)](https://crates.io/crates/dxhighlight) |
| [`dxcomp-primitives`](crates/dxcomp-primitives/README.md) | Temporary fork of upstream `dioxus-primitives` | [![crates.io](https://img.shields.io/crates/v/dxcomp-primitives.svg)](https://crates.io/crates/dxcomp-primitives) |
| [`dxcomp-attributes`](crates/dxcomp-attributes/README.md) | Companion fork of upstream attribute macros | [![crates.io](https://img.shields.io/crates/v/dxcomp-attributes.svg)](https://crates.io/crates/dxcomp-attributes) |

## Use

Add the crates you need as Git dependencies. For example, in a Dioxus web app:

```toml
[dependencies]
dioxus = { version = "0.7.9", features = ["web"] }
dxcomp = { git = "https://github.com/theduke/dxutil", features = ["web"] }
```

Each crate's README includes usage examples, styling requirements, and platform
support. Both terminal crates are written in Rust. `dxhighlight` links its selected
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
headers, and pkg-config. `dxhighlight` grammars need a C compiler; wasm builds need
a wasm-capable clang (LLVM clang, not Apple clang).

Run commands from the repository root:

| Command | Checks |
| --- | --- |
| `make check` | Rust workspace and all targets |
| `make lint` | Clippy |
| `make test` | Native tests and doctests |
| `make fmt` | Formatting; use `cargo fmt --all` to apply it |
| `make check-headless check-wasm check-desktop check-native` | Feature and renderer configurations |
| `make test-code-native test-code-wasm` | Highlighter golden tests and real WASM runtime tests |
| `make measure-code-sizes` | Core WASM sizes for each grammar feature set |
| `make test-terminal-wasm` | Terminal component in a real browser |
| `make docs` | API documentation |
| `make install-js test-js` | JavaScript tooling and browser test types |
| `make build-playground test-playground` | Release web playground and browser interactions |
| `make check-playground check-playground-desktop` | Playground WASM and desktop compatibility |

The [editor browser suite](crates/dxeditor/e2e/README.md) tests Chromium, Firefox,
and WebKit through the [editor harness](crates/dxeditor_harness/README.md).
CI uses stable Rust and runs native checks on Linux, macOS, and Windows.

## Publishing

All library crates share `workspace.package.version` in the root `Cargo.toml`.
When bumping it, update the internal versions in `workspace.dependencies` to
match. Package metadata and dependency versions are checked in CI with
`make check-package-metadata`. The playground and editor harness are private
workspace applications with `publish = false`.

From a clean checkout, validate the release without uploading:

```sh
make publish-dry-run
```

Then publish the libraries together with `cargo publish --workspace --locked`.
Cargo orders packages by their dependencies. `dxcomp-primitives` and
`dxcomp-attributes` are temporary forks of the previously pinned upstream Git
dependency; their READMEs record provenance and licenses. See [TODO.md](TODO.md)
for switching back to published upstream crates.

## License

[MIT](LICENSE). Third-party components retain their own notices, including the
[virtualizer's MIT/Apache-2.0 licenses](crates/dxcomp/src/components/virtual_list/).
The two upstream forks retain their original MIT OR Apache-2.0 licenses.
