//! Pure pointer/wheel reducer. All positions and timestamps come from the caller.
use crate::{EdgeSide, NodeId, Point, Rect, Viewport, ViewportLimits};
use indexmap::IndexMap;
const DRAG_THRESHOLD: f64 = 4.0;
const DOUBLE_CLICK_MS: u64 = 350;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GestureTarget {
    Background,
    Node(NodeId),
    Connect(NodeId, EdgeSide),
}
#[derive(Clone, Debug, PartialEq)]
pub enum GestureInput {
    PointerDown {
        pointer: i32,
        client: Point,
        target: GestureTarget,
        shift: bool,
        primary: bool,
        timestamp_ms: u64,
    },
    PointerMove {
        pointer: i32,
        client: Point,
    },
    PointerUp {
        pointer: i32,
        client: Point,
        timestamp_ms: u64,
    },
    PointerCancel {
        pointer: i32,
    },
    Wheel {
        client: Point,
        delta: Point,
        ctrl: bool,
    },
}
#[derive(Clone, Debug, PartialEq)]
pub enum GestureEffect {
    SetViewport(Viewport),
    BackgroundDoubleClick {
        world: Point,
    },
    MarqueeMove(Rect),
    MarqueeEnd(Option<Rect>),
    ConnectMove {
        source: NodeId,
        side: EdgeSide,
        world: Point,
    },
    ConnectEnd {
        source: NodeId,
        side: EdgeSide,
        world: Point,
    },
    ConnectCancel,
    NodesDragMove(Vec<(NodeId, Point)>),
    NodesDragEnd(Vec<(NodeId, Point)>),
    NodeDragStart(NodeId),
    NodeDragMove {
        node: NodeId,
        position: Point,
    },
    NodeDragEnd {
        node: NodeId,
        position: Point,
    },
    NodeClick {
        node: NodeId,
        shift: bool,
    },
    NodeDoubleClick(NodeId),
    BackgroundClick {
        world: Point,
    },
    CapturePointer(i32),
    ReleasePointer(i32),
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WheelMode {
    Pan,
    #[default]
    Zoom,
    /// Plain wheel scrolls the host page; modifier wheel zooms.
    ModifierZoom,
}
pub struct GestureContext<'a> {
    pub viewport: Viewport,
    pub container_origin: Point,
    pub limits: ViewportLimits,
    pub wheel_mode: WheelMode,
    pub node_position: &'a dyn Fn(&NodeId) -> Option<Point>,
}
#[derive(Clone, Debug)]
struct Press {
    pointer: i32,
    client: Point,
    target: GestureTarget,
    shift: bool,
    viewport: Viewport,
    node_origin: Point,
}
#[derive(Clone, Debug, Default)]
enum Mode {
    #[default]
    Idle,
    Pending(Press),
    Panning(Press),
    Dragging {
        press: Press,
        position: Point,
    },
    Marquee(Press),
    Connecting(Press),
    Pinching {
        viewport: Viewport,
        midpoint: Point,
        distance: f64,
    },
}
#[derive(Clone, Debug)]
struct Click {
    target: GestureTarget,
    client: Point,
    timestamp_ms: u64,
}
#[derive(Clone, Debug, Default)]
pub struct GestureState {
    pub marquee_enabled: bool,
    pub node_drag_disabled: bool,
    pointers: IndexMap<i32, Point>,
    mode: Mode,
    last_click: Option<Click>,
}
impl GestureState {
    /// True once a pointer gesture manipulates geometry or draws an overlay.
    pub fn is_manipulating(&self) -> bool {
        !matches!(self.mode, Mode::Idle | Mode::Pending(_))
    }
    /// Cancel ports and marquee without committing them; pointer captures are released.
    pub fn cancel(&mut self) -> Vec<GestureEffect> {
        let mut effects: Vec<_> = self
            .pointers
            .keys()
            .copied()
            .map(GestureEffect::ReleasePointer)
            .collect();
        if matches!(self.mode, Mode::Connecting(_)) {
            effects.push(GestureEffect::ConnectCancel);
        }
        if matches!(self.mode, Mode::Marquee(_)) {
            effects.push(GestureEffect::MarqueeEnd(None));
        }
        self.pointers.clear();
        self.mode = Mode::Idle;
        self.last_click = None;
        effects
    }

