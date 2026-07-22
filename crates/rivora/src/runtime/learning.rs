//! Learning engine (RFC-010).

use crate::domain::{
    InvestigationId, InvestigationStatus, LearningOutcome, ObjectId, OutcomeDisposition,
    Provenance, RecommendationStatus,
};
use crate::error::{RivoraError, RivoraResult};
use crate::runtime::Runtime;

/// Request to record an observed outcome.
#[derive(Debug, Clone)]
pub struct RecordOutcomeRequest {
    /// Investigation that produced the Recommendation.
    pub investigation_id: InvestigationId,
    /// Recommendation the outcome relates to.
    pub recommendation_id: Option<ObjectId>,
    /// Observed disposition.
    pub disposition: OutcomeDisposition,
    /// Notes about what happened.
    pub notes: String,
    /// Optional impact description.
    pub impact: Option<String>,
    /// Actor recording the outcome.
    pub actor: String,
}

impl Runtime {
    /// Record a Learning Outcome without rewriting history.
    pub fn record_outcome(&self, request: RecordOutcomeRequest) -> RivoraResult<LearningOutcome> {
        let mut inv = self.store.load_investigation(&request.investigation_id)?;

        // Ensure we can enter Learning status.
        if inv.status == InvestigationStatus::Completed {
            return Err(RivoraError::OperationNotAllowed {
                status: inv.status,
                message: "cannot record outcome on completed investigation; reopen first".into(),
            });
        }

        // Advance toward Learning if needed (from Recommending or earlier after recs exist).
        if inv.status != InvestigationStatus::Learning {
            // Prefer advancing if recommendations exist.
            let recs = self.store.list_recommendations(&request.investigation_id)?;
            if recs.is_empty() && request.recommendation_id.is_some() {
                return Err(RivoraError::Precondition(
                    "recommendation not found for learning".into(),
                ));
            }
            if inv.status == InvestigationStatus::Recommending
                || inv.status == InvestigationStatus::Verifying
                || inv.status == InvestigationStatus::Evaluating
                || inv.status == InvestigationStatus::Understanding
                || inv.status == InvestigationStatus::Collecting
            {
                self.ensure_status_at_least(
                    request.investigation_id,
                    InvestigationStatus::Learning,
                )?;
                inv = self.store.load_investigation(&request.investigation_id)?;
            }
        }

        if let Some(rec_id) = request.recommendation_id {
            let mut rec = self
                .store
                .load_recommendation(&request.investigation_id, &rec_id)?;
            // Update recommendation status based on disposition (does not rewrite Memory).
            rec.status = match request.disposition {
                OutcomeDisposition::Accepted | OutcomeDisposition::Successful => {
                    RecommendationStatus::Accepted
                }
                OutcomeDisposition::Rejected => RecommendationStatus::Rejected,
                OutcomeDisposition::Ignored => RecommendationStatus::Ignored,
                OutcomeDisposition::Unsuccessful => RecommendationStatus::Accepted,
            };
            self.store.save_recommendation(&rec)?;
        }

        let provenance =
            Provenance::now(request.actor, "runtime").with_capability("record_outcome");

        let outcome = LearningOutcome::new(
            request.investigation_id,
            request.recommendation_id,
            request.disposition,
            request.notes,
            request.impact,
            provenance,
        );

        self.store.append_learning(&outcome)?;

        // Learning influences future reasoning: persist simple heuristic stats as metadata note
        // on Investigation without rewriting historical objects.
        inv.metadata.insert(
            "last_learning_disposition".into(),
            serde_json::Value::String(outcome.disposition.as_str().to_string()),
        );
        inv.metadata.insert(
            "last_learning_id".into(),
            serde_json::Value::String(outcome.id.to_string()),
        );
        inv.updated_at = chrono::Utc::now();
        self.store.save_investigation(&inv)?;

        Ok(outcome)
    }

    /// List Learning Outcomes for an Investigation.
    pub fn list_learning(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<Vec<LearningOutcome>> {
        let _ = self.store.load_investigation(&investigation_id)?;
        self.store.list_learning(&investigation_id)
    }

    /// Collect prior Learning dispositions for future deterministic reasoning.
    pub fn prior_learning_dispositions(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<Vec<OutcomeDisposition>> {
        Ok(self
            .list_learning(investigation_id)?
            .into_iter()
            .map(|o| o.disposition)
            .collect())
    }
}
