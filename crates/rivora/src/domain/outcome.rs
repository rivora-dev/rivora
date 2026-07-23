//! Implementation Records, Measured Learning Outcomes, and Learning Patterns
//! (RFC-022, RFC-023, RFC-024).
//!
//! These types close the feedback loop after an Improvement Proposal is
//! implemented *outside* Rivora. They are distinct from the v0.1 Recommendation
//! disposition type [`super::LearningOutcome`].

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{Confidence, InvestigationId, ObjectId, Provenance};
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

// ---------------------------------------------------------------------------
// Implementation Record
// ---------------------------------------------------------------------------

string_enum!(
    /// How an external implementation was reported.
    ImplementationSource {
        /// Explicitly declared by a human.
        HumanDeclared => "human_declared",
        /// Linked to a Git commit.
        GitCommit => "git_commit",
        /// Linked to a pull request.
        PullRequest => "pull_request",
        /// Linked to a patch.
        Patch => "patch",
        /// Linked to a deployment.
        Deployment => "deployment",
        /// Linked to a configuration change.
        ConfigurationChange => "configuration_change",
        /// Linked to a runbook execution.
        RunbookExecution => "runbook_execution",
        /// Reported by an external coding agent (without Rivora invoking it).
        ExternalAgent => "external_agent",
        /// Other source.
        Other => "other"
    }
);

string_enum!(
    /// Lifecycle status of an Implementation Record.
    ///
    /// Status never encodes success of the change.
    ImplementationStatus {
        /// Reported without linked evidence.
        Reported => "reported",
        /// Evidence object identifiers have been linked.
        EvidenceLinked => "evidence_linked",
        /// Ready for Measured Learning Outcome evaluation.
        ReadyForEvaluation => "ready_for_evaluation",
        /// Withdrawn with an explicit reason.
        Withdrawn => "withdrawn",
        /// Replaced by a superseding record.
        Superseded => "superseded"
    }
);

/// Typed external reference describing how work was performed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImplementationReference {
    /// Git commit SHA.
    CommitSha {
        /// Commit hash.
        sha: String,
    },
    /// Pull request identifier or URL.
    PullRequest {
        /// PR number or URL.
        reference: String,
    },
    /// Branch name.
    Branch {
        /// Branch name.
        name: String,
    },
    /// Deployment identifier.
    DeploymentId {
        /// Deployment id.
        id: String,
    },
    /// Build identifier.
    BuildId {
        /// Build id.
        id: String,
    },
    /// Incident identifier.
    IncidentId {
        /// Incident id.
        id: String,
    },
    /// Workflow run identifier.
    WorkflowRun {
        /// Workflow run id or URL.
        id: String,
    },
    /// Path to an artifact.
    ArtifactPath {
        /// Artifact path.
        path: String,
    },
    /// External URI (never fetched automatically).
    ExternalUri {
        /// URI string.
        uri: String,
    },
    /// Free-form human note.
    HumanNote {
        /// Note text.
        note: String,
    },
}

/// Preserved Implementation Record lifecycle transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplementationTransition {
    /// Previous status.
    pub from: ImplementationStatus,
    /// New status.
    pub to: ImplementationStatus,
    /// Explicit actor.
    pub actor: String,
    /// Non-empty reason.
    pub reason: String,
    /// Transition timestamp.
    pub at: DateTime<Utc>,
}

/// Durable evidence that external work was performed for a Proposal.
///
/// Does **not** prove the work succeeded. Acceptance of a Proposal never
/// creates an Implementation Record automatically.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImplementationRecord {
    /// Snapshot identifier.
    pub id: ObjectId,
    /// Stable lineage across revisions.
    pub lineage_id: ObjectId,
    /// One-based revision number.
    pub revision_number: u32,
    /// Prior immutable snapshot.
    pub parent_record_id: Option<ObjectId>,
    /// Successor when explicitly superseded.
    pub superseding_record_id: Option<ObjectId>,
    /// Owning Investigation.
    pub investigation_id: InvestigationId,
    /// Exact Proposal snapshot referenced at creation.
    pub proposal_id: ObjectId,
    /// Proposal lineage.
    pub proposal_lineage_id: ObjectId,
    /// Proposal revision at link time.
    pub proposal_revision_number: u32,
    /// Who reported the implementation.
    pub actor: String,
    /// Typed source of the report.
    pub source: ImplementationSource,
    /// Lifecycle status.
    pub status: ImplementationStatus,
    /// Human-readable summary.
    pub summary: String,
    /// Typed implementation references.
    pub references: Vec<ImplementationReference>,
    /// Optional implementation timestamp.
    pub implemented_at: Option<DateTime<Utc>>,
    /// Declared observed files (never authoritative).
    pub observed_files: Vec<String>,
    /// Declared observed components.
    pub observed_components: Vec<String>,
    /// Declared scope description.
    pub declared_scope: String,
    /// Linked evidence object identifiers.
    pub evidence_ids: Vec<ObjectId>,
    /// Preserved lifecycle transitions.
    pub transitions: Vec<ImplementationTransition>,
    /// Provenance.
    pub provenance: Provenance,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Snapshot update timestamp.
    pub updated_at: DateTime<Utc>,
}

