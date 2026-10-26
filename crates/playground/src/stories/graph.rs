use dioxus::prelude::*;
use dxcomp::button::{Button, ButtonVariant};
use dxgraph::{
    Activation, CanvasConfig, GraphCanvas, GraphControls, GraphEdge, GraphMinimap, GraphModel,
    GraphNode, LayoutConfig, NodeId, NodeRenderContext, use_graph_controller,
};

fn project_map() -> GraphModel<String, ()> {
    let mut graph = GraphModel::default();
    for (id, parent, title) in [
        ("project", None, "Your next project"),
        ("interface", Some("project"), "Interface"),
        ("data", Some("project"), "Data & state"),
        ("tools", Some("project"), "Developer tools"),
        ("components", Some("interface"), "Components"),
        ("documents", Some("interface"), "Documents"),
        ("forms", Some("data"), "Forms"),
        ("code", Some("tools"), "Code editor"),
        ("terminal", Some("tools"), "Terminal"),
    ] {
        let mut node = GraphNode::new(id, title.to_string());
        node.layout_parent = parent.map(NodeId::from);
        graph.upsert_node(node);
        if let Some(parent) = parent {
            graph
                .insert_edge(GraphEdge::new(format!("edge-{id}"), parent, id, ()))
                .expect("parent precedes child");
        }
    }
    graph
}

#[component]
pub fn Graph() -> Element {
    let mut model = use_signal(project_map);
    let controller = use_graph_controller();
    let mut layout = use_signal(|| "tree".to_string());
    let mut selected = use_signal(|| "No nodes selected".to_string());
    let config = match layout().as_str() {
        "mindmap" => LayoutConfig::MindMap(Default::default()),
        "radial" => LayoutConfig::Radial(Default::default()),
        "force" => LayoutConfig::Force(Default::default()),
        _ => LayoutConfig::Tree(Default::default()),
    };
    rsx! {
        div { class: "controls-strip",
            label { "Layout"
                select { aria_label: "Layout", value: layout, onchange: move |e| { layout.set(e.value()); controller.reset_positions(); },
                    option { value: "tree", "Tree" } option { value: "mindmap", "Mind map" }
                    option { value: "radial", "Radial" } option { value: "force", "Force directed" }
                }
            }
            Button { variant: ButtonVariant::Outline, onclick: move |_| {
                let index = model.peek().nodes().count();
                let id = format!("idea-{index}");
                let mut node = GraphNode::new(id.clone(), format!("New idea {}", index - 8));
                node.layout_parent = Some("project".into());
                let mut graph = model.write();
                graph.upsert_node(node);
                graph.insert_edge(GraphEdge::new(format!("edge-{id}"), "project", id, ())).expect("root exists");
            }, "Add node" }
            Button { variant: ButtonVariant::Outline, onclick: move |_| controller.fit_view(), "Fit graph" }
            span { class: "muted", "{model.read().nodes().count()} nodes" }
        }
        div { class: "graph-surface",
            GraphCanvas::<String, ()> {
                model, controller, layout: config,
                config: CanvasConfig {
                    activation: Activation::OnFocus,
                    wheel_mode: dxgraph::interaction::WheelMode::ModifierZoom,
                    marquee_selection: true, multi_drag: true,
                    aria_label: "Project map".into(), ..Default::default()
                },
                on_selection_change: move |nodes: Vec<NodeId>| selected.set(if nodes.is_empty() { "No nodes selected".into() } else { format!("Selected: {}", nodes.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")) }),
                render_node: |node: NodeRenderContext<String>| rsx! {
                    div { class: "project-node", "data-selected": node.selected,
                        span { class: "node-icon", "◇" } span { "{node.data}" }
                    }
                },
                node_label: |node: NodeRenderContext<String>| node.data,
                GraphControls { controller }
                GraphMinimap { controller }
            }
        }
        p { class: "state-line", role: "status", "{selected}" }
    }
}
