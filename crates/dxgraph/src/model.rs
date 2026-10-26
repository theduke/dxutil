//! Serializable content-agnostic graph model. Array order preserves insertion order.
use crate::{
    edge::EdgeStyle,
    geometry::{Point, Rect, Size},
};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fmt;
macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);
        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }
        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}
id_type!(NodeId);
id_type!(EdgeId);
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphNode<N> {
    pub id: NodeId,
    pub data: N,
    pub position: Option<Point>,
    pub size_hint: Size,
    pub pinned: bool,
    pub layout_parent: Option<NodeId>,
    pub order_key: Option<String>,
    /// The side of its layout parent the node is placed on by layouts that
    /// grow branches both ways (the mind map's first-level branches).
    /// `None` lets the layout choose; other layouts ignore it.
    #[serde(default)]
    pub branch_side: Option<crate::layout::BranchSide>,
}
impl<N> GraphNode<N> {
    pub fn new(id: impl Into<NodeId>, data: N) -> Self {
        Self {
            id: id.into(),
            data,
            position: None,
            size_hint: Size::new(160.0, 64.0),
            pinned: false,
            layout_parent: None,
            order_key: None,
            branch_side: None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphEdge<E> {
    pub id: EdgeId,
    pub source: NodeId,
    pub target: NodeId,
    pub data: E,
    pub style: EdgeStyle,
    pub label: Option<String>,
}
impl<E> GraphEdge<E> {
    pub fn new(
        id: impl Into<EdgeId>,
        source: impl Into<NodeId>,
        target: impl Into<NodeId>,
        data: E,
    ) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
            target: target.into(),
            data,
            style: EdgeStyle::default(),
            label: None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphModel<N, E> {
    nodes: IndexMap<NodeId, GraphNode<N>>,
    edges: IndexMap<EdgeId, GraphEdge<E>>,
}
impl<N, E> Default for GraphModel<N, E> {
    fn default() -> Self {
        Self {
            nodes: IndexMap::new(),
            edges: IndexMap::new(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GraphError {
    #[error("Unknown graph node: {0}")]
    UnknownNode(NodeId),
    #[error("Graph node already exists: {0}")]
    DuplicateNode(NodeId),
    #[error("Graph edge already exists: {0}")]
    DuplicateEdge(EdgeId),
}
impl<N, E> GraphModel<N, E> {
    pub fn insert_node(&mut self, node: GraphNode<N>) -> Result<(), GraphError> {
        if self.nodes.contains_key(&node.id) {
            return Err(GraphError::DuplicateNode(node.id));
        }
        self.nodes.insert(node.id.clone(), node);
        Ok(())
    }
    /// Replace content and metadata, preserving the existing position and pin.
    pub fn upsert_node(&mut self, mut node: GraphNode<N>) -> Option<GraphNode<N>> {
        if let Some(existing) = self.nodes.get(&node.id) {
            node.position = existing.position;
            node.pinned = existing.pinned;
        }
        self.nodes.insert(node.id.clone(), node)
    }
    pub fn remove_node(&mut self, id: &NodeId) -> Option<GraphNode<N>> {
        let removed = self.nodes.shift_remove(id)?;
        self.edges
            .retain(|_, edge| &edge.source != id && &edge.target != id);
        for node in self.nodes.values_mut() {
            if node.layout_parent.as_ref() == Some(id) {
                node.layout_parent = None;
            }
        }
        Some(removed)
    }
    pub fn insert_edge(&mut self, edge: GraphEdge<E>) -> Result<(), GraphError> {
        for id in [&edge.source, &edge.target] {
            if !self.nodes.contains_key(id) {
                return Err(GraphError::UnknownNode(id.clone()));
            }
        }
        if self.edges.contains_key(&edge.id) {
            return Err(GraphError::DuplicateEdge(edge.id));
        }
        self.edges.insert(edge.id.clone(), edge);
        Ok(())
    }
    pub fn remove_edge(&mut self, id: &EdgeId) -> Option<GraphEdge<E>> {
        self.edges.shift_remove(id)
    }
    pub fn node(&self, id: &NodeId) -> Option<&GraphNode<N>> {
        self.nodes.get(id)
    }
    pub fn node_data_mut(&mut self, id: &NodeId) -> Option<&mut N> {
        self.nodes.get_mut(id).map(|node| &mut node.data)
    }
    pub fn nodes(&self) -> impl ExactSizeIterator<Item = &GraphNode<N>> {
        self.nodes.values()
    }
    pub fn edges(&self) -> impl ExactSizeIterator<Item = &GraphEdge<E>> {
        self.edges.values()
    }

    pub fn edge(&self, id: &EdgeId) -> Option<&GraphEdge<E>> {
        self.edges.get(id)
    }

    pub fn edge_style_mut(&mut self, id: &EdgeId) -> Option<&mut EdgeStyle> {
        self.edges.get_mut(id).map(|edge| &mut edge.style)
    }

    pub fn contains_edge(&self, id: &EdgeId) -> bool {
        self.edges.contains_key(id)
    }
    pub fn incident_edges<'a>(&'a self, id: &'a NodeId) -> impl Iterator<Item = &'a GraphEdge<E>> {
        self.edges
            .values()
            .filter(move |e| &e.source == id || &e.target == id)
    }
    pub fn neighbors<'a>(&'a self, id: &'a NodeId) -> impl Iterator<Item = &'a NodeId> {
        let neighbors: indexmap::IndexSet<_> = self
            .incident_edges(id)
            .map(|edge| {
                if &edge.source == id {
                    &edge.target
                } else {
                    &edge.source
                }
            })
            .collect();
        neighbors.into_iter()
    }
    pub fn set_position(&mut self, id: &NodeId, position: Point) -> Result<(), GraphError> {
        self.nodes
            .get_mut(id)
            .ok_or_else(|| GraphError::UnknownNode(id.clone()))?
            .position = Some(position);
        Ok(())
    }
    pub fn set_pinned(&mut self, id: &NodeId, pinned: bool) -> Result<(), GraphError> {
        self.nodes
            .get_mut(id)
            .ok_or_else(|| GraphError::UnknownNode(id.clone()))?
            .pinned = pinned;
        Ok(())
    }

    pub fn set_layout_parent(
        &mut self,
        id: &NodeId,
        parent: Option<NodeId>,
    ) -> Result<(), GraphError> {
        self.nodes
            .get_mut(id)
            .ok_or_else(|| GraphError::UnknownNode(id.clone()))?
            .layout_parent = parent;
        Ok(())
    }

    /// Return every node to automatic layout without changing graph topology.
    pub fn reset_positions(&mut self) {
        for node in self.nodes.values_mut() {
            node.pinned = false;
            node.position = None;
        }
    }
    pub fn retain_nodes(&mut self, mut predicate: impl FnMut(&GraphNode<N>) -> bool) {
        let removed: Vec<_> = self
            .nodes
            .values()
            .filter(|node| !predicate(node))
            .map(|node| node.id.clone())
            .collect();
        for id in removed {
            self.remove_node(&id);
        }
    }
    pub fn bounds(&self, sizes: &IndexMap<NodeId, Size>) -> Option<Rect> {
        self.nodes
            .values()
            .filter_map(|node| {
                node.position.map(|position| {
                    Rect::new(
                        position,
                        sizes.get(&node.id).copied().unwrap_or(node.size_hint),
                    )
                })
            })
            .reduce(Rect::union)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbors_deduplicate_incident_edges_and_support_direct_lookup() {
        let mut model = GraphModel::default();
        for id in ["a", "b", "c"] {
            model.insert_node(GraphNode::new(id, ())).unwrap();
        }
        for (id, source, target) in [("ab", "a", "b"), ("ba", "b", "a"), ("ac", "a", "c")] {
            model
                .insert_edge(GraphEdge::new(id, source, target, ()))
                .unwrap();
        }
        assert_eq!(
            model.neighbors(&"a".into()).cloned().collect::<Vec<_>>(),
            vec![NodeId::from("b"), NodeId::from("c")]
        );
        assert!(model.contains_edge(&"ab".into()));
        model.edge_style_mut(&"ab".into()).unwrap().dashed = true;
        assert!(model.edge(&"ab".into()).unwrap().style.dashed);
        assert_eq!(model.edge(&"ab".into()).unwrap().source, NodeId::from("a"));
        assert_eq!(model.edge(&"ab".into()).unwrap().target, NodeId::from("b"));
        assert!(model.edge(&"unknown".into()).is_none());
    }
    #[test]
    fn payload_and_position_updates_preserve_node_identity_and_topology() {
        let mut model = GraphModel::default();
        model.insert_node(GraphNode::new("a", "old")).unwrap();
        model.insert_node(GraphNode::new("b", "other")).unwrap();
        model
            .insert_edge(GraphEdge::new("ab", "a", "b", ()))
            .unwrap();
        *model.node_data_mut(&"a".into()).unwrap() = "new";
        model
            .set_layout_parent(&"b".into(), Some("a".into()))
            .unwrap();
        for id in ["a", "b"] {
            model
                .set_position(&id.into(), Point::new(42.0, 24.0))
                .unwrap();
            model.set_pinned(&id.into(), true).unwrap();
        }
        model.reset_positions();
        assert_eq!(model.node(&"a".into()).unwrap().id, NodeId::from("a"));
        assert_eq!(model.node(&"a".into()).unwrap().data, "new");
        assert_eq!(
            model.node(&"b".into()).unwrap().layout_parent,
            Some("a".into())
        );
        assert!(
            model
                .nodes()
                .all(|node| !node.pinned && node.position.is_none())
        );
        assert_eq!(model.edge(&"ab".into()).unwrap().source, NodeId::from("a"));
        assert_eq!(model.edge(&"ab".into()).unwrap().target, NodeId::from("b"));
        assert_eq!(
            model.set_layout_parent(&"missing".into(), None),
            Err(GraphError::UnknownNode("missing".into()))
        );
    }

    #[test]
    fn mutation_invariants() {
        let mut graph = GraphModel::<(), ()>::default();
        assert_eq!(
            graph.insert_edge(GraphEdge::new("e", "a", "b", ())),
            Err(GraphError::UnknownNode("a".into()))
        );
        for id in ["a", "b", "c"] {
            graph.insert_node(GraphNode::new(id, ())).unwrap();
        }
        assert!(graph.insert_node(GraphNode::new("a", ())).is_err());
        graph
            .insert_edge(GraphEdge::new("e", "a", "b", ()))
            .unwrap();
        assert!(
            graph
                .insert_edge(GraphEdge::new("e", "a", "b", ()))
                .is_err()
        );
        assert_eq!(graph.incident_edges(&"a".into()).count(), 1);
        assert_eq!(
            graph.neighbors(&"a".into()).collect::<Vec<_>>(),
            vec![&NodeId::from("b")]
        );
        graph
            .set_position(&"a".into(), Point::new(10.0, 20.0))
            .unwrap();
        graph.set_pinned(&"a".into(), true).unwrap();
        assert!(graph.set_pinned(&"missing".into(), true).is_err());
        graph.upsert_node(GraphNode::new("a", ()));
        assert!(graph.node(&"a".into()).unwrap().pinned);
        assert_eq!(
            graph.node(&"a".into()).unwrap().position,
            Some(Point::new(10.0, 20.0))
        );
        assert_eq!(
            graph.bounds(&IndexMap::new()).unwrap().size,
            Size::new(160.0, 64.0)
        );
        graph
            .set_layout_parent(&"b".into(), Some("a".into()))
            .unwrap();
        graph.remove_node(&"a".into());
        assert_eq!(graph.edges().count(), 0);
        assert_eq!(graph.node(&"b".into()).unwrap().layout_parent, None);
        graph
            .insert_edge(GraphEdge::new("bc", "b", "c", ()))
            .unwrap();
        assert!(graph.remove_edge(&"bc".into()).is_some());
        graph.retain_nodes(|n| n.id == NodeId::from("b"));
        assert_eq!(graph.nodes().count(), 1);
        let encoded = serde_json::to_string(&graph).unwrap();
        assert_eq!(
            serde_json::from_str::<GraphModel<(), ()>>(&encoded).unwrap(),
            graph
        );
    }
}
