//! Durable Improvement Proposals (RFC-020 and RFC-021).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{Confidence, InvestigationId, ObjectId, Provenance, Severity};
use crate::error::{RivoraError, RivoraResult};

macro_rules! string_enum {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident => $value:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($(#[$vmeta])* $variant),+ }
        impl $name {
            /// Stable string form.
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $value),+ }
            }
        }
    };
}

string_enum!(
    /// Improvement Proposal category.
    ProposalCategory {
        /// Source-code change.
        Code => "code",
        /// Configuration change.
        Configuration => "configuration",
        /// Test or fixture change.
        Testing => "testing",
        /// Reliability improvement.
        Reliability => "reliability",
        /// Performance improvement.
        Performance => "performance",
        /// Security improvement.
        Security => "security",
        /// Observability improvement.
        Observability => "observability",
        /// Infrastructure improvement.
        Infrastructure => "infrastructure",
        /// Developer-experience improvement.
        DeveloperExperience => "developer_experience",
        /// Process improvement.
        Process => "process",
        /// Documentation improvement.
        Documentation => "documentation"
    }
);

string_enum!(
    /// Human-controlled Proposal lifecycle status.
    ProposalStatus {
        /// Generated candidate, not yet submitted.
        Draft => "draft",
        /// Submitted for consideration.
        Proposed => "proposed",
        /// Under explicit human review.
        UnderReview => "under_review",
        /// Accepted for possible later implementation.
        Accepted => "accepted",
        /// Rejected while remaining durable.
        Rejected => "rejected",
        /// Deferred while remaining durable.
        Deferred => "deferred",
        /// Replaced by another Proposal.
        Superseded => "superseded",
        /// Withdrawn by an explicit caller.
        Withdrawn => "withdrawn"
    }
);

string_enum!(
    /// Proposal priority.
    ProposalPriority {
        /// Immediate critical need.
        Critical => "critical",
        /// High priority.
        High => "high",
        /// Medium priority.
        Medium => "medium",
        /// Low priority.
        Low => "low",
        /// Exploratory candidate.
        Exploratory => "exploratory"
    }
);

string_enum!(
    /// Coarse implementation effort estimate.
    ProposalEffort {
        /// Small local change.
        Small => "small",
        /// Moderate multi-file change.
        Medium => "medium",
        /// Large or migration-heavy change.
        Large => "large"
    }
);

string_enum!(
    /// Method used to generate a Proposal.
    ProposalGenerationMethod {
        /// Explicitly created by a caller.
        Human => "human",
        /// Deterministic local Runtime baseline.
        Deterministic => "deterministic",
        /// Optional model-assisted structured generation.
        ModelAssisted => "model_assisted"
    }
);

string_enum!(
    /// Whether evidence belongs to the current or a historical Investigation.
    EvidenceScope {
        /// Current Investigation evidence.
        Current => "current",
        /// Labeled historical evidence.
        Historical => "historical"
    }
);

string_enum!(
    /// Authority requesting a lifecycle transition.
    ProposalTransitionAuthority {
        /// Rivora Runtime automation.
        Runtime => "runtime",
        /// Explicit human or external caller action.
        ExternalCaller => "external_caller"
    }
);

string_enum!(
    /// Explicit feedback category.
    ProposalFeedbackCategory {
        /// Proposal is too broad.
        TooBroad => "too_broad",
        /// Proposal is too risky.
        TooRisky => "too_risky",
        /// Proposal is too expensive.
        TooExpensive => "too_expensive",
        /// Evidence is insufficient.
        InsufficientEvidence => "insufficient_evidence",
        /// Affected component is incorrect.
        WrongComponent => "wrong_component",
        /// An alternative is missing.
        MissingAlternative => "missing_alternative",
        /// A test is missing.
        MissingTest => "missing_test",
        /// Proposal violates architecture.
        ViolatesArchitecture => "violates_architecture",
        /// Proposal should be split.
        ShouldSplit => "should_split",
        /// Proposal should be combined.
        ShouldCombine => "should_combine",
        /// More verification is required.
        NeedsVerification => "needs_verification",
        /// General explicit feedback.
        Other => "other"
    }
);

