//! Learning outcomes (RFC-010).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{empty_metadata, Confidence, InvestigationId, Metadata, ObjectId, Provenance};

/// Disposition of a Recommendation outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeDisposition {
    /// Human accepted the Recommendation.
    Accepted,
    /// Human rejected the Recommendation.
    Rejected,
    /// Recommendation was ignored.
    Ignored,
    /// Accepted Recommendation produced a successful outcome.
    Successful,
    /// Accepted Recommendation produced an unsuccessful outcome.
    Unsuccessful,
}

impl OutcomeDisposition {
    /// Display name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Ignored => "ignored",
            Self::Successful => "successful",
            Self::Unsuccessful => "unsuccessful",
        }
    }
}

/// Learning outcome recorded for future reasoning.
///
/// Learning never rewrites historical Investigations or Memory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LearningOutcome {
    /// Stable object identifier.
    pub id: ObjectId,
    /// Primary Investigation.
    pub investigation_id: InvestigationId,
    /// Related Recommendation when applicable.
    pub recommendation_id: Option<ObjectId>,
    /// Observed disposition.
    pub disposition: OutcomeDisposition,
    /// Notes describing the observed outcome.
    pub notes: String,
    /// Optional measured impact summary.
    pub impact: Option<String>,
    /// Confidence that the outcome is accurately recorded.
    pub confidence: Confidence,
    /// When the outcome was observed.
    pub observed_at: DateTime<Utc>,
    /// Provenance.
    pub provenance: Provenance,
    /// Metadata.
    pub metadata: Metadata,
}

impl LearningOutcome {
    /// Construct a Learning Outcome.
    pub fn new(
        investigation_id: InvestigationId,
        recommendation_id: Option<ObjectId>,
        disposition: OutcomeDisposition,
        notes: impl Into<String>,
        impact: Option<String>,
        provenance: Provenance,
    ) -> Self {
        Self {
            id: ObjectId::new(),
            investigation_id,
            recommendation_id,
            disposition,
            notes: notes.into(),
            impact,
            confidence: Confidence::certain(),
            observed_at: Utc::now(),
            provenance,
            metadata: empty_metadata(),
        }
    }
}
