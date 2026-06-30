//! Deterministic sample graphs for testing.

use std::collections::BTreeMap;

use rivora_types::{NonEmptyString, Version};

use crate::confidence::GraphConfidence;
use crate::edge::Edge;
use crate::graph::ContextGraph;
use crate::kind::{EdgeKind, NodeKind};
use crate::metadata::{GraphMetadata, GraphTimestamps, GraphVersion};
use crate::node::Node;
use crate::provenance::GraphProvenance;
use crate::GraphId;

fn provenance() -> GraphProvenance {
    GraphProvenance::new("fixture-connector", "test", "0.1.0", "2026-06-25T12:00:00Z").unwrap()
}

fn confidence() -> GraphConfidence {
    GraphConfidence::new(0.8, "Fixture confidence").unwrap()
}

fn timestamps() -> GraphTimestamps {
    GraphTimestamps::new("2026-06-25T12:00:00Z").unwrap()
}

fn version() -> GraphVersion {
    GraphVersion::new(Version::new(1, 0, 0), 1)
}

fn graph_metadata() -> GraphMetadata {
    GraphMetadata::new()
}

pub fn empty_graph() -> ContextGraph {
    ContextGraph::new(
        GraphId::new_unchecked("graph-fixture-empty"),
        graph_metadata(),
        timestamps(),
        version(),
    )
}

pub fn single_service_graph() -> ContextGraph {
    let mut g = ContextGraph::new(
        GraphId::new_unchecked("graph-fixture-single-service"),
        graph_metadata(),
        timestamps(),
        version(),
    );
    let node = Node::new(
        "svc-api-gateway",
        NodeKind::Service,
        "api-gateway",
        provenance(),
        confidence(),
        timestamps(),
        version(),
    )
    .unwrap();
    g.add_node(node).unwrap();
    g
}

pub fn service_with_repo_graph() -> ContextGraph {
    let mut g = ContextGraph::new(
        GraphId::new_unchecked("graph-fixture-service-repo"),
        graph_metadata(),
        timestamps(),
        version(),
    );
    let svc = Node::new(
        "svc-api-gateway",
        NodeKind::Service,
        "api-gateway",
        provenance(),
        confidence(),
        timestamps(),
        version(),
    )
    .unwrap();
    let repo = Node::new(
        "repo-api-gateway",
        NodeKind::Repository,
        "api-gateway-repo",
        provenance(),
        confidence(),
        timestamps(),
        version(),
    )
    .unwrap();
    g.add_node(svc).unwrap();
    g.add_node(repo).unwrap();
    let edge = Edge::new(
        "edge-repo-owns-svc",
        EdgeKind::Owns,
        "repo-api-gateway",
        "svc-api-gateway",
        provenance(),
        confidence(),
        timestamps(),
        version(),
    )
    .unwrap();
    g.add_edge(edge).unwrap();
    g
}

pub fn deployment_affecting_service_graph() -> ContextGraph {
    let mut g = ContextGraph::new(
        GraphId::new_unchecked("graph-fixture-deployment-service"),
        graph_metadata(),
        timestamps(),
        version(),
    );
    let svc = Node::new(
        "svc-payment",
        NodeKind::Service,
        "payment-service",
        provenance(),
        confidence(),
        timestamps(),
        version(),
    )
    .unwrap();
    let dep = Node::new(
        "dep-v2.1.0",
        NodeKind::Deployment,
        "payment-service-v2.1.0",
        provenance(),
        confidence(),
        timestamps(),
        version(),
    )
    .unwrap();
    g.add_node(svc).unwrap();
    g.add_node(dep).unwrap();
    let edge = Edge::new(
        "edge-dep-deployed-to-svc",
        EdgeKind::DeployedTo,
        "dep-v2.1.0",
        "svc-payment",
        provenance(),
        confidence(),
        timestamps(),
        version(),
    )
    .unwrap();
    g.add_edge(edge).unwrap();
    g
}

