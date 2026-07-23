//! Capability Engineering Loop contract (RFC-028 / v0.7).
//!
//! Connectors provide normalized external facts. Capabilities express
//! engineering intent and typed lifecycle contributions. The Runtime
//! transforms those contributions into durable engineering knowledge.
//!
//! ```text
//! Memory → Evaluation → Verification → Improvement → Learning
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{InvestigationId, ObjectId, ObservationKind, Provenance};
use crate::error::{RivoraError, RivoraResult};

/// Schema version for lifecycle contribution and run records.
pub const ENGINEERING_LOOP_SCHEMA_VERSION: u32 = 1;

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
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

// ---------------------------------------------------------------------------
// Stages and participation
// ---------------------------------------------------------------------------

string_enum!(
    /// One stage of the Capability Engineering Loop.
    EngineeringLoopStage {
        /// Durable fact retention.
        Memory => "memory",
        /// Explainable assessment.
        Evaluation => "evaluation",
        /// Independent validation.
        Verification => "verification",
        /// Bounded improvement context / proposal generation.
        Improvement => "improvement",
        /// Measured outcome and learning.
        Learning => "learning"
    }
);

string_enum!(
    /// How a Capability participates in one Engineering Loop stage.
    ///
    /// Distinct from absence: every stage has an explicit declaration.
    LifecycleParticipation {
        /// Stage applies and the Capability can contribute typed context.
        Supported => "supported",
        /// Stage does not conceptually apply to this Capability.
        NotApplicable => "not_applicable",
        /// Stage applies but is not implemented yet.
        Unsupported => "unsupported",
        /// Stage is intentionally deferred (e.g. awaiting measured evidence).
        Deferred => "deferred"
    }
);

string_enum!(
    /// Durable status of one stage within a lifecycle run.
    LifecycleStageStatus {
        /// Not yet started.
        Pending => "pending",
        /// Currently processing.
        Running => "running",
        /// Finished successfully.
        Completed => "completed",
        /// Failed with an explicit error.
        Failed => "failed",
        /// Explicitly skipped (e.g. prerequisite missing, declared skip).
        Skipped => "skipped",
        /// Deferred pending future evidence or work.
        Deferred => "deferred",
        /// Not applicable for this Capability.
        NotApplicable => "not_applicable",
        /// Declared unsupported.
        Unsupported => "unsupported",
        /// Blocked by policy, missing prerequisite, or upstream failure.
        Blocked => "blocked"
    }
);

string_enum!(
    /// Overall status of a Capability Engineering Loop run.
    LifecycleRunStatus {
        /// Created, not yet started.
        Pending => "pending",
        /// At least one stage is in progress.
        Running => "running",
        /// All supported stages completed (deferred stages allowed).
        Completed => "completed",
        /// At least one required stage failed.
        Failed => "failed",
        /// Partial progress with remaining work possible.
        Partial => "partial",
        /// Blocked before completing supported stages.
        Blocked => "blocked"
    }
);

/// Declared participation of a Capability across all Engineering Loop stages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineeringLoopParticipation {
    /// Memory stage.
    pub memory: LifecycleParticipation,
    /// Evaluation stage.
    pub evaluation: LifecycleParticipation,
    /// Verification stage.
    pub verification: LifecycleParticipation,
    /// Improvement stage.
    pub improvement: LifecycleParticipation,
    /// Learning stage.
    pub learning: LifecycleParticipation,
}

impl Default for EngineeringLoopParticipation {
    fn default() -> Self {
        // Safe default for legacy descriptors: explicit deferred, not silent support.
        Self {
            memory: LifecycleParticipation::Deferred,
            evaluation: LifecycleParticipation::Deferred,
            verification: LifecycleParticipation::Deferred,
            improvement: LifecycleParticipation::Deferred,
            learning: LifecycleParticipation::Deferred,
        }
    }
}

impl EngineeringLoopParticipation {
    /// Participation for a bounded execution capability (v0.7 vertical slice).
    ///
    /// Memory / Evaluation / Verification are supported. Improvement and
    /// Learning remain deferred until measured outcomes exist.
    pub fn execution_capability_default() -> Self {
        Self {
            memory: LifecycleParticipation::Supported,
            evaluation: LifecycleParticipation::Supported,
            verification: LifecycleParticipation::Supported,
            improvement: LifecycleParticipation::Deferred,
            learning: LifecycleParticipation::Deferred,
        }
    }