/// Reference to evidence with explicit temporal scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceReference {
    /// Engineering Object identifier.
    pub object_id: ObjectId,
    /// Current or historical scope.
    pub scope: EvidenceScope,
}

/// One Proposal risk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProposalRisk {
    /// Risk description.
    pub description: String,
    /// Risk severity.
    pub severity: Severity,
    /// Proposed mitigation or verification.
    pub mitigation: String,
}

/// One alternative summarized inside a Proposal artifact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProposalAlternative {
    /// Alternative title.
    pub title: String,
    /// Expected benefit.
    pub expected_benefit: String,
    /// Effort category.
    pub effort: ProposalEffort,
    /// Implementation risk.
    pub implementation_risk: String,
    /// Verification complexity.
    pub verification_complexity: String,
    /// Reversibility description.
    pub reversibility: String,
    /// Architectural fit.
    pub architectural_fit: String,
    /// Evidence strength description.
    pub evidence_strength: String,
    /// Known drawbacks.
    pub drawbacks: Vec<String>,
}

/// Concrete, unexecuted Verification Plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProposalVerificationPlan {
    /// Claims to verify.
    pub claims: Vec<String>,
    /// Required preconditions.
    pub preconditions: Vec<String>,
    /// Tests or fixtures to run or add.
    pub tests: Vec<String>,
    /// Static, compatibility, or migration checks.
    pub checks: Vec<String>,
    /// Manual workflows.
    pub manual_workflows: Vec<String>,
    /// Expected evidence.
    pub expected_evidence: Vec<String>,
    /// Success criteria.
    pub success_criteria: Vec<String>,
    /// Failure criteria.
    pub failure_criteria: Vec<String>,
    /// Inconclusive conditions.
    pub inconclusive_conditions: Vec<String>,
    /// Recovery or rollback checks.
    pub recovery_checks: Vec<String>,
}

/// Preserved lifecycle transition provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalTransition {
    /// Previous status.
    pub from: ProposalStatus,
    /// New status.
    pub to: ProposalStatus,
    /// Explicit actor.
    pub actor: String,
    /// Non-empty reason.
    pub reason: String,
    /// Transition timestamp.
    pub at: DateTime<Utc>,
}

/// Explicit Proposal feedback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalFeedback {
    /// Feedback category.
    pub category: ProposalFeedbackCategory,
    /// Feedback comment.
    pub comment: String,
    /// Actor providing feedback.
    pub actor: String,
    /// Feedback timestamp.
    pub at: DateTime<Utc>,
}

