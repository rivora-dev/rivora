//! Engineering Assistance outputs (RFC-019).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{
    empty_metadata, Confidence, InvestigationId, Metadata, ObjectId, Provenance, Severity,
};

/// Feasibility of a suggested verification step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationFeasibility {
    /// Can be performed with available evidence or connectors.
    Feasible,
    /// Blocked by missing prerequisites.
    Blocked,
    /// Requires explicit human action.
    RequiresHuman,
}

impl VerificationFeasibility {
    /// Stable string form.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Feasible => "feasible",
            Self::Blocked => "blocked",
            Self::RequiresHuman => "requires_human",
        }
    }
}

/// Suggested next verification or inspection (RFC-019).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerificationSuggestion {
    /// Stable identifier.
    pub id: ObjectId,
    /// Primary Investigation.
    pub investigation_id: InvestigationId,
    /// Optional Hypothesis under test.
    pub hypothesis_id: Option<ObjectId>,
    /// Claim being tested.
    pub claim: String,
    /// Expected evidence description.
    pub expected_evidence: String,
    /// Why this verification matters.
    pub reason: String,
    /// Available method description.
    pub method: String,
    /// Estimated confidence impact in `[0.0, 1.0]`.
    pub estimated_confidence_impact: f64,
    /// Prerequisites.
    pub prerequisites: Vec<String>,
    /// Feasibility.
    pub feasibility: VerificationFeasibility,
    /// Whether confirmation is required before acting on the suggestion.
    pub confirmation_required: bool,
    /// Supporting evidence ids.
    pub supporting_evidence: Vec<ObjectId>,
    /// Rank (1 = best next).
    pub rank: u32,
    /// When generated.
    pub generated_at: DateTime<Utc>,
    /// Provenance.
    pub provenance: Provenance,
    /// Metadata.
    pub metadata: Metadata,
}

/// Deployment readiness status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessStatus {
    /// Evidence supports proceeding with care.
    Ready,
    /// Hold until blockers are resolved.
    Hold,
    /// Inspect further before deciding.
    Inspect,
    /// Insufficient evidence to assess.
    Unknown,
}

impl ReadinessStatus {
    /// Stable string form.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Hold => "hold",
            Self::Inspect => "inspect",
            Self::Unknown => "unknown",
        }
    }
}

/// One readiness dimension assessment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadinessDimension {
    /// Dimension name (e.g. `ci_status`, `verification_coverage`).
    pub name: String,
    /// Dimension status summary.
    pub status: String,
    /// Severity contribution.
    pub severity: Severity,
    /// Explanation.
    pub explanation: String,
    /// Supporting evidence ids.
    pub evidence_ids: Vec<ObjectId>,
}

/// Deployment readiness assessment (RFC-019).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeploymentReadiness {
    /// Stable identifier.
    pub id: ObjectId,
    /// Primary Investigation.
    pub investigation_id: InvestigationId,
    /// Overall readiness status.
    pub status: ReadinessStatus,
    /// Confidence in the assessment.
    pub confidence: Confidence,
    /// Assessed dimensions.
    pub dimensions: Vec<ReadinessDimension>,
    /// Blockers that force hold.
    pub blockers: Vec<String>,
    /// Non-blocking warnings.
    pub warnings: Vec<String>,
    /// Supporting evidence ids.
    pub supporting_evidence: Vec<ObjectId>,
    /// Contradicting evidence ids.
    pub contradicting_evidence: Vec<ObjectId>,
    /// Required verification descriptions.
    pub required_verifications: Vec<String>,
    /// Recommendation summary (proposal only).
    pub recommendation_summary: String,
    /// When assessed.
    pub assessed_at: DateTime<Utc>,
    /// Provenance.
    pub provenance: Provenance,
    /// Metadata.
    pub metadata: Metadata,
}

/// Risk forecast category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskCategory {
    /// Regression risk.
    Regression,
    /// Deployment risk.
    Deployment,
    /// Operational risk.
    Operational,
    /// Verification risk.
    Verification,
    /// Evidence quality risk.
    EvidenceQuality,
    /// Recurrence risk from history.
    Recurrence,
}

impl RiskCategory {
    /// Stable string form.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Regression => "regression",
            Self::Deployment => "deployment",
            Self::Operational => "operational",
            Self::Verification => "verification",
            Self::EvidenceQuality => "evidence_quality",
            Self::Recurrence => "recurrence",
        }
    }
}

/// One forecasted risk item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiskItem {
    /// Category.
    pub category: RiskCategory,
    /// Severity.
    pub severity: Severity,
    /// Confidence.
    pub confidence: Confidence,
    /// Supporting evidence ids.
    pub supporting_evidence: Vec<ObjectId>,
    /// Historical comparison note.
    pub historical_comparison: String,
    /// Mitigation or verification suggestion.
    pub mitigation: String,
    /// Explanation.
    pub explanation: String,
}

