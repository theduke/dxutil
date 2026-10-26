//! Stable BFS forest. Explicit layout parents take precedence over graph edges.
use super::LayoutInput;
use std::collections::{BTreeMap, VecDeque};
#[derive(Debug)]
pub(super) struct Forest {
    pub roots: Vec<usize>,
    pub children: Vec<Vec<usize>>,
    pub components: Vec<Vec<usize>>,
}
pub(super) fn forest(input: &LayoutInput) -> Forest {
    forest_with_roots(input, &input.roots)
}

pub(super) fn forest_with_roots(input: &LayoutInput, roots: &[crate::NodeId]) -> Forest {
    let n = input.nodes.len();
    let by_id: BTreeMap<_, _> = input
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (&n.id, i))
        .collect();
    let mut adjacency = vec![Vec::new(); n];
    let mut has_parent = vec![false; n];
    for (i, node) in input.nodes.iter().enumerate() {
        if let Some(&parent) = node.layout_parent.as_ref().and_then(|id| by_id.get(id))
            && i != parent
        {
            adjacency[parent].push(i);
            has_parent[i] = true;
        }
    }
    for edge in &input.edges {
        if let (Some(&source), Some(&target)) = (by_id.get(&edge.source), by_id.get(&edge.target)) {
            if !has_parent[target] {
                adjacency[source].push(target);
            }
            if !has_parent[source] {
                adjacency[target].push(source);
            }
        }
    }
    let key = |i: &usize| (&input.nodes[*i].order_key, &input.nodes[*i].id);
    for list in &mut adjacency {
        list.sort_by_key(key);
        list.dedup();
    }
    let mut candidates: Vec<_> = roots
        .iter()
        .filter_map(|id| by_id.get(id).copied())
        .collect();
    let mut other: Vec<_> = (0..n).filter(|&i| !has_parent[i]).collect();
    other.sort_by_key(key);
    candidates.extend(other);
    let mut remaining: Vec<_> = (0..n).collect();
    remaining.sort_by_key(key);
    candidates.extend(remaining);
    let mut visited = vec![false; n];
    let mut result = Forest {
        roots: Vec::new(),
        children: vec![Vec::new(); n],
        components: Vec::new(),
    };
    for root in candidates {
        if visited[root] {
            continue;
        }
        result.roots.push(root);
        visited[root] = true;
        let mut component = Vec::new();
        let mut queue = VecDeque::from([root]);
        while let Some(parent) = queue.pop_front() {
            component.push(parent);
            for &child in &adjacency[parent] {
                if !visited[child] {
                    visited[child] = true;
                    result.children[parent].push(child);
                    queue.push_back(child);
                }
            }
        }
        result.components.push(component);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{LayoutEdge, tests::fixture};
    #[test]
    fn cycles_and_disconnected() {
        let mut input = fixture(4);
        input.nodes[2].layout_parent = None;
        input.edges.push(LayoutEdge {
            source: "n003".into(),
            target: "n000".into(),
            weight: 1.0,
        });
        let f = forest(&input);
        assert_eq!(f.components.iter().map(Vec::len).sum::<usize>(), 4);
        assert_eq!(f.components.len(), 2);
    }
}