/// Durable concrete candidate improvement, never an applied change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImprovementProposal {
    /// Snapshot identifier.
    pub id: ObjectId,
    /// Owning Investigation.
    pub investigation_id: InvestigationId,
    /// Stable lineage identifier.
    pub lineage_id: ObjectId,
    /// One-based revision number.
    pub revision_number: u32,
    /// Prior immutable snapshot.
    pub parent_proposal_id: Option<ObjectId>,
    /// Replacement Proposal when explicitly superseded.
    pub superseding_proposal_id: Option<ObjectId>,
    /// Proposal title.
    pub title: String,
    /// Concise summary.
    pub summary: String,
    /// Detailed evidence-backed rationale.
    pub rationale: String,
    /// Category.
    pub category: ProposalCategory,
    /// Lifecycle status.
    pub status: ProposalStatus,
    /// Priority.
    pub priority: ProposalPriority,
    /// Confidence.
    pub confidence: Confidence,
    /// Expected impact.
    pub expected_impact: String,
    /// Affected components.
    pub affected_components: Vec<String>,
    /// Likely affected files or resources, never treated as authoritative.
    pub affected_resources: Vec<String>,
    /// Supporting evidence.
    pub supporting_evidence: Vec<EvidenceReference>,
    /// Contradicting evidence.
    pub contradicting_evidence: Vec<EvidenceReference>,
    /// Related Hypotheses.
    pub hypothesis_ids: Vec<ObjectId>,
    /// Related Evaluations.
    pub evaluation_ids: Vec<ObjectId>,
    /// Related Verification Receipts.
    pub verification_ids: Vec<ObjectId>,
    /// Source Recommendations.
    pub source_recommendation_ids: Vec<ObjectId>,
    /// Related historical Investigations.
    pub related_investigation_ids: Vec<InvestigationId>,
    /// Relevant Learning Outcomes.
    pub learning_outcome_ids: Vec<ObjectId>,
    /// Assumptions.
    pub assumptions: Vec<String>,
    /// Constraints.
    pub constraints: Vec<String>,
    /// Risks.
    pub risks: Vec<ProposalRisk>,
    /// Alternatives considered.
    pub alternatives: Vec<ProposalAlternative>,
    /// Bounded expected implementation steps.
    pub implementation_outline: Vec<String>,
    /// Proposed test strategy.
    pub test_strategy: Vec<String>,
    /// Proposed Verification Plan.
    pub verification_plan: ProposalVerificationPlan,
    /// Success criteria.
    pub success_criteria: Vec<String>,
    /// Rollback or reversibility considerations.
    pub reversibility: String,
    /// Effort estimate.
    pub estimated_effort: ProposalEffort,
    /// Unresolved questions.
    pub unresolved_questions: Vec<String>,
    /// Optional inert external implementation reference, never inferred.
    pub external_implementation_reference: Option<String>,
    /// Original creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Snapshot update timestamp.
    pub updated_at: DateTime<Utc>,
    /// Provenance.
    pub provenance: Provenance,
    /// Generation method.
    pub generation_method: ProposalGenerationMethod,
    /// Versioned derivation method.
    pub derivation_method: String,
    /// Preserved lifecycle transitions.
    pub transitions: Vec<ProposalTransition>,
    /// Explicit feedback history.
    pub feedback: Vec<ProposalFeedback>,
}

impl ImprovementProposal {
    /// Construct a deterministic or model-assisted Draft Proposal.
    #[allow(clippy::too_many_arguments)]
    pub fn generated(
        investigation_id: InvestigationId,
        title: impl Into<String>,
        summary: impl Into<String>,
        rationale: impl Into<String>,
        category: ProposalCategory,
        priority: ProposalPriority,
        confidence: Confidence,
        generation_method: ProposalGenerationMethod,
        provenance: Provenance,
    ) -> RivoraResult<Self> {
        let title = title.into();
        let summary = summary.into();
        let rationale = rationale.into();
        if title.trim().is_empty() || summary.trim().is_empty() || rationale.trim().is_empty() {
            return Err(RivoraError::validation(
                "proposal title, summary, and rationale are required",
            ));
        }
        let id = ObjectId::new();
        let now = Utc::now();
        Ok(Self {
            id,
            investigation_id,
            lineage_id: id,
            revision_number: 1,
            parent_proposal_id: None,
            superseding_proposal_id: None,
            title: title.trim().into(),
            summary: summary.trim().into(),
            rationale: rationale.trim().into(),
            category,
            status: ProposalStatus::Draft,
            priority,
            confidence,
            expected_impact: String::new(),
            affected_components: Vec::new(),
            affected_resources: Vec::new(),
            supporting_evidence: Vec::new(),
            contradicting_evidence: Vec::new(),
            hypothesis_ids: Vec::new(),
            evaluation_ids: Vec::new(),
            verification_ids: Vec::new(),
            source_recommendation_ids: Vec::new(),
            related_investigation_ids: Vec::new(),
            learning_outcome_ids: Vec::new(),
            assumptions: Vec::new(),
            constraints: Vec::new(),
            risks: Vec::new(),
            alternatives: Vec::new(),
            implementation_outline: Vec::new(),
            test_strategy: Vec::new(),
            verification_plan: ProposalVerificationPlan::default(),
            success_criteria: Vec::new(),
            reversibility: String::new(),
            estimated_effort: ProposalEffort::Medium,
            unresolved_questions: Vec::new(),
            external_implementation_reference: None,
            created_at: now,
            updated_at: now,
            provenance,
            generation_method,
            derivation_method: "proposal_manual_v1".into(),
            transitions: Vec::new(),
            feedback: Vec::new(),
        })
    }

