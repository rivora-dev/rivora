//! Serde round-trip integration tests for the public receipt schema.
//!
//! These tests verify that every receipt type that participates in the JSON
//! wire format can be serialized to JSON and deserialized back into an equal
//! value. They exercise the public API of the `rivora-receipts` crate through
//! its re-exported types and fixtures, treating the crate as a black box.

use rivora_receipts::fixtures;
use rivora_receipts::{
    ActionKind, ApprovalRequirement, Confidence, Evidence, EvidenceKind, EvidenceSource,
    HumanApproval, Receipt, ReceiptKind, ReceiptStatus, RiskLevel, SuggestedAction,
};
use rivora_types::NonEmptyString;

/// Convenience helper for building a `NonEmptyString` from a static string.
fn nes(s: &str) -> NonEmptyString {
    NonEmptyString::new(s).unwrap()
}

#[test]
fn receipt_round_trips_through_json() {
    let receipt = fixtures::observation_receipt();
    let json = serde_json::to_string(&receipt).unwrap();
    let back: Receipt = serde_json::from_str(&json).unwrap();

    assert_eq!(back.id, receipt.id);
    assert_eq!(back.kind, receipt.kind);
    assert_eq!(back.status, receipt.status);
    assert_eq!(back.subject, receipt.subject);
    assert_eq!(back.summary, receipt.summary);
    assert_eq!(back.evidence, receipt.evidence);
    assert_eq!(back.reasoning, receipt.reasoning);
    assert_eq!(back.confidence, receipt.confidence);
    assert_eq!(back.risk, receipt.risk);
    assert_eq!(back.provenance, receipt.provenance);
    assert_eq!(back.timestamps, receipt.timestamps);
    assert_eq!(back.version, receipt.version);
    assert_eq!(back.suggested_actions, receipt.suggested_actions);
    assert_eq!(back.inference, receipt.inference);
    assert_eq!(back.ability, receipt.ability);

    // Full structural equality: Receipt derives PartialEq, and the fixture
    // tests already rely on comparing whole receipts.
    assert_eq!(back, receipt);
}

#[test]
fn receipt_serializes_to_json_value() {
    let receipt = fixtures::recommendation_receipt();
    let value = serde_json::to_value(&receipt).unwrap();

    // Top-level object.
    assert!(value.is_object());

    // Required scalar fields are present.
    assert_eq!(value["id"], "receipt_fixture_recommendation_1");
    assert_eq!(value["kind"], "recommendation");
    assert_eq!(value["status"], "valid");

    // Structured sections are present and non-null.
    assert!(value["subject"].is_object());
    assert!(value["summary"].is_object());
    assert!(value["evidence"].is_array());
    assert!(!value["evidence"].as_array().unwrap().is_empty());
    assert!(value["reasoning"].is_array());
    assert!(!value["reasoning"].as_array().unwrap().is_empty());
    assert!(value["confidence"].is_object());
    assert!(value["risk"].is_object());
    assert!(value["provenance"].is_object());
    assert!(value["timestamps"].is_object());
    assert!(value["version"].is_object());

    // Optional convenience fields are present for this fixture.
    assert!(value["inference"].is_object());
    assert!(value["suggested_actions"].is_array());
    assert!(!value["suggested_actions"].as_array().unwrap().is_empty());

    // The confidence score is faithfully serialized as a JSON number.
    assert_eq!(value["confidence"]["score"], 0.92);
}

#[test]
fn evidence_round_trips_through_json() {
    let source = EvidenceSource {
        provider: nes("aws"),
        version: nes("0.1.0"),
    };
    let evidence = Evidence::new(
        EvidenceKind::Metric,
        source,
        "CPU spike",
        "CPU exceeded 90% for 5 minutes",
        "2026-06-25T12:00:00Z",
        0.8,
    )
    .unwrap()
    .with_raw_ref("arn:aws:ecs:us-east-1:123:service/api")
    .with_metadata(serde_json::json!({"region": "us-east-1", "az": 3}));

    let json = serde_json::to_string(&evidence).unwrap();
    let back: Evidence = serde_json::from_str(&json).unwrap();

    assert_eq!(back, evidence);
    assert_eq!(back.kind, EvidenceKind::Metric);
    assert_eq!(back.raw_ref, evidence.raw_ref);
    assert_eq!(back.metadata, evidence.metadata);
    assert_eq!(back.confidence_contribution, 0.8);
}

