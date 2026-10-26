use super::{
    NodeDetail,
    state::{CanvasState, SelectionEvent, reduce_selection},
};
use crate::interaction::GestureEffect;
use crate::layout::{LayoutConfig, LayoutInput, NodeHints, run_layout};
use crate::{GraphModel, NodeId, Point, Rect, Size};
use indexmap::IndexMap;
use std::rc::Rc;

pub(super) fn rects<N, E>(model: &GraphModel<N, E>, state: &CanvasState) -> IndexMap<NodeId, Rect> {
    model
        .nodes()
        .filter_map(|node| {
            state
                .positions
                .get(&node.id)
                .or(node.position.as_ref())
                .map(|p| {
                    (
                        node.id.clone(),
                        Rect {
                            origin: *p,
                            size: state
                                .measured
                                .get(&node.id)
                                .copied()
                                .unwrap_or(node.size_hint),
                        },
                    )
                })
        })
        .collect()
}

#[cfg(test)]
pub(super) fn calculate_layout<N, E>(
    model: &GraphModel<N, E>,
    state: &mut CanvasState,
    layout: &LayoutConfig,
    preserve: bool,
) {
    calculate_layout_pass(
        model,
        state,
        layout,
        LayoutPass {
            preserve,
            fresh: false,
        },
    );
}

/// How one layout run treats positions the canvas already shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct LayoutPass {
    /// Keep nodes placed before the current batch where they are (incremental
    /// layouts only: structural layouts always recompute every position).
    pub preserve: bool,
    /// Start from scratch: ignore stale model positions as warm-start hints.
    pub fresh: bool,
}

pub(super) fn calculate_layout_pass<N, E>(
    model: &GraphModel<N, E>,
    state: &mut CanvasState,
    layout: &LayoutConfig,
    pass: LayoutPass,
) {
    let structural = layout.is_structural();
    let preserve = pass.preserve && !structural;
    let input = LayoutInput::from_model(model, |node| {
        let current = state.positions.get(&node.id).copied();
        let known = state.stable.contains(&node.id) || state.moved.contains(&node.id);
        let handover = state.handover.get(&node.id).copied();
        NodeHints {
            size: state
                .measured
                .get(&node.id)
                .copied()
                .unwrap_or(node.size_hint),
            fixed: if node.pinned {
                node.position.or(current)
            } else if state.moved.contains(&node.id)
                || (preserve && state.stable.contains(&node.id))
            {
                current
            } else {
                None
            },
            // A handed-over arrangement continues where another canvas left
            // off; it only exists during this canvas's first batch.
            previous: if handover.is_some() {
                handover
            } else if structural {
                // The displayed position orients the node (e.g. its mind map
                // side); stored model positions belong to other layouts.
                current.filter(|_| !pass.fresh)
            } else if known {
                current.or(node.position)
            } else if pass.fresh && matches!(layout, LayoutConfig::Force(_)) {
                None
            } else {
                node.position
            },
        }
    });
    state.merge_positions(model, run_layout(layout, &input).positions);
}

/// Events affecting world geometry. Viewport zoom never changes layout sizes.
pub(super) enum CanvasInput {
    Start,
    ModelChanged,
    LayoutChanged(LayoutConfig, u64),
    ResetPositions(u64),
    Measured {
        id: NodeId,
        size: Size,
        detail: NodeDetail,
    },
    FlushMeasurements(u64),
    MeasureTimeout(u64),
    ContainerResized(Size),
    Gesture(GestureEffect),
    Selection(SelectionEvent),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum CanvasEffect {
    RectsChanged,
    ContainerChanged,
    FitView,
    /// A layout change or re-layout settled: frame its new geometry.
    FrameLayout,
    ScheduleTimeout(u64),
    ScheduleMeasurements(u64),
}

/// Pure owner of measurement, layout, geometry revisions and initial fitting.
/// The UI only applies effects and renders the stored rectangles.
pub(super) struct CanvasPipeline {
    pub state: CanvasState,
    pub position_source: super::PositionSource,
    pub rects: Rc<IndexMap<NodeId, Rect>>,
    pub revision: u64,
    pub animation: u64,
    pub container: Option<Size>,
    /// A saved view this canvas restores instead of re-framing on resize,
    /// until its measurements settle.
    pub restore: Option<super::SavedView>,
    layout: LayoutConfig,
    layout_revision: u64,
    reset_revision: u64,
    signature: Vec<String>,
    batch: u64,
    flush: u64,
    flush_scheduled: bool,
    dirty: bool,
    preserve: bool,
    /// The current batch lays out from scratch (initial mount, layout change
    /// or an explicit re-layout) until its measurements settle.
    fresh: bool,
    measured_once: bool,
    fitted: bool,
    /// Frame the content once the current layout change settles.
    reframe_layout: bool,
    /// Where the anchor node (selection or root) was on screen when the
    /// pending layout change began; a hand-framed view keeps it there.
    pub layout_anchor: Option<(NodeId, crate::Point)>,
    layout_sizes: IndexMap<NodeId, Size>,
    /// The last settled arrangement of automatic layouts switched away from.
    arrangements: Vec<(LayoutConfig, IndexMap<NodeId, Point>)>,
}

/// How many layouts' arrangements a canvas keeps for switching back.
const REMEMBERED_ARRANGEMENTS: usize = 4;

impl CanvasPipeline {
    /// The layout this canvas currently applies.
    pub fn layout_config(&self) -> &LayoutConfig {
        &self.layout
    }
    pub fn preserve_viewport(&mut self) {
        self.fitted = true;
    }
    /// A canvas for `model`. `handover` continues the arrangement another
    /// canvas showed under the same layout (see [`super::LayoutArrangement`]).
    pub fn new<N, E>(
        model: &GraphModel<N, E>,
        layout: LayoutConfig,
        layout_revision: u64,
        reset_revision: u64,
        handover: Option<&super::LayoutArrangement>,
    ) -> Self {
        let handover = handover
            .filter(|arrangement| arrangement.layout == layout)
            .map(|arrangement| (*arrangement.positions).clone())
            .unwrap_or_default();
        let mut pipeline = Self {
            state: CanvasState {
                handover,
                ..CanvasState::default()
            },
            position_source: super::PositionSource::default(),
            rects: Rc::default(),
            revision: 0,
            animation: 0,
            container: None,
            restore: None,
            layout,
            layout_revision,
            reset_revision,
            signature: super::state::layout_signature(model),
            batch: 1,
            flush: 0,
            flush_scheduled: false,
            dirty: false,
            preserve: false,
            fresh: true,
            measured_once: false,
            fitted: false,
            reframe_layout: false,
            layout_anchor: None,
            layout_sizes: IndexMap::new(),
            arrangements: Vec::new(),
        };
        pipeline.state.pending = model.nodes().map(|node| node.id.clone()).collect();
        pipeline.layout(model, false);
        pipeline.refresh_rects(model, &mut Vec::new());
        pipeline
    }

