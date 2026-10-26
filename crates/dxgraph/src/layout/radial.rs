//! Radial tree: hop-distance rings around a focus, with each subtree drawn in
//! an angular wedge proportional to its leaf count so children stay next to
//! their parent. Ring radii follow the actual node rectangles: a node is placed
//! at the smallest radius that clears everything already placed, so one wide
//! node pushes out only itself instead of inflating every ring.
use super::{
    LayoutAlgorithm, LayoutInput, LayoutOutput, pack_components, spanning::forest_with_roots,
};
use crate::{NodeId, Point, Size};
use serde::{Deserialize, Serialize};
use std::f64::consts::{PI, TAU};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RadialOptions {
    pub focus: Option<NodeId>,
    /// Clearance between a node and the rings inside it.
    pub ring_gap: f64,
}
impl Default for RadialOptions {
    fn default() -> Self {
        Self {
            focus: None,
            ring_gap: 64.0,
        }
    }
}
/// Clearance between neighbours on the same ring.
const SIBLING_GAP: f64 = 24.0;
#[derive(Clone, Debug, Default)]
pub struct RadialLayout(pub RadialOptions);
impl LayoutAlgorithm for RadialLayout {
    fn layout(&self, input: &LayoutInput) -> LayoutOutput {
        let mut roots = input.roots.clone();
        if let Some(focus) = self
            .0
            .focus
            .as_ref()
            .filter(|id| input.nodes.iter().any(|n| &n.id == *id))
        {
            roots.retain(|r| r != focus);
            roots.insert(0, focus.clone());
        }
        let f = forest_with_roots(input, &roots);
        let mut output = LayoutOutput::default();
        let ring_gap = self.0.ring_gap.max(0.0);
        for (&root, component) in f.roots.iter().zip(&f.components) {
            let angles = wedge_angles(&f.children, root, component, input.nodes.len());
            let mut placed: Vec<Placed> = Vec::with_capacity(component.len());
            let mut place = |placed: &mut Vec<Placed>, i: usize, center: Point, level: usize| {
                let size = input.nodes[i].size;
                placed.push(Placed {
                    center,
                    size,
                    level,
                });
                output.positions.insert(
                    input.nodes[i].id.clone(),
                    Point::new(center.x - size.width / 2.0, center.y - size.height / 2.0),
                );
            };
            place(&mut placed, root, Point::default(), 0);
            // Each node sits at the smallest radius beyond its parent that
            // clears the inner rings by the ring gap and earlier neighbours on
            // its own ring by the sibling gap. Similar nodes therefore share a
            // ring, while one very wide idea only pushes itself outwards.
            let mut radii = vec![0.0_f64; input.nodes.len()];
            let mut ring = vec![root];
            for level in 1.. {
                let parents: Vec<(usize, usize)> = ring
                    .iter()
                    .flat_map(|&p| f.children[p].iter().map(move |&c| (c, p)))
                    .collect();
                if parents.is_empty() {
                    break;
                }
                ring = parents.iter().map(|&(child, _)| child).collect();
                // Crowded rings grow until angular neighbours fit side by side.
                let crowded = ring_radius(
                    (0..ring.len())
                        .filter_map(|index| {
                            let next = (index + 1) % ring.len();
                            (next != index).then(|| {
                                neighbour_radius(
                                    (angles[ring[index]], input.nodes[ring[index]].size),
                                    (angles[ring[next]], input.nodes[ring[next]].size),
                                )
                            })
                        })
                        .collect(),
                );
                let clear: Vec<f64> = parents
                    .iter()
                    .map(|&(i, parent)| {
                        let from = radii[parent].max(crowded);
                        clear_radius(angles[i], input.nodes[i].size, from, &placed, |_| ring_gap)
                    })
                    .collect();
                // Align ordinary ideas on one ring; outliers stay outside it.
                let common = ring_radius(clear.clone());
                for (&i, &clear) in ring.iter().zip(&clear) {
                    let from = if clear <= common { common } else { clear };
                    let r = clear_radius(angles[i], input.nodes[i].size, from, &placed, |other| {
                        if other == level {
                            SIBLING_GAP
                        } else {
                            ring_gap
                        }
                    });
                    radii[i] = r;
                    let center = Point::new(r * angles[i].cos(), r * angles[i].sin());
                    place(&mut placed, i, center, level);
                }
            }
        }
        if f.components.len() > 1 || input.nodes.iter().any(|n| n.fixed.is_some()) {
            pack_components(
                input,
                &f.components,
                &mut output.positions,
                super::COMPONENT_GAP,
            );
        }
        if input.nodes.iter().any(|n| n.fixed.is_some()) {
            // New ideas placed around kept nodes keep the layout's spacing.
            super::remove_overlaps(input, &mut output.positions, super::FIXED_NODE_CLEARANCE);
        }
        output
    }
}
struct Placed {
    center: Point,
    size: Size,
    level: usize,
}
/// The angle of every node in `component`: subtrees get wedges proportional to
/// their leaf count, the first branch centred at the top, clockwise.
fn wedge_angles(children: &[Vec<usize>], root: usize, component: &[usize], n: usize) -> Vec<f64> {
    let mut leaves = vec![1.0_f64; n];
    // Components are in BFS order, so reversing visits children first.
    for &i in component.iter().rev() {
        if !children[i].is_empty() {
            leaves[i] = children[i].iter().map(|&c| leaves[c]).sum();
        }
    }
    let mut angles = vec![-PI / 2.0; n];
    let first = children[root].first().map_or(0.0, |&c| leaves[c]);
    let mut wedges = vec![(root, -PI / 2.0 - PI * first / leaves[root])];
    while let Some((i, start)) = wedges.pop() {
        let span = TAU * leaves[i] / leaves[root];
        if i != root {
            angles[i] = start + span / 2.0;
        }
        let mut cursor = start;
        for &child in &children[i] {
            wedges.push((child, cursor));
            cursor += TAU * leaves[child] / leaves[root];
        }
    }
    angles
}
/// The smallest radius `>= from` at which a rectangle centred on the ray at
/// `angle` overlaps none of `placed` (each inflated by `gap(level)`).
fn clear_radius(
    angle: f64,
    size: Size,
    from: f64,
    placed: &[Placed],
    gap: impl Fn(usize) -> f64,
) -> f64 {
    let (ux, uy) = (angle.cos(), angle.sin());
    // Along the ray the overlap with each rectangle is one open interval.
    let mut blocked: Vec<(f64, f64)> = placed
        .iter()
        .filter_map(|other| {
            let g = gap(other.level);
            let w = (size.width + other.size.width) / 2.0 + g;
            let h = (size.height + other.size.height) / 2.0 + g;
            let (x0, x1) = axis_interval(ux, other.center.x, w)?;
            let (y0, y1) = axis_interval(uy, other.center.y, h)?;
            let (lo, hi) = (x0.max(y0), x1.min(y1));
            (lo < hi).then_some((lo, hi))
        })
        .collect();
    blocked.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut r = from;
    for (lo, hi) in blocked {
        if lo < r && r < hi {
            r = hi + 1e-6;
        }
    }
    r
}
/// The radii `r` with `|r * u - center| < half` on one axis.
fn axis_interval(u: f64, center: f64, half: f64) -> Option<(f64, f64)> {
    if u.abs() < 1e-12 {
        return (center.abs() < half).then_some((f64::NEG_INFINITY, f64::INFINITY));
    }
    let (a, b) = ((center - half) / u, (center + half) / u);
    Some((a.min(b), a.max(b)))
}
/// The smallest common radius at which two ring neighbours do not overlap.
fn neighbour_radius((a, sa): (f64, Size), (b, sb): (f64, Size)) -> f64 {
    let w = (sa.width + sb.width) / 2.0 + SIBLING_GAP;
    let h = (sa.height + sb.height) / 2.0 + SIBLING_GAP;
    let dx = (a.cos() - b.cos()).abs();
    let dy = (a.sin() - b.sin()).abs();
    let along = |gap: f64, delta: f64| {
        if delta < 1e-12 {
            f64::INFINITY
        } else {
            gap / delta
        }
    };
    let radius = along(w, dx).min(along(h, dy));
    if radius.is_finite() { radius } else { 0.0 }
}
/// The largest requirement that is not an outlier: at most half again the
/// (lower) median requirement.
fn ring_radius(mut values: Vec<f64>) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f64::total_cmp);
    let limit = values[(values.len() - 1) / 2] * 1.25;
    values
        .into_iter()
        .rev()
        .find(|&value| value <= limit)
        .unwrap_or_default()
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::tests::fixture;
    #[test]
    fn hop_rings_increase() {
        let input = fixture(7);
        let output = RadialLayout(RadialOptions {
            focus: Some(input.nodes[0].id.clone()),
            ..Default::default()
        })
        .layout(&input);
        let center = |i: usize| {
            let p = output.positions[&input.nodes[i].id];
            Point::new(
                p.x + input.nodes[i].size.width / 2.0,
                p.y + input.nodes[i].size.height / 2.0,
            )
        };
        assert!(center(0).distance(Point::default()) < 0.001);
        assert!(center(1).distance(Point::default()) < center(3).distance(Point::default()));
        assert!(
            (center(1).distance(Point::default()) - center(2).distance(Point::default())).abs()
                < 0.001
        );
    }
    use crate::layout::tests::{assert_clear, bounds, ideas};
    #[test]
    fn a_wide_idea_pushes_only_itself_out() {
        let input = ideas();
        let output = RadialLayout::default().layout(&input);
        assert_clear(&input, &output);
        let bounds = bounds(&input, &output);
        // Fits a typical inline frame (900x480 inside the chrome) at >= 0.5.
        let fit = (900.0 / bounds.size.width).min(480.0 / bounds.size.height);
        assert!(fit >= 0.5, "fit zoom {fit} for {bounds:?}");
        let distance = |id: &str| {
            let node = input.nodes.iter().find(|n| n.id.0 == id).unwrap();
            let p = output.positions[&node.id];
            Point::new(p.x + node.size.width / 2.0, p.y + node.size.height / 2.0)
                .distance(Point::default())
        };
        // Ordinary ideas of one depth share a ring.
        assert!((distance("research") - distance("design")).abs() < 1.0);
        assert!((distance("research") - distance("build")).abs() < 1.0);
    }
    #[test]
    fn children_stay_within_their_parents_wedge() {
        let input = ideas();
        let output = RadialLayout::default().layout(&input);
        let angle = |id: &str| {
            let node = input.nodes.iter().find(|n| n.id.0 == id).unwrap();
            let p = output.positions[&node.id];
            (p.y + node.size.height / 2.0).atan2(p.x + node.size.width / 2.0)
        };
        let gap = |a: f64, b: f64| ((a - b + PI).rem_euclid(TAU) - PI).abs();
        // Research owns half the circle (two of four leaves).
        for child in ["interview", "recruit"] {
            assert!(gap(angle(child), angle("research")) <= PI / 2.0 + 1e-9);
        }
        assert!(gap(angle("prototype"), angle("build")) < 1e-6);
        assert!(gap(angle("test"), angle("build")) < 1e-6);
    }
}
