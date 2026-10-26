use super::{
    NodeDetail, NodeEvent, NodeRenderContext,
    keyboard::{KeyboardAction, keyboard_action},
};
use crate::{NodeId, Point, Size};
use dioxus::prelude::*;

fn node_border_box(event: &ResizeEvent) -> Option<Size> {
    // Native resize events can bubble. A node's size must never replace the
    // container dimensions used by fit, center, and culling.
    event.stop_propagation();
    event.get_border_box_size().ok().map(|size| Size {
        width: size.width,
        height: size.height,
    })
}

fn node_key_action(event: &KeyboardEvent, host: impl FnOnce() -> bool) -> Option<KeyboardAction> {
    let action = match keyboard_action(&event.key().to_string()) {
        Some(action @ (KeyboardAction::Activate | KeyboardAction::Select)) => action,
        _ => return None,
    };
    if host() {
        event.stop_propagation();
        return None;
    }
    event.stop_propagation();
    event.prevent_default();
    Some(action)
}

#[derive(Props, Clone, PartialEq)]
pub(crate) struct NodeViewProps<N: Clone + PartialEq + 'static> {
    pub id: NodeId,
    pub controller: super::GraphController,
    pub data: N,
    pub position: Point,
    pub size: Size,
    pub selected: bool,
    pub hidden: bool,
    pub detail: NodeDetail,
    pub label: String,
    pub render_node: Callback<NodeRenderContext<N>, Element>,
    pub render_minimal: Option<Callback<NodeRenderContext<N>, Element>>,
    pub on_pointer: EventHandler<(NodeId, PointerEvent)>,
    pub on_measure: EventHandler<(NodeId, Size, NodeDetail)>,
    pub on_activate: EventHandler<NodeEvent>,
    pub on_select: EventHandler<NodeEvent>,
    /// Keyboard focus entered (`true`) or left (`false`) the node.
    pub on_focus: EventHandler<(NodeId, bool)>,
    pub host_key_down: Option<Callback<KeyboardEvent, bool>>,
    pub draggable: bool,
    /// Roving tab stop: only one node is in the page tab order, so Tab moves
    /// past the graph instead of visiting every node (arrows navigate).
    pub tab_stop: bool,
    /// The node is the drop target of an in-progress connection drag.
    pub connect_target: bool,
    /// The node follows the pointer in a drag.
    #[props(default)]
    pub dragging: bool,
}