impl ImplementationRecord {
    /// Create a newly reported Implementation Record (revision 1).
    #[allow(clippy::too_many_arguments)]
    pub fn reported(
        investigation_id: InvestigationId,
        proposal_id: ObjectId,
        proposal_lineage_id: ObjectId,
        proposal_revision_number: u32,
        actor: impl Into<String>,
        source: ImplementationSource,
        summary: impl Into<String>,
        provenance: Provenance,
    ) -> RivoraResult<Self> {
        let actor = actor.into();
        let summary = summary.into();
        if actor.trim().is_empty() {
            return Err(RivoraError::validation(
                "implementation record actor is required",
            ));
        }
        if summary.trim().is_empty() {
            return Err(RivoraError::validation(
                "implementation record summary is required",
            ));
        }
        let id = ObjectId::new();
        let now = Utc::now();
        Ok(Self {
            id,
            lineage_id: id,
            revision_number: 1,
            parent_record_id: None,
            superseding_record_id: None,
            investigation_id,
            proposal_id,
            proposal_lineage_id,
            proposal_revision_number,
            actor: actor.trim().into(),
            source,
            status: ImplementationStatus::Reported,
            summary: summary.trim().into(),
            references: Vec::new(),
            implemented_at: None,
            observed_files: Vec::new(),
            observed_components: Vec::new(),
            declared_scope: String::new(),
            evidence_ids: Vec::new(),
            transitions: Vec::new(),
            provenance,
            created_at: now,
            updated_at: now,
        })
    }

    /// Create an immutable content revision, preserving lifecycle history.
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
                "implementation revision actor and reason are required",
            ));
        }
        if matches!(
            self.status,
            ImplementationStatus::Withdrawn | ImplementationStatus::Superseded
        ) {
            return Err(RivoraError::validation(format!(
                "cannot revise implementation in terminal status {}",
                self.status.as_str()
            )));
        }
        let mut next = self.clone();
        next.id = ObjectId::new();
        next.parent_record_id = Some(self.id);
        next.revision_number = self.revision_number.saturating_add(1);
        next.updated_at = at;
        next.provenance = Provenance::now(actor.trim(), "runtime")
            .with_capability("revise_implementation_record")
            .with_evidence(vec![self.id]);
        Ok(next)
    }

    /// Create an immutable successor snapshot for a lifecycle transition.
    pub fn transitioned(
        &self,
        to: ImplementationStatus,
        actor: impl Into<String>,
        reason: impl Into<String>,
        at: DateTime<Utc>,
    ) -> RivoraResult<Self> {
        let actor = actor.into();
        let reason = reason.into();
        if actor.trim().is_empty() || reason.trim().is_empty() {
            return Err(RivoraError::validation(
                "implementation transition actor and reason are required",
            ));
        }
        if !valid_implementation_transition(self.status, to) {
            return Err(RivoraError::validation(format!(
                "invalid implementation transition: {} -> {}",
                self.status.as_str(),
                to.as_str()
            )));
        }
        let mut next = self.clone();
        next.id = ObjectId::new();
        next.parent_record_id = Some(self.id);
        next.revision_number = self.revision_number.saturating_add(1);
        next.status = to;
        next.updated_at = at;
        next.transitions.push(ImplementationTransition {
            from: self.status,
            to,
            actor: actor.trim().into(),
            reason: reason.trim().into(),
            at,
        });
        next.provenance = Provenance::now(actor.trim(), "runtime")
            .with_capability("transition_implementation_record")
            .with_evidence(vec![self.id]);
        Ok(next)
    }
}

fn valid_implementation_transition(from: ImplementationStatus, to: ImplementationStatus) -> bool {
    use ImplementationStatus::*;
    match from {
        Reported => matches!(
            to,
            EvidenceLinked | ReadyForEvaluation | Withdrawn | Superseded
        ),
        EvidenceLinked => matches!(to, ReadyForEvaluation | Withdrawn | Superseded),
        ReadyForEvaluation => matches!(to, Withdrawn | Superseded),
        Withdrawn | Superseded => false,
    }
}

/// One isolated corrupted Implementation Record diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplementationStorageDiagnostic {
    /// Corrupted record path.
    pub path: String,
    /// Serialization error message.
    pub error: String,
}

/// Valid Implementation Records plus visible corruption diagnostics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ImplementationListing {
    /// Valid records, deterministically ordered.
    pub records: Vec<ImplementationRecord>,
    /// Corrupted sibling records that were isolated.
    pub diagnostics: Vec<ImplementationStorageDiagnostic>,
}

// ---------------------------------------------------------------------------
// Measured Learning Outcome
// ---------------------------------------------------------------------------

string_enum!(
    /// Lifecycle status of a Measured Learning Outcome.
    MeasuredOutcomeStatus {
        /// Outcome exists but has not collected enough evidence.
        Draft => "draft",
        /// Baseline and post-change evidence are being assembled.
        EvidenceCollection => "evidence_collection",
        /// Deterministic evaluation is running or awaiting completion.
        UnderEvaluation => "under_evaluation",
        /// A conclusion exists but is not yet explicitly verified.
        Evaluated => "evaluated",
        /// An authorized actor has confirmed the conclusion.
        Verified => "verified",
        /// Retained but no longer active.
        Archived => "archived",
        /// Invalid or abandoned Outcome; reason required.
        Withdrawn => "withdrawn",
        /// Replaced by a newer Outcome; successor reference required.
        Superseded => "superseded"
    }
);

