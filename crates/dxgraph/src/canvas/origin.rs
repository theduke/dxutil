use crate::{
    Point, Size,
    interaction::{GestureInput, GestureState, WheelMode, platform},
};
use dioxus::prelude::*;
use std::collections::VecDeque;
use std::{cell::RefCell, rc::Rc};

#[derive(Default)]
pub(super) struct OriginCache {
    point: Point,
    active: bool,
    dirty: bool,
}
impl OriginCache {
    pub fn point(&self) -> Point {
        self.point
    }
    pub fn set_point(&mut self, point: Point) {
        self.point = point;
    }
    pub fn refresh_for_input(input: &GestureInput, wheel_mode: WheelMode) -> bool {
        matches!(input, GestureInput::PointerDown { .. })
            || matches!(input, GestureInput::Wheel { ctrl, .. } if *ctrl || wheel_mode == WheelMode::Zoom)
    }
    pub fn request_refresh(&mut self) -> bool {
        self.dirty = true;
        if self.active {
            return false;
        }
        self.active = true;
        true
    }
    pub fn begin_refresh(&mut self) {
        self.dirty = false;
    }
    pub fn finish_refresh(&mut self, point: Option<Point>) -> bool {
        if let Some(point) = point {
            self.point = point;
        }
        self.active = self.dirty;
        self.active
    }
}

/// One bounds refresh task per canvas, with ordered native input replay.
#[derive(Clone)]
pub(super) struct OriginRefresh {
    pub cache: Rc<RefCell<OriginCache>>,
    pub inputs: Rc<RefCell<NativeInputs>>,
    pub mounted: Signal<Option<Rc<MountedData>>>,
    pub gesture: Signal<GestureState>,
    pub resized: Callback<Size>,
    pub moved: Callback<Point>,
    pub dispatch: Callback<(GestureInput, Point)>,
}
impl OriginRefresh {
    pub fn request(&self) {
        if !self.cache.borrow_mut().request_refresh() {
            return;
        }
        self.cache.borrow_mut().begin_refresh();
        // Web bounds must be read synchronously before the first input after a shift.
        let first = platform::client_geometry(self.mounted.peek().as_deref());
        if let Some((point, size)) = first {
            self.cache.borrow_mut().set_point(point);
            self.moved.call(point);
            self.resized.call(size);
        }
        let refresh = self.clone();
        spawn(async move { refresh.run(first).await });
    }

    async fn run(self, mut synchronous: Option<(Point, Size)>) {
        loop {
            // A wheel arriving during mount's request waits for the next fresh query.
            let batch = self.inputs.borrow_mut().begin_batch();
            let element = self.mounted.peek().clone();
            let next = if let Some(geometry) = synchronous.take() {
                Some(geometry)
            } else if let Some(element) = element {
                element.get_client_rect().await.ok().map(|rect| {
                    (
                        Point::new(rect.origin.x, rect.origin.y),
                        Size::new(rect.size.width, rect.size.height),
                    )
                })
            } else {
                None
            };
            if let Some((point, size)) = next {
                self.cache.borrow_mut().set_point(point);
                self.moved.call(point);
                self.resized.call(size);
                for input in batch {
                    self.dispatch.call((input, point));
                }
                let following = self.inputs.borrow_mut().finish_batch();
                for input in following {
                    self.dispatch.call((input, point));
                }
            } else {
                // Cancel captured gestures when the query fails, rather than stranding them.
                let gated = self.inputs.borrow().gated();
                self.inputs.borrow_mut().clear();
                if gated {
                    let point = self.cache.borrow().point();
                    let pointers = self.gesture.peek().active_pointers();
                    for pointer in pointers {
                        self.dispatch
                            .call((GestureInput::PointerCancel { pointer }, point));
                    }
                }
            }
            if !self.inputs.borrow().is_empty() {
                self.cache.borrow_mut().request_refresh();
            }
            dioxus_sdk_time::sleep(std::time::Duration::from_millis(16)).await;
            if !self
                .cache
                .borrow_mut()
                .finish_refresh(next.map(|(point, _)| point))
            {
                break;
            }
            self.cache.borrow_mut().begin_refresh();
            synchronous = platform::client_geometry(self.mounted.peek().as_deref());
        }
    }
}

