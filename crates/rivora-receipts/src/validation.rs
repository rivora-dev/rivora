//! Validation rules for reliability receipts.
//!
//! Every receipt must pass validation before being surfaced to an engineer.
//! Invalid receipts are treated as engine failures.

use rivora_errors::RivoraError;

use crate::action::{ActionKind, ApprovalRequirement};
use crate::receipt::Receipt;

/// Validates a receipt against all canonical invariants.
///
/// # Returns
///
/// - `Ok(())` if the receipt is valid
/// - `Err(RivoraError::Receipt { .. })` describing the first invariant
///   violation found
///
/// # Invariants
///
/// 1. The receipt has a non-empty `id`.
/// 2. The receipt has at least one piece of evidence.
/// 3. The receipt has at least one reasoning step.
/// 4. Reasoning steps are ordered by `step` (strictly increasing from 1).
/// 5. Every mutating `SuggestedAction` has `ApprovalRequirement::Required`
///    or stronger.
/// 6. Every mutating `SuggestedAction` has a `rollback_strategy`.
/// 7. The `confidence.score` is in `[0.0, 1.0]`.
/// 8. The `confidence.method` is non-empty.
/// 9. The `confidence.uncertainty` is non-empty.
/// 10. The `risk.description` is non-empty.
/// 11. If the receipt status is `Valid`, all of the above must hold.
/// 12. If the receipt status is `Invalid`, validation must fail.
pub fn validate_receipt(receipt: &Receipt) -> Result<(), RivoraError> {
    // 1. id non-empty
    if receipt.id.as_str().is_empty() {
        return Err(RivoraError::receipt("id must not be empty"));
    }

    // 2. at least one piece of evidence
    if receipt.evidence.is_empty() {
        return Err(RivoraError::receipt(
            "evidence must not be empty — a receipt without evidence is invalid",
        ));
    }

    // 3. at least one reasoning step
    if receipt.reasoning.is_empty() {
        return Err(RivoraError::receipt(
            "reasoning must not be empty — a receipt must explain how it reached its conclusion",
        ));
    }

    // 4. reasoning steps are ordered (strictly increasing from 1)
    validate_reasoning_order(&receipt.reasoning)?;

    // 5 & 6. mutating actions must require approval and have a rollback strategy
    for (i, action) in receipt.suggested_actions.iter().enumerate() {
        if action.mutates_infrastructure {
            // Must require approval (at least Required; Blocked is also fine)
            if matches!(
                action.approval,
                ApprovalRequirement::NotRequired | ApprovalRequirement::Recommended
            ) {
                return Err(RivoraError::receipt(format!(
                    "suggested action #{} ({}) mutates infrastructure but has approval={:?}; mutating actions require ApprovalRequirement::Required or stronger",
                    i + 1,
                    action.title.as_str(),
                    action.approval
                )));
            }
            // Must have a rollback strategy
            if action.rollback_strategy.is_none() {
                return Err(RivoraError::receipt(format!(
                    "suggested action #{} ({}) mutates infrastructure but has no rollback_strategy",
                    i + 1,
                    action.title.as_str()
                )));
            }
        }
    }

    // 7. confidence score in [0.0, 1.0] — already enforced by Confidence::new,
    // but double-check in case the receipt was constructed differently.
    if !(0.0..=1.0).contains(&receipt.confidence.score) {
        return Err(RivoraError::receipt(format!(
            "confidence.score must be in [0.0, 1.0], got {}",
            receipt.confidence.score
        )));
    }

    // 8. confidence.method non-empty
    if receipt.confidence.method.as_str().is_empty() {
        return Err(RivoraError::receipt("confidence.method must not be empty"));
    }

    // 9. confidence.uncertainty non-empty
    if receipt.confidence.uncertainty.as_str().is_empty() {
        return Err(RivoraError::receipt(
            "confidence.uncertainty must not be empty",
        ));
    }

    // 10. risk.description non-empty
    if receipt.risk.description.as_str().is_empty() {
        return Err(RivoraError::receipt("risk.description must not be empty"));
    }

    // 11. if status is Valid, all above must hold (already checked above)
    // 12. if status is Invalid, validation must fail
    if matches!(receipt.status, crate::status::ReceiptStatus::Invalid) {
        return Err(RivoraError::receipt(
            "receipt status is Invalid; cannot validate",
        ));
    }

    Ok(())
}

