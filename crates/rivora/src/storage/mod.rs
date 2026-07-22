//! Local durable persistence for Engineering Objects.

mod local;

pub use local::LocalStore;

use crate::domain::{
    Evaluation, Investigation, InvestigationId, InvestigationRelationship, KnowledgeObject,
    LearningOutcome, MemoryRecord, ObjectId, Observation, RecalledContext, Recommendation,
    TimelineEntry, VerificationReceipt,
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
}