    #[cfg(not(all(feature = "web", target_arch = "wasm32")))]
    pub(crate) fn requires_fresh_origin(
        &self,
        input: &GestureInput,
        wheel_mode: WheelMode,
    ) -> bool {
        match input {
            GestureInput::Wheel { ctrl, .. } => *ctrl || wheel_mode == WheelMode::Zoom,
            GestureInput::PointerDown {
                pointer, primary, ..
            } => *primary && self.pointers.len() == 1 && !self.pointers.contains_key(pointer),
            GestureInput::PointerUp {
                pointer, client, ..
            } => {
                matches!(&self.mode, Mode::Pending(press) if press.pointer == *pointer && press.target == GestureTarget::Background && press.client.distance(*client) <= DRAG_THRESHOLD)
            }
            _ => false,
        }
    }
    pub(crate) fn active_pointers(&self) -> Vec<i32> {
        self.pointers.keys().copied().collect()
    }
    pub fn handle(&mut self, input: GestureInput, ctx: &GestureContext<'_>) -> Vec<GestureEffect> {
        let mut effects = Vec::new();
        match input {
            GestureInput::PointerDown {
                pointer,
                client,
                target,
                shift,
                primary,
                ..
            } => {
                if !primary || self.pointers.contains_key(&pointer) || self.pointers.len() >= 2 {
                    return effects;
                }
                self.pointers.insert(pointer, client);
                effects.push(GestureEffect::CapturePointer(pointer));
                if self.pointers.len() == 2 {
                    if matches!(self.mode, Mode::Connecting(_)) {
                        effects.push(GestureEffect::ConnectCancel);
                    }
                    if matches!(self.mode, Mode::Marquee(_)) {
                        effects.push(GestureEffect::MarqueeEnd(None));
                    }
                    if let Mode::Dragging { press, position } = &self.mode
                        && let GestureTarget::Node(node) = &press.target
                    {
                        effects.push(GestureEffect::NodeDragEnd {
                            node: node.clone(),
                            position: *position,
                        });
                    }
                    let (midpoint, distance) = self.pinch_geometry();
                    self.mode = Mode::Pinching {
                        viewport: ctx.viewport,
                        midpoint,
                        distance,
                    };
                    self.last_click = None;
                } else {
                    let node_origin = match &target {
                        GestureTarget::Node(id) => (ctx.node_position)(id).unwrap_or_default(),
                        GestureTarget::Background | GestureTarget::Connect(..) => Point::default(),
                    };
                    let connecting = matches!(target, GestureTarget::Connect(..));
                    let press = Press {
                        pointer,
                        client,
                        target,
                        shift,
                        viewport: ctx.viewport,
                        node_origin,
                    };
                    self.mode = if connecting {
                        Mode::Connecting(press)
                    } else {
                        Mode::Pending(press)
                    };
                }
            }
            GestureInput::PointerMove { pointer, client } => {
                self.move_pointer(pointer, client, ctx, &mut effects)
            }
            GestureInput::PointerUp {
                pointer,
                client,
                timestamp_ms,
            } => {
                if !self.pointers.contains_key(&pointer) {
                    return effects;
                }
                self.move_pointer(pointer, client, ctx, &mut effects);
                match &self.mode {
                    Mode::Pending(press) if press.pointer == pointer => {
                        match &press.target {
                            GestureTarget::Background => {
                                effects.push(GestureEffect::BackgroundClick {
                                    world: ctx
                                        .viewport
                                        .screen_to_world(local(client, ctx.container_origin)),
                                })
                            }
                            GestureTarget::Connect(..) => {}
                            GestureTarget::Node(node) => effects.push(GestureEffect::NodeClick {
                                node: node.clone(),
                                shift: press.shift,
                            }),
                        }
                        if self.last_click.as_ref().is_some_and(|last| {
                            last.target == press.target
                                && timestamp_ms >= last.timestamp_ms
                                && timestamp_ms - last.timestamp_ms <= DOUBLE_CLICK_MS
                                && last.client.distance(client) <= DRAG_THRESHOLD
                        }) {
                            match &press.target {
                                GestureTarget::Node(node) => {
                                    effects.push(GestureEffect::NodeDoubleClick(node.clone()))
                                }
                                GestureTarget::Background => {
                                    effects.push(GestureEffect::BackgroundDoubleClick {
                                        world: ctx
                                            .viewport
                                            .screen_to_world(local(client, ctx.container_origin)),
                                    })
                                }
                                _ => {}
                            }
                            self.last_click = None;
                        } else {
                            self.last_click = Some(Click {
                                target: press.target.clone(),
                                client,
                                timestamp_ms,
                            });
                        }
                    }
                    Mode::Marquee(press) if press.pointer == pointer => effects.push(
                        GestureEffect::MarqueeEnd(Some(crate::canvas::editing_rectangle(
                            ctx.viewport
                                .screen_to_world(local(press.client, ctx.container_origin)),
                            ctx.viewport
                                .screen_to_world(local(client, ctx.container_origin)),
                        ))),
                    ),
                    Mode::Connecting(press) if press.pointer == pointer => {
                        if let GestureTarget::Connect(source, side) = &press.target {
                            effects.push(GestureEffect::ConnectEnd {
                                source: source.clone(),
                                side: *side,
                                world: ctx
                                    .viewport
                                    .screen_to_world(local(client, ctx.container_origin)),
                            });
                        }
                    }
                    Mode::Dragging { press, position } if press.pointer == pointer => {
                        if let GestureTarget::Node(node) = &press.target {
                            effects.push(GestureEffect::NodeDragEnd {
                                node: node.clone(),
                                position: *position,
                            });
                        }
                    }
                    _ => {}
                }
                self.release(pointer, ctx, &mut effects);
            }
            GestureInput::PointerCancel { pointer } => {
                if !self.pointers.contains_key(&pointer) {
                    return effects;
                }
                if let Mode::Dragging { press, position } = &self.mode
                    && press.pointer == pointer
                    && let GestureTarget::Node(node) = &press.target
                {
                    effects.push(GestureEffect::NodeDragEnd {
                        node: node.clone(),
                        position: *position,
                    });
                }
                if matches!(self.mode, Mode::Connecting(_)) {
                    effects.push(GestureEffect::ConnectCancel);
                }
                if matches!(self.mode, Mode::Marquee(_)) {
                    effects.push(GestureEffect::MarqueeEnd(None));
                }
                self.last_click = None;
                self.release(pointer, ctx, &mut effects);
            }
            GestureInput::Wheel {
                client,
                delta,
                ctrl,
            } => {
                if ctrl || ctx.wheel_mode == WheelMode::Zoom {
                    effects.push(GestureEffect::SetViewport(ctx.viewport.zoom_at(
                        local(client, ctx.container_origin),
                        (-delta.y * 0.002).exp(),
                        ctx.limits.min_zoom,
                        ctx.limits.max_zoom,
                    )));
                } else if ctx.wheel_mode == WheelMode::Pan {
                    effects.push(GestureEffect::SetViewport(Viewport {
                        x: ctx.viewport.x - delta.x,
                        y: ctx.viewport.y - delta.y,
                        ..ctx.viewport
                    }));
                }
            }
        }
        effects
    }
    fn move_pointer(
        &mut self,
        pointer: i32,
        client: Point,
        ctx: &GestureContext<'_>,
        effects: &mut Vec<GestureEffect>,
    ) {
        let Some(point) = self.pointers.get_mut(&pointer) else {
            return;
        };
        *point = client;
        if let Mode::Pending(press) = &self.mode
            && press.pointer == pointer
            && press.client.distance(client) > DRAG_THRESHOLD
        {
            self.last_click = None;
            self.mode = match &press.target {
                GestureTarget::Background if press.shift && self.marquee_enabled => {
                    Mode::Marquee(press.clone())
                }
                GestureTarget::Background => Mode::Panning(press.clone()),
                GestureTarget::Connect(..) => Mode::Connecting(press.clone()),
                GestureTarget::Node(_) if self.node_drag_disabled => Mode::Panning(press.clone()),
                GestureTarget::Node(node) => {
                    effects.push(GestureEffect::NodeDragStart(node.clone()));
                    Mode::Dragging {
                        press: press.clone(),
                        position: press.node_origin,
                    }
                }
            };
        }
        match &mut self.mode {
            Mode::Marquee(press) if press.pointer == pointer => {
                effects.push(GestureEffect::MarqueeMove(
                    crate::canvas::editing_rectangle(
                        ctx.viewport
                            .screen_to_world(local(press.client, ctx.container_origin)),
                        ctx.viewport
                            .screen_to_world(local(client, ctx.container_origin)),
                    ),
                ));
            }
            Mode::Connecting(press) if press.pointer == pointer => {
                if let GestureTarget::Connect(source, side) = &press.target {
                    effects.push(GestureEffect::ConnectMove {
                        source: source.clone(),
                        side: *side,
                        world: ctx
                            .viewport
                            .screen_to_world(local(client, ctx.container_origin)),
                    });
                }
            }

            Mode::Panning(press) if press.pointer == pointer => {
                effects.push(GestureEffect::SetViewport(Viewport {
                    x: press.viewport.x + client.x - press.client.x,
                    y: press.viewport.y + client.y - press.client.y,
                    ..press.viewport
                }))
            }
            Mode::Dragging { press, position } if press.pointer == pointer => {
                let zoom = if press.viewport.zoom.is_finite() && press.viewport.zoom > 0.0 {
                    press.viewport.zoom
                } else {
                    1.0
                };
                *position = Point::new(
                    press.node_origin.x + (client.x - press.client.x) / zoom,
                    press.node_origin.y + (client.y - press.client.y) / zoom,
                );
                if let GestureTarget::Node(node) = &press.target {
                    effects.push(GestureEffect::NodeDragMove {
                        node: node.clone(),
                        position: *position,
                    });
                }
            }
            Mode::Pinching {
                viewport,
                midpoint,
                distance,
            } => {
                let base = *viewport;
                let old_midpoint = *midpoint;
                let old_distance = *distance;
                let (midpoint, distance) = self.pinch_geometry();
                let zoomed = base.zoom_at(
                    local(old_midpoint, ctx.container_origin),
                    distance / old_distance.max(0.001),
                    ctx.limits.min_zoom,
                    ctx.limits.max_zoom,
                );
                effects.push(GestureEffect::SetViewport(Viewport {
                    x: zoomed.x + midpoint.x - old_midpoint.x,
                    y: zoomed.y + midpoint.y - old_midpoint.y,
                    ..zoomed
                }));
            }
            _ => {}
        }
    }
    fn pinch_geometry(&self) -> (Point, f64) {
        let mut points = self.pointers.values();
        let a = points.next().copied().unwrap_or_default();
        let b = points.next().copied().unwrap_or(a);
        (
            Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0),
            a.distance(b).max(0.001),
        )
    }
    fn release(
        &mut self,
        pointer: i32,
        ctx: &GestureContext<'_>,
        effects: &mut Vec<GestureEffect>,
    ) {
        let viewport = effects
            .iter()
            .rev()
            .find_map(|effect| match effect {
                GestureEffect::SetViewport(viewport) => Some(*viewport),
                _ => None,
            })
            .unwrap_or(ctx.viewport);
        self.pointers.shift_remove(&pointer);
        effects.push(GestureEffect::ReleasePointer(pointer));
        self.mode = if let Some((&pointer, &client)) = self.pointers.first() {
            Mode::Panning(Press {
                pointer,
                client,
                target: GestureTarget::Background,
                shift: false,
                viewport,
                node_origin: Point::default(),
            })
        } else {
            Mode::Idle
        };
    }
}
fn local(client: Point, origin: Point) -> Point {
    Point::new(client.x - origin.x, client.y - origin.y)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disabled_node_drag_preserves_selection_and_pans_instead() {
        let ctx = context();
        let mut state = GestureState {
            node_drag_disabled: true,
            ..Default::default()
        };
        state.handle(
            down(1, Point::default(), GestureTarget::Node("a".into())),
            &ctx,
        );
        assert!(!state.is_manipulating());
        let effects = state.handle(
            GestureInput::PointerMove {
                pointer: 1,
                client: Point::new(20.0, 10.0),
            },
            &ctx,
        );
        assert!(
            effects
                .iter()
                .any(|effect| matches!(effect, GestureEffect::SetViewport(_)))
        );
        assert!(state.is_manipulating());
        assert!(!effects.iter().any(|effect| matches!(
            effect,
            GestureEffect::NodeDragStart(_) | GestureEffect::NodeDragMove { .. }
        )));
        let effects = state.handle(up(1, Point::new(20.0, 10.0), 20), &ctx);
        assert!(
            !effects
                .iter()
                .any(|effect| matches!(effect, GestureEffect::NodeDragEnd { .. }))
        );
        assert!(!state.is_manipulating());
        state.handle(
            down(1, Point::default(), GestureTarget::Node("a".into())),
            &ctx,
        );
        let effects = state.handle(up(1, Point::default(), 40), &ctx);
        assert!(effects.iter().any(
            |effect| matches!(effect, GestureEffect::NodeClick { node, .. } if node.0 == "a")
        ));
    }
    #[test]
    fn modifier_wheel_passes_through_without_modifier() {
        let mut ctx = context();
        ctx.wheel_mode = WheelMode::ModifierZoom;
        let mut state = GestureState::default();
        let event = |ctrl| GestureInput::Wheel {
            client: Point::new(10.0, 20.0),
            delta: Point::new(0.0, 40.0),
            ctrl,
        };
        assert!(state.handle(event(false), &ctx).is_empty());
        assert!(matches!(
            state.handle(event(true), &ctx).as_slice(),
            [GestureEffect::SetViewport(_)]
        ));
    }
    #[test]
    fn marquee_is_opt_in_and_cancellable() {
        let ctx = context();
        let mut state = GestureState {
            marquee_enabled: true,
            ..GestureState::default()
        };
        state.handle(
            GestureInput::PointerDown {
                pointer: 1,
                client: Point::new(10.0, 10.0),
                target: GestureTarget::Background,
                shift: true,
                primary: true,
                timestamp_ms: 0,
            },
            &ctx,
        );
        let effects = state.handle(
            GestureInput::PointerMove {
                pointer: 1,
                client: Point::new(80.0, 60.0),
            },
            &ctx,
        );
        assert!(
            effects
                .iter()
                .any(|effect| matches!(effect, GestureEffect::MarqueeMove(_)))
        );
        assert!(
            !effects
                .iter()
                .any(|effect| matches!(effect, GestureEffect::SetViewport(_)))
        );
        assert!(state.cancel().contains(&GestureEffect::MarqueeEnd(None)));
        assert!(state.active_pointers().is_empty());
    }
    #[test]
    fn connect_tracks_world_position_and_cancels_without_commit() {
        let ctx = context();
        let mut state = GestureState::default();
        state.handle(
            down(
                1,
                Point::new(0.0, 0.0),
                GestureTarget::Connect("a".into(), EdgeSide::Right),
            ),
            &ctx,
        );
        let effects = state.handle(
            GestureInput::PointerMove {
                pointer: 1,
                client: Point::new(50.0, 70.0),
            },
            &ctx,
        );
        assert!(effects.iter().any(|effect|matches!(effect,GestureEffect::ConnectMove {source,..} if *source==NodeId::from("a"))));
        let effects = state.handle(up(1, Point::new(50.0, 70.0), 100), &ctx);
        assert!(effects.iter().any(|effect|matches!(effect,GestureEffect::ConnectEnd {source,..} if *source==NodeId::from("a"))));
        state.handle(
            down(
                1,
                Point::default(),
                GestureTarget::Connect("a".into(), EdgeSide::Right),
            ),
            &ctx,
        );
        assert!(state.cancel().contains(&GestureEffect::ConnectCancel));
    }
    fn context() -> GestureContext<'static> {
        GestureContext {
            viewport: Viewport::default(),
            container_origin: Point::new(10.0, 20.0),
            limits: ViewportLimits::default(),
            wheel_mode: WheelMode::Zoom,
            node_position: &|_| Some(Point::new(30.0, 40.0)),
        }
    }
    fn down(pointer: i32, point: Point, target: GestureTarget) -> GestureInput {
        GestureInput::PointerDown {
            pointer,
            client: point,
            target,
            shift: false,
            primary: true,
            timestamp_ms: 0,
        }
    }
    fn up(pointer: i32, point: Point, timestamp_ms: u64) -> GestureInput {
        GestureInput::PointerUp {
            pointer,
            client: point,
            timestamp_ms,
        }
    }
    fn viewport(effects: &[GestureEffect]) -> Viewport {
        effects
            .iter()
            .find_map(|effect| {
                if let GestureEffect::SetViewport(v) = effect {
                    Some(*v)
                } else {
                    None
                }
            })
            .unwrap()
    }
    #[test]
    fn click_threshold_and_double_click() {
        let ctx = context();
        let mut state = GestureState::default();
        let target = GestureTarget::Node("a".into());
        let p = Point::new(20.0, 30.0);
        state.handle(down(1, p, target.clone()), &ctx);
        assert!(
            state
                .handle(
                    GestureInput::PointerMove {
                        pointer: 1,
                        client: Point::new(24.0, 30.0)
                    },
                    &ctx
                )
                .is_empty()
        );
        let effects = state.handle(up(1, p, 100), &ctx);
        assert!(effects.contains(&GestureEffect::NodeClick {
            node: "a".into(),
            shift: false
        }));
        state.handle(down(1, p, target.clone()), &ctx);
        assert!(
            state
                .handle(up(1, p, 400), &ctx)
                .contains(&GestureEffect::NodeDoubleClick("a".into()))
        );
        state.handle(down(1, p, target), &ctx);
        assert!(
            !state
                .handle(up(1, p, 800), &ctx)
                .iter()
                .any(|e| matches!(e, GestureEffect::NodeDoubleClick(_)))
        );
    }
    #[test]
    fn pan_and_background_click() {
        let ctx = context();
        let mut state = GestureState::default();
        let p = Point::new(20.0, 30.0);
        state.handle(down(1, p, GestureTarget::Background), &ctx);
        let effects = state.handle(
            GestureInput::PointerMove {
                pointer: 1,
                client: Point::new(30.0, 50.0),
            },
            &ctx,
        );
        assert_eq!(
            viewport(&effects),
            Viewport {
                x: 10.0,
                y: 20.0,
                zoom: 1.0
            }
        );
        assert!(
            !state
                .handle(up(1, Point::new(30.0, 50.0), 100), &ctx)
                .iter()
                .any(|e| matches!(e, GestureEffect::BackgroundClick { .. }))
        );
        state.handle(down(1, p, GestureTarget::Background), &ctx);
        assert!(
            state
                .handle(up(1, p, 200), &ctx)
                .contains(&GestureEffect::BackgroundClick {
                    world: Point::new(10.0, 10.0)
                })
        );
    }
    #[test]
    fn zoomed_drag_cancel_and_second_pointer() {
        let mut ctx = context();
        ctx.viewport.zoom = 0.5;
        let mut state = GestureState::default();
        let p = Point::new(20.0, 30.0);
        state.handle(down(1, p, GestureTarget::Node("a".into())), &ctx);
        let effects = state.handle(
            GestureInput::PointerMove {
                pointer: 1,
                client: Point::new(30.0, 40.0),
            },
            &ctx,
        );
        assert!(effects.contains(&GestureEffect::NodeDragStart("a".into())));
        assert!(effects.contains(&GestureEffect::NodeDragMove {
            node: "a".into(),
            position: Point::new(50.0, 60.0)
        }));
        let effects = state.handle(GestureInput::PointerCancel { pointer: 1 }, &ctx);
        assert!(effects.contains(&GestureEffect::NodeDragEnd {
            node: "a".into(),
            position: Point::new(50.0, 60.0)
        }));
        assert!(effects.contains(&GestureEffect::ReleasePointer(1)));
        state.handle(down(1, p, GestureTarget::Node("a".into())), &ctx);
        state.handle(
            GestureInput::PointerMove {
                pointer: 1,
                client: Point::new(30.0, 40.0),
            },
            &ctx,
        );
        assert!(
            state
                .handle(
                    down(2, Point::new(60.0, 40.0), GestureTarget::Background),
                    &ctx
                )
                .iter()
                .any(|e| matches!(e, GestureEffect::NodeDragEnd { .. }))
        );
    }
    #[test]
    fn wheel_cursor_invariant_and_pan() {
        let mut ctx = context();
        let mut state = GestureState::default();
        let client = Point::new(40.0, 50.0);
        let point = local(client, ctx.container_origin);
        let effects = state.handle(
            GestureInput::Wheel {
                client,
                delta: Point::new(0.0, -100.0),
                ctrl: false,
            },
            &ctx,
        );
        let zoomed = viewport(&effects);
        assert!(zoomed.zoom > 1.0);
        assert_eq!(
            zoomed.screen_to_world(point),
            ctx.viewport.screen_to_world(point)
        );
        ctx.wheel_mode = WheelMode::Pan;
        assert_eq!(
            viewport(&state.handle(
                GestureInput::Wheel {
                    client,
                    delta: Point::new(5.0, 10.0),
                    ctrl: false
                },
                &ctx
            )),
            Viewport {
                x: -5.0,
                y: -10.0,
                zoom: 1.0
            }
        );
        assert!(
            viewport(&state.handle(
                GestureInput::Wheel {
                    client,
                    delta: Point::new(0.0, -100.0),
                    ctrl: true
                },
                &ctx
            ))
            .zoom
                > 1.0
        );
    }
    #[test]
    fn pinch_zoom_and_midpoint_pan() {
        let ctx = context();
        let mut state = GestureState::default();
        state.handle(
            down(1, Point::new(10.0, 20.0), GestureTarget::Background),
            &ctx,
        );
        state.handle(
            down(2, Point::new(30.0, 20.0), GestureTarget::Background),
            &ctx,
        );
        let effects = state.handle(
            GestureInput::PointerMove {
                pointer: 2,
                client: Point::new(50.0, 20.0),
            },
            &ctx,
        );
        let viewport = viewport(&effects);
        assert_eq!(viewport.zoom, 2.0);
        assert_eq!(
            viewport.world_to_screen(Point::new(10.0, 0.0)),
            Point::new(20.0, 0.0)
        );
        assert!(
            !state
                .handle(up(2, Point::new(50.0, 20.0), 100), &ctx)
                .iter()
                .any(|e| matches!(e, GestureEffect::BackgroundClick { .. }))
        );
    }
    #[test]
    fn remaining_pointer_continues_from_final_pinch_viewport() {
        let ctx = context();
        let mut state = GestureState::default();
        state.handle(
            down(1, Point::new(10.0, 20.0), GestureTarget::Background),
            &ctx,
        );
        state.handle(
            down(2, Point::new(30.0, 20.0), GestureTarget::Background),
            &ctx,
        );
        let final_effects = state.handle(up(2, Point::new(50.0, 20.0), 100), &ctx);
        let final_viewport = viewport(&final_effects);
        let effects = state.handle(
            GestureInput::PointerMove {
                pointer: 1,
                client: Point::new(20.0, 20.0),
            },
            &ctx,
        );
        assert_eq!(
            viewport(&effects),
            Viewport {
                x: final_viewport.x + 10.0,
                ..final_viewport
            }
        );
    }
    #[test]
    fn non_primary_and_unknown_pointers_ignored() {
        let ctx = context();
        let mut state = GestureState::default();
        assert!(
            state
                .handle(
                    GestureInput::PointerDown {
                        pointer: 1,
                        client: Point::default(),
                        target: GestureTarget::Background,
                        shift: false,
                        primary: false,
                        timestamp_ms: 0
                    },
                    &ctx
                )
                .is_empty()
        );
        assert!(state.handle(up(1, Point::default(), 0), &ctx).is_empty());
        assert!(
            state
                .handle(GestureInput::PointerCancel { pointer: 3 }, &ctx)
                .is_empty()
        );
    }
}
