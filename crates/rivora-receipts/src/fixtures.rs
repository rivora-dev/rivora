//! Sample receipt fixtures for golden snapshot tests and integration tests.
//!
//! All fixtures are deterministic: no random IDs, no time-dependent values.

use crate::action::{ActionKind, SuggestedAction};
use crate::confidence::Confidence;
use crate::evidence::{Evidence, EvidenceKind, EvidenceSource};
use crate::kind::ReceiptKind;
use crate::metadata::{
    AbilityRef, InferenceRef, ReceiptProvenance, ReceiptTimestamps, ReceiptVersion,
};
use crate::reasoning::ReasoningStep;
use crate::receipt::Receipt;
use crate::risk::{Risk, RiskLevel};
use crate::status::ReceiptStatus;
use crate::subject::{ReceiptSubject, ReceiptSummary};
use rivora_core::ReceiptId;
use rivora_types::{NonEmptyString, Version};

fn evidence_source() -> EvidenceSource {
    EvidenceSource {
        provider: NonEmptyString::new("aws").unwrap(),
        version: NonEmptyString::new("0.1.0").unwrap(),
    }
}

fn schema_version() -> ReceiptVersion {
    ReceiptVersion::new(Version::new(1, 0, 0))
}

fn provenance() -> ReceiptProvenance {
    ReceiptProvenance::new("adaptive-engine", "0.1.0").unwrap()
}

fn timestamps() -> ReceiptTimestamps {
    ReceiptTimestamps::new("2026-06-25T12:00:00Z").unwrap()
}

pub fn observation_receipt() -> Receipt {
    let evidence = vec![Evidence::new(
        EvidenceKind::Observation,
        evidence_source(),
        "Error rate anomaly",
        "Error rate for api-gateway exceeded 2% for 3 consecutive minutes",
        "2026-06-25T12:00:00Z",
        0.3,
    )
    .unwrap()];

    let reasoning = vec![ReasoningStep::new(
        1,
        "Detect anomaly",
        "Error rate crossed the 2% threshold for 3 consecutive minutes",
        "Anomaly confirmed; root cause unknown",
        0.2,
    )
    .unwrap()];

    Receipt::builder()
        .id(ReceiptId::new_unchecked("receipt_fixture_observation_1"))
        .kind(ReceiptKind::Observation)
        .status(ReceiptStatus::Draft)
        .subject(ReceiptSubject::new("service", "svc-api-gateway", "api-gateway").unwrap())
        .summary(
            ReceiptSummary::new(
                "Error rate anomaly detected",
                "Error rate exceeded 2% threshold on api-gateway",
            )
            .unwrap(),
        )
        .evidence(evidence)
        .reasoning(reasoning)
        .confidence(
            Confidence::new(
                0.3,
                "observation-threshold-v1",
                "Single observation; no historical comparison available",
            )
            .unwrap(),
        )
        .risk(Risk::new(RiskLevel::Low, "Observation only; no action proposed").unwrap())
        .provenance(provenance())
        .timestamps(timestamps())
        .version(schema_version())
        .build()
        .unwrap()
}

pub fn incident_explanation_receipt() -> Receipt {
    let source = evidence_source();

    let evidence = vec![
        Evidence::new(
            EvidenceKind::Metric,
            source.clone(),
            "Latency spike",
            "p99 latency increased from 80ms to 450ms over 5 minutes",
            "2026-06-25T11:55:00Z",
            0.6,
        )
        .unwrap(),
        Evidence::new(
            EvidenceKind::Log,
            source,
            "Error log entries",
            "500 errors logged from payment-service during the spike window",
            "2026-06-25T11:57:00Z",
            0.4,
        )
        .unwrap(),
    ];

    let reasoning = vec![
        ReasoningStep::new(
            1,
            "Identify latency anomaly",
            "p99 latency crossed the 200ms threshold at 11:55 UTC",
            "Latency anomaly confirmed",
            0.3,
        )
        .unwrap(),
        ReasoningStep::new(
            2,
            "Correlate error logs",
            "500 errors in payment-service logs coincide with the latency spike window",
            "Errors and latency are temporally correlated",
            0.25,
        )
        .unwrap(),
        ReasoningStep::new(
            3,
            "Form root cause hypothesis",
            "Error pattern suggests a downstream dependency failure in payment-service",
            "Likely caused by payment-service dependency degradation",
            0.15,
        )
        .unwrap(),
    ];

    let action = SuggestedAction::new(
        ActionKind::Read,
        "View payment-service dashboard",
        "Open the payment-service observability dashboard",
        "Engineer can inspect current service health",
        RiskLevel::Low,
    )
    .unwrap();

    let mut receipt = Receipt::builder()
        .id(ReceiptId::new_unchecked("receipt_fixture_incident_1"))
        .kind(ReceiptKind::IncidentExplanation)
        .status(ReceiptStatus::Draft)
        .subject(ReceiptSubject::new("service", "svc-payment", "payment-service").unwrap())
        .summary(
            ReceiptSummary::new(
                "Payment latency incident",
                "p99 latency spike correlated with 500 errors from payment-service",
            )
            .unwrap(),
        )
        .evidence(evidence)
        .reasoning(reasoning)
        .confidence(
            Confidence::new(
                0.55,
                "correlation-analysis-v1",
                "Limited historical baseline; correlation not yet confirmed as causation",
            )
            .unwrap(),
        )
        .risk(
            Risk::new(
                RiskLevel::Medium,
                "Service degradation affecting payment processing",
            )
            .unwrap(),
        )
        .provenance(provenance())
        .timestamps(timestamps())
        .version(schema_version())
        .build()
        .unwrap();
    receipt.suggested_actions = vec![action];
    receipt
}