#[allow(non_snake_case)]
pub(crate) fn NodeView<N: Clone + PartialEq + 'static>(props: NodeViewProps<N>) -> Element {
    let mut mounted = use_signal(|| None::<std::rc::Rc<MountedData>>);
    let focus_id = props.id.clone();
    use_effect(move || {
        if props.controller.focus_request.read().as_ref() == Some(&focus_id)
            && let Some(element) = mounted.read().clone()
        {
            spawn(async move {
                let _ = element.set_focus(true).await;
            });
        }
    });
    let context = NodeRenderContext {
        id: props.id.clone(),
        data: props.data,
        selected: props.selected,
        detail: props.detail,
    };
    let content = if props.detail == NodeDetail::Minimal {
        if let Some(renderer) = props.render_minimal {
            renderer.call(context)
        } else {
            rsx! {
                div {
                    class: "dxgraph-placeholder",
                    style: "width:{props.size.width}px;height:{props.size.height}px",
                    aria_label: props.label.clone(),
                }
            }
        }
    } else {
        props.render_node.call(context)
    };
    let pointer_id = props.id.clone();
    let measure_id = props.id.clone();
    let key_id = props.id.clone();
    let focus_in_id = props.id.clone();
    let focus_out_id = props.id.clone();
    let on_pointer_down = move |event: PointerEvent| {
        event.stop_propagation();
        props.on_pointer.call((pointer_id.clone(), event));
    };
    let on_resize = move |event: ResizeEvent| {
        if let Some(size) = node_border_box(&event) {
            props
                .on_measure
                .call((measure_id.clone(), size, props.detail));
        }
    };
    let on_key_down = move |event: KeyboardEvent| {
        let action = node_key_action(&event, || {
            props
                .host_key_down
                .is_some_and(|handler| handler.call(event.clone()))
        });
        let callback = match action {
            Some(KeyboardAction::Activate) => props.on_activate,
            Some(KeyboardAction::Select) => props.on_select,
            _ => return,
        };
        callback.call(NodeEvent {
            id: key_id.clone(),
            shift: event.modifiers().shift(),
        });
    };
    // Reduced detail renders less content than the Full measurement that drives
    // layout and edge anchoring. Keep the wrapper at that size so edges still
    // meet the visible border and content stays centred.
    let size = if props.detail == NodeDetail::Full {
        String::new()
    } else {
        format!(
            "width:{}px;height:{}px;",
            props.size.width, props.size.height
        )
    };
    let style = format!(
        "transform:translate({}px,{}px);{size}visibility:{}",
        props.position.x,
        props.position.y,
        if props.hidden { "hidden" } else { "visible" }
    );
    let detail = match props.detail {
        NodeDetail::Full => "full",
        NodeDetail::Compact => "compact",
        NodeDetail::Minimal => "minimal",
    };
    rsx! {
        div {
            class: "dxgraph-node",
            onmounted: move |event| mounted.set(Some(event.data())),
            "data-dxgraph-node": props.id.to_string(),
            "data-selected": props.selected.to_string(),
            "data-draggable": props.draggable.to_string(),
            "data-detail": detail,
            "data-connect-target": props.connect_target.then_some("true"),
            "data-dragging": props.dragging.then_some("true"),
            tabindex: if props.tab_stop { 0 } else { -1 },
            role: "button",
            aria_label: props.label,
            style,
            onpointerdown: on_pointer_down,
            onresize: on_resize,
            onkeydown: on_key_down,
            onfocusin: move |_| props.on_focus.call((focus_in_id.clone(), true)),
            onfocusout: move |_| props.on_focus.call((focus_out_id.clone(), false)),
            {content}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dioxus::html::{Code, HasKeyboardData, Key, Location, Modifiers, ModifiersInteraction};
    use dioxus::html::{HasResizeData, ResizeError, geometry::PixelsSize};
    struct NodeKey(Key);
    impl ModifiersInteraction for NodeKey {
        fn modifiers(&self) -> Modifiers {
            Modifiers::empty()
        }
    }
    impl HasKeyboardData for NodeKey {
        fn key(&self) -> Key {
            self.0.clone()
        }
        fn code(&self) -> Code {
            Code::Unidentified
        }
        fn location(&self) -> Location {
            Location::Standard
        }
        fn is_auto_repeating(&self) -> bool {
            false
        }
        fn is_composing(&self) -> bool {
            false
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }
    #[test]
    fn node_enter_space_host_precedence_and_default_viewer_actions() {
        for (key, action) in [
            (Key::Enter, KeyboardAction::Activate),
            (Key::Character(" ".into()), KeyboardAction::Select),
        ] {
            let event = Event::new(
                std::rc::Rc::new(KeyboardData::new(NodeKey(key.clone()))),
                true,
            );
            let calls = std::cell::Cell::new(0);
            assert_eq!(
                node_key_action(&event, || {
                    calls.set(calls.get() + 1);
                    true
                }),
                None
            );
            assert_eq!(calls.get(), 1);
            assert!(
                !event.propagates(),
                "A consumed host key must not run again at the canvas"
            );
            let event = Event::new(std::rc::Rc::new(KeyboardData::new(NodeKey(key))), true);
            assert_eq!(node_key_action(&event, || false), Some(action));
            assert!(!event.propagates());
        }
        let event = Event::new(std::rc::Rc::new(KeyboardData::new(NodeKey(Key::Tab))), true);
        assert_eq!(
            node_key_action(&event, || panic!(
                "Forwarded keys run the host at the canvas"
            )),
            None
        );
        assert!(event.propagates());
    }
    struct NodeResize;
    impl HasResizeData for NodeResize {
        fn get_border_box_size(&self) -> Result<PixelsSize, ResizeError> {
            Ok(PixelsSize::new(160.0, 64.0))
        }
        fn get_content_box_size(&self) -> Result<PixelsSize, ResizeError> {
            Ok(PixelsSize::new(140.0, 44.0))
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }
    #[test]
    fn node_resize_measures_border_box_without_reaching_container() {
        let event = Event::new(std::rc::Rc::new(ResizeData::new(NodeResize)), true);
        assert!(event.propagates());
        assert_eq!(
            node_border_box(&event),
            Some(Size {
                width: 160.0,
                height: 64.0
            })
        );
        assert!(
            !event.propagates(),
            "A node resize must not reach the canvas resize handler"
        );
    }
}