/// Validates that reasoning steps are ordered by `step` (strictly increasing
/// starting from 1).
fn validate_reasoning_order(steps: &[crate::reasoning::ReasoningStep]) -> Result<(), RivoraError> {
    if steps.is_empty() {
        return Err(RivoraError::receipt("reasoning must not be empty"));
    }
    for (i, step) in steps.iter().enumerate() {
        let expected = u32::try_from(i + 1).unwrap_or(u32::MAX);
        if step.step != expected {
            return Err(RivoraError::receipt(format!(
                "reasoning step #{} has step number {}, expected {}",
                i + 1,
                step.step,
                expected
            )));
        }
    }
    Ok(())
}

/// Returns `true` if the given action kind is a mutating action.
///
/// Convenience re-export used by validation logic and callers.
#[must_use]
pub fn is_mutating_action_kind(kind: ActionKind) -> bool {
    kind.is_mutating()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::SuggestedAction;
    use crate::confidence::Confidence;
    use crate::evidence::{Evidence, EvidenceKind, EvidenceSource};
    use crate::kind::ReceiptKind;
    use crate::metadata::{ReceiptProvenance, ReceiptTimestamps, ReceiptVersion};
    use crate::reasoning::ReasoningStep;
    use crate::risk::{Risk, RiskLevel};
    use crate::status::ReceiptStatus;
    use crate::subject::{ReceiptSubject, ReceiptSummary};
    use rivora_core::ReceiptId;
    use rivora_types::{NonEmptyString, Version};

    fn minimal_valid_receipt() -> Receipt {
        let source = EvidenceSource {
            provider: NonEmptyString::new("aws").unwrap(),
            version: NonEmptyString::new("0.1.0").unwrap(),
        };
        let evidence = vec![Evidence::new(
            EvidenceKind::Metric,
            source,
            "CPU spike",
            "CPU exceeded 90%",
            "2026-06-25T12:00:00Z",
            0.8,
        )
        .unwrap()];
        let reasoning = vec![ReasoningStep::new(
            1,
            "Detect",
            "Anomaly detected",
            "Likely deploy-induced",
            0.3,
        )
        .unwrap()];
        Receipt::builder()
            .id(ReceiptId::new_unchecked("receipt_test_1"))
            .kind(ReceiptKind::IncidentExplanation)
            .status(ReceiptStatus::Draft)
            .subject(ReceiptSubject::new("service", "svc-1", "api-gateway").unwrap())
            .summary(ReceiptSummary::new("Latency spike", "Latency increased 3x").unwrap())
            .evidence(evidence)
            .reasoning(reasoning)
            .confidence(Confidence::new(0.85, "method-v1", "Limited data").unwrap())
            .risk(Risk::new(RiskLevel::Low, "Minor risk").unwrap())
            .provenance(ReceiptProvenance::new("adaptive-engine", "0.1.0").unwrap())
            .timestamps(ReceiptTimestamps::new("2026-06-25T12:00:00Z").unwrap())
            .version(ReceiptVersion::new(Version::new(1, 0, 0)))
            .build()
            .unwrap()
    }

    #[test]
    fn valid_receipt_passes() {
        let receipt = minimal_valid_receipt();
        assert!(validate_receipt(&receipt).is_ok());
    }

    #[test]
    fn empty_evidence_fails() {
        let mut receipt = minimal_valid_receipt();
        receipt.evidence.clear();
        let err = validate_receipt(&receipt).unwrap_err();
        assert!(err.to_string().contains("evidence must not be empty"));
    }

    #[test]
    fn empty_reasoning_fails() {
        let mut receipt = minimal_valid_receipt();
        receipt.reasoning.clear();
        let err = validate_receipt(&receipt).unwrap_err();
        assert!(err.to_string().contains("reasoning must not be empty"));
    }

    #[test]
    fn unordered_reasoning_fails() {
        let mut receipt = minimal_valid_receipt();
        receipt.reasoning = vec![
            ReasoningStep::new(2, "Step 2", "Second", "Output 2", 0.0).unwrap(),
            ReasoningStep::new(1, "Step 1", "First", "Output 1", 0.0).unwrap(),
        ];
        let err = validate_receipt(&receipt).unwrap_err();
        assert!(err.to_string().contains("reasoning step"));
    }

    #[test]
    fn mutating_action_without_required_approval_fails() {
        let mut receipt = minimal_valid_receipt();
        let action = SuggestedAction::new(
            ActionKind::Rollback,
            "Rollback",
            "Rollback to v1",
            "Restores service",
            RiskLevel::Medium,
        )
        .unwrap()
        .with_approval(ApprovalRequirement::NotRequired);
        receipt.suggested_actions.push(action);
        let err = validate_receipt(&receipt).unwrap_err();
        assert!(err.to_string().contains("approval"));
    }

    #[test]
    fn mutating_action_without_rollback_strategy_fails() {
        let mut receipt = minimal_valid_receipt();
        let action = SuggestedAction::new(
            ActionKind::Rollback,
            "Rollback",
            "Rollback to v1",
            "Restores service",
            RiskLevel::Medium,
        )
        .unwrap();
        // No rollback_strategy set
        receipt.suggested_actions.push(action);
        let err = validate_receipt(&receipt).unwrap_err();
        assert!(err.to_string().contains("rollback_strategy"));
    }

    #[test]
    fn mutating_action_with_approval_and_rollback_passes() {
        let mut receipt = minimal_valid_receipt();
        let action = SuggestedAction::new(
            ActionKind::Rollback,
            "Rollback",
            "Rollback to v1",
            "Restores service",
            RiskLevel::Medium,
        )
        .unwrap()
        .with_rollback_strategy("redeploy prior image");
        receipt.suggested_actions.push(action);
        assert!(validate_receipt(&receipt).is_ok());
    }

    #[test]
    fn read_only_action_without_approval_passes() {
        let mut receipt = minimal_valid_receipt();
        let action = SuggestedAction::new(
            ActionKind::Read,
            "View logs",
            "Show service logs",
            "Logs displayed",
            RiskLevel::Low,
        )
        .unwrap();
        receipt.suggested_actions.push(action);
        assert!(validate_receipt(&receipt).is_ok());
    }

    #[test]
    fn invalid_status_fails_validation() {
        let mut receipt = minimal_valid_receipt();
        receipt.status = ReceiptStatus::Invalid;
        let err = validate_receipt(&receipt).unwrap_err();
        assert!(err.to_string().contains("Invalid"));
    }

    #[test]
    fn empty_id_fails() {
        let mut receipt = minimal_valid_receipt();
        receipt.id = ReceiptId::new_unchecked("");
        let err = validate_receipt(&receipt).unwrap_err();
        assert!(err.to_string().contains("id must not be empty"));
    }

    #[test]
    fn unordered_reasoning_with_gap_fails() {
        let mut receipt = minimal_valid_receipt();
        receipt.reasoning = vec![
            ReasoningStep::new(1, "Step 1", "First", "Output 1", 0.0).unwrap(),
            ReasoningStep::new(3, "Step 3", "Third", "Output 3", 0.0).unwrap(),
        ];
        let err = validate_receipt(&receipt).unwrap_err();
        assert!(err.to_string().contains("step"));
    }
}
