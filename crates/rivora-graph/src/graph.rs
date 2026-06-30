//! The main [`ContextGraph`] type and its operations.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::edge::Edge;
use crate::kind::{EdgeKind, NodeKind};
use crate::metadata::{GraphMetadata, GraphTimestamps, GraphVersion};
use crate::node::Node;
use crate::snapshot::GraphSnapshot;
use crate::GraphId;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextGraph {
    pub id: GraphId,
    pub metadata: GraphMetadata,
    pub timestamps: GraphTimestamps,
    pub version: GraphVersion,
    pub(crate) nodes: BTreeMap<String, Node>,
    pub(crate) edges: BTreeMap<String, Edge>,
}

impl ContextGraph {
    pub fn new(
        id: GraphId,
        metadata: GraphMetadata,
        timestamps: GraphTimestamps,
        version: GraphVersion,
    ) -> Self {
        Self {
            id,
            metadata,
            timestamps,
            version,
            nodes: BTreeMap::new(),
            edges: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn builder() -> crate::builders::GraphBuilder {
        crate::builders::GraphBuilder::new()
    }

    pub fn add_node(&mut self, node: Node) -> Result<(), rivora_errors::RivoraError> {
        let id = node.id.as_str().to_string();
        if self.nodes.contains_key(&id) {
            return Err(rivora_errors::RivoraError::invalid_value(
                "node_id",
                format!("duplicate node id: {id}"),
            ));
        }
        self.nodes.insert(id, node);
        Ok(())
    }

    pub fn add_edge(&mut self, edge: Edge) -> Result<(), rivora_errors::RivoraError> {
        let id = edge.id.as_str().to_string();
        if self.edges.contains_key(&id) {
            return Err(rivora_errors::RivoraError::invalid_value(
                "edge_id",
                format!("duplicate edge id: {id}"),
            ));
        }
        let from = edge.from.as_str();
        if !self.nodes.contains_key(from) {
            return Err(rivora_errors::RivoraError::invalid_value(
                "from",
                format!("edge references non-existent node: {from}"),
            ));
        }
        let to = edge.to.as_str();
        if !self.nodes.contains_key(to) {
            return Err(rivora_errors::RivoraError::invalid_value(
                "to",
                format!("edge references non-existent node: {to}"),
            ));
        }
        self.edges.insert(id, edge);
        Ok(())
    }

    #[must_use]
    pub fn get_node(&self, id: &str) -> Option<&Node> {
        self.nodes.get(id)
    }

    #[must_use]
    pub fn get_edge(&self, id: &str) -> Option<&Edge> {
        self.edges.get(id)
    }

    pub fn remove_node(&mut self, id: &str) -> Option<Node> {
        let removed = self.nodes.remove(id);
        if removed.is_some() {
            self.edges
                .retain(|_, e| e.from.as_str() != id && e.to.as_str() != id);
        }
        removed
    }

    pub fn remove_edge(&mut self, id: &str) -> Option<Edge> {
        self.edges.remove(id)
    }

    #[must_use]
    pub fn neighbors(&self, node_id: &str) -> Vec<&Node> {
        let mut neighbor_ids = std::collections::BTreeSet::new();
        for edge in self.edges.values() {
            if edge.from.as_str() == node_id {
                neighbor_ids.insert(edge.to.as_str().to_string());
            }
            if edge.to.as_str() == node_id {
                neighbor_ids.insert(edge.from.as_str().to_string());
            }
        }
        neighbor_ids
            .iter()
            .filter_map(|id| self.nodes.get(id.as_str()))
            .collect()
    }

    #[must_use]
    pub fn incoming_edges(&self, node_id: &str) -> Vec<&Edge> {
        self.edges
            .values()
            .filter(|e| e.to.as_str() == node_id)
            .collect()
    }

    #[must_use]
    pub fn outgoing_edges(&self, node_id: &str) -> Vec<&Edge> {
        self.edges
            .values()
            .filter(|e| e.from.as_str() == node_id)
            .collect()
    }

    #[must_use]
    pub fn nodes_by_kind(&self, kind: NodeKind) -> Vec<&Node> {
        self.nodes.values().filter(|n| n.kind == kind).collect()
    }

    #[must_use]
    pub fn edges_by_kind(&self, kind: EdgeKind) -> Vec<&Edge> {
        self.edges.values().filter(|e| e.kind == kind).collect()
    }

    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn validate(&self) -> Result<(), rivora_errors::RivoraError> {
        crate::validation::validate_graph(self)
    }

    #[must_use]
    pub fn snapshot(&self) -> GraphSnapshot {
        GraphSnapshot::from_graph(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::confidence::GraphConfidence;
    use crate::fixtures;
    use crate::kind::{EdgeKind, NodeKind};
    use crate::metadata::{GraphTimestamps, GraphVersion};
    use crate::provenance::GraphProvenance;
    use rivora_types::Version;

    fn provenance() -> GraphProvenance {
        GraphProvenance::new("connector", "aws", "0.1.0", "2026-06-25T12:00:00Z").unwrap()
    }

    fn confidence() -> GraphConfidence {
        GraphConfidence::new(0.8, "Strong evidence").unwrap()
    }

    fn timestamps() -> GraphTimestamps {
        GraphTimestamps::new("2026-06-25T12:00:00Z").unwrap()
    }

    fn version() -> GraphVersion {
        GraphVersion::new(Version::new(1, 0, 0), 1)
    }

    fn make_node(id: &str, kind: NodeKind) -> Node {
        Node::new(
            id,
            kind,
            id,
            provenance(),
            confidence(),
            timestamps(),
            version(),
        )
        .unwrap()
    }

    fn make_edge(id: &str, kind: EdgeKind, from: &str, to: &str) -> Edge {
        Edge::new(
            id,
            kind,
            from,
            to,
            provenance(),
            confidence(),
            timestamps(),
            version(),
        )
        .unwrap()
    }

    fn empty_graph() -> ContextGraph {
        ContextGraph::new(
            GraphId::new_unchecked("graph-test-1"),
            GraphMetadata::new(),
            timestamps(),
            version(),
        )
    }

    #[test]
    fn add_node_succeeds() {
        let mut g = empty_graph();
        assert!(g.add_node(make_node("svc-1", NodeKind::Service)).is_ok());
        assert_eq!(g.node_count(), 1);
    }

    #[test]
    fn add_node_rejects_duplicate() {
        let mut g = empty_graph();
        g.add_node(make_node("svc-1", NodeKind::Service)).unwrap();
        let err = g
            .add_node(make_node("svc-1", NodeKind::Service))
            .unwrap_err();
        assert!(err.to_string().contains("duplicate"));
    }

    #[test]
    fn add_edge_succeeds() {
        let mut g = empty_graph();
        g.add_node(make_node("org-1", NodeKind::Organization))
            .unwrap();
        g.add_node(make_node("svc-1", NodeKind::Service)).unwrap();
        assert!(g
            .add_edge(make_edge("e-1", EdgeKind::Owns, "org-1", "svc-1"))
            .is_ok());
        assert_eq!(g.edge_count(), 1);
    }

    #[test]
    fn add_edge_rejects_duplicate() {
        let mut g = empty_graph();
        g.add_node(make_node("org-1", NodeKind::Organization))
            .unwrap();
        g.add_node(make_node("svc-1", NodeKind::Service)).unwrap();
        g.add_edge(make_edge("e-1", EdgeKind::Owns, "org-1", "svc-1"))
            .unwrap();
        let err = g
            .add_edge(make_edge("e-1", EdgeKind::Owns, "org-1", "svc-1"))
            .unwrap_err();
        assert!(err.to_string().contains("duplicate"));
    }

    #[test]
    fn add_edge_rejects_dangling_from() {
        let mut g = empty_graph();
        g.add_node(make_node("svc-1", NodeKind::Service)).unwrap();
        let err = g
            .add_edge(make_edge("e-1", EdgeKind::Owns, "missing", "svc-1"))
            .unwrap_err();
        assert!(err.to_string().contains("non-existent"));
    }

    #[test]
    fn add_edge_rejects_dangling_to() {
        let mut g = empty_graph();
        g.add_node(make_node("org-1", NodeKind::Organization))
            .unwrap();
        let err = g
            .add_edge(make_edge("e-1", EdgeKind::Owns, "org-1", "missing"))
            .unwrap_err();
        assert!(err.to_string().contains("non-existent"));
    }

    #[test]
    fn get_node_returns_some_for_existing() {
        let mut g = empty_graph();
        g.add_node(make_node("svc-1", NodeKind::Service)).unwrap();
        assert!(g.get_node("svc-1").is_some());
    }

    #[test]
    fn get_node_returns_none_for_missing() {
        let g = empty_graph();
        assert!(g.get_node("missing").is_none());
    }

    #[test]
    fn get_edge_returns_some_for_existing() {
        let mut g = empty_graph();
        g.add_node(make_node("a", NodeKind::Service)).unwrap();
        g.add_node(make_node("b", NodeKind::Service)).unwrap();
        g.add_edge(make_edge("e-1", EdgeKind::Owns, "a", "b"))
            .unwrap();
        assert!(g.get_edge("e-1").is_some());
    }

    #[test]
    fn get_edge_returns_none_for_missing() {
        let g = empty_graph();
        assert!(g.get_edge("missing").is_none());
    }

    #[test]
    fn remove_node_also_removes_edges() {
        let mut g = empty_graph();
        g.add_node(make_node("org-1", NodeKind::Organization))
            .unwrap();
        g.add_node(make_node("svc-1", NodeKind::Service)).unwrap();
        g.add_edge(make_edge("e-1", EdgeKind::Owns, "org-1", "svc-1"))
            .unwrap();
        let removed = g.remove_node("svc-1");
        assert!(removed.is_some());
        assert_eq!(g.node_count(), 1);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn remove_node_returns_none_for_missing() {
        let mut g = empty_graph();
        assert!(g.remove_node("missing").is_none());
    }

    #[test]
    fn remove_edge_succeeds() {
        let mut g = empty_graph();
        g.add_node(make_node("a", NodeKind::Service)).unwrap();
        g.add_node(make_node("b", NodeKind::Service)).unwrap();
        g.add_edge(make_edge("e-1", EdgeKind::Owns, "a", "b"))
            .unwrap();
        let removed = g.remove_edge("e-1");
        assert!(removed.is_some());
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn remove_edge_returns_none_for_missing() {
        let mut g = empty_graph();
        assert!(g.remove_edge("missing").is_none());
    }

    #[test]
    fn neighbors_returns_connected_nodes() {
        let mut g = empty_graph();
        g.add_node(make_node("a", NodeKind::Service)).unwrap();
        g.add_node(make_node("b", NodeKind::Service)).unwrap();
        g.add_node(make_node("c", NodeKind::Service)).unwrap();
        g.add_node(make_node("d", NodeKind::Service)).unwrap();
        g.add_edge(make_edge("e-1", EdgeKind::DependsOn, "a", "b"))
            .unwrap();
        g.add_edge(make_edge("e-2", EdgeKind::DependsOn, "c", "a"))
            .unwrap();
        let neighbors = g.neighbors("a");
        let ids: Vec<&str> = neighbors.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains(&"b"));
        assert!(ids.contains(&"c"));
        assert!(!ids.contains(&"d"));
    }

    #[test]
    fn neighbors_returns_empty_for_isolated_node() {
        let mut g = empty_graph();
        g.add_node(make_node("a", NodeKind::Service)).unwrap();
        assert!(g.neighbors("a").is_empty());
    }

    #[test]
    fn incoming_edges_returns_edges_where_to_matches() {
        let mut g = empty_graph();
        g.add_node(make_node("a", NodeKind::Service)).unwrap();
        g.add_node(make_node("b", NodeKind::Service)).unwrap();
        g.add_node(make_node("c", NodeKind::Service)).unwrap();
        g.add_edge(make_edge("e-1", EdgeKind::DependsOn, "a", "b"))
            .unwrap();
        g.add_edge(make_edge("e-2", EdgeKind::DependsOn, "c", "b"))
            .unwrap();
        g.add_edge(make_edge("e-3", EdgeKind::DependsOn, "b", "a"))
            .unwrap();
        let incoming = g.incoming_edges("b");
        assert_eq!(incoming.len(), 2);
    }

    #[test]
    fn outgoing_edges_returns_edges_where_from_matches() {
        let mut g = empty_graph();
        g.add_node(make_node("a", NodeKind::Service)).unwrap();
        g.add_node(make_node("b", NodeKind::Service)).unwrap();
        g.add_node(make_node("c", NodeKind::Service)).unwrap();
        g.add_edge(make_edge("e-1", EdgeKind::DependsOn, "a", "b"))
            .unwrap();
        g.add_edge(make_edge("e-2", EdgeKind::DependsOn, "a", "c"))
            .unwrap();
        g.add_edge(make_edge("e-3", EdgeKind::DependsOn, "b", "a"))
            .unwrap();
        let outgoing = g.outgoing_edges("a");
        assert_eq!(outgoing.len(), 2);
    }

    #[test]
    fn nodes_by_kind_filters_correctly() {
        let mut g = empty_graph();
        g.add_node(make_node("svc-1", NodeKind::Service)).unwrap();
        g.add_node(make_node("svc-2", NodeKind::Service)).unwrap();
        g.add_node(make_node("dep-1", NodeKind::Deployment))
            .unwrap();
        assert_eq!(g.nodes_by_kind(NodeKind::Service).len(), 2);
        assert_eq!(g.nodes_by_kind(NodeKind::Deployment).len(), 1);
        assert_eq!(g.nodes_by_kind(NodeKind::Incident).len(), 0);
    }

    #[test]
    fn edges_by_kind_filters_correctly() {
        let mut g = empty_graph();
        g.add_node(make_node("a", NodeKind::Service)).unwrap();
        g.add_node(make_node("b", NodeKind::Service)).unwrap();
        g.add_edge(make_edge("e-1", EdgeKind::Owns, "a", "b"))
            .unwrap();
        g.add_edge(make_edge("e-2", EdgeKind::DependsOn, "a", "b"))
            .unwrap();
        assert_eq!(g.edges_by_kind(EdgeKind::Owns).len(), 1);
        assert_eq!(g.edges_by_kind(EdgeKind::DependsOn).len(), 1);
        assert_eq!(g.edges_by_kind(EdgeKind::DeployedTo).len(), 0);
    }

    #[test]
    fn node_count_and_edge_count() {
        let mut g = empty_graph();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
        g.add_node(make_node("a", NodeKind::Service)).unwrap();
        g.add_node(make_node("b", NodeKind::Service)).unwrap();
        assert_eq!(g.node_count(), 2);
        g.add_edge(make_edge("e-1", EdgeKind::Owns, "a", "b"))
            .unwrap();
        assert_eq!(g.edge_count(), 1);
    }

    #[test]
    fn validate_delegates_to_validation_module() {
        let g = fixtures::empty_graph();
        assert!(g.validate().is_ok());
    }

    #[test]
    fn snapshot_creates_deterministic_snapshot() {
        let g = fixtures::service_with_repo_graph();
        let snap = g.snapshot();
        assert_eq!(snap.id, g.id);
        assert_eq!(snap.nodes.len(), g.node_count());
        assert_eq!(snap.edges.len(), g.edge_count());
    }
}