    pub fn reduce<N, E>(
        &mut self,
        model: &GraphModel<N, E>,
        input: CanvasInput,
    ) -> Vec<CanvasEffect> {
        let mut effects = Vec::new();
        if self.position_source == super::PositionSource::ModelPinned
            && let CanvasInput::Gesture(GestureEffect::NodesDragEnd(nodes)) = &input
        {
            for (id, _) in nodes {
                if let Some(node) = model.node(id) {
                    self.state.pin_waiting.insert(id.clone(), node.position);
                }
            }
        }
        match input {
            CanvasInput::Start => {
                effects.push(CanvasEffect::RectsChanged);
                if !self.state.pending.is_empty() {
                    effects.push(CanvasEffect::ScheduleTimeout(self.batch));
                }
            }
            CanvasInput::ModelChanged => {
                let signature = super::state::layout_signature(model);
                if self.signature != signature {
                    self.signature = signature;
                    self.state.stable = self.state.positions.keys().cloned().collect();
                    self.restart_measurement(model, true, &mut effects);
                }
                self.refresh_rects(model, &mut effects);
            }
            CanvasInput::LayoutChanged(layout, revision) => {
                if self.layout != layout || self.layout_revision != revision {
                    // Switching back to a layout continues the arrangement it
                    // showed (mind map sides); an explicit re-layout rebalances.
                    let relayout = self.layout_revision != revision;
                    self.remember_arrangement();
                    let resumed = (!relayout)
                        .then(|| self.remembered_arrangement(&layout))
                        .flatten();
                    self.layout = layout;
                    self.layout_revision = revision;
                    self.state.stable.clear();
                    self.state.handover = resumed.unwrap_or_default();
                    self.fresh = true;
                    self.reframe_layout = true;
                    self.restart_measurement(model, false, &mut effects);
                }
            }
            CanvasInput::ResetPositions(revision) => {
                if self.reset_revision != revision {
                    self.reset_revision = revision;
                    self.state.moved.clear();
                    self.state.stable.clear();
                    self.state.handover.clear();
                    self.reframe_layout = true;
                    self.restart_measurement(model, false, &mut effects);
                }
            }
            CanvasInput::Measured { id, size, detail } => {
                // Measurements belong to Full detail, including the initial hidden pass.
                if detail != NodeDetail::Full || model.node(&id).is_none() || !valid_size(size) {
                    return effects;
                }
                let was_pending = self.state.pending.contains(&id);
                let changed = self.state.measured.get(&id) != Some(&size);
                self.state.measure(id.clone(), size);
                if changed {
                    let significant = self.layout_sizes.get(&id).is_none_or(|old| {
                        (old.width - size.width).abs() > 8.0
                            || (old.height - size.height).abs() > 8.0
                    });
                    self.dirty |= significant || was_pending;
                    self.refresh_rects(model, &mut effects);
                }
                if self.state.pending.is_empty() && (self.dirty || !self.measured_once) {
                    self.schedule_flush(&mut effects);
                }
            }
            CanvasInput::FlushMeasurements(flush) => {
                if flush == self.flush && self.flush_scheduled {
                    self.flush_scheduled = false;
                    if self.state.pending.is_empty() {
                        self.finish_measurement(model, &mut effects);
                    }
                }
            }
            CanvasInput::MeasureTimeout(batch) => {
                if batch == self.batch && !self.state.pending.is_empty() {
                    self.state.pending.clear();
                    self.finish_measurement(model, &mut effects);
                }
            }
            CanvasInput::ContainerResized(size) => {
                if valid_size(size) && self.container != Some(size) {
                    self.container = Some(size);
                    effects.push(CanvasEffect::ContainerChanged);
                }
            }
            CanvasInput::Gesture(effect) => {
                self.state.apply_effect(&effect);
                if matches!(
                    effect,
                    GestureEffect::NodeDragMove { .. }
                        | GestureEffect::NodeDragEnd { .. }
                        | GestureEffect::NodesDragMove(_)
                        | GestureEffect::NodesDragEnd(_)
                ) {
                    self.refresh_rects(model, &mut effects);
                }
            }
            CanvasInput::Selection(event) => {
                self.state.selection = reduce_selection(&self.state.selection, event);
            }
        }
        self.maybe_fit(&mut effects);
        effects
    }

    fn restart_measurement<N, E>(
        &mut self,
        model: &GraphModel<N, E>,
        preserve: bool,
        effects: &mut Vec<CanvasEffect>,
    ) {
        self.batch += 1;
        self.flush += 1;
        self.flush_scheduled = false;
        self.dirty = false;
        self.preserve = preserve && self.measured_once;
        self.state.pending = model
            .nodes()
            .filter(|node| !self.state.measured.contains_key(&node.id))
            .map(|node| node.id.clone())
            .collect();
        self.layout(model, preserve);
        self.refresh_rects(model, effects);
        if self.state.pending.is_empty() {
            self.fresh = false;
            self.state.handover.clear();
        } else {
            effects.push(CanvasEffect::ScheduleTimeout(self.batch));
        }
    }

