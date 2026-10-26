# dxutil playground

One component browser for `dxcomp`, `dxeditor`, `dxgraph`, `dxform`, `dxterm`,
and `dxhighlight`. Stories use the real public components with local, disposable
state. Search by component or crate, change controls, inspect Rust source,
switch themes, constrain the preview width, or reset a story.

## Run

From the workspace root, enter `nix develop .#ui` when Nix is installed, then:

```sh
make serve-playground
```

Open the URL printed by Dioxus, under `/dxutil/`. Story links use
`/dxutil/?story=graph`, so refresh and browser back/forward work on a static
host without a catch-all route or custom 404 page. Dioxus CLI 0.7.9 and the
WASM target are required; see the [workspace setup](../../README.md#development).

For a desktop window using Dioxus's native **webview** renderer:

```sh
make serve-playground-desktop
```

This app does not target the separate `dioxus-native` renderer.
Several dxcomp interactions require a DOM and JavaScript
evaluation. The app itself uses Dioxus APIs rather than direct browser bindings.
On web WASM, dxeditor uses its rich-text DOM implementation; on desktop,
Markdown and the composer use the crate's textarea fallbacks. GTK 3,
WebKitGTK 4.1, libxdo, librsvg, OpenSSL, and pkg-config are needed for Linux
desktop builds.

## Stories

| Crate | Coverage |
| --- | --- |
| dxcomp | Six button variants and four sizes; disabled state; controlled input, switch, slider and progress; modal dismissal/confirmation; inline Spinner and centered BlockSpinner with size and accessible label controls; 10,000-row virtual list with jump and selection |
| dxeditor | Controlled Markdown notes and replacement; structured message composer and submission events |
| dxgraph | Tree, mind map, radial and force layouts; node insertion, selection, dragging, fit, zoom, controls and minimap |
| dxform | Typed fields; blur/submit validation; error messages; dirty/touched/validity metadata; submission and reset |
| dxterm | ANSI sample output, local echo, reset, font size, resize, selection and link event feedback |
| dxhighlight | Read-only Rust/TypeScript/JSON highlighting and language detection; selection callbacks; line navigation |

The browser is a curated set of integration examples, not an exhaustive catalog.
It does not cover every dxcomp primitive, structured block document editing,
suggestions/uploads, nested or asynchronous forms, graph connection editing,
highlight token APIs, or large-file performance. The terminal is not a shell:
it executes nothing and has no PTY transport. Form submission updates local state only. Navigating away unmounts a story and discards its state.
Each crate has its own module under `src/stories/`.

The current Markdown importer preserves some inline formatting inside list
items as opaque source fragments. The samples keep formatted paragraphs
separate from lists.

## Build and test

```sh
make check-playground check-playground-desktop
make build-playground
npm ci --prefix crates/playground/e2e
cd crates/playground/e2e
npx playwright install --with-deps chromium
npm test
```

`make test-playground` builds the release app and runs the browser suite.
`make install-js test-js` also installs and type-checks these tests alongside
the existing suites. Browser tests exercise routes and refresh under the real
`/dxutil/` prefix, navigation history, controls, modal keyboard dismissal,
virtualization, Markdown replacement, message submission, graph selection,
form validation, bundled editor/terminal engines, mobile overflow, and keyboard
skip navigation. Desktop is compile-checked in the existing feature CI matrix;
these browser tests do not establish desktop runtime behavior.

The production artifact is `target/dx/dxplayground/release/web/public/`.
The release target omits DWARF debug sections and retains function names for
readable panic traces.
To inspect it locally without Dioxus's development server:

```sh
DXPLAYGROUND_DIR="$PWD/target/dx/dxplayground/release/web/public" \
  python3 crates/playground/e2e/serve.py
# Open http://127.0.0.1:4174/dxutil/
```

## GitHub Pages

The [CI workflow](../../.github/workflows/ci.yml) builds and tests the release
artifact on pull requests and main. After all required checks pass on main,
it uploads that artifact to GitHub Pages and deploys it to
**https://theduke.github.io/dxutil/**. This is the configured deployment address;
a successful main workflow deployment is required before it serves the app.
Manual CI runs can deploy only when run against main.

Repository Settings → Pages must use **GitHub Actions** as the publishing source.
The deployment job uses the `github-pages` environment and grants only
`pages: write` and `id-token: write`; build jobs have read-only repository access.
If environment protection rules are added, allow deployments from main.
`Dioxus.toml` fixes the build base path to `dxutil`, including generated assets.
For another repository or hosting path, update that setting and the browser
test server/base URL together. No generated site is committed to the repository.

The deployment setup follows the upstream [Dioxus base-path documentation](https://dioxuslabs.com/learn/0.7/guides/tools/configure/)
and [GitHub Pages custom workflow requirements](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages).

The **Form fields** story demonstrates dxcomp's optional dxform bindings: string
and numeric drafts, password visibility, typed toggles and selects, submit/reset
actions, and a table row that supplies its own label and field context. The
**Validation & submission** story remains a headless dxform example using HTML.
