use dioxus::prelude::*;
use dxgraph::{CanvasConfig, GraphCanvas, GraphModel, NodeRenderContext};

thread_local! {
    static SWITCH: std::cell::Cell<Option<Callback<()>>> = const { std::cell::Cell::new(None) };
}

fn remount() -> Element {
    let mut fullscreen = use_signal(|| false);
    let switch = use_callback(move |()| fullscreen.set(true));
    use_hook(move || SWITCH.set(Some(switch)));
    rsx! {
        section { if !fullscreen() { CanvasFixture {} } }
        dialog { if fullscreen() { CanvasFixture {} } }
    }
}

#[component]
fn CanvasFixture() -> Element {
    canvas()
}

fn canvas() -> Element {
    let model = use_signal(GraphModel::<String, ()>::default);
    rsx! {
        GraphCanvas {
            model,
            config: CanvasConfig {
                wheel_mode: dxgraph::interaction::WheelMode::ModifierZoom,
                ..Default::default()
            },
            node_label: move |node: NodeRenderContext<String>| node.data,
            render_node: move |node: NodeRenderContext<String>| rsx! { div { {node.data} } },
        }
    }
}

#[test]
fn canvas_root_receives_modifier_wheel() {
    dioxus::html::set_event_converter(Box::new(dioxus::html::SerializedHtmlEventConverter));
    let mut dom = VirtualDom::new(canvas);
    let mutations = dom.rebuild_to_vec();
    assert_wheel(&dom, &mutations);
}

#[test]
fn canvas_root_receives_modifier_wheel_after_dialog_remount() {
    dioxus::html::set_event_converter(Box::new(dioxus::html::SerializedHtmlEventConverter));
    let mut dom = VirtualDom::new(remount);
    let mutations = dom.rebuild_to_vec();
    assert_wheel(&dom, &mutations);
    SWITCH.with(|switch| switch.get().unwrap().call(()));
    let mutations = dom.render_immediate_to_vec();
    assert_wheel(&dom, &mutations);
}

fn assert_wheel(dom: &VirtualDom, mutations: &dioxus::core::Mutations) {
    let targets: Vec<_> = mutations
        .edits
        .iter()
        .filter_map(|mutation| match mutation {
            dioxus::core::Mutation::NewEventListener { name, id } if name == "wheel" => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(targets.len(), 1);
    for modified in [false, true] {
        let data =
            dioxus::html::PlatformEventData::new(Box::new(dioxus::html::SerializedWheelData {
                mouse: dioxus::html::point_interaction::SerializedPointInteraction {
                    ctrl_key: modified,
                    ..Default::default()
                },
                delta_mode: 0,
                delta_x: 0.0,
                delta_y: -100.0,
                delta_z: 0.0,
            }));
        let event = Event::new(std::rc::Rc::new(data), true).into_any();
        dom.runtime()
            .handle_event("wheel", event.clone(), targets[0]);
        assert_eq!(event.default_action_enabled(), !modified);
    }
}