    /// Participation for a given stage.
    pub fn for_stage(&self, stage: EngineeringLoopStage) -> LifecycleParticipation {
        match stage {
            EngineeringLoopStage::Memory => self.memory,
            EngineeringLoopStage::Evaluation => self.evaluation,
            EngineeringLoopStage::Verification => self.verification,
            EngineeringLoopStage::Improvement => self.improvement,
            EngineeringLoopStage::Learning => self.learning,
        }
    }

    /// Ordered stages.
    pub fn stages() -> [EngineeringLoopStage; 5] {
        [
            EngineeringLoopStage::Memory,
            EngineeringLoopStage::Evaluation,
            EngineeringLoopStage::Verification,
            EngineeringLoopStage::Improvement,
            EngineeringLoopStage::Learning,
        ]
    }
}

// ---------------------------------------------------------------------------
// Contribution identity and stage wrappers
// ---------------------------------------------------------------------------

/// Shared identity and provenance for Capability lifecycle contributions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContributionIdentity {
    /// Capability id.
    pub capability_id: String,
    /// Stable invocation key (typically attempt id or synthetic invocation id).
    pub invocation_id: String,
    /// Primary Investigation.
    pub investigation_id: InvestigationId,
    /// Source Observation ids.
    #[serde(default)]
    pub observation_ids: Vec<ObjectId>,
    /// Related engineering object ids.
    #[serde(default)]
    pub engineering_object_ids: Vec<ObjectId>,
    /// Execution Plan id when applicable.
    pub plan_id: Option<ObjectId>,
    /// Execution Attempt id when applicable.
    pub attempt_id: Option<ObjectId>,
    /// Receipt ids when applicable.
    #[serde(default)]
    pub receipt_ids: Vec<ObjectId>,
    /// Proposal id when applicable.
    pub proposal_id: Option<ObjectId>,
    /// Correlation id for distributed traces.
    pub correlation_id: Option<String>,
    /// Causation id (parent event).
    pub causation_id: Option<String>,
    /// Actor (human or system).
    pub actor: String,
    /// Target environment.
    pub environment: Option<String>,
    /// Idempotency key for this contribution set.
    pub idempotency_key: String,
    /// Contribution timestamp.
    pub timestamp: DateTime<Utc>,
    /// Schema version.
    pub schema_version: u32,
    /// Evidence object references.
    #[serde(default)]
    pub evidence_refs: Vec<ObjectId>,
    /// Sanitized metadata (never secrets).
    #[serde(default)]
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

impl ContributionIdentity {
    /// Construct a contribution identity with required fields.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        capability_id: impl Into<String>,
        invocation_id: impl Into<String>,
        investigation_id: InvestigationId,
        actor: impl Into<String>,
        idempotency_key: impl Into<String>,
    ) -> Self {
        Self {
            capability_id: capability_id.into(),
            invocation_id: invocation_id.into(),
            investigation_id,
            observation_ids: Vec::new(),
            engineering_object_ids: Vec::new(),
            plan_id: None,
            attempt_id: None,
            receipt_ids: Vec::new(),
            proposal_id: None,
            correlation_id: None,
            causation_id: None,
            actor: actor.into(),
            environment: None,
            idempotency_key: idempotency_key.into(),
            timestamp: Utc::now(),
            schema_version: ENGINEERING_LOOP_SCHEMA_VERSION,
            evidence_refs: Vec::new(),
            metadata: serde_json::Map::new(),
        }
    }
}

/// Explicit stage contribution — never collapses absence into `None`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "participation", rename_all = "snake_case")]
pub enum StageContribution<T> {
    /// Typed contribution for a supported stage.
    Supported {
        /// Contribution payload.
        value: T,
    },
    /// Stage does not apply.
    NotApplicable {
        /// Human-readable reason.
        reason: String,
    },
    /// Stage applies but is not implemented.
    Unsupported {
        /// Human-readable reason.
        reason: String,
    },
    /// Stage intentionally deferred.
    Deferred {
        /// Human-readable reason.
        reason: String,
    },
}

