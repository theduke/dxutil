# dxterm

An xterm.js terminal component for Dioxus. Connect it to your PTY transport to
display output and receive input, resize events, selections, and activated links.

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
                on_input: move |_data| { /* Forward to the PTY. */ },
                on_resize: move |(_cols, _rows)| { /* Resize the PTY. */ },
                on_link: move |_link| { /* Open the URL or file. */ },
                on_selection: move |_text| { /* Handle selected text. */ },
            }
        }
    }
}
```

Give the container a definite height. The terminal fits itself to container
resizes and includes its own styles. Interaction requires a DOM-capable Dioxus
renderer with JavaScript evaluation support.

## PTY integration

Create a writer with `use_terminal_writer()` and pass it to `TerminalView`.
Call `write_base64` with Base64-encoded PTY bytes. Encode bytes directly rather
than decoding each chunk as UTF-8; xterm handles sequences split across writes.
Await writes to sequence output; the writer consumes them while the view is mounted.
`writer.reset().await` resets the terminal.

Your app manages the PTY process, transport, connection lifecycle, and buffering.
`on_input` sends typed data to that transport; `on_resize` reports `(cols, rows)`.
`TerminalLink` distinguishes URLs from file links with a path, line, and optional
column.

## Appearance

Set `theme: TerminalTheme`, `font_family`, and `font_size` on the component, or
customize the CSS variables `--terminal-background`, `--terminal-foreground`,
`--terminal-cursor`, and `--terminal-selection`.

## Development

See [workspace setup](../../README.md#development). Run JavaScript tests and
rebuild the embedded bundle from the repository root:

```sh
npm ci --prefix crates/dxterm/web
npm test --prefix crates/dxterm/web
npm run build --prefix crates/dxterm/web
```
