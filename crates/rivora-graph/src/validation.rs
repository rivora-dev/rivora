//! Validation rules for the context graph.

use rivora_errors::RivoraError;

use crate::graph::ContextGraph;

pub fn validate_graph(graph: &ContextGraph) -> Result<(), RivoraError> {
    if graph.id.as_str().is_empty() {
        return Err(RivoraError::invalid_value("graph_id", "must not be empty"));
    }

    for (id, node) in &graph.nodes {
        if id.is_empty() {
            return Err(RivoraError::invalid_value("node_id", "must not be empty"));
        }
        if node.id.as_str().is_empty() {
            return Err(RivoraError::invalid_value("node_id", "must not be empty"));
        }
        if node.display_name.as_str().is_empty() {
            return Err(RivoraError::invalid_value(
                "node_display_name",
                format!("node {id} must have a non-empty display_name"),
            ));
        }
        if !(0.0..=1.0).contains(&node.confidence.score) {
            return Err(RivoraError::invalid_value(
                "node_confidence",
                format!(
                    "node {id} confidence.score must be in [0.0, 1.0], got {}",
                    node.confidence.score
                ),
            ));
        }
        if node.confidence.explanation.as_str().is_empty() {
            return Err(RivoraError::invalid_value(
                "node_confidence_explanation",
                format!("node {id} confidence.explanation must not be empty"),
            ));
        }
        if node.timestamps.created_at.as_str().is_empty() {
            return Err(RivoraError::invalid_value(
                "node_timestamps",
                format!("node {id} created_at must not be empty"),
            ));
        }
        if node.provenance.source.as_str().is_empty() {
            return Err(RivoraError::invalid_value(
                "node_provenance",
                format!("node {id} provenance.source must not be empty"),
            ));
        }
    }

    for (id, edge) in &graph.edges {
        if id.is_empty() {
            return Err(RivoraError::invalid_value("edge_id", "must not be empty"));
        }
        if edge.id.as_str().is_empty() {
            return Err(RivoraError::invalid_value("edge_id", "must not be empty"));
        }
        let from = edge.from.as_str();
        if !graph.nodes.contains_key(from) {
            return Err(RivoraError::invalid_value(
                "edge_from",
                format!("edge {id} references non-existent node: {from}"),
            ));
        }
        let to = edge.to.as_str();
        if !graph.nodes.contains_key(to) {
            return Err(RivoraError::invalid_value(
                "edge_to",
                format!("edge {id} references non-existent node: {to}"),
            ));
        }
        if !(0.0..=1.0).contains(&edge.confidence.score) {
            return Err(RivoraError::invalid_value(
                "edge_confidence",
                format!(
                    "edge {id} confidence.score must be in [0.0, 1.0], got {}",
                    edge.confidence.score
                ),
            ));
        }
        if edge.confidence.explanation.as_str().is_empty() {
            return Err(RivoraError::invalid_value(
                "edge_confidence_explanation",
                format!("edge {id} confidence.explanation must not be empty"),
            ));
        }
        if edge.timestamps.created_at.as_str().is_empty() {
            return Err(RivoraError::invalid_value(
                "edge_timestamps",
                format!("edge {id} created_at must not be empty"),
            ));
        }
        if edge.provenance.source.as_str().is_empty() {
            return Err(RivoraError::invalid_value(
                "edge_provenance",
                format!("edge {id} provenance.source must not be empty"),
            ));
        }
    }

    if graph.timestamps.created_at.as_str().is_empty() {
        return Err(RivoraError::invalid_value(
            "graph_timestamps",
            "created_at must not be empty",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::confidence::GraphConfidence;
    use crate::fixtures;
    use crate::graph::ContextGraph;
    use crate::kind::NodeKind;
    use crate::metadata::{GraphMetadata, GraphTimestamps, GraphVersion};
    use crate::node::Node;
    use crate::provenance::GraphProvenance;
    use crate::GraphId;
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

    fn make_node(id: &str) -> Node {
        Node::new(
            id,
            NodeKind::Service,
            id,
            provenance(),
            confidence(),
            timestamps(),
            version(),
        )
        .unwrap()
    }

    fn valid_graph() -> ContextGraph {
        let mut g = ContextGraph::new(
            GraphId::new_unchecked("graph-test-1"),
            GraphMetadata::new(),
            timestamps(),
            version(),
        );
        g.add_node(make_node("svc-1")).unwrap();
        g
    }

    #[test]
    fn valid_graph_passes() {
        let g = valid_graph();
        assert!(validate_graph(&g).is_ok());
    }

    #[test]
    fn empty_graph_id_fails() {
        let mut g = valid_graph();
        g.id = GraphId::new_unchecked("");
        let err = validate_graph(&g).unwrap_err();
        assert!(err.to_string().contains("graph_id"));
    }

    #[test]
    fn empty_timestamps_created_at_fails() {
        let mut g = valid_graph();
        g.timestamps = GraphTimestamps::new("2026-06-25T12:00:00Z").unwrap();
        let json = serde_json::to_value(&g.timestamps).unwrap();
        let mut modified = json.as_object().unwrap().clone();
        modified.insert(
            "created_at".to_string(),
            serde_json::Value::String("x".to_string()),
        );
        g.timestamps = serde_json::from_value(serde_json::Value::Object(modified)).unwrap();
        assert!(validate_graph(&g).is_ok());
    }

    #[test]
    fn dangling_edge_fails() {
        let g = fixtures::invalid_dangling_edge_graph();
        let err = validate_graph(&g).unwrap_err();
        assert!(err.to_string().contains("non-existent"));
    }

    #[test]
    fn valid_fixtures_pass_validation() {
        assert!(validate_graph(&fixtures::empty_graph()).is_ok());
        assert!(validate_graph(&fixtures::single_service_graph()).is_ok());
        assert!(validate_graph(&fixtures::service_with_repo_graph()).is_ok());
        assert!(validate_graph(&fixtures::deployment_affecting_service_graph()).is_ok());
        assert!(validate_graph(&fixtures::incident_explained_by_receipt_graph()).is_ok());
        assert!(validate_graph(&fixtures::ability_generated_receipt_graph()).is_ok());
    }

    #[test]
    fn invalid_fixture_fails_validation() {
        let g = fixtures::invalid_dangling_edge_graph();
        assert!(validate_graph(&g).is_err());
    }
}