impl<T> StageContribution<T> {
    /// Participation implied by this contribution.
    pub fn participation(&self) -> LifecycleParticipation {
        match self {
            Self::Supported { .. } => LifecycleParticipation::Supported,
            Self::NotApplicable { .. } => LifecycleParticipation::NotApplicable,
            Self::Unsupported { .. } => LifecycleParticipation::Unsupported,
            Self::Deferred { .. } => LifecycleParticipation::Deferred,
        }
    }

    /// Borrow supported value when present.
    pub fn as_supported(&self) -> Option<&T> {
        match self {
            Self::Supported { value } => Some(value),
            _ => None,
        }
    }

    /// Reason when not supported.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Supported { .. } => None,
            Self::NotApplicable { reason }
            | Self::Unsupported { reason }
            | Self::Deferred { reason } => Some(reason.as_str()),
        }
    }
}

/// What durable engineering fact should be remembered.
///
/// Capabilities never write Memory directly; the Runtime applies this.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryContribution {
    /// Factual summary to remember.
    pub summary: String,
    /// Optional Observation id to attach (or Runtime synthesizes a system observation).
    pub observation_id: Option<ObjectId>,
    /// Confidence in the fact.
    pub confidence: f64,
    /// Evidence object ids.
    #[serde(default)]
    pub evidence_ids: Vec<ObjectId>,
}

/// Request for evaluation against expectation and evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationContributionRequest {
    /// What should be evaluated.
    pub subject: String,
    /// Expectation or criteria.
    pub expectation: String,
    /// Why evaluation is needed.
    pub rationale: String,
    /// Supporting evidence ids.
    #[serde(default)]
    pub evidence_ids: Vec<ObjectId>,
    /// Suggested severity label (informational for Runtime).
    pub suggested_severity: Option<String>,
}

/// Independent verification strategy request.
///
/// Must not treat Capability execution result as proof of success.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerificationContributionRequest {
    /// Verification strategy summary.
    pub strategy: String,
    /// Required evidence descriptions.
    #[serde(default)]
    pub required_evidence: Vec<String>,
    /// Related execution verification id when available (reference only).
    pub execution_verification_id: Option<ObjectId>,
    /// Whether independent observation is still required.
    pub requires_independent_observation: bool,
    /// Evidence object ids already available.
    #[serde(default)]
    pub evidence_ids: Vec<ObjectId>,
}

/// Bounded context for Improvement Proposal generation.
///
/// Never auto-applies a change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImprovementContributionContext {
    /// Context summary for proposal generation.
    pub summary: String,
    /// Suggested focus areas.
    #[serde(default)]
    pub focus_areas: Vec<String>,
    /// Whether Runtime should attempt proposal generation now.
    pub generate_proposal: bool,
    /// Evidence ids.
    #[serde(default)]
    pub evidence_ids: Vec<ObjectId>,
}

/// Context for measured outcomes and learning.
///
/// Never infers success without measured evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LearningContributionContext {
    /// Learning context summary.
    pub summary: String,
    /// Linked measured outcome id when present.
    pub measured_outcome_id: Option<ObjectId>,
    /// Linked implementation record id when present.
    pub implementation_record_id: Option<ObjectId>,
    /// Whether measured evidence is available.
    pub measured_evidence_available: bool,
    /// Evidence ids.
    #[serde(default)]
    pub evidence_ids: Vec<ObjectId>,
}

/// Full typed Capability lifecycle contributions for one invocation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityLifecycleContributions {
    /// Shared identity and provenance.
    pub identity: ContributionIdentity,
    /// Memory contribution.
    pub memory: StageContribution<MemoryContribution>,
    /// Evaluation request.
    pub evaluation: StageContribution<EvaluationContributionRequest>,
    /// Verification request.
    pub verification: StageContribution<VerificationContributionRequest>,
    /// Improvement context.
    pub improvement: StageContribution<ImprovementContributionContext>,
    /// Learning context.
    pub learning: StageContribution<LearningContributionContext>,
}

