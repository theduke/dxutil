# Browser editor tests

The suite mounts the real Rust/Dioxus public editors through `dxeditor_harness`.
It has no JavaScript editor implementation. `helpers.ts` dispatches browser events
and inspects DOM/output solely for testing.

Use `nix develop .#ui --command make test-editor-native test-editor-wasm test-editor-e2e`.
The flake pins Playwright browsers to the same 1.60.0 version as `package.json` and
its `playwright-core` override. Nix browser RPATHs supply their shared libraries;
the shell skips Playwright's host-distribution metadata check and the suite still
launches and exercises every browser. The pinned package definition is
[upstream nixpkgs](https://github.com/NixOS/nixpkgs/blob/89570f24e97e614aa34aa9ab1c927b6578a43775/pkgs/development/web/playwright/driver.nix).

Without Nix, install Rust wasm32, Dioxus CLI and the matching wasm-bindgen runner,
then `npm ci` and `npx playwright install chromium firefox webkit`. Linux browser
shared libraries must be available. `PLAYWRIGHT_CHROMIUM_PATH` can override Chrome;
`DXEDITOR_HARNESS_DIR` can override the built web directory. The Python server
provides SPA fallback routes.

`npm run typecheck` checks the test scripts. `npx playwright test --project chromium`
selects one browser; the default runs all three. Chromium alone runs CDP composition
and performance cases. Performance JSON is attached to each Playwright test result;
reported budgets are assessed in the validation report against the actual machine.