string_enum!(
    /// Overall classification of a Measured Learning Outcome.
    OutcomeClassification {
        /// Default before evaluation.
        Pending => "pending",
        /// All required expectations satisfied and no material regression.
        Successful => "successful",
        /// Most required expectations satisfied with bounded gaps.
        PartiallySuccessful => "partially_successful",
        /// Meaningful benefits and harms coexist.
        Mixed => "mixed",
        /// Required expectations not satisfied.
        Unsuccessful => "unsuccessful",
        /// Material net degradation.
        Regressed => "regressed",
        /// Insufficient or contradictory evidence.
        Inconclusive => "inconclusive",
        /// Implementation not proven.
        NotImplemented => "not_implemented",
        /// Measurement assumptions invalid.
        Invalidated => "invalidated"
    }
);

string_enum!(
    /// Kind of measurable expected result.
    ExpectedResultKind {
        /// Boolean pass/fail.
        Boolean => "boolean",
        /// Numeric threshold.
        NumericThreshold => "numeric_threshold",
        /// Directional improvement.
        DirectionalImprovement => "directional_improvement",
        /// Categorical state.
        CategoricalState => "categorical_state",
        /// Event occurrence.
        EventOccurrence => "event_occurrence",
        /// Latency or duration.
        LatencyDuration => "latency_duration",
        /// Count or frequency.
        CountFrequency => "count_frequency",
        /// Reliability rate.
        ReliabilityRate => "reliability_rate",
        /// Test result.
        TestResult => "test_result",
        /// Human assessment.
        HumanAssessment => "human_assessment",
        /// Composite criteria.
        Composite => "composite"
    }
);

string_enum!(
    /// Per-expected-result assessment kind (RFC-023).
    ResultAssessmentKind {
        /// Expectation fully met.
        Satisfied => "satisfied",
        /// Expectation partially met.
        PartiallySatisfied => "partially_satisfied",
        /// Expectation not met.
        NotSatisfied => "not_satisfied",
        /// Result regressed relative to baseline.
        Regressed => "regressed",
        /// Evidence insufficient to decide.
        Inconclusive => "inconclusive",
        /// Result was not measured.
        NotMeasured => "not_measured",
        /// Assessment invalid under current assumptions.
        Invalid => "invalid"
    }
);

string_enum!(
    /// Typed relationship between evidence and a Measured Learning Outcome.
    OutcomeEvidenceRelation {
        /// Supports an expected result.
        SupportsExpectedResult => "supports_expected_result",
        /// Contradicts an expected result.
        ContradictsExpectedResult => "contradicts_expected_result",
        /// Indicates a regression.
        IndicatesRegression => "indicates_regression",
        /// Confirms implementation occurred.
        ConfirmsImplementation => "confirms_implementation",
        /// Disputes that implementation occurred.
        DisputesImplementation => "disputes_implementation",
        /// Baseline measurement.
        IsBaseline => "is_baseline",
        /// Post-change measurement.
        IsPostChange => "is_post_change",
        /// Inconclusive evidence.
        IsInconclusive => "is_inconclusive",
        /// Superseded by other evidence.
        IsSuperseded => "is_superseded",
        /// Dismissed with a reason.
        IsDismissed => "is_dismissed"
    }
);

string_enum!(
    /// Conservative causality wording (RFC-023).
    CausalLanguage {
        /// Observed after implementation (default).
        ObservedAfterImplementation => "observed_after_implementation",
        /// Correlated with implementation.
        CorrelatedWithImplementation => "correlated_with_implementation",
        /// Consistent with expected mechanism.
        ConsistentWithExpectedMechanism => "consistent_with_expected_mechanism",
        /// Directly verified.
        DirectlyVerified => "directly_verified",
        /// Causally proven (only when evidence truly warrants).
        CausallyProven => "causally_proven"
    }
);

string_enum!(
    /// Typed regression category.
    RegressionType {
        /// Correctness regression.
        Correctness => "correctness",
        /// Reliability regression.
        Reliability => "reliability",
        /// Performance regression.
        Performance => "performance",
        /// Security regression.
        Security => "security",
        /// Cost regression.
        Cost => "cost",
        /// Maintainability regression.
        Maintainability => "maintainability",
        /// Developer-experience regression.
        DeveloperExperience => "developer_experience",
        /// Observability regression.
        Observability => "observability",
        /// Compatibility regression.
        Compatibility => "compatibility",
        /// Operational burden regression.
        OperationalBurden => "operational_burden",
        /// User-experience regression.
        UserExperience => "user_experience",
        /// Process throughput regression.
        ProcessThroughput => "process_throughput",
        /// Other regression.
        Other => "other"
    }
);

string_enum!(
    /// Severity of a regression or contradiction.
    MaterialitySeverity {
        /// Negligible impact.
        Negligible => "negligible",
        /// Minor impact.
        Minor => "minor",
        /// Moderate impact.
        Moderate => "moderate",
        /// Material impact.
        Material => "material",
        /// Critical impact.
        Critical => "critical"
    }
);

/// One measurable expected result seeded from a Proposal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExpectedResultSpec {
    /// Stable identifier within the Outcome.
    pub id: ObjectId,
    /// Human-readable description.
    pub description: String,
    /// Kind of result.
    pub kind: ExpectedResultKind,
    /// Metric or observable name.
    pub metric: Option<String>,
    /// Target or expected direction.
    pub target: Option<String>,
    /// Allowed tolerance description.
    pub tolerance: Option<String>,
    /// Whether a baseline is required.
    pub requires_baseline: bool,
    /// Relative importance weight (non-negative).
    pub weight: f64,
    /// Whether this expectation is required for overall success.
    pub required: bool,
    /// Verification method description.
    pub verification_method: Option<String>,
    /// Source text from Proposal success criteria or verification plan.
    pub source_text: String,
}

