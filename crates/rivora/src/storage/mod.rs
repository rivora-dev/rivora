//! Local durable persistence for Engineering Objects.

mod local;

pub use local::LocalStore;

use std::path::Path;

use crate::domain::{
    AssistedWorkflow, CapabilityLifecycleRun, CapabilityLifecycleRunListing, DeploymentReadiness,
    EngineeringReport, Evaluation, ExecutionApproval, ExecutionApprovalListing, ExecutionAttempt,
    ExecutionAttemptListing, ExecutionPlan, ExecutionPlanListing, ExecutionReceipt,
    ExecutionReceiptListing, ExecutionVerification, ExecutionVerificationListing, Hypothesis,
    ImplementationListing, ImplementationRecord, ImprovementProposal, Investigation,
    InvestigationId, InvestigationRelationship, KnowledgeObject, LearningOutcome, LearningPattern,
    MeasuredLearningOutcome, MeasuredOutcomeListing, MemoryRecord, ObjectId, Observation,
    ProposalArtifact, ProposalArtifactListing, ProposalListing, RecalledContext, Recommendation,
    RiskForecast, RootCauseGuidance, StoreHealthReport, TimelineEntry, VerificationReceipt,
    VerificationSuggestion,
};
use crate::error::RivoraResult;

/// Persistence interface for Rivora Runtime storage.
pub trait Store: Send + Sync {
    /// Persist a new or updated Investigation.
    fn save_investigation(&self, investigation: &Investigation) -> RivoraResult<()>;

    /// Load an Investigation by id.
    fn load_investigation(&self, id: &InvestigationId) -> RivoraResult<Investigation>;

    /// List all Investigation identifiers.
    fn list_investigations(&self) -> RivoraResult<Vec<InvestigationId>>;

    /// Append an Observation (fails if id already exists).
    fn append_observation(&self, observation: &Observation) -> RivoraResult<()>;

    /// Load all Observations for an Investigation.
    fn list_observations(&self, id: &InvestigationId) -> RivoraResult<Vec<Observation>>;

    /// Find Observation by idempotency key within an Investigation.
    fn find_observation_by_idempotency(
        &self,
        investigation_id: &InvestigationId,
        key: &str,
    ) -> RivoraResult<Option<Observation>>;

    /// Append a Memory record (append-only; never update).
    fn append_memory(&self, record: &MemoryRecord) -> RivoraResult<()>;

    /// Load Memory for an Investigation, chronological by recorded_at.
    fn list_memory(&self, id: &InvestigationId) -> RivoraResult<Vec<MemoryRecord>>;

    /// Replace derived Knowledge for an Investigation (Knowledge is refreshable).
    fn replace_knowledge(
        &self,
        investigation_id: &InvestigationId,
        objects: &[KnowledgeObject],
    ) -> RivoraResult<()>;

    /// List Knowledge for an Investigation.
    fn list_knowledge(&self, id: &InvestigationId) -> RivoraResult<Vec<KnowledgeObject>>;

    /// Append an Evaluation.
    fn append_evaluation(&self, evaluation: &Evaluation) -> RivoraResult<()>;

    /// List Evaluations for an Investigation.
    fn list_evaluations(&self, id: &InvestigationId) -> RivoraResult<Vec<Evaluation>>;

    /// Append a Verification Receipt.
    fn append_verification(&self, receipt: &VerificationReceipt) -> RivoraResult<()>;

    /// List Verification Receipts for an Investigation.
    fn list_verifications(&self, id: &InvestigationId) -> RivoraResult<Vec<VerificationReceipt>>;

    /// Append a Recommendation.
    fn append_recommendation(&self, recommendation: &Recommendation) -> RivoraResult<()>;

    /// List Recommendations for an Investigation.
    fn list_recommendations(&self, id: &InvestigationId) -> RivoraResult<Vec<Recommendation>>;

    /// Update a Recommendation (status only for learning disposition).
    fn save_recommendation(&self, recommendation: &Recommendation) -> RivoraResult<()>;

