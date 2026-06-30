//! Evidence referenced by a reliability receipt.

use serde::{Deserialize, Serialize};

use rivora_types::NonEmptyString;

/// The kind of evidence referenced by a receipt.
///
/// Evidence kinds are deliberately generic — no provider-specific types
/// (AWS, GitHub, Kubernetes, etc.) are hard-coded. Future provider crates
/// can extend this by adding new variants, but the core schema is
/// provider-agnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    /// An observation from a read-only infrastructure source.
    Observation,
    /// A historical metric (e.g. CPU, latency, error rate).
    Metric,
    /// A log line or log event.
    Log,
    /// A deployment event.
    Deployment,
    /// An incident record.
    Incident,
    /// A configuration value (read-only).
    Configuration,
    /// A user/engineer-supplied fact or annotation.
    Annotation,
    /// Some other kind of evidence not covered by the above.
    Other,
}

impl EvidenceKind {
    /// Stable lowercase string tag for the kind.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Observation => "observation",
            Self::Metric => "metric",
            Self::Log => "log",
            Self::Deployment => "deployment",
            Self::Incident => "incident",
            Self::Configuration => "configuration",
            Self::Annotation => "annotation",
            Self::Other => "other",
        }
    }
}

impl std::fmt::Display for EvidenceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The source of a piece of evidence.
///
/// Sources are provider-agnostic. The `provider` field is a generic string
/// (e.g. `"aws"`, `"github"`, `"manual"`) — not a hard-coded enum — to allow
/// new providers without modifying the core schema.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EvidenceSource {
    /// The connector or system that produced this evidence
    /// (e.g. `"aws"`, `"github"`, `"manual"`).
    pub provider: NonEmptyString,
    /// The version of the connector or system that produced this evidence.
    pub version: NonEmptyString,
}

/// A single piece of evidence referenced by a receipt.
///
/// Every receipt must include at least one piece of evidence. A receipt with
/// zero evidence is **invalid** per the canonical spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    /// The kind of evidence.
    pub kind: EvidenceKind,
    /// The source that produced this evidence.
    pub source: EvidenceSource,
    /// A short title for this evidence (e.g. `"CPU spike on api-gateway"`).
    pub title: NonEmptyString,
    /// A longer description of what this evidence supports.
    pub description: NonEmptyString,
    /// ISO-8601 timestamp of when this evidence was observed.
    pub observed_at: NonEmptyString,
    /// This evidence's contribution to the receipt's confidence,
    /// in `[0.0, 1.0]`. Sum of contributions need not be 1.0.
    pub confidence_contribution: f64,
    /// An optional reference to the raw underlying data
    /// (e.g. a URL, ARN, or content hash).
    pub raw_ref: Option<String>,
    /// Optional structured metadata about this evidence.
    /// Must not contain secrets.
    pub metadata: Option<serde_json::Value>,
}

impl Evidence {
    /// Creates a new `Evidence` with required fields. Optional fields default
    /// to `None`.
    ///
    /// # Errors
    ///
    /// Returns an error if `confidence_contribution` is not in `[0.0, 1.0]`
    /// or if any required string field is empty.
    pub fn new(
        kind: EvidenceKind,
        source: EvidenceSource,
        title: impl Into<String>,
        description: impl Into<String>,
        observed_at: impl Into<String>,
        confidence_contribution: f64,
    ) -> Result<Self, rivora_errors::RivoraError> {
        if !(0.0..=1.0).contains(&confidence_contribution) {
            return Err(rivora_errors::RivoraError::invalid_value(
                "confidence_contribution",
                format!("must be in [0.0, 1.0], got {confidence_contribution}"),
            ));
        }
        Ok(Self {
            kind,
            source,
            title: NonEmptyString::new(title.into())?,
            description: NonEmptyString::new(description.into())?,
            observed_at: NonEmptyString::new(observed_at.into())?,
            confidence_contribution,
            raw_ref: None,
            metadata: None,
        })
    }

    /// Builder-style setter for `raw_ref`.
    #[must_use]
    pub fn with_raw_ref(mut self, raw_ref: impl Into<String>) -> Self {
        self.raw_ref = Some(raw_ref.into());
        self
    }

    /// Builder-style setter for `metadata`.
    #[must_use]
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> EvidenceSource {
        EvidenceSource {
            provider: NonEmptyString::new("aws").unwrap(),
            version: NonEmptyString::new("0.1.0").unwrap(),
        }
    }

    #[test]
    fn evidence_kind_as_str_is_lowercase() {
        assert_eq!(EvidenceKind::Observation.as_str(), "observation");
        assert_eq!(EvidenceKind::Metric.as_str(), "metric");
    }

    #[test]
    fn evidence_kind_round_trips_through_serde() {
        let json = serde_json::to_string(&EvidenceKind::Deployment).unwrap();
        assert_eq!(json, "\"deployment\"");
        let back: EvidenceKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back, EvidenceKind::Deployment);
    }

    #[test]
    fn evidence_rejects_invalid_confidence() {
        let result = Evidence::new(
            EvidenceKind::Observation,
            source(),
            "title",
            "description",
            "2026-01-01T00:00:00Z",
            1.5,
        );
        assert!(result.is_err());
    }

    #[test]
    fn evidence_rejects_empty_title() {
        let result = Evidence::new(
            EvidenceKind::Observation,
            source(),
            "",
            "description",
            "2026-01-01T00:00:00Z",
            0.5,
        );
        assert!(result.is_err());
    }

    #[test]
    fn evidence_accepts_valid_fields() {
        let e = Evidence::new(
            EvidenceKind::Metric,
            source(),
            "CPU spike",
            "CPU exceeded 90% for 5 minutes",
            "2026-06-25T12:00:00Z",
            0.8,
        )
        .unwrap();
        assert_eq!(e.kind, EvidenceKind::Metric);
        assert_eq!(e.confidence_contribution, 0.8);
    }

    #[test]
    fn evidence_round_trips_through_serde() {
        let e = Evidence::new(
            EvidenceKind::Observation,
            source(),
            "title",
            "description",
            "2026-01-01T00:00:00Z",
            0.5,
        )
        .unwrap()
        .with_raw_ref("arn:aws:ecs:us-east-1:123:service/api")
        .with_metadata(serde_json::json!({"region": "us-east-1"}));
        let json = serde_json::to_string(&e).unwrap();
        let back: Evidence = serde_json::from_str(&json).unwrap();
        assert_eq!(back.kind, e.kind);
        assert_eq!(back.title, e.title);
        assert_eq!(back.confidence_contribution, e.confidence_contribution);
        assert_eq!(back.raw_ref, e.raw_ref);
    }
}