/// Observed result summary for an expected result or overall outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservedResultSummary {
    /// Related expected result when applicable.
    pub expected_result_id: Option<ObjectId>,
    /// Human-readable observation summary.
    pub summary: String,
    /// Optional numeric observed value.
    pub observed_value: Option<f64>,
    /// Optional baseline value.
    pub baseline_value: Option<f64>,
    /// Supporting evidence identifiers.
    pub evidence_ids: Vec<ObjectId>,
}

/// Per-expected-result assessment (RFC-023).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExpectedResultAssessment {
    /// Expected result being assessed.
    pub expected_result_id: ObjectId,
    /// Assessment kind.
    pub kind: ResultAssessmentKind,
    /// Explainable reason.
    pub reason: String,
    /// Confidence in this assessment.
    pub confidence: Confidence,
    /// Evidence references used.
    pub evidence_ids: Vec<ObjectId>,
    /// Whether baseline comparison was available.
    pub baseline_compared: bool,
    /// Contradictions noted for this result.
    pub contradictions: Vec<String>,
    /// Missing evidence descriptions.
    pub missing_evidence: Vec<String>,
}

/// Typed evidence link on a Measured Learning Outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutcomeEvidenceLink {
    /// Linked Engineering Object identifier.
    pub object_id: ObjectId,
    /// Relationship to the Outcome.
    pub relation: OutcomeEvidenceRelation,
    /// Optional related expected result.
    pub expected_result_id: Option<ObjectId>,
    /// Dismissal or annotation reason.
    pub reason: Option<String>,
    /// When the link was recorded.
    pub linked_at: DateTime<Utc>,
    /// Actor who linked the evidence.
    pub actor: String,
}

/// One regression finding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegressionRecord {
    /// Stable identifier.
    pub id: ObjectId,
    /// Regression type.
    pub regression_type: RegressionType,
    /// Severity / materiality.
    pub severity: MaterialitySeverity,
    /// Confidence.
    pub confidence: Confidence,
    /// Description.
    pub description: String,
    /// Baseline state summary.
    pub baseline: Option<String>,
    /// Observed state summary.
    pub observed: Option<String>,
    /// Supporting evidence.
    pub evidence_ids: Vec<ObjectId>,
    /// Affected component.
    pub affected_component: Option<String>,
    /// Related expected result when applicable.
    pub expected_result_id: Option<ObjectId>,
    /// Whether material for overall classification.
    pub material: bool,
    /// Whether a Proposal guardrail was violated.
    pub guardrail_violated: bool,
    /// Follow-up action.
    pub follow_up: Option<String>,
}

/// One contradiction finding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContradictionRecord {
    /// Stable identifier.
    pub id: ObjectId,
    /// Description of the contradiction.
    pub description: String,
    /// Severity.
    pub severity: MaterialitySeverity,
    /// Confidence.
    pub confidence: Confidence,
    /// Supporting evidence for each side.
    pub evidence_ids: Vec<ObjectId>,
    /// Related expected result when applicable.
    pub expected_result_id: Option<ObjectId>,
    /// Whether resolved.
    pub resolved: bool,
    /// Resolution notes.
    pub resolution: Option<String>,
}

/// Explicit verification receipt for a Measured Learning Outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutcomeVerification {
    /// Actor who verified (must be external/explicit).
    pub actor: String,
    /// Non-empty reason.
    pub reason: String,
    /// Verification timestamp.
    pub at: DateTime<Utc>,
    /// Whether readiness was overridden.
    pub override_readiness: bool,
    /// Override reason when readiness was false.
    pub override_reason: Option<String>,
}

/// Structured lesson extracted from an Outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LessonRecord {
    /// Stable identifier.
    pub id: ObjectId,
    /// What worked or failed.
    pub summary: String,
    /// Conditions under which the lesson applies.
    pub conditions: Vec<String>,
    /// Evidence strength description.
    pub evidence_strength: String,
    /// Related proposal category label.
    pub proposal_category: Option<String>,
    /// Applicability constraints.
    pub applicability: Vec<String>,
    /// Known exceptions.
    pub exceptions: Vec<String>,
}

/// One explained confidence component contribution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceComponent {
    /// Component name.
    pub name: String,
    /// Contribution in `[0.0, 1.0]`.
    pub value: f64,
    /// Human-readable explanation.
    pub explanation: String,
}

/// One explicit confidence penalty.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfidencePenalty {
    /// Penalty name.
    pub name: String,
    /// Penalty amount in `[0.0, 1.0]`.
    pub amount: f64,
    /// Human-readable explanation.
    pub explanation: String,
}

/// Decomposed confidence for a Measured Learning Outcome (RFC-023).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ConfidenceBreakdown {
    /// Final confidence after penalties.
    pub final_confidence: Confidence,
    /// Explained components.
    pub components: Vec<ConfidenceComponent>,
    /// Explicit penalties.
    pub penalties: Vec<ConfidencePenalty>,
    /// What would increase confidence.
    pub improvement_hints: Vec<String>,
}

impl ConfidenceBreakdown {
    /// Neutral pending breakdown.
    pub fn pending() -> Self {
        Self {
            final_confidence: Confidence::none(),
            components: Vec::new(),
            penalties: Vec::new(),
            improvement_hints: vec![
                "Link baseline evidence".into(),
                "Link post-change evidence".into(),
                "Confirm implementation with typed references".into(),
            ],
        }
    }
}