pub fn incident_explained_by_receipt_graph() -> ContextGraph {
    let mut g = ContextGraph::new(
        GraphId::new_unchecked("graph-fixture-incident-receipt"),
        graph_metadata(),
        timestamps(),
        version(),
    );
    let svc = Node::new(
        "svc-payment",
        NodeKind::Service,
        "payment-service",
        provenance(),
        confidence(),
        timestamps(),
        version(),
    )
    .unwrap();
    let inc = Node::new(
        "inc-latency-001",
        NodeKind::Incident,
        "latency-incident-001",
        provenance(),
        confidence(),
        timestamps(),
        version(),
    )
    .unwrap();
    let receipt = Node::new(
        "receipt-explanation-001",
        NodeKind::Receipt,
        "explanation-receipt-001",
        provenance(),
        confidence(),
        timestamps(),
        version(),
    )
    .unwrap();
    g.add_node(svc).unwrap();
    g.add_node(inc).unwrap();
    g.add_node(receipt).unwrap();
    let edge = Edge::new(
        "edge-receipt-explains-inc",
        EdgeKind::Explains,
        "receipt-explanation-001",
        "inc-latency-001",
        provenance(),
        confidence(),
        timestamps(),
        version(),
    )
    .unwrap();
    g.add_edge(edge).unwrap();
    g
}

pub fn ability_generated_receipt_graph() -> ContextGraph {
    let mut g = ContextGraph::new(
        GraphId::new_unchecked("graph-fixture-ability-receipt"),
        graph_metadata(),
        timestamps(),
        version(),
    );
    let ability = Node::new(
        "ability-deploy-validator",
        NodeKind::Ability,
        "deployment-validator",
        provenance(),
        confidence(),
        timestamps(),
        version(),
    )
    .unwrap();
    let receipt = Node::new(
        "receipt-ability-run-001",
        NodeKind::Receipt,
        "ability-run-receipt-001",
        provenance(),
        confidence(),
        timestamps(),
        version(),
    )
    .unwrap();
    g.add_node(ability).unwrap();
    g.add_node(receipt).unwrap();
    let edge = Edge::new(
        "edge-ability-generated-receipt",
        EdgeKind::Generated,
        "ability-deploy-validator",
        "receipt-ability-run-001",
        provenance(),
        confidence(),
        timestamps(),
        version(),
    )
    .unwrap();
    g.add_edge(edge).unwrap();
    g
}

