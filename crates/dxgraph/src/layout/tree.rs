//! Non-layered variable-size tidy trees using contour threads and deferred shifts.
// Algorithm adapted from d3-flextree by Chris Maloney (WTFPL v2),
// https://github.com/Klortho/d3-flextree/blob/master/src/flextree.js,
// implementing van der Ploeg, "Drawing non-layered tidy trees in linear time".
use super::{
    LayoutAlgorithm, LayoutInput, LayoutOutput, pack_components,
    spanning::{Forest, forest},
};
use crate::{NodeId, Point, Size};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TreeDirection {
    #[default]
    TopDown,
    LeftRight,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TreeLayoutOptions {
    pub direction: TreeDirection,
    pub sibling_gap: f64,
    pub level_gap: f64,
}
impl Default for TreeLayoutOptions {
    fn default() -> Self {
        Self {
            direction: TreeDirection::TopDown,
            sibling_gap: 24.0,
            level_gap: 80.0,
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct TreeLayout(pub TreeLayoutOptions);
#[derive(Clone, Default)]
struct Flex {
    breadth: f64,
    depth: f64,
    y: f64,
    x: f64,
    rel: f64,
    prelim: f64,
    shift: f64,
    change: f64,
    left_extreme: usize,
    right_extreme: usize,
    left_extreme_rel: f64,
    right_extreme_rel: f64,
    left_thread: Option<usize>,
    right_thread: Option<usize>,
}
impl LayoutAlgorithm for TreeLayout {
    fn layout(&self, input: &LayoutInput) -> LayoutOutput {
        let f = forest(input);
        let mut output = compact_positions(input, &f, &self.0);
        pack_components(
            input,
            &f.components,
            &mut output.positions,
            super::COMPONENT_GAP,
        );
        if input.nodes.iter().any(|n| n.fixed.is_some()) {
            // New ideas placed around kept nodes keep the layout's spacing.
            super::remove_overlaps(input, &mut output.positions, self.0.sibling_gap.max(0.0));
        }
        output
    }
}
/// Horizontal distance from a parent's centre, where the connector spine
/// runs, to the left side of its stacked leaf column.
pub const STACK_INDENT: f64 = 24.0;
/// Whether each node of `forest` stacks its children in an indented column
/// below itself (a compact org chart) instead of fanning them out in a row:
/// in top-down trees, every non-root idea whose two or more children are all
/// leaves. This keeps real-sized trees from growing into one very wide row.
fn stacking_parents(forest: &Forest, direction: TreeDirection) -> Vec<bool> {
    let mut stacks = vec![false; forest.children.len()];
    if direction != TreeDirection::TopDown {
        return stacks;
    }
    let roots: HashSet<_> = forest.roots.iter().copied().collect();
    for (parent, children) in forest.children.iter().enumerate() {
        stacks[parent] = !roots.contains(&parent)
            && children.len() >= 2
            && children
                .iter()
                .all(|&child| forest.children[child].is_empty());
    }
    stacks
}
/// The ideas a [`TreeLayout`] with `options` places in a stacked leaf column
/// under their parent. Their connectors should leave the parent's bottom and
/// enter the leaf's left side ([`STACK_INDENT`] right of the parent's centre).
pub fn stacked_leaves(input: &LayoutInput, options: &TreeLayoutOptions) -> HashSet<NodeId> {
    let forest = forest(input);
    stacking_parents(&forest, options.direction)
        .into_iter()
        .zip(&forest.children)
        .filter(|(stacks, _)| *stacks)
        .flat_map(|(_, children)| children.iter().map(|&i| input.nodes[i].id.clone()))
        .collect()
}
/// Vertical gap between the ideas of a stacked leaf column.
fn stack_gap(options: &TreeLayoutOptions) -> f64 {
    options.sibling_gap.max(0.0) * 0.75
}
/// Tidy tree positions where stacking parents (see [`stacking_parents`]) are
/// laid out as one block with their leaf column, which is placed afterwards.
fn compact_positions(
    input: &LayoutInput,
    forest: &Forest,
    options: &TreeLayoutOptions,
) -> LayoutOutput {
    let stacks = stacking_parents(forest, options.direction);
    let gap = stack_gap(options);
    let size = |i: usize| {
        let size = input.nodes[i].size;
        Size::new(size.width.max(0.0), size.height.max(0.0))
    };
    let mut sizes: Vec<_> = (0..input.nodes.len()).map(size).collect();
    let mut children = forest.children.clone();
    for (parent, _) in stacks.iter().enumerate().filter(|(_, stacks)| **stacks) {
        let leaves = std::mem::take(&mut children[parent]);
        let own = sizes[parent];
        let column = leaves.iter().map(|&i| size(i).width).fold(0.0, f64::max);
        let height: f64 = leaves.iter().map(|&i| gap + size(i).height).sum();
        // The block's left edge is the parent's, so the parent keeps the
        // block's top-left corner.
        sizes[parent] = Size::new(
            own.width.max(own.width / 2.0 + STACK_INDENT + column),
            own.height + height,
        );
    }
    let mut output = tidy_with_sizes(input, &children, &forest.roots, options, |i| sizes[i]);
    for (parent, _) in stacks.iter().enumerate().filter(|(_, stacks)| **stacks) {
        let Some(origin) = output.positions.get(&input.nodes[parent].id).copied() else {
            continue;
        };
        let own = size(parent);
        let x = origin.x + own.width / 2.0 + STACK_INDENT;
        let mut y = origin.y + own.height;
        for &leaf in &forest.children[parent] {
            y += gap;
            output
                .positions
                .insert(input.nodes[leaf].id.clone(), Point::new(x, y));
            y += size(leaf).height;
        }
    }
    output
}
pub(super) fn tidy_positions(
    input: &LayoutInput,
    children: &[Vec<usize>],
    roots: &[usize],
    options: &TreeLayoutOptions,
) -> LayoutOutput {
    tidy_with_sizes(input, children, roots, options, |i| input.nodes[i].size)
}
fn tidy_with_sizes(
    input: &LayoutInput,
    children: &[Vec<usize>],
    roots: &[usize],
    options: &TreeLayoutOptions,
    size: impl Fn(usize) -> Size,
) -> LayoutOutput {
    let mut arena: Vec<_> = (0..input.nodes.len())
        .map(|i| {
            let size = size(i);
            let (breadth, depth) = match options.direction {
                TreeDirection::TopDown => (size.width, size.height),
                TreeDirection::LeftRight => (size.height, size.width),
            };
            Flex {
                breadth: breadth.max(0.0),
                depth: depth.max(0.0) + options.level_gap.max(0.0),
                left_extreme: i,
                right_extreme: i,
                ..Default::default()
            }
        })
        .collect();
    let mut output = LayoutOutput::default();
    for &root in roots {
        let mut order = Vec::new();
        let mut stack = vec![root];
        while let Some(i) = stack.pop() {
            order.push(i);
            for &child in children[i].iter().rev() {
                arena[child].y = arena[i].y + arena[i].depth;
                stack.push(child);
            }
        }
        for &i in order.iter().rev() {
            let mut lows = Vec::<(f64, usize)>::new();
            for (index, &kid) in children[i].iter().enumerate() {
                let extreme = if index == 0 {
                    arena[kid].left_extreme
                } else {
                    arena[kid].right_extreme
                };
                let low_y = bottom(&arena, extreme);
                if index > 0 {
                    separate(
                        &mut arena,
                        children,
                        i,
                        index,
                        &lows,
                        options.sibling_gap.max(0.0),
                    );
                }
                while lows.last().is_some_and(|&(y, _)| low_y >= y) {
                    lows.pop();
                }
                lows.push((low_y, index));
            }
            let mut shift_sum = 0.0;
            let mut change_sum = 0.0;
            for &child in &children[i] {
                shift_sum += arena[child].shift;
                change_sum += shift_sum + arena[child].change;
                arena[child].rel += change_sum;
            }
            if let (Some(&first), Some(&last)) = (children[i].first(), children[i].last()) {
                arena[i].prelim = (arena[first].prelim + arena[first].rel
                    - arena[first].breadth / 2.0
                    + arena[last].rel
                    + arena[last].prelim
                    + arena[last].breadth / 2.0)
                    / 2.0;
                arena[i].left_extreme = arena[first].left_extreme;
                arena[i].left_extreme_rel = arena[first].left_extreme_rel;
                arena[i].right_extreme = arena[last].right_extreme;
                arena[i].right_extreme_rel = arena[last].right_extreme_rel;
            }
        }
        let mut stack = vec![(root, -arena[root].rel - arena[root].prelim)];
        while let Some((i, previous_sum)) = stack.pop() {
            let sum = previous_sum + arena[i].rel;
            arena[i].x = sum + arena[i].prelim;
            let breadth_corner = arena[i].x - arena[i].breadth / 2.0;
            let point = match options.direction {
                TreeDirection::TopDown => Point::new(breadth_corner, arena[i].y),
                TreeDirection::LeftRight => Point::new(arena[i].y, breadth_corner),
            };
            output.positions.insert(input.nodes[i].id.clone(), point);
            for &child in children[i].iter().rev() {
                stack.push((child, sum));
            }
        }
    }
    output
}
fn bottom(arena: &[Flex], i: usize) -> f64 {
    arena[i].y + arena[i].depth
}
fn next_left(arena: &[Flex], children: &[Vec<usize>], i: usize) -> Option<usize> {
    children[i].first().copied().or(arena[i].left_thread)
}
fn next_right(arena: &[Flex], children: &[Vec<usize>], i: usize) -> Option<usize> {
    children[i].last().copied().or(arena[i].right_thread)
}
fn separate(
    arena: &mut [Flex],
    children: &[Vec<usize>],
    parent: usize,
    index: usize,
    lows: &[(f64, usize)],
    gap: f64,
) {
    let sibling = children[parent][index - 1];
    let current = children[parent][index];
    let mut right = Some(sibling);
    let mut left = Some(current);
    let mut rmod = arena[sibling].rel;
    let mut lmod = arena[current].rel;
    let mut first = true;
    let mut low_index = lows.len().saturating_sub(1);
    while let (Some(r), Some(l)) = (right, left) {
        while low_index > 0 && bottom(arena, r) > lows[low_index].0 {
            low_index -= 1;
        }
        let distance = rmod + arena[r].prelim - lmod - arena[l].prelim
            + (arena[r].breadth + arena[l].breadth) / 2.0
            + gap;
        if distance > 0.0 || (distance < 0.0 && first) {
            lmod += distance;
            arena[current].rel += distance;
            arena[current].left_extreme_rel += distance;
            arena[current].right_extreme_rel += distance;
            let left_sibling = lows.get(low_index).map(|&(_, i)| i).unwrap_or(index - 1);
            let count = index - left_sibling;
            if count > 1 {
                let delta = distance / count as f64;
                arena[children[parent][left_sibling + 1]].shift += delta;
                arena[current].shift -= delta;
                arena[current].change -= distance - delta;
            }
        }
        first = false;
        let rb = bottom(arena, r);
        let lb = bottom(arena, l);
        if rb <= lb {
            right = next_right(arena, children, r);
            if let Some(r) = right {
                rmod += arena[r].rel;
            }
        }
        if rb >= lb {
            left = next_left(arena, children, l);
            if let Some(l) = left {
                lmod += arena[l].rel;
            }
        }
    }
    if let (None, Some(l)) = (right, left) {
        let first_child = children[parent][0];
        let extreme = arena[first_child].left_extreme;
        arena[extreme].left_thread = Some(l);
        let difference = lmod - arena[l].rel - arena[first_child].left_extreme_rel;
        arena[extreme].rel += difference;
        arena[extreme].prelim -= difference;
        arena[first_child].left_extreme = arena[current].left_extreme;
        arena[first_child].left_extreme_rel = arena[current].left_extreme_rel;
    } else if let (Some(r), None) = (right, left) {
        let extreme = arena[current].right_extreme;
        arena[extreme].right_thread = Some(r);
        let difference = rmod - arena[r].rel - arena[current].right_extreme_rel;
        arena[extreme].rel += difference;
        arena[extreme].prelim -= difference;
        arena[current].right_extreme = arena[sibling].right_extreme;
        arena[current].right_extreme_rel = arena[sibling].right_extreme_rel;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Size,
        layout::tests::{assert_clear, fixture},
    };
    #[test]
    fn chain_and_star_golden() {
        let mut input = fixture(3);
        for node in &mut input.nodes {
            node.size = Size::new(40.0, 20.0);
        }
        input.nodes[2].layout_parent = Some(input.nodes[1].id.clone());
        let output = TreeLayout::default().layout(&input);
        assert_eq!(output.positions[&input.nodes[0].id], Point::new(0.0, 0.0));
        assert_eq!(output.positions[&input.nodes[1].id], Point::new(0.0, 100.0));
        assert_eq!(output.positions[&input.nodes[2].id], Point::new(0.0, 200.0));
        input.nodes[2].layout_parent = Some(input.nodes[0].id.clone());
        let output = TreeLayout::default().layout(&input);
        assert_eq!(output.positions[&input.nodes[0].id], Point::new(32.0, 0.0));
        assert_eq!(output.positions[&input.nodes[1].id], Point::new(0.0, 100.0));
        assert_eq!(
            output.positions[&input.nodes[2].id],
            Point::new(64.0, 100.0)
        );
    }
    #[test]
    fn generated_variable_size_trees() {
        for seed in 0..20 {
            let mut input = fixture(100);
            let mut rng = super::super::rng::Rng::new(seed);
            for i in 1..input.nodes.len() {
                input.nodes[i].size =
                    Size::new(20.0 + rng.next() * 180.0, 20.0 + rng.next() * 120.0);
                input.nodes[i].layout_parent =
                    Some(input.nodes[(rng.next() * i as f64) as usize].id.clone());
            }
            for direction in [TreeDirection::TopDown, TreeDirection::LeftRight] {
                assert_clear(
                    &input,
                    &TreeLayout(TreeLayoutOptions {
                        direction,
                        ..Default::default()
                    })
                    .layout(&input),
                );
            }
        }
    }

    /// A root with `branches` main branches of `leaves` leaf ideas each.
    fn org_chart(branches: usize, leaves: usize) -> LayoutInput {
        let mut input = fixture(1 + branches * (1 + leaves));
        input.nodes[0].size = Size::new(180.0, 48.0);
        for b in 0..branches {
            let branch = 1 + b * (1 + leaves);
            input.nodes[branch].size = Size::new(150.0, 44.0);
            input.nodes[branch].layout_parent = Some(input.nodes[0].id.clone());
            for l in 0..leaves {
                let leaf = branch + 1 + l;
                input.nodes[leaf].size = Size::new(120.0 + (l % 3) as f64 * 30.0, 40.0);
                input.nodes[leaf].layout_parent = Some(input.nodes[branch].id.clone());
            }
        }
        input
    }
    fn bounds(input: &LayoutInput, output: &LayoutOutput) -> crate::Rect {
        input
            .nodes
            .iter()
            .map(|node| crate::Rect::new(output.positions[&node.id], node.size))
            .reduce(|a, b| a.union(b))
            .unwrap()
    }
    #[test]
    fn top_down_trees_stack_leaf_children_into_indented_columns() {
        let input = org_chart(7, 6);
        let layout = TreeLayout::default();
        let output = layout.layout(&input);
        assert_clear(&input, &output);
        // Deterministic.
        assert_eq!(output, layout.layout(&input));
        // Seven columns instead of one row of 42 leaves (about 6000px).
        let bounds = bounds(&input, &output);
        assert!(bounds.size.width < 2400.0, "{bounds:?}");
        assert!(bounds.size.height < 700.0, "{bounds:?}");
        let stacked = stacked_leaves(&input, &TreeLayoutOptions::default());
        assert_eq!(stacked.len(), 42);
        for b in 0..7 {
            let branch = &input.nodes[1 + b * 7];
            let origin = output.positions[&branch.id];
            let spine = origin.x + branch.size.width / 2.0;
            let mut previous_bottom = origin.y + branch.size.height;
            for leaf in &input.nodes[2 + b * 7..8 + b * 7] {
                assert!(stacked.contains(&leaf.id));
                let position = output.positions[&leaf.id];
                // Hanging off the spine below the branch's centre, in order.
                assert_eq!(position.x, spine + STACK_INDENT);
                assert!(position.y > previous_bottom);
                previous_bottom = position.y + leaf.size.height;
            }
        }
        // Main branches still fan out in a row under the root.
        let y = output.positions[&input.nodes[1].id].y;
        assert!((0..7).all(|b| output.positions[&input.nodes[1 + b * 7].id].y == y));
    }
    #[test]
    fn roots_and_left_right_trees_keep_fanning_out() {
        // A root's leaves form a row: a stacked list would not read as a tree.
        let input = org_chart(5, 0);
        let output = TreeLayout::default().layout(&input);
        let y = output.positions[&input.nodes[1].id].y;
        assert!((1..6).all(|i| output.positions[&input.nodes[i].id].y == y));
        assert!(stacked_leaves(&input, &TreeLayoutOptions::default()).is_empty());
        // Left-to-right trees already stack siblings vertically.
        let options = TreeLayoutOptions {
            direction: TreeDirection::LeftRight,
            ..Default::default()
        };
        let input = org_chart(3, 4);
        assert!(stacked_leaves(&input, &options).is_empty());
        let output = TreeLayout(options).layout(&input);
        assert_clear(&input, &output);
        let x = output.positions[&input.nodes[2].id].x;
        assert!((2..6).all(|i| output.positions[&input.nodes[i].id].x == x));
    }
}