pub fn deployment_review_receipt() -> Receipt {
    let source = evidence_source();

    let evidence = vec![
        Evidence::new(
            EvidenceKind::Deployment,
            source.clone(),
            "Deployment v2.1.0",
            "payment-service v2.1.0 deployed to production at 12:00 UTC",
            "2026-06-25T12:00:00Z",
            0.5,
        )
        .unwrap(),
        Evidence::new(
            EvidenceKind::Metric,
            source,
            "Health check metrics",
            "All health checks passed within 2 minutes of deployment",
            "2026-06-25T12:02:00Z",
            0.5,
        )
        .unwrap(),
    ];

    let reasoning = vec![
        ReasoningStep::new(
            1,
            "Verify deployment success",
            "Deployment v2.1.0 completed without errors at 12:00 UTC",
            "Deployment completed successfully",
            0.4,
        )
        .unwrap(),
        ReasoningStep::new(
            2,
            "Validate health checks",
            "All health checks passed within 2 minutes of deployment",
            "Deployment is healthy and stable",
            0.35,
        )
        .unwrap(),
    ];

    let action = SuggestedAction::new(
        ActionKind::Read,
        "View deployment details",
        "Open the deployment details for v2.1.0",
        "Engineer can review deployment configuration and rollout status",
        RiskLevel::Low,
    )
    .unwrap();

    let ability_ref = AbilityRef::new("deployment-validator", "1.0.0", "approved").unwrap();

    let mut receipt = Receipt::builder()
        .id(ReceiptId::new_unchecked("receipt_fixture_deployment_1"))
        .kind(ReceiptKind::DeploymentReview)
        .status(ReceiptStatus::Valid)
        .subject(ReceiptSubject::new("deployment", "dep-v2.1.0", "payment-service-v2.1.0").unwrap())
        .summary(
            ReceiptSummary::new(
                "Deployment review passed",
                "Deployment v2.1.0 passed all health checks and is stable",
            )
            .unwrap(),
        )
        .evidence(evidence)
        .reasoning(reasoning)
        .confidence(
            Confidence::new(
                0.85,
                "deployment-health-check-v1",
                "Health checks passed but long-term stability not yet observed",
            )
            .unwrap(),
        )
        .risk(Risk::new(RiskLevel::Low, "Deployment is stable; no issues detected").unwrap())
        .provenance(provenance().with_ability(ability_ref.clone()))
        .timestamps(timestamps())
        .version(schema_version())
        .build()
        .unwrap();
    receipt.suggested_actions = vec![action];
    receipt.ability = Some(ability_ref);
    receipt
}

