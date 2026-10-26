//! Deterministic, framework-independent layouts. Positions are world top-left corners.
use crate::{NodeId, Point, Rect, Size};
use indexmap::IndexMap;
use std::{fmt, rc::Rc};
pub mod force;
pub mod mindmap;
pub mod radial;
mod rng;
mod spanning;
pub mod tree;
pub use force::{ForceLayout, ForceOptions};
pub use mindmap::{BranchSide, MindMapBalance, MindMapLayout, MindMapOptions};
pub use radial::{RadialLayout, RadialOptions};
pub use tree::{STACK_INDENT, TreeDirection, TreeLayout, TreeLayoutOptions, stacked_leaves};
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutNode {
    pub id: NodeId,
    pub size: Size,
    pub fixed: Option<Point>,
    pub previous: Option<Point>,
    pub layout_parent: Option<NodeId>,
    pub order_key: Option<String>,
    /// See [`crate::GraphNode::branch_side`].
    pub branch_side: Option<BranchSide>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutEdge {
    pub source: NodeId,
    pub target: NodeId,
    pub weight: f64,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayoutInput {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub roots: Vec<NodeId>,
}
/// Per-node geometry hints supplied by the caller of [`LayoutInput::from_model`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NodeHints {
    pub size: Size,
    pub fixed: Option<Point>,
    pub previous: Option<Point>,
}
impl LayoutInput {
    /// Build the input for `model`. Structure (layout parents, order keys,
    /// edges and roots) always comes from the model, so the canvas and any
    /// offline caller (such as a host's "tidy up") lay out the same graph;
    /// `hints` supplies each node's size and position hints.
    pub fn from_model<N, E>(
        model: &crate::GraphModel<N, E>,
        mut hints: impl FnMut(&crate::GraphNode<N>) -> NodeHints,
    ) -> Self {
        Self {
            nodes: model
                .nodes()
                .map(|node| {
                    let hints = hints(node);
                    LayoutNode {
                        id: node.id.clone(),
                        size: hints.size,
                        fixed: hints.fixed,
                        previous: hints.previous,
                        layout_parent: node.layout_parent.clone(),
                        order_key: node.order_key.clone(),
                        branch_side: node.branch_side,
                    }
                })
                .collect(),
            edges: model
                .edges()
                .map(|edge| LayoutEdge {
                    source: edge.source.clone(),
                    target: edge.target.clone(),
                    weight: 1.0,
                })
                .collect(),
            roots: model
                .nodes()
                .filter(|node| node.layout_parent.is_none())
                .map(|node| node.id.clone())
                .collect(),
        }
    }
    /// The input of a from-scratch layout: only explicit model pins are kept.
    pub fn fresh<N, E>(
        model: &crate::GraphModel<N, E>,
        mut size: impl FnMut(&crate::GraphNode<N>) -> Size,
    ) -> Self {
        Self::from_model(model, |node| NodeHints {
            size: size(node),
            fixed: node.position.filter(|_| node.pinned),
            previous: None,
        })
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayoutOutput {
    pub positions: IndexMap<NodeId, Point>,
}
pub trait LayoutAlgorithm {
    fn layout(&self, input: &LayoutInput) -> LayoutOutput;
}
#[derive(Clone)]
pub enum LayoutConfig {
    Tree(TreeLayoutOptions),
    MindMap(MindMapOptions),
    Force(ForceOptions),
    Radial(RadialOptions),
    Manual,
    Custom(Rc<dyn LayoutAlgorithm>),
}

impl PartialEq for LayoutConfig {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Tree(a), Self::Tree(b)) => a == b,
            (Self::MindMap(a), Self::MindMap(b)) => a == b,
            (Self::Force(a), Self::Force(b)) => a == b,
            (Self::Radial(a), Self::Radial(b)) => a == b,
            (Self::Manual, Self::Manual) => true,
            (Self::Custom(a), Self::Custom(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl fmt::Debug for LayoutConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tree(options) => f.debug_tuple("Tree").field(options).finish(),
            Self::MindMap(options) => f.debug_tuple("MindMap").field(options).finish(),
            Self::Force(options) => f.debug_tuple("Force").field(options).finish(),
            Self::Radial(options) => f.debug_tuple("Radial").field(options).finish(),
            Self::Manual => f.write_str("Manual"),
            Self::Custom(_) => f.write_str("Custom"),
        }
    }
}

const COMPONENT_GAP: f64 = 80.0;
/// Clearance between nodes placed around fixed nodes when a layout has no
/// sibling spacing of its own.
const FIXED_NODE_CLEARANCE: f64 = 24.0;
impl Default for LayoutConfig {
    fn default() -> Self {
        Self::Tree(TreeLayoutOptions::default())
    }
}
impl LayoutConfig {
    /// Deterministic structural layouts recompute every position from the
    /// graph's structure; earlier positions are only orientation hints (such as
    /// a mind map branch's side), never constraints.
    pub fn is_structural(&self) -> bool {
        matches!(self, Self::Tree(_) | Self::MindMap(_) | Self::Radial(_))
    }
}
pub fn run_layout(config: &LayoutConfig, input: &LayoutInput) -> LayoutOutput {
    match config {
        LayoutConfig::Tree(options) => TreeLayout(options.clone()).layout(input),
        LayoutConfig::MindMap(options) => MindMapLayout(options.clone()).layout(input),
        LayoutConfig::Force(options) => ForceLayout(options.clone()).layout(input),
        LayoutConfig::Radial(options) => RadialLayout(options.clone()).layout(input),
        LayoutConfig::Custom(engine) => engine.layout(input),
        LayoutConfig::Manual => LayoutOutput {
            positions: input
                .nodes
                .iter()
                .map(|n| (n.id.clone(), n.fixed.or(n.previous).unwrap_or_default()))
                .collect(),
        },
    }
}
/// Pack disconnected components in rows. Components containing pins retain their
/// world coordinates; movable components are placed outside their bounds.
pub(super) fn pack_components(
    input: &LayoutInput,
    components: &[Vec<usize>],
    positions: &mut IndexMap<NodeId, Point>,
    gap: f64,
) {
    let mut occupied = Vec::<Rect>::new();
    for component in components
        .iter()
        .filter(|c| c.iter().any(|&i| input.nodes[i].fixed.is_some()))
    {
        if let Some(&anchor) = component.iter().find(|&&i| input.nodes[i].fixed.is_some()) {
            let node = &input.nodes[anchor];
            if let (Some(fixed), Some(current)) = (node.fixed, positions.get(&node.id).copied()) {
                let delta = Point::new(fixed.x - current.x, fixed.y - current.y);
                for &i in component {
                    let node = &input.nodes[i];
                    if let Some(p) = positions.get_mut(&node.id) {
                        *p = node
                            .fixed
                            .unwrap_or(Point::new(p.x + delta.x, p.y + delta.y));
                    }
                }
            }
        }
        if let Some(bounds) = component_bounds(input, component, positions) {
            occupied.push(bounds);
        }
    }
    let max_width = components
        .iter()
        .filter_map(|c| component_bounds(input, c, positions))
        .map(|r| (r.size.width + gap) * (r.size.height + gap))
        .sum::<f64>()
        .sqrt()
        .max(1000.0);
    let mut cursor = Point::default();
    let mut row_height: f64 = 0.0;
    for component in components
        .iter()
        .filter(|c| !c.iter().any(|&i| input.nodes[i].fixed.is_some()))
    {
        let Some(bounds) = component_bounds(input, component, positions) else {
            continue;
        };
        if cursor.x > 0.0 && cursor.x + bounds.size.width > max_width {
            cursor.x = 0.0;
            cursor.y += row_height + gap;
            row_height = 0.0;
        }
        // Jump past pinned components, rather than repeatedly nudging by a pixel.
        loop {
            let candidate = Rect::new(cursor, bounds.size).inflate(gap / 2.0);
            let collision = occupied.iter().find(|r| {
                let other = r.inflate(gap / 2.0);
                candidate.overlaps(other)
            });
            match collision {
                Some(rect) => cursor.x = (rect.right() + gap).max(cursor.x + 1e-6),
                None => break,
            }
        }
        let delta = Point::new(cursor.x - bounds.origin.x, cursor.y - bounds.origin.y);
        for &i in component {
            if let Some(p) = positions.get_mut(&input.nodes[i].id) {
                p.x += delta.x;
                p.y += delta.y;
            }
        }
        occupied.push(Rect::new(cursor, bounds.size));
        cursor.x += bounds.size.width + gap;
        row_height = row_height.max(bounds.size.height);
    }
    for node in &input.nodes {
        if let Some(fixed) = node.fixed {
            positions.insert(node.id.clone(), fixed);
        }
    }
}
fn component_bounds(
    input: &LayoutInput,
    component: &[usize],
    positions: &IndexMap<NodeId, Point>,
) -> Option<Rect> {
    component
        .iter()
        .filter_map(|&i| {
            let node = &input.nodes[i];
            positions.get(&node.id).map(|&p| Rect::new(p, node.size))
        })
        .reduce(Rect::union)
}
/// Separate rectangles along the axis of least overlap; pins are never moved.
/// A final ordered placement guarantees clearance even for a dense initial pile.
pub(super) fn remove_overlaps(
    input: &LayoutInput,
    positions: &mut IndexMap<NodeId, Point>,
    padding: f64,
) {
    // Keep the quadratic collision loop in indexed arrays. Looking up string ids
    // in the ordered map for every pair dominates layout time in debug builds.
    let mut points: Vec<_> = input
        .nodes
        .iter()
        .map(|node| positions.get(&node.id).copied())
        .collect();
    for _ in 0..80 {
        let mut changed = false;
        for i in 0..input.nodes.len() {
            for j in i + 1..input.nodes.len() {
                let (a, b) = (&input.nodes[i], &input.nodes[j]);
                if a.fixed.is_some() && b.fixed.is_some() {
                    continue;
                }
                let (Some(pa), Some(pb)) = (points[i], points[j]) else {
                    continue;
                };
                let ox = (pa.x + a.size.width).min(pb.x + b.size.width) - pa.x.max(pb.x) + padding;
                let oy =
                    (pa.y + a.size.height).min(pb.y + b.size.height) - pa.y.max(pb.y) + padding;
                if ox <= 0.0 || oy <= 0.0 {
                    continue;
                }
                changed = true;
                let delta = if ox < oy {
                    Point::new(
                        if pa.x + a.size.width / 2.0 <= pb.x + b.size.width / 2.0 {
                            -ox - 0.001
                        } else {
                            ox + 0.001
                        },
                        0.0,
                    )
                } else {
                    Point::new(
                        0.0,
                        if pa.y + a.size.height / 2.0 <= pb.y + b.size.height / 2.0 {
                            -oy - 0.001
                        } else {
                            oy + 0.001
                        },
                    )
                };
                let share_a = if a.fixed.is_some() {
                    0.0
                } else if b.fixed.is_some() {
                    1.0
                } else {
                    0.5
                };
                let share_b = 1.0 - share_a;
                points[i] = Some(Point::new(
                    pa.x + delta.x * share_a,
                    pa.y + delta.y * share_a,
                ));
                points[j] = Some(Point::new(
                    pb.x - delta.x * share_b,
                    pb.y - delta.y * share_b,
                ));
            }
        }
        if !changed {
            break;
        }
    }
    for (node, point) in input.nodes.iter().zip(points) {
        if let Some(point) = point {
            positions.insert(node.id.clone(), point);
        }
    }
    let mut placed: Vec<Rect> = input
        .nodes
        .iter()
        .filter_map(|n| n.fixed.map(|p| Rect::new(p, n.size).inflate(padding / 2.0)))
        .collect();
    for node in input.nodes.iter().filter(|n| n.fixed.is_none()) {
        let Some(mut position) = positions.get(&node.id).copied() else {
            continue;
        };
        loop {
            let rect = Rect::new(position, node.size).inflate(padding / 2.0);
            let Some(other) = placed.iter().find(|other| rect.overlaps(**other)) else {
                break;
            };
            position.x = other.right() + padding / 2.0 + 0.001;
        }
        placed.push(Rect::new(position, node.size).inflate(padding / 2.0));
        positions.insert(node.id.clone(), position);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn fixture(count: usize) -> LayoutInput {
        LayoutInput {
            nodes: (0..count)
                .map(|i| LayoutNode {
                    id: format!("n{i:03}").into(),
                    size: Size::new(40.0 + (i * 17 % 70) as f64, 25.0 + (i * 29 % 90) as f64),
                    fixed: None,
                    previous: None,
                    layout_parent: (i > 0).then(|| format!("n{:03}", (i - 1) / 2).into()),
                    order_key: None,
                    branch_side: None,
                })
                .collect(),
            edges: Vec::new(),
            roots: vec!["n000".into()],
        }
    }
    pub(super) fn assert_clear(input: &LayoutInput, output: &LayoutOutput) {
        for (i, a) in input.nodes.iter().enumerate() {
            for b in &input.nodes[i + 1..] {
                let ra = Rect::new(output.positions[&a.id], a.size);
                let rb = Rect::new(output.positions[&b.id], b.size);
                assert!(
                    ra.right() <= rb.origin.x + 0.0001
                        || rb.right() <= ra.origin.x + 0.0001
                        || ra.bottom() <= rb.origin.y + 0.0001
                        || rb.bottom() <= ra.origin.y + 0.0001,
                    "{} overlaps {}: {ra:?}, {rb:?}",
                    a.id,
                    b.id
                );
            }
        }
    }
    #[test]
    fn algorithms_obey_contract() {
        let algorithms: Vec<Box<dyn LayoutAlgorithm>> = vec![
            Box::new(TreeLayout::default()),
            Box::new(TreeLayout(TreeLayoutOptions {
                direction: TreeDirection::LeftRight,
                ..Default::default()
            })),
            Box::new(MindMapLayout::default()),
            Box::new(ForceLayout(ForceOptions {
                iterations: 40,
                ..Default::default()
            })),
            Box::new(RadialLayout(RadialOptions {
                focus: Some("n000".into()),
                ..Default::default()
            })),
        ];
        for algorithm in algorithms {
            for count in [0, 1, 2, 7, 31] {
                let input = fixture(count);
                let output = algorithm.layout(&input);
                assert_eq!(output.positions.len(), count);
                assert_eq!(output, algorithm.layout(&input));
                assert_clear(&input, &output);
            }
            let mut input = fixture(12);
            input.nodes[3].fixed = Some(Point::new(-120.0, -80.0));
            input.nodes[7].layout_parent = None;
            input.edges.push(LayoutEdge {
                source: "n011".into(),
                target: "n000".into(),
                weight: 1.0,
            });
            let output = algorithm.layout(&input);
            assert_eq!(
                output.positions[&input.nodes[3].id],
                Point::new(-120.0, -80.0)
            );
            assert_clear(&input, &output);
        }
    }
    #[test]
    fn pack_components_accepts_touching_padding_boundaries() {
        let mut input = fixture(3);
        for node in &mut input.nodes {
            node.layout_parent = None;
            node.size = Size::new(40.0, 20.0);
        }
        let mut positions = input
            .nodes
            .iter()
            .map(|node| (node.id.clone(), Point::default()))
            .collect();
        pack_components(&input, &[vec![0], vec![1], vec![2]], &mut positions, 10.0);
        for (index, node) in input.nodes.iter().enumerate() {
            assert_eq!(positions[&node.id], Point::new(index as f64 * 50.0, 0.0));
        }
        // Fractional measurements may leave a sub-ulp intersection after a
        // jump. Packing must still make progress instead of repeating it.
        for width in [40.1, 40.123456789, 40.99999999] {
            for node in &mut input.nodes {
                node.size.width = width;
            }
            for point in positions.values_mut() {
                *point = Point::new(-0.17, -0.31);
            }
            pack_components(&input, &[vec![0], vec![1], vec![2]], &mut positions, 10.1);
            for (index, node) in input.nodes.iter().enumerate() {
                assert!((positions[&node.id].x - index as f64 * (width + 10.1)).abs() < 1e-4);
            }
        }
    }
    #[test]
    fn overlap_removal_keeps_requested_clearance_from_fixed_nodes() {
        let mut input = fixture(2);
        for node in &mut input.nodes {
            node.size = Size::new(100.0, 60.0);
            node.layout_parent = None;
        }
        input.nodes[0].fixed = Some(Point::new(0.0, 0.0));
        let mut positions = IndexMap::from([
            (input.nodes[0].id.clone(), Point::new(0.0, 0.0)),
            (input.nodes[1].id.clone(), Point::new(10.0, 50.0)),
        ]);
        remove_overlaps(&input, &mut positions, 24.0);
        let fixed = Rect::new(positions[&input.nodes[0].id], input.nodes[0].size);
        let moved = Rect::new(positions[&input.nodes[1].id], input.nodes[1].size);
        assert_eq!(fixed.origin, Point::new(0.0, 0.0), "pins never move");
        assert!(
            moved.origin.y >= fixed.bottom() + 23.99 || moved.origin.x >= fixed.right() + 23.99,
            "{moved:?} must clear {fixed:?} by the layout gap"
        );
    }
    #[test]
    fn manual_preserves_previous_and_fixed() {
        let mut input = fixture(2);
        input.nodes[0].previous = Some(Point::new(3.0, 4.0));
        input.nodes[1].fixed = Some(Point::new(7.0, 8.0));
        let output = run_layout(&LayoutConfig::Manual, &input);
        assert_eq!(output.positions[&input.nodes[0].id], Point::new(3.0, 4.0));
        assert_eq!(output.positions[&input.nodes[1].id], Point::new(7.0, 8.0));
    }

    #[test]
    fn many_components_pack_in_rows_using_area() {
        let mut input = fixture(100);
        for node in &mut input.nodes {
            node.layout_parent = None;
            node.size = Size::new(400.0, 400.0);
        }
        let components: Vec<_> = (0..100).map(|i| vec![i]).collect();
        let mut positions = input
            .nodes
            .iter()
            .map(|n| (n.id.clone(), Point::default()))
            .collect();
        pack_components(&input, &components, &mut positions, COMPONENT_GAP);
        let output = LayoutOutput { positions };
        assert_clear(&input, &output);
        let bounds = input
            .nodes
            .iter()
            .map(|n| Rect::new(output.positions[&n.id], n.size))
            .reduce(Rect::union)
            .unwrap();
        assert!(bounds.size.width / bounds.size.height > 0.8);
        assert!(bounds.size.width / bounds.size.height < 1.2);
    }

    #[test]
    fn custom_layout_dispatch_and_pointer_equality() {
        struct Custom;
        impl LayoutAlgorithm for Custom {
            fn layout(&self, input: &LayoutInput) -> LayoutOutput {
                LayoutOutput {
                    positions: input
                        .nodes
                        .iter()
                        .map(|n| (n.id.clone(), Point::new(12.0, 34.0)))
                        .collect(),
                }
            }
        }
        let config = LayoutConfig::Custom(Rc::new(Custom));
        assert_eq!(config, config.clone());
        assert_ne!(config, LayoutConfig::Custom(Rc::new(Custom)));
        assert_eq!(
            run_layout(&config, &fixture(1)).positions[&NodeId::from("n000")],
            Point::new(12.0, 34.0)
        );
    }

    /// A small idea map with one very wide idea and a cross-link, as built in
    /// the graph block: Central → Research (→ Interview, Recruit), Design,
    /// Build → Prototype → Test, and Test ⇢ Research.
    pub(super) fn ideas() -> LayoutInput {
        let spec: [(&str, f64, f64, Option<&str>); 8] = [
            ("central", 140.0, 60.0, None),
            ("research", 90.0, 45.0, Some("central")),
            ("design", 80.0, 45.0, Some("central")),
            ("build", 80.0, 45.0, Some("central")),
            ("interview", 560.0, 115.0, Some("research")),
            ("recruit", 160.0, 45.0, Some("research")),
            ("prototype", 100.0, 45.0, Some("build")),
            ("test", 120.0, 45.0, Some("prototype")),
        ];
        let mut input = LayoutInput {
            roots: vec!["central".into()],
            ..Default::default()
        };
        for (index, (id, width, height, parent)) in spec.into_iter().enumerate() {
            input.nodes.push(LayoutNode {
                id: id.into(),
                size: Size::new(width, height),
                fixed: None,
                previous: None,
                layout_parent: parent.map(Into::into),
                order_key: Some(format!("{index:04}")),
                branch_side: None,
            });
            if let Some(parent) = parent {
                input.edges.push(LayoutEdge {
                    source: parent.into(),
                    target: id.into(),
                    weight: 1.0,
                });
            }
        }
        input.edges.push(LayoutEdge {
            source: "test".into(),
            target: "research".into(),
            weight: 1.0,
        });
        input
    }
    /// A random tree with occasional very wide ideas and cross-links.
    pub(super) fn random_ideas(count: usize, seed: u64) -> LayoutInput {
        let mut rng = rng::Rng::new(seed);
        let mut input = fixture(count);
        input.edges.clear();
        for i in 0..count {
            let wide = rng.next() < 0.1;
            input.nodes[i].size = Size::new(
                if wide {
                    560.0
                } else {
                    60.0 + rng.next() * 160.0
                },
                30.0 + rng.next() * if wide { 90.0 } else { 40.0 },
            );
            if i > 0 {
                let parent = input.nodes[(rng.next() * i as f64) as usize].id.clone();
                input.nodes[i].layout_parent = Some(parent.clone());
                input.edges.push(LayoutEdge {
                    source: parent,
                    target: input.nodes[i].id.clone(),
                    weight: 1.0,
                });
            }
            if i > 2 && rng.next() < 0.15 {
                input.edges.push(LayoutEdge {
                    source: input.nodes[i].id.clone(),
                    target: input.nodes[(rng.next() * i as f64) as usize].id.clone(),
                    weight: 1.0,
                });
            }
        }
        input
    }
    pub(super) fn bounds(input: &LayoutInput, output: &LayoutOutput) -> Rect {
        input
            .nodes
            .iter()
            .map(|n| Rect::new(output.positions[&n.id], n.size))
            .reduce(Rect::union)
            .unwrap_or_default()
    }
    #[test]
    fn generated_idea_maps_never_overlap_and_are_deterministic() {
        let configs = [
            LayoutConfig::Tree(Default::default()),
            LayoutConfig::MindMap(Default::default()),
            LayoutConfig::Radial(Default::default()),
            LayoutConfig::Force(Default::default()),
        ];
        for config in &configs {
            for seed in 0..12 {
                let input = random_ideas(5 + seed as usize * 4, seed);
                let output = run_layout(config, &input);
                assert_clear(&input, &output);
                assert_eq!(
                    output,
                    run_layout(config, &input),
                    "{config:?} is deterministic"
                );
            }
        }
    }
    #[test]
    fn fresh_input_keeps_only_model_pins() {
        let mut model = crate::GraphModel::<(), ()>::default();
        for id in ["a", "b", "c"] {
            let mut node = crate::GraphNode::new(id, ());
            node.position = Some(Point::new(5.0, 5.0));
            node.pinned = id == "b";
            node.layout_parent = (id != "a").then(|| "a".into());
            model.insert_node(node).unwrap();
        }
        let input = LayoutInput::fresh(&model, |_| Size::new(10.0, 10.0));
        assert_eq!(input.roots, vec![NodeId::from("a")]);
        assert!(input.nodes.iter().all(|n| n.previous.is_none()));
        let fixed: Vec<_> = input.nodes.iter().map(|n| n.fixed.is_some()).collect();
        assert_eq!(fixed, [false, true, false]);
    }
}
