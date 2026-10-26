use crate::{
    Rect,
    edge::{EdgeMarker, EdgeStyle, edge_geometry_with_offset},
};
use dioxus::prelude::*;

#[component]
pub(crate) fn EdgeView(
    id: String,
    #[props(default)] interactive: bool,
    #[props(default)] selected: bool,
    #[props(default)] on_click: Option<Callback<crate::EdgeId>>,
    #[props(default)] on_activate: Option<Callback<crate::EdgeId>>,
    /// The single edge in the Tab order (roving tabindex); arrows move on.
    #[props(default)]
    tab_stop: bool,
    /// Move keyboard focus to this edge.
    #[props(default)]
    focus: bool,
    /// Arrow keys on a focused edge: step to the previous (-1) or next (1).
    #[props(default)]
    on_step: Option<Callback<(crate::EdgeId, i32)>>,
    source: Rect,
    target: Rect,
    style: EdgeStyle,
    marker: String,
    offset: i32,
    self_loop: bool,
    label: Option<String>,
    /// The name assistive technology announces for an interactive edge.
    #[props(default)]
    accessible_name: Option<String>,
) -> Element {
    let geometry = edge_geometry_with_offset(source, target, &style, offset, self_loop);
    let from = (style.from_end == EdgeMarker::Arrow).then(|| format!("url(#{marker})"));
    let to = (style.to_end == EdgeMarker::Arrow).then(|| format!("url(#{marker})"));
    let class = format!("dxgraph-edge {}", style.class.as_deref().unwrap_or(""));
    let mut mounted = use_signal(|| None::<std::rc::Rc<MountedData>>);
    use_effect(use_reactive!(|focus| {
        if focus && let Some(element) = mounted.peek().clone() {
            spawn(crate::interaction::platform::focus(element));
        }
    }));
    rsx! {
        g { class, "data-dxgraph-edge": id.clone(), "data-selected": selected.to_string(),
            if interactive {
                path { class: "dxgraph-edge-hit", d: geometry.path_d.clone(), fill: "none", stroke: "transparent", stroke_width: "12", vector_effect: "non-scaling-stroke", "tabindex": if tab_stop { "0" } else { "-1" }, role: "button", "aria-label": accessible_name.clone().or_else(|| label.clone()).unwrap_or_else(||"Graph connection".into()),
                    onmounted: move |event: MountedEvent| mounted.set(Some(event.data())),
                    onfocus: { let id=id.clone(); move |_| { if !selected && let Some(callback)=on_click { callback.call(id.clone().into()); } } },
                    onpointerdown: move |event: PointerEvent|event.stop_propagation(),
                    onclick: { let id=id.clone(); move |event: MouseEvent| { event.stop_propagation(); if let Some(callback)=on_click { callback.call(id.clone().into()); } } },
                    ondoubleclick: { let id=id.clone(); move |event: MouseEvent| { event.stop_propagation(); if let Some(callback)=on_activate { callback.call(id.clone().into()); } } },
                    onkeydown: { let id=id.clone(); move |event: KeyboardEvent| {
                        let key = event.key().to_string();
                        if key=="Enter" {event.stop_propagation();event.prevent_default();if let Some(callback)=on_activate {callback.call(id.clone().into());}}
                        let step = match key.as_str() { "ArrowRight" | "ArrowDown" => 1, "ArrowLeft" | "ArrowUp" => -1, _ => 0 };
                        if step != 0 && let Some(callback) = on_step {
                            event.stop_propagation();
                            event.prevent_default();
                            callback.call((id.clone().into(), step));
                        }
                    } },
                }
            }
            path {
                d: geometry.path_d.clone(),
                fill: "none",
                marker_start: from,
                marker_end: to,
                stroke_dasharray: if style.dashed { "5 4" } else { "none" },
            }
            if let Some(label) = label {
                text {
                    class: "dxgraph-edge-label",
                    x: geometry.label_pos.x,
                    y: geometry.label_pos.y,
                    text_anchor: "middle",
                    {label}
                }
            }
        }
    }
}

/// "Connection from A to B", with the edge's own label when it has one.
pub(crate) fn accessible_name(source: &str, target: &str, label: Option<&str>) -> String {
    let name = |label: &str| {
        let label = label.trim();
        if label.is_empty() {
            "untitled".to_owned()
        } else {
            label.to_owned()
        }
    };
    let mut text = format!("Connection from {} to {}", name(source), name(target));
    if let Some(label) = label.map(str::trim).filter(|label| !label.is_empty()) {
        text.push_str(": ");
        text.push_str(label);
    }
    text
}

#[cfg(test)]
mod tests {
    #[test]
    fn edges_are_named_by_their_ends_and_label() {
        assert_eq!(
            super::accessible_name("Plan", "Build", None),
            "Connection from Plan to Build"
        );
        assert_eq!(
            super::accessible_name(" ", "Build", Some(" supports ")),
            "Connection from untitled to Build: supports"
        );
    }
}