pub fn recommendation_receipt() -> Receipt {
    let source = evidence_source();

    let evidence = vec![
        Evidence::new(
            EvidenceKind::Metric,
            source.clone(),
            "Sustained latency spike",
            "p99 latency remained above 400ms for 15 minutes after deployment",
            "2026-06-25T12:05:00Z",
            0.4,
        )
        .unwrap(),
        Evidence::new(
            EvidenceKind::Log,
            source.clone(),
            "Recurring 500 errors",
            "500 errors correlated with new deployment version v2.1.0",
            "2026-06-25T12:07:00Z",
            0.3,
        )
        .unwrap(),
        Evidence::new(
            EvidenceKind::Deployment,
            source,
            "Recent deployment v2.1.0",
            "Deployment v2.1.0 occurred 5 minutes before the latency spike began",
            "2026-06-25T12:00:00Z",
            0.3,
        )
        .unwrap(),
    ];

    let reasoning = vec![
        ReasoningStep::new(
            1,
            "Confirm latency regression",
            "p99 latency increased from 80ms to 450ms after deployment v2.1.0",
            "Latency regression confirmed and correlated with deployment",
            0.35,
        )
        .unwrap(),
        ReasoningStep::new(
            2,
            "Link errors to deployment",
            "500 errors began within 2 minutes of v2.1.0 deployment",
            "Errors are causally linked to the new deployment",
            0.3,
        )
        .unwrap(),
        ReasoningStep::new(
            3,
            "Evaluate rollback feasibility",
            "Previous version v2.0.0 was stable for 7 days with no latency issues",
            "Rollback to v2.0.0 is the recommended action",
            0.27,
        )
        .unwrap(),
    ];

    let action = SuggestedAction::new(
        ActionKind::Rollback,
        "Rollback payment-service to v2.0.0",
        "Rollback the payment-service deployment from v2.1.0 to the previous stable version v2.0.0",
        "Latency and error rate return to pre-deployment levels",
        RiskLevel::Medium,
    )
    .unwrap()
    .with_scope(vec![NonEmptyString::new("payment-service").unwrap()])
    .with_rollback_strategy("Redeploy v2.1.0 if rollback does not resolve the issue");

    let inference_ref = InferenceRef::new(
        "anthropic",
        "claude-opus-4",
        "20250101",
        0.0,
        "req_fixture_recommendation_1",
    )
    .unwrap();

    let mut receipt = Receipt::builder()
        .id(ReceiptId::new_unchecked("receipt_fixture_recommendation_1"))
        .kind(ReceiptKind::Recommendation)
        .status(ReceiptStatus::Valid)
        .subject(ReceiptSubject::new("service", "svc-payment", "payment-service").unwrap())
        .summary(
            ReceiptSummary::new(
                "Recommend rollback payment-service",
                "Rollback to v2.0.0 to resolve latency spike caused by v2.1.0 deployment",
            )
            .unwrap(),
        )
        .evidence(evidence)
        .reasoning(reasoning)
        .confidence(
            Confidence::new(
                0.92,
                "root-cause-analysis-v2",
                "High confidence based on correlated evidence but causation not fully confirmed",
            )
            .unwrap(),
        )
        .risk(
            Risk::new(
                RiskLevel::Medium,
                "Rollback may briefly interrupt payment processing",
            )
            .unwrap(),
        )
        .provenance(provenance().with_inference(inference_ref.clone()))
        .timestamps(timestamps())
        .version(schema_version())
        .build()
        .unwrap();
    receipt.suggested_actions = vec![action];
    receipt.inference = Some(inference_ref);
    receipt
}

pub fn ability_run_receipt() -> Receipt {
    let source = evidence_source();

    let evidence = vec![
        Evidence::new(
            EvidenceKind::Observation,
            source.clone(),
            "Service configuration observed",
            "Current replica count and resource limits recorded for api-gateway",
            "2026-06-25T12:00:00Z",
            0.5,
        )
        .unwrap(),
        Evidence::new(
            EvidenceKind::Configuration,
            source,
            "Deployment configuration",
            "Deployment configuration matches expected schema for ability validation",
            "2026-06-25T12:01:00Z",
            0.5,
        )
        .unwrap(),
    ];

    let reasoning = vec![
        ReasoningStep::new(
            1,
            "Load current configuration",
            "Retrieved current replica count and resource limits for api-gateway",
            "Configuration loaded successfully",
            0.35,
        )
        .unwrap(),
        ReasoningStep::new(
            2,
            "Validate against schema",
            "Configuration matches the expected deployment validation schema",
            "Configuration is valid",
            0.3,
        )
        .unwrap(),
    ];

    let action = SuggestedAction::new(
        ActionKind::Read,
        "View ability run results",
        "Display the full ability run output and validation results",
        "Engineer can review the ability execution details",
        RiskLevel::Low,
    )
    .unwrap();

    let inference_ref = InferenceRef::new(
        "anthropic",
        "claude-opus-4",
        "20250101",
        0.0,
        "req_fixture_ability_run_1",
    )
    .unwrap();

    let ability_ref =
        AbilityRef::new("deployment-validation-ability", "1.2.0", "approved").unwrap();

    let mut receipt = Receipt::builder()
        .id(ReceiptId::new_unchecked("receipt_fixture_ability_run_1"))
        .kind(ReceiptKind::AbilityRun)
        .status(ReceiptStatus::Valid)
        .subject(ReceiptSubject::new("service", "svc-api-gateway", "api-gateway").unwrap())
        .summary(
            ReceiptSummary::new(
                "Ability run completed",
                "Deployment validation ability completed successfully for api-gateway",
            )
            .unwrap(),
        )
        .evidence(evidence)
        .reasoning(reasoning)
        .confidence(
            Confidence::new(
                0.88,
                "ability-validation-v1",
                "Ability logic validated but external dependencies not verified",
            )
            .unwrap(),
        )
        .risk(Risk::new(RiskLevel::Low, "Read-only ability run; no mutations").unwrap())
        .provenance(
            provenance()
                .with_inference(inference_ref.clone())
                .with_ability(ability_ref.clone()),
        )
        .timestamps(timestamps())
        .version(schema_version())
        .build()
        .unwrap();
    receipt.suggested_actions = vec![action];
    receipt.inference = Some(inference_ref);
    receipt.ability = Some(ability_ref);
    receipt
}

