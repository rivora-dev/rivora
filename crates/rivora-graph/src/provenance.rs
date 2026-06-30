//! Provenance information for graph entities.

use serde::{Deserialize, Serialize};

use rivora_types::NonEmptyString;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphProvenance {
    pub source: NonEmptyString,
    pub source_kind: NonEmptyString,
    pub source_version: NonEmptyString,
    pub observed_at: NonEmptyString,
    pub receipt_id: Option<String>,
    pub raw_ref: Option<String>,
    pub connector_ref: Option<String>,
    pub inference_ref: Option<String>,
    pub ability_ref: Option<String>,
}

impl GraphProvenance {
    #[must_use]
    pub fn builder() -> crate::builders::ProvenanceBuilder {
        crate::builders::ProvenanceBuilder::new()
    }

    pub fn new(
        source: impl Into<String>,
        source_kind: impl Into<String>,
        source_version: impl Into<String>,
        observed_at: impl Into<String>,
    ) -> Result<Self, rivora_errors::RivoraError> {
        Ok(Self {
            source: NonEmptyString::new(source.into())?,
            source_kind: NonEmptyString::new(source_kind.into())?,
            source_version: NonEmptyString::new(source_version.into())?,
            observed_at: NonEmptyString::new(observed_at.into())?,
            receipt_id: None,
            raw_ref: None,
            connector_ref: None,
            inference_ref: None,
            ability_ref: None,
        })
    }

    #[must_use]
    pub fn with_receipt_id(mut self, receipt_id: impl Into<String>) -> Self {
        self.receipt_id = Some(receipt_id.into());
        self
    }

    #[must_use]
    pub fn with_raw_ref(mut self, raw_ref: impl Into<String>) -> Self {
        self.raw_ref = Some(raw_ref.into());
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provenance_rejects_empty_source() {
        assert!(GraphProvenance::new("", "kind", "1.0.0", "2026-01-01T00:00:00Z").is_err());
    }

    #[test]
    fn provenance_rejects_empty_source_kind() {
        assert!(GraphProvenance::new("src", "", "1.0.0", "2026-01-01T00:00:00Z").is_err());
    }

    #[test]
    fn provenance_rejects_empty_source_version() {
        assert!(GraphProvenance::new("src", "kind", "", "2026-01-01T00:00:00Z").is_err());
    }

    #[test]
    fn provenance_rejects_empty_observed_at() {
        assert!(GraphProvenance::new("src", "kind", "1.0.0", "").is_err());
    }

    #[test]
    fn provenance_accepts_valid_fields() {
        let p = GraphProvenance::new("connector", "aws", "0.1.0", "2026-06-25T12:00:00Z").unwrap();
        assert_eq!(p.source.as_str(), "connector");
        assert_eq!(p.source_kind.as_str(), "aws");
        assert_eq!(p.source_version.as_str(), "0.1.0");
        assert_eq!(p.observed_at.as_str(), "2026-06-25T12:00:00Z");
    }

    #[test]
    fn provenance_with_optional_fields() {
        let p = GraphProvenance::new("connector", "aws", "0.1.0", "2026-06-25T12:00:00Z")
            .unwrap()
            .with_receipt_id("receipt_1")
            .with_raw_ref("arn:aws:ecs:us-east-1:123:service/api")
            .with_connector_ref("aws-connector-v1")
            .with_inference_ref("anthropic:claude-opus-4")
            .with_ability_ref("deployment-validator:1.0.0");
        assert_eq!(p.receipt_id.as_deref(), Some("receipt_1"));
        assert!(p.raw_ref.is_some());
        assert!(p.connector_ref.is_some());
        assert!(p.inference_ref.is_some());
        assert!(p.ability_ref.is_some());
    }

    #[test]
    fn provenance_round_trips_through_serde() {
        let p = GraphProvenance::new("connector", "aws", "0.1.0", "2026-06-25T12:00:00Z")
            .unwrap()
            .with_receipt_id("receipt_1");
        let json = serde_json::to_string(&p).unwrap();
        let back: GraphProvenance = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }
}
