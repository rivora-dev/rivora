use rivora_graph::fixtures;
use rivora_graph::{ContextGraph, Edge, EdgeKind, GraphSnapshot, Node, NodeKind};

#[test]
fn graph_round_trips_through_json() {
    let graph = fixtures::single_service_graph();
    let json = serde_json::to_string(&graph).unwrap();
    let back: ContextGraph = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id, graph.id);
    assert_eq!(back.node_count(), graph.node_count());
    assert_eq!(back.edge_count(), graph.edge_count());
    let original_node = graph.get_node("svc-api-gateway").unwrap();
    let roundtripped_node = back.get_node("svc-api-gateway").unwrap();
    assert_eq!(roundtripped_node.id, original_node.id);
    assert_eq!(roundtripped_node.kind, original_node.kind);
    assert_eq!(roundtripped_node.display_name, original_node.display_name);
}

#[test]
fn graph_serializes_to_json_value() {
    let graph = fixtures::service_with_repo_graph();
    let value: serde_json::Value = serde_json::to_value(&graph).unwrap();
    assert!(value.get("id").is_some());
    assert!(value.get("metadata").is_some());
    assert!(value.get("timestamps").is_some());
    assert!(value.get("version").is_some());
    assert!(value.get("nodes").is_some());
    assert!(value.get("edges").is_some());
    let nodes = value.get("nodes").unwrap().as_object().unwrap();
    assert_eq!(nodes.len(), 2);
    let edges = value.get("edges").unwrap().as_object().unwrap();
    assert_eq!(edges.len(), 1);
}

#[test]
fn node_round_trips_through_json() {
    let graph = fixtures::single_service_graph();
    let node = graph.get_node("svc-api-gateway").unwrap();
    let json = serde_json::to_string(node).unwrap();
    let back: Node = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id, node.id);
    assert_eq!(back.kind, node.kind);
    assert_eq!(back.display_name, node.display_name);
    assert_eq!(back.description, node.description);
    assert_eq!(back.labels, node.labels);
    assert_eq!(back.provenance, node.provenance);
    assert_eq!(back.confidence, node.confidence);
    assert_eq!(back.timestamps, node.timestamps);
    assert_eq!(back.version, node.version);
}

#[test]
fn edge_round_trips_through_json() {
    let graph = fixtures::service_with_repo_graph();
    let edge = graph.get_edge("edge-repo-owns-svc").unwrap();
    let json = serde_json::to_string(edge).unwrap();
    let back: Edge = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id, edge.id);
    assert_eq!(back.kind, edge.kind);
    assert_eq!(back.from, edge.from);
    assert_eq!(back.to, edge.to);
    assert_eq!(back.display_name, edge.display_name);
    assert_eq!(back.labels, edge.labels);
    assert_eq!(back.provenance, edge.provenance);
    assert_eq!(back.confidence, edge.confidence);
    assert_eq!(back.timestamps, edge.timestamps);
    assert_eq!(back.version, edge.version);
}

#[test]
fn snapshot_round_trips_through_json() {
    let graph = fixtures::service_with_repo_graph();
    let snapshot = graph.snapshot();
    let json = serde_json::to_string(&snapshot).unwrap();
    let back: GraphSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id, snapshot.id);
    assert_eq!(back.nodes.len(), snapshot.nodes.len());
    assert_eq!(back.edges.len(), snapshot.edges.len());
    assert_eq!(back.timestamps, snapshot.timestamps);
    assert_eq!(back.version, snapshot.version);
    for (original, roundtripped) in snapshot.nodes.iter().zip(back.nodes.iter()) {
        assert_eq!(roundtripped.id, original.id);
        assert_eq!(roundtripped.kind, original.kind);
    }
    for (original, roundtripped) in snapshot.edges.iter().zip(back.edges.iter()) {
        assert_eq!(roundtripped.id, original.id);
        assert_eq!(roundtripped.kind, original.kind);
        assert_eq!(roundtripped.from, original.from);
        assert_eq!(roundtripped.to, original.to);
    }
}