/// Keep every transition and move in order while bounds-dependent input waits for IPC.
#[derive(Default)]
pub(super) struct NativeInputs {
    pending: VecDeque<GestureInput>,
    in_flight: bool,
}
impl NativeInputs {
    pub fn gated(&self) -> bool {
        self.in_flight || !self.pending.is_empty()
    }
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
    #[cfg(any(test, not(all(feature = "web", target_arch = "wasm32"))))]
    pub fn push(&mut self, input: GestureInput) {
        if let Some(GestureInput::Wheel {
            client,
            delta,
            ctrl,
        }) = self.pending.back_mut()
            && let GestureInput::Wheel {
                client: next,
                delta: added,
                ctrl: next_ctrl,
            } = &input
            && client == next
            && ctrl == next_ctrl
            && delta.y.signum() == added.y.signum()
        {
            // Same-anchor monotonic zooms compose, including saturation at zoom limits.
            delta.x += added.x;
            delta.y += added.y;
            return;
        }
        self.pending.push_back(input);
    }
    pub fn begin_batch(&mut self) -> VecDeque<GestureInput> {
        self.in_flight = !self.pending.is_empty();
        std::mem::take(&mut self.pending)
    }
    pub fn finish_batch(&mut self) -> VecDeque<GestureInput> {
        if std::mem::take(&mut self.in_flight) {
            std::mem::take(&mut self.pending)
        } else {
            // Inputs arriving during a mount/resize query need a new bounds response.
            VecDeque::new()
        }
    }
    pub fn clear(&mut self) {
        self.pending.clear();
        self.in_flight = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interaction::GestureTarget;
    fn movement(x: f64) -> GestureInput {
        GestureInput::PointerMove {
            pointer: 1,
            client: Point::new(x, 0.0),
        }
    }
    fn wheel(y: f64) -> GestureInput {
        GestureInput::Wheel {
            client: Point::new(40.0, 50.0),
            delta: Point::new(0.0, y),
            ctrl: true,
        }
    }
    fn down(pointer: i32) -> GestureInput {
        GestureInput::PointerDown {
            pointer,
            client: Point::default(),
            target: GestureTarget::Background,
            shift: false,
            primary: true,
            timestamp_ms: 0,
        }
    }
    #[test]
    fn delta_input_and_release_never_request_bounds() {
        for input in [
            movement(1.0),
            GestureInput::PointerUp {
                pointer: 1,
                client: Point::default(),
                timestamp_ms: 1,
            },
            GestureInput::PointerCancel { pointer: 1 },
            GestureInput::Wheel {
                client: Point::default(),
                delta: Point::new(0.0, 2.0),
                ctrl: false,
            },
        ] {
            assert!(!OriginCache::refresh_for_input(&input, WheelMode::Pan));
        }
        assert!(OriginCache::refresh_for_input(&down(1), WheelMode::Pan));
        assert!(OriginCache::refresh_for_input(&wheel(3.0), WheelMode::Pan));
    }
    #[test]
    fn refresh_bursts_have_one_active_task_and_keep_cached_origin() {
        let mut cache = OriginCache::default();
        cache.set_point(Point::new(15.0, 25.0));
        assert!(cache.request_refresh());
        for _ in 0..100 {
            assert!(!cache.request_refresh());
        }
        cache.begin_refresh();
        assert_eq!(cache.point(), Point::new(15.0, 25.0));
        assert!(!cache.request_refresh());
        assert!(cache.finish_refresh(Some(Point::new(30.0, 40.0))));
        cache.begin_refresh();
        assert!(!cache.finish_refresh(None));
        assert_eq!(cache.point(), Point::new(30.0, 40.0));
    }
    #[test]
    fn wheel_during_mount_request_waits_for_next_fresh_batch_and_coalesces() {
        let mut inputs = NativeInputs::default();
        assert!(inputs.begin_batch().is_empty());
        for _ in 0..100 {
            inputs.push(wheel(2.0));
        }
        assert!(inputs.finish_batch().is_empty());
        inputs.push(wheel(-3.0));
        assert_eq!(
            inputs.begin_batch(),
            VecDeque::from([wheel(200.0), wheel(-3.0)])
        );
        assert!(inputs.finish_batch().is_empty());
        assert!(inputs.is_empty());
    }
    #[test]
    fn gated_moves_preserve_every_sample_and_termination_in_order() {
        let mut inputs = NativeInputs::default();
        inputs.push(down(2));
        assert_eq!(inputs.begin_batch(), VecDeque::from([down(2)]));
        assert!(inputs.gated());
        for x in [1.0, 3.0, 20.0, 0.0, 0.0] {
            inputs.push(movement(x));
        }
        let up = GestureInput::PointerUp {
            pointer: 1,
            client: Point::default(),
            timestamp_ms: 2,
        };
        inputs.push(up.clone());
        inputs.push(GestureInput::PointerCancel { pointer: 2 });
        assert_eq!(
            inputs.finish_batch(),
            VecDeque::from([
                movement(1.0),
                movement(3.0),
                movement(20.0),
                movement(0.0),
                movement(0.0),
                up,
                GestureInput::PointerCancel { pointer: 2 }
            ])
        );
        assert!(!inputs.gated());
        assert!(inputs.is_empty());
    }
    #[test]
    fn failure_clears_gate_and_pending_events() {
        let mut inputs = NativeInputs::default();
        inputs.push(down(2));
        inputs.begin_batch();
        inputs.push(movement(20.0));
        inputs.clear();
        assert!(!inputs.gated());
        assert!(inputs.is_empty());
    }
    #[test]
    fn queued_wheel_gates_following_press_and_pan_preserves_zoom() {
        use crate::{
            Viewport, ViewportLimits,
            interaction::{GestureContext, GestureEffect},
        };
        for during_mount in [false, true] {
            let mut inputs = NativeInputs::default();
            if during_mount {
                assert!(inputs.begin_batch().is_empty());
            }
            inputs.push(wheel(-120.0));
            assert!(inputs.gated());
            inputs.push(down(1));
            inputs.push(movement(20.0));
            inputs.push(GestureInput::PointerUp {
                pointer: 1,
                client: Point::new(20.0, 0.0),
                timestamp_ms: 1,
            });
            if during_mount {
                assert!(inputs.finish_batch().is_empty());
                assert!(inputs.gated());
            }
            let batch = inputs.begin_batch();
            assert_eq!(batch.len(), 4);
            assert!(inputs.gated());
            let mut state = GestureState::default();
            let mut viewport = Viewport::default();
            let mut zoomed = None;
            let positions = |_: &crate::NodeId| None;
            for (index, event) in batch.into_iter().enumerate() {
                let context = GestureContext {
                    viewport,
                    container_origin: Point::new(80.0, 30.0),
                    limits: ViewportLimits::default(),
                    wheel_mode: WheelMode::Zoom,
                    node_position: &positions,
                };
                for effect in state.handle(event, &context) {
                    if let GestureEffect::SetViewport(next) = effect {
                        viewport = next;
                    }
                }
                if index == 0 {
                    zoomed = Some(viewport);
                }
            }
            let zoomed = zoomed.unwrap();
            assert!(zoomed.zoom > 1.0);
            assert_eq!(viewport.zoom, zoomed.zoom);
            assert_eq!(viewport.x, zoomed.x + 20.0);
            assert_eq!(viewport.y, zoomed.y);
            assert!(inputs.finish_batch().is_empty());
            assert!(!inputs.gated());
        }
    }
    #[test]
    fn queued_excursion_remains_a_drag_after_returning_to_press_origin() {
        use crate::{
            Viewport, ViewportLimits,
            interaction::{GestureContext, GestureEffect, GestureState},
        };
        let positions = |_: &crate::NodeId| Some(Point::default());
        let context = GestureContext {
            viewport: Viewport::default(),
            container_origin: Point::default(),
            limits: ViewportLimits::default(),
            wheel_mode: WheelMode::Zoom,
            node_position: &positions,
        };
        for samples in [[0.1, 0.2, 4.5, 0.0], [3.9, 4.01, 0.0, 0.0]] {
            let mut inputs = NativeInputs::default();
            let mut state = GestureState::default();
            let mut press = down(1);
            if let GestureInput::PointerDown { target, .. } = &mut press {
                *target = GestureTarget::Node("n".into());
            }
            inputs.push(press);
            for event in inputs.begin_batch() {
                state.handle(event, &context);
            }
            for x in samples {
                inputs.push(movement(x));
            }
            inputs.push(GestureInput::PointerUp {
                pointer: 1,
                client: Point::default(),
                timestamp_ms: 10,
            });
            let effects = inputs
                .finish_batch()
                .into_iter()
                .flat_map(|event| state.handle(event, &context))
                .collect::<Vec<_>>();
            assert!(
                effects
                    .iter()
                    .any(|effect| matches!(effect, GestureEffect::NodeDragEnd { .. }))
            );
            assert!(
                !effects
                    .iter()
                    .any(|effect| matches!(effect, GestureEffect::NodeClick { .. }))
            );
        }
    }
    #[test]
    fn pending_background_release_precedes_following_node_selection() {
        use crate::{
            Viewport, ViewportLimits,
            interaction::{GestureContext, GestureEffect, GestureState},
        };
        let positions = |_: &crate::NodeId| Some(Point::default());
        let context = GestureContext {
            viewport: Viewport::default(),
            container_origin: Point::default(),
            limits: ViewportLimits::default(),
            wheel_mode: WheelMode::Zoom,
            node_position: &positions,
        };
        let mut state = GestureState::default();
        state.handle(down(1), &context);
        let mut inputs = NativeInputs::default();
        inputs.push(GestureInput::PointerUp {
            pointer: 1,
            client: Point::default(),
            timestamp_ms: 1,
        });
        let batch = inputs.begin_batch();
        let mut node_press = down(1);
        if let GestureInput::PointerDown { target, .. } = &mut node_press {
            *target = GestureTarget::Node("n".into());
        }
        inputs.push(node_press);
        inputs.push(GestureInput::PointerUp {
            pointer: 1,
            client: Point::default(),
            timestamp_ms: 2,
        });
        let effects = batch
            .into_iter()
            .chain(inputs.finish_batch())
            .flat_map(|event| state.handle(event, &context))
            .filter(|effect| {
                matches!(
                    effect,
                    GestureEffect::BackgroundClick { .. } | GestureEffect::NodeClick { .. }
                )
            })
            .collect::<Vec<_>>();
        assert!(matches!(
            effects.as_slice(),
            [
                GestureEffect::BackgroundClick { .. },
                GestureEffect::NodeClick { .. }
            ]
        ));
    }
    #[test]
    fn alternating_pinch_moves_preserve_all_samples_in_order() {
        let mut inputs = NativeInputs::default();
        inputs.push(down(1));
        inputs.push(down(2));
        inputs.begin_batch();
        for x in 0..100 {
            for pointer in [1, 2] {
                inputs.push(GestureInput::PointerMove {
                    pointer,
                    client: Point::new(x as f64, 0.0),
                });
            }
        }
        let samples = inputs.finish_batch();
        assert_eq!(samples.len(), 200);
        for (index, event) in samples.iter().enumerate() {
            assert_eq!(
                *event,
                GestureInput::PointerMove {
                    pointer: 1 + (index % 2) as i32,
                    client: Point::new((index / 2) as f64, 0.0),
                }
            );
        }
        for pointer in [1, 2] {
            let last = samples.iter().rev().find(|event| matches!(event, GestureInput::PointerMove { pointer: id, .. } if *id == pointer)).unwrap();
            assert_eq!(
                *last,
                GestureInput::PointerMove {
                    pointer,
                    client: Point::new(99.0, 0.0)
                }
            );
        }
    }
    #[test]
    fn coalesced_wheel_matches_sequential_zoom_at_limits_and_keeps_reversal() {
        use crate::{
            Viewport, ViewportLimits,
            interaction::{GestureContext, GestureEffect, GestureState},
        };
        let replay = |events: Vec<GestureInput>| {
            let mut viewport = Viewport {
                zoom: 19.9,
                ..Viewport::default()
            };
            let mut state = GestureState::default();
            let positions = |_: &crate::NodeId| None;
            for event in events {
                let context = GestureContext {
                    viewport,
                    container_origin: Point::new(80.0, 30.0),
                    limits: ViewportLimits::default(),
                    wheel_mode: WheelMode::Zoom,
                    node_position: &positions,
                };
                for effect in state.handle(event, &context) {
                    if let GestureEffect::SetViewport(next) = effect {
                        viewport = next;
                    }
                }
            }
            viewport
        };
        let events = vec![wheel(-120.0), wheel(-120.0), wheel(60.0), wheel(60.0)];
        let expected = replay(events.clone());
        let mut inputs = NativeInputs::default();
        for event in events {
            inputs.push(event);
        }
        let batch = inputs.begin_batch();
        assert_eq!(batch.len(), 2);
        let actual = replay(batch.into_iter().collect());
        assert!((actual.zoom - expected.zoom).abs() < 1e-8);
        assert!((actual.x - expected.x).abs() < 1e-8);
        assert!((actual.y - expected.y).abs() < 1e-8);
    }
}
