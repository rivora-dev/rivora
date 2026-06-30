//! An edge in the context graph.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use rivora_types::NonEmptyString;

use crate::builders::EdgeBuilder;
use crate::confidence::GraphConfidence;
use crate::kind::EdgeKind;
use crate::metadata::{EdgeMetadata, GraphTimestamps, GraphVersion};
use crate::provenance::GraphProvenance;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub id: NonEmptyString,
    pub kind: EdgeKind,
    pub from: NonEmptyString,
    pub to: NonEmptyString,
    pub display_name: Option<NonEmptyString>,
    pub labels: BTreeMap<NonEmptyString, NonEmptyString>,
    pub metadata: EdgeMetadata,
    pub provenance: GraphProvenance,
    pub confidence: GraphConfidence,
    pub timestamps: GraphTimestamps,
    pub version: GraphVersion,
}

impl Edge {
    pub fn new(
        id: impl Into<String>,
        kind: EdgeKind,
        from: impl Into<String>,
        to: impl Into<String>,
        provenance: GraphProvenance,
        confidence: GraphConfidence,
        timestamps: GraphTimestamps,
        version: GraphVersion,
    ) -> Result<Self, rivora_errors::RivoraError> {
        Ok(Self {
            id: NonEmptyString::new(id.into())?,
            kind,
            from: NonEmptyString::new(from.into())?,
            to: NonEmptyString::new(to.into())?,
            display_name: None,
            labels: BTreeMap::new(),
            metadata: EdgeMetadata::default(),
            provenance,
            confidence,
            timestamps,
            version,
        })
    }

    #[must_use]
    pub fn builder() -> EdgeBuilder {
        EdgeBuilder::new()
    }

    #[must_use]
    pub fn with_display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = Some(NonEmptyString::new(display_name.into()).unwrap());
        self
    }

    #[must_use]
    pub fn with_labels(mut self, labels: BTreeMap<NonEmptyString, NonEmptyString>) -> Self {
        self.labels = labels;
        self
    }

    #[must_use]
    pub fn with_metadata(mut self, metadata: EdgeMetadata) -> Self {
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
    fn edge_rejects_empty_id() {
        let result = Edge::new(
            "",
            EdgeKind::Owns,
            "a",
            "b",
            provenance(),
            confidence(),
            timestamps(),
            version(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn edge_rejects_empty_from() {
        let result = Edge::new(
            "edge-1",
            EdgeKind::Owns,
            "",
            "b",
            provenance(),
            confidence(),
            timestamps(),
            version(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn edge_rejects_empty_to() {
        let result = Edge::new(
            "edge-1",
            EdgeKind::Owns,
            "a",
            "",
            provenance(),
            confidence(),
            timestamps(),
            version(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn edge_accepts_valid_fields() {
        let edge = Edge::new(
            "edge-1",
            EdgeKind::Owns,
            "org-1",
            "svc-1",
            provenance(),
            confidence(),
            timestamps(),
            version(),
        )
        .unwrap();
        assert_eq!(edge.id.as_str(), "edge-1");
        assert_eq!(edge.kind, EdgeKind::Owns);
        assert_eq!(edge.from.as_str(), "org-1");
        assert_eq!(edge.to.as_str(), "svc-1");
        assert!(edge.display_name.is_none());
        assert!(edge.labels.is_empty());
    }

    #[test]
    fn edge_with_display_name() {
        let edge = Edge::new(
            "edge-1",
            EdgeKind::Owns,
            "org-1",
            "svc-1",
            provenance(),
            confidence(),
            timestamps(),
            version(),
        )
        .unwrap()
        .with_display_name("owns relationship");
        assert_eq!(
            edge.display_name.as_ref().unwrap().as_str(),
            "owns relationship"
        );
    }

    #[test]
    fn edge_with_labels() {
        let mut labels = BTreeMap::new();
        labels.insert(
            NonEmptyString::new("weight").unwrap(),
            NonEmptyString::new("high").unwrap(),
        );
        let edge = Edge::new(
            "edge-1",
            EdgeKind::Owns,
            "org-1",
            "svc-1",
            provenance(),
            confidence(),
            timestamps(),
            version(),
        )
        .unwrap()
        .with_labels(labels.clone());
        assert_eq!(edge.labels, labels);
    }

    #[test]
    fn edge_round_trips_through_serde() {
        let edge = Edge::new(
            "edge-1",
            EdgeKind::Owns,
            "org-1",
            "svc-1",
            provenance(),
            confidence(),
            timestamps(),
            version(),
        )
        .unwrap()
        .with_display_name("owns relationship");
        let json = serde_json::to_string(&edge).unwrap();
        let back: Edge = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, edge.id);
        assert_eq!(back.kind, edge.kind);
        assert_eq!(back.from, edge.from);
        assert_eq!(back.to, edge.to);
    }
}