    /// Load a Recommendation by id.
    fn load_recommendation(
        &self,
        investigation_id: &InvestigationId,
        id: &ObjectId,
    ) -> RivoraResult<Recommendation>;

    /// Append a Learning Outcome.
    fn append_learning(&self, outcome: &LearningOutcome) -> RivoraResult<()>;

    /// List Learning Outcomes for an Investigation.
    fn list_learning(&self, id: &InvestigationId) -> RivoraResult<Vec<LearningOutcome>>;

    /// Build a chronological timeline from Memory.
    fn timeline(&self, id: &InvestigationId) -> RivoraResult<Vec<TimelineEntry>>;

    /// Persist a new or updated Investigation Relationship (upsert).
    fn save_relationship(&self, relationship: &InvestigationRelationship) -> RivoraResult<()>;

    /// Load an Investigation Relationship by id.
    fn load_relationship(&self, id: &ObjectId) -> RivoraResult<InvestigationRelationship>;

    /// List all Investigation Relationships, ordered deterministically.
    fn list_relationships(&self) -> RivoraResult<Vec<InvestigationRelationship>>;

    /// Delete an Investigation Relationship by id.
    fn delete_relationship(&self, id: &ObjectId) -> RivoraResult<()>;

    /// Persist a new or updated Recalled Context record (upsert; RFC-017).
    fn save_recalled_context(&self, context: &RecalledContext) -> RivoraResult<()>;

    /// Load a Recalled Context record by id.
    fn load_recalled_context(
        &self,
        investigation_id: &InvestigationId,
        id: &ObjectId,
    ) -> RivoraResult<RecalledContext>;

    /// List Recalled Context records for an Investigation.
    fn list_recalled_context(&self, id: &InvestigationId) -> RivoraResult<Vec<RecalledContext>>;

    /// Persist a new or updated Assisted Workflow (upsert; RFC-018).
    fn save_workflow(&self, workflow: &AssistedWorkflow) -> RivoraResult<()>;

    /// Load an Assisted Workflow by id.
    fn load_workflow(
        &self,
        investigation_id: &InvestigationId,
        id: &ObjectId,
    ) -> RivoraResult<AssistedWorkflow>;

    /// List Assisted Workflows for an Investigation.
    fn list_workflows(&self, id: &InvestigationId) -> RivoraResult<Vec<AssistedWorkflow>>;

    /// Append a Hypothesis (RFC-019).
    fn append_hypothesis(&self, hypothesis: &Hypothesis) -> RivoraResult<()>;

    /// List Hypotheses for an Investigation.
    fn list_hypotheses(&self, id: &InvestigationId) -> RivoraResult<Vec<Hypothesis>>;

    /// Append a Verification Suggestion.
    fn append_verification_suggestion(
        &self,
        suggestion: &VerificationSuggestion,
    ) -> RivoraResult<()>;

