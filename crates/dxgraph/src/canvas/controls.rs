use crate::{NodeId, Point, Rect, Size, Viewport, ViewportLimits};
use dioxus::prelude::*;
use indexmap::{IndexMap, IndexSet};
use std::rc::Rc;

const FIT_PADDING: f64 = 40.0;

/// Screen-space margins covered by host chrome (toolbars, overlays) that Fit
/// keeps content clear of. They are added to the regular fit padding.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FitInsets {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}
impl FitInsets {
    pub const fn new(top: f64, right: f64, bottom: f64, left: f64) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }
    /// Drop insets that would leave less than half the container for content.
    fn effective(self, container: Size) -> Self {
        let valid = |value: f64| {
            if value.is_finite() {
                value.max(0.0)
            } else {
                0.0
            }
        };
        let mut insets = Self::new(
            valid(self.top),
            valid(self.right),
            valid(self.bottom),
            valid(self.left),
        );
        if insets.left + insets.right > container.width / 2.0 {
            insets.left = 0.0;
            insets.right = 0.0;
        }
        if insets.top + insets.bottom > container.height / 2.0 {
            insets.top = 0.0;
            insets.bottom = 0.0;
        }
        insets
    }
}

/// Fit `bounds` into `container`, keeping at least the regular fit padding on
/// every side and the larger host `insets` where chrome covers the canvas.
pub(crate) fn fit_viewport(
    bounds: Rect,
    container: Size,
    insets: FitInsets,
    min_zoom: f64,
    max_zoom: f64,
) -> Viewport {
    let insets = insets.effective(container);
    let top = insets.top.max(FIT_PADDING);
    let right = insets.right.max(FIT_PADDING);
    let bottom = insets.bottom.max(FIT_PADDING);
    let left = insets.left.max(FIT_PADDING);
    let available = Size::new(
        (container.width - left - right).max(1.0),
        (container.height - top - bottom).max(1.0),
    );
    let viewport = Viewport::fit(bounds, available, 0.0, min_zoom, max_zoom);
    Viewport {
        x: viewport.x + left,
        y: viewport.y + top,
        ..viewport
    }
}

/// Automatic framing: fit `bounds` like [`fit_viewport`], but never below the
/// readable `floor` zoom. Content that no longer fits is centred on `anchor`
/// (the selection or the root idea) per axis, clamped so the view never pans
/// past the content edge into empty space.
pub(crate) fn frame_viewport(
    bounds: Rect,
    anchor: Option<Rect>,
    container: Size,
    insets: FitInsets,
    floor: Option<f64>,
    min_zoom: f64,
    max_zoom: f64,
) -> Viewport {
    let fit = fit_viewport(bounds, container, insets, min_zoom, max_zoom);
    let Some(floor) = floor
        .filter(|floor| floor.is_finite() && *floor > 0.0)
        .map(|floor| floor.clamp(min_zoom, max_zoom))
    else {
        return fit;
    };
    if fit.zoom >= floor {
        return fit;
    }
    let insets = insets.effective(container);
    let zoom = floor;
    let anchor = anchor.unwrap_or(bounds).center();
    // Per axis: the screen span available for content and the content span.
    let axis = |start: f64, end: f64, low: f64, high: f64, anchor: f64| {
        let (start, end) = (start, end.max(start + 1.0));
        if (high - low) * zoom <= end - start {
            (start + end) / 2.0 - (low + high) / 2.0 * zoom
        } else {
            let centred = (start + end) / 2.0 - anchor * zoom;
            centred.clamp(end - high * zoom, start - low * zoom)
        }
    };
    Viewport {
        x: axis(
            insets.left.max(FIT_PADDING),
            container.width - insets.right.max(FIT_PADDING),
            bounds.origin.x,
            bounds.right(),
            anchor.x,
        ),
        y: axis(
            insets.top.max(FIT_PADDING),
            container.height - insets.bottom.max(FIT_PADDING),
            bounds.origin.y,
            bounds.bottom(),
            anchor.y,
        ),
        zoom,
    }
}

/// The union of all settled node rects and curved edges, or `None` without
/// nodes.
fn settled_bounds(geometry: &ControllerGeometry) -> Option<Rect> {
    let nodes = geometry
        .settled
        .values()
        .copied()
        .reduce(|a, b| a.union(b))?;
    Some(
        geometry
            .edge_bounds
            .map_or(nodes, |edges| nodes.union(edges)),
    )
}

