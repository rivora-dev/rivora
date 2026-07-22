//! Recalled Context — explicit historical context for a current
//! Investigation (RFC-017).
//!
//! A Recalled Context record belongs to the current Investigation and
//! references a source Investigation. Historical intelligence informs
//! current reasoning; it never becomes current fact. Only attached
//! context influences Evaluation and Recommendation reasoning; dismissed
//! context never does.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::{Confidence, InvestigationId, ObjectId, Provenance};
use crate::error::{RivoraError, RivoraResult};

/// How a Recalled Context record entered the current Investigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecallOrigin {
    /// The Runtime recalled the context automatically from related or
    /// similar Investigations.
    Automatic,
    /// A human (or explicit caller) selected the context.
    Manual,
}

impl RecallOrigin {
    /// Stable string form.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::Manual => "manual",
        }
    }
}

/// Lifecycle state of a Recalled Context record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecalledContextState {
    /// Recalled automatically; not yet reviewed.
    Suggested,
    /// Confirmed as relevant input to current reasoning.
    Attached,
    /// Rejected as irrelevant; never influences reasoning.
    Dismissed,
}

impl RecalledContextState {
    /// Stable string form.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Suggested => "suggested",
            Self::Attached => "attached",
            Self::Dismissed => "dismissed",
        }
    }
}

/// Explicit, provenance-preserving historical context recalled into a
/// current Investigation (RFC-017).
///
/// The record is owned by the current Investigation. It is never
/// appended to the source Investigation and never appears in any
/// Investigation's Memory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecalledContext {
    /// Stable identifier.
    pub id: ObjectId,
    /// Current Investigation (owner).
    pub investigation_id: InvestigationId,
    /// Source Investigation the evidence comes from.
    pub source_investigation_id: InvestigationId,
    /// Source Engineering Object IDs the context references.
    pub source_object_ids: Vec<ObjectId>,
    /// Human-readable summary of the selected evidence.
    pub evidence_summary: String,
    /// Reason for the recall.
    pub reason: String,
    /// Relationship or search explanation that justified the recall.
    pub explanation: String,
    /// Confidence in the relevance of the context.
    pub confidence: Confidence,
    /// Whether the context was automatically recalled or explicitly
    /// selected.
    pub origin: RecallOrigin,
    /// Suggested, attached, or dismissed.
    pub state: RecalledContextState,
    /// When the context was recalled.
    pub recalled_at: DateTime<Utc>,
    /// Provenance of the recall.
    pub provenance: Provenance,
}

impl RecalledContext {
    /// Create a recalled context record in the given state.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        investigation_id: InvestigationId,
        source_investigation_id: InvestigationId,
        source_object_ids: Vec<ObjectId>,
        evidence_summary: impl Into<String>,
        reason: impl Into<String>,
        explanation: impl Into<String>,
        confidence: Confidence,
        origin: RecallOrigin,
        state: RecalledContextState,
        provenance: Provenance,
    ) -> RivoraResult<Self> {
        if investigation_id == source_investigation_id {
            return Err(RivoraError::validation(
                "cannot recall context from the same investigation",
            ));
        }
        let evidence_summary = evidence_summary.into();
        let reason = reason.into();
        if reason.trim().is_empty() {
            return Err(RivoraError::validation("recall reason cannot be empty"));
        }
        Ok(Self {
            id: ObjectId::new(),
            investigation_id,
            source_investigation_id,
            source_object_ids,
            evidence_summary,
            reason,
            explanation: explanation.into(),
            confidence,
            origin,
            state,
            recalled_at: Utc::now(),
            provenance,
        })
    }

    /// True when this context may influence Evaluation and Recommendation.
    pub fn influences_reasoning(&self) -> bool {
        self.state == RecalledContextState::Attached
    }

    /// Mark the context as attached (confirmed relevant).
    pub fn attach(&mut self) {
        self.state = RecalledContextState::Attached;
    }

    /// Mark the context as dismissed (never influences reasoning).
    pub fn dismiss(&mut self) {
        self.state = RecalledContextState::Dismissed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recalled_context_validates_and_serializes() {
        let context = RecalledContext::new(
            InvestigationId::new(),
            InvestigationId::new(),
            vec![ObjectId::new()],
            "prior failing build evidence",
            "similar failure signature",
            "shared_repository relationship",
            Confidence::new(0.9),
            RecallOrigin::Automatic,
            RecalledContextState::Suggested,
            Provenance::now("tester", "runtime"),
        )
        .unwrap();
        let json = serde_json::to_string_pretty(&context).unwrap();
        assert!(json.contains("\"origin\": \"automatic\""));
        assert!(json.contains("\"state\": \"suggested\""));
        let decoded: RecalledContext = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, context);
    }

    #[test]
    fn recalled_context_rejects_self_reference_and_empty_reason() {
        let id = InvestigationId::new();
        let err = RecalledContext::new(
            id,
            id,
            vec![],
            "evidence",
            "reason",
            "explanation",
            Confidence::neutral(),
            RecallOrigin::Manual,
            RecalledContextState::Attached,
            Provenance::now("tester", "runtime"),
        )
        .unwrap_err();
        assert!(matches!(err, RivoraError::Validation(_)));

        let err = RecalledContext::new(
            InvestigationId::new(),
            InvestigationId::new(),
            vec![],
            "evidence",
            "   ",
            "explanation",
            Confidence::neutral(),
            RecallOrigin::Manual,
            RecalledContextState::Attached,
            Provenance::now("tester", "runtime"),
        )
        .unwrap_err();
        assert!(matches!(err, RivoraError::Validation(_)));
    }
}
