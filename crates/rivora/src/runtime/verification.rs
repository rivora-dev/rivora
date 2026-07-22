//! Verification engine (RFC-009).

use crate::domain::{
    AssessmentType, Confidence, InvestigationId, InvestigationStatus, ObjectId, Provenance,
    VerificationReceipt, VerificationResult,
};
use crate::error::{RivoraError, RivoraResult};
use crate::runtime::Runtime;

impl Runtime {
    /// Verify an Evaluation (or the latest risk evaluation if none specified).
    pub fn verify_conclusion(
        &self,
        investigation_id: InvestigationId,
        evaluation_id: Option<ObjectId>,
        actor: impl Into<String>,
    ) -> RivoraResult<VerificationReceipt> {
        let actor = actor.into();
        let _ = self.store.load_investigation(&investigation_id)?;
        let evaluations = self.store.list_evaluations(&investigation_id)?;
        if evaluations.is_empty() {
            return Err(RivoraError::Precondition(
                "cannot verify without evaluations".into(),
            ));
        }

        let evaluation = if let Some(id) = evaluation_id {
            evaluations
                .iter()
                .find(|e| e.id == id)
                .cloned()
                .ok_or(RivoraError::ObjectNotFound(id))?
        } else {
            evaluations
                .iter()
                .find(|e| e.assessment_type == AssessmentType::Risk)
                .cloned()
                .or_else(|| evaluations.last().cloned())
                .expect("evaluations non-empty")
        };

        self.ensure_status_at_least(investigation_id, InvestigationStatus::Verifying)?;

        let memory = self.store.list_memory(&investigation_id)?;
        let evidence_ids = evaluation.supporting_memory_ids.clone();
        let evidence_present = !evidence_ids.is_empty()
            && evidence_ids
                .iter()
                .all(|id| memory.iter().any(|m| m.id == *id));

        let (result, confidence, reason) = if evidence_present && !memory.is_empty() {
            // Deterministic MVP: pass when evaluation cites existing Memory.
            // Fail risk evaluations that claim high severity without risk-related knowledge?
            // Keep simple: pass with evidence; inconclusive without.
            (
                VerificationResult::Pass,
                Confidence::new(evaluation.confidence.value() * 0.95),
                format!(
                    "Verified evaluation '{}': all {} cited Memory record(s) exist in Investigation.",
                    evaluation.summary,
                    evidence_ids.len()
                ),
            )
        } else if memory.is_empty() {
            (
                VerificationResult::Fail,
                Confidence::new(0.9),
                "Verification failed: no Memory evidence available.".into(),
            )
        } else {
            (
                VerificationResult::Inconclusive,
                Confidence::new(0.4),
                "Verification inconclusive: evaluation lacks resolvable Memory evidence links."
                    .into(),
            )
        };

        let provenance = Provenance::now(actor, "runtime")
            .with_capability("verify_conclusion")
            .with_evidence(evidence_ids.clone());

        let receipt = VerificationReceipt::new(
            investigation_id,
            evaluation.id,
            evaluation.summary.clone(),
            result,
            confidence,
            evidence_ids,
            Vec::new(),
            reason,
            provenance,
        );

        self.store.append_verification(&receipt)?;
        Ok(receipt)
    }

    /// Verify all current Evaluations, preserving pass/fail/inconclusive.
    pub fn verify_all(
        &self,
        investigation_id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<Vec<VerificationReceipt>> {
        let actor = actor.into();
        let evaluations = {
            let mut evals = self.store.list_evaluations(&investigation_id)?;
            if evals.is_empty() {
                evals = self.evaluate_investigation(investigation_id, actor.clone())?;
            }
            evals
        };
        let mut receipts = Vec::new();
        for evaluation in evaluations {
            receipts.push(self.verify_conclusion(
                investigation_id,
                Some(evaluation.id),
                actor.clone(),
            )?);
        }
        Ok(receipts)
    }

    /// List Verification Receipts.
    pub fn list_verifications(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<Vec<VerificationReceipt>> {
        let _ = self.store.load_investigation(&investigation_id)?;
        self.store.list_verifications(&investigation_id)
    }
}