    /// Keep the settled arrangement of the current automatic layout for a
    /// later switch back to it.
    fn remember_arrangement(&mut self) {
        if matches!(self.layout, LayoutConfig::Manual)
            || !self.is_settled()
            || self.state.positions.is_empty()
        {
            return;
        }
        self.arrangements
            .retain(|(layout, _)| *layout != self.layout);
        self.arrangements
            .push((self.layout.clone(), self.state.positions.clone()));
        if self.arrangements.len() > REMEMBERED_ARRANGEMENTS {
            self.arrangements.remove(0);
        }
    }

    fn remembered_arrangement(&self, layout: &LayoutConfig) -> Option<IndexMap<NodeId, Point>> {
        self.arrangements
            .iter()
            .find(|(remembered, _)| remembered == layout)
            .map(|(_, positions)| positions.clone())
    }

    fn schedule_flush(&mut self, effects: &mut Vec<CanvasEffect>) {
        if !self.flush_scheduled {
            self.flush += 1;
            self.flush_scheduled = true;
            effects.push(CanvasEffect::ScheduleMeasurements(self.flush));
        }
    }

    fn finish_measurement<N, E>(
        &mut self,
        model: &GraphModel<N, E>,
        effects: &mut Vec<CanvasEffect>,
    ) {
        self.layout(model, self.preserve);
        self.dirty = false;
        self.measured_once = true;
        self.preserve = false;
        self.fresh = false;
        self.state.handover.clear();
        self.state.stable = self.state.positions.keys().cloned().collect();
        self.refresh_rects(model, effects);
    }

    fn layout<N, E>(&mut self, model: &GraphModel<N, E>, preserve: bool) {
        let pass = LayoutPass {
            preserve,
            fresh: self.fresh,
        };
        calculate_layout_pass(model, &mut self.state, &self.layout, pass);
        self.layout_sizes = model
            .nodes()
            .map(|node| {
                (
                    node.id.clone(),
                    self.state
                        .measured
                        .get(&node.id)
                        .copied()
                        .unwrap_or(node.size_hint),
                )
            })
            .collect();
    }

    fn refresh_rects<N, E>(&mut self, model: &GraphModel<N, E>, effects: &mut Vec<CanvasEffect>) {
        if self.position_source == super::PositionSource::ModelPinned {
            for node in model.nodes().filter(|node| node.pinned) {
                self.state.moved.shift_remove(&node.id);
                if self
                    .state
                    .pin_waiting
                    .get(&node.id)
                    .is_some_and(|old| *old == node.position)
                {
                    continue;
                }
                self.state.pin_waiting.shift_remove(&node.id);
                if self.state.dragging.as_ref() != Some(&node.id)
                    && !self.state.drag_group.contains_key(&node.id)
                    && let Some(position) = node.position
                {
                    self.state.positions.insert(node.id.clone(), position);
                }
            }
        }
        let rects = rects(model, &self.state);
        if *self.rects != rects {
            self.rects = Rc::new(rects);
            self.revision += 1;
            effects.push(CanvasEffect::RectsChanged);
        }
    }

    /// A layout change waits to be framed once it settles.
    pub fn reframe_pending(&self) -> bool {
        self.reframe_layout
    }

    /// Every node is measured and laid out; the rects are final.
    pub fn is_settled(&self) -> bool {
        self.state.pending.is_empty() && !self.dirty && !self.flush_scheduled
    }

    /// Frame the content once its measurements settle, so a resize during
    /// a remount never fits stale, unmeasured bounds. Returns whether the
    /// content is settled already, in which case the caller frames it now.
    pub fn fit_when_settled(&mut self) -> bool {
        if self.is_settled() {
            return true;
        }
        self.fitted = false;
        false
    }

    fn maybe_fit(&mut self, effects: &mut Vec<CanvasEffect>) {
        if self.container.is_none() || !self.is_settled() || self.rects.is_empty() {
            return;
        }
        if !self.fitted {
            self.fitted = true;
            self.reframe_layout = false;
            effects.push(CanvasEffect::FitView);
        } else if self.reframe_layout {
            self.reframe_layout = false;
            effects.push(CanvasEffect::FrameLayout);
        }
    }
}

fn valid_size(size: Size) -> bool {
    size.width.is_finite() && size.height.is_finite() && size.width > 0.0 && size.height > 0.0
}

#[cfg(test)]
mod tests {
    #[test]
    fn model_pins_update_without_layout_and_keep_pending_group_drag() {
        let mut model = crate::GraphModel::<(), ()>::default();
        for id in ["a", "b"] {
            let mut node = crate::GraphNode::new(id, ());
            node.pinned = true;
            node.position = Some(crate::Point::new(20.0, 30.0));
            model.insert_node(node).unwrap();
        }
        let mut pipeline =
            super::CanvasPipeline::new(&model, crate::LayoutConfig::Manual, 0, 0, None);
        pipeline.position_source = crate::PositionSource::ModelPinned;
        model
            .set_position(&"a".into(), crate::Point::new(80.0, 90.0))
            .unwrap();
        pipeline.reduce(&model, super::CanvasInput::ModelChanged);
        assert_eq!(
            pipeline.rects[&crate::NodeId::from("a")].origin,
            crate::Point::new(80.0, 90.0)
        );
        pipeline.state.drag_group = indexmap::IndexMap::from([
            ("a".into(), crate::Point::new(80.0, 90.0)),
            ("b".into(), crate::Point::new(20.0, 30.0)),
        ]);
        let moved = vec![
            ("a".into(), crate::Point::new(100.0, 110.0)),
            ("b".into(), crate::Point::new(40.0, 50.0)),
        ];
        pipeline.reduce(
            &model,
            super::CanvasInput::Gesture(crate::interaction::GestureEffect::NodesDragMove(
                moved.clone(),
            )),
        );
        assert_eq!(
            pipeline.rects[&crate::NodeId::from("b")].origin,
            crate::Point::new(40.0, 50.0)
        );
        pipeline.reduce(
            &model,
            super::CanvasInput::Gesture(crate::interaction::GestureEffect::NodesDragEnd(moved)),
        );
        assert_eq!(
            pipeline.rects[&crate::NodeId::from("a")].origin,
            crate::Point::new(100.0, 110.0)
        );
        model
            .set_position(&"a".into(), crate::Point::new(105.0, 115.0))
            .unwrap();
        pipeline.reduce(&model, super::CanvasInput::ModelChanged);
        assert_eq!(
            pipeline.rects[&crate::NodeId::from("a")].origin,
            crate::Point::new(105.0, 115.0)
        );
        assert!(!pipeline.state.moved.contains(&crate::NodeId::from("a")));
    }

