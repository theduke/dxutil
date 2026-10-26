# dxgraph

A content-agnostic Dioxus graph canvas. Nodes contain arbitrary HTML components;
edges are SVG paths. Layouts and the gesture reducer are pure Rust and work on
native and web targets.

## Use

```toml
[dependencies]
dioxus = { version = "0.7.9", features = ["web"] }
dxgraph = { git = "https://github.com/theduke/dxutil", features = ["web"] }
dxcomp = { git = "https://github.com/theduke/dxutil", features = ["web"] }
```

Render `dxgraph::Stylesheet` for canvas styles and `dxcomp::Stylesheet` when using
`GraphControls`, which contains dxcomp buttons. Give the canvas container a
definite height and provide an accessible `node_label` for every node. The graph
model does not depend on Semantic entities or a particular node component.


```rust,no_run
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
        Stylesheet {}
        dxcomp::Stylesheet {}
        div { style: "height:600px",
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

Dioxus/Rust owns the model, world positions, selection, and viewport. The
viewport signal is read by the world wrapper and dotted background, while node
and edge components receive viewport-independent props. Culling uses a margin
and 15% zoom hysteresis. Full, compact and minimal detail thresholds default to
0.6 and 0.3; minimal nodes use a placeholder instead of invoking the full renderer.

Measured unscaled Full-detail border-box sizes replace each node's `size_hint`.
Compact and Minimal measurements never change layout sizes. All valid Full sizes
update anchors and culling; changes within eight pixels do not trigger layout.
Resize bursts request at most one layout per queued tick. A 100 ms fallback reveals
nodes whose renderer does not provide resize notifications.

Wrap buttons, links, inputs and editors in `NoDrag` so they do not start gestures.
Wrap scrollable content in `NoWheel` to retain its wheel behavior. These wrappers
stop Dioxus events; no custom JavaScript is used. The `web` feature adds pointer
capture through web-sys; native drags end when the pointer leaves the canvas.

Layouts include a variable-size tidy tree (top-down or left-right), a balanced
left/right mind map, deterministic warm-started force layout with rectangle
collision removal, concentric radial rings, and manual positions. Implement
`LayoutAlgorithm` and pass `LayoutConfig::Custom(Rc::new(engine))` to supply
another engine. Custom engine identity is compared by `Rc` pointer. Fixed nodes
are preserved, disconnected components are packed, and
cycles become a spanning forest for tree layouts.

The canvas keeps dragged positions for its lifetime and emits `on_node_moved`.
The caller decides whether to persist the position and set the model node's
`pinned` flag. `controller.reset_positions()` clears canvas drag positions and
returns them to automatic layout. Explicit model pins remain caller-owned and fixed.
Incremental expansion retains existing positions. Calling
`controller.relayout()` or changing `layout` requests
a fresh layout, while user-moved and pinned nodes remain fixed.

Keyboard: Enter activates a focused node, Space selects, Escape clears selection,
`+`/`-` zoom, `0` fits, and arrow keys pan. Shift-click toggles selection.
`GraphController` also provides `fit_view`, `zoom_by`, `center_on` and
`set_viewport`. `container_size` reports measured screen-pixel dimensions for
host camera policies. Each node needs an accessible label through `node_label`.

GraphModel, node/edge IDs, geometry, and styles support Serde serialization. Future persisted canvas schemas can map their positions,
sizes, anchors and markers onto JSON Canvas without coupling dxgraph to entities.

Embedded editors can opt into editing without changing viewer defaults:

```rust,ignore
CanvasConfig {
    position_source: PositionSource::ModelPinned,
    wheel_mode: dxgraph::interaction::WheelMode::ModifierZoom,
    activation: Activation::OnFocus,
    interactive_edges: true,
    multi_drag: true,
    marquee_selection: true,
    animate_layout: true,
    snap_threshold: Some(6.0),
    snap_grid: Some(8.0),
    ..Default::default()
}
```

`ModelPinned` responds to undo and external pin changes immediately, and holds a
finished drag until the model acknowledges it. `on_nodes_moved` commits a group
in one host transaction; the existing `on_node_moved` also fires for each node.
`on_drag_move((nodes, pointer_world))` supports host drop targets.
Set `draggable_nodes: false` for view-only canvases: node clicks still select,
and dragging over a node pans the viewport without moving it.
`on_interaction_change(active)` reports pointer gesture transitions after movement
callbacks, allowing hosts to hide chrome while dragging, panning, or pinching. Shift-background
drag selects intersecting nodes; Alt bypasses snapping. `snap` and `marquee_hits`
are framework-independent helpers. Guides only consider visible unselected nodes.

`controller.set_selection`, `select_all`, `selection`, `set_selected_edge`,
`positions` and `rects` support host-owned editing. An optional `on_key_down`
callback runs before canvas keys; returning true consumes built-in handling.
Hosts that consume Escape can call `controller.cancel_interaction()` to restore
a dragged group and dismiss connection/marquee previews without committing.
`on_background_double_click` receives a world point. ModifierZoom leaves plain
wheel events to the page; Ctrl/Command wheel still zooms. OnFocus permits page
scrolling until focus or the first canvas click; blur deactivates it.

Put `ConnectHandle { node, side: EdgeSide::Right }` inside a node renderer.
The preview follows its port; release calls `on_connect((source, target))` or
`on_connect_to_empty((source, world_point))`. Escape cancels port and marquee
previews. Opt-in edge hit paths are twelve screen pixels wide, carry selection
attributes, and call `on_edge_click`/`on_edge_activate`. `render_edge_label`
receives an `EdgeRenderContext` and renders an HTML pill.

`GraphMinimap { controller }` provides pointer navigation. Use
`use_screen_anchor(controller, node_ids)` to anchor browser toolbars to selection
bounds. `animate_layout` and `controller.animate_to(viewport)` interpolate geometry
for 220 ms; browser reduced-motion preferences disable animation.
Set `preserve_viewport` when remounting a canvas with the same controller to retain
zoom and world center, including when the new container has a different size.
The default still fits content on each mount.

Native graph-block layout timing reports are available with
`cargo test -p dxgraph --test performance -- --ignored --nocapture` inside the
Nix devshell. The report covers 2,000-node tree/mind-map layouts and 500-node
Organic layout; timings depend on the build profile and hardware.

## Features and development

No features are enabled by default. `web` enables direct browser bindings for
pointer capture, focus, geometry, timing, and reduced-motion checks on WASM;
select the Dioxus web renderer in the consuming application too. `demo` enables
the Dioxus desktop renderer for the bundled example:

```sh
cargo run -p dxgraph --example demo --features demo
```

Desktop builds require the platform libraries described in the
[root development guide](../../README.md). From the workspace root, use
`nix develop .#ui` when Nix is available:

```sh
cargo check -p dxgraph --all-targets --locked --quiet --message-format=short
cargo test -p dxgraph --locked --quiet --message-format=short
make check-wasm check-desktop
```

Tests cover model/layout behavior, geometry, gestures, events, and static
rendering. The ignored timing test above is a hardware-dependent report and is
not part of the default test run.
