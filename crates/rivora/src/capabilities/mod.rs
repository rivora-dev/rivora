//! Capability layer — intent-oriented orchestration (RFC-011).
//!
//! Capabilities coordinate Runtime subsystems. They do not implement
//! engineering reasoning themselves. Workspace and CLI share this service.

use std::sync::Arc;

use crate::domain::{
    Evaluation, Investigation, InvestigationId, InvestigationRelationship, KnowledgeObject,
    LearningOutcome, MemoryRecord, ObjectId, Observation, ObservationKind, OutcomeDisposition,
    Recommendation, TimelineEntry, VerificationReceipt,
};
use crate::error::RivoraResult;
use crate::runtime::graph::{RelatedInvestigation, RelationshipExplanation};
use crate::runtime::learning::RecordOutcomeRequest;
use crate::runtime::observation::IngestObservationRequest;
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