/// Evaluation readiness and report stored on a revision (RFC-023).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutcomeEvaluationReport {
    /// Whether verification readiness is true.
    pub verification_ready: bool,
    /// Ordered evaluation step explanations.
    pub steps: Vec<String>,
    /// Method version.
    pub method: String,
    /// Evaluation timestamp.
    pub evaluated_at: DateTime<Utc>,
}

/// Preserved Measured Learning Outcome lifecycle transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeasuredOutcomeTransition {
    /// Previous status.
    pub from: MeasuredOutcomeStatus,
    /// New status.
    pub to: MeasuredOutcomeStatus,
    /// Explicit actor.
    pub actor: String,
    /// Non-empty reason.
    pub reason: String,
    /// Transition timestamp.
    pub at: DateTime<Utc>,
}

/// Durable, auditable conclusion about the measured effect of an implemented Proposal.
///
/// Distinct from the v0.1 Recommendation disposition [`super::LearningOutcome`].
/// Stored under `learning_outcomes/`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeasuredLearningOutcome {
    /// Snapshot identifier.
    pub id: ObjectId,
    /// Stable lineage across revisions.
    pub lineage_id: ObjectId,
    /// One-based revision number.
    pub revision_number: u32,
    /// Prior immutable snapshot.
    pub parent_outcome_id: Option<ObjectId>,
    /// Successor when explicitly superseded.
    pub superseding_outcome_id: Option<ObjectId>,
    /// Owning Investigation.
    pub investigation_id: InvestigationId,
    /// Exact Proposal snapshot.
    pub proposal_id: ObjectId,
    /// Proposal lineage.
    pub proposal_lineage_id: ObjectId,
    /// Proposal revision at link time.
    pub proposal_revision_number: u32,
    /// Linked Implementation Record snapshot.
    pub implementation_record_id: ObjectId,
    /// Implementation lineage.
    pub implementation_lineage_id: ObjectId,
    /// Lifecycle status.
    pub status: MeasuredOutcomeStatus,
    /// Outcome classification.
    pub classification: OutcomeClassification,
    /// Overall confidence.
    pub confidence: Confidence,
    /// Decomposed confidence breakdown.
    pub confidence_breakdown: ConfidenceBreakdown,
    /// Expected results seeded from Proposal.
    pub expected_results: Vec<ExpectedResultSpec>,
    /// Observed result summaries.
    pub observed_results: Vec<ObservedResultSummary>,
    /// Per-expected-result assessments.
    pub assessments: Vec<ExpectedResultAssessment>,
    /// Typed regressions.
    pub regressions: Vec<RegressionRecord>,
    /// Typed contradictions.
    pub contradictions: Vec<ContradictionRecord>,
    /// Remaining uncertainty.
    pub unresolved_questions: Vec<String>,
    /// Typed evidence relationships.
    pub evidence_links: Vec<OutcomeEvidenceLink>,
    /// Explicit verification receipt when verified.
    pub verification: Option<OutcomeVerification>,
    /// Evaluation report when evaluated.
    pub evaluation_report: Option<OutcomeEvaluationReport>,
    /// Conservative causality wording.
    pub causal_language: CausalLanguage,
    /// Structured lessons.
    pub lessons: Vec<LessonRecord>,
    /// Recommended follow-up actions.
    pub recommended_follow_up: Vec<String>,
    /// Whether patterns may consume this Outcome.
    pub historical_learning_eligible: bool,
    /// Preserved lifecycle transitions.
    pub transitions: Vec<MeasuredOutcomeTransition>,
    /// Provenance.
    pub provenance: Provenance,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Snapshot update timestamp.
    pub updated_at: DateTime<Utc>,
}

impl MeasuredLearningOutcome {
    /// Create a Draft Measured Learning Outcome linked to a Proposal and Implementation.
    #[allow(clippy::too_many_arguments)]
    pub fn draft(
        investigation_id: InvestigationId,
        proposal_id: ObjectId,
        proposal_lineage_id: ObjectId,
        proposal_revision_number: u32,
        implementation_record_id: ObjectId,
        implementation_lineage_id: ObjectId,
        expected_results: Vec<ExpectedResultSpec>,
        provenance: Provenance,
    ) -> RivoraResult<Self> {
        if expected_results.is_empty() {
            return Err(RivoraError::validation(
                "measured learning outcome requires at least one expected result",
            ));
        }
        let id = ObjectId::new();
        let now = Utc::now();
        Ok(Self {
            id,
            lineage_id: id,
            revision_number: 1,
            parent_outcome_id: None,
            superseding_outcome_id: None,
            investigation_id,
            proposal_id,
            proposal_lineage_id,
            proposal_revision_number,
            implementation_record_id,
            implementation_lineage_id,
            status: MeasuredOutcomeStatus::Draft,
            classification: OutcomeClassification::Pending,
            confidence: Confidence::none(),
            confidence_breakdown: ConfidenceBreakdown::pending(),
            expected_results,
            observed_results: Vec::new(),
            assessments: Vec::new(),
            regressions: Vec::new(),
            contradictions: Vec::new(),
            unresolved_questions: Vec::new(),
            evidence_links: Vec::new(),
            verification: None,
            evaluation_report: None,
            causal_language: CausalLanguage::ObservedAfterImplementation,
            lessons: Vec::new(),
            recommended_follow_up: Vec::new(),
            historical_learning_eligible: false,
            transitions: Vec::new(),
            provenance,
            created_at: now,
            updated_at: now,
        })
    }

