//! Recommendations — evidence-backed proposals (RFC-004).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{empty_metadata, Confidence, InvestigationId, Metadata, ObjectId, Provenance};

/// Recommendation lifecycle status within v0.1.
///
/// Recommendations are never automatically applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationStatus {
    /// Proposed for human review.
    Proposed,
    /// Explicitly accepted by a human (outcome may still be unknown).
    Accepted,
    /// Explicitly rejected by a human.
    Rejected,
    /// Not acted upon.
    Ignored,
}

impl RecommendationStatus {
    /// Display name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Ignored => "ignored",
        }
    }
}

/// Evidence-backed engineering recommendation (proposal, not a fact).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recommendation {
    /// Stable object identifier.
    pub id: ObjectId,
    /// Primary Investigation.
    pub investigation_id: InvestigationId,
    /// Proposed action summary.
    pub summary: String,
    /// Inspectable reasoning.
    pub rationale: String,
    /// Supporting Evaluation identifiers.
    pub evaluation_ids: Vec<ObjectId>,
    /// Supporting Verification Receipt identifiers.
    pub verification_ids: Vec<ObjectId>,
    /// Confidence in the recommendation.
    pub confidence: Confidence,
    /// Current status (always starts as Proposed).
    pub status: RecommendationStatus,
    /// When the Recommendation was generated.
    pub recommended_at: DateTime<Utc>,
    /// Provenance.
    pub provenance: Provenance,
    /// Metadata.
    pub metadata: Metadata,
}

impl Recommendation {
    /// Construct a proposed Recommendation.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        investigation_id: InvestigationId,
        summary: impl Into<String>,
        rationale: impl Into<String>,
        evaluation_ids: Vec<ObjectId>,
        verification_ids: Vec<ObjectId>,
        confidence: Confidence,
        provenance: Provenance,
    ) -> Self {
        Self {
            id: ObjectId::new(),
            investigation_id,
            summary: summary.into(),
            rationale: rationale.into(),
            evaluation_ids,
            verification_ids,
            confidence,
            status: RecommendationStatus::Proposed,
            recommended_at: Utc::now(),
            provenance,
            metadata: empty_metadata(),
        }
    }
}
