pub mod gesture;
pub(crate) mod input;
pub(crate) mod platform;
use dioxus::prelude::*;
pub use gesture::*;

/// Wrap interactive node content so it does not start a node drag.
#[component]
pub fn NoDrag(children: Element) -> Element {
    let on_pointer_down = move |event: PointerEvent| event.stop_propagation();
    let on_click = move |event: MouseEvent| event.stop_propagation();
    let on_key_down = move |event: KeyboardEvent| event.stop_propagation();
    rsx! {
        div {
            class: "dxgraph-nodrag",
            onpointerdown: on_pointer_down,
            onclick: on_click,
            onkeydown: on_key_down,
            {children}
        }
    }
}
/// Wrap scrollable node content so it retains its own wheel behavior.
#[component]
pub fn NoWheel(children: Element) -> Element {
    let on_wheel = move |event: WheelEvent| event.stop_propagation();
    rsx! {
        div { class: "dxgraph-nowheel", onwheel: on_wheel, {children} }
    }
}
