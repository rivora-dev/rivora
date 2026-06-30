//! Provenance information for memory records.

use serde::{Deserialize, Serialize};

use rivora_types::NonEmptyString;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryProvenance {
    pub source: NonEmptyString,
    pub source_version: NonEmptyString,
    pub observed_at: NonEmptyString,
    pub learned_at: NonEmptyString,
    pub graph_id: Option<String>,
    pub graph_node_ids: Vec<String>,
    pub graph_edge_ids: Vec<String>,
    pub receipt_id: Option<String>,
    pub connector_ref: Option<String>,
    pub inference_ref: Option<String>,
    pub ability_ref: Option<String>,
    pub human_ref: Option<String>,
    pub raw_ref: Option<String>,
}

impl MemoryProvenance {
    #[must_use]
    pub fn builder() -> crate::builders::MemoryProvenanceBuilder {
        crate::builders::MemoryProvenanceBuilder::new()
    }

    pub fn new(
        source: impl Into<String>,
        source_version: impl Into<String>,
        observed_at: impl Into<String>,
        learned_at: impl Into<String>,
    ) -> Result<Self, rivora_errors::RivoraError> {
        Ok(Self {
            source: NonEmptyString::new(source.into())?,
            source_version: NonEmptyString::new(source_version.into())?,
            observed_at: NonEmptyString::new(observed_at.into())?,
            learned_at: NonEmptyString::new(learned_at.into())?,
            graph_id: None,
            graph_node_ids: Vec::new(),
            graph_edge_ids: Vec::new(),
            receipt_id: None,
            connector_ref: None,
            inference_ref: None,
            ability_ref: None,
            human_ref: None,
            raw_ref: None,
        })
    }

    #[must_use]
    pub fn with_graph_id(mut self, graph_id: impl Into<String>) -> Self {
        self.graph_id = Some(graph_id.into());
        self
    }

    #[must_use]
    pub fn with_graph_node_ids(mut self, graph_node_ids: Vec<String>) -> Self {
        self.graph_node_ids = graph_node_ids;
        self
    }

    #[must_use]
    pub fn with_graph_edge_ids(mut self, graph_edge_ids: Vec<String>) -> Self {
        self.graph_edge_ids = graph_edge_ids;
        self
    }

    #[must_use]
    pub fn with_receipt_id(mut self, receipt_id: impl Into<String>) -> Self {
        self.receipt_id = Some(receipt_id.into());
        self
    }

    #[must_use]
    pub fn with_connector_ref(mut self, connector_ref: impl Into<String>) -> Self {
        self.connector_ref = Some(connector_ref.into());
        self
    }

    #[must_use]
    pub fn with_inference_ref(mut self, inference_ref: impl Into<String>) -> Self {
        self.inference_ref = Some(inference_ref.into());
        self
    }

    #[must_use]
    pub fn with_ability_ref(mut self, ability_ref: impl Into<String>) -> Self {
        self.ability_ref = Some(ability_ref.into());
        self
    }

    #[must_use]
    pub fn with_human_ref(mut self, human_ref: impl Into<String>) -> Self {
        self.human_ref = Some(human_ref.into());
        self
    }

    #[must_use]
    pub fn with_raw_ref(mut self, raw_ref: impl Into<String>) -> Self {
        self.raw_ref = Some(raw_ref.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provenance_rejects_empty_source() {
        assert!(
            MemoryProvenance::new("", "0.1.0", "2026-06-25T12:00:00Z", "2026-06-25T12:00:00Z")
                .is_err()
        );
    }

    #[test]
    fn provenance_rejects_empty_source_version() {
        assert!(MemoryProvenance::new(
            "connector",
            "",
            "2026-06-25T12:00:00Z",
            "2026-06-25T12:00:00Z"
        )
        .is_err());
    }

    #[test]
    fn provenance_rejects_empty_observed_at() {
        assert!(MemoryProvenance::new("connector", "0.1.0", "", "2026-06-25T12:00:00Z").is_err());
    }

    #[test]
    fn provenance_rejects_empty_learned_at() {
        assert!(MemoryProvenance::new("connector", "0.1.0", "2026-06-25T12:00:00Z", "").is_err());
    }

    #[test]
    fn provenance_accepts_valid_fields() {
        let p = MemoryProvenance::new(
            "connector",
            "0.1.0",
            "2026-06-25T12:00:00Z",
            "2026-06-25T12:00:00Z",
        )
        .unwrap();
        assert_eq!(p.source.as_str(), "connector");
        assert_eq!(p.source_version.as_str(), "0.1.0");
        assert_eq!(p.observed_at.as_str(), "2026-06-25T12:00:00Z");
        assert_eq!(p.learned_at.as_str(), "2026-06-25T12:00:00Z");
    }

    #[test]
    fn provenance_with_optional_fields() {
        let p = MemoryProvenance::new(
            "connector",
            "0.1.0",
            "2026-06-25T12:00:00Z",
            "2026-06-25T12:00:00Z",
        )
        .unwrap()
        .with_graph_id("graph-1")
        .with_graph_node_ids(vec!["node-1".to_string()])
        .with_graph_edge_ids(vec!["edge-1".to_string()])
        .with_receipt_id("receipt_1")
        .with_connector_ref("aws-connector-v1")
        .with_inference_ref("anthropic:claude-opus-4")
        .with_ability_ref("deployment-validator:1.0.0")
        .with_human_ref("engineer:sergio")
        .with_raw_ref("arn:aws:ecs:us-east-1:123:service/api");
        assert_eq!(p.graph_id.as_deref(), Some("graph-1"));
        assert_eq!(p.graph_node_ids.len(), 1);
        assert_eq!(p.graph_edge_ids.len(), 1);
        assert_eq!(p.receipt_id.as_deref(), Some("receipt_1"));
        assert!(p.connector_ref.is_some());
        assert!(p.inference_ref.is_some());
        assert!(p.ability_ref.is_some());
        assert!(p.human_ref.is_some());
        assert!(p.raw_ref.is_some());
    }

    #[test]
    fn provenance_round_trips_through_serde() {
        let p = MemoryProvenance::new(
            "connector",
            "0.1.0",
            "2026-06-25T12:00:00Z",
            "2026-06-25T12:00:00Z",
        )
        .unwrap()
        .with_receipt_id("receipt_1");
        let json = serde_json::to_string(&p).unwrap();
        let back: MemoryProvenance = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }
}
