//! Native 300-node canvas smoke test. Run with `cargo run -p dxgraph --example demo --features demo`.
use dioxus::prelude::*;
use dxgraph::{
    Activation, CanvasConfig, ConnectHandle, EdgeSide, GraphCanvas, GraphControls, GraphEdge,
    GraphMinimap, GraphModel, GraphNode, NodeId, NodeRenderContext, Stylesheet,
    use_graph_controller,
};

fn main() {
    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            dioxus::desktop::Config::new().with_window(
                dioxus::desktop::WindowBuilder::new()
                    .with_title("dxgraph 300-node demo")
                    .with_inner_size(dioxus::desktop::LogicalSize::new(1280.0, 900.0)),
            ),
        )
        .launch(App);
}

#[allow(non_snake_case)]
fn App() -> Element {
    let mut model = use_signal(|| {
        let mut model = GraphModel::<usize, ()>::default();
        for index in 0..300 {
            let mut node = GraphNode::new(format!("node-{index:03}"), index);
            node.layout_parent =
                (index > 0).then(|| NodeId::from(format!("node-{:03}", (index - 1) / 2)));
            let parent = node.layout_parent.clone();
            let id = node.id.clone();
            model.upsert_node(node);
            if let Some(parent) = parent {
                model
                    .insert_edge(GraphEdge::new(format!("edge-{index:03}"), parent, id, ()))
                    .expect("the parent and child have both been inserted");
            }
        }
        model
    });
    let controller = use_graph_controller();
    let mut status = use_signal(|| {
        "Shift-drag selects; drag selected nodes together; Alt disables snap. Drag a port to connect.".to_owned()
    });
    rsx! {
        Stylesheet {}
        style {
            {
                "html,body,#main{margin:0;width:100%;height:100%;font-family:system-ui;}button{padding:8px;}"
            }
        }
        div { style: "width:100vw;height:100vh;",
            GraphCanvas::<usize,()> {
                model,
                controller,
                config: CanvasConfig {activation:Activation::OnFocus,interactive_edges:true,multi_drag:true,marquee_selection:true,animate_layout:true,snap_threshold:Some(6.0),snap_grid:Some(8.0),..Default::default()},
                on_nodes_moved: move |nodes: Vec<(NodeId,dxgraph::Point)>| {
                    for (id,position) in &nodes { model.write().set_position(id,*position).unwrap(); model.write().set_pinned(id,true).unwrap(); }
                    status.set(format!("Moved {} nodes",nodes.len()));
                },
                on_connect: move |connection: dxgraph::ConnectEvent| {let id=format!("manual-{}",model.peek().edges().count());model.write().insert_edge(GraphEdge::new(id,connection.source,connection.target,())).unwrap();status.set("Connection created".into());},
                on_connect_to_empty: move |(_,point): (NodeId,dxgraph::Point)|status.set(format!("Empty drop at {:.0}, {:.0}",point.x,point.y)),
                on_background_double_click: move |point:dxgraph::Point|status.set(format!("Double-click at {:.0}, {:.0}",point.x,point.y)),
                on_edge_click: move |id:dxgraph::EdgeId|status.set(format!("Selected edge {id}")),
                render_node: |context: NodeRenderContext<usize>| rsx! {
                    div { style: "box-sizing:border-box;width:160px;min-height:64px;padding:16px;background:#fff;border:1px solid #aaa;border-radius:6px;",
                        "Node {context.data}"
                        ConnectHandle {node:context.id.clone(),side:EdgeSide::Right}
                    }
                },
                node_label: |node: NodeRenderContext<usize>| format!("Node {}", node.data),
                div { style: "position:absolute;top:16px;left:16px;pointer-events:auto;display:flex;gap:8px;",
                    button {
                        onclick: move |_| {
                            let current = controller.viewport();
                            controller.zoom_by(1.0 / current.zoom);
                            controller.center_on("node-000".into());
                        },
                        "Focus root at 1x"
                    }
                    button { onclick: move |_| controller.fit_view(), "Fit 300 nodes" }
                }
                div {style:"position:absolute;top:70px;left:16px;max-width:520px;font-size:12px;padding:8px;background:#fffd;", "{status}"}
                GraphMinimap {controller}
                GraphControls { controller }
            }
        }
    }
}
