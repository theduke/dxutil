# dxterm

A Rust terminal component for Dioxus 0.7. The same text-row renderer runs in
browsers, desktop system webviews and Dioxus Native (Blitz). Terminal parsing
and input encoding are provided by `dxterm_core`. No PTY or transport is included.

Choose `web`, `desktop`, or `native` to match the application's renderer. The
default feature set is headless for workspace checks. `native` takes precedence
over webview input glue if features are combined.

```rust,ignore
use dioxus::prelude::*;
use dxterm::{TerminalView, use_terminal_writer};

fn terminal() -> Element {
    let writer = use_terminal_writer();
    use_future(move || async move {
        writer.write(b"\x1b[32mhello\x1b[0m\r\n").await;
    });
    rsx! {
        div { style: "width:100%;height:400px;",
            TerminalView {
                writer,
                on_input: move |bytes: Vec<u8>| { /* send bytes to PTY */ },
                on_resize: move |(cols, rows)| { /* resize PTY */ },
                on_link: move |link| { /* open URL or file */ },
                on_selection: move |text| { /* selection changed */ },
            }
        }
    }
}
```

On web and desktop, give the component's parent a definite height and width. Cell dimensions come
from a mounted text probe. The container and probe are measured through
`get_client_rect`, including after font or container changes; `on_resize` fires
only when the column or row counts change. Measurements are polled approximately
every 100 ms, avoiding renderer-specific resize events. Zero-sized containers
are ignored until visible. Use an actually monospace font.

`TerminalWriter` is a copyable hook handle. `write(impl AsRef<[u8]>).await` queues
owned bytes in call order; `reset().await` resets the model. Completion means the
bytes have been parsed and a render snapshot published, not that the screen has
painted. Await writes to apply backpressure to a streaming transport. Concurrent
calls preserve enqueue order; await reset before replaying output. Mount a single
`TerminalView` per writer. Queued operations wait while no view is mounted.

`on_input: EventHandler<Vec<u8>>` emits exact PTY input bytes, including terminal
replies (DSR/DA etc.). Send these to the same session that supplies output.
`on_resize: EventHandler<(u16,u16)>`, `on_link: EventHandler<TerminalLink>` and
`on_selection: EventHandler<String>` are required callbacks. `TerminalLink` is
`File { path, line, column }` or `Url(String)`; opening links is the host's choice.
Optional `on_title`, `on_bell` and `on_clipboard_store` report title changes,
bells and OSC 52 store requests. The default OSC 52 callback does nothing; a host
must explicitly grant clipboard access. Clipboard reads requested by programs
are unsupported.

Optional appearance props are `theme: TerminalTheme`, `font_family: Option<String>`
and `font_size: Option<u16>` (default 13 on web/desktop, 13⅓ on Blitz for its
fixed cell metrics). Palette keys retain xterm's names:
`black`, `red`, …, `white`, `brightBlack`, …, `brightWhite`. Background, foreground,
cursor and selection default to CSS variables `--terminal-background`,
`--terminal-foreground`, `--terminal-cursor` and `--terminal-selection` with
fallback colors. Font defaults follow `--semantic-font-mono`, `--font-mono` and
a system monospace stack.

Click to focus. Drag selects cells, double-click selects a word and triple-click
selects a line. Ctrl/Cmd+C copies when there is a selection, otherwise Ctrl+C is
sent to the application. Ctrl/Cmd+backtick propagates to the host shortcut.
Shift temporarily bypasses application mouse reporting for local selection and
scrolling. Wheel scrolls history unless the application enables mouse reporting.
Shift+PageUp/PageDown scroll history by a page; Shift+Home/End go to its top/bottom.
Click detected URLs, file links or OSC 8 links to invoke `on_link`.

| Capability | Web (wasm) | Desktop (wry system webview) | Native (Blitz) |
| --- | --- | --- | --- |
| Text rows, colors, wide cells, scrollback, cursor | Yes | Yes | Yes |
| Keyboard, mouse, cell/word/line selection | Yes | Yes | Yes, with fixed metrics and viewport-origin placement |
| Wheel | Yes | Yes | Unavailable: Blitz does not dispatch wheel to Dioxus |
| Copy | navigator.clipboard; secure context and permission required | arboard | arboard; clipboard service required |
| Paste | Browser paste event, including bracketed paste | Ctrl/Cmd+V and paste via arboard | Ctrl/Cmd+V via arboard |
| IME/composition and preedit | Hidden textarea at cursor | Hidden textarea at cursor | Unsupported; handlers omitted |
| OS paste / Dioxus clipboard events | Yes | Yes | Unsupported; handlers omitted |
| Dioxus resize events | Not used | Not used | Unsupported; handlers omitted |
| Resize by mounted geometry polling | Yes | Yes | Unavailable: no mounted conversion; fixed 80×24 grid, 8×16 px cells |
| Focus in/out reports | Yes | Yes | Unavailable: renderer does not dispatch focus events |
| JavaScript | Browser clipboard/textarea APIs only | Textarea clearing via gated eval only | None |

Blitz support is best effort. Clipboard availability depends on the system's
display/clipboard service. Clipboard failures are ignored. No composition,
clipboard, pointer, wheel, mounted or resize-event conversion is requested from
dioxus-native-dom, whose converters for those events are unimplemented. Mouse
events use supported mouse data and client coordinates. On Blitz, place the
terminal at the viewport origin for correct cell hit testing and use an 8 px
monospace advance with 16 px line height. The fixed 80×24 size is reported once;
container resize and custom fonts cannot be measured. Keyboard history scrolling
remains available. Dioxus desktop is a webview and
does not share these Blitz limitations.

Rows are real text, with an application label, a log region and a throttled
polite live region (approximately twice per second, bounded to 4 KiB). Custom
selection and cursor are overlays. Assistive technology behavior varies between
renderers. Output is coalesced into one snapshot per frame tick; row components
memoize shared row allocations. Reduced-motion preferences disable CSS blinking.

Graphics (sixel, kitty and iTerm images), media keys and standalone modifier-key
reports are unsupported. Selection drag is limited to the component bounds.
Font ligatures are disabled; shaping and fallback fonts can vary by renderer.
The core's DEC 2027 grapheme clustering defaults off and follows application
requests. See `dxterm_core` for supported terminal/input modes.

Run the local echo examples without any PTY:

```sh
nix develop .#ui --command cargo run --locked -p dxterm --example desktop --features desktop
nix develop .#ui --command cargo run --locked -p dxterm --example native --features native
```

`make test-terminal-wasm` mounts a real Dioxus component in a browser, checks
colored and Unicode text, key bytes and resize calculations. `make check-wasm`,
`make check-desktop` and `make check-native` cover the three renderer features.