#[test]
fn confidence_round_trips_through_json() {
    let confidence = Confidence::new(0.85, "pattern-frequency-v1", "Limited data")
        .unwrap()
        .with_contributing_factors(vec![nes("stable pattern"), nes("multiple sources")])
        .with_limiting_factors(vec![nes("small sample")]);

    let json = serde_json::to_string(&confidence).unwrap();
    let back: Confidence = serde_json::from_str(&json).unwrap();

    assert_eq!(back, confidence);
    assert_eq!(back.score, 0.85);
    assert_eq!(back.method, confidence.method);
    assert_eq!(back.contributing_factors, confidence.contributing_factors);
    assert_eq!(back.limiting_factors, confidence.limiting_factors);
}

#[test]
fn suggested_action_round_trips_through_json() {
    let action = SuggestedAction::new(
        ActionKind::Rollback,
        "Rollback payment-service to v2.0.0",
        "Rollback the payment-service deployment to the previous stable version",
        "Latency and error rate return to pre-deployment levels",
        RiskLevel::Medium,
    )
    .unwrap()
    .with_scope(vec![nes("payment-service"), nes("us-east-1")])
    .with_rollback_strategy("Redeploy v2.1.0 if rollback does not resolve the issue")
    .with_human_approval(
        HumanApproval::new(
            ApprovalRequirement::Required,
            "on-call",
            "mutating action against production payment service",
        )
        .unwrap(),
    );

    let json = serde_json::to_string(&action).unwrap();
    let back: SuggestedAction = serde_json::from_str(&json).unwrap();

    assert_eq!(back, action);
    assert_eq!(back.kind, ActionKind::Rollback);
    assert!(back.mutates_infrastructure);
    assert_eq!(back.approval, ApprovalRequirement::Required);
    assert_eq!(back.scope, action.scope);
    assert_eq!(back.rollback_strategy, action.rollback_strategy);
    assert_eq!(back.human_approval, action.human_approval);
}

#[test]
fn all_fixture_receipts_round_trip() {
    let receipts = vec![
        fixtures::observation_receipt(),
        fixtures::incident_explanation_receipt(),
        fixtures::deployment_review_receipt(),
        fixtures::recommendation_receipt(),
        fixtures::ability_run_receipt(),
        fixtures::memory_candidate_created_receipt(),
        fixtures::memory_approved_receipt(),
        fixtures::recall_result_receipt(),
        fixtures::human_feedback_recorded_receipt(),
    ];

    for receipt in receipts {
        let json = serde_json::to_string(&receipt).unwrap();
        let back: Receipt = serde_json::from_str(&json).unwrap();
        assert_eq!(back, receipt);
    }
}

#[test]
fn memory_candidate_created_receipt_round_trips_through_json() {
    let receipt = fixtures::memory_candidate_created_receipt();
    let json = serde_json::to_string(&receipt).unwrap();
    let back: Receipt = serde_json::from_str(&json).unwrap();
    assert_eq!(back, receipt);
    assert_eq!(back.kind, ReceiptKind::MemoryCandidateCreated);
    assert_eq!(back.status, ReceiptStatus::Draft);
}

#[test]
fn memory_approved_receipt_round_trips_through_json() {
    let receipt = fixtures::memory_approved_receipt();
    let json = serde_json::to_string(&receipt).unwrap();
    let back: Receipt = serde_json::from_str(&json).unwrap();
    assert_eq!(back, receipt);
    assert_eq!(back.kind, ReceiptKind::MemoryApproved);
    assert_eq!(back.status, ReceiptStatus::Valid);
}