impl CapabilityLifecycleContributions {
    /// Contribution for a stage.
    pub fn for_stage(&self, stage: EngineeringLoopStage) -> StageContributionRef<'_> {
        match stage {
            EngineeringLoopStage::Memory => StageContributionRef::Memory(&self.memory),
            EngineeringLoopStage::Evaluation => StageContributionRef::Evaluation(&self.evaluation),
            EngineeringLoopStage::Verification => {
                StageContributionRef::Verification(&self.verification)
            }
            EngineeringLoopStage::Improvement => {
                StageContributionRef::Improvement(&self.improvement)
            }
            EngineeringLoopStage::Learning => StageContributionRef::Learning(&self.learning),
        }
    }

    /// Validate contributions against declared participation.
    ///
    /// Rejects Supported contributions for stages not declared Supported.
    pub fn validate_against(
        &self,
        participation: &EngineeringLoopParticipation,
    ) -> RivoraResult<()> {
        for stage in EngineeringLoopParticipation::stages() {
            let declared = participation.for_stage(stage);
            let actual = match stage {
                EngineeringLoopStage::Memory => self.memory.participation(),
                EngineeringLoopStage::Evaluation => self.evaluation.participation(),
                EngineeringLoopStage::Verification => self.verification.participation(),
                EngineeringLoopStage::Improvement => self.improvement.participation(),
                EngineeringLoopStage::Learning => self.learning.participation(),
            };
            if actual == LifecycleParticipation::Supported
                && declared != LifecycleParticipation::Supported
            {
                return Err(RivoraError::validation(format!(
                    "capability contributed Supported payload for {} but declared {}",
                    stage.as_str(),
                    declared.as_str()
                )));
            }
        }
        if self.identity.schema_version == 0 {
            return Err(RivoraError::validation(
                "lifecycle contribution schema_version must be >= 1",
            ));
        }
        if self.identity.idempotency_key.trim().is_empty() {
            return Err(RivoraError::validation(
                "lifecycle contribution idempotency_key is required",
            ));
        }
        if self.identity.capability_id.trim().is_empty() {
            return Err(RivoraError::validation(
                "lifecycle contribution capability_id is required",
            ));
        }
        Ok(())
    }

    /// Build empty contributions matching declared participation (no Supported payloads).
    pub fn from_participation(
        identity: ContributionIdentity,
        participation: &EngineeringLoopParticipation,
    ) -> Self {
        fn map_part(p: LifecycleParticipation, stage: &str) -> StageContribution<()> {
            match p {
                LifecycleParticipation::Supported => StageContribution::Deferred {
                    reason: format!("{stage} declared supported but no contribution produced"),
                },
                LifecycleParticipation::NotApplicable => StageContribution::NotApplicable {
                    reason: format!("{stage} does not apply"),
                },
                LifecycleParticipation::Unsupported => StageContribution::Unsupported {
                    reason: format!("{stage} not implemented"),
                },
                LifecycleParticipation::Deferred => StageContribution::Deferred {
                    reason: format!("{stage} deferred"),
                },
            }
        }

        // Map unit contributions into typed shells via remapping helpers.
        let mem = map_part(participation.memory, "memory");
        let eva = map_part(participation.evaluation, "evaluation");
        let ver = map_part(participation.verification, "verification");
        let imp = map_part(participation.improvement, "improvement");
        let lea = map_part(participation.learning, "learning");

        Self {
            identity,
            memory: remap_unit(mem),
            evaluation: remap_unit(eva),
            verification: remap_unit(ver),
            improvement: remap_unit(imp),
            learning: remap_unit(lea),
        }
    }
}

fn remap_unit<T>(c: StageContribution<()>) -> StageContribution<T> {
    match c {
        StageContribution::Supported { .. } => StageContribution::Deferred {
            reason: "supported stage missing typed contribution".into(),
        },
        StageContribution::NotApplicable { reason } => StageContribution::NotApplicable { reason },
        StageContribution::Unsupported { reason } => StageContribution::Unsupported { reason },
        StageContribution::Deferred { reason } => StageContribution::Deferred { reason },
    }
}