/// The node automatic framing keeps in view: the `selected` node, else the
/// layout `root`, else the first node.
pub(crate) fn frame_anchor(
    rects: &IndexMap<NodeId, Rect>,
    selected: Option<&NodeId>,
    root: Option<&NodeId>,
) -> Option<(NodeId, Rect)> {
    selected
        .and_then(|id| rects.get_key_value(id))
        .or_else(|| root.and_then(|id| rects.get_key_value(id)))
        .or_else(|| rects.first())
        .map(|(id, rect)| (id.clone(), *rect))
}

/// A canvas view saved for a later remount, e.g. the inline view while a
/// fullscreen surface temporarily owns the shared controller.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SavedView {
    pub viewport: Viewport,
    /// The canvas size the viewport was framed for.
    pub container: Size,
    /// Whether the user had panned or zoomed the view by hand.
    pub manual: bool,
}

/// The viewport that restores `saved` in a canvas measuring `container`:
/// exactly the saved one for the same frame size, otherwise the same zoom
/// and world centre (a remount may report interim sizes before it settles).
pub(crate) fn restored_viewport(saved: SavedView, container: Size) -> Viewport {
    let dx = container.width - saved.container.width;
    let dy = container.height - saved.container.height;
    if dx.abs() < 1.0 && dy.abs() < 1.0 {
        return saved.viewport;
    }
    Viewport {
        x: saved.viewport.x + dx / 2.0,
        y: saved.viewport.y + dy / 2.0,
        ..saved.viewport
    }
}

/// Where a canvas placed its nodes under `layout`. A canvas that mounts on
/// the same controller with that layout continues the arrangement (mind map
/// branch sides, organic positions) instead of laying out from scratch, so
/// switching between an inline and a fullscreen surface never reshuffles the
/// map. Hosts may also persist it and seed it after a reload.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutArrangement {
    pub layout: crate::LayoutConfig,
    pub positions: Rc<IndexMap<NodeId, Point>>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ControllerGeometry {
    pub origin: Point,
    pub revision: u64,
    pub container: Size,
    /// The rects on screen, including interpolated layout animation frames.
    pub rects: Rc<IndexMap<NodeId, Rect>>,
    /// The final rects of the current layout, never an animation frame.
    pub settled: Rc<IndexMap<NodeId, Rect>>,
    /// The extent of curved edges (cross-links) in the settled layout, which
    /// may loop beyond the nodes.
    pub edge_bounds: Option<Rect>,
    /// The layout root automatic framing anchors on without a selection.
    pub root: Option<NodeId>,
    pub limits: ViewportLimits,
    pub fit_max_zoom: Option<f64>,
    pub fit_insets: FitInsets,
    /// The readable zoom floor of automatic framing ([`frame_viewport`]).
    pub frame_min_zoom: Option<f64>,
}
impl ControllerGeometry {
    /// The highest zoom Fit and automatic framing use.
    fn fit_zoom_cap(&self) -> f64 {
        self.fit_max_zoom
            .filter(|zoom| zoom.is_finite() && *zoom > 0.0)
            .map(|zoom| zoom.max(self.limits.min_zoom).min(self.limits.max_zoom))
            .unwrap_or(self.limits.max_zoom)
    }
}
impl Default for ControllerGeometry {
    fn default() -> Self {
        Self {
            origin: Point::default(),
            revision: 0,
            container: Size {
                width: 800.0,
                height: 600.0,
            },
            rects: Rc::default(),
            settled: Rc::default(),
            edge_bounds: None,
            root: None,
            limits: ViewportLimits::default(),
            fit_max_zoom: None,
            fit_insets: FitInsets::default(),
            frame_min_zoom: None,
        }
    }
}

