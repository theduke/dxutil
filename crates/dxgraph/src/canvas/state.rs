use indexmap::{IndexMap, IndexSet};

use super::NodeDetail;
use crate::{GraphModel, NodeId, Point, Rect, Size};

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CanvasState {
    pub measured: IndexMap<NodeId, Size>,
    pub positions: IndexMap<NodeId, Point>,
    pub selection: IndexSet<NodeId>,
    pub moved: IndexSet<NodeId>,
    /// Nodes placed before the current measurement batch began.
    pub stable: IndexSet<NodeId>,
    pub dragging: Option<NodeId>,
    pub drag_group: IndexMap<NodeId, Point>,
    pub pin_waiting: IndexMap<NodeId, Option<Point>>,
    pub pending: IndexSet<NodeId>,
    /// Positions another canvas showed under the same layout (see
    /// [`super::LayoutArrangement`]). They orient the from-scratch layout of
    /// this canvas's first batch, e.g. keep each mind map branch on its side.
    pub handover: IndexMap<NodeId, Point>,
}

impl CanvasState {
    /// Apply graph-local effects independently of the renderer and viewport.
    pub fn apply_effect(&mut self, effect: &crate::interaction::GestureEffect) {
        use crate::interaction::GestureEffect;
        match effect {
            GestureEffect::NodesDragMove(nodes) | GestureEffect::NodesDragEnd(nodes) => {
                for (id, position) in nodes {
                    self.positions.insert(id.clone(), *position);
                }
                if matches!(effect, GestureEffect::NodesDragEnd(_)) {
                    self.dragging = None;
                    self.drag_group.clear();
                    for (id, _) in nodes {
                        self.moved.insert(id.clone());
                    }
                }
            }
            GestureEffect::NodeDragStart(id) => self.dragging = Some(id.clone()),
            GestureEffect::NodeDragMove { node, position } => {
                self.positions.insert(node.clone(), *position);
            }
            GestureEffect::NodeDragEnd { node, position } => {
                self.dragging = None;
                self.moved.insert(node.clone());
                self.positions.insert(node.clone(), *position);
            }
            GestureEffect::NodeClick { node, shift } => {
                self.selection = reduce_selection(
                    &self.selection,
                    SelectionEvent::Click {
                        node: node.clone(),
                        shift: *shift,
                    },
                )
            }
            GestureEffect::BackgroundClick { .. } => self.selection.clear(),
            _ => {}
        }
    }
    /// Store a validated Full-detail measurement; significance belongs to the pipeline.
    pub fn measure(&mut self, id: NodeId, size: Size) {
        self.pending.shift_remove(&id);
        self.measured.insert(id, size);
    }

    pub fn merge_positions<N, E>(
        &mut self,
        model: &GraphModel<N, E>,
        positions: IndexMap<NodeId, Point>,
    ) {
        let mut next = IndexMap::new();
        for node in model.nodes() {
            let position = if node.pinned {
                node.position
                    .or_else(|| self.positions.get(&node.id).copied())
            } else if self.moved.contains(&node.id) {
                self.positions.get(&node.id).copied()
            } else {
                positions.get(&node.id).copied().or(node.position)
            };
            if let Some(position) = position {
                next.insert(node.id.clone(), position);
            }
        }
        self.positions = next;
        self.selection.retain(|id| model.node(id).is_some());
        self.measured.retain(|id, _| model.node(id).is_some());
        self.moved.retain(|id| model.node(id).is_some());
        self.pin_waiting.retain(|id, _| model.node(id).is_some());
        self.stable.retain(|id| model.node(id).is_some());
    }
}

pub fn needs_recull(culled: Rect, visible: Rect, last_zoom: f64, zoom: f64) -> bool {
    !culled.contains(visible.origin)
        || !culled.contains(Point {
            x: visible.origin.x + visible.size.width,
            y: visible.origin.y + visible.size.height,
        })
        || last_zoom <= 0.0
        || ((zoom / last_zoom) - 1.0).abs() > 0.15
}

