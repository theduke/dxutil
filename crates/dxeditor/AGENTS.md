After engine or DOM changes, run `make test-editor-native test-editor-wasm test-editor-e2e`.
Use the Nix `ui` devshell when Nix is available. Browser test code belongs in
`e2e/`; the editor implementation belongs in Rust, with browser APIs in `src/dom/`.
