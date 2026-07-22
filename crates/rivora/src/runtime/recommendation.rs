//! Recommendation engine (RFC-004 Improvement/Recommendation).

use crate::domain::{
    AssessmentType, Confidence, InvestigationId, InvestigationStatus, Provenance, Recommendation,
    Severity, VerificationResult,
};
use crate::error::{RivoraError, RivoraResult};
use crate::runtime::Runtime;

impl Runtime {
    /// Generate evidence-backed Recommendations.
    ///
    /// Recommendations are proposals only — never auto-applied.
    pub fn generate_recommendation(
        &self,
        investigation_id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<Vec<Recommendation>> {
        let actor = actor.into();
        let _ = self.store.load_investigation(&investigation_id)?;

        let mut evaluations = self.store.list_evaluations(&investigation_id)?;
        if evaluations.is_empty() {
            evaluations = self.evaluate_investigation(investigation_id, actor.clone())?;
        }

        let mut receipts = self.store.list_verifications(&investigation_id)?;
        if receipts.is_empty() {
            receipts = self.verify_all(investigation_id, actor.clone())?;
        }

        self.ensure_status_at_least(investigation_id, InvestigationStatus::Recommending)?;

        let risk = evaluations
            .iter()
            .find(|e| e.assessment_type == AssessmentType::Risk);
        let passed = receipts
            .iter()
            .any(|r| r.result == VerificationResult::Pass);
        let failed = receipts
            .iter()
            .any(|r| r.result == VerificationResult::Fail);

        if !passed && failed {
            return Err(RivoraError::Precondition(
                "cannot recommend when all verifications failed".into(),
            ));
        }

        let evaluation_ids: Vec<_> = evaluations.iter().map(|e| e.id).collect();
        let verification_ids: Vec<_> = receipts.iter().map(|r| r.id).collect();

        let provenance = Provenance::now(actor, "runtime")
            .with_capability("generate_recommendation")
            .with_evidence(
                evaluation_ids
                    .iter()
                    .chain(verification_ids.iter())
                    .copied()
                    .collect(),
            );

        let (summary, rationale, confidence): (String, String, Confidence) = if let Some(risk) =
            risk
        {
            if matches!(risk.severity, Severity::High | Severity::Critical) {
                (
                    "Investigate and remediate failure signals before promoting changes."
                        .to_string(),
                    format!(
                        "Risk evaluation reports severity {} with confidence {:.0}%. \
                         Verification receipts: {} pass, {} fail, {} inconclusive. \
                         Recommendation is a proposal and must be approved by a human.",
                        risk.severity.as_str(),
                        risk.confidence.value() * 100.0,
                        receipts
                            .iter()
                            .filter(|r| r.result == VerificationResult::Pass)
                            .count(),
                        receipts
                            .iter()
                            .filter(|r| r.result == VerificationResult::Fail)
                            .count(),
                        receipts
                            .iter()
                            .filter(|r| r.result == VerificationResult::Inconclusive)
                            .count(),
                    ),
                    Confidence::new(risk.confidence.value() * 0.9),
                )
            } else {
                (
                    "Continue monitoring; no urgent remediation indicated.".to_string(),
                    format!(
                        "Risk severity is {}. Verified evaluations support a low-urgency posture. \
                         This recommendation remains a proposal.",
                        risk.severity.as_str()
                    ),
                    Confidence::new(0.65),
                )
            }
        } else {
            (
                    "Review Investigation evidence and decide next engineering action.".to_string(),
                    "No risk evaluation available; generic review recommendation based on verified assessments."
                        .to_string(),
                    Confidence::new(0.5),
                )
        };

        let mut recommendation = Recommendation::new(
            investigation_id,
            summary,
            rationale,
            evaluation_ids,
            verification_ids,
            confidence,
            provenance,
        );

        // Cite attached Recalled Context: prior outcomes may warn or note
        // success, but previous Recommendations are never auto-repeated
        // (RFC-017).
        let influence = self.historical_influence(investigation_id)?;
        Self::apply_historical_influence_to_recommendation(&mut recommendation, &influence);

        // Recommendations must never be auto-applied: status stays Proposed.
        debug_assert_eq!(
            recommendation.status,
            crate::domain::RecommendationStatus::Proposed
        );

        self.store.append_recommendation(&recommendation)?;
        Ok(vec![recommendation])
    }

    /// List Recommendations for an Investigation.
    pub fn list_recommendations(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<Vec<Recommendation>> {
        let _ = self.store.load_investigation(&investigation_id)?;
        self.store.list_recommendations(&investigation_id)
    }
}
