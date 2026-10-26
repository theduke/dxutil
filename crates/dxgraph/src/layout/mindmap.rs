//! Greedily balanced left/right trees around a centered root.
use super::{
    LayoutAlgorithm, LayoutInput, LayoutOutput, TreeDirection, TreeLayoutOptions, pack_components,
    spanning::{Forest, forest},
    tree::tidy_positions,
};
use crate::{NodeId, Point};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MindMapBalance {
    #[default]
    Auto,
    AllRight,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MindMapOptions {
    pub h_gap: f64,
    pub v_gap: f64,
    pub balance: MindMapBalance,
}
impl Default for MindMapOptions {
    fn default() -> Self {
        Self {
            h_gap: 80.0,
            v_gap: 24.0,
            balance: MindMapBalance::Auto,
        }
    }
}
/// The side of its root a first-level mind map branch grows on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BranchSide {
    Left,
    Right,
}
#[derive(Clone, Debug, Default)]
pub struct MindMapLayout(pub MindMapOptions);
impl MindMapLayout {
    /// The side each first-level branch of every root is laid out on (see
    /// `root_sides`). Hosts persist these so the arrangement is identical
    /// wherever the map is opened.
    pub fn branch_sides(&self, input: &LayoutInput) -> IndexMap<NodeId, BranchSide> {
        let f = forest(input);
        let extents = self.extents(input, &f);
        f.roots
            .iter()
            .flat_map(|&root| {
                let sides = self.root_sides(input, &f, root, &extents);
                f.children[root]
                    .iter()
                    .map(|&child| input.nodes[child].id.clone())
                    .zip(sides)
                    .collect::<Vec<_>>()
            })
            .collect()
    }
    /// The vertical space each subtree needs.
    fn extents(&self, input: &LayoutInput, f: &Forest) -> Vec<f64> {
        let mut extents = vec![0.0; input.nodes.len()];
        for component in &f.components {
            for &i in component.iter().rev() {
                extents[i] = input.nodes[i].size.height.max(
                    f.children[i].iter().map(|&j| extents[j]).sum::<f64>()
                        + f.children[i].len().saturating_sub(1) as f64 * self.0.v_gap.max(0.0),
                );
            }
        }
        extents
    }
    fn root_sides(
        &self,
        input: &LayoutInput,
        f: &Forest,
        root: usize,
        extents: &[f64],
    ) -> Vec<BranchSide> {
        if self.0.balance == MindMapBalance::Auto {
            root_sides(input, root, &f.children[root], extents, self.0.v_gap)
        } else {
            vec![BranchSide::Right; f.children[root].len()]
        }
    }
}
impl LayoutAlgorithm for MindMapLayout {
    fn layout(&self, input: &LayoutInput) -> LayoutOutput {
        let f = forest(input);
        let mut children = f.children.clone();
        let by_id: std::collections::BTreeMap<_, _> =
            input.nodes.iter().map(|n| (&n.id, n)).collect();
        let mut output = LayoutOutput::default();
        let extents = self.extents(input, &f);
        for &root in &f.roots {
            let sides = self.root_sides(input, &f, root, &extents);
            let (mut left, mut right) = (Vec::new(), Vec::new());
            for (&child, side) in f.children[root].iter().zip(sides) {
                match side {
                    BranchSide::Left => left.push(child),
                    BranchSide::Right => right.push(child),
                }
            }
            for (branches, mirror) in [(left, true), (right, false)] {
                children[root] = branches;
                let side = tidy_positions(
                    input,
                    &children,
                    &[root],
                    &TreeLayoutOptions {
                        direction: TreeDirection::LeftRight,
                        sibling_gap: self.0.v_gap,
                        level_gap: self.0.h_gap,
                    },
                );
                for (id, p) in side.positions {
                    let Some(node) = by_id.get(&id) else {
                        continue;
                    };
                    let x = if mirror {
                        -p.x - node.size.width + input.nodes[root].size.width / 2.0
                    } else {
                        p.x - input.nodes[root].size.width / 2.0
                    };
                    output.positions.insert(id, Point::new(x, p.y));
                }
                children[root] = f.children[root].clone();
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
            super::remove_overlaps(input, &mut output.positions, self.0.v_gap.max(0.0));
        }
        output
    }
}
/// Choose a side for each first-level branch of `root`.
///
/// A branch with a [`super::LayoutNode::branch_side`] always takes it. Others
/// keep the side they were last shown on (their previous centre relative to
/// the root's), so growing one branch never moves another across the map.
/// New branches inserted between siblings join their preceding sibling; others
/// go to the side with less content. Without any stored sides or previous
/// positions this is a deterministic greedy balance by subtree extent.
fn root_sides(
    input: &LayoutInput,
    root: usize,
    branches: &[usize],
    extents: &[f64],
    gap: f64,
) -> Vec<BranchSide> {
    let center_x = |i: usize| {
        input.nodes[i]
            .previous
            .map(|p| p.x + input.nodes[i].size.width / 2.0)
    };
    let root_x = center_x(root).unwrap_or(0.0);
    let mut sides: Vec<_> = branches
        .iter()
        .map(|&child| {
            input.nodes[child].branch_side.or_else(|| {
                center_x(child).map(|x| {
                    if x < root_x {
                        BranchSide::Left
                    } else {
                        BranchSide::Right
                    }
                })
            })
        })
        .collect();
    let weight = |i: usize| extents[i] + gap.max(0.0);
    let (mut left, mut right) = (0.0, 0.0);
    for (&child, side) in branches.iter().zip(&sides) {
        match side {
            Some(BranchSide::Left) => left += weight(child),
            Some(BranchSide::Right) => right += weight(child),
            None => {}
        }
    }
    for index in 0..branches.len() {
        if sides[index].is_some() {
            continue;
        }
        let inserted = sides[index + 1..].iter().any(Option::is_some);
        let side = match index.checked_sub(1).and_then(|before| sides[before]) {
            Some(side) if inserted => side,
            _ if left < right => BranchSide::Left,
            _ => BranchSide::Right,
        };
        match side {
            BranchSide::Left => left += weight(branches[index]),
            BranchSide::Right => right += weight(branches[index]),
        }
        sides[index] = Some(side);
    }
    sides
        .into_iter()
        .map(|side| side.unwrap_or(BranchSide::Right))
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::tests::fixture;
    #[test]
    fn center_and_balanced_sides() {
        let input = fixture(7);
        let output = MindMapLayout::default().layout(&input);
        let root = &input.nodes[0];
        let center = output.positions[&root.id];
        assert_eq!(
            center,
            Point::new(-root.size.width / 2.0, -root.size.height / 2.0)
        );
        let right = output.positions[&input.nodes[1].id].x;
        let left = output.positions[&input.nodes[2].id].x;
        assert!(left < 0.0 && right > 0.0);
    }
    #[test]
    fn all_right() {
        let input = fixture(10);
        let output = MindMapLayout(MindMapOptions {
            balance: MindMapBalance::AllRight,
            ..Default::default()
        })
        .layout(&input);
        for node in &input.nodes[1..] {
            assert!(output.positions[&node.id].x > 0.0);
        }
    }
    use crate::{
        Size,
        layout::{
            LayoutNode,
            tests::{assert_clear, random_ideas},
        },
    };
    fn star(branches: usize) -> LayoutInput {
        let mut input = fixture(branches + 1);
        for node in &mut input.nodes {
            node.size = Size::new(100.0, 40.0);
            node.layout_parent = (node.id != input.roots[0]).then(|| input.roots[0].clone());
        }
        input
    }
    fn center_x(input: &LayoutInput, output: &LayoutOutput, index: usize) -> f64 {
        output.positions[&input.nodes[index].id].x + input.nodes[index].size.width / 2.0
    }
    fn with_previous(mut input: LayoutInput, output: &LayoutOutput) -> LayoutInput {
        for node in &mut input.nodes {
            node.previous = output.positions.get(&node.id).copied();
        }
        input
    }
    #[test]
    fn growing_a_branch_never_moves_other_branches_across() {
        let input = star(3);
        let first = MindMapLayout::default().layout(&input);
        let sides = |input: &LayoutInput, output: &LayoutOutput| {
            (1..4)
                .map(|i| center_x(input, output, i) < center_x(input, output, 0))
                .collect::<Vec<_>>()
        };
        assert_eq!(sides(&input, &first), [false, true, false]);
        // The first branch grows tall: a greedy rebalance would move the
        // third branch to the left; a kept side does not.
        let mut grown = with_previous(input, &first);
        for i in 0..6 {
            grown.nodes.push(LayoutNode {
                id: format!("tall{i}").into(),
                size: Size::new(100.0, 120.0),
                fixed: None,
                previous: None,
                layout_parent: Some(grown.nodes[1].id.clone()),
                order_key: None,
                branch_side: None,
            });
        }
        let mut fresh = grown.clone();
        for node in &mut fresh.nodes {
            node.previous = None;
        }
        let rebalanced = MindMapLayout::default().layout(&fresh);
        assert_eq!(sides(&fresh, &rebalanced), [false, true, true]);
        let output = MindMapLayout::default().layout(&grown);
        assert_eq!(sides(&grown, &output), [false, true, false]);
        assert_clear(&grown, &output);
    }
    #[test]
    fn new_branches_join_their_preceding_sibling_or_the_lighter_side() {
        let input = star(3);
        let first = MindMapLayout::default().layout(&input);
        let mut next = with_previous(star(5), &first);
        // n004 is inserted after the left branch n002; n005 is appended while
        // the right side holds more content.
        next.nodes[1].size.height = 200.0;
        for (index, key) in [(1, "a"), (2, "b"), (4, "c"), (3, "d"), (5, "e")] {
            next.nodes[index].order_key = Some(key.into());
        }
        let output = MindMapLayout::default().layout(&next);
        let left = |i: usize| center_x(&next, &output, i) < center_x(&next, &output, 0);
        assert!(left(2) && left(4), "inserted after a left branch");
        assert!(!left(1) && !left(3));
        assert!(left(5), "appended to the lighter side");
    }
    #[test]
    fn dropping_a_branch_across_the_root_switches_its_side() {
        let input = star(3);
        let first = MindMapLayout::default().layout(&input);
        let mut moved = with_previous(input, &first);
        moved.nodes[3].previous = Some(Point::new(-400.0, 0.0));
        let output = MindMapLayout::default().layout(&moved);
        assert!(center_x(&moved, &output, 3) < center_x(&moved, &output, 0));
    }
    #[test]
    fn stored_sides_win_over_balance_and_previous_positions() {
        let input = star(4);
        let first = MindMapLayout::default().layout(&input);
        let mut stored = with_previous(input, &first);
        // Every branch on the left, even one last shown on the right.
        for node in &mut stored.nodes[1..] {
            node.branch_side = Some(BranchSide::Left);
        }
        let output = MindMapLayout::default().layout(&stored);
        for i in 1..5 {
            assert!(center_x(&stored, &output, i) < center_x(&stored, &output, 0));
        }
        assert_clear(&stored, &output);
        // The same stored sides give the same arrangement without any
        // previous positions: a reload or another device shows it too.
        let mut fresh = stored.clone();
        for node in &mut fresh.nodes {
            node.previous = None;
        }
        assert_eq!(MindMapLayout::default().layout(&fresh), output);
    }
    #[test]
    fn branch_sides_report_the_laid_out_sides() {
        let mut input = star(5);
        input.nodes[2].branch_side = Some(BranchSide::Right);
        let layout = MindMapLayout::default();
        let sides = layout.branch_sides(&input);
        let output = layout.layout(&input);
        assert_eq!(sides.len(), 5);
        assert_eq!(sides[&input.nodes[2].id], BranchSide::Right);
        for i in 1..6 {
            let left = center_x(&input, &output, i) < center_x(&input, &output, 0);
            assert_eq!(
                sides[&input.nodes[i].id],
                if left {
                    BranchSide::Left
                } else {
                    BranchSide::Right
                }
            );
        }
        // A new branch without a side joins the lighter side.
        input.nodes[1].size.height = 400.0;
        for node in &mut input.nodes[1..5] {
            node.branch_side = Some(BranchSide::Right);
        }
        assert_eq!(
            layout.branch_sides(&input)[&input.nodes[5].id],
            BranchSide::Left
        );
    }
    #[test]
    fn descendants_stay_on_their_branch_side() {
        for seed in 0..10 {
            let input = random_ideas(40, seed);
            let output = MindMapLayout::default().layout(&input);
            let root = center_x(&input, &output, 0);
            let index: std::collections::HashMap<_, _> = input
                .nodes
                .iter()
                .enumerate()
                .map(|(i, n)| (&n.id, i))
                .collect();
            for (i, node) in input.nodes.iter().enumerate().skip(1) {
                let mut branch = i;
                while let Some(parent) = &input.nodes[branch].layout_parent {
                    if index[parent] == 0 {
                        break;
                    }
                    branch = index[parent];
                }
                let side = center_x(&input, &output, branch) < root;
                assert_eq!(
                    center_x(&input, &output, i) < root,
                    side,
                    "{} must stay on its branch's side",
                    node.id
                );
            }
        }
    }
}
