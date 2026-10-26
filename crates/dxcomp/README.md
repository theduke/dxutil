# dxcomp

Styled Dioxus components: buttons and inputs, dialogs and menus, layout controls,
date and color pickers, and virtual lists.

## Use

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

Components are grouped in modules such as `button`, `dialog`, `combobox`, and
`virtual_list`. Lower-level primitives are available through `dxcomp::primitives`.

## Styling and interaction

The theme follows `prefers-color-scheme`. Set `data-theme="light"` or
`data-theme="dark"` on the HTML element to override it. Customize shared `--dx-*`
and component CSS variables in your app stylesheet. `dxcomp::CSS` exposes the
stylesheet for applications that manage style injection themselves.

Popups, sidebar hooks, and virtual lists require a DOM-capable renderer with
Dioxus JavaScript evaluation support. Static rendering produces markup without
those interactions.

Give `VirtualList` a scrolling container with a bounded height. For imperative
scrolling, call `use_virtual_list_scroller()` in an ancestor component, then call
`scroll_to(index)` on its returned handle. The hook provides the handle to
descendant lists through Dioxus context.

## Features

No features are enabled by default. Choose the feature matching your renderer:

| Feature | Enables |
| --- | --- |
| `web` | Dioxus web renderer and web primitives |
| `desktop` | Dioxus desktop renderer |
| `fullstack` | Fullstack support |
| `server` | Server and fullstack support |
| `native` | Dioxus native renderer |

## Development

See [workspace setup](../../README.md#development). Run crate tests from the root:

```sh
cargo test -p dxcomp --locked --quiet --message-format=short
```
