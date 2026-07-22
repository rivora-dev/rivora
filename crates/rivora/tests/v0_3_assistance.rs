//! v0.3 Phase 3 — Explainable Engineering Assistance (RFC-019).

use std::sync::Arc;

use chrono::Utc;
use rivora::domain::{HypothesisStatus, ObservationKind, ReadinessStatus};
use rivora::storage::LocalStore;
use rivora::{CapabilityService, Runtime};

fn setup() -> (CapabilityService, rivora::InvestigationId) {
    let dir = tempfile::tempdir().unwrap().keep();
    let store = Arc::new(LocalStore::open(dir).unwrap());
    let caps = CapabilityService::new(Arc::new(Runtime::new(store)));
    let inv = caps
        .create_investigation("assistance case", None, "tester")
        .unwrap();
    caps.ingest_observation(
        inv.id,
        ObservationKind::WorkflowRun,
        "CI workflow failed on deploy job",
        serde_json::json!({"conclusion": "failure", "name": "deploy"}),
        "github_actions",
        Utc::now(),
        Some("assist-ci".into()),
        "tester",
    )
    .unwrap();
    caps.ingest_observation(
        inv.id,
        ObservationKind::Observability,
        "Sentry error alert: timeout in payment handler",
        serde_json::json!({"level": "error", "title": "timeout"}),
        "sentry",
        Utc::now(),
        Some("assist-sentry".into()),
        "tester",
    )
    .unwrap();
    caps.ingest_observation(
        inv.id,
        ObservationKind::Infrastructure,
        "Kubernetes pod api-1 unhealthy phase=Failed",
        serde_json::json!({"phase": "Failed"}),
        "kubernetes",
        Utc::now(),
        Some("assist-k8s".into()),
        "tester",
    )
    .unwrap();
    (caps, inv.id)
}

#[test]
fn hypotheses_are_ranked_with_evidence() {
    let (caps, id) = setup();
    let hyps = caps.generate_hypotheses(id, "tester").unwrap();
    assert!(!hyps.is_empty());
    assert_eq!(hyps[0].rank, 1);
    for h in &hyps {
        assert!(!h.statement.is_empty());
        assert!(!h.derivation_method.is_empty());
        assert!(
            !matches!(h.status, HypothesisStatus::Verified),
            "unverified hypotheses must not be Verified without receipts"
        );
    }
    // Supporting or gap evidence should be present for leading hyp.
    assert!(
        !hyps[0].supporting_evidence.is_empty()
            || hyps[0].derivation_method.contains("evidence_gap")
    );
}

#[test]
fn next_verification_is_explainable() {
    let (caps, id) = setup();
    let suggestions = caps.recommend_next_verification(id, "tester").unwrap();
    assert!(!suggestions.is_empty());
    let s = &suggestions[0];
    assert!(!s.claim.is_empty());
    assert!(!s.reason.is_empty());
    assert!(!s.method.is_empty());
    assert!(s.estimated_confidence_impact > 0.0);
}

#[test]
fn deployment_readiness_is_inspectable() {
    let (caps, id) = setup();
    let readiness = caps.assess_deployment_readiness(id, "tester").unwrap();
    assert!(!readiness.dimensions.is_empty());
    // Failure observations should force Hold or Inspect, not Ready.
    assert!(
        matches!(
            readiness.status,
            ReadinessStatus::Hold | ReadinessStatus::Inspect
        ),
        "status={}",
        readiness.status.as_str()
    );
    assert!(!readiness.recommendation_summary.is_empty());
    assert!(!readiness.supporting_evidence.is_empty() || !readiness.blockers.is_empty());
}

#[test]
fn risk_forecast_has_categories_and_mitigations() {
    let (caps, id) = setup();
    let forecast = caps.forecast_risk(id, "tester").unwrap();
    assert!(!forecast.items.is_empty());
    for item in &forecast.items {
        assert!(!item.explanation.is_empty());
        assert!(!item.mitigation.is_empty());
    }
}

#[test]
fn root_cause_guidance_is_probabilistic() {
    let (caps, id) = setup();
    let guidance = caps.generate_root_cause_guidance(id, "tester").unwrap();
    assert!(!guidance.guidance.is_empty());
    assert!(
        guidance.guidance.to_lowercase().contains("probabilistic")
            || guidance.guidance.to_lowercase().contains("not a verified")
            || !guidance.leading_hypothesis_ids.is_empty()
    );
    assert!(!guidance.verification_order.is_empty() || !guidance.known_gaps.is_empty());
}

#[test]
fn recommendations_are_prioritized_with_factors() {
    let (caps, id) = setup();
    let _ = caps.run_full_pipeline(id, "tester").unwrap();
    let ranked = caps.prioritize_recommendations(id, "tester").unwrap();
    assert!(!ranked.is_empty());
    assert_eq!(ranked[0].rank, 1);
    assert!(!ranked[0].factors.is_empty());
    for f in &ranked[0].factors {
        assert!(!f.name.is_empty());
        assert!(!f.explanation.is_empty());
    }
    assert!(ranked[0].explanation.contains("Proposal") || ranked[0].score > 0.0);
}

#[test]
fn engineering_report_from_runtime_data() {
    let (caps, id) = setup();
    let _ = caps.run_full_pipeline(id, "tester").unwrap();
    let _ = caps.generate_hypotheses(id, "tester").unwrap();
    let _ = caps.assess_deployment_readiness(id, "tester").unwrap();
    let report = caps.generate_engineering_report(id, "tester").unwrap();
    assert!(!report.sections.is_empty());
    assert!(report.markdown.contains("Engineering Report") || report.markdown.contains("#"));
    assert!(report.markdown.contains("proposal") || report.markdown.contains("Proposal"));
    let listed = caps.list_engineering_reports(id).unwrap();
    assert_eq!(listed.len(), 1);
}

#[test]
fn summarize_investigation_state() {
    let (caps, id) = setup();
    let summary = caps.summarize_investigation_state(id, "tester").unwrap();
    assert_eq!(summary.investigation_id, id);
    assert!(summary.counts.memory >= 1);
    assert!(!summary.summary.is_empty());
}

#[test]
fn supporting_and_contradicting_evidence_visible_on_hypotheses() {
    let (caps, id) = setup();
    // Create low-risk evaluations then hypotheses.
    let _ = caps.derive_knowledge(id, "tester").unwrap();
    // Also add a benign observation so knowledge is mixed.
    caps.ingest_observation(
        id,
        ObservationKind::Event,
        "routine health check ok",
        serde_json::json!({"ok": true}),
        "test",
        Utc::now(),
        Some("benign".into()),
        "tester",
    )
    .unwrap();
    let hyps = caps.generate_hypotheses(id, "tester").unwrap();
    assert!(!hyps.is_empty());
    // Fields exist and are serializable.
    let json = serde_json::to_value(&hyps[0]).unwrap();
    assert!(json.get("supporting_evidence").is_some());
    assert!(json.get("contradicting_evidence").is_some());
}
