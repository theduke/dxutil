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

`dxcode` and `dxterm` embed committed JavaScript bundles. After changing their web
sources, run `make test-js build-js` and commit the regenerated `web/dist/` files.
Do not add shared abstractions until there are concrete consumers.