/// Controls the viewport and transient canvas drag positions.
/// Callers own durable positions configured through model pins.
#[derive(Clone, Copy, PartialEq)]
pub struct GraphController {
    pub(crate) viewport: Signal<Viewport>,
    pub(crate) geometry: Signal<ControllerGeometry>,
    pub(crate) revision: Signal<u64>,
    pub(crate) reset_revision: Signal<u64>,
    pub(crate) selection: Signal<IndexSet<NodeId>>,
    pub(crate) selected_edge: Signal<Option<crate::EdgeId>>,
    pub(crate) animation: Signal<u64>,
    pub(crate) focus_request: Signal<Option<NodeId>>,
    pub(crate) cancel_revision: Signal<u64>,
    /// The user panned or zoomed since the last Fit; container resizes then
    /// keep the view centre instead of re-framing the content.
    pub(crate) manual_view: Signal<bool>,
    /// Bumped to move keyboard focus back to the canvas element.
    pub(crate) canvas_focus: Signal<u64>,
    /// A view the next mounted canvas restores instead of re-framing.
    pub(crate) pending_restore: Signal<Option<SavedView>>,
    /// The arrangement the last settled canvas showed, for the next mount.
    pub(crate) arrangement: Signal<Option<LayoutArrangement>>,
}
pub fn use_graph_controller() -> GraphController {
    GraphController {
        viewport: use_signal(Viewport::default),
        geometry: use_signal(ControllerGeometry::default),
        revision: use_signal(|| 0),
        reset_revision: use_signal(|| 0),
        selection: use_signal(IndexSet::new),
        selected_edge: use_signal(|| None),
        animation: use_signal(|| 0),
        focus_request: use_signal(|| None),
        cancel_revision: use_signal(|| 0),
        manual_view: use_signal(|| false),
        canvas_focus: use_signal(|| 0),
        pending_restore: use_signal(|| None),
        arrangement: use_signal(|| None),
    }
}
impl GraphController {
    /// Cancel an active drag, connection, or marquee without saving a change.
    pub fn cancel_interaction(mut self) {
        let revision = *self.cancel_revision.peek() + 1;
        self.cancel_revision.set(revision);
    }
    pub fn positions(self) -> IndexMap<NodeId, Point> {
        self.geometry
            .peek()
            .rects
            .iter()
            .map(|(id, rect)| (id.clone(), rect.origin))
            .collect()
    }
    pub fn rects(self) -> Rc<IndexMap<NodeId, Rect>> {
        self.geometry.read().rects.clone()
    }
    /// The current node rectangles without subscribing to geometry.
    pub fn peek_rects(self) -> Rc<IndexMap<NodeId, Rect>> {
        self.geometry.peek().rects.clone()
    }
    /// The current rectangle of one node without subscribing to geometry.
    pub fn peek_rect(self, id: &NodeId) -> Option<Rect> {
        self.geometry.peek().rects.get(id).copied()
    }
    pub fn selection(self) -> Vec<NodeId> {
        self.selection.read().iter().cloned().collect()
    }
    pub fn set_selection(mut self, nodes: impl IntoIterator<Item = NodeId>) {
        self.selection.set(nodes.into_iter().collect());
    }
    pub fn select_all(self) {
        self.set_selection(
            self.geometry
                .peek()
                .rects
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
        );
    }
    /// Focus the mounted node wrapper, including a node mounted after centering.
    pub fn focus_node(mut self, id: NodeId) {
        self.focus_request.set(Some(id));
    }
    pub fn selected_edge(self) -> Option<crate::EdgeId> {
        self.selected_edge.read().clone()
    }
    pub fn set_selected_edge(mut self, edge: Option<crate::EdgeId>) {
        self.selected_edge.set(edge);
    }
    pub fn animate_to(mut self, target: Viewport) {
        if super::editing::reduced_motion() {
            self.set_viewport(target);
            return;
        }
        let token = *self.animation.peek() + 1;
        self.animation.set(token);
        let start = self.viewport();
        spawn(async move {
            let began = super::platform::timestamp_ms();
            loop {
                if *self.animation.peek() != token {
                    break;
                }
                let t =
                    (super::platform::timestamp_ms().saturating_sub(began) as f64 / 220.0).min(1.0);
                let t = super::editing::ease_out(t);
                let limits = self.geometry.peek().limits;
                self.viewport.set(Viewport {
                    x: start.x + (target.x - start.x) * t,
                    y: start.y + (target.y - start.y) * t,
                    zoom: (start.zoom + (target.zoom - start.zoom) * t)
                        .clamp(limits.min_zoom, limits.max_zoom),
                });
                if t >= 1.0 {
                    break;
                }
                dioxus_sdk_time::sleep(std::time::Duration::from_millis(16)).await;
            }
        });
    }

    /// Return the measured canvas size in screen pixels.
    pub fn container_size(self) -> Size {
        self.geometry.peek().container
    }