#[test]
fn all_fixture_graphs_round_trip() {
    let fixture_graphs: Vec<(&str, ContextGraph)> = vec![
        ("empty_graph", fixtures::empty_graph()),
        ("single_service_graph", fixtures::single_service_graph()),
        (
            "service_with_repo_graph",
            fixtures::service_with_repo_graph(),
        ),
        (
            "deployment_affecting_service_graph",
            fixtures::deployment_affecting_service_graph(),
        ),
        (
            "incident_explained_by_receipt_graph",
            fixtures::incident_explained_by_receipt_graph(),
        ),
        (
            "ability_generated_receipt_graph",
            fixtures::ability_generated_receipt_graph(),
        ),
        (
            "invalid_dangling_edge_graph",
            fixtures::invalid_dangling_edge_graph(),
        ),
    ];
    for (name, graph) in &fixture_graphs {
        let json = serde_json::to_string(graph).unwrap();
        let back: ContextGraph = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, graph.id, "id mismatch for {name}");
        assert_eq!(
            back.node_count(),
            graph.node_count(),
            "node_count mismatch for {name}"
        );
        assert_eq!(
            back.edge_count(),
            graph.edge_count(),
            "edge_count mismatch for {name}"
        );
    }
}

#[test]
fn graph_json_is_deterministic() {
    let graph = fixtures::incident_explained_by_receipt_graph();
    let json1 = serde_json::to_string(&graph).unwrap();
    let json2 = serde_json::to_string(&graph).unwrap();
    assert_eq!(json1, json2);
}

#[test]
fn node_kind_serializes_as_snake_case() {
    let cases = [
        (NodeKind::Organization, "\"organization\""),
        (NodeKind::Service, "\"service\""),
        (NodeKind::Deployment, "\"deployment\""),
        (NodeKind::Incident, "\"incident\""),
        (NodeKind::Environment, "\"environment\""),
        (NodeKind::Repository, "\"repository\""),
        (NodeKind::Team, "\"team\""),
        (NodeKind::Owner, "\"owner\""),
        (NodeKind::Dependency, "\"dependency\""),
        (NodeKind::Resource, "\"resource\""),
        (NodeKind::Signal, "\"signal\""),
        (NodeKind::Receipt, "\"receipt\""),
        (NodeKind::Ability, "\"ability\""),
        (NodeKind::ExternalSystem, "\"external_system\""),
        (NodeKind::Unknown, "\"unknown\""),
    ];
    for (kind, expected) in cases {
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, expected, "NodeKind::{kind:?} serialized unexpectedly");
    }
}

#[test]
fn edge_kind_serializes_as_snake_case() {
    let cases = [
        (EdgeKind::Owns, "\"owns\""),
        (EdgeKind::DependsOn, "\"depends_on\""),
        (EdgeKind::DeployedTo, "\"deployed_to\""),
        (EdgeKind::Triggered, "\"triggered\""),
        (EdgeKind::Affected, "\"affected\""),
        (EdgeKind::Observed, "\"observed\""),
        (EdgeKind::Explains, "\"explains\""),
        (EdgeKind::Supports, "\"supports\""),
        (EdgeKind::Generated, "\"generated\""),
        (EdgeKind::RunsIn, "\"runs_in\""),
        (EdgeKind::BelongsTo, "\"belongs_to\""),
        (EdgeKind::References, "\"references\""),
        (EdgeKind::RelatedTo, "\"related_to\""),
        (EdgeKind::Supersedes, "\"supersedes\""),
        (EdgeKind::Unknown, "\"unknown\""),
    ];
    for (kind, expected) in cases {
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, expected, "EdgeKind::{kind:?} serialized unexpectedly");
    }
}

#[test]
fn snapshot_ordering_is_deterministic() {
    let graph = fixtures::incident_explained_by_receipt_graph();
    let snapshot = graph.snapshot();
    let node_ids: Vec<&str> = snapshot.nodes.iter().map(|n| n.id.as_str()).collect();
    let mut sorted_node_ids = node_ids.clone();
    sorted_node_ids.sort();
    assert_eq!(node_ids, sorted_node_ids);
    let edge_ids: Vec<&str> = snapshot.edges.iter().map(|e| e.id.as_str()).collect();
    let mut sorted_edge_ids = edge_ids.clone();
    sorted_edge_ids.sort();
    assert_eq!(edge_ids, sorted_edge_ids);
}

#[test]
fn invalid_graph_still_serializes() {
    let graph = fixtures::invalid_dangling_edge_graph();
    let json = serde_json::to_string(&graph).unwrap();
    let back: ContextGraph = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id, graph.id);
    assert_eq!(back.node_count(), graph.node_count());
    assert_eq!(back.edge_count(), graph.edge_count());
    assert!(back.get_edge("edge-dangling").is_some());
}