    use super::*;
    use crate::{GraphNode, LayoutAlgorithm, LayoutOutput, Point};
    use std::{cell::Cell, rc::Rc};

    struct CountingLayout(Rc<Cell<usize>>);
    impl LayoutAlgorithm for CountingLayout {
        fn layout(&self, input: &LayoutInput) -> LayoutOutput {
            self.0.set(self.0.get() + 1);
            LayoutOutput {
                positions: input
                    .nodes
                    .iter()
                    .enumerate()
                    .map(|(index, node)| {
                        (
                            node.id.clone(),
                            node.fixed.unwrap_or(Point::new(index as f64 * 500.0, 20.0)),
                        )
                    })
                    .collect(),
            }
        }
    }

    fn fixture() -> (GraphModel<(), ()>, CanvasPipeline, Rc<Cell<usize>>) {
        let mut model = GraphModel::default();
        for id in ["a", "b"] {
            model.insert_node(GraphNode::new(id, ())).unwrap();
        }
        let count = Rc::new(Cell::new(0));
        let pipeline = CanvasPipeline::new(
            &model,
            LayoutConfig::Custom(Rc::new(CountingLayout(count.clone()))),
            0,
            0,
            None,
        );
        (model, pipeline, count)
    }

    fn measure(
        pipeline: &mut CanvasPipeline,
        model: &GraphModel<(), ()>,
        id: &str,
        size: Size,
        detail: NodeDetail,
    ) -> Vec<CanvasEffect> {
        pipeline.reduce(
            model,
            CanvasInput::Measured {
                id: id.into(),
                size,
                detail,
            },
        )
    }

    fn flush(pipeline: &mut CanvasPipeline, model: &GraphModel<(), ()>) -> Vec<CanvasEffect> {
        pipeline.reduce(model, CanvasInput::FlushMeasurements(pipeline.flush))
    }

    fn settle(pipeline: &mut CanvasPipeline, model: &GraphModel<(), ()>) {
        measure(
            pipeline,
            model,
            "a",
            Size::new(100.0, 60.0),
            NodeDetail::Full,
        );
        measure(
            pipeline,
            model,
            "b",
            Size::new(100.0, 60.0),
            NodeDetail::Full,
        );
        flush(pipeline, model);
    }

    #[test]
    fn lower_detail_measurements_never_change_layout_sizes_or_positions() {
        let (model, mut pipeline, count) = fixture();
        settle(&mut pipeline, &model);
        let before = pipeline.rects.clone();
        for detail in [NodeDetail::Compact, NodeDetail::Minimal] {
            let effects = measure(&mut pipeline, &model, "a", Size::new(70.0, 20.0), detail);
            assert!(effects.is_empty());
        }
        // Returning to Full with the same content also leaves layout untouched.
        assert!(
            measure(
                &mut pipeline,
                &model,
                "a",
                Size::new(100.0, 60.0),
                NodeDetail::Full
            )
            .is_empty()
        );
        assert_eq!(pipeline.rects, before);
        assert_eq!(count.get(), 2);
    }

    #[test]
    fn measurement_bursts_schedule_one_layout_using_all_received_sizes() {
        let (model, mut pipeline, count) = fixture();
        settle(&mut pipeline, &model);
        let first = measure(
            &mut pipeline,
            &model,
            "a",
            Size::new(140.0, 80.0),
            NodeDetail::Full,
        );
        let second = measure(
            &mut pipeline,
            &model,
            "b",
            Size::new(160.0, 90.0),
            NodeDetail::Full,
        );
        assert_eq!(
            first
                .iter()
                .filter(|effect| matches!(effect, CanvasEffect::ScheduleMeasurements(_)))
                .count(),
            1
        );
        assert!(
            !second
                .iter()
                .any(|effect| matches!(effect, CanvasEffect::ScheduleMeasurements(_)))
        );
        assert_eq!(count.get(), 2, "resize callbacks must not run the engine");
        flush(&mut pipeline, &model);
        assert_eq!(count.get(), 3);
        assert_eq!(
            pipeline.rects[&NodeId::from("a")].size,
            Size::new(140.0, 80.0)
        );
        assert_eq!(
            pipeline.rects[&NodeId::from("b")].size,
            Size::new(160.0, 90.0)
        );
        flush(&mut pipeline, &model);
        assert_eq!(count.get(), 3, "a duplicate tick must be harmless");
    }