    /// Create an immutable content revision.
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
                "measured outcome revision actor and reason are required",
            ));
        }
        if matches!(
            self.status,
            MeasuredOutcomeStatus::Verified
                | MeasuredOutcomeStatus::Archived
                | MeasuredOutcomeStatus::Withdrawn
                | MeasuredOutcomeStatus::Superseded
        ) {
            return Err(RivoraError::validation(format!(
                "cannot revise measured outcome in terminal status {}; create a superseding outcome",
                self.status.as_str()
            )));
        }
        let mut next = self.clone();
        next.id = ObjectId::new();
        next.parent_outcome_id = Some(self.id);
        next.revision_number = self.revision_number.saturating_add(1);
        next.updated_at = at;
        next.verification = None;
        next.provenance = Provenance::now(actor.trim(), "runtime")
            .with_capability("revise_measured_learning_outcome")
            .with_evidence(vec![self.id]);
        next.unresolved_questions
            .push(format!("Revision reason: {}", reason.trim()));
        Ok(next)
    }

    /// Create an immutable successor for a lifecycle transition.
    pub fn transitioned(
        &self,
        to: MeasuredOutcomeStatus,
        actor: impl Into<String>,
        reason: impl Into<String>,
        at: DateTime<Utc>,
    ) -> RivoraResult<Self> {
        let actor = actor.into();
        let reason = reason.into();
        if actor.trim().is_empty() || reason.trim().is_empty() {
            return Err(RivoraError::validation(
                "measured outcome transition actor and reason are required",
            ));
        }
        if to == MeasuredOutcomeStatus::Verified {
            return Err(RivoraError::validation(
                "use verify_measured_learning_outcome for verification transitions",
            ));
        }
        if !valid_measured_outcome_transition(self.status, to) {
            return Err(RivoraError::validation(format!(
                "invalid measured outcome transition: {} -> {}",
                self.status.as_str(),
                to.as_str()
            )));
        }
        let mut next = self.clone();
        next.id = ObjectId::new();
        next.parent_outcome_id = Some(self.id);
        next.revision_number = self.revision_number.saturating_add(1);
        next.status = to;
        next.updated_at = at;
        next.transitions.push(MeasuredOutcomeTransition {
            from: self.status,
            to,
            actor: actor.trim().into(),
            reason: reason.trim().into(),
            at,
        });
        next.provenance = Provenance::now(actor.trim(), "runtime")
            .with_capability("transition_measured_learning_outcome")
            .with_evidence(vec![self.id]);
        Ok(next)
    }

    /// Create an immutable Verified successor with explicit authority.
    ///
    /// Requires status `Evaluated`, non-empty actor and reason. Automation must
    /// not auto-verify solely because confidence is high.
    pub fn verified(
        &self,
        actor: impl Into<String>,
        reason: impl Into<String>,
        at: DateTime<Utc>,
        override_readiness: bool,
        override_reason: Option<String>,
    ) -> RivoraResult<Self> {
        let actor = actor.into();
        let reason = reason.into();
        if actor.trim().is_empty() || reason.trim().is_empty() {
            return Err(RivoraError::validation(
                "verification actor and reason are required",
            ));
        }
        if self.status != MeasuredOutcomeStatus::Evaluated {
            return Err(RivoraError::validation(format!(
                "verification requires Evaluated status, found {}",
                self.status.as_str()
            )));
        }
        let ready = self
            .evaluation_report
            .as_ref()
            .map(|r| r.verification_ready)
            .unwrap_or(false);
        if !ready && !override_readiness {
            return Err(RivoraError::precondition(
                "outcome is not verification-ready; supply override_readiness with reason",
            ));
        }
        if override_readiness {
            let or = override_reason.as_deref().unwrap_or("").trim();
            if or.is_empty() {
                return Err(RivoraError::validation(
                    "override_reason is required when override_readiness is true",
                ));
            }
        }
        let mut next = self.clone();
        next.id = ObjectId::new();
        next.parent_outcome_id = Some(self.id);
        next.revision_number = self.revision_number.saturating_add(1);
        next.status = MeasuredOutcomeStatus::Verified;
        next.updated_at = at;
        next.historical_learning_eligible = true;
        next.verification = Some(OutcomeVerification {
            actor: actor.trim().into(),
            reason: reason.trim().into(),
            at,
            override_readiness,
            override_reason: override_reason
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
        });
        next.transitions.push(MeasuredOutcomeTransition {
            from: self.status,
            to: MeasuredOutcomeStatus::Verified,
            actor: actor.trim().into(),
            reason: reason.trim().into(),
            at,
        });
        next.provenance = Provenance::now(actor.trim(), "runtime")
            .with_capability("verify_measured_learning_outcome")
            .with_evidence(vec![self.id]);
        Ok(next)
    }
}

fn valid_measured_outcome_transition(
    from: MeasuredOutcomeStatus,
    to: MeasuredOutcomeStatus,
) -> bool {
    use MeasuredOutcomeStatus::*;
    match from {
        Draft => matches!(to, EvidenceCollection | Withdrawn | Superseded),
        EvidenceCollection => matches!(to, UnderEvaluation | Withdrawn | Superseded),
        UnderEvaluation => matches!(to, Evaluated | Withdrawn | Superseded),
        Evaluated => matches!(to, Archived | Superseded),
        Verified => matches!(to, Archived | Superseded),
        Archived | Withdrawn | Superseded => false,
    }
}