    pub fn viewport(self) -> Viewport {
        *self.viewport.peek()
    }
    /// The viewport, subscribing the caller to every pan and zoom.
    pub fn read_viewport(self) -> Viewport {
        *self.viewport.read()
    }
    /// Whether the user has panned or zoomed since the content was last fitted.
    pub fn is_manual_view(self) -> bool {
        *self.manual_view.peek()
    }
    /// Record a user-driven pan or zoom: container resizes stop re-framing.
    pub fn mark_manual_view(mut self) {
        if !*self.manual_view.peek() {
            self.manual_view.set(true);
        }
    }
    pub(crate) fn clear_manual_view(mut self) {
        if *self.manual_view.peek() {
            self.manual_view.set(false);
        }
    }
    /// Return keyboard focus to the canvas, e.g. after a toolbar action.
    pub fn focus_canvas(mut self) {
        let next = self.canvas_focus.peek().wrapping_add(1);
        self.canvas_focus.set(next);
    }
    pub fn set_viewport(mut self, viewport: Viewport) {
        let next = *self.animation.peek() + 1;
        self.animation.set(next);
        let limits = self.geometry.peek().limits;
        self.viewport.set(Viewport {
            zoom: viewport.zoom.clamp(limits.min_zoom, limits.max_zoom),
            ..viewport
        });
    }
    pub fn zoom_by(mut self, factor: f64) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let geometry = self.geometry.peek();
        let mut viewport = *self.viewport.peek();
        viewport = viewport.zoom_at(
            Point {
                x: geometry.container.width / 2.0,
                y: geometry.container.height / 2.0,
            },
            factor,
            geometry.limits.min_zoom,
            geometry.limits.max_zoom,
        );
        drop(geometry);
        self.viewport.set(viewport);
        self.mark_manual_view();
    }
    /// Animate to `zoom` around the centre of `focus`, or around the centre of
    /// the canvas when no node is given.
    pub fn zoom_to(self, zoom: f64, focus: Option<&NodeId>) {
        let geometry = self.geometry.peek();
        let current = *self.viewport.peek();
        let center = Point::new(
            geometry.container.width / 2.0,
            geometry.container.height / 2.0,
        );
        let world = focus
            .and_then(|id| geometry.rects.get(id))
            .map(|rect| rect.center())
            .unwrap_or_else(|| current.screen_to_world(center));
        let zoom = zoom.clamp(geometry.limits.min_zoom, geometry.limits.max_zoom);
        drop(geometry);
        self.mark_manual_view();
        self.animate_to(Viewport {
            x: center.x - world.x * zoom,
            y: center.y - world.y * zoom,
            zoom,
        });
    }
    /// The viewport that fits all nodes, or `None` before any node is measured.
    pub fn fit_target(self) -> Option<Viewport> {
        let bounds = settled_bounds(&self.geometry.peek())?;
        Some(self.fit_bounds(bounds))
    }
    /// The viewport that fits `bounds` within the container and host insets.
    pub(crate) fn fit_bounds(self, bounds: Rect) -> Viewport {
        let geometry = self.geometry.peek();
        fit_viewport(
            bounds,
            geometry.container,
            geometry.fit_insets,
            geometry.limits.min_zoom,
            geometry.fit_zoom_cap(),
        )
    }
    /// The node automatic framing keeps in view among `rects`: the selection,
    /// else the layout root.
    pub(crate) fn frame_anchor_in(self, rects: &IndexMap<NodeId, Rect>) -> Option<(NodeId, Rect)> {
        let selection = self.selection.peek();
        let geometry = self.geometry.peek();
        frame_anchor(rects, selection.first(), geometry.root.as_ref())
    }
    /// Automatic framing of `bounds`: like [`Self::fit_bounds`], but kept at
    /// a readable zoom, centred on the selection or root when content is cut.
    /// The anchor is looked up in the settled layout, never in an animation
    /// frame that still shows the previous layout.
    pub(crate) fn frame_bounds(self, bounds: Rect) -> Viewport {
        let settled = self.geometry.peek().settled.clone();
        let anchor = self.frame_anchor_in(&settled).map(|(_, rect)| rect);
        self.frame_bounds_at(bounds, anchor)
    }
    /// [`Self::frame_bounds`] kept around the world rect `anchor`.
    fn frame_bounds_at(self, bounds: Rect, anchor: Option<Rect>) -> Viewport {
        let geometry = self.geometry.peek();
        frame_viewport(
            bounds,
            anchor,
            geometry.container,
            geometry.fit_insets,
            geometry.frame_min_zoom,
            geometry.limits.min_zoom,
            geometry.fit_zoom_cap(),
        )
    }
    /// The automatically framed viewport, or `None` before any node exists.
    pub fn frame_target(self) -> Option<Viewport> {
        let bounds = settled_bounds(&self.geometry.peek())?;
        Some(self.frame_bounds(bounds))
    }
    /// Automatic framing like [`Self::frame_target`], but kept around `node`
    /// (when it is laid out) instead of the selection when content is cut,
    /// e.g. the idea a group of new ideas was added to.
    pub fn frame_target_around(self, node: &NodeId) -> Option<Viewport> {
        let bounds = settled_bounds(&self.geometry.peek())?;
        let anchor = self.geometry.peek().settled.get(node).copied();
        Some(match anchor {
            Some(anchor) => self.frame_bounds_at(bounds, Some(anchor)),
            None => self.frame_bounds(bounds),
        })
    }
    /// Animate to [`Self::frame_target_around`] `node` and resume following
    /// container resizes.
    pub fn animate_frame_view_around(self, node: &NodeId) {
        if let Some(viewport) = self.frame_target_around(node) {
            self.animate_to(viewport);
            self.clear_manual_view();
        }
    }
    /// Animate to the automatic framing (readable zoom, anchored on the
    /// selection or root) and resume following container resizes.
    pub fn animate_frame_view(self) {
        if let Some(viewport) = self.frame_target() {
            self.animate_to(viewport);
            self.clear_manual_view();
        }
    }
    /// The current view and whether the user framed it by hand.
    pub fn saved_view(self) -> SavedView {
        SavedView {
            viewport: self.viewport(),
            container: self.container_size(),
            manual: self.is_manual_view(),
        }
    }
    /// Restore `view` in the next canvas that mounts with this controller,
    /// instead of re-framing it when that canvas measures itself.
    pub fn restore_view_on_mount(mut self, view: SavedView) {
        self.pending_restore.set(Some(view));
    }
    /// The arrangement the last settled canvas showed, subscribing the caller
    /// (e.g. a host persisting it across reloads).
    pub fn arrangement(self) -> Option<LayoutArrangement> {
        self.arrangement.read().clone()
    }
    /// Continue `arrangement` in the next canvas that mounts with this
    /// controller, e.g. one restored after a reload. A live arrangement of
    /// this controller always wins over a seeded one.
    pub fn seed_arrangement(mut self, arrangement: LayoutArrangement) {
        if self.arrangement.peek().is_none() {
            self.arrangement.set(Some(arrangement));
        }
    }
    pub fn fit_view(mut self) {
        if let Some(viewport) = self.fit_target() {
            self.viewport.set(viewport);
            self.clear_manual_view();
        }
    }
    /// Fit all nodes with the standard viewport animation.
    pub fn animate_fit_view(self) {
        if let Some(viewport) = self.fit_target() {
            self.animate_to(viewport);
            self.clear_manual_view();
        }
    }
    /// Smoothly center a node while keeping text at least `min_zoom` scale.
    pub fn animate_to_node(self, id: NodeId, min_zoom: f64) {
        let geometry = self.geometry.peek();
        let Some(rect) = geometry.settled.get(&id) else {
            return;
        };
        let zoom = self
            .viewport
            .peek()
            .zoom
            .max(min_zoom)
            .clamp(geometry.limits.min_zoom, geometry.limits.max_zoom);
        let center = rect.center();
        let target = Viewport {
            x: geometry.container.width / 2.0 - center.x * zoom,
            y: geometry.container.height / 2.0 - center.y * zoom,
            zoom,
        };
        drop(geometry);
        self.animate_to(target);
    }

    pub fn center_on(mut self, id: NodeId) {
        let geometry = self.geometry.peek();
        if let Some(rect) = geometry.settled.get(&id) {
            let p = rect.center();
            let zoom = self.viewport.peek().zoom;
            let viewport = Viewport {
                x: geometry.container.width / 2.0 - p.x * zoom,
                y: geometry.container.height / 2.0 - p.y * zoom,
                zoom,
            };
            drop(geometry);
            self.viewport.set(viewport);
        }
    }
    /// Return canvas-owned dragged positions to automatic layout.
    /// Explicit model pins remain fixed; callers own those durable positions.
    pub fn reset_positions(mut self) {
        let next = *self.reset_revision.peek() + 1;
        self.reset_revision.set(next);
    }
    pub fn relayout(mut self) {
        let next = *self.revision.peek() + 1;
        self.revision.set(next);
    }
}

