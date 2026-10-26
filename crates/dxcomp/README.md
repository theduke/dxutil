# dxcomp

Prestyled Dioxus components, based on `DioxusLabs/dioxus-components` and bundled
as a reusable Rust library. It includes buttons and inputs, dialogs and menus,
layout components, date/color pickers, and virtual lists. The upstream primitive
API is also available through `dxcomp::primitives`.

## Use

Until a crates.io release is available, use the repository dependency:

```toml
[dependencies]
dioxus = { version = "0.7.9", features = ["web"] }
dxcomp = { git = "https://github.com/theduke/dxutil", features = ["web"] }
```

Render `Stylesheet` once near your app root:

```rust
use dioxus::prelude::*;
use dxcomp::button::{Button, ButtonVariant};

fn App() -> Element {
    let mut count = use_signal(|| 0);
    rsx! {
        dxcomp::Stylesheet {}
        Button {
            variant: ButtonVariant::Outline,
            onclick: move |_| count += 1,
            "Clicked {count} times"
        }
    }
}
```

Component modules follow names such as `dxcomp::button`, `dxcomp::dialog`, and
`dxcomp::virtual_list`. `CSS` exposes the entire stylesheet for hosts that manage
style injection themselves. `ThemeStylesheet` and `THEME_CSS` are compatibility
names; use `Stylesheet` and `CSS` for new integrations.

The theme follows `prefers-color-scheme` unless the HTML element sets
`data-theme="light"` or `data-theme="dark"`. Override the shared `--dx-*` and
component CSS variables in your app stylesheet to customize colors and geometry.

## Features and runtime

There are no default crate features. Select the feature matching your Dioxus
renderer:

| Feature | Enables |
| --- | --- |
| `web` | Dioxus web renderer and web primitives |
| `desktop` | Dioxus desktop renderer |
| `fullstack` | Dioxus fullstack support |
| `server` | Dioxus server and fullstack support |
| `native` | Dioxus native renderer |

Interactive popup positioning, sidebar hooks, and virtual lists use Dioxus's
JavaScript evaluation bridge. They require a renderer that supports browser DOM
access; static rendering alone does not exercise those interactions. Floating
menus use the browser popover top layer to avoid ancestor clipping.

`VirtualList` accepts the upstream primitive's `VirtualListProps`. Give its
scrolling container a bounded height. For imperative scrolling, call
`use_virtual_list_scroller()` in an ancestor component; it provides the handle
through Dioxus context to descendant lists. Call `scroll_to(index)` on the handle.

## Development

From the workspace root, enter `nix develop .#ui` when Nix is available:

```sh
cargo check -p dxcomp --all-targets --locked --quiet --message-format=short
cargo test -p dxcomp --locked --quiet --message-format=short
make check-wasm check-desktop
```

Changes to `dioxus-primitives` should keep its pinned Git revision deliberate.
The local virtualizer retains upstream MIT/Apache-2.0 notices in
[`src/components/virtual_list/`](src/components/virtual_list/); keep those notices
when modifying or redistributing it.