    /// List Verification Suggestions for an Investigation.
    fn list_verification_suggestions(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<Vec<VerificationSuggestion>>;

    /// Append a Deployment Readiness assessment.
    fn append_deployment_readiness(&self, readiness: &DeploymentReadiness) -> RivoraResult<()>;

    /// List Deployment Readiness assessments for an Investigation.
    fn list_deployment_readiness(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<Vec<DeploymentReadiness>>;

    /// Append a Risk Forecast.
    fn append_risk_forecast(&self, forecast: &RiskForecast) -> RivoraResult<()>;

    /// List Risk Forecasts for an Investigation.
    fn list_risk_forecasts(&self, id: &InvestigationId) -> RivoraResult<Vec<RiskForecast>>;

    /// Append Root-Cause Guidance.
    fn append_root_cause_guidance(&self, guidance: &RootCauseGuidance) -> RivoraResult<()>;

    /// List Root-Cause Guidance for an Investigation.
    fn list_root_cause_guidance(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<Vec<RootCauseGuidance>>;

    /// Append an Engineering Report.
    fn append_engineering_report(&self, report: &EngineeringReport) -> RivoraResult<()>;

    /// List Engineering Reports for an Investigation.
    fn list_engineering_reports(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<Vec<EngineeringReport>>;

    /// Append one immutable Improvement Proposal snapshot.
    fn append_proposal(&self, proposal: &ImprovementProposal) -> RivoraResult<()>;

    /// Load one Proposal snapshot owned by an Investigation.
    fn load_proposal(
        &self,
        investigation_id: &InvestigationId,
        id: &ObjectId,
    ) -> RivoraResult<ImprovementProposal>;

    /// List valid Proposal snapshots and isolated corruption diagnostics.
    fn list_proposals(&self, id: &InvestigationId) -> RivoraResult<ProposalListing>;

    /// List all immutable snapshots in one Proposal lineage.
    fn list_proposal_revisions(
        &self,
        id: &InvestigationId,
        lineage_id: &ObjectId,
    ) -> RivoraResult<ProposalListing>;

    /// Append one durable Proposal artifact snapshot.
    fn append_proposal_artifact(&self, artifact: &ProposalArtifact) -> RivoraResult<()>;

    /// List durable Proposal artifacts for an Investigation.
    fn list_proposal_artifacts(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<ProposalArtifactListing>;

    /// Append one immutable Implementation Record snapshot.
    fn append_implementation_record(&self, record: &ImplementationRecord) -> RivoraResult<()>;

    /// Load one Implementation Record snapshot owned by an Investigation.
    fn load_implementation_record(
        &self,
        investigation_id: &InvestigationId,
        id: &ObjectId,
    ) -> RivoraResult<ImplementationRecord>;

    /// List valid Implementation Records and isolated corruption diagnostics.
    fn list_implementation_records(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<ImplementationListing>;

    /// List all immutable snapshots in one Implementation Record lineage.
    fn list_implementation_revisions(
        &self,
        id: &InvestigationId,
        lineage_id: &ObjectId,
    ) -> RivoraResult<ImplementationListing>;

    /// Append one immutable Measured Learning Outcome snapshot.
    fn append_measured_learning_outcome(
        &self,
        outcome: &MeasuredLearningOutcome,
    ) -> RivoraResult<()>;

    /// Load one Measured Learning Outcome snapshot owned by an Investigation.
    fn load_measured_learning_outcome(
        &self,
        investigation_id: &InvestigationId,
        id: &ObjectId,
    ) -> RivoraResult<MeasuredLearningOutcome>;

    /// List valid Measured Learning Outcomes and isolated corruption diagnostics.
    fn list_measured_learning_outcomes(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<MeasuredOutcomeListing>;

    /// List all immutable snapshots in one Measured Learning Outcome lineage.
    fn list_measured_outcome_revisions(
        &self,
        id: &InvestigationId,
        lineage_id: &ObjectId,
    ) -> RivoraResult<MeasuredOutcomeListing>;

    /// Append one Learning Pattern snapshot (store-root learning/patterns).
    fn append_learning_pattern(&self, pattern: &LearningPattern) -> RivoraResult<()>;

    /// Load one Learning Pattern by id.
    fn load_learning_pattern(&self, id: &ObjectId) -> RivoraResult<LearningPattern>;

    /// List all Learning Patterns at the store root.
    fn list_learning_patterns(&self) -> RivoraResult<Vec<LearningPattern>>;

    /// Append one immutable Execution Plan snapshot.
    fn append_execution_plan(&self, plan: &ExecutionPlan) -> RivoraResult<()>;

    /// Load one Execution Plan snapshot.
    fn load_execution_plan(
        &self,
        investigation_id: &InvestigationId,
        id: &ObjectId,
    ) -> RivoraResult<ExecutionPlan>;

    /// List valid Execution Plans and diagnostics.
    fn list_execution_plans(&self, id: &InvestigationId) -> RivoraResult<ExecutionPlanListing>;

    /// List Execution Plan revisions for a lineage.
    fn list_execution_plan_revisions(
        &self,
        id: &InvestigationId,
        lineage_id: &ObjectId,
    ) -> RivoraResult<ExecutionPlanListing>;

    /// Persist an Execution Approval (create or update consumption/invalidation flags).
    fn save_execution_approval(&self, approval: &ExecutionApproval) -> RivoraResult<()>;

    /// Load one Execution Approval.
    fn load_execution_approval(
        &self,
        investigation_id: &InvestigationId,
        id: &ObjectId,
    ) -> RivoraResult<ExecutionApproval>;

    /// List Execution Approvals for an Investigation.
    fn list_execution_approvals(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<ExecutionApprovalListing>;

    /// Append one Execution Attempt.
    fn append_execution_attempt(&self, attempt: &ExecutionAttempt) -> RivoraResult<()>;

    /// Atomically reserve a Started Attempt. Returns false when already reserved.
    fn try_reserve_execution_attempt(&self, attempt: &ExecutionAttempt) -> RivoraResult<bool>;

    /// Load one Execution Attempt.
    fn load_execution_attempt(
        &self,
        investigation_id: &InvestigationId,
        id: &ObjectId,
    ) -> RivoraResult<ExecutionAttempt>;

    /// List Execution Attempts.
    fn list_execution_attempts(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<ExecutionAttemptListing>;

    /// Append one Execution Receipt.
    fn append_execution_receipt(&self, receipt: &ExecutionReceipt) -> RivoraResult<()>;

    /// Load one Execution Receipt.
    fn load_execution_receipt(
        &self,
        investigation_id: &InvestigationId,
        id: &ObjectId,
    ) -> RivoraResult<ExecutionReceipt>;

    /// List Execution Receipts.
    fn list_execution_receipts(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<ExecutionReceiptListing>;

    /// Append one Execution Verification.
    fn append_execution_verification(
        &self,
        verification: &ExecutionVerification,
    ) -> RivoraResult<()>;

    /// Load one Execution Verification.
    fn load_execution_verification(
        &self,
        investigation_id: &InvestigationId,
        id: &ObjectId,
    ) -> RivoraResult<ExecutionVerification>;

    /// List Execution Verifications.
    fn list_execution_verifications(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<ExecutionVerificationListing>;

    /// Atomically consume one-time approval authority.
    fn try_consume_execution_approval(&self, approval: &ExecutionApproval) -> RivoraResult<bool>;

    /// Append one immutable Capability Engineering Loop run snapshot (v0.7).
    fn append_lifecycle_run(&self, run: &CapabilityLifecycleRun) -> RivoraResult<()>;

    /// Load one Capability Engineering Loop run snapshot.
    fn load_lifecycle_run(
        &self,
        investigation_id: &InvestigationId,
        id: &ObjectId,
    ) -> RivoraResult<CapabilityLifecycleRun>;

    /// List Capability Engineering Loop runs for an Investigation (with isolation).
    fn list_lifecycle_runs(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<CapabilityLifecycleRunListing>;

    /// Find a lifecycle run by idempotency key (returns latest revision if any).
    fn find_lifecycle_run_by_idempotency(
        &self,
        investigation_id: &InvestigationId,
        key: &str,
    ) -> RivoraResult<Option<CapabilityLifecycleRun>>;

    /// Local store health / integrity report (v0.9).
    fn health_report(&self) -> RivoraResult<StoreHealthReport>;

    /// Sanitized diagnostic export as JSON (v0.9).
    fn diagnostic_export(&self) -> RivoraResult<serde_json::Value>;

    /// Copy store contents to a backup directory (excludes live lock file).
    fn backup_to(&self, dest: &Path) -> RivoraResult<()>;

    /// Rebuild derived observation idempotency indexes from canonical records.
    fn rebuild_observation_indexes(&self) -> RivoraResult<u64>;
}