    #[test]
    fn small_size_changes_update_geometry_and_accumulate_against_last_layout() {
        let (model, mut pipeline, count) = fixture();
        settle(&mut pipeline, &model);
        let revision = pipeline.revision;
        let effects = measure(
            &mut pipeline,
            &model,
            "a",
            Size::new(107.0, 60.0),
            NodeDetail::Full,
        );
        assert_eq!(effects, vec![CanvasEffect::RectsChanged]);
        assert_eq!(pipeline.rects[&NodeId::from("a")].size.width, 107.0);
        assert!(pipeline.revision > revision);
        let effects = measure(
            &mut pipeline,
            &model,
            "a",
            Size::new(109.0, 60.0),
            NodeDetail::Full,
        );
        assert!(
            effects
                .iter()
                .any(|effect| matches!(effect, CanvasEffect::ScheduleMeasurements(_)))
        );
        flush(&mut pipeline, &model);
        assert_eq!(count.get(), 3);
    }

    #[test]
    fn preserved_viewport_skips_initial_fit() {
        let (model, mut pipeline, _) = fixture();
        pipeline.preserve_viewport();
        pipeline.reduce(
            &model,
            CanvasInput::ContainerResized(Size::new(800.0, 600.0)),
        );
        measure(
            &mut pipeline,
            &model,
            "a",
            Size::new(100.0, 60.0),
            NodeDetail::Full,
        );
        measure(
            &mut pipeline,
            &model,
            "b",
            Size::new(100.0, 60.0),
            NodeDetail::Full,
        );
        assert!(!flush(&mut pipeline, &model).contains(&CanvasEffect::FitView));
    }

    #[test]
    fn remount_resize_frames_measured_bounds_not_placeholders() {
        let (model, mut pipeline, _) = fixture();
        pipeline.preserve_viewport();
        pipeline.reduce(
            &model,
            CanvasInput::ContainerResized(Size::new(403.0, 438.0)),
        );
        // The canvas asks to re-frame while nodes still use size hints.
        assert!(!pipeline.fit_when_settled());
        measure(
            &mut pipeline,
            &model,
            "a",
            Size::new(100.0, 60.0),
            NodeDetail::Full,
        );
        measure(
            &mut pipeline,
            &model,
            "b",
            Size::new(100.0, 60.0),
            NodeDetail::Full,
        );
        assert!(flush(&mut pipeline, &model).contains(&CanvasEffect::FitView));
        assert!(
            pipeline.fit_when_settled(),
            "settled content frames at once"
        );
    }

    #[test]
    fn layout_changes_frame_once_after_they_settle() {
        let (model, mut pipeline, _) = fixture();
        pipeline.reduce(
            &model,
            CanvasInput::ContainerResized(Size::new(800.0, 600.0)),
        );
        settle(&mut pipeline, &model);
        let relayout = pipeline.reduce(
            &model,
            CanvasInput::LayoutChanged(pipeline.layout.clone(), pipeline.layout_revision + 1),
        );
        assert_eq!(
            relayout
                .iter()
                .filter(|effect| **effect == CanvasEffect::FrameLayout)
                .count(),
            1
        );
        let reset = pipeline.reduce(&model, CanvasInput::ResetPositions(1));
        assert!(reset.contains(&CanvasEffect::FrameLayout));
        // Ordinary edits and resizes never re-frame through this path.
        assert!(
            !pipeline
                .reduce(&model, CanvasInput::ModelChanged)
                .contains(&CanvasEffect::FrameLayout)
        );
        assert!(
            !pipeline
                .reduce(
                    &model,
                    CanvasInput::ContainerResized(Size::new(600.0, 500.0))
                )
                .contains(&CanvasEffect::FrameLayout)
        );
    }

    #[test]
    fn initial_fit_waits_for_container_and_coalesced_measurements() {
        for container_first in [true, false] {
            let (model, mut pipeline, count) = fixture();
            if container_first {
                assert!(
                    !pipeline
                        .reduce(
                            &model,
                            CanvasInput::ContainerResized(Size::new(800.0, 600.0))
                        )
                        .contains(&CanvasEffect::FitView)
                );
            }
            measure(
                &mut pipeline,
                &model,
                "a",
                Size::new(100.0, 60.0),
                NodeDetail::Full,
            );
            let effects = measure(
                &mut pipeline,
                &model,
                "b",
                Size::new(100.0, 60.0),
                NodeDetail::Full,
            );
            assert!(!effects.contains(&CanvasEffect::FitView));
            let effects = flush(&mut pipeline, &model);
            assert_eq!(effects.contains(&CanvasEffect::FitView), container_first);
            let effects = pipeline.reduce(
                &model,
                CanvasInput::ContainerResized(Size::new(900.0, 700.0)),
            );
            assert_eq!(effects.contains(&CanvasEffect::FitView), !container_first);
            assert_eq!(count.get(), 2, "container resizing must never run layout");
            assert!(
                !pipeline
                    .reduce(
                        &model,
                        CanvasInput::ContainerResized(Size::new(600.0, 500.0))
                    )
                    .contains(&CanvasEffect::FitView)
            );
        }
    }

    #[test]
    fn stale_measurement_timeout_cannot_complete_a_new_model_batch() {
        let (mut model, mut pipeline, count) = fixture();
        let first_batch = pipeline.batch;
        model.insert_node(GraphNode::new("c", ())).unwrap();
        pipeline.reduce(&model, CanvasInput::ModelChanged);
        assert!(
            pipeline
                .reduce(&model, CanvasInput::MeasureTimeout(first_batch))
                .is_empty()
        );
        assert_eq!(pipeline.state.pending.len(), 3);
        let current_batch = pipeline.batch;
        pipeline.reduce(&model, CanvasInput::MeasureTimeout(current_batch));
        assert!(pipeline.state.pending.is_empty());
        assert_eq!(pipeline.rects.len(), 3);
        assert_eq!(count.get(), 3);
    }

