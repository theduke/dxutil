# dxterm_core

A headless terminal model for native Rust and `wasm32-unknown-unknown`. It owns
VT parsing, scrollback, incremental render snapshots, selection, links and input
encoding. It has no Dioxus, windowing, PTY or transport dependency.

```rust
use dxterm_core::{Terminal, TerminalEvent};
use dxterm_core::input::{Key, KeyEvent, Modifiers};

let mut terminal = Terminal::new(80, 24, 10_000);
terminal.write(b"\x1b[31mhello\x1b[0m\r\n");
let snapshot = terminal.snapshot();
assert!(snapshot.rows[0].text().starts_with("hello"));
let bytes = terminal.encode_key(&KeyEvent::new(Key::Enter, Modifiers::default()));
assert_eq!(bytes, b"\r");
for event in terminal.drain_events() {
    if let TerminalEvent::PtyWrite(reply) = event {
        // Send reply to the same application that supplied terminal output.
        let _ = reply;
    }
}
```

`write(&[u8])` accepts arbitrary chunks and retains incomplete UTF-8 and escape
sequences. `resize(cols, rows)` reflows wrapped text. Dimensions are clamped to
two columns and one row. `reset()` clears parser state, screen, history, modes
and selection, preserving the configured theme. `scroll(lines)` moves towards
older history for positive values; `scroll_to_bottom()` resumes the live view.

`snapshot()` returns shared `Arc<Row>` values. Only damaged rows are recomputed;
unchanged rows keep their allocation. Each row contains style runs with a start
column, occupied cell width, full grapheme text, resolved colors and attributes.
Wide graphemes have their own two-cell runs; spacer cells are omitted. Snapshots
also include cursor position, shape, visibility and blink state, viewport offset,
and selection ranges. `damaged_rows` is relative to the previous snapshot.

Use `start_selection(point, kind)` and `update_selection(point)` with zero-based
viewport cells. Kinds are simple, word and line. `selection_text()` joins soft
wrapped lines, preserves hard newlines and omits wide-cell spacers. New output,
reset and resize clear the selection. `link_at(point)` detects OSC 8 links, URLs
and `path:line[:column]` links, including visible wrapped lines. OSC 8 takes
precedence. File positions use a nonzero `u32` line and an optional `u32` column.

`encode_key`, `encode_mouse`, `paste` and `focus` use the current application
modes. Keys support xterm application cursor/keypad modes, modifiers,
modifyOtherKeys and kitty's progressive keyboard flags. Hosts supply repeat and
release events and optional alternate-key/text information. Mouse reporting
supports X10, normal, button motion, any motion, SGR 1006 and UTF-8 coordinates.
Paste strips ESC and uses bracketed paste when requested; ordinary paste maps
newlines to carriage returns. Focus reports are emitted only when enabled.

Events are PTY reply bytes, title, bell and OSC 52 clipboard-store requests.
Clipboard reads are intentionally not serviced. Hosts decide whether to grant
store requests. Drain events after writes. No event opens a link or accesses a
system clipboard itself.

`TerminalTheme` accepts xterm palette keys (`black` through `brightWhite`) and
optional background, foreground, cursor and selection colors. Colors resolve to
CSS strings; missing defaults reference `--terminal-background`,
`--terminal-foreground`, `--terminal-cursor`, and `--terminal-selection`, with
fallback colors. Indexed colors 16–255 and truecolor SGR are supported, as are
OSC palette changes. Headless color queries use RGB hex colors or concrete
fallbacks when a CSS variable cannot be resolved.

| Target | Support |
| --- | --- |
| Native Rust | Model, snapshots, selection, links and encoders |
| wasm32-unknown-unknown | Same headless API |

Grapheme clustering (DEC mode 2027) is off by default for conventional wcwidth
applications and follows application DECSET/DECRST requests. The parser is
`rio-vt =0.5.28`, confined to a private adapter; its types are absent from this
crate's public API. Graphics (sixel, kitty images, iTerm images), audio,
notifications and shell integration UI are not rendered. Input encoding covers
text, navigation, F1–F35 and numeric keypad keys; media and standalone modifier
keys are not exposed. Link detection cannot reconstruct a wrapped prefix that
has scrolled out of the visible viewport.
