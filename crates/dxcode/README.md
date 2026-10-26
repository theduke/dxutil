# dxcode

Tree-sitter highlighting in Rust and a read-only Dioxus code viewer. No editor,
JavaScript bundle, Node build step or runtime grammar download is required.

## Use

```toml
[dependencies]
dioxus = { version = "0.7.9", features = ["web"] }
dxcode = { git = "https://github.com/theduke/dxutil", features = ["web", "markdown"] }
```

```rust
use dioxus::prelude::*;
use dxcode::{CodeView, LineTarget};

fn Example() -> Element {
    rsx! {
        div { style: "height: 400px",
            CodeView {
                value: "fn main() {}\n".to_string(),
                path: "main.rs".to_string(),
                goto: LineTarget { line: 1, column: None },
                line_numbers: true,
            }
        }
    }
}
```

`value` is immutable viewer input. `path` selects an enabled grammar by extension
or file name. Unknown and disabled languages render as plain text. `goto` uses
one-based line/Unicode scalar column numbers. It marks the target row, renders
its viewport, and requests scrolling/focus through the renderer. The column is
exposed as `data-column`; horizontal column positioning is not implemented.

The default viewport is 120 fixed-height rows plus overscan. Scroll events adapt
the window to the visible height. Give the viewer a bounded parent height and
keep rows at 20 px without wrapping. `viewport_lines: Some(n)` sets the initial
window; `None` renders all lines for SSR, export or renderers without scroll
events. Selection is limited to currently rendered lines. With `dom`,
`on_selection` reports selected text and one-based line bounds on mouse release
or keyboard selection; line numbers are excluded.

## Highlight API

The core has no Dioxus dependency. For a core-only consumer, disable defaults and
select grammar features explicitly:

```toml
dxcode = { git = "https://github.com/theduke/dxutil", default-features = false, features = ["rust", "json"] }
```

```rust
use dxcode::{HighlightSession, TextEdit, highlight_lines};

let rows = highlight_lines("rust", &["let café = 42;"])?;
let mut session = HighlightSession::new("rust", "let café = 42;\n")?;
let viewport = session.highlight(0..14)?;
let changed = session.apply_edit(TextEdit {
    range: 12..14,
    replacement: "99".into(),
})?;
for range in changed {
    let updated = session.highlight(range)?;
}
# Ok::<(), dxcode::CodeError>(())
```

`highlight_lines` is synchronous and parses the supplied lines together, retaining
multiline context. It returns `TokenSpan { start, end, class }` with half-open
Unicode scalar (**char**) offsets within each line, excluding a trailing CR.
Provide lines without LF terminators; a trailing CR for CRLF input is accepted.
It has no fixed line limit. An empty list returns an empty list.

`HighlightSession` stores UTF-8 text and an incremental tree. `TextEdit.range`,
`HighlightSpan.range`, requested viewports and returned changed ranges all use
half-open UTF-8 **byte** offsets, not char or UTF-16 offsets. Invalid ranges and
non-char boundaries return `CodeError::InvalidRange`. CRLF is preserved in session
text; tree-sitter columns count bytes since the preceding LF. Markdown parser
input receives a private terminal LF to handle unterminated final lines; it is
excluded from public text and output offsets.

`apply_edit` reparses with the edited old tree. Invalidation unions structural
changed ranges, the edit, dependent syntax ancestors and enclosing injections.
Returned ranges refer to the new text. Consumers maintaining a token cache must
also remove old edited spans and shift subsequent offsets by the edit's byte
delta. `highlight(range)` runs queries only over overlapping captures and clips
the output to the viewport. `highlight_all()` is convenient for small documents.

Queries are compiled lazily once per language **per thread**, including failures.
Sessions can move between threads. The small internal highlight call is independent of
the viewer. It uses upstream `QueryCursor::set_byte_range`, rather than
`tree-sitter-highlight`, to reuse trees and limit captures.

## Features and build requirements

Defaults: **`viewer`, `rust`, `json`**. This keeps the common Rust/config use case
small; additional grammars increase download and compile costs.

| Feature | Language / aliases |
| --- | --- |
| `rust` | Rust, `rs` |
| `javascript` | JavaScript/JSX, `js`, `jsx`, `node` |
| `typescript` | TypeScript, `ts` |
| `tsx` | TSX |
| `json` | JSON, `jsonc` (JSON grammar; comments may be error nodes) |
| `toml` | TOML, including `Cargo.lock` |
| `yaml` | YAML, `yml` |
| `markdown` | Markdown block **and inline**, `md`, `commonmark` |
| `html` | HTML, `htm` |
| `css` | CSS |
| `python` | Python, `py`, `python3` |
| `go` | Go, `golang` |
| `bash` | Bash/shell, `sh`, `shell`, `zsh`; shell startup files |
| `sql` | SQL |
| `c` | C (`.c`, `.h`) |
| `cpp` | C++, `c++`, `cc`, `cxx` |
| `all-languages` | All languages above |
| `viewer` | RSX component, no specific renderer |
| `dom` | Optional DOM selection through Dioxus eval |
| `web` | `dom` and Dioxus web |
| `desktop` | `dom` and Dioxus desktop (wry webview) |

Name, alias and path lookup only return enabled languages. Markdown fences resolve
all enabled aliases, including `language-rust`; unknown injected grammars remain
plain. HTML script/style injection requires `javascript`/`css` respectively.
Language features do not implicitly enable other injectable languages.