/// Inline 24px stroke icons (Lucide geometry) so the controls need no icon crate.
fn control_icon(paths: &'static [&'static str]) -> Element {
    rsx! {
        svg {
            class: "dxgraph-control-icon",
            view_box: "0 0 24 24",
            width: "16",
            height: "16",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            "aria-hidden": "true",
            for d in paths {
                path { d: *d }
            }
        }
    }
}

const ICON_PLUS: &[&str] = &["M5 12h14", "M12 5v14"];
const ICON_MINUS: &[&str] = &["M5 12h14"];
const ICON_FIT: &[&str] = &[
    "M3 7V5a2 2 0 0 1 2-2h2",
    "M17 3h2a2 2 0 0 1 2 2v2",
    "M21 17v2a2 2 0 0 1-2 2h-2",
    "M7 21H5a2 2 0 0 1-2-2v-2",
    "M8 12h8",
    "M12 8v8",
];
const ICON_RELAYOUT: &[&str] = &[
    "M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8",
    "M21 3v5h-5",
    "M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16",
    "M8 16H3v5",
];

/// Zoom percentage shown between the zoom buttons.
pub(crate) fn zoom_label(zoom: f64) -> String {
    if zoom.is_finite() {
        format!("{}%", (zoom * 100.0).round() as i64)
    } else {
        "100%".into()
    }
}