pub fn memory_candidate_created_receipt() -> Receipt {
    let evidence = vec![Evidence::new(
        EvidenceKind::Observation,
        evidence_source(),
        "Observed latency pattern",
        "payment-service p99 latency spiked after the last 3 deployments to production",
        "2026-06-25T12:00:00Z",
        0.3,
    )
    .unwrap()];

    let reasoning = vec![ReasoningStep::new(
        1,
         "Extract memory candidate",
        "Observed a recurring correlation between production deployments and payment-service latency spikes",
        "Memory candidate created for the deploy-induced latency pattern",
        0.2,
    )
    .unwrap()];

    Receipt::builder()
        .id(ReceiptId::new_unchecked("receipt_fixture_memory_candidate_1"))
        .kind(ReceiptKind::MemoryCandidateCreated)
        .status(ReceiptStatus::Draft)
        .subject(
            ReceiptSubject::new(
                "memory",
                "mem-candidate-payment-latency-1",
                "Memory candidate: payment-service deploy latency pattern",
            )
            .unwrap(),
        )
        .summary(
            ReceiptSummary::new(
                "Memory candidate created",
                "A new memory candidate was extracted from observed deployment and latency correlations",
            )
            .unwrap(),
        )
        .evidence(evidence)
        .reasoning(reasoning)
        .confidence(
            Confidence::new(
                0.3,
                "memory-candidate-extraction-v1",
                "Single observation window; pattern has not yet been confirmed across multiple incidents",
            )
            .unwrap(),
        )
        .risk(
            Risk::new(
                RiskLevel::Low,
                "Memory candidate is read-only and requires human approval before being applied",
            )
            .unwrap(),
        )
        .provenance(provenance())
        .timestamps(timestamps())
        .version(schema_version())
        .build()
        .unwrap()
}

pub fn memory_approved_receipt() -> Receipt {
    let evidence = vec![Evidence::new(
        EvidenceKind::Annotation,
        evidence_source(),
        "Human approval of memory",
        "On-call engineer approved the memory candidate after reviewing corroborating incidents",
        "2026-06-25T13:00:00Z",
        0.5,
    )
    .unwrap()];

    let reasoning = vec![ReasoningStep::new(
        1,
        "Record human approval",
        "Engineer reviewed the memory candidate against three historical incidents and confirmed the pattern",
        "Memory candidate promoted to an approved memory",
        0.4,
    )
    .unwrap()];

    Receipt::builder()
        .id(ReceiptId::new_unchecked(
            "receipt_fixture_memory_approved_1",
        ))
        .kind(ReceiptKind::MemoryApproved)
        .status(ReceiptStatus::Valid)
        .subject(
            ReceiptSubject::new(
                "memory",
                "mem-payment-latency-1",
                "Approved memory: payment-service deploy latency pattern",
            )
            .unwrap(),
        )
        .summary(
            ReceiptSummary::new(
                "Memory approved by human",
                "A memory candidate was approved by an engineer and promoted to an active memory",
            )
            .unwrap(),
        )
        .evidence(evidence)
        .reasoning(reasoning)
        .confidence(
            Confidence::new(
                0.6,
                "human-approval-v1",
                "Approved by a single engineer; cross-team review has not yet been performed",
            )
            .unwrap(),
        )
        .risk(
            Risk::new(
                RiskLevel::Low,
                "Approved memory is applied as read-only context for future recommendations",
            )
            .unwrap(),
        )
        .provenance(provenance())
        .timestamps(timestamps())
        .version(schema_version())
        .build()
        .unwrap()
}

