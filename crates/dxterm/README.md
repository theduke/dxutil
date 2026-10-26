# dxterm

A Dioxus terminal component backed by xterm.js. It displays byte-oriented terminal
output and reports input, dimensions, selections, and activated links to the host.
It does not create a PTY, start processes, or transport terminal data.

## Use

```toml
[dependencies]
dioxus = { version = "0.7.9", features = ["web"] }
dxterm = { git = "https://github.com/theduke/dxutil" }
```

```rust
use dioxus::prelude::*;
use dxterm::{TerminalView, use_terminal_writer};

fn App() -> Element {
    let writer = use_terminal_writer();
    use_effect(move || {
        spawn(async move {
            // Base64 of "Hello, terminal!\r\n".
            writer.write_base64("SGVsbG8sIHRlcm1pbmFsIQ0K".into()).await;
        });
    });
    rsx! {
        div { style: "height: 320px;",
            TerminalView {
                writer,
                on_input: move |_data| { /* Send data to your PTY transport. */ },
                on_resize: move |(_cols, _rows)| { /* Resize the host PTY. */ },
                on_link: move |_link| { /* Open a TerminalLink::File or ::Url. */ },
                on_selection: move |_text| { /* Update host selection state. */ },
            }
        }
    }
}
```

Give the component a container with a definite height. It observes host size
changes and uses xterm's Fit addon to update rows and columns.

## Output and integration

`TerminalWriter::write_base64` accepts a Base64-encoded byte chunk. Preserve PTY
output as bytes rather than decoding each chunk separately: xterm's UTF-8 decoder
can carry incomplete characters between writes. Awaiting a write provides host
sequencing after the browser write callback; it does not return a transport error.
`writer.reset().await` resets the mounted terminal.

Create the writer with `use_terminal_writer()` and pass it to `TerminalView`.
Writes are consumed by that mounted view; do not wait for writes before mounting
it. The host owns connection setup, disconnection, buffering, and PTY lifecycle.

`TerminalLink` distinguishes URL links from `path:line(:column)` file links.
`TerminalTheme`, `font_family`, and `font_size` customize appearance. The component
also reads `--terminal-background`, `--terminal-foreground`, `--terminal-cursor`,
and `--terminal-selection` CSS properties. Its font fallback checks
`--semantic-font-mono`, then `--font-mono`. Existing names remain for compatibility.

## Runtime and build assets

There are no crate feature flags. Interactive rendering requires a DOM-capable
Dioxus renderer and its JavaScript evaluation bridge. xterm.js and its styles are
embedded from committed `web/dist/` files, so Rust consumers do not need Node.js.
The wrapper uses the compatibility global `window.SemanticTerminal`. Unmounting
disconnects its observers and disposes its xterm instance.

## Development

From the workspace root, enter `nix develop .#ui` when Nix is available:

```sh
cargo check -p dxterm --all-targets --locked --quiet --message-format=short
cargo test -p dxterm --locked --quiet --message-format=short
npm ci --prefix crates/dxterm/web
npm test --prefix crates/dxterm/web
npm run build --prefix crates/dxterm/web
```

Commit regenerated `web/dist/` files with TypeScript or dependency changes.
The workspace `make test-js check-bundles` commands validate both JS-backed
libraries. Current terminal JavaScript unit tests cover file-link parsing; they
do not replace an interactive PTY integration test.