/// Type-erased view of a stage contribution for inspection.
#[derive(Debug)]
pub enum StageContributionRef<'a> {
    /// Memory.
    Memory(&'a StageContribution<MemoryContribution>),
    /// Evaluation.
    Evaluation(&'a StageContribution<EvaluationContributionRequest>),
    /// Verification.
    Verification(&'a StageContribution<VerificationContributionRequest>),
    /// Improvement.
    Improvement(&'a StageContribution<ImprovementContributionContext>),
    /// Learning.
    Learning(&'a StageContribution<LearningContributionContext>),
}

impl StageContributionRef<'_> {
    /// Participation.
    pub fn participation(&self) -> LifecycleParticipation {
        match self {
            Self::Memory(c) => c.participation(),
            Self::Evaluation(c) => c.participation(),
            Self::Verification(c) => c.participation(),
            Self::Improvement(c) => c.participation(),
            Self::Learning(c) => c.participation(),
        }
    }

    /// Reason when not supported.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Memory(c) => c.reason(),
            Self::Evaluation(c) => c.reason(),
            Self::Verification(c) => c.reason(),
            Self::Improvement(c) => c.reason(),
            Self::Learning(c) => c.reason(),
        }
    }
}

// ---------------------------------------------------------------------------
// Durable lifecycle run
// ---------------------------------------------------------------------------

/// Status record for one stage in a lifecycle run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LifecycleStageRecord {
    /// Stage.
    pub stage: EngineeringLoopStage,
    /// Declared participation.
    pub participation: LifecycleParticipation,
    /// Runtime status for this stage.
    pub status: LifecycleStageStatus,
    /// Human-readable detail.
    pub detail: Option<String>,
    /// Artifact object ids produced (Memory, Evaluation, etc.).
    #[serde(default)]
    pub artifact_ids: Vec<ObjectId>,
    /// When the stage finished (if terminal for this run).
    pub finished_at: Option<DateTime<Utc>>,
    /// Error message when failed.
    pub error: Option<String>,
}

/// Durable Capability Engineering Loop run for one invocation.
///
/// Append-only snapshots: each update creates a new record with the same
/// lineage id and a higher revision number (same pattern as ExecutionAttempt).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityLifecycleRun {
    /// Snapshot id (unique per revision).
    pub id: ObjectId,
    /// Stable lineage id for this invocation's loop run.
    pub lineage_id: ObjectId,
    /// Revision number starting at 1.
    pub revision_number: u32,
    /// Primary Investigation.
    pub investigation_id: InvestigationId,
    /// Capability id.
    pub capability_id: String,
    /// Invocation id (attempt id or synthetic).
    pub invocation_id: String,
    /// Plan id when from execution.
    pub plan_id: Option<ObjectId>,
    /// Attempt id when from execution.
    pub attempt_id: Option<ObjectId>,
    /// Source observation ids.
    #[serde(default)]
    pub observation_ids: Vec<ObjectId>,
    /// Overall run status.
    pub status: LifecycleRunStatus,
    /// Per-stage records in fixed order.
    pub stages: Vec<LifecycleStageRecord>,
    /// Idempotency key (suppresses duplicate full runs).
    pub idempotency_key: String,
    /// Schema version.
    pub schema_version: u32,
    /// Provenance.
    pub provenance: Provenance,
    /// Created at (this snapshot).
    pub created_at: DateTime<Utc>,
    /// Human-readable explanation of the current state.
    pub explanation: String,
}

impl CapabilityLifecycleRun {
    /// Create the initial pending run.
    #[allow(clippy::too_many_arguments)]
    pub fn pending(
        investigation_id: InvestigationId,
        capability_id: impl Into<String>,
        invocation_id: impl Into<String>,
        participation: &EngineeringLoopParticipation,
        plan_id: Option<ObjectId>,
        attempt_id: Option<ObjectId>,
        observation_ids: Vec<ObjectId>,
        idempotency_key: impl Into<String>,
        provenance: Provenance,
    ) -> Self {
        let lineage_id = ObjectId::new();
        let stages = EngineeringLoopParticipation::stages()
            .into_iter()
            .map(|stage| {
                let participation = participation.for_stage(stage);
                let status = match participation {
                    LifecycleParticipation::Supported => LifecycleStageStatus::Pending,
                    LifecycleParticipation::NotApplicable => LifecycleStageStatus::NotApplicable,
                    LifecycleParticipation::Unsupported => LifecycleStageStatus::Unsupported,
                    LifecycleParticipation::Deferred => LifecycleStageStatus::Deferred,
                };
                LifecycleStageRecord {
                    stage,
                    participation,
                    status,
                    detail: None,
                    artifact_ids: Vec::new(),
                    finished_at: None,
                    error: None,
                }
            })
            .collect();
        Self {
            id: ObjectId::new(),
            lineage_id,
            revision_number: 1,
            investigation_id,
            capability_id: capability_id.into(),
            invocation_id: invocation_id.into(),
            plan_id,
            attempt_id,
            observation_ids,
            status: LifecycleRunStatus::Pending,
            stages,
            idempotency_key: idempotency_key.into(),
            schema_version: ENGINEERING_LOOP_SCHEMA_VERSION,
            provenance,
            created_at: Utc::now(),
            explanation: "Capability Engineering Loop created; stages pending Runtime processing"
                .into(),
        }
    }