    #[test]
    fn incremental_measurement_retains_dragged_and_existing_positions() {
        let (mut model, mut pipeline, _count) = fixture();
        settle(&mut pipeline, &model);
        let point = Point::new(45.0, 90.0);
        pipeline.reduce(
            &model,
            CanvasInput::Gesture(GestureEffect::NodeDragEnd {
                node: "a".into(),
                position: point,
            }),
        );
        let old_b = pipeline.rects[&NodeId::from("b")].origin;
        model.insert_node(GraphNode::new("c", ())).unwrap();
        pipeline.reduce(&model, CanvasInput::ModelChanged);
        measure(
            &mut pipeline,
            &model,
            "c",
            Size::new(130.0, 80.0),
            NodeDetail::Full,
        );
        flush(&mut pipeline, &model);
        assert_eq!(pipeline.rects[&NodeId::from("a")].origin, point);
        assert_eq!(pipeline.rects[&NodeId::from("b")].origin, old_b);
    }

    #[test]
    fn custom_engine_identity_and_controller_revision_request_layout() {
        let (model, mut pipeline, count) = fixture();
        settle(&mut pipeline, &model);
        let same_engine = pipeline.layout.clone();
        pipeline.reduce(&model, CanvasInput::LayoutChanged(same_engine.clone(), 0));
        assert_eq!(count.get(), 2);
        pipeline.reduce(&model, CanvasInput::LayoutChanged(same_engine, 1));
        assert_eq!(count.get(), 3);
        let other_count = Rc::new(Cell::new(0));
        pipeline.reduce(
            &model,
            CanvasInput::LayoutChanged(
                LayoutConfig::Custom(Rc::new(CountingLayout(other_count.clone()))),
                1,
            ),
        );
        assert_eq!(other_count.get(), 1);
    }

    #[test]
    fn remount_starts_with_current_controller_revisions() {
        let (model, _, count) = fixture();
        let layout = LayoutConfig::Custom(Rc::new(CountingLayout(count.clone())));
        let mut pipeline = CanvasPipeline::new(&model, layout.clone(), 7, 3, None);
        let before = count.get();
        pipeline.reduce(&model, CanvasInput::Start);
        pipeline.reduce(&model, CanvasInput::LayoutChanged(layout, 7));
        pipeline.reduce(&model, CanvasInput::ResetPositions(3));
        assert_eq!(count.get(), before);
    }

    #[test]
    fn container_resize_preserves_shared_rects_and_emits_only_container_change() {
        let (model, mut pipeline, _) = fixture();
        let rects = pipeline.rects.clone();
        let revision = pipeline.revision;
        let effects = pipeline.reduce(
            &model,
            CanvasInput::ContainerResized(Size::new(600.0, 400.0)),
        );
        assert_eq!(effects, vec![CanvasEffect::ContainerChanged]);
        assert!(Rc::ptr_eq(&rects, &pipeline.rects));
        assert_eq!(revision, pipeline.revision);
        assert!(
            pipeline
                .reduce(
                    &model,
                    CanvasInput::ContainerResized(Size::new(600.0, 400.0))
                )
                .is_empty()
        );
    }

    #[test]
    fn resetting_positions_releases_dragged_nodes_for_automatic_layout() {
        let (model, mut pipeline, count) = fixture();
        settle(&mut pipeline, &model);
        let original = pipeline.state.positions[&NodeId::from("a")];
        pipeline.reduce(
            &model,
            CanvasInput::Gesture(GestureEffect::NodeDragEnd {
                node: "a".into(),
                position: Point::new(100.0, 200.0),
            }),
        );
        let layout = pipeline.layout.clone();
        pipeline.reduce(&model, CanvasInput::LayoutChanged(layout, 1));
        assert_eq!(
            pipeline.state.positions[&NodeId::from("a")],
            Point::new(100.0, 200.0)
        );
        pipeline.reduce(&model, CanvasInput::ResetPositions(1));
        assert!(pipeline.state.moved.is_empty());
        assert!(pipeline.state.stable.is_empty());
        assert_eq!(pipeline.state.positions[&NodeId::from("a")], original);
        let before = count.get();
        assert!(
            pipeline
                .reduce(&model, CanvasInput::ResetPositions(1))
                .is_empty()
        );
        assert_eq!(count.get(), before);
    }

    #[test]
    fn reset_and_relayout_keep_explicit_model_pins() {
        let mut model = GraphModel::<(), ()>::default();
        let mut pinned = GraphNode::new("a", ());
        let pin = Point::new(700.0, 900.0);
        pinned.position = Some(pin);
        pinned.pinned = true;
        model.insert_node(pinned).unwrap();
        model.insert_node(GraphNode::new("b", ())).unwrap();
        let mut pipeline = CanvasPipeline::new(&model, LayoutConfig::default(), 0, 0, None);
        settle(&mut pipeline, &model);
        assert_eq!(pipeline.state.positions[&NodeId::from("a")], pin);
        let original = pipeline.state.positions[&NodeId::from("b")];
        let dragged = Point::new(100.0, 200.0);
        pipeline.reduce(
            &model,
            CanvasInput::Gesture(GestureEffect::NodeDragEnd {
                node: "b".into(),
                position: dragged,
            }),
        );
        pipeline.reduce(
            &model,
            CanvasInput::LayoutChanged(pipeline.layout.clone(), 1),
        );
        assert_eq!(pipeline.state.positions[&NodeId::from("a")], pin);
        assert_eq!(pipeline.state.positions[&NodeId::from("b")], dragged);
        pipeline.reduce(&model, CanvasInput::ResetPositions(1));
        assert!(pipeline.state.moved.is_empty());
        assert_eq!(pipeline.state.positions[&NodeId::from("a")], pin);
        assert_eq!(pipeline.state.positions[&NodeId::from("b")], original);
        assert!(model.node(&"a".into()).unwrap().pinned);
    }
    #[test]
    fn invalid_measurements_leave_initial_batch_pending() {
        let (model, mut pipeline, count) = fixture();
        for size in [
            Size::new(f64::NAN, 10.0),
            Size::new(0.0, 10.0),
            Size::new(10.0, -1.0),
        ] {
            assert!(measure(&mut pipeline, &model, "a", size, NodeDetail::Full).is_empty());
        }
        assert_eq!(pipeline.state.pending.len(), 2);
        assert_eq!(count.get(), 1);
    }
    #[test]
    fn stale_flush_cannot_finish_measurement_after_model_change() {
        let (mut model, mut pipeline, count) = fixture();
        settle(&mut pipeline, &model);
        measure(
            &mut pipeline,
            &model,
            "a",
            Size::new(150.0, 90.0),
            NodeDetail::Full,
        );
        let stale_flush = pipeline.flush;
        model.insert_node(GraphNode::new("c", ())).unwrap();
        pipeline.reduce(&model, CanvasInput::ModelChanged);
        let layouts = count.get();
        assert!(
            pipeline
                .reduce(&model, CanvasInput::FlushMeasurements(stale_flush))
                .is_empty()
        );
        assert!(pipeline.state.pending.contains(&NodeId::from("c")));
        assert_eq!(count.get(), layouts);
        measure(
            &mut pipeline,
            &model,
            "c",
            Size::new(140.0, 80.0),
            NodeDetail::Full,
        );
        flush(&mut pipeline, &model);
        assert_eq!(count.get(), layouts + 1);
    }