pub fn recall_result_receipt() -> Receipt {
    let evidence = vec![Evidence::new(
        EvidenceKind::Metric,
        evidence_source(),
        "Current latency metric",
        "payment-service p99 latency is 420ms, matching the previously memorized deployment-induced pattern",
        "2026-06-25T12:30:00Z",
        0.6,
    )
    .unwrap()];

    let reasoning = vec![
        ReasoningStep::new(
            1,
            "Query memory store",
            "Recall query matched the approved memory about payment-service deploy latency patterns",
            "One matching memory was returned by the recall query",
            0.4,
        )
        .unwrap(),
        ReasoningStep::new(
            2,
            "Score match against current observation",
            "Current latency of 420ms aligns with the memorized post-deployment latency range of 400-500ms",
            "High-confidence match between the current observation and the recalled memory",
            0.35,
        )
        .unwrap(),
    ];

    Receipt::builder()
        .id(ReceiptId::new_unchecked("receipt_fixture_recall_result_1"))
        .kind(ReceiptKind::RecallResult)
        .status(ReceiptStatus::Valid)
        .subject(ReceiptSubject::new("service", "svc-payment", "payment-service").unwrap())
        .summary(
            ReceiptSummary::new(
                "Recall result matched past memory",
                "Recall query returned a matching memory linking current latency to the deploy-induced pattern",
            )
            .unwrap(),
        )
        .evidence(evidence)
        .reasoning(reasoning)
        .confidence(
            Confidence::new(
                0.85,
                "recall-similarity-v1",
                "Strong similarity between the current metric and the recalled memory, but limited to a single matching memory",
            )
            .unwrap(),
        )
        .risk(
            Risk::new(
                RiskLevel::Low,
                "Recall result is informational and does not propose any infrastructure mutation",
            )
            .unwrap(),
        )
        .provenance(provenance())
        .timestamps(timestamps())
        .version(schema_version())
        .build()
        .unwrap()
}

pub fn human_feedback_recorded_receipt() -> Receipt {
    let evidence = vec![Evidence::new(
        EvidenceKind::Observation,
        evidence_source(),
        "Engineer feedback observation",
        "Engineer marked the rollback recommendation as helpful and confirmed it resolved the incident",
        "2026-06-25T14:00:00Z",
        0.5,
    )
    .unwrap()];

    let reasoning = vec![ReasoningStep::new(
        1,
        "Capture human feedback",
        "Engineer provided explicit feedback that the recommendation was accurate and the rollback resolved the latency incident",
        "Positive feedback recorded against the recommendation and its associated memory",
        0.4,
    )
    .unwrap()];

    Receipt::builder()
        .id(ReceiptId::new_unchecked(
            "receipt_fixture_human_feedback_recorded_1",
        ))
        .kind(ReceiptKind::HumanFeedbackRecorded)
        .status(ReceiptStatus::Valid)
        .subject(
            ReceiptSubject::new(
                "memory",
                "mem-payment-latency-1",
                "Memory: payment-service deploy latency pattern",
            )
            .unwrap(),
        )
        .summary(
            ReceiptSummary::new(
                "Human feedback recorded",
                "Engineer feedback was captured and linked to the recommendation and its supporting memory",
            )
            .unwrap(),
        )
        .evidence(evidence)
        .reasoning(reasoning)
        .confidence(
            Confidence::new(
                0.9,
                "human-feedback-v1",
                "Direct, explicit feedback from the engineer who applied the recommendation",
            )
            .unwrap(),
        )
        .risk(
            Risk::new(
                RiskLevel::Low,
                "Feedback recording is read-only and does not mutate infrastructure",
            )
            .unwrap(),
        )
        .provenance(provenance())
        .timestamps(timestamps())
        .version(schema_version())
        .build()
        .unwrap()
}

