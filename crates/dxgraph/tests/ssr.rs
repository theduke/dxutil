use dioxus::prelude::*;
use dxgraph::*;

fn graph() -> GraphModel<String, ()> {
    let mut graph = GraphModel::default();
    for id in ["root", "a", "b"] {
        graph
            .insert_node(GraphNode::new(id, id.to_string()))
            .unwrap();
    }
    graph
        .insert_edge(GraphEdge::new("ra", "root", "a", ()))
        .unwrap();
    graph
        .insert_edge(GraphEdge::new("rb", "root", "b", ()))
        .unwrap();
    graph
}
fn canvas() -> Element {
    let model = use_signal(graph);
    let controller = use_graph_controller();
    use_hook(move || {
        controller.set_viewport(Viewport {
            x: 11.0,
            y: 22.0,
            zoom: 0.8,
        })
    });
    rsx! {
        GraphCanvas {
            model,
            controller,
            render_node: move |node: NodeRenderContext<String>| rsx! {
                div { {node.data} }
            },
            node_label: move |node: NodeRenderContext<String>| format!("Node {}", node.id),
        }
    }
}
fn controls() -> Element {
    let controller = use_graph_controller();
    rsx! {
        GraphControls { controller }
    }
}
#[test]
fn canvas_has_nodes_paths_markers_transform_and_accessibility() {
    let mut dom = VirtualDom::new(canvas);
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);
    assert_eq!(html.matches("data-dxgraph-node=").count(), 3, "{html}");
    assert_eq!(html.matches("data-draggable=\"true\"").count(), 3, "{html}");
    assert!(html.contains("dxgraph-arrow-"));
    assert_eq!(
        html.matches("<path ").count(),
        3,
        "one marker plus one path per edge"
    );
    assert!(html.contains("data-dxgraph-edge=\"ra\""));
    assert!(html.contains("translate(11px,22px) scale(0.8)"));
    assert!(html.contains("role=\"application\""));
    assert!(html.contains("aria-label=\"Node root\""));
    assert!(html.contains("tabindex=0"), "{html}");
}
#[test]
fn controls_are_accessible() {
    let mut dom = VirtualDom::new(controls);
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);
    for label in ["Zoom in", "Zoom out", "Fit graph", "Re-layout graph"] {
        assert!(html.contains(label));
    }
}

struct CustomLayout;
impl LayoutAlgorithm for CustomLayout {
    fn layout(&self, input: &LayoutInput) -> LayoutOutput {
        LayoutOutput {
            positions: input
                .nodes
                .iter()
                .map(|node| (node.id.clone(), Point::new(123.0, 456.0)))
                .collect(),
        }
    }
}
fn custom_canvas() -> Element {
    let model = use_signal(graph);
    let layout = LayoutConfig::Custom(std::rc::Rc::new(CustomLayout));
    rsx! {
        GraphCanvas {
            model,
            layout,
            render_node: |node: NodeRenderContext<String>| rsx! {
                div { {node.data} }
            },
            node_label: |node: NodeRenderContext<String>| node.data,
        }
    }
}
#[test]
fn canvas_accepts_a_custom_layout_engine() {
    let mut dom = VirtualDom::new(custom_canvas);
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);
    assert_eq!(html.matches("transform:translate(123px,456px)").count(), 3);
}

fn editing_canvas() -> Element {
    let model = use_signal(graph);
    let controller = use_graph_controller();
    use_hook(move || {
        controller.set_selected_edge(Some("ra".into()));
    });
    rsx! {
        GraphCanvas {
            model, controller,
            config: CanvasConfig {activation:Activation::OnFocus,interactive_edges:true,..Default::default()},
            render_node: |node:NodeRenderContext<String>|rsx! {div {{node.data} ConnectHandle {node:node.id,side:EdgeSide::Right}}},
            node_label: |node:NodeRenderContext<String>|node.data,
            render_edge_label: |edge:EdgeRenderContext<()>|rsx! {span {"Label {edge.id}"}},
            GraphMinimap {controller}
        }
    }
}
#[test]
fn editing_options_render_ports_hit_paths_html_labels_and_minimap() {
    let mut dom = VirtualDom::new(editing_canvas);
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);
    assert!(html.contains("touch-action: pan-y"), "{html}");
    assert_eq!(html.matches("dxgraph-edge-hit").count(), 2);
    assert!(html.contains("aria-hidden=\"false\""), "{html}");
    assert_eq!(html.matches("dxgraph-edge-pill").count(), 2);
    assert!(html.contains("data-selected=\"true\""), "{html}");
    assert_eq!(html.matches("dxgraph-connect-right").count(), 3);
    assert!(html.contains("dxgraph-minimap-viewport"));
    assert!(html.contains("Label ra"));
}