    /// Create an immutable successor snapshot for a lifecycle transition.
    pub fn transitioned(
        &self,
        to: ProposalStatus,
        actor: impl Into<String>,
        reason: impl Into<String>,
        at: DateTime<Utc>,
        authority: ProposalTransitionAuthority,
    ) -> RivoraResult<Self> {
        let actor = actor.into();
        let reason = reason.into();
        if actor.trim().is_empty() || reason.trim().is_empty() {
            return Err(RivoraError::validation(
                "proposal transition actor and reason are required",
            ));
        }
        if to == ProposalStatus::Accepted
            && authority != ProposalTransitionAuthority::ExternalCaller
        {
            return Err(RivoraError::validation(
                "only an explicit external caller may accept a proposal",
            ));
        }
        if !valid_transition(self.status, to) {
            return Err(RivoraError::validation(format!(
                "invalid proposal transition: {} -> {}",
                self.status.as_str(),
                to.as_str()
            )));
        }
        let mut next = self.clone();
        next.id = ObjectId::new();
        next.parent_proposal_id = Some(self.id);
        next.revision_number = self.revision_number.saturating_add(1);
        next.status = to;
        next.updated_at = at;
        next.transitions.push(ProposalTransition {
            from: self.status,
            to,
            actor: actor.trim().into(),
            reason: reason.trim().into(),
            at,
        });
        Ok(next)
    }

    /// Create an immutable content revision, preserving feedback and lifecycle history.
    pub fn revised(
        &self,
        actor: impl Into<String>,
        reason: impl Into<String>,
        at: DateTime<Utc>,
    ) -> RivoraResult<Self> {
        let actor = actor.into();
        let reason = reason.into();
        if actor.trim().is_empty() || reason.trim().is_empty() {
            return Err(RivoraError::validation(
                "proposal revision actor and reason are required",
            ));
        }
        let mut next = self.clone();
        next.id = ObjectId::new();
        next.parent_proposal_id = Some(self.id);
        next.revision_number = self.revision_number.saturating_add(1);
        next.updated_at = at;
        next.provenance = Provenance::now(actor.trim(), "runtime")
            .with_capability("refine_improvement_proposal")
            .with_evidence(vec![self.id]);
        next.unresolved_questions
            .push(format!("Revision reason: {}", reason.trim()));
        Ok(next)
    }
}

fn valid_transition(from: ProposalStatus, to: ProposalStatus) -> bool {
    use ProposalStatus::*;
    match from {
        Draft => matches!(to, Proposed | Rejected | Deferred | Superseded | Withdrawn),
        Proposed => matches!(
            to,
            UnderReview | Rejected | Deferred | Superseded | Withdrawn
        ),
        UnderReview => matches!(
            to,
            Proposed | Accepted | Rejected | Deferred | Superseded | Withdrawn
        ),
        Deferred => matches!(
            to,
            Proposed | UnderReview | Rejected | Superseded | Withdrawn
        ),
        Accepted | Rejected | Superseded | Withdrawn => false,
    }
}

/// One isolated corrupted Proposal record diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalStorageDiagnostic {
    /// Corrupted record path.
    pub path: String,
    /// Serialization error message.
    pub error: String,
}

/// Valid Proposal records plus visible corruption diagnostics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProposalListing {
    /// Valid records, deterministically ordered.
    pub proposals: Vec<ImprovementProposal>,
    /// Corrupted sibling records that were isolated.
    pub diagnostics: Vec<ProposalStorageDiagnostic>,
}
