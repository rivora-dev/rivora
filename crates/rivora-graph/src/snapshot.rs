//! Deterministic graph snapshots.

use serde::{Deserialize, Serialize};

use crate::edge::Edge;
use crate::graph::ContextGraph;
use crate::metadata::{GraphMetadata, GraphTimestamps, GraphVersion};
use crate::node::Node;
use crate::GraphId;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphSnapshot {
    pub id: GraphId,
    pub metadata: GraphMetadata,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub timestamps: GraphTimestamps,
    pub version: GraphVersion,
}

impl GraphSnapshot {
    #[must_use]
    pub fn from_graph(graph: &ContextGraph) -> Self {
        let mut nodes: Vec<Node> = graph.nodes.values().cloned().collect();
        nodes.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));

        let mut edges: Vec<Edge> = graph.edges.values().cloned().collect();
        edges.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));

        Self {
            id: graph.id.clone(),
            metadata: graph.metadata.clone(),
            nodes,
            edges,
            timestamps: graph.timestamps.clone(),
            version: graph.version.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    #[test]
    fn snapshot_from_empty_graph() {
        let g = fixtures::empty_graph();
        let snap = GraphSnapshot::from_graph(&g);
        assert_eq!(snap.id, g.id);
        assert!(snap.nodes.is_empty());
        assert!(snap.edges.is_empty());
    }

    #[test]
    fn snapshot_nodes_sorted_by_id() {
        let g = fixtures::incident_explained_by_receipt_graph();
        let snap = GraphSnapshot::from_graph(&g);
        let ids: Vec<&str> = snap.nodes.iter().map(|n| n.id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn snapshot_edges_sorted_by_id() {
        let g = fixtures::service_with_repo_graph();
        let snap = GraphSnapshot::from_graph(&g);
        let ids: Vec<&str> = snap.edges.iter().map(|e| e.id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn snapshot_round_trips_through_serde() {
        let g = fixtures::service_with_repo_graph();
        let snap = GraphSnapshot::from_graph(&g);
        let json = serde_json::to_string(&snap).unwrap();
        let back: GraphSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, snap.id);
        assert_eq!(back.nodes.len(), snap.nodes.len());
        assert_eq!(back.edges.len(), snap.edges.len());
    }

    #[test]
    fn snapshot_is_deterministic() {
        let g = fixtures::deployment_affecting_service_graph();
        let snap1 = GraphSnapshot::from_graph(&g);
        let snap2 = GraphSnapshot::from_graph(&g);
        assert_eq!(snap1, snap2);
    }

    #[test]
    fn snapshot_preserves_counts() {
        let g = fixtures::ability_generated_receipt_graph();
        let snap = GraphSnapshot::from_graph(&g);
        assert_eq!(snap.nodes.len(), g.node_count());
        assert_eq!(snap.edges.len(), g.edge_count());
    }
}