pub fn detail_for_zoom(zoom: f64, full: f64, compact: f64) -> NodeDetail {
    if zoom >= full {
        NodeDetail::Full
    } else if zoom >= compact {
        NodeDetail::Compact
    } else {
        NodeDetail::Minimal
    }
}

/// The nodes to mount: those in `visible`, plus the selected, dragged and
/// keyboard-focused ones wherever they are. Unmounting the focused node would
/// drop keyboard focus to the page.
pub fn visible_nodes(
    rects: &IndexMap<NodeId, Rect>,
    visible: Rect,
    selected: &IndexSet<NodeId>,
    dragging: Option<&NodeId>,
    focused: Option<&NodeId>,
) -> IndexSet<NodeId> {
    rects
        .iter()
        .filter(|(id, rect)| {
            rect.intersects(visible)
                || selected.contains(*id)
                || dragging == Some(*id)
                || focused == Some(*id)
        })
        .map(|(id, _)| id.clone())
        .collect()
}

/// The edge `step` places after `current` in `edges`, wrapping around.
pub(super) fn step_edge(
    edges: &[crate::EdgeId],
    current: &crate::EdgeId,
    step: i32,
) -> Option<crate::EdgeId> {
    let at = edges.iter().position(|edge| edge == current)? as i64;
    let len = edges.len() as i64;
    Some(edges[(at + step as i64).rem_euclid(len) as usize].clone())
}

pub fn edge_visible(source: &NodeId, target: &NodeId, visible: &IndexSet<NodeId>) -> bool {
    visible.contains(source) || visible.contains(target)
}

#[derive(Clone, Debug, PartialEq)]
pub enum SelectionEvent {
    Click { node: NodeId, shift: bool },
    Clear,
}

pub fn reduce_selection(current: &IndexSet<NodeId>, event: SelectionEvent) -> IndexSet<NodeId> {
    match event {
        SelectionEvent::Clear => IndexSet::new(),
        SelectionEvent::Click { node, shift: false } => IndexSet::from([node]),
        SelectionEvent::Click { node, shift: true } => {
            let mut next = current.clone();
            if !next.shift_remove(&node) {
                next.insert(node);
            }
            next
        }
    }
}