pub fn invalid_receipt() -> Receipt {
    let evidence = vec![Evidence::new(
        EvidenceKind::Observation,
        evidence_source(),
        "Placeholder evidence",
        "This evidence will be removed to create an invalid receipt",
        "2026-06-25T12:00:00Z",
        0.5,
    )
    .unwrap()];

    let reasoning = vec![ReasoningStep::new(
        1,
        "Placeholder step",
        "Placeholder reasoning for invalid receipt",
        "Placeholder conclusion",
        0.0,
    )
    .unwrap()];

    let mut receipt = Receipt::builder()
        .id(ReceiptId::new_unchecked("receipt_fixture_invalid_1"))
        .kind(ReceiptKind::Observation)
        .status(ReceiptStatus::Draft)
        .subject(ReceiptSubject::new("service", "svc-invalid", "invalid-service").unwrap())
        .summary(
            ReceiptSummary::new(
                "Invalid receipt fixture",
                "This receipt has empty evidence and should fail validation",
            )
            .unwrap(),
        )
        .evidence(evidence)
        .reasoning(reasoning)
        .confidence(Confidence::new(0.5, "placeholder-v1", "Placeholder uncertainty").unwrap())
        .risk(Risk::new(RiskLevel::Low, "Placeholder risk").unwrap())
        .provenance(provenance())
        .timestamps(timestamps())
        .version(schema_version())
        .build()
        .unwrap();
    receipt.evidence.clear();
    receipt.status = ReceiptStatus::Invalid;
    receipt
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::ApprovalRequirement;
    use crate::confidence::ConfidenceLevel;
    use crate::validation::validate_receipt;

    #[test]
    fn observation_receipt_has_expected_shape() {
        let receipt = observation_receipt();
        assert_eq!(receipt.kind, ReceiptKind::Observation);
        assert_eq!(receipt.status, ReceiptStatus::Draft);
        assert!(receipt.has_evidence());
        assert!(receipt.has_reasoning());
        assert_eq!(receipt.evidence.len(), 1);
        assert_eq!(receipt.reasoning.len(), 1);
        assert_eq!(receipt.confidence.level, ConfidenceLevel::Low);
        assert_eq!(receipt.risk.level, RiskLevel::Low);
        assert!(receipt.suggested_actions.is_empty());
        assert!(receipt.inference.is_none());
        assert!(receipt.ability.is_none());
    }

    #[test]
    fn incident_explanation_receipt_has_expected_shape() {
        let receipt = incident_explanation_receipt();
        assert_eq!(receipt.kind, ReceiptKind::IncidentExplanation);
        assert_eq!(receipt.status, ReceiptStatus::Draft);
        assert!(receipt.has_evidence());
        assert!(receipt.has_reasoning());
        assert_eq!(receipt.evidence.len(), 2);
        assert_eq!(receipt.reasoning.len(), 3);
        assert_eq!(receipt.confidence.level, ConfidenceLevel::Medium);
        assert_eq!(receipt.risk.level, RiskLevel::Medium);
        assert_eq!(receipt.suggested_actions.len(), 1);
        assert!(!receipt.suggested_actions[0].mutates_infrastructure);
        assert!(receipt.inference.is_none());
        assert!(receipt.ability.is_none());
    }

    #[test]
    fn deployment_review_receipt_has_expected_shape() {
        let receipt = deployment_review_receipt();
        assert_eq!(receipt.kind, ReceiptKind::DeploymentReview);
        assert_eq!(receipt.status, ReceiptStatus::Valid);
        assert!(receipt.has_evidence());
        assert!(receipt.has_reasoning());
        assert_eq!(receipt.evidence.len(), 2);
        assert_eq!(receipt.reasoning.len(), 2);
        assert_eq!(receipt.confidence.level, ConfidenceLevel::High);
        assert_eq!(receipt.risk.level, RiskLevel::Low);
        assert_eq!(receipt.suggested_actions.len(), 1);
        assert!(!receipt.suggested_actions[0].mutates_infrastructure);
        assert!(receipt.provenance.ability.is_some());
        assert!(receipt.ability.is_some());
        assert!(receipt.inference.is_none());
    }

    #[test]
    fn recommendation_receipt_has_expected_shape() {
        let receipt = recommendation_receipt();
        assert_eq!(receipt.kind, ReceiptKind::Recommendation);
        assert_eq!(receipt.status, ReceiptStatus::Valid);
        assert!(receipt.has_evidence());
        assert!(receipt.has_reasoning());
        assert_eq!(receipt.evidence.len(), 3);
        assert_eq!(receipt.reasoning.len(), 3);
        assert_eq!(receipt.confidence.score, 0.92);
        assert_eq!(receipt.confidence.level, ConfidenceLevel::High);
        assert_eq!(receipt.risk.level, RiskLevel::Medium);
        assert_eq!(receipt.suggested_actions.len(), 1);
        assert!(receipt.suggested_actions[0].mutates_infrastructure);
        assert_eq!(receipt.suggested_actions[0].kind, ActionKind::Rollback);
        assert_eq!(
            receipt.suggested_actions[0].approval,
            ApprovalRequirement::Required
        );
        assert!(receipt.suggested_actions[0].rollback_strategy.is_some());
        assert!(receipt.provenance.inference.is_some());
        assert!(receipt.inference.is_some());
        assert!(receipt.ability.is_none());
    }

    #[test]
    fn ability_run_receipt_has_expected_shape() {
        let receipt = ability_run_receipt();
        assert_eq!(receipt.kind, ReceiptKind::AbilityRun);
        assert_eq!(receipt.status, ReceiptStatus::Valid);
        assert!(receipt.has_evidence());
        assert!(receipt.has_reasoning());
        assert_eq!(receipt.evidence.len(), 2);
        assert_eq!(receipt.reasoning.len(), 2);
        assert_eq!(receipt.confidence.level, ConfidenceLevel::High);
        assert_eq!(receipt.risk.level, RiskLevel::Low);
        assert_eq!(receipt.suggested_actions.len(), 1);
        assert!(!receipt.suggested_actions[0].mutates_infrastructure);
        assert!(receipt.provenance.inference.is_some());
        assert!(receipt.provenance.ability.is_some());
        assert!(receipt.inference.is_some());
        assert!(receipt.ability.is_some());
    }

    #[test]
    fn memory_candidate_created_receipt_has_expected_shape() {
        let receipt = memory_candidate_created_receipt();
        assert_eq!(receipt.kind, ReceiptKind::MemoryCandidateCreated);
        assert_eq!(receipt.status, ReceiptStatus::Draft);
        assert!(receipt.has_evidence());
        assert!(receipt.has_reasoning());
        assert_eq!(receipt.evidence.len(), 1);
        assert_eq!(receipt.reasoning.len(), 1);
        assert_eq!(receipt.confidence.level, ConfidenceLevel::Low);
        assert_eq!(receipt.risk.level, RiskLevel::Low);
        assert!(receipt.suggested_actions.is_empty());
        assert!(receipt.inference.is_none());
        assert!(receipt.ability.is_none());
    }

    #[test]
    fn memory_approved_receipt_has_expected_shape() {
        let receipt = memory_approved_receipt();
        assert_eq!(receipt.kind, ReceiptKind::MemoryApproved);
        assert_eq!(receipt.status, ReceiptStatus::Valid);
        assert!(receipt.has_evidence());
        assert!(receipt.has_reasoning());
        assert_eq!(receipt.evidence.len(), 1);
        assert_eq!(receipt.reasoning.len(), 1);
        assert_eq!(receipt.confidence.level, ConfidenceLevel::Medium);
        assert_eq!(receipt.risk.level, RiskLevel::Low);
        assert!(receipt.suggested_actions.is_empty());
        assert!(receipt.inference.is_none());
        assert!(receipt.ability.is_none());
    }

    #[test]
    fn recall_result_receipt_has_expected_shape() {
        let receipt = recall_result_receipt();
        assert_eq!(receipt.kind, ReceiptKind::RecallResult);
        assert_eq!(receipt.status, ReceiptStatus::Valid);
        assert!(receipt.has_evidence());
        assert!(receipt.has_reasoning());
        assert_eq!(receipt.evidence.len(), 1);
        assert_eq!(receipt.reasoning.len(), 2);
        assert_eq!(receipt.confidence.level, ConfidenceLevel::High);
        assert_eq!(receipt.risk.level, RiskLevel::Low);
        assert!(receipt.suggested_actions.is_empty());
        assert!(receipt.inference.is_none());
        assert!(receipt.ability.is_none());
    }

    #[test]
    fn human_feedback_recorded_receipt_has_expected_shape() {
        let receipt = human_feedback_recorded_receipt();
        assert_eq!(receipt.kind, ReceiptKind::HumanFeedbackRecorded);
        assert_eq!(receipt.status, ReceiptStatus::Valid);
        assert!(receipt.has_evidence());
        assert!(receipt.has_reasoning());
        assert_eq!(receipt.evidence.len(), 1);
        assert_eq!(receipt.reasoning.len(), 1);
        assert_eq!(receipt.confidence.level, ConfidenceLevel::High);
        assert_eq!(receipt.risk.level, RiskLevel::Low);
        assert!(receipt.suggested_actions.is_empty());
        assert!(receipt.inference.is_none());
        assert!(receipt.ability.is_none());
    }

    #[test]
    fn invalid_receipt_fails_validation() {
        let receipt = invalid_receipt();
        assert_eq!(receipt.kind, ReceiptKind::Observation);
        assert_eq!(receipt.status, ReceiptStatus::Invalid);
        assert!(!receipt.has_evidence());
        assert!(receipt.has_reasoning());
        let err = validate_receipt(&receipt).unwrap_err();
        assert!(err.to_string().contains("evidence must not be empty"));
    }

    #[test]
    fn valid_fixtures_pass_validation() {
        assert!(validate_receipt(&observation_receipt()).is_ok());
        assert!(validate_receipt(&incident_explanation_receipt()).is_ok());
        assert!(validate_receipt(&deployment_review_receipt()).is_ok());
        assert!(validate_receipt(&recommendation_receipt()).is_ok());
        assert!(validate_receipt(&ability_run_receipt()).is_ok());
        assert!(validate_receipt(&memory_candidate_created_receipt()).is_ok());
        assert!(validate_receipt(&memory_approved_receipt()).is_ok());
        assert!(validate_receipt(&recall_result_receipt()).is_ok());
        assert!(validate_receipt(&human_feedback_recorded_receipt()).is_ok());
    }

    #[test]
    fn fixture_ids_are_deterministic() {
        assert_eq!(
            observation_receipt().id.as_str(),
            "receipt_fixture_observation_1"
        );
        assert_eq!(
            incident_explanation_receipt().id.as_str(),
            "receipt_fixture_incident_1"
        );
        assert_eq!(
            deployment_review_receipt().id.as_str(),
            "receipt_fixture_deployment_1"
        );
        assert_eq!(
            recommendation_receipt().id.as_str(),
            "receipt_fixture_recommendation_1"
        );
        assert_eq!(
            ability_run_receipt().id.as_str(),
            "receipt_fixture_ability_run_1"
        );
        assert_eq!(
            memory_candidate_created_receipt().id.as_str(),
            "receipt_fixture_memory_candidate_1"
        );
        assert_eq!(
            memory_approved_receipt().id.as_str(),
            "receipt_fixture_memory_approved_1"
        );
        assert_eq!(
            recall_result_receipt().id.as_str(),
            "receipt_fixture_recall_result_1"
        );
        assert_eq!(
            human_feedback_recorded_receipt().id.as_str(),
            "receipt_fixture_human_feedback_recorded_1"
        );
        assert_eq!(invalid_receipt().id.as_str(), "receipt_fixture_invalid_1");
    }

    #[test]
    fn fixtures_are_deterministic() {
        assert_eq!(observation_receipt(), observation_receipt());
        assert_eq!(
            incident_explanation_receipt(),
            incident_explanation_receipt()
        );
        assert_eq!(deployment_review_receipt(), deployment_review_receipt());
        assert_eq!(recommendation_receipt(), recommendation_receipt());
        assert_eq!(ability_run_receipt(), ability_run_receipt());
        assert_eq!(
            memory_candidate_created_receipt(),
            memory_candidate_created_receipt()
        );
        assert_eq!(memory_approved_receipt(), memory_approved_receipt());
        assert_eq!(recall_result_receipt(), recall_result_receipt());
        assert_eq!(
            human_feedback_recorded_receipt(),
            human_feedback_recorded_receipt()
        );
        assert_eq!(invalid_receipt(), invalid_receipt());
    }

    #[test]
    fn recommendation_receipt_has_mutating_action() {
        assert!(recommendation_receipt().has_mutating_actions());
    }

    #[test]
    fn non_recommendation_receipts_have_no_mutating_actions() {
        assert!(!observation_receipt().has_mutating_actions());
        assert!(!incident_explanation_receipt().has_mutating_actions());
        assert!(!deployment_review_receipt().has_mutating_actions());
        assert!(!ability_run_receipt().has_mutating_actions());
        assert!(!memory_candidate_created_receipt().has_mutating_actions());
        assert!(!memory_approved_receipt().has_mutating_actions());
        assert!(!recall_result_receipt().has_mutating_actions());
        assert!(!human_feedback_recorded_receipt().has_mutating_actions());
    }
}