Stable Rust **1.90+** and a C compiler are required. Grammars are linked statically
on every target. All grammar crates use Arborium 2.18.2 with upstream tree-sitter
0.27.1, except Markdown: upstream `tree-sitter-md` supplies matching block and
inline grammars because Arborium has no upstream-compatible inline crate.
Neither Arborium's umbrella engine nor its forked core is used.

For `wasm32-unknown-unknown`, use a clang with the WebAssembly backend. LLVM clang
18 works; **Apple clang lacks this backend**. On macOS, install Homebrew `llvm`:

```sh
brew install llvm
export CC_wasm32_unknown_unknown="$(brew --prefix llvm)/bin/clang"
export AR_wasm32_unknown_unknown="$(brew --prefix llvm)/bin/llvm-ar"
# Linux, for example:
export CC_wasm32_unknown_unknown=clang-18
cargo check --target wasm32-unknown-unknown --features dxcode/web --locked
```

The Nix `ui` shell includes LLVM clang and configures these target variables.

## Styling and platforms

The viewer includes its stylesheet. Light colors are the default;
`html[data-theme=dark]` selects dark colors. Override `--code-text`,
`--code-background`, `--code-gutter`, `--code-muted`, `--code-border`,
`--code-active-line`, `--code-selection`, `--code-keyword`, `--code-string`,
`--code-number`, `--code-comment`, `--code-type`, `--code-name`,
`--code-punctuation`, and `--code-invalid` on a parent or the viewer.

Stable token classes are `tok-keyword`, `tok-operator`, `tok-string`,
`tok-number`, `tok-comment`, `tok-type`, `tok-name`, `tok-punctuation`,
`tok-invalid`, `tok-heading`, `tok-emphasis`, and `tok-link`. More specific
captures map to this vocabulary; internal and unrecognized captures are ignored.
Numbers, booleans and constants share `tok-number`; variables, properties,
functions, tags and attributes share `tok-name`. Headings use the type color,
links the name color, and emphasis inherits text color.

| Renderer | Rendering / highlighting | Interactive extras | Verification |
| --- | --- | --- | --- |
| Dioxus web (browser wasm) | Rust core, RSX tokens and rows | Scroll viewport; mounted goto; DOM selection with `web` | Node WASM core and deferred-parse runtime tests; viewer browser interaction not yet verified |
| Dioxus desktop (wry webview) | Core runs in the **native host**, webview renders RSX | Scroll viewport; mounted goto; DOM selection with `desktop` | Compile check; interactive behavior not yet verified |
| Dioxus Native / Blitz | Plain RSX/classes/data attributes; Rust core | Viewport where scroll events work; goto where mounted APIs work; eval selection unavailable | Renderer runtime not yet verified |
| SSR | Tokens, numbers, target attributes, selected window or all lines | None | SSR tests |

Core rendering never calls eval or requires JavaScript. Optional selection uses
an isolated eval call; Unsupported errors are ignored. Unsupported mounted
scroll/focus operations are also ignored. Neither prevents rendering.

## Costs and limitations

Read-only only: no editing, change/save events, editing keymaps, wrapping, or
selection across unrendered rows. A value/path change creates a fresh parser
session. Above 128 KiB, the viewer first renders plain rows, then parses on a
native host thread or yields between bounded tree-sitter parse steps on wasm.
Small documents parse synchronously for immediate SSR tokens. Query compilation
is synchronous and cached; very long injected blocks can still pause the browser.
Large SSR inputs initially render plain text; use the core API to pre-highlight
exports when needed. Injection parsers are
recreated for each intersecting region; nested injection depth is bounded at 8.
Highlight quality depends on upstream queries. Scope-aware locals and arbitrary
custom query directives are not interpreted; text predicates are handled by
tree-sitter. Injections currently parse each captured region independently.

Run `make measure-code-sizes` for a reproducible core-only wasm probe (release
`opt-level=s`, LTO, one codegen unit, abort panic; `wasm-opt -Oz`; gzip -9).
Set `WASM_OPT` and `CC_wasm32_unknown_unknown` to explicit tool paths as needed.
The default row includes default viewer features, but the probe calls only the
core, so unused viewer code is stripped. These are not whole application sizes.

Measured on Linux x86_64 with Rust 1.99.1, LLVM clang 18 and Binaryen 133;
bytes, including gzip headers:

| Features | Raw wasm | wasm-opt | Gzip |
| --- | ---: | ---: | ---: |
| none | 125,264 | 56,981 | 25,513 |
| rust | 3,266,955 | 2,232,592 | 612,674 |
| default | 3,798,072 | 2,633,435 | 714,201 |
| all-languages | 25,696,003 | 19,225,077 | 2,782,263 |

Native release core builds in a fresh target directory took roughly 36.5 s for
the grammar-free core/dependencies, another 7.6 s for Rust+JSON, then 19.5 s for
the remaining grammars. These sequential builds reuse earlier dependencies and
exclude the viewer; application build times depend on existing Dioxus artifacts.
The compiler wrappers were bypassed for these native timings.

Grammar and runtime license texts are included in [NOTICE.md](NOTICE.md) and
[licenses/](licenses/). All selected grammar licenses are MIT.