    /// Create a new revision snapshot with updated stages/status.
    pub fn revised(
        &self,
        stages: Vec<LifecycleStageRecord>,
        status: LifecycleRunStatus,
        explanation: impl Into<String>,
        provenance: Provenance,
    ) -> Self {
        Self {
            id: ObjectId::new(),
            lineage_id: self.lineage_id,
            revision_number: self.revision_number + 1,
            investigation_id: self.investigation_id,
            capability_id: self.capability_id.clone(),
            invocation_id: self.invocation_id.clone(),
            plan_id: self.plan_id,
            attempt_id: self.attempt_id,
            observation_ids: self.observation_ids.clone(),
            status,
            stages,
            idempotency_key: self.idempotency_key.clone(),
            schema_version: self.schema_version,
            provenance,
            created_at: Utc::now(),
            explanation: explanation.into(),
        }
    }

    /// Find stage record.
    pub fn stage(&self, stage: EngineeringLoopStage) -> Option<&LifecycleStageRecord> {
        self.stages.iter().find(|s| s.stage == stage)
    }
}

/// Listing with corruption isolation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CapabilityLifecycleRunListing {
    /// Valid runs (all revisions).
    pub runs: Vec<CapabilityLifecycleRun>,
    /// Diagnostics for corrupt files.
    pub diagnostics: Vec<LifecycleStorageDiagnostic>,
}

/// Storage diagnostic for lifecycle records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleStorageDiagnostic {
    /// File path.
    pub path: String,
    /// Error message.
    pub error: String,
}

/// End-to-end lineage from Observation / execution through the Engineering Loop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityLifecycleTrace {
    /// Investigation.
    pub investigation_id: InvestigationId,
    /// Capability id.
    pub capability_id: String,
    /// Invocation id.
    pub invocation_id: String,
    /// Lifecycle run lineage id.
    pub run_lineage_id: Option<ObjectId>,
    /// Latest run snapshot id.
    pub run_id: Option<ObjectId>,
    /// Overall status.
    pub status: Option<LifecycleRunStatus>,
    /// Plan id.
    pub plan_id: Option<ObjectId>,
    /// Attempt id.
    pub attempt_id: Option<ObjectId>,
    /// Observation ids.
    pub observation_ids: Vec<ObjectId>,
    /// Stage summaries.
    pub stages: Vec<LifecycleStageRecord>,
    /// Artifact references by stage name.
    pub artifacts: serde_json::Map<String, serde_json::Value>,
    /// Human explanation.
    pub explanation: String,
}

// ---------------------------------------------------------------------------
// Connector → Capability routing
// ---------------------------------------------------------------------------

/// Canonical input type identifier (provider-independent).
///
/// Routing prefers these stable type ids over human-readable names.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CanonicalInputType(pub String);

impl CanonicalInputType {
    /// Create from a stable type id.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Borrow the type id.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Map an ObservationKind to a canonical input type id.
    pub fn from_observation_kind(kind: &ObservationKind) -> Self {
        Self(kind.as_str().to_string())
    }
}

/// Deterministic routing decision for one Observation (or correlated set).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityRoutingDecision {
    /// Source observation ids considered.
    pub observation_ids: Vec<ObjectId>,
    /// Canonical input types used for matching.
    pub input_types: Vec<String>,
    /// Ordered capability matches (deterministic by capability_id).
    pub matches: Vec<CapabilityRouteMatch>,
    /// Explicit unsupported (zero compatible capabilities).
    pub unsupported: bool,
    /// Explicit ambiguous (multiple matches without a primary selector).
    pub ambiguous: bool,
    /// Version incompatibility notes.
    #[serde(default)]
    pub version_incompatibilities: Vec<String>,
    /// Missing prerequisite notes.
    #[serde(default)]
    pub missing_prerequisites: Vec<String>,
    /// Human-readable reasons.
    #[serde(default)]
    pub reasons: Vec<String>,
    /// Schema version.
    pub schema_version: u32,
}

