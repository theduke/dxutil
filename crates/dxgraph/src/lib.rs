//! Content-agnostic graph models, geometry, layouts, and Dioxus rendering.
use dioxus::prelude::*;
pub mod canvas;
pub use canvas::use_node_ready;
pub use canvas::{
    Activation, CanvasConfig, ConnectEvent, ConnectHandle, DropAxis, EdgeRenderContext, FitInsets,
    GraphCanvas, GraphCanvasProps, GraphController, GraphControls, GraphMinimap, Guide,
    LayoutArrangement, NodeDetail, NodeEvent, NodeRenderContext, PositionSource, SavedView,
    StructuralDropIndicator, StructuralDropTarget, marquee_hits, snap, structural_drop_target,
    use_graph_controller, use_screen_anchor,
};
pub use interaction::{NoDrag, NoWheel};
pub mod edge;
pub mod geometry;
pub mod interaction;
pub mod layout;
pub mod model;
pub use edge::{
    BusySides, EdgeAnchor, EdgeGeometry, EdgeMarker, EdgePathStyle, EdgeRoute, EdgeSide, EdgeStyle,
    edge_geometry, edge_geometry_with_offset, edge_sides, floating_anchor, route_around,
};
pub use geometry::{Point, Rect, Size, Viewport, ViewportLimits};
pub use layout::{
    BranchSide, LayoutAlgorithm, LayoutConfig, LayoutEdge, LayoutInput, LayoutNode, LayoutOutput,
    run_layout,
};
pub use model::{EdgeId, GraphEdge, GraphError, GraphModel, GraphNode, NodeId};

pub const CSS: &str = include_str!("dxgraph.css");
#[component]
pub fn Stylesheet() -> Element {
    rsx! {
        style { {CSS} }
    }
}
