# Follow-up work

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