/// A structural snapshot excludes content and positions: dragging does not lay out.
pub fn layout_signature<N, E>(model: &GraphModel<N, E>) -> Vec<String> {
    let mut signature: Vec<_> = model
        .nodes()
        .map(|node| {
            format!(
                "n:{}:{:?}:{:?}:{:?}:{}:{}",
                node.id,
                node.layout_parent,
                node.order_key,
                node.branch_side,
                node.size_hint.width,
                node.size_hint.height
            )
        })
        .chain(
            model
                .edges()
                .map(|edge| format!("e:{}:{}:{}", edge.id, edge.source, edge.target)),
        )
        .collect();
    signature.sort();
    signature
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edge_steps_wrap_around_the_visible_edges() {
        let edges: Vec<crate::EdgeId> = ["a", "b", "c"].map(crate::EdgeId::from).to_vec();
        let step = |from: &str, by| step_edge(&edges, &from.into(), by).map(|id| id.0);
        assert_eq!(step("a", 1).as_deref(), Some("b"));
        assert_eq!(step("c", 1).as_deref(), Some("a"));
        assert_eq!(step("a", -1).as_deref(), Some("c"));
        assert_eq!(step("gone", 1), None);
    }
    fn rect(x: f64) -> Rect {
        Rect {
            origin: Point { x, y: 0.0 },
            size: Size {
                width: 100.0,
                height: 100.0,
            },
        }
    }
    #[test]
    fn recull_has_margin_and_zoom_hysteresis() {
        let c = rect(0.0).inflate(100.0);
        assert!(!needs_recull(c, rect(20.0), 1.0, 1.1));
        assert!(needs_recull(c, rect(201.0), 1.0, 1.0));
        assert!(needs_recull(c, rect(0.0), 1.0, 1.2));
    }
    #[test]
    fn culling_keeps_the_keyboard_focused_node() {
        let a = NodeId::from("a");
        let b = NodeId::from("b");
        let rects = IndexMap::from([(a.clone(), rect(0.0)), (b.clone(), rect(999.0))]);
        // Off-screen and no longer selected (a paste selected other ideas):
        // the node holding DOM focus must stay mounted.
        let kept = visible_nodes(&rects, rect(0.0), &IndexSet::new(), None, Some(&b));
        assert!(kept.contains(&b));
        assert!(kept.contains(&a));
        let culled = visible_nodes(&rects, rect(0.0), &IndexSet::new(), None, None);
        assert!(!culled.contains(&b));
    }
    #[test]
    fn culling_keeps_selected_and_incident_edges() {
        let a = NodeId::from("a");
        let b = NodeId::from("b");
        let rects = IndexMap::from([(a.clone(), rect(0.0)), (b.clone(), rect(999.0))]);
        let v = visible_nodes(&rects, rect(0.0), &IndexSet::new(), None, None);
        assert_eq!(v.len(), 1);
        assert!(edge_visible(&a, &b, &v));
        assert_eq!(
            visible_nodes(&rects, rect(0.0), &IndexSet::from([b]), None, None).len(),
            2
        );
    }
    #[test]
    fn detail_thresholds() {
        assert_eq!(detail_for_zoom(0.6, 0.6, 0.3), NodeDetail::Full);
        assert_eq!(detail_for_zoom(0.3, 0.6, 0.3), NodeDetail::Compact);
        assert_eq!(detail_for_zoom(0.29, 0.6, 0.3), NodeDetail::Minimal);
    }
    #[test]
    fn selection_shift_toggles_and_escape_clears() {
        let a = NodeId::from("a");
        let b = NodeId::from("b");
        let selection = reduce_selection(
            &IndexSet::from([a.clone()]),
            SelectionEvent::Click {
                node: b.clone(),
                shift: true,
            },
        );
        assert_eq!(selection.len(), 2);
        assert_eq!(
            reduce_selection(
                &selection,
                SelectionEvent::Click {
                    node: a,
                    shift: true
                }
            ),
            IndexSet::from([b])
        );
        assert!(reduce_selection(&selection, SelectionEvent::Clear).is_empty());
    }
    #[test]
    fn drag_effects_keep_position_and_mark_moved() {
        use crate::interaction::GestureEffect;
        let mut state = CanvasState::default();
        let id = NodeId::from("a");
        let p = Point { x: 42.0, y: 9.0 };
        state.apply_effect(&GestureEffect::NodeDragStart(id.clone()));
        assert_eq!(state.dragging, Some(id.clone()));
        state.apply_effect(&GestureEffect::NodeDragMove {
            node: id.clone(),
            position: p,
        });
        state.apply_effect(&GestureEffect::NodeDragEnd {
            node: id.clone(),
            position: p,
        });
        assert!(state.dragging.is_none());
        assert!(state.moved.contains(&id));
        assert_eq!(state.positions[&id], p);
    }
    #[test]
    fn pinned_and_user_moved_positions_win_and_removal_prunes() {
        let mut model = GraphModel::<(), ()>::default();
        let mut pinned = crate::GraphNode::new("p", ());
        pinned.position = Some(Point { x: 7.0, y: 8.0 });
        pinned.pinned = true;
        model.insert_node(pinned).unwrap();
        model.insert_node(crate::GraphNode::new("u", ())).unwrap();
        let mut state = CanvasState::default();
        state
            .positions
            .insert("u".into(), Point { x: 11.0, y: 12.0 });
        state.moved.insert("u".into());
        state.merge_positions(
            &model,
            IndexMap::from([
                ("p".into(), Point::default()),
                ("u".into(), Point::default()),
            ]),
        );
        assert_eq!(
            state.positions[&NodeId::from("p")],
            Point { x: 7.0, y: 8.0 }
        );
        assert_eq!(
            state.positions[&NodeId::from("u")],
            Point { x: 11.0, y: 12.0 }
        );
        model.remove_node(&"u".into());
        state.merge_positions(&model, IndexMap::new());
        assert!(!state.positions.contains_key(&NodeId::from("u")));
    }
    fn signature_fixture() -> GraphModel<String, ()> {
        let mut model = GraphModel::default();
        model
            .insert_node(crate::GraphNode::new("a", "one".into()))
            .unwrap();
        model
    }

    #[test]
    fn layout_signature_ignores_payload_changes() {
        let mut model = signature_fixture();
        let before = layout_signature(&model);
        *model.node_data_mut(&"a".into()).unwrap() = "two".into();
        assert_eq!(layout_signature(&model), before);
    }

    #[test]
    fn layout_signature_ignores_drag_positions() {
        let mut model = signature_fixture();
        let before = layout_signature(&model);
        model
            .set_position(&"a".into(), Point::new(10.0, 12.0))
            .unwrap();
        assert_eq!(layout_signature(&model), before);
    }

    #[test]
    fn layout_signature_detects_structure_changes() {
        let mut model = signature_fixture();
        let before = layout_signature(&model);
        model
            .insert_node(crate::GraphNode::new("b", "three".into()))
            .unwrap();
        assert_ne!(layout_signature(&model), before);
    }

    #[test]
    fn three_hundred_nodes_keep_the_visible_set_bounded() {
        use crate::{LayoutAlgorithm, LayoutInput, LayoutNode, Viewport, layout::TreeLayout};
        let input = LayoutInput {
            nodes: (0..300)
                .map(|index| LayoutNode {
                    id: NodeId::from(format!("node-{index:03}")),
                    size: Size::new(160.0, 64.0),
                    fixed: None,
                    previous: None,
                    layout_parent: (index > 0)
                        .then(|| NodeId::from(format!("node-{:03}", (index - 1) / 2))),
                    order_key: None,
                    branch_side: None,
                })
                .collect(),
            edges: Vec::new(),
            roots: vec!["node-000".into()],
        };
        let positions = TreeLayout::default().layout(&input).positions;
        let rects: IndexMap<_, _> = input
            .nodes
            .iter()
            .map(|node| (node.id.clone(), Rect::new(positions[&node.id], node.size)))
            .collect();
        let root = positions[&input.nodes[0].id];
        let viewport = Viewport {
            x: -root.x + 450.0,
            y: -root.y + 100.0,
            zoom: 1.0,
        };
        let container = Size::new(1000.0, 700.0);
        let visible = viewport.visible_world_rect(container).inflate(200.0);
        let nodes = visible_nodes(&rects, visible, &IndexSet::new(), None, None);
        assert!(nodes.contains(&input.nodes[0].id));
        assert!(
            nodes.len() < 60,
            "culling unexpectedly exposes {} nodes",
            nodes.len()
        );
        let remote = input
            .nodes
            .iter()
            .find(|node| !nodes.contains(&node.id))
            .unwrap()
            .id
            .clone();
        let selected = IndexSet::from([remote.clone()]);
        let kept = visible_nodes(&rects, visible, &selected, None, None);
        assert!(kept.contains(&remote));
        assert_eq!(kept.len(), nodes.len() + 1);
        assert!(
            visible_nodes(&rects, visible, &IndexSet::new(), Some(&remote), None).contains(&remote)
        );
        // Screen-space panning only changes which subset is exposed, not node positions.
        let panned = Viewport {
            x: viewport.x - 600.0,
            ..viewport
        };
        assert_ne!(
            nodes,
            visible_nodes(
                &rects,
                panned.visible_world_rect(container).inflate(200.0),
                &IndexSet::new(),
                None,
                None
            )
        );
        assert_eq!(
            viewport.screen_to_world(viewport.world_to_screen(Point::new(50.0, 70.0))),
            Point::new(50.0, 70.0)
        );
    }
}
