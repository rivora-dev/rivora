//! Capability layer — intent-oriented orchestration (RFC-011).
//!
//! Capabilities coordinate Runtime subsystems. They do not implement
//! engineering reasoning themselves. Workspace and CLI share this service.

use std::sync::Arc;

use crate::domain::{
    AssistedWorkflow, CapabilityLifecycleRun, CapabilityLifecycleRunListing,
    CapabilityLifecycleTrace, CapabilityRoutingDecision, CompositeCapabilityDefinition,
    DeploymentReadiness, DryRunResult, EngineeringReport, Evaluation, ExecutionApproval,
    ExecutionAttempt, ExecutionAttemptListing, ExecutionCapabilityDescriptor, ExecutionPlan,
    ExecutionPlanListing, ExecutionPolicyDecision, ExecutionReceiptListing, ExecutionTrace,
    ExecutionVerification, HistoricalInfluenceExplanation, Hypothesis, ImplementationListing,
    ImplementationRecord, ImprovementProposal, Investigation, InvestigationId,
    InvestigationRelationship, InvestigationSummary, KnowledgeObject, LearningOutcome,
    LearningPattern, MeasuredLearningOutcome, MeasuredOutcomeListing, MeasuredOutcomeStatus,
    MemoryRecord, ObjectId, Observation, ObservationKind, OutcomeDisposition,
    PrioritizedRecommendation, ProposalArtifact, ProposalArtifactListing, ProposalComparison,
    ProposalFeedbackCategory, ProposalListing, ProposalStatus, ProposalTrace,
    ProposalTransitionAuthority, ProposalVerificationPlan, RecalledContext, Recommendation,
    RetrySafety, RiskForecast, RootCauseGuidance, TimelineEntry, VerificationReceipt,
    VerificationSuggestion,
};
use crate::error::RivoraResult;
use crate::runtime::context::{DetectedPattern, HistoricalTrend};
use crate::runtime::execution::{CreateExecutionPlanRequest, ReviseExecutionPlanRequest};
use crate::runtime::graph::{RelatedInvestigation, RelationshipExplanation};
use crate::runtime::learning::RecordOutcomeRequest;
use crate::runtime::observation::IngestObservationRequest;
use crate::runtime::outcome::{
    CollectOutcomeEvidenceRequest, MeasuredOutcomeTrace, RecordImplementationRequest,
    ReviseImplementationRequest, ReviseMeasuredOutcomeRequest,
};
use crate::runtime::proposal::{
    CreateProposalRequest, ProposalPortfolioFilter, RefineProposalRequest,
};
use crate::runtime::search::{
    OutcomeFilter, PriorOutcome, RecalledEvidence, SearchQuery, SearchResult,
};
use crate::runtime::Runtime;

/// Shared Capability service used by every interface.
#[derive(Clone)]
pub struct CapabilityService {
    runtime: Arc<Runtime>,
}

impl CapabilityService {
    /// Create a Capability service over a Runtime.
    pub fn new(runtime: Arc<Runtime>) -> Self {
        Self { runtime }
    }

    /// Borrow the underlying Runtime (for tests verifying shared use).
    pub fn runtime(&self) -> &Arc<Runtime> {
        &self.runtime
    }

    /// Create Investigation.
    pub fn create_investigation(
        &self,
        title: impl Into<String>,
        description: Option<String>,
        actor: impl Into<String>,
    ) -> RivoraResult<Investigation> {
        self.runtime.create_investigation(title, description, actor)
    }

    /// Open Investigation.
    pub fn open_investigation(&self, id: InvestigationId) -> RivoraResult<Investigation> {
        self.runtime.open_investigation(id)
    }

    /// List Investigations.
    pub fn list_investigations(&self) -> RivoraResult<Vec<InvestigationId>> {
        self.runtime.list_investigations()
    }

    /// Ingest Observation.
    #[allow(clippy::too_many_arguments)]
    pub fn ingest_observation(
        &self,
        investigation_id: InvestigationId,
        kind: ObservationKind,
        summary: impl Into<String>,
        payload: serde_json::Value,
        source: impl Into<String>,
        observed_at: chrono::DateTime<chrono::Utc>,
        idempotency_key: Option<String>,
        actor: impl Into<String>,
    ) -> RivoraResult<(Observation, MemoryRecord, bool)> {
        let result = self.runtime.ingest_observation(IngestObservationRequest {
            investigation_id,
            kind,
            summary: summary.into(),
            payload,
            source: source.into(),
            observed_at,
            idempotency_key,
            actor: actor.into(),
        })?;
        Ok((result.observation, result.memory, result.idempotent_replay))
    }

    /// Recall Investigation Memory.
    pub fn recall_memory(&self, id: InvestigationId) -> RivoraResult<Vec<MemoryRecord>> {
        self.runtime.recall_memory(id)
    }

    /// Generate Timeline.
    pub fn generate_timeline(&self, id: InvestigationId) -> RivoraResult<Vec<TimelineEntry>> {
        self.runtime.generate_timeline(id)
    }