/// Risk forecast collection (RFC-019).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiskForecast {
    /// Stable identifier.
    pub id: ObjectId,
    /// Primary Investigation.
    pub investigation_id: InvestigationId,
    /// Forecasted risk items.
    pub items: Vec<RiskItem>,
    /// Overall summary.
    pub summary: String,
    /// When forecasted.
    pub forecasted_at: DateTime<Utc>,
    /// Provenance.
    pub provenance: Provenance,
    /// Metadata.
    pub metadata: Metadata,
}

/// Probabilistic root-cause guidance (RFC-019).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RootCauseGuidance {
    /// Stable identifier.
    pub id: ObjectId,
    /// Primary Investigation.
    pub investigation_id: InvestigationId,
    /// Leading hypothesis ids, ordered.
    pub leading_hypothesis_ids: Vec<ObjectId>,
    /// Narrative of leading guidance (probabilistic).
    pub guidance: String,
    /// Supporting evidence ids.
    pub supporting_evidence: Vec<ObjectId>,
    /// Contradicting evidence ids.
    pub contradicting_evidence: Vec<ObjectId>,
    /// Related prior Investigation ids.
    pub related_investigation_ids: Vec<InvestigationId>,
    /// Prior mitigation notes (labeled historical).
    pub prior_mitigation_notes: Vec<String>,
    /// Overall confidence.
    pub confidence: Confidence,
    /// Recommended verification order (claims).
    pub verification_order: Vec<String>,
    /// Known gaps.
    pub known_gaps: Vec<String>,
    /// When generated.
    pub generated_at: DateTime<Utc>,
    /// Provenance.
    pub provenance: Provenance,
    /// Metadata.
    pub metadata: Metadata,
}

/// One inspectable ranking factor for a Recommendation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankingFactor {
    /// Factor name.
    pub name: String,
    /// Weight in ranking.
    pub weight: f64,
    /// Contribution to score.
    pub contribution: f64,
    /// Explanation.
    pub explanation: String,
}

/// Prioritized Recommendation view (RFC-019).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrioritizedRecommendation {
    /// Recommendation id.
    pub recommendation_id: ObjectId,
    /// Rank (1 = strongest).
    pub rank: u32,
    /// Aggregate score.
    pub score: f64,
    /// Recommendation summary (copied for convenience).
    pub summary: String,
    /// Ranking factors.
    pub factors: Vec<RankingFactor>,
    /// Overall ranking explanation.
    pub explanation: String,
}

/// One section of an engineering report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportSection {
    /// Section title.
    pub title: String,
    /// Body text (markdown-friendly plain text).
    pub body: String,
    /// Referenced object ids.
    pub object_refs: Vec<ObjectId>,
}

/// Durable engineering report snapshot (RFC-019).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineeringReport {
    /// Stable identifier.
    pub id: ObjectId,
    /// Primary Investigation.
    pub investigation_id: InvestigationId,
    /// Report title.
    pub title: String,
    /// Ordered sections.
    pub sections: Vec<ReportSection>,
    /// Full markdown body.
    pub markdown: String,
    /// When generated.
    pub generated_at: DateTime<Utc>,
    /// Provenance.
    pub provenance: Provenance,
    /// Metadata.
    pub metadata: Metadata,
}

impl EngineeringReport {
    /// Build a report with empty metadata.
    pub fn new(
        investigation_id: InvestigationId,
        title: impl Into<String>,
        sections: Vec<ReportSection>,
        markdown: impl Into<String>,
        provenance: Provenance,
    ) -> Self {
        Self {
            id: ObjectId::new(),
            investigation_id,
            title: title.into(),
            sections,
            markdown: markdown.into(),
            generated_at: Utc::now(),
            provenance,
            metadata: empty_metadata(),
        }
    }
}

/// Concise investigation state summary (assistance).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvestigationSummary {
    /// Investigation id.
    pub investigation_id: InvestigationId,
    /// Title.
    pub title: String,
    /// Status string.
    pub status: String,
    /// Summary narrative.
    pub summary: String,
    /// Counts of key object kinds.
    pub counts: SummaryCounts,
    /// Leading open questions / gaps.
    pub gaps: Vec<String>,
    /// When summarized.
    pub summarized_at: DateTime<Utc>,
    /// Provenance.
    pub provenance: Provenance,
}

/// Object counts for a summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SummaryCounts {
    /// Memory records.
    pub memory: usize,
    /// Knowledge objects.
    pub knowledge: usize,
    /// Evaluations.
    pub evaluations: usize,
    /// Verifications.
    pub verifications: usize,
    /// Recommendations.
    pub recommendations: usize,
    /// Hypotheses.
    pub hypotheses: usize,
    /// Learning outcomes.
    pub learning: usize,
}
