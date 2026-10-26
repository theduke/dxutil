# Rust editor design

`dxeditor` edits component documents in Rust. The platform-neutral engine operates
on the existing `ComponentDocumentV2` block schema; the browser adapter owns the
contenteditable DOM through `web-sys`. Dioxus renders the host, contextual surfaces,
and application islands. There is no JavaScript editing engine or message bridge.

## Profiles and persisted formats

All browser editors use the same engine:

- `ComponentEditor` uses the block-document catalog or the restricted graph-node catalog.
  Its public handle invokes Rust commands directly. The stored
  `semantic.block-document` v2 document shape is unchanged.
- `Editor` and `MarkdownEditor` decode the selected standard document format and
  project it through `engine::markdown_blocks` to block containers. Saving reverses
  that transform and uses the existing format codec. Markdown remains a string.
  Opaque Markdown source survives neighboring edits. Inline paragraph images split
  into before/image/after blocks; images inside inline-only table cells are inert
  opaque atoms and restore their exact standard node on save.
- `profiles::composer::ComposerEditor` maps text/chip segments to paragraph blocks
  and chip atoms. Its `ComposerValue`, chip payloads and host events remain compatible.

The notes transform preserves original codec identities. A namespaced metadata
sidecar stores shallow list/quote/paragraph/row shells that flattening would
otherwise discard. The inverse uses current edited inline content, restores the
shell identities, and removes the sidecar. Source metadata is retained, including
an existing value under the sidecar key.

The full editing UI targets browser wasm32 builds. Native builds render block
documents with the Rust read-only renderer and an editing notice, while notes and
composer use textarea fallbacks. The engine itself compiles and tests natively.

## Module ownership

| Module | Responsibility |
| --- | --- |
| `engine/model`, `selection`, `schema`, `profile` | Block lookup, inline units, selections and catalog/profile capabilities |
| `engine/transaction`, `history`, `identity` | Invertible block operations, bounded grouped history, UUIDv7 repair and copied-island identity rekeying |
| `engine/text`, `commands` | Text, block, table, media and island commands |
| `engine/keymap`, `input_types`, `input_rules`, `suggestion` | Shortcut precedence, beforeinput translation, Markdown rules, cancellable trigger state |
| `engine/clipboard`, `html`, `markdown_blocks` | Clipboard envelopes, safe HTML, TSV and persisted Markdown projection |
| `engine/ui_state` | Platform-neutral contextual state |
| `dom` | Keyed rendering, DOM/model mapping, selection, input/composition, clipboard/drop, geometry, media and island adoption |
| `ui` | Dioxus format/link/block/table/image/entity surfaces and pure placement math |
| `component_editor`, `component`, `profiles/composer` | Public components, controlled values, host callbacks and lifecycle |
| `islands`, `fallback`, `render_v2` | Dioxus island roots, native fallback and retained read-only rendering |

Engine modules do not import Dioxus, web-sys, js-sys or wasm-bindgen. The browser
boundary is `EngineInput`/`Command` in and `EngineOutput` out. Host providers run as
Rust futures, with generation checks discarding stale suggestion and preview
responses. Public host events are direct Rust values, without protocol sessions.

## Transactions and host updates

Commands produce invertible operations over blocks or their content. History
stores operations, rather than full document snapshots. Adjacent typing and
deleting transactions group within the configured window; structural commands and
input rules retain separate undo entries. Identity repair and caret-safe
normalization belong to the same transaction. Transient upload attributes and
collapsed state stay outside persisted snapshots.

Every local document change validates and synchronously emits `ComponentChange`
with the document, revision and dirty block IDs. Invalid changes remain live but
call `on_invalid_change` instead of reporting a valid save. No extra change is
emitted on blur or unmount. `snapshot_document` reads live Rust state immediately.
An indexed block lookup avoids scanning the document during ordinary typing.
Text-only changes can reuse a previously validated structure when their exact
operation preserves its schema and limits; other changes run full validation.
The host still receives one owned document snapshot for every valid change.
Controlled external revisions acknowledge matching local state, replace clean
state, or report a conflict when pending local edits differ. Explicit replacement
uses `Reset` or `Preserve` history and emits no local change. `Preserve` folds the
external replacement into the preceding history entry, so undo/redo stays valid
even when block identities or structure change; `Reset` clears history.

Handles become inert on unmount. The browser adapter releases listeners,
observers and adopted roots when destroyed. Browser callbacks restore the captured
Dioxus runtime and component scope before publishing signals or spawning tasks.

