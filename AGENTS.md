# Development

This workspace contains reusable Dioxus libraries copied from Semantic.
Keep them independent of application-specific crates. Preserve existing public
APIs and behavior when making maintenance changes.

When Nix is available, run checks and tests through `nix develop .#ui --command`.
Use `cargo check --quiet --message-format=short` and
`cargo test --quiet --message-format=short`, with `--locked`.
Run `cargo fmt --all` after changes. Use explicit `Result<T, E>` types.

After editor engine or DOM changes, run
`make test-editor-native test-editor-wasm test-editor-e2e`.
Keep browser implementation code in Rust under `crates/dxeditor/src/dom/` and
browser tests under `crates/dxeditor/e2e/`.

`dxterm_core` is headless Rust; keep rio-vt types in its private adapter.
`dxterm` shares DOM rows across web, desktop webviews and Native/Blitz. Keep JS
and unsupported composition/clipboard/mounted/pointer/wheel/resize handlers out
of Native paths. Use dxutil for shared IDs and clocks.
After terminal changes, run `make test-terminal-wasm check-wasm check-desktop check-native`.
Do not add shared abstractions until there are concrete consumers.

For dxhighlight changes, run `make test-code-native test-code-wasm measure-code-sizes`.