/// One isolated corrupted Measured Learning Outcome diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeasuredOutcomeStorageDiagnostic {
    /// Corrupted record path.
    pub path: String,
    /// Serialization error message.
    pub error: String,
}

/// Valid Measured Learning Outcomes plus visible corruption diagnostics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MeasuredOutcomeListing {
    /// Valid records, deterministically ordered.
    pub outcomes: Vec<MeasuredLearningOutcome>,
    /// Corrupted sibling records that were isolated.
    pub diagnostics: Vec<MeasuredOutcomeStorageDiagnostic>,
}

// ---------------------------------------------------------------------------
// Learning Pattern (RFC-024)
// ---------------------------------------------------------------------------

string_enum!(
    /// Lifecycle status of a Learning Pattern.
    PatternStatus {
        /// Newly derived with limited support.
        Emerging => "emerging",
        /// Multiple supporting Outcomes.
        Supported => "supported",
        /// Contradictory later evidence.
        Contested => "contested",
        /// Retired with reason; not deleted.
        Retired => "retired"
    }
);

/// Aggregate derived from verified Measured Learning Outcomes.
///
/// A derived summary — never a replacement for source Outcomes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LearningPattern {
    /// Stable pattern identifier.
    pub id: ObjectId,
    /// One-based revision number.
    pub revision_number: u32,
    /// Prior revision when revised.
    pub parent_pattern_id: Option<ObjectId>,
    /// Human-readable title.
    pub title: String,
    /// Normalized signature / grouping key.
    pub signature: String,
    /// Proposal category label used for grouping.
    pub proposal_category: Option<String>,
    /// Scope constraints (components, environments).
    pub scope: Vec<String>,
    /// Supporting verified Outcome snapshot ids.
    pub supporting_outcome_ids: Vec<ObjectId>,
    /// Contradicting verified Outcome snapshot ids.
    pub contradicting_outcome_ids: Vec<ObjectId>,
    /// Mixed classification Outcome snapshot ids.
    pub mixed_outcome_ids: Vec<ObjectId>,
    /// Counts by classification (lineage-deduped).
    pub classification_counts: PatternClassificationCounts,
    /// Pattern confidence.
    pub confidence: Confidence,
    /// Applicability constraints.
    pub applicability_constraints: Vec<String>,
    /// Known exceptions.
    pub known_exceptions: Vec<String>,
    /// First observed timestamp.
    pub first_observed: DateTime<Utc>,
    /// Last observed timestamp.
    pub last_observed: DateTime<Utc>,
    /// Status.
    pub status: PatternStatus,
    /// Retirement reason when retired.
    pub retirement_reason: Option<String>,
    /// Provenance.
    pub provenance: Provenance,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Classification counts for a Learning Pattern (one count per Outcome lineage).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PatternClassificationCounts {
    /// Successful count.
    pub successful: u32,
    /// Partially successful count.
    pub partially_successful: u32,
    /// Mixed count.
    pub mixed: u32,
    /// Unsuccessful count.
    pub unsuccessful: u32,
    /// Regressed count.
    pub regressed: u32,
    /// Inconclusive count.
    pub inconclusive: u32,
    /// Not implemented count.
    pub not_implemented: u32,
    /// Invalidated count.
    pub invalidated: u32,
}

impl LearningPattern {
    /// Construct a newly derived Learning Pattern.
    pub fn derived(
        title: impl Into<String>,
        signature: impl Into<String>,
        proposal_category: Option<String>,
        provenance: Provenance,
    ) -> RivoraResult<Self> {
        let title = title.into();
        let signature = signature.into();
        if title.trim().is_empty() || signature.trim().is_empty() {
            return Err(RivoraError::validation(
                "learning pattern title and signature are required",
            ));
        }
        let now = Utc::now();
        Ok(Self {
            id: ObjectId::new(),
            revision_number: 1,
            parent_pattern_id: None,
            title: title.trim().into(),
            signature: signature.trim().into(),
            proposal_category,
            scope: Vec::new(),
            supporting_outcome_ids: Vec::new(),
            contradicting_outcome_ids: Vec::new(),
            mixed_outcome_ids: Vec::new(),
            classification_counts: PatternClassificationCounts::default(),
            confidence: Confidence::none(),
            applicability_constraints: Vec::new(),
            known_exceptions: Vec::new(),
            first_observed: now,
            last_observed: now,
            status: PatternStatus::Emerging,
            retirement_reason: None,
            provenance,
            created_at: now,
            updated_at: now,
        })
    }

    /// Create an immutable retired successor.
    pub fn retired(
        &self,
        actor: impl Into<String>,
        reason: impl Into<String>,
        at: DateTime<Utc>,
    ) -> RivoraResult<Self> {
        let actor = actor.into();
        let reason = reason.into();
        if actor.trim().is_empty() || reason.trim().is_empty() {
            return Err(RivoraError::validation(
                "pattern retirement actor and reason are required",
            ));
        }
        if self.status == PatternStatus::Retired {
            return Err(RivoraError::validation("pattern is already retired"));
        }
        let mut next = self.clone();
        next.id = ObjectId::new();
        next.parent_pattern_id = Some(self.id);
        next.revision_number = self.revision_number.saturating_add(1);
        next.status = PatternStatus::Retired;
        next.retirement_reason = Some(reason.trim().into());
        next.updated_at = at;
        next.provenance = Provenance::now(actor.trim(), "runtime")
            .with_capability("retire_learning_pattern")
            .with_evidence(vec![self.id]);
        Ok(next)
    }
}

