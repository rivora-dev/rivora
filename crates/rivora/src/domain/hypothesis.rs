//! Hypotheses — ranked, uncertain engineering statements (RFC-019).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{empty_metadata, Confidence, InvestigationId, Metadata, ObjectId, Provenance};

/// Lifecycle status of a Hypothesis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HypothesisStatus {
    /// Newly generated, not yet assessed.
    Proposed,
    /// Current evidence supports the statement.
    Supported,
    /// Current evidence contradicts the statement.
    Contradicted,
    /// Verified by a Verification Receipt.
    Verified,
    /// Explicitly rejected.
    Rejected,
    /// Insufficient evidence either way.
    Inconclusive,
}

impl HypothesisStatus {
    /// Stable string form.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Supported => "supported",
            Self::Contradicted => "contradicted",
            Self::Verified => "verified",
            Self::Rejected => "rejected",
            Self::Inconclusive => "inconclusive",
        }
    }
}

/// Ranked, uncertain statement about what may be happening (RFC-019).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hypothesis {
    /// Stable identifier.
    pub id: ObjectId,
    /// Primary Investigation.
    pub investigation_id: InvestigationId,
    /// Uncertain statement.
    pub statement: String,
    /// Status.
    pub status: HypothesisStatus,
    /// Confidence (never fact without verification).
    pub confidence: Confidence,
    /// Supporting evidence object ids.
    pub supporting_evidence: Vec<ObjectId>,
    /// Contradicting evidence object ids.
    pub contradicting_evidence: Vec<ObjectId>,
    /// Related prior Investigation ids.
    pub related_investigation_ids: Vec<InvestigationId>,
    /// Derivation method description.
    pub derivation_method: String,
    /// Verification summary (e.g. unverified, partial).
    pub verification_summary: String,
    /// Rank among generated hypotheses (1 = strongest).
    pub rank: u32,
    /// When generated.
    pub generated_at: DateTime<Utc>,
    /// Provenance.
    pub provenance: Provenance,
    /// Metadata.
    pub metadata: Metadata,
}

impl Hypothesis {
    /// Construct a proposed hypothesis.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        investigation_id: InvestigationId,
        statement: impl Into<String>,
        status: HypothesisStatus,
        confidence: Confidence,
        supporting_evidence: Vec<ObjectId>,
        contradicting_evidence: Vec<ObjectId>,
        related_investigation_ids: Vec<InvestigationId>,
        derivation_method: impl Into<String>,
        verification_summary: impl Into<String>,
        rank: u32,
        provenance: Provenance,
    ) -> Self {
        Self {
            id: ObjectId::new(),
            investigation_id,
            statement: statement.into(),
            status,
            confidence,
            supporting_evidence,
            contradicting_evidence,
            related_investigation_ids,
            derivation_method: derivation_method.into(),
            verification_summary: verification_summary.into(),
            rank,
            generated_at: Utc::now(),
            provenance,
            metadata: empty_metadata(),
        }
    }
}
