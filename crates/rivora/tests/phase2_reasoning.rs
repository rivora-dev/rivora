//! Phase 2 end-to-end reasoning pipeline.

use std::sync::Arc;

use chrono::Utc;
use rivora::domain::{
    InvestigationStatus, ObservationKind, OutcomeDisposition, RecommendationStatus,
};
use rivora::runtime::observation::IngestObservationRequest;
use rivora::storage::LocalStore;
use rivora::Runtime;

fn runtime(dir: &std::path::Path) -> Runtime {
    Runtime::new(Arc::new(LocalStore::open(dir).unwrap()))
}

#[test]
fn observation_to_learning_pipeline() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());

    let inv = rt
        .create_investigation(
            "CI regression",
            Some("Pipeline failure on main".into()),
            "tester",
        )
        .unwrap();

    // Observation → Memory
    let ingest = rt
        .ingest_observation(IngestObservationRequest {
            investigation_id: inv.id,
            kind: ObservationKind::CheckResult,
            summary: "CI check failed on main".into(),
            payload: serde_json::json!({
                "check": "unit-tests",
                "status": "failure",
                "error": "assertion failed"
            }),
            source: "local".into(),
            observed_at: Utc::now(),
            idempotency_key: Some("ci-fail-1".into()),
            actor: "tester".into(),
        })
        .unwrap();
    assert!(!ingest.idempotent_replay);
    assert_eq!(ingest.observation.investigation_id, inv.id);
    assert_eq!(ingest.memory.investigation_id, inv.id);

    // Idempotent re-ingest
    let again = rt
        .ingest_observation(IngestObservationRequest {
            investigation_id: inv.id,
            kind: ObservationKind::CheckResult,
            summary: "CI check failed on main".into(),
            payload: serde_json::json!({"check": "unit-tests", "status": "failure"}),
            source: "local".into(),
            observed_at: Utc::now(),
            idempotency_key: Some("ci-fail-1".into()),
            actor: "tester".into(),
        })
        .unwrap();
    assert!(again.idempotent_replay);
    assert_eq!(again.observation.id, ingest.observation.id);

    rt.ingest_observation(IngestObservationRequest {
        investigation_id: inv.id,
        kind: ObservationKind::Commit,
        summary: "Latest commit on main".into(),
        payload: serde_json::json!({"sha": "abc123"}),
        source: "local".into(),
        observed_at: Utc::now(),
        idempotency_key: Some("commit-abc".into()),
        actor: "tester".into(),
    })
    .unwrap();

    let memory = rt.recall_memory(inv.id).unwrap();
    assert_eq!(memory.len(), 2);
    assert!(memory
        .windows(2)
        .all(|w| w[0].recorded_at <= w[1].recorded_at));

    let timeline = rt.generate_timeline(inv.id).unwrap();
    assert_eq!(timeline.len(), 2);

    // Knowledge
    let knowledge = rt.derive_knowledge(inv.id, "tester").unwrap();
    assert!(!knowledge.is_empty());
    for k in &knowledge {
        assert_eq!(k.investigation_id, inv.id);
        assert!(!k.supporting_memory_ids.is_empty());
        assert!(!k.derivation.method.is_empty());
    }
    assert!(knowledge
        .iter()
        .any(|k| matches!(k.kind, rivora::domain::KnowledgeKind::RiskSignal)));

    // Evaluation
    let evaluations = rt.evaluate_investigation(inv.id, "tester").unwrap();
    assert!(!evaluations.is_empty());
    for e in &evaluations {
        assert_eq!(e.investigation_id, inv.id);
        assert!(!e.explanation.is_empty());
        assert!(!e.supporting_knowledge_ids.is_empty() || !e.supporting_memory_ids.is_empty());
    }

    // Verification
    let receipts = rt.verify_all(inv.id, "tester").unwrap();
    assert_eq!(receipts.len(), evaluations.len());
    for r in &receipts {
        assert_eq!(r.investigation_id, inv.id);
        assert!(!r.reason.is_empty());
    }

    // Recommendation
    let recs = rt.generate_recommendation(inv.id, "tester").unwrap();
    assert!(!recs.is_empty());
    for rec in &recs {
        assert_eq!(rec.investigation_id, inv.id);
        assert_eq!(rec.status, RecommendationStatus::Proposed);
        assert!(!rec.evaluation_ids.is_empty());
        assert!(!rec.verification_ids.is_empty());
        assert!(!rec.rationale.is_empty());
    }

    // Learning
    let outcome = rt
        .record_outcome(rivora::runtime::learning::RecordOutcomeRequest {
            investigation_id: inv.id,
            recommendation_id: Some(recs[0].id),
            disposition: OutcomeDisposition::Accepted,
            notes: "Engineer accepted recommendation to investigate CI failure".into(),
            impact: Some("investigation opened".into()),
            actor: "tester".into(),
        })
        .unwrap();
    assert_eq!(outcome.investigation_id, inv.id);
    assert_eq!(outcome.recommendation_id, Some(recs[0].id));

    // History not rewritten: original memory still present
    let memory_after = rt.recall_memory(inv.id).unwrap();
    assert_eq!(memory_after.len(), 2);
    assert_eq!(memory_after[0].id, memory[0].id);

    // Survive reload
    let rt2 = runtime(dir.path());
    let reloaded = rt2.open_investigation(inv.id).unwrap();
    assert_eq!(reloaded.id, inv.id);
    assert_eq!(rt2.recall_memory(inv.id).unwrap().len(), 2);
    assert!(!rt2.list_knowledge(inv.id).unwrap().is_empty());
    assert!(!rt2.list_evaluations(inv.id).unwrap().is_empty());
    assert!(!rt2.list_verifications(inv.id).unwrap().is_empty());
    assert!(!rt2.list_recommendations(inv.id).unwrap().is_empty());
    assert!(!rt2.list_learning(inv.id).unwrap().is_empty());

    // Complete
    let done = rt2
        .complete_investigation(inv.id, Some("resolved".into()))
        .unwrap();
    assert_eq!(done.status, InvestigationStatus::Completed);
}

#[test]
fn malformed_observation_does_not_corrupt_state() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let inv = rt.create_investigation("safe", None, "t").unwrap();

    let err = rt
        .ingest_observation(IngestObservationRequest {
            investigation_id: inv.id,
            kind: ObservationKind::Event,
            summary: "   ".into(),
            payload: serde_json::json!({}),
            source: "test".into(),
            observed_at: Utc::now(),
            idempotency_key: None,
            actor: "t".into(),
        })
        .unwrap_err();
    assert!(matches!(err, rivora::RivoraError::Validation(_)));

    let loaded = rt.open_investigation(inv.id).unwrap();
    assert_eq!(loaded.status, InvestigationStatus::Created);
    assert!(rt.recall_memory(inv.id).unwrap().is_empty());
}
