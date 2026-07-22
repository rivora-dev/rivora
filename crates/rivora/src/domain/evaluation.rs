//! Evaluation — explainable engineering assessments (RFC-008).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{empty_metadata, Confidence, InvestigationId, Metadata, ObjectId, Provenance};

/// Severity of an Evaluation assessment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Informational only.
    Info,
    /// Low priority.
    Low,
    /// Moderate attention required.
    Medium,
    /// High priority.
    High,
    /// Critical.
    Critical,
}

impl Severity {
    /// Display name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

/// Type of assessment produced by Evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentType {
    /// Overall Investigation health.
    Health,
    /// Risk assessment.
    Risk,
    /// Confidence in understanding.
    Confidence,
    /// Readiness for action.
    Readiness,
    /// Severity classification.
    Severity,
}

impl AssessmentType {
    /// Stable string form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Health => "health",
            Self::Risk => "risk",
            Self::Confidence => "confidence",
            Self::Readiness => "readiness",
            Self::Severity => "severity",
        }
    }
}

/// Explainable Evaluation of Investigation Knowledge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evaluation {
    /// Stable object identifier.
    pub id: ObjectId,
    /// Primary Investigation.
    pub investigation_id: InvestigationId,
    /// Assessment category.
    pub assessment_type: AssessmentType,
    /// Human-readable assessment summary.
    pub summary: String,
    /// Severity of the assessment.
    pub severity: Severity,
    /// Confidence in the assessment.
    pub confidence: Confidence,
    /// Supporting Knowledge identifiers.
    pub supporting_knowledge_ids: Vec<ObjectId>,
    /// Supporting Memory identifiers (for direct evidence links).
    pub supporting_memory_ids: Vec<ObjectId>,
    /// Structured explanation of reasoning.
    pub explanation: String,
    /// When the Evaluation was produced.
    pub evaluated_at: DateTime<Utc>,
    /// Provenance.
    pub provenance: Provenance,
    /// Metadata.
    pub metadata: Metadata,
}

impl Evaluation {
    /// Construct an Evaluation.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        investigation_id: InvestigationId,
        assessment_type: AssessmentType,
        summary: impl Into<String>,
        severity: Severity,
        confidence: Confidence,
        supporting_knowledge_ids: Vec<ObjectId>,
        supporting_memory_ids: Vec<ObjectId>,
        explanation: impl Into<String>,
        provenance: Provenance,
    ) -> Self {
        Self {
            id: ObjectId::new(),
            investigation_id,
            assessment_type,
            summary: summary.into(),
            severity,
            confidence,
            supporting_knowledge_ids,
            supporting_memory_ids,
            explanation: explanation.into(),
            evaluated_at: Utc::now(),
            provenance,
            metadata: empty_metadata(),
        }
    }
}
