# dxgraph

An interactive Dioxus graph canvas with custom node components, SVG edges,
selection, pan/zoom, and Rust layout algorithms.

## Use

```toml
[dependencies]
dioxus = { version = "0.7.9", features = ["web"] }
dxgraph = { git = "https://github.com/theduke/dxutil", features = ["web"] }
dxcomp = { git = "https://github.com/theduke/dxutil", features = ["web"] }
```

```rust
use dioxus::prelude::*;
use dxgraph::*;

fn App() -> Element {
    let model = use_signal(|| {
        let mut graph = GraphModel::<String, ()>::default();
        let _ = graph.insert_node(GraphNode::new("root", "Hello".into()));
        graph
    });
    let controller = use_graph_controller();
    rsx! {
        dxgraph::Stylesheet {}
        dxcomp::Stylesheet {}
        div { style: "height: 600px;",
            GraphCanvas {
                model,
                controller,
                render_node: |node: NodeRenderContext<String>| rsx! { div { {node.data} } },
                node_label: |node: NodeRenderContext<String>| node.data,
                GraphControls { controller }
            }
        }
    }
}
```

Give the container a definite height and provide an accessible `node_label`.
`dxgraph::Stylesheet` styles the canvas; `GraphControls` also needs the dxcomp
stylesheet. Wrap interactive node contents in `NoDrag` and scrollable contents in
`NoWheel` to keep their events from controlling the canvas.

## Layouts and navigation

Layouts include tidy trees, left/right mind maps, force-directed layouts, radial
rings, and manual positioning. Implement `LayoutAlgorithm` and use
`LayoutConfig::Custom` for a custom algorithm. Nodes can be pinned; disconnected
components and cyclic input graphs are supported.

`GraphController` exposes `fit_view`, `zoom_by`, `center_on`, `set_viewport`, and
selection operations. `GraphMinimap { controller }` adds pointer navigation.
Set `preserve_viewport` to retain the camera when remounting a canvas with the
same controller.

| Input | Action |
| --- | --- |
| Enter | Activate focused node |
| Space | Select focused node |
| Shift-click | Toggle node selection |
| Escape | Clear selection or cancel a gesture |
| `+` / `-` | Zoom |
| `0` | Fit graph |
| Arrow keys | Pan |

## Editing

Editing behaviors are opt-in through `CanvasConfig`:

```rust
use dxgraph::{Activation, CanvasConfig, PositionSource};
use dxgraph::interaction::WheelMode;

let config = CanvasConfig {
    position_source: PositionSource::ModelPinned,
    wheel_mode: WheelMode::ModifierZoom,
    activation: Activation::OnFocus,
    interactive_edges: true,
    multi_drag: true,
    marquee_selection: true,
    snap_threshold: Some(6.0),
    snap_grid: Some(8.0),
    ..Default::default()
};
```

Persist drag results through `on_node_moved` or `on_nodes_moved`. `ModelPinned`
responds to external position changes, including undo; the default keeps dragged
positions locally until `controller.reset_positions()` is called. For a view-only
canvas, set `draggable_nodes: false`.

`ModifierZoom` leaves ordinary wheel events to the page. `OnFocus` enables canvas
interaction after focus or a click. Shift-drag on the background selects nodes;
Alt bypasses snapping.

Add `ConnectHandle { node, side: EdgeSide::Right }` inside a node renderer to
create connections through `on_connect` and `on_connect_to_empty`.
`render_edge_label` renders custom edge labels, and `use_screen_anchor` positions
host toolbars relative to selected nodes. Models, IDs, geometry, and styles
support Serde serialization.

## Features and development

No features are enabled by default. `web` enables browser pointer capture, focus,
and geometry on WASM; enable the Dioxus web renderer in your app as well. Native
drags end when the pointer leaves the canvas. `demo` enables the desktop example:

```sh
cargo run -p dxgraph --example demo --features demo
cargo test -p dxgraph --locked --quiet --message-format=short
```

See [workspace setup](../../README.md#development) for platform dependencies.
Layout timing reports are available with
`cargo test -p dxgraph --test performance -- --ignored --nocapture`.