/// One capability match from routing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityRouteMatch {
    /// Capability id.
    pub capability_id: String,
    /// Capability version.
    pub version: String,
    /// Matched input type ids.
    pub matched_input_types: Vec<String>,
    /// Deterministic rank (lower is earlier; capability_id tie-break).
    pub rank: u32,
    /// Match reason.
    pub reason: String,
}

/// Map observation kinds / type ids to accepted input types for built-in capabilities.
pub fn default_accepted_input_types(capability_id: &str) -> Vec<String> {
    match capability_id {
        "mock.record" => vec![
            "execution_result".into(),
            "mutation_request".into(),
            "event".into(),
        ],
        "github_actions.workflow_dispatch" => vec![
            "workflow_run".into(),
            "workflow_dispatch_request".into(),
            "check_result".into(),
        ],
        "github.issue.comment" | "github.issue.label" | "github.issue.create" => {
            vec!["issue".into(), "pull_request".into()]
        }
        "github.pull_request.create_draft" => {
            vec!["pull_request".into(), "commit".into(), "git_status".into()]
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn participation_default_is_explicit_deferred() {
        let p = EngineeringLoopParticipation::default();
        assert_eq!(p.memory, LifecycleParticipation::Deferred);
        assert_eq!(p.learning, LifecycleParticipation::Deferred);
    }

    #[test]
    fn rejects_supported_contribution_when_undeclared() {
        let identity = ContributionIdentity::new(
            "mock.record",
            "inv-1",
            InvestigationId::new(),
            "tester",
            "key-1",
        );
        let contributions = CapabilityLifecycleContributions {
            identity,
            memory: StageContribution::Supported {
                value: MemoryContribution {
                    summary: "fact".into(),
                    observation_id: None,
                    confidence: 1.0,
                    evidence_ids: vec![],
                },
            },
            evaluation: StageContribution::Deferred {
                reason: "n/a".into(),
            },
            verification: StageContribution::Deferred {
                reason: "n/a".into(),
            },
            improvement: StageContribution::Deferred {
                reason: "n/a".into(),
            },
            learning: StageContribution::Deferred {
                reason: "n/a".into(),
            },
        };
        let participation = EngineeringLoopParticipation::default(); // all deferred
        let err = contributions.validate_against(&participation).unwrap_err();
        assert!(err.to_string().contains("contributed Supported"));
    }

    #[test]
    fn accepts_matching_supported_contribution() {
        let identity = ContributionIdentity::new(
            "mock.record",
            "inv-1",
            InvestigationId::new(),
            "tester",
            "key-1",
        );
        let contributions = CapabilityLifecycleContributions {
            identity,
            memory: StageContribution::Supported {
                value: MemoryContribution {
                    summary: "fact".into(),
                    observation_id: None,
                    confidence: 1.0,
                    evidence_ids: vec![],
                },
            },
            evaluation: StageContribution::Deferred {
                reason: "awaiting".into(),
            },
            verification: StageContribution::Deferred {
                reason: "awaiting".into(),
            },
            improvement: StageContribution::Deferred {
                reason: "awaiting".into(),
            },
            learning: StageContribution::Deferred {
                reason: "awaiting".into(),
            },
        };
        let participation = EngineeringLoopParticipation {
            memory: LifecycleParticipation::Supported,
            ..EngineeringLoopParticipation::default()
        };
        contributions.validate_against(&participation).unwrap();
    }

    #[test]
    fn lifecycle_run_pending_sets_explicit_stage_status() {
        let participation = EngineeringLoopParticipation::execution_capability_default();
        let run = CapabilityLifecycleRun::pending(
            InvestigationId::new(),
            "mock.record",
            "attempt-1",
            &participation,
            None,
            None,
            vec![],
            "idem-1",
            Provenance::now("tester", "runtime"),
        );
        assert_eq!(run.status, LifecycleRunStatus::Pending);
        assert_eq!(
            run.stage(EngineeringLoopStage::Memory).unwrap().status,
            LifecycleStageStatus::Pending
        );
        assert_eq!(
            run.stage(EngineeringLoopStage::Learning).unwrap().status,
            LifecycleStageStatus::Deferred
        );
    }
}
