//! Optional graph editing helpers. All hit tests and snapping operate in world units.
use super::GraphController;
use crate::{EdgeSide, NodeId, Point, Rect, Size, Viewport};
use dioxus::prelude::*;
use indexmap::IndexMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Guide {
    pub from: Point,
    pub to: Point,
}

pub fn marquee_hits(rect: Rect, nodes: &IndexMap<NodeId, Rect>) -> Vec<NodeId> {
    nodes
        .iter()
        .filter(|(_, node)| rect.intersects(**node))
        .map(|(id, _)| id.clone())
        .collect()
}

pub fn rectangle_between(a: Point, b: Point) -> Rect {
    Rect::new(
        Point::new(a.x.min(b.x), a.y.min(b.y)),
        Size::new((a.x - b.x).abs(), (a.y - b.y).abs()),
    )
}

/// Align edges and centers independently on each axis. Node alignment wins over grid snapping.
pub fn snap(
    dragged: Rect,
    others: &[Rect],
    threshold: f64,
    grid: Option<f64>,
) -> (Point, Vec<Guide>) {
    let threshold = if threshold.is_finite() {
        threshold.max(0.0)
    } else {
        0.0
    };
    let mut x: Option<(f64, Guide)> = None;
    let mut y: Option<(f64, Guide)> = None;
    for other in others {
        for a in [dragged.origin.x, dragged.center().x, dragged.right()] {
            for b in [other.origin.x, other.center().x, other.right()] {
                let delta = b - a;
                if delta.abs() <= threshold
                    && x.as_ref().is_none_or(|(old, _)| delta.abs() < old.abs())
                {
                    x = Some((
                        delta,
                        Guide {
                            from: Point::new(b, dragged.origin.y.min(other.origin.y)),
                            to: Point::new(b, dragged.bottom().max(other.bottom())),
                        },
                    ));
                }
            }
        }
        for a in [dragged.origin.y, dragged.center().y, dragged.bottom()] {
            for b in [other.origin.y, other.center().y, other.bottom()] {
                let delta = b - a;
                if delta.abs() <= threshold
                    && y.as_ref().is_none_or(|(old, _)| delta.abs() < old.abs())
                {
                    y = Some((
                        delta,
                        Guide {
                            from: Point::new(dragged.origin.x.min(other.origin.x), b),
                            to: Point::new(dragged.right().max(other.right()), b),
                        },
                    ));
                }
            }
        }
    }
    let grid = grid.filter(|value| value.is_finite() && *value > 0.0);
    let mut delta = Point::default();
    let mut guides = Vec::new();
    if let Some((d, g)) = x {
        delta.x = d;
        guides.push(g);
    } else if let Some(grid) = grid {
        delta.x = (dragged.origin.x / grid).round() * grid - dragged.origin.x;
    }
    if let Some((d, g)) = y {
        delta.y = d;
        guides.push(g);
    } else if let Some(grid) = grid {
        delta.y = (dragged.origin.y / grid).round() * grid - dragged.origin.y;
    }
    (delta, guides)
}

