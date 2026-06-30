//! A node in the context graph.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use rivora_types::NonEmptyString;

use crate::builders::NodeBuilder;
use crate::confidence::GraphConfidence;
use crate::kind::NodeKind;
use crate::metadata::{GraphTimestamps, GraphVersion, NodeMetadata};
use crate::provenance::GraphProvenance;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NonEmptyString,
    pub kind: NodeKind,
    pub display_name: NonEmptyString,
    pub description: Option<NonEmptyString>,
    pub labels: BTreeMap<NonEmptyString, NonEmptyString>,
    pub metadata: NodeMetadata,
    pub provenance: GraphProvenance,
    pub confidence: GraphConfidence,
    pub timestamps: GraphTimestamps,
    pub version: GraphVersion,
}

impl Node {
    pub fn new(
        id: impl Into<String>,
        kind: NodeKind,
        display_name: impl Into<String>,
        provenance: GraphProvenance,
        confidence: GraphConfidence,
        timestamps: GraphTimestamps,
        version: GraphVersion,
    ) -> Result<Self, rivora_errors::RivoraError> {
        Ok(Self {
            id: NonEmptyString::new(id.into())?,
            kind,
            display_name: NonEmptyString::new(display_name.into())?,
            description: None,
            labels: BTreeMap::new(),
            metadata: NodeMetadata::default(),
            provenance,
            confidence,
            timestamps,
            version,
        })
    }

    #[must_use]
    pub fn builder() -> NodeBuilder {
        NodeBuilder::new()
    }

    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(NonEmptyString::new(description.into()).unwrap());
        self
    }

    #[must_use]
    pub fn with_labels(mut self, labels: BTreeMap<NonEmptyString, NonEmptyString>) -> Self {
        self.labels = labels;
        self
    }

    #[must_use]
    pub fn with_metadata(mut self, metadata: NodeMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::confidence::GraphConfidence;
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

    #[test]
    fn node_rejects_empty_id() {
        let result = Node::new(
            "",
            NodeKind::Service,
            "display",
            provenance(),
            confidence(),
            timestamps(),
            version(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn node_rejects_empty_display_name() {
        let result = Node::new(
            "node-1",
            NodeKind::Service,
            "",
            provenance(),
            confidence(),
            timestamps(),
            version(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn node_accepts_valid_fields() {
        let node = Node::new(
            "node-1",
            NodeKind::Service,
            "api-gateway",
            provenance(),
            confidence(),
            timestamps(),
            version(),
        )
        .unwrap();
        assert_eq!(node.id.as_str(), "node-1");
        assert_eq!(node.kind, NodeKind::Service);
        assert_eq!(node.display_name.as_str(), "api-gateway");
        assert!(node.description.is_none());
        assert!(node.labels.is_empty());
    }

    #[test]
    fn node_with_description() {
        let node = Node::new(
            "node-1",
            NodeKind::Service,
            "api-gateway",
            provenance(),
            confidence(),
            timestamps(),
            version(),
        )
        .unwrap()
        .with_description("The main API gateway");
        assert_eq!(
            node.description.as_ref().unwrap().as_str(),
            "The main API gateway"
        );
    }

    #[test]
    fn node_with_labels() {
        let mut labels = BTreeMap::new();
        labels.insert(
            NonEmptyString::new("env").unwrap(),
            NonEmptyString::new("prod").unwrap(),
        );
        let node = Node::new(
            "node-1",
            NodeKind::Service,
            "api-gateway",
            provenance(),
            confidence(),
            timestamps(),
            version(),
        )
        .unwrap()
        .with_labels(labels.clone());
        assert_eq!(node.labels, labels);
    }

    #[test]
    fn node_round_trips_through_serde() {
        let node = Node::new(
            "node-1",
            NodeKind::Service,
            "api-gateway",
            provenance(),
            confidence(),
            timestamps(),
            version(),
        )
        .unwrap()
        .with_description("The main API gateway");
        let json = serde_json::to_string(&node).unwrap();
        let back: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, node.id);
        assert_eq!(back.kind, node.kind);
        assert_eq!(back.display_name, node.display_name);
    }
}