## Browser input, rendering and selection

The adapter handles cancellable beforeinput events as engine commands. Native
composition remains in the active inline container: no rendering touches that
container until composition ends. Read-back computes a changed text interval,
preserves surrounding marks/atoms, and commits one edit. Target ranges support
replacement text and browser word/line deletes. A MutationObserver reconciles
unexpected simple text edits and restores unsupported DOM mutations.

Model offsets count Unicode scalars and inline atoms; DOM offsets are UTF-16.
Conversion is confined to `dom/mapping`. Grapheme-aware deletion uses Unicode
segmentation. Text, whole-block, atomic-node and rectangular-cell selections share
one model selection type. DOM selection writes avoid echoes; non-text selections
use `data-selected` styling.

Blocks reconcile by stable UID. An unchanged inline mark structure can update its
text node directly. Empty containers have a caret anchor, and inline atoms have
zero-width boundary anchors. The adapter owns the editable subtree; Dioxus never
renders editable text into it.
Collapsed carets at text-node ends use an equivalent parent boundary, preserving
the native range during text updates and avoiding redundant selection writes.
When that boundary has no browser rectangle, geometry measures an equivalent
detached text-node range without moving the native selection. Empty containers
fall back to their block rectangle.

Dioxus roots park outside the editable subtree and are adopted into island
placeholders. Roots already in place are left untouched so focused inputs and
open dialogs survive attribute saves. Removed island state has a 250 ms grace
period for moves/undo. Islands isolate their input from the outer editor.

Clipboard copy writes the validated internal envelope, HTML and plain text. File
clipboard data creates upload placeholders and host requests. Textual paste
prefers validated internal data, then safe parsed HTML and TSV/Markdown/plain text.
Internal pasted identities are refreshed. Tables keep widths in the
model; rectangular paste and structural actions form one undoable transaction.

## Contextual UI and accessibility

One `EditorUiState` feeds Dioxus surfaces inside the overlay container. DOM
geometry converts client rectangles into wrapper-local coordinates, including
independent X/Y scale factors and visual-viewport bounds. Pure placement math
flips, shifts and clamps surfaces. Mobile suggestion lists use a bottom sheet.

The editable root has a label, `role=textbox`, and `aria-multiline`. Suggestions
use listbox/option roles and `aria-activedescendant`, retaining editor focus.
Checkboxes and disclosure buttons keep native semantics. Controls have accessible
names, visible focus and touch-sized targets. Status/errors use a polite live
region. Read-only mode disables mutations.
Toolbars use roving tabindex and arrow-key navigation. Focus in the editor's own
contextual surfaces retains its selection; adopted islands keep independent focus.
Dynamic search and URL fields receive explicit focus when mounted.

## Validation

Run from the Nix `ui` devshell when Nix is available:

```sh
make test-editor-native test-editor-wasm test-editor-e2e
make check-web check-desktop
cargo fmt --all
```

The wasm target runs `wasm-bindgen-test` with headless Chromium/chromedriver. Its
runner currently rejects Cargo's forwarded `--quiet`, so only that target omits
the flag and retains short compiler diagnostics. Native tests cover engine
commands, history, Markdown fidelity, clipboard policy and positioning.

`dxeditor_harness` mounts the real public components at `/blocks`, `/graph-node`,
`/notes`, `/composer`, `/islands` and `/perf`. Playwright projects cover Chromium,
Firefox and WebKit. Test-only scripts exercise browser input, clipboard,
composition through CDP, contextual surfaces, adopted islands and axe checks.
Performance cases attach mount and 30-keystroke p95 measurements for 2k/10k blocks
with 50 islands. Test scripts and generated wasm binding glue are distinct from
editor implementation code.

Actual validation results and environment limitations belong in
`docs/plans/2026-10-25-oxidize-editor/validation.md`.

## Deliberate limitations and device follow-up

The approved feature cuts are the URL-paste choice menu, margin rubber-band
selection, searchable table actions (the plain menu retains the actions), selected
text drag/drop, draggable table column resizing, and image resize handles. Image
width presets remain available in the image menu. These features can be
reconsidered independently without changing persisted formats.

**TODO(android-builds):** when Android builds are added, run real-device GBoard
composition in paragraph/list/table/caption/composer, autocorrect replacement,
Enter/Backspace during composition, bottom-sheet placement with the keyboard,
long-press selection and pinch zoom. Add an iOS Safari pass for autocorrect,
Shift+Enter and visual-viewport docking. Automated composition tests run now;
Android/iOS device verification is explicitly deferred.