    fn measure_all(pipeline: &mut CanvasPipeline, model: &GraphModel<(), ()>, size: Size) {
        let ids: Vec<_> = model.nodes().map(|node| node.id.clone()).collect();
        for id in ids {
            pipeline.reduce(
                model,
                CanvasInput::Measured {
                    id,
                    size,
                    detail: NodeDetail::Full,
                },
            );
        }
        flush(pipeline, model);
    }

    fn child(model: &mut GraphModel<(), ()>, id: &str, parent: &str) {
        let mut node = GraphNode::new(id, ());
        node.layout_parent = Some(parent.into());
        model.insert_node(node).unwrap();
    }

    #[test]
    fn folding_reflows_structural_layouts_without_overlap_or_stale_geometry() {
        let size = Size::new(100.0, 40.0);
        for layout in [
            LayoutConfig::MindMap(Default::default()),
            LayoutConfig::Tree(Default::default()),
            LayoutConfig::Tree(crate::layout::TreeLayoutOptions {
                direction: crate::layout::TreeDirection::LeftRight,
                ..Default::default()
            }),
            LayoutConfig::Radial(Default::default()),
        ] {
            let mut full = GraphModel::<(), ()>::default();
            full.insert_node(GraphNode::new("root", ())).unwrap();
            child(&mut full, "a", "root");
            child(&mut full, "b", "root");
            for i in 0..6 {
                child(&mut full, &format!("a{i}"), "a");
            }
            let mut folded = full.clone();
            for i in 0..6 {
                folded.remove_node(&NodeId::from(format!("a{i}")));
            }
            let mut pipeline = CanvasPipeline::new(&full, layout.clone(), 0, 0, None);
            measure_all(&mut pipeline, &full, size);
            for model in [&folded, &full, &folded, &full] {
                pipeline.reduce(model, CanvasInput::ModelChanged);
                measure_all(&mut pipeline, model, size);
                assert_eq!(pipeline.rects.len(), model.nodes().len(), "{layout:?}");
                let rects: Vec<_> = pipeline.rects.values().copied().collect();
                for (i, a) in rects.iter().enumerate() {
                    for b in &rects[i + 1..] {
                        assert!(!a.intersects(*b), "{layout:?}: {a:?} overlaps {b:?}");
                    }
                }
                assert!(pipeline.is_settled());
            }
        }
    }

    #[test]
    fn structural_layouts_relayout_everything_but_keep_branch_sides() {
        let mut model = GraphModel::<(), ()>::default();
        model.insert_node(GraphNode::new("root", ())).unwrap();
        for id in ["a", "b", "c"] {
            child(&mut model, id, "root");
        }
        let mut pipeline = CanvasPipeline::new(
            &model,
            LayoutConfig::MindMap(Default::default()),
            0,
            0,
            None,
        );
        measure_all(&mut pipeline, &model, Size::new(100.0, 40.0));
        let left = |pipeline: &CanvasPipeline, id: &str| {
            pipeline.rects[&NodeId::from(id)].center().x
                < pipeline.rects[&NodeId::from("root")].center().x
        };
        assert_eq!(
            [
                left(&pipeline, "a"),
                left(&pipeline, "b"),
                left(&pipeline, "c")
            ],
            [false, true, false]
        );
        // Rapid structural edits: each one is laid out from the structure,
        // so children always sit beyond their parent on its side.
        for step in 0..8 {
            let parent = if step % 2 == 0 { "a" } else { "b" };
            child(&mut model, &format!("x{step}"), parent);
            pipeline.reduce(&model, CanvasInput::ModelChanged);
            measure_all(&mut pipeline, &model, Size::new(100.0, 40.0));
        }
        assert_eq!(
            [
                left(&pipeline, "a"),
                left(&pipeline, "b"),
                left(&pipeline, "c")
            ],
            [false, true, false]
        );
        for step in 0..8 {
            let id = format!("x{step}");
            let parent = if step % 2 == 0 { "a" } else { "b" };
            let (node, parent) = (
                pipeline.rects[&NodeId::from(id.as_str())],
                pipeline.rects[&NodeId::from(parent)],
            );
            if step % 2 == 0 {
                assert!(node.origin.x > parent.right(), "{id} right of its parent");
            } else {
                assert!(node.right() < parent.origin.x, "{id} left of its parent");
            }
        }
        let rects: Vec<_> = pipeline.rects.values().copied().collect();
        for (i, a) in rects.iter().enumerate() {
            for b in &rects[i + 1..] {
                assert!(!a.intersects(*b), "{a:?} overlaps {b:?}");
            }
        }
    }

