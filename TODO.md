# Follow-up work

- [ ] Remove the two quick-xml audit exceptions in `.cargo/audit.toml` when
  Dioxus Native's accessibility stack upgrades past `atspi-common 0.6` /
  `zbus-lockstep 0.4` / `zbus_xml 4`. These versions pin quick-xml 0.30, with
  no compatible patched release. Its XML parsing is confined to build-time
  macros and upstream signature tests over bundled protocol definitions.
  Recheck this reachability assumption whenever updating those dependencies.

- [ ] Replace the temporary `dxcomp-primitives` fork with upstream
  `dioxus-primitives` once a crates.io release provides the component APIs used
  by `dxcomp`. The current upstream `0.0.0` is a placeholder. Both
  `dxcomp-primitives` and its companion `dxcomp-attributes` were copied from
  `DioxusLabs/dioxus-components` revision
  `02801f27e4b3e30606e5c77e435c9d955c708eb3`. Compare upstream changes against
  that revision, restore the workspace registry dependency, preserve
  `dxcomp::primitives` and its attribute macro re-export, then remove both forks
  when no workspace consumers remain. Verify native and browser behavior before
  releasing the switch.

- [ ] Build and publish a prebuilt, separately loaded highlighter WebAssembly
  module running in a Web Worker. Keep a narrow request boundary for text,
  language, edits and viewport ranges; keep the current static Rust backend for
  native/desktop. This would remove wasm clang from consumer builds, keep grammar
  code outside the main app bundle, and move parse/query work off the UI thread.
  Prototype gzip costs: parser + JSON ~40 KB; queries/regex add ~360 KB fixed;
  Rust +150 KB; TS+TSX +370 KB; Python +70 KB; JS +60 KB; Markdown +70 KB;
  HTML/JSON +5–20 KB. Arborium Rust+JSON measured ~611 KB; the removed CodeMirror
  bundle was 919 KB raw / 322 KB gzip. Re-measure the shipping worker and define
  cancellation, incremental updates and worker lifetime before switching web.
