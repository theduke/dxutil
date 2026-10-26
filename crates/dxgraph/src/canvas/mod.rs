mod background;
mod editing;
pub(crate) use editing::rectangle_between as editing_rectangle;
pub use editing::{
    ConnectHandle, DropAxis, GraphMinimap, Guide, StructuralDropIndicator, StructuralDropTarget,
    marquee_hits, snap, structural_drop_target, use_screen_anchor,
};
mod controls;
mod edges;
mod keyboard;
mod node;
mod origin;
mod pipeline;
mod state;
use background::GraphBackground;
pub use controls::{
    FitInsets, GraphController, GraphControls, LayoutArrangement, SavedView, use_graph_controller,
};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum NodeDetail {
    #[default]
    Full,
    Compact,
    Minimal,
}

use crate::{
    GraphModel, NodeId, Point, Rect, Size, Viewport, ViewportLimits,
    interaction::{
        GestureContext, GestureEffect, GestureInput, GestureState, GestureTarget, WheelMode, input,
        platform,
    },
    layout::LayoutConfig,
};
use dioxus::prelude::*;
use indexmap::{IndexMap, IndexSet};
use keyboard::{KeyboardAction, keyboard_action};
use origin::{NativeInputs, OriginCache, OriginRefresh};
use pipeline::{CanvasEffect, CanvasInput, CanvasPipeline};
use state::SelectionEvent;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasConfig {
    pub position_source: PositionSource,
    pub activation: Activation,
    pub interactive_edges: bool,
    pub animate_layout: bool,
    /// Keep the shared controller viewport and its world center when remounting.
    pub preserve_viewport: bool,
    /// Cap automatic and explicit Fit zoom without restricting manual zoom.
    pub fit_max_zoom: Option<f64>,
    /// Readable zoom floor of automatic framing (initial fit, resizes and
    /// layout changes). Larger content is anchored on the selection or root
    /// and reachable by panning; explicit Fit still shows everything.
    pub frame_min_zoom: Option<f64>,
    /// Screen areas covered by host overlays that Fit keeps content clear of.
    pub fit_insets: FitInsets,
    pub multi_drag: bool,
    /// Allow node movement; disabling it still permits selection and panning.
    pub draggable_nodes: bool,
    pub marquee_selection: bool,
    pub snap_threshold: Option<f64>,
    pub snap_grid: Option<f64>,
    pub limits: ViewportLimits,
    pub full_detail_zoom: f64,
    pub compact_detail_zoom: f64,
    pub cull_margin: f64,
    pub wheel_mode: WheelMode,
    pub aria_label: String,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PositionSource {
    #[default]
    CanvasOwned,
    ModelPinned,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Activation {
    #[default]
    Always,
    OnFocus,
}

impl Default for CanvasConfig {
    fn default() -> Self {
        Self {
            position_source: PositionSource::default(),
            activation: Activation::default(),
            interactive_edges: false,
            animate_layout: false,
            preserve_viewport: false,
            fit_max_zoom: None,
            frame_min_zoom: None,
            fit_insets: FitInsets::default(),
            multi_drag: false,
            draggable_nodes: true,
            marquee_selection: false,
            snap_threshold: None,
            snap_grid: None,
            limits: ViewportLimits::default(),
            full_detail_zoom: 0.6,
            compact_detail_zoom: 0.3,
            cull_margin: 200.0,
            wheel_mode: WheelMode::Zoom,
            aria_label: "Graph canvas".into(),
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct NodeRenderContext<N> {
    pub id: NodeId,
    pub data: N,
    pub selected: bool,
    pub detail: NodeDetail,
}
#[derive(Clone, Debug, PartialEq)]
pub struct NodeEvent {
    pub id: NodeId,
    pub shift: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EdgeRenderContext<E> {
    pub id: crate::EdgeId,
    pub data: E,
    pub label: Option<String>,
    pub selected: bool,
}

#[derive(Props, Clone, PartialEq)]
pub struct GraphCanvasProps<N: Clone + PartialEq + 'static, E: Clone + PartialEq + 'static> {
    pub model: ReadSignal<GraphModel<N, E>>,
    #[props(default)]
    pub on_key_down: Option<Callback<KeyboardEvent, bool>>,
    #[props(default)]
    pub on_nodes_moved: Option<EventHandler<Vec<(NodeId, Point)>>>,
    #[props(default)]
    pub on_drag_move: Option<EventHandler<(Vec<NodeId>, Point)>>,
    #[props(default)]
    pub on_interaction_change: Option<EventHandler<bool>>,
    #[props(default)]
    pub on_edge_click: Option<EventHandler<crate::EdgeId>>,
    #[props(default)]
    pub on_edge_activate: Option<EventHandler<crate::EdgeId>>,
    #[props(default)]
    pub render_edge_label: Option<Callback<EdgeRenderContext<E>, Element>>,
    #[props(default)]
    pub on_connect: Option<EventHandler<ConnectEvent>>,
    #[props(default)]
    pub on_connect_to_empty: Option<EventHandler<(NodeId, Point)>>,
    #[props(default)]
    pub on_background_double_click: Option<EventHandler<Point>>,
    pub render_node: Callback<NodeRenderContext<N>, Element>,
    pub node_label: Callback<NodeRenderContext<N>, String>,
    #[props(default)]
    pub render_minimal: Option<Callback<NodeRenderContext<N>, Element>>,
    #[props(default)]
    pub layout: LayoutConfig,
    #[props(default)]
    pub config: CanvasConfig,
    #[props(default)]
    pub controller: Option<GraphController>,
    #[props(default)]
    pub on_node_click: Option<EventHandler<NodeEvent>>,
    #[props(default)]
    pub on_node_activate: Option<EventHandler<NodeEvent>>,
    #[props(default)]
    pub on_node_moved: Option<EventHandler<(NodeId, Point)>>,
    #[props(default)]
    pub on_selection_change: Option<EventHandler<Vec<NodeId>>>,
    #[props(default)]
    pub on_background_click: Option<EventHandler<Point>>,
    #[props(default)]
    pub class: Option<String>,
    #[props(default)]
    pub children: Element,
}

/// Apply reducer effects in one place, keeping controller geometry in sync.
fn apply_canvas_input<N: Clone + PartialEq + 'static, E: Clone + PartialEq + 'static>(
    mut pipeline: Signal<CanvasPipeline>,
    model: ReadSignal<GraphModel<N, E>>,
    mut controller: GraphController,
    config: CanvasConfig,
    input: CanvasInput,
) {
    if controller.geometry.peek().limits != config.limits {
        controller.geometry.write().limits = config.limits;
    }
    if controller.geometry.peek().fit_max_zoom != config.fit_max_zoom {
        controller.geometry.write().fit_max_zoom = config.fit_max_zoom;
    }
    if controller.geometry.peek().fit_insets != config.fit_insets {
        controller.geometry.write().fit_insets = config.fit_insets;
    }
    if controller.geometry.peek().frame_min_zoom != config.frame_min_zoom {
        controller.geometry.write().frame_min_zoom = config.frame_min_zoom;
    }
    let animate = config.animate_layout
        && !editing::reduced_motion()
        && matches!(
            &input,
            CanvasInput::ModelChanged
                | CanvasInput::LayoutChanged(..)
                | CanvasInput::FlushMeasurements(_)
                | CanvasInput::MeasureTimeout(_)
        )
        && pipeline.peek().state.dragging.is_none();
    let previous = controller.geometry.peek().rects.clone();
    let measured_container = pipeline.peek().container;
    pipeline.write().position_source = config.position_source;
    let reframe_was_pending = pipeline.peek().reframe_pending();
    let effects = pipeline.write().reduce(&model.peek(), input);
    if !reframe_was_pending
        && (pipeline.peek().reframe_pending() || effects.contains(&CanvasEffect::FrameLayout))
    {
        // A layout change begins: remember where its anchor is on screen.
        let viewport = controller.viewport();
        pipeline.write().layout_anchor = controller
            .frame_anchor_in(&previous)
            .map(|(id, rect)| (id, viewport.world_to_screen(rect.center())));
    }
    // Animated layouts temporarily replace the rects with interpolated frames:
    // only freshly computed rects are final, otherwise keep the last final ones.
    let settled = if effects.contains(&CanvasEffect::RectsChanged) {
        pipeline.peek().rects.clone()
    } else {
        controller.geometry.peek().settled.clone()
    };
    if effects.contains(&CanvasEffect::RectsChanged) {
        let token = pipeline.peek().animation + 1;
        pipeline.write().animation = token;
        if animate && !previous.is_empty() {
            let target = pipeline.peek().rects.clone();
            let initial = target
                .iter()
                .map(|(id, rect)| (id.clone(), previous.get(id).copied().unwrap_or(*rect)))
                .collect();
            pipeline.write().rects = Rc::new(initial);
            spawn(async move {
                let began = platform::timestamp_ms();
                loop {
                    dioxus_sdk_time::sleep(std::time::Duration::from_millis(16)).await;
                    if pipeline.peek().animation != token {
                        break;
                    }
                    let t =
                        (platform::timestamp_ms().saturating_sub(began) as f64 / 220.0).min(1.0);
                    let rects = Rc::new(
                        target
                            .iter()
                            .map(|(id, rect)| {
                                (
                                    id.clone(),
                                    editing::interpolate_rect(
                                        previous.get(id).copied().unwrap_or(*rect),
                                        *rect,
                                        t,
                                    ),
                                )
                            })
                            .collect::<IndexMap<_, _>>(),
                    );
                    {
                        let mut state = pipeline.write();
                        state.rects = rects.clone();
                        state.revision += 1;
                    }
                    {
                        let mut geometry = controller.geometry.write();
                        geometry.rects = rects;
                        geometry.revision += 1;
                    }
                    if t >= 1.0 {
                        break;
                    }
                }
            });
        }
    }
    for effect in effects {
        match effect {
            CanvasEffect::RectsChanged => {
                let root = model
                    .peek()
                    .nodes()
                    .find(|node| node.layout_parent.is_none())
                    .map(|node| node.id.clone());
                {
                    let pipeline = pipeline.peek();
                    let mut geometry = controller.geometry.write();
                    geometry.rects = pipeline.rects.clone();
                    geometry.settled = settled.clone();
                    geometry.edge_bounds = curved_edge_bounds(&model.peek(), &settled);
                    geometry.revision = pipeline.revision;
                    geometry.root = root;
                }
            }
            CanvasEffect::ContainerChanged => {
                let container = pipeline.peek().container;
                if let Some(container) = container {
                    // A remount that preserves the viewport compares against the
                    // size the shared controller last saw.
                    let previous = measured_container.or_else(|| {
                        config
                            .preserve_viewport
                            .then(|| controller.geometry.peek().container)
                    });
                    controller.geometry.write().container = container;
                    let bounds = content_bounds(&model.peek(), &settled);
                    // A saved view is authoritative over resize re-framing until
                    // the remounted canvas has settled.
                    let restore = pipeline.peek().restore;
                    match resize_reframe(
                        restore,
                        previous,
                        container,
                        controller.is_manual_view(),
                        bounds,
                    ) {
                        Reframe::Restore(viewport, manual) => {
                            controller.set_viewport(viewport);
                            if manual {
                                controller.mark_manual_view();
                            } else {
                                controller.clear_manual_view();
                            }
                        }
                        Reframe::Fit(bounds) => {
                            // Unsettled rects are a remount's unmeasured
                            // placeholders: frame once measurements settle.
                            if pipeline.write().fit_when_settled() {
                                controller.set_viewport(controller.frame_bounds(bounds));
                            }
                        }
                        Reframe::KeepCenter(previous) => {
                            let viewport = controller.viewport();
                            controller.set_viewport(Viewport {
                                x: viewport.x + (container.width - previous.width) / 2.0,
                                y: viewport.y + (container.height - previous.height) / 2.0,
                                ..viewport
                            });
                        }
                        Reframe::None => {}
                    }
                }
            }
            // Fit the settled layout, not the first frame of its animation.
            CanvasEffect::FitView => {
                let bounds = content_bounds(&model.peek(), &settled);
                if let Some(bounds) = bounds {
                    controller.set_viewport(controller.frame_bounds(bounds));
                    controller.clear_manual_view();
                }
            }
            // Follow a layout switch or re-layout alongside its animation,
            // unless the user framed the view by hand and can still see it.
            CanvasEffect::FrameLayout => {
                let anchor = pipeline.write().layout_anchor.take();
                let bounds = content_bounds(&model.peek(), &settled);
                if let Some(bounds) = bounds {
                    let viewport = controller.viewport();
                    let anchored = anchor.and_then(|(id, screen)| {
                        let center = settled.get(&id)?.center();
                        Some(Viewport {
                            x: screen.x - center.x * viewport.zoom,
                            y: screen.y - center.y * viewport.zoom,
                            ..viewport
                        })
                    });
                    match anchored {
                        // A hand-framed view keeps its zoom and the anchor's
                        // place on screen while the layout moves around it.
                        Some(target) if controller.is_manual_view() => {
                            controller.animate_to(target)
                        }
                        _ => {
                            controller.animate_to(controller.frame_bounds(bounds));
                            controller.clear_manual_view();
                        }
                    }
                }
            }
            CanvasEffect::ScheduleTimeout(batch) => {
                let config = config.clone();
                spawn(async move {
                    dioxus_sdk_time::sleep(std::time::Duration::from_millis(100)).await;
                    apply_canvas_input(
                        pipeline,
                        model,
                        controller,
                        config,
                        CanvasInput::MeasureTimeout(batch),
                    );
                });
            }
            CanvasEffect::ScheduleMeasurements(flush) => {
                let config = config.clone();
                spawn(async move {
                    // A single queued tick absorbs a burst of ResizeObserver notifications.
                    dioxus_sdk_time::sleep(std::time::Duration::from_millis(16)).await;
                    apply_canvas_input(
                        pipeline,
                        model,
                        controller,
                        config,
                        CanvasInput::FlushMeasurements(flush),
                    );
                });
            }
        }
    }
    // A restored view stays authoritative until this canvas has measured its
    // content and frame (the host model may arrive after mounting, and the
    // frame may report interim sizes, e.g. while a page scrollbar returns);
    // later resizes follow the regular re-framing rules.
    let restored = {
        let pipeline = pipeline.peek();
        pipeline.restore.is_some()
            && pipeline.container.is_some()
            && !pipeline.rects.is_empty()
            && pipeline.is_settled()
    };
    if restored {
        let restore = pipeline.write().restore.take();
        if *controller.pending_restore.peek() == restore {
            controller.pending_restore.set(None);
        }
    }
    remember_arrangement(&pipeline.peek(), controller);
}

/// Hand the settled arrangement to the next canvas mounting on `controller`.
/// Interim batches and drags are not an arrangement anyone chose to keep.
fn remember_arrangement(pipeline: &CanvasPipeline, mut controller: GraphController) {
    if !pipeline.is_settled()
        || pipeline.state.dragging.is_some()
        || !pipeline.state.drag_group.is_empty()
        || pipeline.state.positions.is_empty()
    {
        return;
    }
    let current = controller.arrangement.peek();
    let unchanged = current.as_ref().is_some_and(|arrangement| {
        arrangement.layout == *pipeline.layout_config()
            && *arrangement.positions == pipeline.state.positions
    });
    drop(current);
    if !unchanged {
        controller.arrangement.set(Some(LayoutArrangement {
            layout: pipeline.layout_config().clone(),
            positions: Rc::new(pipeline.state.positions.clone()),
        }));
    }
}

/// How the viewport follows a container resize.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Reframe {
    /// Restore a saved view, with its manual flag.
    Restore(Viewport, bool),
    /// Re-fit the content: the view was framed automatically.
    Fit(Rect),
    /// Keep the world point at the canvas centre where it was.
    KeepCenter(Size),
    None,
}

/// A view the user has not panned or zoomed keeps its content framed when the
/// canvas resizes (panel toggle, wide mode, frame or window resize). After a
/// manual pan or zoom the centre stays put instead. Collapsed (hidden) canvases
/// never fit, so returning to a visible size re-frames correctly.
fn container_reframe(
    previous: Option<Size>,
    container: Size,
    manual: bool,
    bounds: Option<Rect>,
) -> Reframe {
    let usable = |size: Size| size.width > 32.0 && size.height > 32.0;
    let Some(previous) = previous else {
        return Reframe::None;
    };
    match bounds {
        Some(bounds) if !manual && usable(container) => Reframe::Fit(bounds),
        _ => Reframe::KeepCenter(previous),
    }
}

/// [`container_reframe`], unless a saved view is waiting to be restored: that
/// view wins over the size the shared controller saw last (e.g. fullscreen).
fn resize_reframe(
    restore: Option<controls::SavedView>,
    previous: Option<Size>,
    container: Size,
    manual: bool,
    bounds: Option<Rect>,
) -> Reframe {
    match restore {
        Some(saved) => {
            Reframe::Restore(controls::restored_viewport(saved, container), saved.manual)
        }
        None => container_reframe(previous, container, manual, bounds),
    }
}

/// The topmost node under `world`, excluding the connection source.
/// A connection drawn by dragging a port onto another node.
#[derive(Clone, Debug, PartialEq)]
pub struct ConnectEvent {
    pub source: NodeId,
    /// The port the drag started from.
    pub source_side: crate::EdgeSide,
    pub target: NodeId,
    /// The target port the drop snapped to; `None` for a drop elsewhere on
    /// the target, which leaves its attach side automatic.
    pub target_side: Option<crate::EdgeSide>,
}

/// World distance within which a connection drop snaps to a target port, at
/// least [`PORT_SNAP_SCREEN`] screen pixels at any zoom.
const PORT_SNAP_WORLD: f64 = 16.0;
const PORT_SNAP_SCREEN: f64 = 12.0;

/// The port of `target` a connection dropped at `world` snaps to: the
/// nearest side midpoint within the snap radius.
fn connection_port(target: Rect, world: Point, zoom: f64) -> Option<crate::EdgeSide> {
    use crate::EdgeSide;
    let radius = PORT_SNAP_WORLD.max(PORT_SNAP_SCREEN / zoom.max(0.001));
    [
        EdgeSide::Top,
        EdgeSide::Right,
        EdgeSide::Bottom,
        EdgeSide::Left,
    ]
    .into_iter()
    .map(|side| (side, crate::edge::side_anchor(target, side).distance(world)))
    .filter(|(_, distance)| *distance <= radius)
    .min_by(|a, b| a.1.total_cmp(&b.1))
    .map(|(side, _)| side)
}

fn connection_target(
    rects: &IndexMap<NodeId, Rect>,
    source: &NodeId,
    world: Point,
) -> Option<NodeId> {
    rects
        .iter()
        .rev()
        .find(|(id, rect)| *id != source && rect.contains(world))
        .map(|(id, _)| id.clone())
}

/// The live connection path: it leaves the dragged handle's side and snaps to
/// the target border while hovering a valid node, or to a hovered port.
fn connection_preview(
    source: Rect,
    side: crate::EdgeSide,
    pointer: Point,
    target: Option<Rect>,
    zoom: f64,
) -> String {
    let style = crate::EdgeStyle {
        source_anchor: crate::EdgeAnchor::Side(side),
        target_anchor: target
            .and_then(|target| connection_port(target, pointer, zoom))
            .map_or(crate::EdgeAnchor::Floating, crate::EdgeAnchor::Side),
        ..Default::default()
    };
    let target = target.unwrap_or(Rect::new(pointer, Size::default()));
    crate::edge_geometry(source, target, &style, false).path_d
}

type EdgeGroups = IndexMap<(NodeId, NodeId), Vec<(crate::EdgeId, bool)>>;

/// Lane indices use half-spacing units so even-sized groups are centred too.
fn parallel_edge_offsets<N, E>(model: &GraphModel<N, E>) -> IndexMap<crate::EdgeId, i32> {
    let mut groups = EdgeGroups::new();
    for edge in model.edges() {
        let reversed = edge.source > edge.target;
        let pair = if reversed {
            (edge.target.clone(), edge.source.clone())
        } else {
            (edge.source.clone(), edge.target.clone())
        };
        groups
            .entry(pair)
            .or_default()
            .push((edge.id.clone(), reversed));
    }
    let mut offsets = IndexMap::new();
    for (pair, group) in &groups {
        let count = group.len() as i32;
        for (index, (edge, reversed)) in group.iter().enumerate() {
            let lane = if pair.0 == pair.1 {
                index as i32 * 2
            } else {
                index as i32 * 2 - (count - 1)
            };
            // Geometry computes its perpendicular from the directed edge.
            offsets.insert(edge.clone(), if *reversed { -lane } else { lane });
        }
    }
    offsets
}

/// The extent of the content: the nodes and the bows of curved edges, which
/// may loop beyond them. Framing and Fit show all of it.
fn content_bounds<N, E>(model: &GraphModel<N, E>, rects: &IndexMap<NodeId, Rect>) -> Option<Rect> {
    let nodes = rects.values().copied().reduce(|a, b| a.union(b))?;
    Some(curved_edge_bounds(model, rects).map_or(nodes, |edges| nodes.union(edges)))
}

/// The extent of the curved edges (cross-links) between `rects`, as routed
/// around the nodes in their way.
fn curved_edge_bounds<N, E>(
    model: &GraphModel<N, E>,
    rects: &IndexMap<NodeId, Rect>,
) -> Option<Rect> {
    /// Room for the stroke and arrowheads around the sampled curve points.
    const MARGIN: f64 = 8.0;
    let curved =
        |edge: &&crate::GraphEdge<E>| edge.style.curvature != 0.0 && edge.source != edge.target;
    if !model.edges().any(|edge| curved(&edge)) {
        return None;
    }
    let offsets = parallel_edge_offsets(model);
    let routed = routed_edges(model, rects, &offsets);
    model
        .edges()
        .filter(curved)
        .filter_map(|edge| {
            let source = rects.get(&edge.source)?;
            let target = rects.get(&edge.target)?;
            let style = routed
                .get(&edge.id)
                .map_or(&edge.style, |route| &route.style);
            let offset = offsets.get(&edge.id).copied().unwrap_or_default();
            let geometry =
                crate::edge::edge_geometry_with_offset(*source, *target, style, offset, false);
            // A bow's extreme lies at its midpoint (where the label sits).
            [geometry.start, geometry.end, geometry.label_pos]
                .into_iter()
                .map(|point| Rect::new(point, Size::default()))
                .reduce(|a, b| a.union(b))
                .map(|rect| rect.inflate(MARGIN))
        })
        .reduce(|a, b| a.union(b))
}

/// Routes of curved secondary edges (cross-links) re-routed around the nodes
/// lying between their ends. Lists only edges whose route had to change.
fn routed_edges<N, E>(
    model: &GraphModel<N, E>,
    rects: &IndexMap<NodeId, Rect>,
    offsets: &IndexMap<crate::EdgeId, i32>,
) -> IndexMap<crate::EdgeId, crate::EdgeRoute> {
    let mut routed = IndexMap::new();
    // Sides carrying straight (hierarchy) connections, where a secondary edge
    // attaching too would merge into them.
    let mut busy = IndexMap::<&NodeId, Vec<crate::EdgeSide>>::new();
    for edge in model.edges().filter(|edge| edge.style.curvature == 0.0) {
        if let (Some(source), Some(target)) = (rects.get(&edge.source), rects.get(&edge.target)) {
            let (from, to) = crate::edge_sides(*source, *target, &edge.style);
            busy.entry(&edge.source).or_default().extend(from);
            busy.entry(&edge.target).or_default().extend(to);
        }
    }
    let sides = |id: &NodeId| busy.get(id).map(Vec::as_slice).unwrap_or_default();
    for edge in model
        .edges()
        .filter(|edge| edge.style.curvature != 0.0 && edge.source != edge.target)
    {
        let (Some(source), Some(target)) = (rects.get(&edge.source), rects.get(&edge.target))
        else {
            continue;
        };
        // The strongest candidate bow bounds where the curve can go.
        let reach = source.union(*target).inflate(
            source.center().distance(target.center()) * 3.0 * edge.style.curvature.abs() + 24.0,
        );
        let obstacles = rects
            .iter()
            .filter(|(id, rect)| {
                **id != edge.source && **id != edge.target && rect.intersects(reach)
            })
            .map(|(_, rect)| *rect)
            .collect::<Vec<_>>();
        let offset = offsets.get(&edge.id).copied().unwrap_or_default();
        let busy = crate::BusySides {
            source: sides(&edge.source),
            target: sides(&edge.target),
        };
        if let Some(route) =
            crate::route_around(*source, *target, &edge.style, offset, &obstacles, busy)
        {
            routed.insert(edge.id.clone(), route);
        }
    }
    routed
}

type VisibilityRevision = (u64, IndexSet<NodeId>, Option<NodeId>, Option<NodeId>);

#[derive(Clone, Default, PartialEq)]
struct CanvasVisibility {
    region: Option<(Rect, f64)>,
    data: Option<VisibilityRevision>,
    visible: IndexSet<NodeId>,
    detail: NodeDetail,
}

#[allow(non_snake_case)]
pub fn GraphCanvas<N: Clone + PartialEq + 'static, E: Clone + PartialEq + 'static>(
    props: GraphCanvasProps<N, E>,
) -> Element {
    let fallback = use_graph_controller();
    let mut controller = props.controller.unwrap_or(fallback);
    let model = props.model;
    let mut pipeline = use_signal(|| {
        let mut pipeline = CanvasPipeline::new(
            &model.peek(),
            props.layout.clone(),
            *controller.revision.peek(),
            *controller.reset_revision.peek(),
            controller.arrangement.peek().as_ref(),
        );
        if props.config.preserve_viewport {
            pipeline.preserve_viewport();
        }
        // Only this canvas restores the view handed over for its mount.
        pipeline.restore = controller.pending_restore.peek().as_ref().copied();
        pipeline
    });
    let mut gesture = use_signal(GestureState::default);
    let mut focused = use_signal(|| props.config.activation == Activation::Always);
    let mut marquee = use_signal(|| None::<Rect>);
    let mut connection = use_signal(|| None::<(NodeId, crate::EdgeSide, Point)>);
    let mut connect_target = use_signal(|| None::<NodeId>);
    // The node holding keyboard focus; culling keeps it mounted.
    let mut focused_node = use_signal(|| None::<NodeId>);
    let mut guides = use_signal(Vec::<Guide>::new);
    let mut alt_pressed = use_signal(|| false);
    let mut mounted = use_signal(|| None::<Rc<MountedData>>);
    let config = props.config.clone();
    let apply = use_callback(move |input| {
        apply_canvas_input(pipeline, model, controller, config.clone(), input);
    });
    let on_interaction_change = props.on_interaction_change;
    let cancel = use_callback(move |()| {
        let was_active = gesture.peek().is_manipulating();
        for effect in gesture.write().cancel() {
            if let GestureEffect::ReleasePointer(pointer) = effect {
                platform::release(mounted.peek().as_deref(), pointer);
            }
        }
        let original: Vec<_> = pipeline
            .peek()
            .state
            .drag_group
            .iter()
            .map(|(id, position)| (id.clone(), *position))
            .collect();
        if !original.is_empty() {
            apply.call(CanvasInput::Gesture(GestureEffect::NodesDragEnd(original)));
        }
        marquee.set(None);
        connection.set(None);
        connect_target.set(None);
        guides.set(Vec::new());
        if was_active && let Some(callback) = on_interaction_change {
            callback.call(false);
        }
    });
    use_effect(move || {
        let _revision = (controller.cancel_revision)();
        cancel.call(());
    });
    use_effect(move || apply.call(CanvasInput::Start));
    let layout = props.layout.clone();
    let config = props.config.clone();
    use_effect(use_reactive!(|layout, config| {
        let _ = config; // Config changes must synchronize controller limits as well.
        let _current = model.read();
        let revision = (controller.revision)();
        let reset_revision = (controller.reset_revision)();
        apply.call(CanvasInput::ModelChanged);
        apply.call(CanvasInput::LayoutChanged(layout.clone(), revision));
        apply.call(CanvasInput::ResetPositions(reset_revision));
    }));
    use_effect(move || {
        let selected = controller.selection.read().clone();
        if pipeline.peek().state.selection != selected {
            pipeline.write().state.selection = selected;
        }
    });
    // Connections added or restyled without moving any node still change
    // what framing must include.
    use_effect(move || {
        let bounds = curved_edge_bounds(&model.read(), &controller.geometry.peek().settled);
        if controller.geometry.peek().edge_bounds != bounds {
            controller.geometry.write().edge_bounds = bounds;
        }
    });
    let offsets = use_memo(move || parallel_edge_offsets(&model.read()));
    let routed =
        use_memo(move || routed_edges(&model.read(), &pipeline.read().rects, &offsets.read()));
    let mut visibility = use_signal(|| CanvasVisibility {
        visible: model.peek().nodes().map(|node| node.id.clone()).collect(),
        ..CanvasVisibility::default()
    });
    let cull_config = props.config.clone();
    use_effect(use_reactive!(|cull_config| {
        let viewport = *controller.viewport.read();
        let geometry = controller.geometry.read();
        let pipeline = pipeline.read();
        let mut next = visibility.peek().clone();
        let view = viewport.visible_world_rect(geometry.container);
        let recull = next
            .region
            .is_none_or(|(rect, zoom)| state::needs_recull(rect, view, zoom, viewport.zoom));
        if recull {
            next.region = Some((view.inflate(cull_config.cull_margin), viewport.zoom));
        }
        let focused_node = focused_node.read().clone();
        let data = (
            pipeline.revision,
            pipeline.state.selection.clone(),
            pipeline.state.dragging.clone(),
            focused_node.clone(),
        );
        if recull || next.data.as_ref() != Some(&data) {
            let region = next.region.map(|(rect, _)| rect).unwrap_or(view);
            next.visible = state::visible_nodes(
                &pipeline.rects,
                region,
                &pipeline.state.selection,
                pipeline.state.dragging.as_ref(),
                focused_node.as_ref(),
            );
            next.data = Some(data);
        }
        next.detail = state::detail_for_zoom(
            viewport.zoom,
            cull_config.full_detail_zoom,
            cull_config.compact_detail_zoom,
        );
        if *visibility.peek() != next {
            visibility.set(next);
        }
    }));
    let callbacks = props.clone();
    let dispatch = use_callback(move |(event, origin): (GestureInput, Point)| {
        controller.geometry.write().origin = origin;
        gesture.write().marquee_enabled = callbacks.config.marquee_selection;
        gesture.write().node_drag_disabled = !callbacks.config.draggable_nodes;
        let pointer_world = match &event {
            GestureInput::PointerMove { client, .. } | GestureInput::PointerUp { client, .. } => {
                Some(
                    controller
                        .viewport()
                        .screen_to_world(Point::new(client.x - origin.x, client.y - origin.y)),
                )
            }
            _ => None,
        };
        let position_rects = pipeline.peek().rects.clone();
        let positions = |id: &NodeId| position_rects.get(id).map(|rect| rect.origin);
        let context = GestureContext {
            viewport: controller.viewport(),
            container_origin: origin,
            limits: callbacks.config.limits,
            wheel_mode: callbacks.config.wheel_mode,
            node_position: &positions,
        };
        let was_active = gesture.peek().is_manipulating();
        let effects = gesture.write().handle(event, &context);
        let active = gesture.peek().is_manipulating();
        for mut effect in effects {
            if let GestureEffect::NodeDragStart(id) = &effect {
                let selection = pipeline.peek().state.selection.clone();
                let ids: Vec<_> = if callbacks.config.multi_drag && selection.contains(id) {
                    selection.iter().cloned().collect()
                } else {
                    vec![id.clone()]
                };
                let group = ids
                    .into_iter()
                    .filter_map(|id| positions(&id).map(|position| (id, position)))
                    .collect();
                pipeline.write().state.drag_group = group;
                // Keep the selection stable while dragging a selected group.
            }
            if let GestureEffect::NodeDragMove { node, position }
            | GestureEffect::NodeDragEnd { node, position } = &effect
            {
                let is_end = matches!(effect, GestureEffect::NodeDragEnd { .. });
                let snapshot = pipeline.peek();
                let base = snapshot
                    .state
                    .drag_group
                    .get(node)
                    .copied()
                    .unwrap_or(*position);
                let mut delta = Point::new(position.x - base.x, position.y - base.y);
                if !*alt_pressed.peek()
                    && (callbacks.config.snap_threshold.is_some()
                        || callbacks.config.snap_grid.is_some())
                {
                    let others: Vec<_> = snapshot
                        .rects
                        .iter()
                        .filter(|(id, _)| {
                            !snapshot.state.drag_group.contains_key(*id)
                                && visibility.peek().visible.contains(*id)
                        })
                        .map(|(_, rect)| *rect)
                        .collect();
                    if let Some(rect) = snapshot.rects.get(node) {
                        let dragged = Rect::new(*position, rect.size);
                        let (adjustment, next_guides) = snap(
                            dragged,
                            &others,
                            callbacks.config.snap_threshold.unwrap_or(0.0),
                            callbacks.config.snap_grid,
                        );
                        delta.x += adjustment.x;
                        delta.y += adjustment.y;
                        guides.set(next_guides);
                    }
                } else if !guides.peek().is_empty() {
                    guides.set(Vec::new());
                }
                let nodes: Vec<_> = snapshot
                    .state
                    .drag_group
                    .iter()
                    .map(|(id, point)| {
                        (id.clone(), Point::new(point.x + delta.x, point.y + delta.y))
                    })
                    .collect();
                drop(snapshot);
                if !nodes.is_empty() {
                    effect = if is_end {
                        GestureEffect::NodesDragEnd(nodes)
                    } else {
                        GestureEffect::NodesDragMove(nodes)
                    };
                }
            }
            if matches!(
                &effect,
                GestureEffect::NodeDragStart(_)
                    | GestureEffect::NodeDragMove { .. }
                    | GestureEffect::NodeDragEnd { .. }
                    | GestureEffect::NodesDragMove(_)
                    | GestureEffect::NodesDragEnd(_)
                    | GestureEffect::NodeClick { .. }
                    | GestureEffect::BackgroundClick { .. }
            ) {
                apply.call(CanvasInput::Gesture(effect.clone()));
            }
            match effect {
                GestureEffect::SetViewport(viewport) => {
                    controller.set_viewport(viewport);
                    controller.mark_manual_view();
                }
                GestureEffect::NodeDragStart(_) | GestureEffect::NodeDragMove { .. } => {}
                GestureEffect::NodesDragMove(nodes) => {
                    if let (Some(callback), Some(pointer)) = (callbacks.on_drag_move, pointer_world)
                    {
                        callback.call((nodes.iter().map(|(id, _)| id.clone()).collect(), pointer));
                    }
                }
                GestureEffect::NodesDragEnd(nodes) => {
                    guides.set(Vec::new());
                    if let Some(callback) = callbacks.on_node_moved {
                        for node in &nodes {
                            callback.call(node.clone());
                        }
                    }
                    if let Some(callback) = callbacks.on_nodes_moved {
                        callback.call(nodes);
                    }
                }
                GestureEffect::BackgroundDoubleClick { world } => {
                    if let Some(callback) = callbacks.on_background_double_click {
                        callback.call(world);
                    }
                }
                GestureEffect::MarqueeMove(rect) => marquee.set(Some(rect)),
                GestureEffect::MarqueeEnd(rect) => {
                    marquee.set(None);
                    if let Some(rect) = rect {
                        let selection = marquee_hits(rect, &pipeline.peek().rects);
                        controller.set_selection(selection.clone());
                        pipeline.write().state.selection = selection.iter().cloned().collect();
                        if let Some(callback) = callbacks.on_selection_change {
                            callback.call(selection);
                        }
                    }
                }
                GestureEffect::ConnectMove {
                    source,
                    side,
                    world,
                } => {
                    let target = connection_target(&pipeline.peek().rects, &source, world);
                    if *connect_target.peek() != target {
                        connect_target.set(target);
                    }
                    connection.set(Some((source, side, world)));
                }
                GestureEffect::ConnectEnd {
                    source,
                    side,
                    world,
                } => {
                    connection.set(None);
                    connect_target.set(None);
                    let target = connection_target(&pipeline.peek().rects, &source, world);
                    if let Some(target) = target {
                        let target_side = pipeline.peek().rects.get(&target).and_then(|rect| {
                            connection_port(*rect, world, controller.viewport().zoom)
                        });
                        if let Some(callback) = callbacks.on_connect {
                            callback.call(ConnectEvent {
                                source,
                                source_side: side,
                                target,
                                target_side,
                            });
                        }
                    } else if let Some(callback) = callbacks.on_connect_to_empty {
                        callback.call((source, world));
                    }
                }
                GestureEffect::ConnectCancel => {
                    connection.set(None);
                    connect_target.set(None);
                }
                GestureEffect::NodeDragEnd { node, position } => {
                    if let Some(callback) = callbacks.on_node_moved {
                        callback.call((node, position));
                    }
                }
                GestureEffect::NodeClick { node, shift } => {
                    controller.set_selection(
                        pipeline
                            .peek()
                            .state
                            .selection
                            .iter()
                            .cloned()
                            .collect::<Vec<_>>(),
                    );
                    if let Some(callback) = callbacks.on_selection_change {
                        callback.call(pipeline.peek().state.selection.iter().cloned().collect());
                    }
                    if let Some(callback) = callbacks.on_node_click {
                        callback.call(NodeEvent { id: node, shift });
                    }
                }
                GestureEffect::NodeDoubleClick(node) => {
                    if let Some(callback) = callbacks.on_node_activate {
                        callback.call(NodeEvent {
                            id: node,
                            shift: false,
                        });
                    }
                }
                GestureEffect::BackgroundClick { world } => {
                    controller.set_selection(Vec::new());
                    if let Some(callback) = callbacks.on_selection_change {
                        callback.call(Vec::new());
                    }
                    if let Some(callback) = callbacks.on_background_click {
                        callback.call(world);
                    }
                }
                GestureEffect::CapturePointer(pointer) => {
                    platform::capture(mounted.peek().as_deref(), pointer)
                }
                GestureEffect::ReleasePointer(pointer) => {
                    platform::release(mounted.peek().as_deref(), pointer)
                }
            }
        }
        // The host may discard drop-target state when a gesture ends. Deliver
        // its final movement first so the drop still commits that target.
        if was_active != active
            && let Some(callback) = callbacks.on_interaction_change
        {
            callback.call(active);
        }
    });
    let origin = use_hook(|| Rc::new(RefCell::new(OriginCache::default())));
    let native = use_hook(|| Rc::new(RefCell::new(NativeInputs::default())));
    let resized = use_callback(move |size| {
        if pipeline.peek().container != Some(size) {
            apply.call(CanvasInput::ContainerResized(size));
        }
    });
    let origin_moved = use_callback(move |point| {
        if controller.geometry.peek().origin != point {
            controller.geometry.write().origin = point;
        }
    });
    let refresh_origin = use_callback({
        let refresh = OriginRefresh {
            cache: origin.clone(),
            inputs: native.clone(),
            mounted,
            gesture,
            resized,
            moved: origin_moved,
            dispatch,
        };
        move |()| refresh.request()
    });
    use_hook(move || Rc::new(platform::listen_for_scroll(refresh_origin)));
    let send_input = use_callback({
        let origin = origin.clone();
        let native = native.clone();
        move |event: GestureInput| {
            #[cfg(not(all(feature = "web", target_arch = "wasm32")))]
            {
                let needs_origin = gesture
                    .peek()
                    .requires_fresh_origin(&event, props.config.wheel_mode);
                let gated = native.borrow().gated();
                if needs_origin || gated {
                    native.borrow_mut().push(event);
                    if needs_origin {
                        refresh_origin.call(());
                    }
                    return;
                }
            }
            #[cfg(all(feature = "web", target_arch = "wasm32"))]
            let _ = &native;
            if OriginCache::refresh_for_input(&event, props.config.wheel_mode) {
                refresh_origin.call(());
            }
            // Ordinary pan/drag input uses the cache unless an earlier input is awaiting bounds.
            let cached = origin.borrow().point();
            dispatch.call((event, cached));
        }
    });
    let connect_start = use_callback(
        move |(node, side, event): (NodeId, crate::EdgeSide, PointerEvent)| {
            focused.set(true);
            send_input.call(input::pointer_down(
                &event,
                GestureTarget::Connect(node, side),
                platform::timestamp_ms(),
            ));
        },
    );
    use_context_provider(|| editing::ConnectContext(connect_start));
    let activate = use_callback(move |event: NodeEvent| {
        if let Some(callback) = props.on_node_activate {
            callback.call(event);
        }
    });
    let select = use_callback(move |event: NodeEvent| {
        apply.call(CanvasInput::Selection(SelectionEvent::Click {
            node: event.id,
            shift: event.shift,
        }));
        controller.set_selection(
            pipeline
                .peek()
                .state
                .selection
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
        );
        if let Some(callback) = props.on_selection_change {
            callback.call(pipeline.peek().state.selection.iter().cloned().collect());
        }
    });
    let node_pointer = use_callback(move |(id, event): (NodeId, PointerEvent)| {
        focused.set(true);
        send_input.call(input::pointer_down(
            &event,
            GestureTarget::Node(id),
            platform::timestamp_ms(),
        ));
    });
    let measure = use_callback(move |(id, size, detail): (NodeId, Size, NodeDetail)| {
        if detail == NodeDetail::Full {
            apply.call(CanvasInput::Measured { id, size, detail });
        }
    });
    use_effect(move || {
        if (controller.canvas_focus)() == 0 {
            return;
        }
        if let Some(element) = mounted.peek().clone() {
            spawn(platform::focus(element));
        }
    });
    let on_mounted = move |event: MountedEvent| {
        let element = event.data();
        mounted.set(Some(element));
        refresh_origin.call(());
    };
    let on_resize = move |event: ResizeEvent| {
        // The canvas size is ours; hosts measure their own containers.
        event.stop_propagation();
        if let Ok(size) = event.get_border_box_size() {
            apply.call(CanvasInput::ContainerResized(Size::new(
                size.width,
                size.height,
            )));
        }
        refresh_origin.call(());
    };
    let on_pointer_down = move |event: PointerEvent| {
        if props.config.activation == Activation::OnFocus && !*focused.peek() {
            focused.set(true);
            return;
        }
        send_input.call(input::pointer_down(
            &event,
            GestureTarget::Background,
            platform::timestamp_ms(),
        ));
    };
    let on_pointer_move = move |event: PointerEvent| {
        alt_pressed.set(event.modifiers().alt());
        send_input.call(input::pointer_move(&event));
    };
    let on_pointer_up = move |event: PointerEvent| {
        send_input.call(input::pointer_up(&event, platform::timestamp_ms()))
    };
    let on_pointer_cancel = move |event: PointerEvent| {
        send_input.call(GestureInput::PointerCancel {
            pointer: event.pointer_id(),
        })
    };
    let on_pointer_leave = move |event: PointerEvent| {
        #[cfg(not(all(feature = "web", target_arch = "wasm32")))]
        send_input.call(GestureInput::PointerCancel {
            pointer: event.pointer_id(),
        });
        #[cfg(all(feature = "web", target_arch = "wasm32"))]
        let _ = event;
    };
    let on_wheel = move |event: WheelEvent| {
        if props.config.wheel_mode == WheelMode::ModifierZoom
            && !event.modifiers().ctrl()
            && !event.modifiers().meta()
        {
            return;
        }
        event.prevent_default();
        // Origin refresh synchronously updates geometry before dispatch.
        // Release this read guard before invoking that callback.
        let container = controller.geometry.peek().container;
        send_input.call(input::wheel(&event, container));
    };
    let on_key_down = move |event: KeyboardEvent| {
        if props
            .on_key_down
            .is_some_and(|callback| callback.call(event.clone()))
        {
            return;
        }
        if event.key().to_string() == "Escape" {
            cancel.call(());
        }
        let handled = match keyboard_action(&event.key().to_string()) {
            Some(KeyboardAction::ClearSelection) => {
                apply.call(CanvasInput::Selection(SelectionEvent::Clear));
                controller.set_selection(Vec::new());
                controller.set_selected_edge(None);
                if let Some(callback) = props.on_selection_change {
                    callback.call(Vec::new());
                }
                true
            }
            Some(KeyboardAction::ZoomBy(factor)) => {
                controller.zoom_by(factor);
                true
            }
            Some(KeyboardAction::Fit) => {
                controller.fit_view();
                true
            }
            Some(KeyboardAction::Pan(delta)) => {
                let mut viewport = controller.viewport();
                viewport.x += delta.x;
                viewport.y += delta.y;
                controller.set_viewport(viewport);
                controller.mark_manual_view();
                true
            }
            _ => false,
        };
        if handled {
            event.prevent_default();
        }
    };
    let edge_click = use_callback(move |id: crate::EdgeId| {
        controller.set_selected_edge(Some(id.clone()));
        if let Some(callback) = props.on_edge_click {
            callback.call(id);
        }
    });
    let edge_activate = use_callback(move |id: crate::EdgeId| {
        controller.set_selected_edge(Some(id.clone()));
        if let Some(callback) = props.on_edge_activate {
            callback.call(id);
        }
    });
    // Edges share one Tab stop; arrow keys select and focus the next one.
    let mut edge_focus = use_signal(|| None::<crate::EdgeId>);
    let edge_step = use_callback(move |(id, step): (crate::EdgeId, i32)| {
        let visible = visibility.peek();
        let edges: Vec<_> = model
            .peek()
            .edges()
            .filter(|edge| state::edge_visible(&edge.source, &edge.target, &visible.visible))
            .map(|edge| edge.id.clone())
            .collect();
        drop(visible);
        let Some(next) = state::step_edge(&edges, &id, step) else {
            return;
        };
        edge_click.call(next.clone());
        edge_focus.set(Some(next));
    });
    let current = model.read();
    let pipeline = pipeline.read();
    let visibility = visibility.read();
    let offsets = offsets.read();
    let routed = routed.read();
    let edge_style = |edge: &crate::GraphEdge<E>| {
        routed
            .get(&edge.id)
            .map(|route| route.style.clone())
            .unwrap_or_else(|| edge.style.clone())
    };
    let marker = use_hook(|| format!("dxgraph-arrow-{}", dioxus::core::current_scope_id().0));
    let class = format!("dxgraph {}", props.class.as_deref().unwrap_or(""));
    let first_node = current.nodes().next().map(|node| node.id.clone());
    let selected_edge = controller.selected_edge();
    let first_edge = current
        .edges()
        .find(|edge| state::edge_visible(&edge.source, &edge.target, &visibility.visible))
        .map(|edge| edge.id.clone());
    let edge_tab_stop = selected_edge.clone().or(first_edge);
    let focused_edge = edge_focus.read().clone();
    let world = rsx! {
        svg { class: "dxgraph-edges", "aria-hidden": (!props.config.interactive_edges).to_string(),
            defs {
                marker {
                    id: marker.clone(),
                    view_box: "0 0 10 10",
                    ref_x: "9",
                    ref_y: "5",
                    marker_width: "9",
                    marker_height: "9",
                    marker_units: "userSpaceOnUse",
                    orient: "auto-start-reverse",
                    path { class: "dxgraph-arrow", d: "M 0 0 L 10 5 L 0 10 z", fill: "currentColor" }
                }
            }
            for edge in current
                .edges()
                .filter(|edge| state::edge_visible(
                    &edge.source,
                    &edge.target,
                    &visibility.visible,
                ))
            {
                if let (Some(source), Some(target)) = (
                    pipeline.rects.get(&edge.source),
                    pipeline.rects.get(&edge.target),
                )
                {
                    edges::EdgeView {
                        key: "{edge.id}",
                        id: edge.id.to_string(),
                        interactive: props.config.interactive_edges,
                        selected: selected_edge.as_ref()==Some(&edge.id),
                        on_click: edge_click,
                        on_activate: edge_activate,
                        tab_stop: edge_tab_stop.as_ref()==Some(&edge.id),
                        focus: focused_edge.as_ref()==Some(&edge.id),
                        on_step: edge_step,
                        source: *source,
                        target: *target,
                        self_loop: edge.source == edge.target,
                        style: edge_style(edge),
                        label: if props.render_edge_label.is_some() {None} else {edge.label.clone()},
                        accessible_name: props.config.interactive_edges.then(|| {
                            let name = |id: &NodeId| current.node(id).map(|node| props.node_label.call(NodeRenderContext {
                                id: node.id.clone(),
                                data: node.data.clone(),
                                selected: false,
                                detail: NodeDetail::Full,
                            })).unwrap_or_default();
                            edges::accessible_name(&name(&edge.source), &name(&edge.target), edge.label.as_deref())
                        }),
                        marker: marker.clone(),
                        offset: offsets.get(&edge.id).copied().unwrap_or_default(),
                    }
                }
            }
        }
        if let Some(render_label)=props.render_edge_label {
            for edge in current.edges().filter(|edge|state::edge_visible(&edge.source,&edge.target,&visibility.visible)) {
                if let (Some(source),Some(target))=(pipeline.rects.get(&edge.source),pipeline.rects.get(&edge.target)) {
                    {
                        let position=crate::edge_geometry_with_offset(*source,*target,&edge_style(edge),offsets.get(&edge.id).copied().unwrap_or_default(),edge.source==edge.target).label_pos;
                        let id=edge.id.clone();
                        rsx! { div { class:"dxgraph-edge-pill", style:"left:{position.x}px;top:{position.y}px", "data-selected":(controller.selected_edge().as_ref()==Some(&edge.id)).to_string(),
                            onpointerdown: move |event:PointerEvent|event.stop_propagation(),
                            onclick: {let id=id.clone();move |event:MouseEvent| {event.stop_propagation();if props.config.interactive_edges {edge_click.call(id.clone());}}},
                            ondoubleclick: move |event:MouseEvent| {event.stop_propagation();if props.config.interactive_edges {edge_activate.call(id.clone());}},
                            {render_label.call(EdgeRenderContext {id:edge.id.clone(),data:edge.data.clone(),label:edge.label.clone(),selected:controller.selected_edge().as_ref()==Some(&edge.id)})}
                        } }
                    }
                }
            }
        }
        if let Some(rect)=*marquee.read() {
            div { class: "dxgraph-marquee", style: "position:absolute;pointer-events:none;left:{rect.origin.x}px;top:{rect.origin.y}px;width:{rect.size.width}px;height:{rect.size.height}px" }
        }
        svg { class: "dxgraph-edges dxgraph-guides",
            for guide in guides.read().iter() { line { x1:guide.from.x,y1:guide.from.y,x2:guide.to.x,y2:guide.to.y } }
        }
        for graph_node in current
            .nodes()
            .filter(|node| {
                visibility.visible.contains(&node.id)
                    || pipeline.state.pending.contains(&node.id)
            })
        {
            node::NodeView {
                key: "{graph_node.id}",
                controller,
                id: graph_node.id.clone(),
                data: graph_node.data.clone(),
                position: pipeline.rects.get(&graph_node.id).map(|rect| rect.origin).unwrap_or_default(),
                size: pipeline.rects.get(&graph_node.id).map(|rect| rect.size).or_else(|| pipeline.state.measured.get(&graph_node.id).copied()).unwrap_or(graph_node.size_hint),
                selected: pipeline.state.selection.contains(&graph_node.id),
                hidden: pipeline.state.pending.contains(&graph_node.id),
                detail: if pipeline.state.pending.contains(&graph_node.id) { NodeDetail::Full } else { visibility.detail },
                label: props
                    .node_label
                    .call(NodeRenderContext {
                        id: graph_node.id.clone(),
                        data: graph_node.data.clone(),
                        selected: pipeline.state.selection.contains(&graph_node.id),
                        detail: visibility.detail,
                    }),
                render_node: props.render_node,
                render_minimal: props.render_minimal,
                on_pointer: node_pointer,
                on_measure: measure,
                on_activate: activate,
                on_select: select,
                on_focus: move |(id, entered): (NodeId, bool)| {
                    if entered {
                        focused_node.set(Some(id));
                    } else if focused_node.peek().as_ref() == Some(&id) {
                        focused_node.set(None);
                    }
                },
                host_key_down: props.on_key_down,
                draggable: props.config.draggable_nodes,
                tab_stop: if pipeline.state.selection.is_empty() { first_node.as_ref() == Some(&graph_node.id) } else { pipeline.state.selection.contains(&graph_node.id) },
                connect_target: connect_target.read().as_ref() == Some(&graph_node.id),
                dragging: pipeline.state.dragging.as_ref() == Some(&graph_node.id),
            }
        }
        // Cross-links that cannot avoid the nodes between their ends are
        // drawn again above them (the interactive copy stays below).
        svg { class: "dxgraph-edges dxgraph-edges--over", "aria-hidden": "true",
            for edge in current.edges().filter(|edge| {
                routed.get(&edge.id).is_some_and(|route| !route.clear)
                    && state::edge_visible(&edge.source, &edge.target, &visibility.visible)
            }) {
                if let (Some(source), Some(target)) = (pipeline.rects.get(&edge.source), pipeline.rects.get(&edge.target)) {
                    edges::EdgeView {
                        key: "{edge.id}",
                        id: edge.id.to_string(),
                        selected: controller.selected_edge().as_ref() == Some(&edge.id),
                        source: *source,
                        target: *target,
                        self_loop: false,
                        style: edge_style(edge),
                        label: None,
                        marker: marker.clone(),
                        offset: offsets.get(&edge.id).copied().unwrap_or_default(),
                    }
                }
            }
        }
        // The live connection is drawn above the nodes it crosses.
        svg { class: "dxgraph-edges dxgraph-connection",
            if let Some((source,side,point))=connection.read().as_ref() {
                if let Some(rect)=pipeline.rects.get(source) {
                    path {
                        class: "dxgraph-connect-preview",
                        "data-snapped": connect_target.read().is_some().to_string(),
                        d: connection_preview(*rect, *side, *point, connect_target.read().as_ref().and_then(|id| pipeline.rects.get(id)).copied(), controller.viewport().zoom),
                        fill: "none",
                    }
                }
            }
        }
    };
    rsx! {
        div {
            class,
            role: "application",
            aria_label: props.config.aria_label.clone(),
            "data-connecting": connection.read().is_some().to_string(),
            tabindex: 0,
            style: if props.config.activation == Activation::OnFocus && !*focused.read() { "touch-action: pan-y" } else { "touch-action: none" },
            onfocus: move |_| focused.set(true),
            onblur: move |_| { if props.config.activation == Activation::OnFocus { focused.set(false); } },
            onmounted: on_mounted,
            onresize: on_resize,
            onpointerdown: on_pointer_down,
            onpointermove: on_pointer_move,
            onpointerup: on_pointer_up,
            onpointercancel: on_pointer_cancel,
            onpointerleave: on_pointer_leave,
            onwheel: on_wheel,
            onkeydown: on_key_down,
            GraphBackground { viewport: controller.viewport }
            background::GraphWorld { viewport: controller.viewport, children: world }
            div { class: "dxgraph-overlay", {props.children} }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::pipeline::{calculate_layout, rects};
    use super::state::CanvasState;
    use super::*;
    use crate::GraphNode;

    #[test]
    fn connection_drops_snap_to_nearby_target_ports() {
        let target = Rect::new(Point::new(100.0, 100.0), Size::new(200.0, 60.0));
        assert_eq!(
            connection_port(target, Point::new(202.0, 104.0), 1.0),
            Some(crate::EdgeSide::Top)
        );
        assert_eq!(
            connection_port(target, Point::new(96.0, 128.0), 1.0),
            Some(crate::EdgeSide::Left)
        );
        // The middle of the idea attaches automatically.
        assert_eq!(connection_port(target, Point::new(200.0, 130.0), 1.0), None);
        // Zoomed out, the snap radius keeps a usable size on screen.
        assert_eq!(
            connection_port(target, Point::new(200.0, 130.0), 0.2),
            Some(crate::EdgeSide::Top)
        );
    }

    // Incremental (force) layouts keep placed nodes; structural layouts recompute.
    #[test]
    fn incremental_layout_keeps_old_nodes_and_places_new_nodes_without_overlap() {
        let incremental = LayoutConfig::Force(Default::default());
        let mut model = GraphModel::<(), ()>::default();
        model.insert_node(GraphNode::new("root", ())).unwrap();
        let mut first = GraphNode::new("first", ());
        first.layout_parent = Some("root".into());
        model.insert_node(first).unwrap();
        let mut state = CanvasState::default();
        state.measured.insert(
            "root".into(),
            Size {
                width: 240.0,
                height: 70.0,
            },
        );
        state.measured.insert(
            "first".into(),
            Size {
                width: 240.0,
                height: 90.0,
            },
        );
        calculate_layout(&model, &mut state, &incremental, false);
        let previous = state.positions.clone();
        state.stable = previous.keys().cloned().collect();
        let mut second = GraphNode::new("second", ());
        second.layout_parent = Some("root".into());
        model.insert_node(second).unwrap();
        calculate_layout(&model, &mut state, &incremental, true);
        state.measured.insert(
            "second".into(),
            Size {
                width: 400.0,
                height: 130.0,
            },
        );
        calculate_layout(&model, &mut state, &incremental, true);
        for (id, point) in previous {
            assert_eq!(state.positions[&id], point);
        }
        let rects = rects(&model, &state);
        let new = rects[&NodeId::from("second")];
        for id in ["root", "first"] {
            assert!(!new.intersects(rects[&NodeId::from(id)]));
        }
    }

    #[test]
    fn partial_measurements_use_received_sizes_and_hints_together() {
        let mut model = GraphModel::<(), ()>::default();
        model.insert_node(GraphNode::new("a", ())).unwrap();
        model.insert_node(GraphNode::new("b", ())).unwrap();
        let mut state = CanvasState::default();
        state.measured.insert(
            "a".into(),
            Size {
                width: 400.0,
                height: 200.0,
            },
        );
        calculate_layout(&model, &mut state, &LayoutConfig::default(), false);
        let rects = rects(&model, &state);
        assert_eq!(rects[&NodeId::from("a")].size.width, 400.0);
        assert_eq!(
            rects[&NodeId::from("b")].size,
            model.node(&"b".into()).unwrap().size_hint
        );
        assert!(!rects[&NodeId::from("a")].intersects(rects[&NodeId::from("b")]));
    }
    #[test]
    fn parallel_and_opposite_edges_have_centred_distinct_lanes() {
        let mut model = GraphModel::<(), ()>::default();
        model.insert_node(GraphNode::new("a", ())).unwrap();
        model.insert_node(GraphNode::new("b", ())).unwrap();
        model
            .insert_edge(crate::GraphEdge::new("ab", "a", "b", ()))
            .unwrap();
        model
            .insert_edge(crate::GraphEdge::new("ba", "b", "a", ()))
            .unwrap();
        let offsets = parallel_edge_offsets(&model);
        assert_eq!(offsets[&crate::EdgeId::from("ab")], -1);
        assert_eq!(offsets[&crate::EdgeId::from("ba")], -1);
        let a = Rect::new(Point::default(), Size::new(100.0, 60.0));
        let b = Rect::new(Point::new(300.0, 0.0), a.size);
        let forward = crate::edge_geometry_with_offset(
            a,
            b,
            &crate::EdgeStyle::default(),
            offsets[&crate::EdgeId::from("ab")],
            false,
        );
        let reverse = crate::edge_geometry_with_offset(
            b,
            a,
            &crate::EdgeStyle::default(),
            offsets[&crate::EdgeId::from("ba")],
            false,
        );
        assert_ne!(
            forward.label_pos, reverse.label_pos,
            "opposite edges must occupy different physical lanes"
        );
        model
            .insert_edge(crate::GraphEdge::new("ab2", "a", "b", ()))
            .unwrap();
        let offsets = parallel_edge_offsets(&model);
        assert_eq!(offsets[&crate::EdgeId::from("ab")], -2);
        assert_eq!(offsets[&crate::EdgeId::from("ba")], 0);
        assert_eq!(offsets[&crate::EdgeId::from("ab2")], 2);
    }

    #[test]
    fn container_resizes_refit_framed_views_and_keep_manual_centres() {
        let bounds = Rect::new(Point::default(), Size::new(300.0, 200.0));
        let small = Size::new(400.0, 440.0);
        let large = Size::new(900.0, 440.0);
        assert_eq!(
            container_reframe(None, large, false, Some(bounds)),
            Reframe::None,
            "the first measurement waits for the layout's own Fit"
        );
        assert_eq!(
            container_reframe(Some(small), large, false, Some(bounds)),
            Reframe::Fit(bounds)
        );
        assert_eq!(
            container_reframe(Some(small), large, true, Some(bounds)),
            Reframe::KeepCenter(small)
        );
        assert_eq!(
            container_reframe(Some(large), Size::default(), false, Some(bounds)),
            Reframe::KeepCenter(large),
            "a hidden canvas never fits to a collapsed size"
        );
        assert_eq!(
            container_reframe(Some(small), large, false, None),
            Reframe::KeepCenter(small)
        );
    }

    /// Regression: leaving fullscreen remounts the inline canvas with the
    /// shared controller still at the fullscreen size. The saved inline view
    /// must win over re-framing, which used unmeasured bounds and showed a
    /// 403×438 frame at 18% instead of the 31% it had before.
    #[test]
    fn remount_after_fullscreen_restores_the_saved_inline_view() {
        let bounds = Rect::new(Point::default(), Size::new(1200.0, 900.0));
        let fullscreen = Size::new(1100.0, 797.0);
        for (inline, zoom) in [
            (Size::new(403.0, 438.0), 0.31),
            (Size::new(279.0, 438.0), 0.19),
            (Size::new(720.0, 440.0), 0.39),
            (Size::new(1180.0, 440.0), 0.42),
        ] {
            let viewport = Viewport {
                x: 12.0,
                y: 40.0,
                zoom,
            };
            for manual in [false, true] {
                let saved = controls::SavedView {
                    viewport,
                    container: inline,
                    manual,
                };
                assert_eq!(
                    resize_reframe(Some(saved), Some(fullscreen), inline, false, Some(bounds)),
                    Reframe::Restore(viewport, manual)
                );
            }
            // Without a saved view the resize re-frames as before.
            assert_eq!(
                resize_reframe(None, Some(fullscreen), inline, false, Some(bounds)),
                Reframe::Fit(bounds)
            );
        }
    }

    #[test]
    fn self_loop_offsets_have_distinct_radii() {
        let mut model = GraphModel::<(), ()>::default();
        model.insert_node(GraphNode::new("a", ())).unwrap();
        for id in ["one", "two", "three"] {
            model
                .insert_edge(crate::GraphEdge::new(id, "a", "a", ()))
                .unwrap();
        }
        let offsets = parallel_edge_offsets(&model);
        assert_eq!(offsets.values().copied().collect::<Vec<_>>(), [0, 2, 4]);
    }

    #[test]
    fn framing_includes_cross_link_bows() {
        let mut model = GraphModel::<(), ()>::default();
        for id in ["a", "b"] {
            model.insert_node(GraphNode::new(id, ())).unwrap();
        }
        let mut rects = IndexMap::new();
        rects.insert(
            NodeId::from("a"),
            Rect::new(Point::new(0.0, 0.0), Size::new(100.0, 40.0)),
        );
        rects.insert(
            NodeId::from("b"),
            Rect::new(Point::new(400.0, 0.0), Size::new(100.0, 40.0)),
        );
        let nodes = rects.values().copied().reduce(|a, b| a.union(b)).unwrap();
        // A straight hierarchy edge stays between the nodes.
        model
            .insert_edge(crate::GraphEdge::new("tree", "a", "b", ()))
            .unwrap();
        assert_eq!(content_bounds(&model, &rects), Some(nodes));
        // A bowed cross-link loops above or below them.
        let mut link = crate::GraphEdge::new("link", "b", "a", ());
        link.style.curvature = 0.25;
        model.insert_edge(link).unwrap();
        let bounds = content_bounds(&model, &rects).unwrap();
        assert!(bounds.contains_rect(nodes));
        assert!(bounds.size.height > nodes.size.height + 40.0, "{bounds:?}");
    }
}