pub fn ease_out(t: f64) -> f64 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}
pub fn interpolate_rect(from: Rect, to: Rect, t: f64) -> Rect {
    let t = ease_out(t);
    Rect::new(
        Point::new(
            from.origin.x + (to.origin.x - from.origin.x) * t,
            from.origin.y + (to.origin.y - from.origin.y) * t,
        ),
        Size::new(
            from.size.width + (to.size.width - from.size.width) * t,
            from.size.height + (to.size.height - from.size.height) * t,
        ),
    )
}
pub(crate) fn reduced_motion() -> bool {
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    {
        web_sys::window()
            .and_then(|window| {
                window
                    .match_media("(prefers-reduced-motion: reduce)")
                    .ok()
                    .flatten()
            })
            .is_some_and(|media| media.matches())
    }
    #[cfg(not(all(feature = "web", target_arch = "wasm32")))]
    {
        false
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ConnectContext(pub Callback<(NodeId, EdgeSide, PointerEvent)>);

/// A connection port, rendered inside a node. Its pointer gesture belongs to the canvas.
#[component]
pub fn ConnectHandle(
    node: NodeId,
    side: EdgeSide,
    /// Use -1 for pointer-only ports when the host supplies keyboard connection actions.
    #[props(default)]
    tab_index: Option<i32>,
) -> Element {
    let context = try_use_context::<ConnectContext>();
    let side_class = format!("dxgraph-connect-handle dxgraph-connect-{:?}", side).to_lowercase();
    rsx! { button { class: side_class, r#type: "button", tabindex: tab_index, aria_label: "Connect node", onpointerdown: move |event: PointerEvent| {
        event.stop_propagation(); event.prevent_default();
        if let Some(context)=context { context.0.call((node.clone(),side,event)); }
    } } }
}

/// Bounds in browser screen coordinates, updated by node geometry and viewport changes.
pub fn use_screen_anchor(controller: GraphController, node_ids: Vec<NodeId>) -> Option<Rect> {
    let geometry = controller.geometry.read();
    let viewport = *controller.viewport.read();
    node_ids
        .iter()
        .filter_map(|id| geometry.rects.get(id).copied())
        .reduce(|a, b| a.union(b))
        .map(|rect| {
            let point = viewport.world_to_screen(rect.origin);
            Rect::new(
                Point::new(point.x + geometry.origin.x, point.y + geometry.origin.y),
                Size::new(
                    rect.size.width * viewport.zoom,
                    rect.size.height * viewport.zoom,
                ),
            )
        })
}

/// Convert a screen point inside the minimap to world coordinates. The overview
/// keeps its aspect ratio (`xMidYMid meet`), so the world bounds are scaled
/// uniformly and centred within the element.
fn minimap_world_point(bounds: Rect, element: Rect, client: Point) -> Point {
    let scale = (element.size.width / bounds.size.width.max(1e-6))
        .min(element.size.height / bounds.size.height.max(1e-6))
        .max(1e-6);
    let left = element.origin.x + (element.size.width - bounds.size.width * scale) / 2.0;
    let top = element.origin.y + (element.size.height - bounds.size.height * scale) / 2.0;
    Point::new(
        bounds.origin.x + (client.x - left) / scale,
        bounds.origin.y + (client.y - top) / scale,
    )
}

/// A small navigable overview. Pointer coordinates are converted through its measured screen bounds.
#[component]
pub fn GraphMinimap(controller: GraphController) -> Element {
    let geometry = controller.geometry.read();
    let bounds = geometry
        .rects
        .values()
        .copied()
        .reduce(|a, b| a.union(b))
        .unwrap_or(Rect::new(Point::default(), Size::new(1.0, 1.0)))
        .inflate(20.0);
    let visible = controller
        .viewport
        .read()
        .visible_world_rect(geometry.container);
    let mut active = use_signal(|| false);
    let mut element = use_signal(|| None::<std::rc::Rc<MountedData>>);
    let navigate = use_callback(move |event: PointerEvent| {
        let client = event.client_coordinates();
        let mounted = element.peek().clone();
        spawn(async move {
            let Some(mounted) = mounted else {
                return;
            };
            let Ok(rect) = mounted.get_client_rect().await else {
                return;
            };
            if rect.size.width <= 0.0 || rect.size.height <= 0.0 {
                return;
            }
            let point = minimap_world_point(
                bounds,
                Rect::new(
                    Point::new(rect.origin.x, rect.origin.y),
                    Size::new(rect.size.width, rect.size.height),
                ),
                Point::new(client.x, client.y),
            );
            let viewport = controller.viewport();
            let container = controller.geometry.peek().container;
            controller.set_viewport(Viewport {
                x: container.width / 2.0 - point.x * viewport.zoom,
                y: container.height / 2.0 - point.y * viewport.zoom,
                ..viewport
            });
        });
    });
    rsx! { svg { class: "dxgraph-minimap", role: "img", "aria-label": "Graph overview", view_box: "{bounds.origin.x} {bounds.origin.y} {bounds.size.width} {bounds.size.height}", preserve_aspect_ratio: "xMidYMid meet",
        onmounted: move |event|element.set(Some(event.data())),
        onpointerdown: move |event: PointerEvent| { event.stop_propagation(); active.set(true); super::platform::capture(element.peek().as_deref(),event.pointer_id()); navigate.call(event); },
        onpointermove: move |event: PointerEvent| { if *active.peek() { navigate.call(event); } },
        onpointerup: move |event: PointerEvent| { active.set(false); super::platform::release(element.peek().as_deref(),event.pointer_id()); },
        onpointercancel: move |_|active.set(false),
        for (id,rect) in geometry.rects.iter() { rect { key: "{id}", x: rect.origin.x,y:rect.origin.y,width:rect.size.width,height:rect.size.height,class:"dxgraph-minimap-node" } }
        rect {x:visible.origin.x,y:visible.origin.y,width:visible.size.width,height:visible.size.height,class:"dxgraph-minimap-viewport"}
    } }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn minimap_points_respect_preserved_aspect_ratio() {
        // A 400x100 world shown in a 160x100 overview is scaled by 0.4 and
        // letterboxed vertically by 30px.
        let bounds = Rect::new(Point::new(-200.0, 0.0), Size::new(400.0, 100.0));
        let element = Rect::new(Point::new(10.0, 20.0), Size::new(160.0, 100.0));
        assert_eq!(
            minimap_world_point(bounds, element, Point::new(90.0, 70.0)),
            Point::new(0.0, 50.0)
        );
        assert_eq!(
            minimap_world_point(bounds, element, Point::new(10.0, 50.0)),
            Point::new(-200.0, 0.0)
        );
    }
    #[test]
    fn marquee_and_reverse_drag() {
        let a = Rect::new(Point::new(10.0, 10.0), Size::new(20.0, 20.0));
        let nodes = IndexMap::from([
            ("a".into(), a),
            ("b".into(), Rect::new(Point::new(100.0, 100.0), a.size)),
        ]);
        assert_eq!(
            marquee_hits(
                rectangle_between(Point::new(40.0, 40.0), Point::default()),
                &nodes
            ),
            vec![NodeId::from("a")]
        );
    }
    #[test]
    fn snapping_aligns_nearest_center_and_grid() {
        let a = Rect::new(Point::new(3.0, 5.0), Size::new(20.0, 20.0));
        let b = Rect::new(Point::new(26.0, 80.0), a.size);
        let (delta, guides) = snap(a, &[b], 4.0, Some(8.0));
        assert_eq!(delta, Point::new(3.0, 3.0));
        assert_eq!(guides.len(), 1);
        assert_eq!(snap(a, &[], 4.0, Some(0.0)).0, Point::default());
    }
    #[test]
    fn interpolation_endpoints() {
        let a = Rect::default();
        let b = Rect::new(Point::new(30.0, 80.0), Size::new(80.0, 60.0));
        assert_eq!(interpolate_rect(a, b, 0.0), a);
        assert_eq!(interpolate_rect(a, b, 1.0), b);
        assert!(interpolate_rect(a, b, 0.5).origin.x > 15.0);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropAxis {
    Horizontal,
    Vertical,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StructuralDropTarget {
    Reparent(NodeId),
    Before(NodeId),
    After(NodeId),
}

/// Host-supplied exclusions should include the dragged nodes and their descendants.
/// Sibling edge bands and gaps offer reorder targets; node interiors offer parenting.
pub fn structural_drop_target(
    pointer: Point,
    rects: &IndexMap<NodeId, Rect>,
    excluded: &[NodeId],
    siblings: &[NodeId],
    axis: DropAxis,
    margin: f64,
) -> Option<StructuralDropTarget> {
    let margin = if margin.is_finite() {
        margin.max(0.0)
    } else {
        0.0
    };
    for (id, rect) in rects.iter().filter(|(id, _)| !excluded.contains(id)) {
        if rect.contains(pointer) {
            if siblings.contains(id) {
                let (start, end, point) = match axis {
                    DropAxis::Horizontal => (rect.origin.x, rect.right(), pointer.x),
                    DropAxis::Vertical => (rect.origin.y, rect.bottom(), pointer.y),
                };
                if point - start <= margin {
                    return Some(StructuralDropTarget::Before(id.clone()));
                }
                if end - point <= margin {
                    return Some(StructuralDropTarget::After(id.clone()));
                }
            }
            return Some(StructuralDropTarget::Reparent(id.clone()));
        }
    }
    rects
        .iter()
        .filter(|(id, rect)| {
            !excluded.contains(id)
                && siblings.contains(id)
                && rect.inflate(margin).contains(pointer)
        })
        .min_by(|(_, a), (_, b)| {
            a.center()
                .distance(pointer)
                .total_cmp(&b.center().distance(pointer))
        })
        .map(|(id, rect)| {
            let before = match axis {
                DropAxis::Horizontal => pointer.x < rect.center().x,
                DropAxis::Vertical => pointer.y < rect.center().y,
            };
            if before {
                StructuralDropTarget::Before(id.clone())
            } else {
                StructuralDropTarget::After(id.clone())
            }
        })
}

#[cfg(test)]
mod structural_tests {
    use super::*;
    #[test]
    fn body_edges_gaps_and_exclusions() {
        let rect = Rect::new(Point::new(50.0, 50.0), Size::new(100.0, 80.0));
        let rects = IndexMap::from([("a".into(), rect)]);
        let siblings = vec!["a".into()];
        assert_eq!(
            structural_drop_target(
                rect.center(),
                &rects,
                &[],
                &siblings,
                DropAxis::Vertical,
                12.0
            ),
            Some(StructuralDropTarget::Reparent("a".into()))
        );
        assert_eq!(
            structural_drop_target(
                Point::new(80.0, 54.0),
                &rects,
                &[],
                &siblings,
                DropAxis::Vertical,
                12.0
            ),
            Some(StructuralDropTarget::Before("a".into()))
        );
        assert_eq!(
            structural_drop_target(
                Point::new(80.0, 139.0),
                &rects,
                &[],
                &siblings,
                DropAxis::Vertical,
                12.0
            ),
            Some(StructuralDropTarget::After("a".into()))
        );
        assert!(
            structural_drop_target(
                rect.center(),
                &rects,
                &siblings,
                &siblings,
                DropAxis::Vertical,
                12.0
            )
            .is_none()
        );
        assert!(
            structural_drop_target(
                Point::default(),
                &rects,
                &[],
                &siblings,
                DropAxis::Vertical,
                12.0
            )
            .is_none()
        );
    }
}

/// Render in the canvas overlay. Parenting uses a ring; reordering uses a bar.
#[component]
pub fn StructuralDropIndicator(
    controller: GraphController,
    target: StructuralDropTarget,
    axis: DropAxis,
) -> Element {
    let (id, before, parenting) = match &target {
        StructuralDropTarget::Reparent(id) => (id, false, true),
        StructuralDropTarget::Before(id) => (id, true, false),
        StructuralDropTarget::After(id) => (id, false, false),
    };
    let rects = controller.rects();
    let Some(rect) = rects.get(id) else {
        return rsx! {};
    };
    let viewport = controller.viewport();
    let origin = viewport.world_to_screen(rect.origin);
    let (mut x, mut y) = (origin.x, origin.y);
    let (mut width, mut height) = (
        rect.size.width * viewport.zoom,
        rect.size.height * viewport.zoom,
    );
    if !parenting {
        match axis {
            DropAxis::Horizontal => {
                x += if before { -5.0 } else { width + 2.0 };
                width = 3.0;
            }
            DropAxis::Vertical => {
                y += if before { -5.0 } else { height + 2.0 };
                height = 3.0;
            }
        }
    }
    rsx! { div {
        class: if parenting { "dxgraph-drop-indicator dxgraph-drop-parent" } else { "dxgraph-drop-indicator dxgraph-drop-order" },
        "aria-hidden": "true",
        style: "left:{x}px;top:{y}px;width:{width}px;height:{height}px",
    } }
}