    /// Derive Knowledge.
    pub fn derive_knowledge(
        &self,
        id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<Vec<KnowledgeObject>> {
        self.runtime.derive_knowledge(id, actor)
    }

    /// List Knowledge.
    pub fn list_knowledge(&self, id: InvestigationId) -> RivoraResult<Vec<KnowledgeObject>> {
        self.runtime.list_knowledge(id)
    }

    /// Evaluate Investigation.
    pub fn evaluate_investigation(
        &self,
        id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<Vec<Evaluation>> {
        self.runtime.evaluate_investigation(id, actor)
    }

    /// List Evaluations.
    pub fn list_evaluations(&self, id: InvestigationId) -> RivoraResult<Vec<Evaluation>> {
        self.runtime.list_evaluations(id)
    }

    /// Verify Conclusion.
    pub fn verify_conclusion(
        &self,
        id: InvestigationId,
        evaluation_id: Option<ObjectId>,
        actor: impl Into<String>,
    ) -> RivoraResult<VerificationReceipt> {
        self.runtime.verify_conclusion(id, evaluation_id, actor)
    }

    /// Verify all Evaluations.
    pub fn verify_all(
        &self,
        id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<Vec<VerificationReceipt>> {
        self.runtime.verify_all(id, actor)
    }

    /// List Verifications.
    pub fn list_verifications(
        &self,
        id: InvestigationId,
    ) -> RivoraResult<Vec<VerificationReceipt>> {
        self.runtime.list_verifications(id)
    }

    /// Generate Recommendation.
    pub fn generate_recommendation(
        &self,
        id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<Vec<Recommendation>> {
        self.runtime.generate_recommendation(id, actor)
    }

    /// List Recommendations.
    pub fn list_recommendations(&self, id: InvestigationId) -> RivoraResult<Vec<Recommendation>> {
        self.runtime.list_recommendations(id)
    }

    /// Record Outcome (Learning).
    pub fn record_outcome(
        &self,
        investigation_id: InvestigationId,
        recommendation_id: Option<ObjectId>,
        disposition: OutcomeDisposition,
        notes: impl Into<String>,
        impact: Option<String>,
        actor: impl Into<String>,
    ) -> RivoraResult<LearningOutcome> {
        self.runtime.record_outcome(RecordOutcomeRequest {
            investigation_id,
            recommendation_id,
            disposition,
            notes: notes.into(),
            impact,
            actor: actor.into(),
        })
    }

    /// List Learning Outcomes.
    pub fn list_learning(&self, id: InvestigationId) -> RivoraResult<Vec<LearningOutcome>> {
        self.runtime.list_learning(id)
    }

    /// Link Investigations (explicit human-created relationship).
    pub fn link_investigations(
        &self,
        source: InvestigationId,
        target: InvestigationId,
        reason: Option<String>,
        actor: impl Into<String>,
    ) -> RivoraResult<InvestigationRelationship> {
        self.runtime
            .link_investigations(source, target, reason, actor)
    }

    /// Unlink Investigations (explicit links only).
    pub fn unlink_investigation(
        &self,
        relationship_id: ObjectId,
        actor: impl Into<String>,
    ) -> RivoraResult<()> {
        self.runtime.unlink_investigation(relationship_id, actor)
    }

    /// List Relationships for an Investigation.
    pub fn list_relationships(
        &self,
        id: InvestigationId,
    ) -> RivoraResult<Vec<InvestigationRelationship>> {
        self.runtime.list_relationships(id)
    }

    /// List Related Investigations (dismissed relationships excluded).
    pub fn list_related_investigations(
        &self,
        id: InvestigationId,
    ) -> RivoraResult<Vec<RelatedInvestigation>> {
        self.runtime.list_related_investigations(id)
    }

    /// Explain Investigation Relationship.
    pub fn explain_relationship(
        &self,
        relationship_id: ObjectId,
    ) -> RivoraResult<RelationshipExplanation> {
        self.runtime.explain_relationship(relationship_id)
    }

    /// Refresh Investigation Relationships (deterministic derivation).
    pub fn refresh_relationships(
        &self,
        id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<Vec<InvestigationRelationship>> {
        self.runtime.refresh_relationships(id, actor)
    }

    /// Confirm Investigation Relationship.
    pub fn confirm_relationship(
        &self,
        relationship_id: ObjectId,
        actor: impl Into<String>,
    ) -> RivoraResult<InvestigationRelationship> {
        self.runtime.confirm_relationship(relationship_id, actor)
    }

    /// Dismiss Investigation Relationship.
    pub fn dismiss_relationship(
        &self,
        relationship_id: ObjectId,
        actor: impl Into<String>,
    ) -> RivoraResult<InvestigationRelationship> {
        self.runtime.dismiss_relationship(relationship_id, actor)
    }

    /// Search Investigations (RFC-016).
    pub fn search_investigations(&self, query: SearchQuery) -> RivoraResult<Vec<SearchResult>> {
        self.runtime.search_investigations(query)
    }

    /// Find Similar Investigations (RFC-016).
    pub fn find_similar_investigations(
        &self,
        id: InvestigationId,
        limit: Option<usize>,
    ) -> RivoraResult<Vec<SearchResult>> {
        self.runtime.find_similar_investigations(id, limit)
    }

    /// Explain Search Result (RFC-016).
    pub fn explain_search_result(
        &self,
        investigation_id: InvestigationId,
        query: SearchQuery,
    ) -> RivoraResult<SearchResult> {
        self.runtime.explain_search_result(investigation_id, query)
    }

    /// Recall Related Evidence (RFC-016).
    pub fn recall_related_evidence(
        &self,
        id: InvestigationId,
    ) -> RivoraResult<Vec<RecalledEvidence>> {
        self.runtime.recall_related_evidence(id)
    }

    /// Recall Prior Outcomes (RFC-016).
    pub fn recall_prior_outcomes(&self, filter: OutcomeFilter) -> RivoraResult<Vec<PriorOutcome>> {
        self.runtime.recall_prior_outcomes(filter)
    }

    /// Suggest Recalled Context from related / similar Investigations (RFC-017).
    pub fn suggest_recalled_context(
        &self,
        id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<Vec<RecalledContext>> {
        self.runtime.suggest_recalled_context(id, actor)
    }

    /// Attach historical context from a source Investigation (RFC-017).
    pub fn attach_recalled_context_from_source(
        &self,
        investigation_id: InvestigationId,
        source_investigation_id: InvestigationId,
        reason: Option<String>,
        actor: impl Into<String>,
    ) -> RivoraResult<RecalledContext> {
        self.runtime.attach_recalled_context_from_source(
            investigation_id,
            source_investigation_id,
            reason,
            actor,
        )
    }

    /// Attach (confirm) a suggested Recalled Context record (RFC-017).
    pub fn attach_recalled_context(
        &self,
        investigation_id: InvestigationId,
        context_id: ObjectId,
        actor: impl Into<String>,
    ) -> RivoraResult<RecalledContext> {
        self.runtime
            .attach_recalled_context(investigation_id, context_id, actor)
    }

    /// Dismiss a Recalled Context record (RFC-017).
    pub fn dismiss_recalled_context(
        &self,
        investigation_id: InvestigationId,
        context_id: ObjectId,
        actor: impl Into<String>,
    ) -> RivoraResult<RecalledContext> {
        self.runtime
            .dismiss_recalled_context(investigation_id, context_id, actor)
    }

    /// List Recalled Context for an Investigation (RFC-017).
    pub fn list_recalled_context(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<Vec<RecalledContext>> {
        self.runtime.list_recalled_context(investigation_id)
    }

    /// Detect Investigation patterns across durable records (RFC-017).
    pub fn detect_patterns(&self, actor: impl Into<String>) -> RivoraResult<Vec<DetectedPattern>> {
        self.runtime.detect_patterns(actor)
    }

    /// Summarize historical trends (RFC-017).
    pub fn summarize_historical_trend(
        &self,
        repository: Option<String>,
    ) -> RivoraResult<HistoricalTrend> {
        self.runtime.summarize_historical_trend(repository)
    }

    /// Complete Investigation.
    pub fn complete_investigation(
        &self,
        id: InvestigationId,
        reason: Option<String>,
    ) -> RivoraResult<Investigation> {
        self.runtime.complete_investigation(id, reason)
    }

    /// Reopen Investigation.
    pub fn reopen_investigation(
        &self,
        id: InvestigationId,
        reason: Option<String>,
    ) -> RivoraResult<Investigation> {
        self.runtime.reopen_investigation(id, reason)
    }

    /// Run the full reasoning pipeline for convenience (still Runtime-backed).
    pub fn run_full_pipeline(
        &self,
        investigation_id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<PipelineResult> {
        let actor = actor.into();
        let knowledge = self.derive_knowledge(investigation_id, actor.clone())?;
        let evaluations = self.evaluate_investigation(investigation_id, actor.clone())?;
        let verifications = self.verify_all(investigation_id, actor.clone())?;
        let recommendations = self.generate_recommendation(investigation_id, actor)?;
        Ok(PipelineResult {
            knowledge,
            evaluations,
            verifications,
            recommendations,
        })
    }

    // --- RFC-018 Composite Capabilities / Assisted Workflows ---

    /// List approved Composite Capability definitions.
    pub fn list_composite_capabilities(&self) -> Vec<CompositeCapabilityDefinition> {
        self.runtime.list_composite_capabilities()
    }

    /// Plan a Composite Capability workflow without executing it.
    pub fn plan_workflow(
        &self,
        investigation_id: InvestigationId,
        intent: impl Into<String>,
        actor: impl Into<String>,
    ) -> RivoraResult<AssistedWorkflow> {
        self.runtime.plan_workflow(investigation_id, intent, actor)
    }

    /// Execute a planned workflow.
    pub fn execute_workflow(
        &self,
        investigation_id: InvestigationId,
        workflow_id: ObjectId,
        actor: impl Into<String>,
    ) -> RivoraResult<AssistedWorkflow> {
        self.runtime
            .execute_workflow(investigation_id, workflow_id, actor)
    }

    /// Plan and run a Composite Capability end to end.
    pub fn run_composite(
        &self,
        investigation_id: InvestigationId,
        intent: impl Into<String>,
        actor: impl Into<String>,
    ) -> RivoraResult<AssistedWorkflow> {
        self.runtime.run_composite(investigation_id, intent, actor)
    }

    /// Open a workflow by id.
    pub fn open_workflow(
        &self,
        investigation_id: InvestigationId,
        workflow_id: ObjectId,
    ) -> RivoraResult<AssistedWorkflow> {
        self.runtime.open_workflow(investigation_id, workflow_id)
    }

    /// List workflows for an Investigation.
    pub fn list_workflows(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<Vec<AssistedWorkflow>> {
        self.runtime.list_workflows(investigation_id)
    }

    /// Cancel a workflow safely.
    pub fn cancel_workflow(
        &self,
        investigation_id: InvestigationId,
        workflow_id: ObjectId,
        reason: Option<String>,
        actor: impl Into<String>,
    ) -> RivoraResult<AssistedWorkflow> {
        self.runtime
            .cancel_workflow(investigation_id, workflow_id, reason, actor)
    }

    /// Resume a partial or failed workflow.
    pub fn resume_workflow(
        &self,
        investigation_id: InvestigationId,
        workflow_id: ObjectId,
        actor: impl Into<String>,
    ) -> RivoraResult<AssistedWorkflow> {
        self.runtime
            .resume_workflow(investigation_id, workflow_id, actor)
    }

    /// Retry a failed workflow step.
    pub fn retry_workflow_step(
        &self,
        investigation_id: InvestigationId,
        workflow_id: ObjectId,
        step_index: u32,
        actor: impl Into<String>,
    ) -> RivoraResult<AssistedWorkflow> {
        self.runtime
            .retry_workflow_step(investigation_id, workflow_id, step_index, actor)
    }

    /// Confirm a confirmation-required workflow step.
    pub fn confirm_workflow_step(
        &self,
        investigation_id: InvestigationId,
        workflow_id: ObjectId,
        step_index: u32,
        actor: impl Into<String>,
    ) -> RivoraResult<AssistedWorkflow> {
        self.runtime
            .confirm_workflow_step(investigation_id, workflow_id, step_index, actor)
    }

    /// Explain workflow decisions and step states.
    pub fn explain_workflow(
        &self,
        investigation_id: InvestigationId,
        workflow_id: ObjectId,
    ) -> RivoraResult<String> {
        self.runtime.explain_workflow(investigation_id, workflow_id)
    }

    /// Summarize a workflow.
    pub fn summarize_workflow(
        &self,
        investigation_id: InvestigationId,
        workflow_id: ObjectId,
    ) -> RivoraResult<String> {
        self.runtime
            .summarize_workflow(investigation_id, workflow_id)
    }

    // --- RFC-019 Engineering Assistance ---

    /// Generate ranked hypotheses.
    pub fn generate_hypotheses(
        &self,
        investigation_id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<Vec<Hypothesis>> {
        self.runtime.generate_hypotheses(investigation_id, actor)
    }

    /// Recommend next-best verification.
    pub fn recommend_next_verification(
        &self,
        investigation_id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<Vec<VerificationSuggestion>> {
        self.runtime
            .recommend_next_verification(investigation_id, actor)
    }

    /// Assess deployment readiness.
    pub fn assess_deployment_readiness(
        &self,
        investigation_id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<DeploymentReadiness> {
        self.runtime
            .assess_deployment_readiness(investigation_id, actor)
    }

    /// Forecast risks.
    pub fn forecast_risk(
        &self,
        investigation_id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<RiskForecast> {
        self.runtime.forecast_risk(investigation_id, actor)
    }

    /// Generate root-cause guidance.
    pub fn generate_root_cause_guidance(
        &self,
        investigation_id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<RootCauseGuidance> {
        self.runtime
            .generate_root_cause_guidance(investigation_id, actor)
    }

    /// Prioritize Recommendations with inspectable factors.
    pub fn prioritize_recommendations(
        &self,
        investigation_id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<Vec<PrioritizedRecommendation>> {
        self.runtime
            .prioritize_recommendations(investigation_id, actor)
    }

    /// Generate an engineering report.
    pub fn generate_engineering_report(
        &self,
        investigation_id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<EngineeringReport> {
        self.runtime
            .generate_engineering_report(investigation_id, actor)
    }

    /// Summarize Investigation state.
    pub fn summarize_investigation_state(
        &self,
        investigation_id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<InvestigationSummary> {
        self.runtime
            .summarize_investigation_state(investigation_id, actor)
    }

    /// List stored hypotheses.
    pub fn list_hypotheses(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<Vec<Hypothesis>> {
        let _ = self.runtime.open_investigation(investigation_id)?;
        self.runtime.store().list_hypotheses(&investigation_id)
    }

    /// List engineering reports.
    pub fn list_engineering_reports(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<Vec<EngineeringReport>> {
        let _ = self.runtime.open_investigation(investigation_id)?;
        self.runtime
            .store()
            .list_engineering_reports(&investigation_id)
    }

    /// Create an explicit concrete Improvement Proposal.
    pub fn create_improvement_proposal(
        &self,
        id: InvestigationId,
        request: CreateProposalRequest,
        actor: impl Into<String>,
    ) -> RivoraResult<ImprovementProposal> {
        self.runtime.create_improvement_proposal(id, request, actor)
    }

    /// Get one Proposal snapshot.
    pub fn get_improvement_proposal(
        &self,
        id: InvestigationId,
        proposal_id: ObjectId,
    ) -> RivoraResult<ImprovementProposal> {
        self.runtime.get_improvement_proposal(id, proposal_id)
    }

    /// List latest Proposals for an Investigation.
    pub fn list_improvement_proposals(&self, id: InvestigationId) -> RivoraResult<ProposalListing> {
        self.runtime.list_improvement_proposals(id)
    }

    /// Explain one Proposal.
    pub fn explain_improvement_proposal(
        &self,
        id: InvestigationId,
        proposal_id: ObjectId,
    ) -> RivoraResult<String> {
        self.runtime.explain_improvement_proposal(id, proposal_id)
    }

    /// Transition Proposal status with explicit provenance.
    #[allow(clippy::too_many_arguments)]
    pub fn update_improvement_proposal_status(
        &self,
        id: InvestigationId,
        proposal_id: ObjectId,
        status: ProposalStatus,
        actor: impl Into<String>,
        reason: impl Into<String>,
        authority: ProposalTransitionAuthority,
    ) -> RivoraResult<ImprovementProposal> {
        self.runtime.update_improvement_proposal_status(
            id,
            proposal_id,
            status,
            actor,
            reason,
            authority,
        )
    }

    /// Attach explicit Proposal feedback as a preserved revision.
    pub fn add_improvement_proposal_feedback(
        &self,
        id: InvestigationId,
        proposal_id: ObjectId,
        category: ProposalFeedbackCategory,
        comment: impl Into<String>,
        actor: impl Into<String>,
    ) -> RivoraResult<ImprovementProposal> {
        self.runtime
            .add_improvement_proposal_feedback(id, proposal_id, category, comment, actor)
    }

    /// Refine a Proposal into a new immutable revision.
    #[allow(clippy::too_many_arguments)]
    pub fn refine_improvement_proposal(
        &self,
        id: InvestigationId,
        proposal_id: ObjectId,
        request: RefineProposalRequest,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<ImprovementProposal> {
        self.runtime
            .refine_improvement_proposal(id, proposal_id, request, actor, reason)
    }

    /// Supersede a Proposal with an explicit replacement.
    pub fn supersede_improvement_proposal(
        &self,
        id: InvestigationId,
        proposal_id: ObjectId,
        replacement_id: ObjectId,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<ImprovementProposal> {
        self.runtime
            .supersede_improvement_proposal(id, proposal_id, replacement_id, actor, reason)
    }

    /// List every Proposal revision.
    pub fn list_improvement_proposal_revisions(
        &self,
        id: InvestigationId,
        lineage_id: ObjectId,
    ) -> RivoraResult<ProposalListing> {
        self.runtime
            .list_improvement_proposal_revisions(id, lineage_id)
    }

    /// Generate deterministic bounded Improvement Proposal alternatives.
    pub fn generate_improvement_proposals(
        &self,
        id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<Vec<ImprovementProposal>> {
        self.runtime.generate_improvement_proposals(id, actor)
    }

    /// Generate alternatives for an improvement opportunity.
    pub fn generate_proposal_alternatives(
        &self,
        id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<Vec<ImprovementProposal>> {
        self.runtime.generate_proposal_alternatives(id, actor)
    }

    /// Compare Proposals with inspectable factors.
    pub fn compare_improvement_proposals(
        &self,
        id: InvestigationId,
        proposal_ids: Vec<ObjectId>,
    ) -> RivoraResult<ProposalComparison> {
        self.runtime.compare_improvement_proposals(id, proposal_ids)
    }

    /// Prioritize latest Proposals with inspectable factors.
    pub fn prioritize_improvement_proposals(
        &self,
        id: InvestigationId,
    ) -> RivoraResult<ProposalComparison> {
        self.runtime.prioritize_improvement_proposals(id)
    }

    /// Generate a concrete proposed Verification Plan without execution.
    pub fn generate_proposal_verification_plan(
        &self,
        id: InvestigationId,
        proposal_id: ObjectId,
    ) -> RivoraResult<ProposalVerificationPlan> {
        self.runtime
            .generate_proposal_verification_plan(id, proposal_id)
    }

    /// Generate a bounded implementation outline without application.
    pub fn generate_proposal_implementation_outline(
        &self,
        id: InvestigationId,
        proposal_id: ObjectId,
    ) -> RivoraResult<Vec<String>> {
        self.runtime
            .generate_proposal_implementation_outline(id, proposal_id)
    }

    /// Explain Proposal input provenance.
    pub fn explain_improvement_proposal_provenance(
        &self,
        id: InvestigationId,
        proposal_id: ObjectId,
    ) -> RivoraResult<String> {
        self.runtime
            .explain_improvement_proposal_provenance(id, proposal_id)
    }

    /// Generate and store a sanitized Proposal artifact.
    pub fn generate_proposal_artifact(
        &self,
        id: InvestigationId,
        proposal_id: ObjectId,
        actor: impl Into<String>,
    ) -> RivoraResult<ProposalArtifact> {
        self.runtime
            .generate_proposal_artifact(id, proposal_id, actor)
    }

    /// List durable Proposal artifacts.
    pub fn list_proposal_artifacts(
        &self,
        id: InvestigationId,
    ) -> RivoraResult<ProposalArtifactListing> {
        self.runtime.list_proposal_artifacts(id)
    }

    /// Generate coding-agent handoff text without invocation.
    pub fn generate_coding_agent_handoff(
        &self,
        id: InvestigationId,
        proposal_id: ObjectId,
    ) -> RivoraResult<String> {
        self.runtime.generate_coding_agent_handoff(id, proposal_id)
    }

    /// Filter the Investigation Proposal portfolio.
    pub fn proposal_portfolio(
        &self,
        id: InvestigationId,
        filter: ProposalPortfolioFilter,
    ) -> RivoraResult<Vec<ImprovementProposal>> {
        self.runtime.proposal_portfolio(id, filter)
    }

    /// Trace Engineering Objects through one Proposal.
    pub fn trace_improvement_proposal(
        &self,
        id: InvestigationId,
        proposal_id: ObjectId,
    ) -> RivoraResult<ProposalTrace> {
        self.runtime.trace_improvement_proposal(id, proposal_id)
    }

    /// Record an inert manual external implementation reference.
    pub fn record_external_implementation_reference(
        &self,
        id: InvestigationId,
        proposal_id: ObjectId,
        reference: impl Into<String>,
        actor: impl Into<String>,
    ) -> RivoraResult<ImprovementProposal> {
        self.runtime
            .record_external_implementation_reference(id, proposal_id, reference, actor)
    }

    // -----------------------------------------------------------------------
    // v0.5 Implementation Records / Measured Outcomes / Patterns
    // -----------------------------------------------------------------------

    /// Record that external work associated with a Proposal was performed.
    pub fn record_external_implementation(
        &self,
        id: InvestigationId,
        proposal_id: ObjectId,
        request: RecordImplementationRequest,
        actor: impl Into<String>,
    ) -> RivoraResult<ImplementationRecord> {
        self.runtime
            .record_external_implementation(id, proposal_id, request, actor)
    }

    /// Revise an Implementation Record (immutable successor).
    pub fn revise_implementation_record(
        &self,
        id: InvestigationId,
        record_id: ObjectId,
        request: ReviseImplementationRequest,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<ImplementationRecord> {
        self.runtime
            .revise_implementation_record(id, record_id, request, actor, reason)
    }

    /// Link evidence to an Implementation Record.
    pub fn link_implementation_evidence(
        &self,
        id: InvestigationId,
        record_id: ObjectId,
        evidence_ids: Vec<ObjectId>,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<ImplementationRecord> {
        self.runtime
            .link_implementation_evidence(id, record_id, evidence_ids, actor, reason)
    }

    /// Mark an Implementation Record ready for evaluation.
    pub fn mark_implementation_ready(
        &self,
        id: InvestigationId,
        record_id: ObjectId,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<ImplementationRecord> {
        self.runtime
            .mark_implementation_ready(id, record_id, actor, reason)
    }

    /// Withdraw an Implementation Record.
    pub fn withdraw_implementation(
        &self,
        id: InvestigationId,
        record_id: ObjectId,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<ImplementationRecord> {
        self.runtime
            .withdraw_implementation(id, record_id, actor, reason)
    }

    /// Supersede an Implementation Record.
    pub fn supersede_implementation(
        &self,
        id: InvestigationId,
        record_id: ObjectId,
        successor_id: ObjectId,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<ImplementationRecord> {
        self.runtime
            .supersede_implementation(id, record_id, successor_id, actor, reason)
    }

    /// List Implementation Records.
    pub fn list_implementation_records(
        &self,
        id: InvestigationId,
    ) -> RivoraResult<ImplementationListing> {
        self.runtime.list_implementation_records(id)
    }

    /// Get one Implementation Record.
    pub fn get_implementation_record(
        &self,
        id: InvestigationId,
        record_id: ObjectId,
    ) -> RivoraResult<ImplementationRecord> {
        self.runtime.get_implementation_record(id, record_id)
    }

    /// List Implementation Record revisions.
    pub fn list_implementation_revisions(
        &self,
        id: InvestigationId,
        lineage_id: ObjectId,
    ) -> RivoraResult<ImplementationListing> {
        self.runtime.list_implementation_revisions(id, lineage_id)
    }

    /// Create a Draft Measured Learning Outcome.
    pub fn create_measured_learning_outcome(
        &self,
        id: InvestigationId,
        proposal_id: ObjectId,
        implementation_record_id: ObjectId,
        actor: impl Into<String>,
    ) -> RivoraResult<MeasuredLearningOutcome> {
        self.runtime.create_measured_learning_outcome(
            id,
            proposal_id,
            implementation_record_id,
            actor,
        )
    }

    /// Collect typed evidence on a Measured Learning Outcome.
    pub fn collect_outcome_evidence(
        &self,
        id: InvestigationId,
        outcome_id: ObjectId,
        request: CollectOutcomeEvidenceRequest,
        actor: impl Into<String>,
    ) -> RivoraResult<MeasuredLearningOutcome> {
        self.runtime
            .collect_outcome_evidence(id, outcome_id, request, actor)
    }

    /// Revise a Measured Learning Outcome.
    pub fn revise_measured_learning_outcome(
        &self,
        id: InvestigationId,
        outcome_id: ObjectId,
        request: ReviseMeasuredOutcomeRequest,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<MeasuredLearningOutcome> {
        self.runtime
            .revise_measured_learning_outcome(id, outcome_id, request, actor, reason)
    }

    /// Transition a Measured Learning Outcome lifecycle status.
    pub fn transition_measured_learning_outcome(
        &self,
        id: InvestigationId,
        outcome_id: ObjectId,
        to: MeasuredOutcomeStatus,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<MeasuredLearningOutcome> {
        self.runtime
            .transition_measured_learning_outcome(id, outcome_id, to, actor, reason)
    }

    /// Withdraw a Measured Learning Outcome.
    pub fn withdraw_measured_learning_outcome(
        &self,
        id: InvestigationId,
        outcome_id: ObjectId,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<MeasuredLearningOutcome> {
        self.runtime
            .withdraw_measured_learning_outcome(id, outcome_id, actor, reason)
    }

    /// Supersede a Measured Learning Outcome.
    pub fn supersede_measured_learning_outcome(
        &self,
        id: InvestigationId,
        outcome_id: ObjectId,
        successor_id: ObjectId,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<MeasuredLearningOutcome> {
        self.runtime.supersede_measured_learning_outcome(
            id,
            outcome_id,
            successor_id,
            actor,
            reason,
        )
    }

    /// Deterministically evaluate a Measured Learning Outcome.
    pub fn evaluate_measured_learning_outcome(
        &self,
        id: InvestigationId,
        outcome_id: ObjectId,
        actor: impl Into<String>,
    ) -> RivoraResult<MeasuredLearningOutcome> {
        self.runtime
            .evaluate_measured_learning_outcome(id, outcome_id, actor)
    }

    /// Explicitly verify a Measured Learning Outcome.
    pub fn verify_measured_learning_outcome(
        &self,
        id: InvestigationId,
        outcome_id: ObjectId,
        actor: impl Into<String>,
        reason: impl Into<String>,
        override_readiness: bool,
        override_reason: Option<String>,
    ) -> RivoraResult<MeasuredLearningOutcome> {
        self.runtime.verify_measured_learning_outcome(
            id,
            outcome_id,
            actor,
            reason,
            override_readiness,
            override_reason,
        )
    }

    /// List Measured Learning Outcomes.
    pub fn list_measured_learning_outcomes(
        &self,
        id: InvestigationId,
    ) -> RivoraResult<MeasuredOutcomeListing> {
        self.runtime.list_measured_learning_outcomes(id)
    }

    /// Get one Measured Learning Outcome.
    pub fn get_measured_learning_outcome(
        &self,
        id: InvestigationId,
        outcome_id: ObjectId,
    ) -> RivoraResult<MeasuredLearningOutcome> {
        self.runtime.get_measured_learning_outcome(id, outcome_id)
    }

    /// List Measured Learning Outcome revisions.
    pub fn list_measured_outcome_revisions(
        &self,
        id: InvestigationId,
        lineage_id: ObjectId,
    ) -> RivoraResult<MeasuredOutcomeListing> {
        self.runtime.list_measured_outcome_revisions(id, lineage_id)
    }

    /// Trace Proposal → Implementation → Measured Learning Outcome.
    pub fn trace_measured_learning_outcome(
        &self,
        id: InvestigationId,
        outcome_id: ObjectId,
    ) -> RivoraResult<MeasuredOutcomeTrace> {
        self.runtime.trace_measured_learning_outcome(id, outcome_id)
    }

    /// Export Measured Learning Outcome as Markdown.
    pub fn export_measured_learning_outcome_markdown(
        &self,
        id: InvestigationId,
        outcome_id: ObjectId,
    ) -> RivoraResult<String> {
        self.runtime
            .export_measured_learning_outcome_markdown(id, outcome_id)
    }

    /// Export Measured Learning Outcome as JSON.
    pub fn export_measured_learning_outcome_json(
        &self,
        id: InvestigationId,
        outcome_id: ObjectId,
    ) -> RivoraResult<String> {
        self.runtime
            .export_measured_learning_outcome_json(id, outcome_id)
    }

    /// Derive Learning Patterns from verified Outcomes.
    pub fn derive_learning_patterns(
        &self,
        actor: impl Into<String>,
    ) -> RivoraResult<Vec<LearningPattern>> {
        self.runtime.derive_learning_patterns(actor)
    }

    /// List Learning Patterns.
    pub fn list_learning_patterns(&self) -> RivoraResult<Vec<LearningPattern>> {
        self.runtime.list_learning_patterns()
    }

    /// Show one Learning Pattern.
    pub fn get_learning_pattern(&self, pattern_id: ObjectId) -> RivoraResult<LearningPattern> {
        self.runtime.get_learning_pattern(pattern_id)
    }

    /// Retire a Learning Pattern.
    pub fn retire_learning_pattern(
        &self,
        pattern_id: ObjectId,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<LearningPattern> {
        self.runtime
            .retire_learning_pattern(pattern_id, actor, reason)
    }

    /// Explain historical influence for a Proposal.
    pub fn explain_historical_influence(
        &self,
        id: InvestigationId,
        proposal_id: ObjectId,
    ) -> RivoraResult<HistoricalInfluenceExplanation> {
        self.runtime.explain_historical_influence(id, proposal_id)
    }

    /// Export Learning Pattern as Markdown.
    pub fn export_learning_pattern_markdown(&self, pattern_id: ObjectId) -> RivoraResult<String> {
        self.runtime.export_learning_pattern_markdown(pattern_id)
    }

    /// Export Learning Pattern as JSON.
    pub fn export_learning_pattern_json(&self, pattern_id: ObjectId) -> RivoraResult<String> {
        self.runtime.export_learning_pattern_json(pattern_id)
    }

    // -----------------------------------------------------------------------
    // v0.7 Capability Engineering Loop (RFC-028)
    // -----------------------------------------------------------------------

    /// Route Observations to compatible registered Capabilities.
    pub fn route_observations_to_capabilities(
        &self,
        investigation_id: InvestigationId,
        observation_ids: &[ObjectId],
    ) -> RivoraResult<CapabilityRoutingDecision> {
        self.runtime
            .route_observations_to_capabilities(investigation_id, observation_ids)
    }

    /// Run the Engineering Loop for a completed execution attempt.
    pub fn run_capability_lifecycle_for_attempt(
        &self,
        investigation_id: InvestigationId,
        attempt_id: ObjectId,
        actor: impl Into<String>,
    ) -> RivoraResult<CapabilityLifecycleRun> {
        self.runtime
            .run_capability_lifecycle_for_attempt(investigation_id, attempt_id, actor)
    }

    /// List Engineering Loop runs for an Investigation.
    pub fn list_lifecycle_runs(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<CapabilityLifecycleRunListing> {
        self.runtime.list_lifecycle_runs(investigation_id)
    }

    /// Load one Engineering Loop run snapshot.
    pub fn get_lifecycle_run(
        &self,
        investigation_id: InvestigationId,
        run_id: ObjectId,
    ) -> RivoraResult<CapabilityLifecycleRun> {
        self.runtime.get_lifecycle_run(investigation_id, run_id)
    }

    /// Trace Capability Engineering Loop lineage for an invocation or run id.
    pub fn trace_capability_lifecycle(
        &self,
        investigation_id: InvestigationId,
        invocation_or_run_id: &str,
    ) -> RivoraResult<CapabilityLifecycleTrace> {
        self.runtime
            .trace_capability_lifecycle(investigation_id, invocation_or_run_id)
    }

    // -----------------------------------------------------------------------
    // v0.6 Execution Through External Systems (RFC-025/026/027)
    // -----------------------------------------------------------------------

    /// Register an execution capability adapter on the Runtime.
    pub fn register_execution_capability(
        &self,
        capability: std::sync::Arc<dyn crate::domain::ExecutionCapability>,
    ) -> RivoraResult<()> {
        self.runtime.register_execution_capability(capability)
    }

    /// List registered execution capabilities.
    pub fn list_execution_capabilities(&self) -> Vec<ExecutionCapabilityDescriptor> {
        self.runtime.list_execution_capabilities()
    }

    /// Show one execution capability.
    pub fn show_execution_capability(
        &self,
        capability_id: &str,
    ) -> RivoraResult<ExecutionCapabilityDescriptor> {
        self.runtime.show_execution_capability(capability_id)
    }

    /// First-party Capability and Connector coverage report (v0.8).
    pub fn capability_coverage_report(&self) -> crate::domain::CapabilityCoverageReport {
        self.runtime.capability_coverage_report()
    }

    /// Local store health report (v0.9 production diagnostics).
    pub fn store_health(&self) -> RivoraResult<crate::domain::StoreHealthReport> {
        self.runtime.store_health()
    }

    /// Sanitized diagnostic export (v0.9).
    pub fn diagnostic_export(&self) -> RivoraResult<serde_json::Value> {
        self.runtime.diagnostic_export()
    }

    /// Backup the store to a destination directory.
    pub fn backup_store(&self, dest: impl AsRef<std::path::Path>) -> RivoraResult<()> {
        self.runtime.backup_store(dest)
    }

    /// Rebuild derived observation indexes from canonical records.
    pub fn rebuild_observation_indexes(&self) -> RivoraResult<u64> {
        self.runtime.rebuild_observation_indexes()
    }

    /// Create an Execution Plan for an accepted Proposal.
    pub fn create_execution_plan(
        &self,
        investigation_id: InvestigationId,
        request: CreateExecutionPlanRequest,
        actor: impl Into<String>,
    ) -> RivoraResult<ExecutionPlan> {
        self.runtime
            .create_execution_plan(investigation_id, request, actor)
    }

    /// Revise an Execution Plan.
    pub fn revise_execution_plan(
        &self,
        investigation_id: InvestigationId,
        plan_id: ObjectId,
        request: ReviseExecutionPlanRequest,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<ExecutionPlan> {
        self.runtime
            .revise_execution_plan(investigation_id, plan_id, request, actor, reason)
    }

    /// Validate an Execution Plan (Draft → ReadyForReview).
    pub fn validate_execution_plan(
        &self,
        investigation_id: InvestigationId,
        plan_id: ObjectId,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<ExecutionPlan> {
        self.runtime
            .validate_execution_plan(investigation_id, plan_id, actor, reason)
    }

    /// Preview / dry-run an Execution Plan.
    pub fn preview_execution_plan(
        &self,
        investigation_id: InvestigationId,
        plan_id: ObjectId,
    ) -> RivoraResult<DryRunResult> {
        self.runtime
            .preview_execution_plan(investigation_id, plan_id)
    }

    /// Approve an exact Execution Plan revision.
    #[allow(clippy::too_many_arguments)]
    pub fn approve_execution_plan(
        &self,
        investigation_id: InvestigationId,
        plan_id: ObjectId,
        actor: impl Into<String>,
        reason: impl Into<String>,
        approved_actions: Vec<String>,
        denied_actions: Vec<String>,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
        one_time: bool,
    ) -> RivoraResult<(ExecutionPlan, ExecutionApproval)> {
        self.runtime.approve_execution_plan(
            investigation_id,
            plan_id,
            actor,
            reason,
            approved_actions,
            denied_actions,
            expires_at,
            one_time,
        )
    }

    /// Reject an Execution Plan.
    pub fn reject_execution_plan(
        &self,
        investigation_id: InvestigationId,
        plan_id: ObjectId,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<ExecutionPlan> {
        self.runtime
            .reject_execution_plan(investigation_id, plan_id, actor, reason)
    }

    /// Cancel an Execution Plan.
    pub fn cancel_execution_plan(
        &self,
        investigation_id: InvestigationId,
        plan_id: ObjectId,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<ExecutionPlan> {
        self.runtime
            .cancel_execution_plan(investigation_id, plan_id, actor, reason)
    }

    /// Execute an approved plan (or dry-run).
    #[allow(clippy::too_many_arguments)]
    pub fn execute_plan(
        &self,
        investigation_id: InvestigationId,
        plan_id: ObjectId,
        approval_id: ObjectId,
        actor: impl Into<String>,
        idempotency_key: impl Into<String>,
        dry_run: bool,
    ) -> RivoraResult<ExecutionAttempt> {
        self.runtime.execute_plan(
            investigation_id,
            plan_id,
            approval_id,
            actor,
            idempotency_key,
            dry_run,
        )
    }

    /// List execution plans.
    pub fn list_execution_plans(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<ExecutionPlanListing> {
        self.runtime.list_execution_plans(investigation_id)
    }

    /// Get one execution plan.
    pub fn get_execution_plan(
        &self,
        investigation_id: InvestigationId,
        plan_id: ObjectId,
    ) -> RivoraResult<ExecutionPlan> {
        self.runtime.get_execution_plan(investigation_id, plan_id)
    }

    /// List execution plan revisions.
    pub fn list_execution_plan_revisions(
        &self,
        investigation_id: InvestigationId,
        lineage_id: ObjectId,
    ) -> RivoraResult<ExecutionPlanListing> {
        self.runtime
            .list_execution_plan_revisions(investigation_id, lineage_id)
    }

    /// List execution attempts.
    pub fn list_execution_attempts(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<ExecutionAttemptListing> {
        self.runtime.list_execution_attempts(investigation_id)
    }

    /// Get one execution attempt.
    pub fn get_execution_attempt(
        &self,
        investigation_id: InvestigationId,
        attempt_id: ObjectId,
    ) -> RivoraResult<ExecutionAttempt> {
        self.runtime
            .get_execution_attempt(investigation_id, attempt_id)
    }

    /// Verify an execution attempt independently.
    pub fn verify_execution_attempt(
        &self,
        investigation_id: InvestigationId,
        attempt_id: ObjectId,
        actor: impl Into<String>,
    ) -> RivoraResult<ExecutionVerification> {
        self.runtime
            .verify_execution_attempt(investigation_id, attempt_id, actor)
    }

    /// List execution receipts.
    pub fn list_execution_receipts(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<ExecutionReceiptListing> {
        self.runtime.list_execution_receipts(investigation_id)
    }

    /// Close a verified execution plan.
    pub fn close_execution_plan(
        &self,
        investigation_id: InvestigationId,
        plan_id: ObjectId,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<ExecutionPlan> {
        self.runtime
            .close_execution_plan(investigation_id, plan_id, actor, reason)
    }

    /// Link execution attempt to an Implementation Record.
    pub fn link_execution_to_implementation(
        &self,
        investigation_id: InvestigationId,
        attempt_id: ObjectId,
        actor: impl Into<String>,
        summary: impl Into<String>,
    ) -> RivoraResult<ImplementationRecord> {
        self.runtime
            .link_execution_to_implementation(investigation_id, attempt_id, actor, summary)
    }

    /// Create a rollback plan draft from attempt metadata.
    pub fn create_rollback_plan(
        &self,
        investigation_id: InvestigationId,
        attempt_id: ObjectId,
        actor: impl Into<String>,
    ) -> RivoraResult<ExecutionPlan> {
        self.runtime
            .create_rollback_plan(investigation_id, attempt_id, actor)
    }

    /// Explain execution policy for a plan.
    pub fn explain_execution_policy(
        &self,
        investigation_id: InvestigationId,
        plan_id: ObjectId,
    ) -> RivoraResult<ExecutionPolicyDecision> {
        self.runtime
            .explain_execution_policy(investigation_id, plan_id)
    }

    /// Trace execution lineage.
    pub fn trace_execution(
        &self,
        investigation_id: InvestigationId,
        plan_id: ObjectId,
    ) -> RivoraResult<ExecutionTrace> {
        self.runtime.trace_execution(investigation_id, plan_id)
    }

    /// Export execution plan JSON.
    pub fn export_execution_plan(
        &self,
        investigation_id: InvestigationId,
        plan_id: ObjectId,
    ) -> RivoraResult<String> {
        self.runtime
            .export_execution_plan(investigation_id, plan_id)
    }

    /// Export execution receipt JSON.
    pub fn export_execution_receipt(
        &self,
        investigation_id: InvestigationId,
        receipt_id: ObjectId,
    ) -> RivoraResult<String> {
        self.runtime
            .export_execution_receipt(investigation_id, receipt_id)
    }

    /// Classify retry safety for an attempt.
    pub fn classify_retry_safety(
        &self,
        investigation_id: InvestigationId,
        attempt_id: ObjectId,
    ) -> RivoraResult<RetrySafety> {
        self.runtime
            .classify_retry_safety(investigation_id, attempt_id)
    }
}

/// Result of running the full reasoning pipeline.
#[derive(Debug, Clone)]
pub struct PipelineResult {
    /// Derived Knowledge.
    pub knowledge: Vec<KnowledgeObject>,
    /// Evaluations.
    pub evaluations: Vec<Evaluation>,
    /// Verification Receipts.
    pub verifications: Vec<VerificationReceipt>,
    /// Recommendations.
    pub recommendations: Vec<Recommendation>,
}

impl std::fmt::Debug for CapabilityService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CapabilityService").finish_non_exhaustive()
    }
}