/// Explainable historical influence of Patterns on a Proposal ranking signal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoricalInfluenceExplanation {
    /// Proposal snapshot considered.
    pub proposal_id: ObjectId,
    /// Patterns considered.
    pub patterns_considered: Vec<HistoricalPatternInfluence>,
    /// Aggregate advisory influence score contribution (not a correctness proof).
    pub aggregate_influence: f64,
    /// Whether current evidence overrode history.
    pub current_evidence_overrode: bool,
    /// Human-readable summary.
    pub explanation: String,
}

/// One Pattern's advisory influence on a Proposal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoricalPatternInfluence {
    /// Pattern identifier.
    pub pattern_id: ObjectId,
    /// Relevance reason.
    pub relevance: String,
    /// Influence magnitude (signed advisory).
    pub magnitude: f64,
    /// Direction label.
    pub direction: String,
    /// Supporting Outcome ids.
    pub supporting_outcome_ids: Vec<ObjectId>,
    /// Contradicting Outcome ids.
    pub contradicting_outcome_ids: Vec<ObjectId>,
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn implementation_report_and_transition_immutable() {
        let inv = InvestigationId::new();
        let proposal_id = ObjectId::new();
        let record = ImplementationRecord::reported(
            inv,
            proposal_id,
            proposal_id,
            1,
            "engineer",
            ImplementationSource::HumanDeclared,
            "Deployed config guard",
            Provenance::now("engineer", "test"),
        )
        .unwrap();
        assert_eq!(record.status, ImplementationStatus::Reported);
        assert_eq!(record.revision_number, 1);
        assert_eq!(record.lineage_id, record.id);

        let next = record
            .transitioned(
                ImplementationStatus::EvidenceLinked,
                "engineer",
                "linked commit",
                Utc::now(),
            )
            .unwrap();
        assert_eq!(record.status, ImplementationStatus::Reported);
        assert_ne!(next.id, record.id);
        assert_eq!(next.lineage_id, record.lineage_id);
        assert_eq!(next.parent_record_id, Some(record.id));
        assert_eq!(next.revision_number, 2);
        assert_eq!(next.status, ImplementationStatus::EvidenceLinked);
    }

    #[test]
    fn implementation_rejects_invalid_transition() {
        let inv = InvestigationId::new();
        let proposal_id = ObjectId::new();
        let record = ImplementationRecord::reported(
            inv,
            proposal_id,
            proposal_id,
            1,
            "engineer",
            ImplementationSource::GitCommit,
            "shipped",
            Provenance::now("engineer", "test"),
        )
        .unwrap();
        let err = record
            .transitioned(
                ImplementationStatus::Superseded,
                "",
                "no actor",
                Utc::now(),
            )
            .unwrap_err();
        assert!(matches!(err, RivoraError::Validation(_)));
    }

    #[test]
    fn measured_outcome_verify_requires_evaluated_and_actor() {
        let inv = InvestigationId::new();
        let proposal_id = ObjectId::new();
        let impl_id = ObjectId::new();
        let expected = ExpectedResultSpec {
            id: ObjectId::new(),
            description: "latency improves".into(),
            kind: ExpectedResultKind::DirectionalImprovement,
            metric: Some("p99".into()),
            target: Some("lower".into()),
            tolerance: None,
            requires_baseline: true,
            weight: 1.0,
            required: true,
            verification_method: None,
            source_text: "latency improves".into(),
        };
        let draft = MeasuredLearningOutcome::draft(
            inv,
            proposal_id,
            proposal_id,
            1,
            impl_id,
            impl_id,
            vec![expected],
            Provenance::now("runtime", "test"),
        )
        .unwrap();
        let err = draft
            .verified("reviewer", "looks good", Utc::now(), false, None)
            .unwrap_err();
        assert!(matches!(err, RivoraError::Validation(_)));
    }

    #[test]
    fn learning_pattern_retirement_preserves_history() {
        let pattern = LearningPattern::derived(
            "Config guards succeed with baseline",
            "configuration:config_guard",
            Some("configuration".into()),
            Provenance::now("runtime", "test"),
        )
        .unwrap();
        let retired = pattern
            .retired("reviewer", "contradicted by later outcomes", Utc::now())
            .unwrap();
        assert_eq!(pattern.status, PatternStatus::Emerging);
        assert_eq!(retired.status, PatternStatus::Retired);
        assert_ne!(retired.id, pattern.id);
        assert_eq!(retired.parent_pattern_id, Some(pattern.id));
        assert!(retired.retirement_reason.is_some());
    }

    #[test]
    fn serialization_round_trips() {
        let inv = InvestigationId::new();
        let proposal_id = ObjectId::new();
        let mut record = ImplementationRecord::reported(
            inv,
            proposal_id,
            proposal_id,
            1,
            "engineer",
            ImplementationSource::PullRequest,
            "merged PR",
            Provenance::now("engineer", "test"),
        )
        .unwrap();
        record.references.push(ImplementationReference::PullRequest {
            reference: "https://example.com/pr/1".into(),
        });
        let json = serde_json::to_string(&record).unwrap();
        let decoded: ImplementationRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, record);
    }
}