    /// Entering or leaving fullscreen mounts a new canvas on the shared
    /// controller. It must continue the arrangement the user saw: a grown
    /// branch keeps other branches on their sides, where a from-scratch
    /// layout would rebalance and mirror part of the map.
    #[test]
    fn remounted_canvas_continues_the_handed_over_arrangement() {
        let mut model = GraphModel::<(), ()>::default();
        model.insert_node(GraphNode::new("root", ())).unwrap();
        for id in ["a", "b", "c"] {
            child(&mut model, id, "root");
        }
        let layout = LayoutConfig::MindMap(Default::default());
        let size = Size::new(100.0, 40.0);
        let mut inline = CanvasPipeline::new(&model, layout.clone(), 0, 0, None);
        measure_all(&mut inline, &model, size);
        // Branch "a" grows tall: kept sides leave "c" on the right, while a
        // fresh greedy balance would move it to the left.
        for index in 0..6 {
            child(&mut model, &format!("a{index}"), "a");
        }
        inline.reduce(&model, CanvasInput::ModelChanged);
        measure_all(&mut inline, &model, size);
        let side = |pipeline: &CanvasPipeline, id: &str| {
            pipeline.rects[&NodeId::from(id)].center().x
                < pipeline.rects[&NodeId::from("root")].center().x
        };
        let sides = |pipeline: &CanvasPipeline| {
            [
                side(pipeline, "a"),
                side(pipeline, "b"),
                side(pipeline, "c"),
            ]
        };
        assert_eq!(sides(&inline), [false, true, false]);
        let fresh = run_layout(&layout, &LayoutInput::fresh(&model, |_| size)).positions;
        assert!(
            fresh[&NodeId::from("c")].x < fresh[&NodeId::from("root")].x,
            "the scenario must differ from a fresh layout"
        );
        let arrangement = super::super::LayoutArrangement {
            layout: layout.clone(),
            positions: Rc::new(inline.state.positions.clone()),
        };
        // Fullscreen, then inline again: identical every time.
        let mut current = inline;
        for _ in 0..3 {
            let mut next = CanvasPipeline::new(&model, layout.clone(), 0, 0, Some(&arrangement));
            measure_all(&mut next, &model, size);
            assert_eq!(next.state.positions, current.state.positions);
            assert!(
                next.state.handover.is_empty(),
                "hints only orient the first batch"
            );
            current = next;
        }
        // Another layout's arrangement is no orientation hint.
        let other = super::super::LayoutArrangement {
            layout: LayoutConfig::Tree(Default::default()),
            ..arrangement.clone()
        };
        let mut switched = CanvasPipeline::new(&model, layout.clone(), 0, 0, Some(&other));
        measure_all(&mut switched, &model, size);
        assert_eq!(switched.state.positions, fresh);
        // An explicit re-layout rebalances from scratch.
        let mut relayout = CanvasPipeline::new(&model, layout.clone(), 0, 0, Some(&arrangement));
        relayout.reduce(&model, CanvasInput::LayoutChanged(layout.clone(), 1));
        measure_all(&mut relayout, &model, size);
        assert_eq!(relayout.state.positions, fresh);
        // Freeform and back: the mind map keeps its sides.
        let kept = current.state.positions.clone();
        current.reduce(&model, CanvasInput::LayoutChanged(LayoutConfig::Manual, 0));
        measure_all(&mut current, &model, size);
        current.reduce(&model, CanvasInput::LayoutChanged(layout, 0));
        measure_all(&mut current, &model, size);
        assert_eq!(current.state.positions, kept);
        assert!(current.state.handover.is_empty());
    }

    #[test]
    fn force_layout_and_relayout_start_from_scratch() {
        let mut model = GraphModel::<(), ()>::default();
        // Stale stored positions (e.g. from an earlier freeform arrangement)
        // must not freeze the simulation.
        for (index, id) in ["root", "a", "b", "c", "d"].into_iter().enumerate() {
            let mut node = GraphNode::new(id, ());
            node.position = Some(Point::new(index as f64 * 3.0, 0.0));
            node.layout_parent = (id != "root").then(|| "root".into());
            model.insert_node(node).unwrap();
        }
        let layout = LayoutConfig::Force(Default::default());
        let mut pipeline = CanvasPipeline::new(&model, layout.clone(), 0, 0, None);
        let size = Size::new(100.0, 40.0);
        measure_all(&mut pipeline, &model, size);
        let expected = run_layout(&layout, &LayoutInput::fresh(&model, |_| size)).positions;
        assert_eq!(pipeline.state.positions, expected);
        pipeline.reduce(&model, CanvasInput::LayoutChanged(layout, 1));
        assert_eq!(
            pipeline.state.positions, expected,
            "re-layout is deterministic"
        );
    }

    #[test]
    fn automatic_layout_matches_the_offline_fresh_layout() {
        let mut model = GraphModel::<(), ()>::default();
        model.insert_node(GraphNode::new("root", ())).unwrap();
        for (id, parent) in [("a", "root"), ("b", "root"), ("c", "a"), ("d", "b")] {
            child(&mut model, id, parent);
        }
        for layout in [
            LayoutConfig::MindMap(Default::default()),
            LayoutConfig::Tree(Default::default()),
            LayoutConfig::Radial(Default::default()),
        ] {
            let mut pipeline = CanvasPipeline::new(&model, layout.clone(), 0, 0, None);
            let size = Size::new(120.0, 50.0);
            measure_all(&mut pipeline, &model, size);
            let expected = run_layout(&layout, &LayoutInput::fresh(&model, |_| size)).positions;
            assert_eq!(pipeline.state.positions, expected, "{layout:?}");
        }
    }
}