/// Compact icon controls for zoom, fit and re-layout. Every action returns
/// keyboard focus to the canvas so shortcuts keep working after a click.
#[component]
pub fn GraphControls(
    controller: GraphController,
    /// Show the Re-layout action; hosts hide it for manual or read-only graphs.
    #[props(default = true)]
    relayout: bool,
    /// Handles Re-layout instead of [`GraphController::relayout`], e.g. for
    /// hosts that first rebalance an arrangement they persist.
    #[props(default)]
    on_relayout: Option<EventHandler<()>>,
) -> Element {
    let zoom = use_memo(move || zoom_label(controller.read_viewport().zoom));
    let act = move |action: fn(GraphController)| {
        move |_| {
            action(controller);
            controller.focus_canvas();
        }
    };
    rsx! {
        div {
            class: "dxgraph-controls",
            role: "group",
            aria_label: "Graph controls",
            dxcomp::button::Button {
                aria_label: "Zoom out",
                title: "Zoom out (−)",
                onclick: act(|controller| controller.zoom_by(1.0 / 1.2)),
                {control_icon(ICON_MINUS)}
            }
            dxcomp::button::Button {
                class: "dxgraph-controls__zoom",
                aria_label: "Reset zoom to 100%",
                title: "Reset zoom to 100% (1)",
                onclick: act(|controller| {
                    let focus = controller.selection.peek().first().cloned();
                    controller.zoom_to(1.0, focus.as_ref());
                }),
                "{zoom}"
            }
            dxcomp::button::Button {
                aria_label: "Zoom in",
                title: "Zoom in (+)",
                onclick: act(|controller| controller.zoom_by(1.2)),
                {control_icon(ICON_PLUS)}
            }
            dxcomp::button::Button {
                aria_label: "Fit graph",
                title: "Fit to view (0)",
                onclick: act(|controller| controller.animate_fit_view()),
                {control_icon(ICON_FIT)}
            }
            if relayout {
                dxcomp::button::Button {
                    aria_label: "Re-layout graph",
                    title: "Re-layout",
                    onclick: move |_| {
                        match on_relayout {
                            Some(handler) => handler.call(()),
                            None => controller.relayout(),
                        }
                        controller.focus_canvas();
                    },
                    {control_icon(ICON_RELAYOUT)}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn fit_keeps_content_clear_of_host_insets() {
        let bounds = Rect::new(Point::new(0.0, 0.0), Size::new(400.0, 200.0));
        let container = Size::new(800.0, 440.0);
        let plain = fit_viewport(bounds, container, FitInsets::default(), 0.1, 1.0);
        assert_eq!(
            plain,
            Viewport::fit(bounds, container, FIT_PADDING, 0.1, 1.0)
        );
        let insets = FitInsets::new(56.0, 0.0, 64.0, 0.0);
        let fitted = fit_viewport(bounds, container, insets, 0.1, 1.0);
        let top = fitted.world_to_screen(bounds.origin).y;
        let bottom = fitted
            .world_to_screen(Point::new(bounds.right(), bounds.bottom()))
            .y;
        assert!(top >= 56.0 && bottom <= 440.0 - 64.0, "{top} {bottom}");
        // Oversized insets are ignored rather than collapsing the content.
        let huge = fit_viewport(
            bounds,
            container,
            FitInsets::new(400.0, 0.0, 400.0, 0.0),
            0.1,
            1.0,
        );
        assert_eq!(huge, plain);
    }

    use super::*;

    #[test]
    fn leaving_fullscreen_restores_the_inline_view_exactly() {
        // Inline frames from narrow (403×438: panel open at 1100) to desktop.
        let bounds = Rect::new(Point::new(-600.0, -300.0), Size::new(1500.0, 700.0));
        for inline in [
            Size::new(403.0, 438.0),
            Size::new(279.0, 438.0),
            Size::new(666.0, 440.0),
            Size::new(720.0, 440.0),
            Size::new(1180.0, 440.0),
        ] {
            for manual in [false, true] {
                let fitted = fit_viewport(bounds, inline, FitInsets::default(), 0.1, 1.0);
                let viewport = if manual {
                    Viewport {
                        x: fitted.x + 37.0,
                        zoom: fitted.zoom * 1.7,
                        ..fitted
                    }
                } else {
                    fitted
                };
                let saved = SavedView {
                    viewport,
                    container: inline,
                    manual,
                };
                assert_eq!(restored_viewport(saved, inline), viewport);
                // Sub-pixel layout differences still count as the same frame.
                let jitter = Size::new(inline.width + 0.4, inline.height - 0.3);
                assert_eq!(restored_viewport(saved, jitter), viewport);
            }
        }
    }

    #[test]
    fn restore_into_a_resized_frame_keeps_zoom_and_world_centre() {
        let viewport = Viewport {
            x: 10.0,
            y: 20.0,
            zoom: 0.8,
        };
        let saved = SavedView {
            viewport,
            container: Size::new(403.0, 438.0),
            manual: true,
        };
        let wider = Size::new(603.0, 438.0);
        for manual in [true, false] {
            let kept = restored_viewport(SavedView { manual, ..saved }, wider);
            assert_eq!((kept.x, kept.y, kept.zoom), (110.0, 20.0, 0.8));
        }
    }

    #[test]
    fn automatic_framing_keeps_a_readable_zoom_anchored_on_the_root() {
        // A wide mind map whose fit would be about 0.2 in a 403px frame.
        let bounds = Rect::new(Point::new(-900.0, -250.0), Size::new(1800.0, 500.0));
        let root = Rect::new(Point::new(-60.0, -25.0), Size::new(120.0, 50.0));
        let container = Size::new(403.0, 438.0);
        let insets = FitInsets::new(60.0, 0.0, 60.0, 0.0);
        let fit = fit_viewport(bounds, container, insets, 0.1, 1.0);
        assert!(fit.zoom < 0.5);
        let framed = frame_viewport(bounds, Some(root), container, insets, Some(0.5), 0.1, 1.0);
        assert_eq!(framed.zoom, 0.5);
        let centre = framed.world_to_screen(root.center());
        assert!(
            (centre.x - container.width / 2.0).abs() < 0.01,
            "{centre:?}"
        );
        // Vertically the whole map fits at 0.5 and is centred in the insets.
        let top = framed.world_to_screen(bounds.origin).y;
        let bottom = framed.world_to_screen(Point::new(0.0, bounds.bottom())).y;
        assert!(
            (top - 60.0 - (378.0 - bottom)).abs() < 0.01,
            "{top} {bottom}"
        );
        // Content that already fits at a readable zoom is fitted unchanged.
        let small = Rect::new(Point::default(), Size::new(300.0, 200.0));
        assert_eq!(
            frame_viewport(small, None, container, insets, Some(0.5), 0.1, 1.0),
            fit_viewport(small, container, insets, 0.1, 1.0)
        );
        assert_eq!(
            frame_viewport(bounds, Some(root), container, insets, None, 0.1, 1.0),
            fit
        );
    }

    #[test]
    fn automatic_framing_never_pans_past_the_content_edge() {
        // A tree whose root sits at the top-left corner of its bounds.
        let bounds = Rect::new(Point::new(0.0, 0.0), Size::new(2400.0, 1200.0));
        let root = Rect::new(Point::new(0.0, 0.0), Size::new(120.0, 50.0));
        let container = Size::new(720.0, 440.0);
        let framed = frame_viewport(
            bounds,
            Some(root),
            container,
            FitInsets::default(),
            Some(0.5),
            0.1,
            1.0,
        );
        let origin = framed.world_to_screen(bounds.origin);
        assert_eq!((origin.x, origin.y), (FIT_PADDING, FIT_PADDING));
        let far = framed.world_to_screen(Point::new(bounds.right(), bounds.bottom()));
        assert!(far.x > container.width && far.y > container.height);
    }

    #[test]
    fn zoom_label_rounds_and_tolerates_invalid_zoom() {
        assert_eq!(zoom_label(1.0), "100%");
        assert_eq!(zoom_label(0.456), "46%");
        assert_eq!(zoom_label(f64::NAN), "100%");
    }

    #[component]
    fn FitPolicyFixture() -> Element {
        let mut controller = use_graph_controller();
        use_hook(move || {
            let rects = Rc::new(IndexMap::from([(
                NodeId("idea".into()),
                Rect::new(Point::new(20.0, 30.0), Size::new(100.0, 60.0)),
            )]));
            controller.geometry.write().rects = rects.clone();
            controller.geometry.write().settled = rects;
            assert_eq!(super::super::CanvasConfig::default().fit_max_zoom, None);
            controller.fit_view();
            assert!(
                controller.viewport().zoom > 1.0,
                "generic Fit keeps its existing zoom behavior"
            );
            controller.geometry.write().fit_max_zoom = Some(1.0);
            controller.fit_view();
            assert_eq!(controller.viewport().zoom, 1.0);
            controller.zoom_by(2.0);
            assert_eq!(
                controller.viewport().zoom,
                2.0,
                "the Fit cap does not restrict manual zoom"
            );
            controller.fit_view();
            assert_eq!(
                controller.viewport().zoom,
                1.0,
                "explicit Fit returns to the capped scale"
            );
            controller.geometry.write().fit_max_zoom = Some(0.001);
            controller.fit_view();
            assert_eq!(
                controller.viewport().zoom,
                controller.geometry.peek().limits.min_zoom
            );
            controller.geometry.write().fit_max_zoom = Some(f64::NAN);
            controller.fit_view();
            assert!(
                controller.viewport().zoom > 1.0,
                "invalid Fit caps leave generic limits intact"
            );
        });
        rsx! {}
    }

    #[test]
    fn fit_cap_preserves_manual_zoom_and_generic_defaults() {
        let mut dom = VirtualDom::new(FitPolicyFixture);
        dom.rebuild_in_place();
    }

    /// A layout switch animates from the old positions. Framing it must use
    /// the settled layout: the root's old place combined with the new bounds
    /// framed an arbitrary branch and left the root off-screen.
    #[component]
    fn LayoutSwitchFixture() -> Element {
        let mut controller = use_graph_controller();
        use_hook(move || {
            let size = Size::new(120.0, 50.0);
            let node = |x: f64, y: f64| Rect::new(Point::new(x, y), size);
            // A wide tree: the root sits on top, 42 leaves span 6000px.
            let mut settled = IndexMap::from([(NodeId("leaf0".into()), node(-3000.0, 400.0))]);
            settled.insert(NodeId("strategy".into()), node(-60.0, 0.0));
            for index in 1..42 {
                let x = -3000.0 + f64::from(index) * 145.0;
                settled.insert(NodeId(format!("leaf{index}")), node(x, 400.0));
            }
            // The first animation frame still shows the mind map positions.
            let mut first_frame = settled.clone();
            first_frame.insert(NodeId("strategy".into()), node(2600.0, 900.0));
            {
                let mut geometry = controller.geometry.write();
                geometry.container = Size::new(1006.0, 440.0);
                geometry.frame_min_zoom = Some(0.5);
                geometry.rects = Rc::new(first_frame);
                geometry.settled = Rc::new(settled.clone());
                geometry.root = Some(NodeId("strategy".into()));
            }
            let viewport = controller.frame_target().unwrap();
            let root = viewport.world_to_screen(settled[&NodeId::from("strategy")].center());
            assert!(
                (0.0..1006.0).contains(&root.x) && (0.0..440.0).contains(&root.y),
                "the root is framed: {root:?}"
            );
            // The root is the anchor although it is not the first node.
            assert!((root.x - 503.0).abs() < 1.0, "{root:?}");
            // A selection still wins over the root.
            controller.set_selection([NodeId("leaf41".into())]);
            let viewport = controller.frame_target().unwrap();
            let leaf = viewport.world_to_screen(settled[&NodeId::from("leaf41")].center());
            assert!((0.0..1006.0).contains(&leaf.x), "{leaf:?}");
        });
        rsx! {}
    }

    #[test]
    fn layout_switch_frames_the_settled_root() {
        let mut dom = VirtualDom::new(LayoutSwitchFixture);
        dom.rebuild_in_place();
    }

    #[test]
    fn frame_anchor_prefers_selection_then_root_then_first() {
        let rect = |x: f64| Rect::new(Point::new(x, 0.0), Size::new(10.0, 10.0));
        let rects = IndexMap::from([
            (NodeId::from("floating"), rect(0.0)),
            (NodeId::from("root"), rect(100.0)),
        ]);
        let id = |anchor: Option<(NodeId, Rect)>| anchor.map(|(id, _)| id.0);
        let root = NodeId::from("root");
        let floating = NodeId::from("floating");
        assert_eq!(
            id(frame_anchor(&rects, None, Some(&root))).as_deref(),
            Some("root")
        );
        assert_eq!(
            id(frame_anchor(&rects, Some(&floating), Some(&root))).as_deref(),
            Some("floating")
        );
        assert_eq!(
            id(frame_anchor(&rects, None, None)).as_deref(),
            Some("floating")
        );
        let gone = NodeId::from("gone");
        assert_eq!(
            id(frame_anchor(&rects, Some(&gone), Some(&gone))).as_deref(),
            Some("floating")
        );
    }
}