pub fn invalid_dangling_edge_graph() -> ContextGraph {
    let mut g = ContextGraph::new(
        GraphId::new_unchecked("graph-fixture-invalid-dangling"),
        graph_metadata(),
        timestamps(),
        version(),
    );
    let svc = Node::new(
        "svc-api-gateway",
        NodeKind::Service,
        "api-gateway",
        provenance(),
        confidence(),
        timestamps(),
        version(),
    )
    .unwrap();
    g.add_node(svc).unwrap();
    let dangling_edge = Edge {
        id: NonEmptyString::new("edge-dangling").unwrap(),
        kind: EdgeKind::Owns,
        from: NonEmptyString::new("svc-api-gateway").unwrap(),
        to: NonEmptyString::new("node-does-not-exist").unwrap(),
        display_name: None,
        labels: BTreeMap::new(),
        metadata: Default::default(),
        provenance: provenance(),
        confidence: confidence(),
        timestamps: timestamps(),
        version: version(),
    };
    g.edges.insert("edge-dangling".to_string(), dangling_edge);
    g
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validation::validate_graph;

    #[test]
    fn empty_graph_has_no_nodes_or_edges() {
        let g = empty_graph();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn single_service_graph_has_one_node() {
        let g = single_service_graph();
        assert_eq!(g.node_count(), 1);
        assert_eq!(g.edge_count(), 0);
        assert!(g.get_node("svc-api-gateway").is_some());
    }

    #[test]
    fn service_with_repo_graph_shape() {
        let g = service_with_repo_graph();
        assert_eq!(g.node_count(), 2);
        assert_eq!(g.edge_count(), 1);
        assert!(g.get_node("svc-api-gateway").is_some());
        assert!(g.get_node("repo-api-gateway").is_some());
        assert!(g.get_edge("edge-repo-owns-svc").is_some());
    }

    #[test]
    fn deployment_affecting_service_graph_shape() {
        let g = deployment_affecting_service_graph();
        assert_eq!(g.node_count(), 2);
        assert_eq!(g.edge_count(), 1);
        assert!(g.get_node("svc-payment").is_some());
        assert!(g.get_node("dep-v2.1.0").is_some());
        let edge = g.get_edge("edge-dep-deployed-to-svc").unwrap();
        assert_eq!(edge.kind, EdgeKind::DeployedTo);
    }

    #[test]
    fn incident_explained_by_receipt_graph_shape() {
        let g = incident_explained_by_receipt_graph();
        assert_eq!(g.node_count(), 3);
        assert_eq!(g.edge_count(), 1);
        assert!(g.get_node("svc-payment").is_some());
        assert!(g.get_node("inc-latency-001").is_some());
        assert!(g.get_node("receipt-explanation-001").is_some());
        let edge = g.get_edge("edge-receipt-explains-inc").unwrap();
        assert_eq!(edge.kind, EdgeKind::Explains);
    }

    #[test]
    fn ability_generated_receipt_graph_shape() {
        let g = ability_generated_receipt_graph();
        assert_eq!(g.node_count(), 2);
        assert_eq!(g.edge_count(), 1);
        assert!(g.get_node("ability-deploy-validator").is_some());
        assert!(g.get_node("receipt-ability-run-001").is_some());
        let edge = g.get_edge("edge-ability-generated-receipt").unwrap();
        assert_eq!(edge.kind, EdgeKind::Generated);
    }

    #[test]
    fn invalid_dangling_edge_graph_fails_validation() {
        let g = invalid_dangling_edge_graph();
        assert!(validate_graph(&g).is_err());
    }

    #[test]
    fn valid_fixtures_pass_validation() {
        assert!(validate_graph(&empty_graph()).is_ok());
        assert!(validate_graph(&single_service_graph()).is_ok());
        assert!(validate_graph(&service_with_repo_graph()).is_ok());
        assert!(validate_graph(&deployment_affecting_service_graph()).is_ok());
        assert!(validate_graph(&incident_explained_by_receipt_graph()).is_ok());
        assert!(validate_graph(&ability_generated_receipt_graph()).is_ok());
    }

    #[test]
    fn fixture_ids_are_deterministic() {
        assert_eq!(empty_graph().id.as_str(), "graph-fixture-empty");
        assert_eq!(
            single_service_graph().id.as_str(),
            "graph-fixture-single-service"
        );
        assert_eq!(
            service_with_repo_graph().id.as_str(),
            "graph-fixture-service-repo"
        );
        assert_eq!(
            deployment_affecting_service_graph().id.as_str(),
            "graph-fixture-deployment-service"
        );
        assert_eq!(
            incident_explained_by_receipt_graph().id.as_str(),
            "graph-fixture-incident-receipt"
        );
        assert_eq!(
            ability_generated_receipt_graph().id.as_str(),
            "graph-fixture-ability-receipt"
        );
        assert_eq!(
            invalid_dangling_edge_graph().id.as_str(),
            "graph-fixture-invalid-dangling"
        );
    }

    #[test]
    fn fixtures_are_deterministic() {
        assert_eq!(empty_graph(), empty_graph());
        assert_eq!(single_service_graph(), single_service_graph());
        assert_eq!(service_with_repo_graph(), service_with_repo_graph());
        assert_eq!(
            deployment_affecting_service_graph(),
            deployment_affecting_service_graph()
        );
        assert_eq!(
            incident_explained_by_receipt_graph(),
            incident_explained_by_receipt_graph()
        );
        assert_eq!(
            ability_generated_receipt_graph(),
            ability_generated_receipt_graph()
        );
    }
}