#[test]
fn recall_result_receipt_round_trips_through_json() {
    let receipt = fixtures::recall_result_receipt();
    let json = serde_json::to_string(&receipt).unwrap();
    let back: Receipt = serde_json::from_str(&json).unwrap();
    assert_eq!(back, receipt);
    assert_eq!(back.kind, ReceiptKind::RecallResult);
    assert_eq!(back.status, ReceiptStatus::Valid);
}

#[test]
fn human_feedback_recorded_receipt_round_trips_through_json() {
    let receipt = fixtures::human_feedback_recorded_receipt();
    let json = serde_json::to_string(&receipt).unwrap();
    let back: Receipt = serde_json::from_str(&json).unwrap();
    assert_eq!(back, receipt);
    assert_eq!(back.kind, ReceiptKind::HumanFeedbackRecorded);
    assert_eq!(back.status, ReceiptStatus::Valid);
}

#[test]
fn receipt_json_is_deterministic() {
    // A recommendation receipt exercises the richest set of fields (optional
    // inference/ability refs, mutating actions, scope, rollback strategy).
    let receipt = fixtures::recommendation_receipt();

    let first = serde_json::to_string(&receipt).unwrap();
    let second = serde_json::to_string(&receipt).unwrap();

    assert_eq!(first, second);

    // Re-deserializing and re-serializing must also be stable.
    let back: Receipt = serde_json::from_str(&first).unwrap();
    let third = serde_json::to_string(&back).unwrap();
    assert_eq!(first, third);
}

#[test]
fn receipt_kind_serializes_as_snake_case() {
    let cases = [
        (ReceiptKind::Observation, "observation"),
        (ReceiptKind::IncidentExplanation, "incident_explanation"),
        (ReceiptKind::DeploymentReview, "deployment_review"),
        (ReceiptKind::Recommendation, "recommendation"),
        (ReceiptKind::AbilityRun, "ability_run"),
        (ReceiptKind::DailySummary, "daily_summary"),
        (ReceiptKind::SystemDiagnostic, "system_diagnostic"),
        (
            ReceiptKind::MemoryCandidateCreated,
            "memory_candidate_created",
        ),
        (ReceiptKind::MemoryApproved, "memory_approved"),
        (ReceiptKind::MemoryRejected, "memory_rejected"),
        (ReceiptKind::MemoryCorrected, "memory_corrected"),
        (ReceiptKind::MemorySuperseded, "memory_superseded"),
        (ReceiptKind::RecallResult, "recall_result"),
        (
            ReceiptKind::HumanFeedbackRecorded,
            "human_feedback_recorded",
        ),
        (ReceiptKind::Unknown, "unknown"),
    ];

    for (kind, expected) in cases {
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, format!("\"{expected}\""));
        let back: ReceiptKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back, kind);
    }
}

#[test]
fn receipt_status_serializes_as_snake_case() {
    let cases = [
        (ReceiptStatus::Draft, "draft"),
        (ReceiptStatus::Valid, "valid"),
        (ReceiptStatus::Invalid, "invalid"),
        (ReceiptStatus::Superseded, "superseded"),
        (ReceiptStatus::Archived, "archived"),
    ];

    for (status, expected) in cases {
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, format!("\"{expected}\""));
        let back: ReceiptStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, status);
    }
}

#[test]
fn invalid_receipt_still_serializes() {
    // An invalid receipt fails *validation*, not serialization. Its struct
    // value must still round-trip through JSON so that it can be persisted,
    // inspected, or surfaced in tooling without surprising engineers.
    let receipt = fixtures::invalid_receipt();
    assert_eq!(receipt.status, ReceiptStatus::Invalid);
    assert!(receipt.evidence.is_empty());

    let json = serde_json::to_string(&receipt).unwrap();
    let back: Receipt = serde_json::from_str(&json).unwrap();

    assert_eq!(back, receipt);
    assert_eq!(back.status, ReceiptStatus::Invalid);
    assert!(back.evidence.is_empty());
    assert_eq!(back.kind, ReceiptKind::Observation);
    assert_eq!(back.id, receipt.id);
}
