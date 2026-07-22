//! Verification Receipts (RFC-009).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{empty_metadata, Confidence, InvestigationId, Metadata, ObjectId, Provenance};

/// Result of a verification attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationResult {
    /// Conclusion is sufficiently supported.
    Pass,
    /// Conclusion is contradicted or unsupported.
    Fail,
    /// Evidence is insufficient to decide.
    Inconclusive,
}

impl VerificationResult {
    /// Display name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Inconclusive => "inconclusive",
        }
    }
}

/// Durable Verification Receipt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerificationReceipt {
    /// Stable object identifier.
    pub id: ObjectId,
    /// Primary Investigation.
    pub investigation_id: InvestigationId,
    /// Evaluation (or conclusion) that was verified.
    pub evaluation_id: ObjectId,
    /// What was verified.
    pub subject: String,
    /// Verification outcome.
    pub result: VerificationResult,
    /// Confidence in the verification.
    pub confidence: Confidence,
    /// Evidence object identifiers used.
    pub evidence_ids: Vec<ObjectId>,
    /// Conflicting evidence identifiers.
    pub conflicting_ids: Vec<ObjectId>,
    /// Explanation, including failure or inconclusive reasons.
    pub reason: String,
    /// When verification ran.
    pub verified_at: DateTime<Utc>,
    /// Provenance.
    pub provenance: Provenance,
    /// Metadata.
    pub metadata: Metadata,
}

impl VerificationReceipt {
    /// Construct a Verification Receipt.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        investigation_id: InvestigationId,
        evaluation_id: ObjectId,
        subject: impl Into<String>,
        result: VerificationResult,
        confidence: Confidence,
        evidence_ids: Vec<ObjectId>,
        conflicting_ids: Vec<ObjectId>,
        reason: impl Into<String>,
        provenance: Provenance,
    ) -> Self {
        Self {
            id: ObjectId::new(),
            investigation_id,
            evaluation_id,
            subject: subject.into(),
            result,
            confidence,
            evidence_ids,
            conflicting_ids,
            reason: reason.into(),
            verified_at: Utc::now(),
            provenance,
            metadata: empty_metadata(),
        }
    }
}
