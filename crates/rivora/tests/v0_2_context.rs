//! v0.2 Phase 3 — Recalled Context and Reusable Knowledge (RFC-017).

use std::path::Path;
use std::sync::Arc;

use chrono::Utc;
use rivora::domain::{
    Investigation, InvestigationId, ObjectId, Observation, ObservationKind, OutcomeDisposition,
    RecalledContextState,
};
use rivora::runtime::observation::IngestObservationRequest;
use rivora::storage::LocalStore;
use rivora::{CapabilityService, RivoraError, Runtime};

fn runtime(dir: &Path) -> Runtime {
    Runtime::new(Arc::new(LocalStore::open(dir).unwrap()))
}

fn ingest(
    rt: &Runtime,
    investigation_id: InvestigationId,
    kind: ObservationKind,
    summary: &str,
    payload: serde_json::Value,
    source: &str,
) -> Observation {
    rt.ingest_observation(IngestObservationRequest {
        investigation_id,
        kind,
        summary: summary.into(),
        payload,
        source: source.into(),
        observed_at: Utc::now(),
        idempotency_key: None,
        actor: "tester".into(),
    })
    .unwrap()
    .observation
}

trait PipelineExt {
    fn run_full_pipeline_for_test(&self, id: InvestigationId) -> Vec<ObjectId>;
}

impl PipelineExt for Runtime {
    fn run_full_pipeline_for_test(&self, id: InvestigationId) -> Vec<ObjectId> {
        let _ = self.derive_knowledge(id, "tester").unwrap();
        let _ = self.evaluate_investigation(id, "tester").unwrap();
        let _ = self.verify_all(id, "tester").unwrap();
        self.generate_recommendation(id, "tester")
            .unwrap()
            .into_iter()
            .map(|r| r.id)
            .collect()
    }
}

/// Shared fixture: two completed related Investigations, one open current.
struct Scenario {
    a: Investigation,
    b: Investigation,
    c: Investigation,
}

fn scenario(rt: &Runtime) -> Scenario {
    let a = rt
        .create_investigation("Deploy regression in acme app", None, "tester")
        .unwrap();
    ingest(
        rt,
        a.id,
        ObservationKind::Repository,
        "Repository metadata for `acme/app`",
        serde_json::json!({"full_name": "acme/app"}),
        "github",
    );
    ingest(
        rt,
        a.id,
        ObservationKind::CheckResult,
        "Check build failed after deploy",
        serde_json::json!({"name": "build", "conclusion": "failure"}),
        "github",
    );
    let recs = rt.run_full_pipeline_for_test(a.id);
    rt.record_outcome(rivora::runtime::learning::RecordOutcomeRequest {
        investigation_id: a.id,
        recommendation_id: Some(recs[0]),
        disposition: OutcomeDisposition::Successful,
        notes: "rollback resolved the regression".into(),
        impact: Some("deploys recovered".into()),
        actor: "tester".into(),
    })
    .unwrap();
    rt.complete_investigation(a.id, Some("resolved".into()))
        .unwrap();

    let b = rt
        .create_investigation("CI build broken in acme app", None, "tester")
        .unwrap();
    ingest(
        rt,
        b.id,
        ObservationKind::Repository,
        "Repository metadata for `acme/app`",
        serde_json::json!({"full_name": "acme/app"}),
        "github",
    );
    ingest(
        rt,
        b.id,
        ObservationKind::CheckResult,
        "Check build failed on main",
        serde_json::json!({"name": "build", "conclusion": "failure"}),
        "github",
    );
    let recs = rt.run_full_pipeline_for_test(b.id);
    rt.record_outcome(rivora::runtime::learning::RecordOutcomeRequest {
        investigation_id: b.id,
        recommendation_id: Some(recs[0]),
        disposition: OutcomeDisposition::Unsuccessful,
        notes: "monitoring alone did not restore builds".into(),
        impact: None,
        actor: "tester".into(),
    })
    .unwrap();
    rt.complete_investigation(b.id, Some("closed".into()))
        .unwrap();

    let c = rt
        .create_investigation("New acme app build failure", None, "tester")
        .unwrap();
    ingest(
        rt,
        c.id,
        ObservationKind::Repository,
        "Repository metadata for `acme/app`",
        serde_json::json!({"full_name": "acme/app"}),
        "github",
    );
    ingest(
        rt,
        c.id,
        ObservationKind::CheckResult,
        "Check build failed after canary",
        serde_json::json!({"name": "build", "conclusion": "failure"}),
        "github",
    );

    Scenario { a, b, c }
}

#[test]
fn suggest_and_attach_recalled_context_preserves_provenance() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let s = scenario(&rt);

    rt.refresh_relationships(s.c.id, "tester").unwrap();
    let suggested = rt.suggest_recalled_context(s.c.id, "tester").unwrap();
    assert!(
        !suggested.is_empty(),
        "expected suggested context from related investigations"
    );
    let source_ids: Vec<_> = suggested
        .iter()
        .map(|c| c.source_investigation_id)
        .collect();
    assert!(source_ids.contains(&s.a.id) || source_ids.contains(&s.b.id));

    let first = &suggested[0];
    assert_eq!(first.investigation_id, s.c.id);
    assert_ne!(first.source_investigation_id, s.c.id);
    assert_eq!(first.state, RecalledContextState::Suggested);
    assert!(!first.reason.is_empty());
    assert!(!first.explanation.is_empty());
    assert_eq!(
        first.provenance.capability.as_deref(),
        Some("suggest_recalled_context")
    );

    let attached = rt
        .attach_recalled_context(s.c.id, first.id, "tester")
        .unwrap();
    assert_eq!(attached.state, RecalledContextState::Attached);
    assert!(attached.influences_reasoning());
}

#[test]
fn dismiss_recalled_context_never_influences_reasoning() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let s = scenario(&rt);

    let attached = rt
        .attach_recalled_context_from_source(s.c.id, s.a.id, Some("manual".into()), "tester")
        .unwrap();
    assert_eq!(attached.state, RecalledContextState::Attached);

    let dismissed = rt
        .dismiss_recalled_context(s.c.id, attached.id, "tester")
        .unwrap();
    assert_eq!(dismissed.state, RecalledContextState::Dismissed);
    assert!(!dismissed.influences_reasoning());

    // Re-evaluate: dismissed context must not appear in metadata.
    rt.derive_knowledge(s.c.id, "tester").unwrap();
    let evaluations = rt.evaluate_investigation(s.c.id, "tester").unwrap();
    for evaluation in evaluations {
        assert!(
            !evaluation.metadata.contains_key("historical_influence"),
            "dismissed context must not influence evaluation"
        );
    }
}

#[test]
fn attached_context_influences_evaluation_and_recommendation_without_rewriting_history() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let s = scenario(&rt);

    // Snapshot historical Investigations before reasoning on C.
    let a_before = rt.open_investigation(s.a.id).unwrap();
    let a_memory_before = rt.recall_memory(s.a.id).unwrap();
    let a_knowledge_before = rt.list_knowledge(s.a.id).unwrap();
    let a_learning_before = rt.list_learning(s.a.id).unwrap();
    let b_before = rt.open_investigation(s.b.id).unwrap();

    rt.attach_recalled_context_from_source(
        s.c.id,
        s.a.id,
        Some("prior successful rollback".into()),
        "tester",
    )
    .unwrap();
    rt.attach_recalled_context_from_source(
        s.c.id,
        s.b.id,
        Some("prior unsuccessful guidance".into()),
        "tester",
    )
    .unwrap();

    let knowledge = rt.derive_knowledge(s.c.id, "tester").unwrap();
    // Knowledge remains derived from C's Memory only — no historical merge.
    for k in &knowledge {
        assert_eq!(k.investigation_id, s.c.id);
    }

    let evaluations = rt.evaluate_investigation(s.c.id, "tester").unwrap();
    assert!(!evaluations.is_empty());
    let eval = &evaluations[0];
    assert_eq!(eval.investigation_id, s.c.id);
    assert!(
        eval.metadata
            .get("historical_influence")
            .and_then(|v| v.as_bool())
            == Some(true),
        "evaluation must record historical influence metadata"
    );
    assert!(
        eval.explanation.contains("Historical context") || eval.explanation.contains("historical"),
        "evaluation explanation must cite historical context: {}",
        eval.explanation
    );
    // Supporting knowledge/memory ids remain current Investigation objects.
    let current_knowledge = rt.list_knowledge(s.c.id).unwrap();
    for kid in &eval.supporting_knowledge_ids {
        let k = current_knowledge.iter().find(|k| k.id == *kid).unwrap();
        assert_eq!(k.investigation_id, s.c.id);
    }

    let _ = rt.verify_all(s.c.id, "tester").unwrap();
    let recommendations = rt.generate_recommendation(s.c.id, "tester").unwrap();
    let rec = &recommendations[0];
    assert_eq!(rec.investigation_id, s.c.id);
    assert_eq!(
        rec.status,
        rivora::domain::RecommendationStatus::Proposed,
        "recommendations remain proposals"
    );
    assert!(
        rec.metadata
            .get("historical_influence")
            .and_then(|v| v.as_bool())
            == Some(true)
    );
    assert!(
        rec.rationale.contains("WARNING")
            || rec.rationale.contains("NOTE")
            || rec.rationale.contains("Historical"),
        "rationale must surface historical notes: {}",
        rec.rationale
    );

    // A and B remain unchanged.
    let a_after = rt.open_investigation(s.a.id).unwrap();
    assert_eq!(a_before, a_after);
    assert_eq!(a_memory_before, rt.recall_memory(s.a.id).unwrap());
    assert_eq!(a_knowledge_before, rt.list_knowledge(s.a.id).unwrap());
    assert_eq!(a_learning_before, rt.list_learning(s.a.id).unwrap());
    assert_eq!(b_before, rt.open_investigation(s.b.id).unwrap());
}

#[test]
fn recalled_context_survives_runtime_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();
    let context_id;
    let inv_id;
    {
        let rt = runtime(&path);
        let s = scenario(&rt);
        inv_id = s.c.id;
        let ctx = rt
            .attach_recalled_context_from_source(s.c.id, s.a.id, None, "tester")
            .unwrap();
        context_id = ctx.id;
    }
    let rt = runtime(&path);
    let listed = rt.list_recalled_context(inv_id).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, context_id);
    assert_eq!(listed[0].state, RecalledContextState::Attached);
}

#[test]
fn suggest_is_idempotent_for_same_sources() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let s = scenario(&rt);
    rt.refresh_relationships(s.c.id, "tester").unwrap();
    let first = rt.suggest_recalled_context(s.c.id, "tester").unwrap();
    let second = rt.suggest_recalled_context(s.c.id, "tester").unwrap();
    assert_eq!(
        first.len(),
        second.len(),
        "suggest must not duplicate non-dismissed context"
    );
}

#[test]
fn cannot_recall_context_from_same_investigation() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let inv = rt.create_investigation("solo", None, "tester").unwrap();
    let err = rt
        .attach_recalled_context_from_source(inv.id, inv.id, None, "tester")
        .unwrap_err();
    assert!(matches!(err, RivoraError::Validation(_)));
}

#[test]
fn detect_patterns_requires_two_investigations_and_cites_support() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let s = scenario(&rt);
    rt.refresh_relationships(s.c.id, "tester").unwrap();

    let patterns = rt.detect_patterns("tester").unwrap();
    assert!(
        !patterns.is_empty(),
        "expected patterns from shared repo and failure signatures"
    );
    for pattern in &patterns {
        assert!(
            pattern.investigation_ids.len() >= 2,
            "pattern must cite at least two investigations"
        );
        assert_eq!(pattern.occurrence_count, pattern.investigation_ids.len());
        assert!(!pattern.description.is_empty());
        assert!(!pattern.derivation_method.is_empty());
    }
    let kinds: Vec<_> = patterns.iter().map(|p| p.kind.as_str()).collect();
    assert!(
        kinds.contains(&"repeated_component")
            || kinds.contains(&"recurring_failure_signature")
            || kinds.contains(&"recurring_connector_evidence"),
        "expected v0.2 pattern kinds, got {kinds:?}"
    );
}

#[test]
fn historical_trends_summarize_durable_records() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let _ = scenario(&rt);

    let trend = rt.summarize_historical_trend(None).unwrap();
    assert!(trend.investigation_count >= 3);
    assert!(
        trend.verification.pass + trend.verification.fail + trend.verification.inconclusive > 0
    );
    assert!(trend.learning.successful >= 1);
    assert!(trend.learning.unsuccessful >= 1);
    assert!(trend.learning.success_rate.is_some());
    assert!(!trend.summary.is_empty());

    let filtered = rt
        .summarize_historical_trend(Some("acme/app".into()))
        .unwrap();
    assert_eq!(filtered.repository_filter.as_deref(), Some("acme/app"));
    assert_eq!(filtered.investigation_count, 3);
    assert!(filtered
        .top_repositories
        .iter()
        .any(|r| r.label == "acme/app"));
}

#[test]
fn end_to_end_cross_investigation_intelligence_flow() {
    let dir = tempfile::tempdir().unwrap();
    let store: Arc<dyn rivora::Store> = Arc::new(LocalStore::open(dir.path()).unwrap());
    let rt = Arc::new(Runtime::new(Arc::clone(&store)));
    let caps = CapabilityService::new(Arc::clone(&rt));

    // 1–4: complete A and B with related evidence.
    let s = scenario(rt.as_ref());

    // 5–7: discover related/similar and explain.
    caps.refresh_relationships(s.c.id, "tester").unwrap();
    let related = caps.list_related_investigations(s.c.id).unwrap();
    assert!(related
        .iter()
        .any(|r| r.related.id == s.a.id || r.related.id == s.b.id));
    let similar = caps.find_similar_investigations(s.c.id, Some(5)).unwrap();
    assert!(similar.iter().any(|r| r.investigation_id == s.a.id));
    assert!(!similar[0].explanation.is_empty());

    // 8: recall selected evidence as historical context.
    let suggested = caps.suggest_recalled_context(s.c.id, "tester").unwrap();
    assert!(!suggested.is_empty());
    let attached = caps
        .attach_recalled_context(s.c.id, suggested[0].id, "tester")
        .unwrap();
    assert_eq!(attached.state, RecalledContextState::Attached);

    // Also attach B for unsuccessful prior outcome influence.
    caps.attach_recalled_context_from_source(
        s.c.id,
        s.b.id,
        Some("prior unsuccessful outcome".into()),
        "tester",
    )
    .unwrap();

    // 9–12: derive current Knowledge, Evaluate, Verify, Recommend.
    let pipeline = caps.run_full_pipeline(s.c.id, "tester").unwrap();
    for k in &pipeline.knowledge {
        assert_eq!(k.investigation_id, s.c.id);
    }
    assert!(pipeline.evaluations.iter().any(|e| {
        e.metadata
            .get("historical_influence")
            .and_then(|v| v.as_bool())
            == Some(true)
    }));
    // Verification remains independent — no historical_influence required.
    assert!(!pipeline.verifications.is_empty());
    assert!(pipeline.recommendations.iter().any(|r| {
        r.metadata
            .get("historical_influence")
            .and_then(|v| v.as_bool())
            == Some(true)
            && (r.rationale.contains("WARNING")
                || r.rationale.contains("NOTE")
                || r.rationale.contains("Historical"))
    }));

    // Patterns and trends available through Capabilities.
    let patterns = caps.detect_patterns("tester").unwrap();
    assert!(!patterns.is_empty());
    let trend = caps
        .summarize_historical_trend(Some("acme/app".into()))
        .unwrap();
    assert!(trend.investigation_count >= 3);

    // 14: A and B unchanged.
    let a = caps.open_investigation(s.a.id).unwrap();
    let b = caps.open_investigation(s.b.id).unwrap();
    assert_eq!(a.status, rivora::domain::InvestigationStatus::Completed);
    assert_eq!(b.status, rivora::domain::InvestigationStatus::Completed);
    assert_eq!(caps.list_recalled_context(s.a.id).unwrap().len(), 0);
    assert_eq!(caps.list_recalled_context(s.b.id).unwrap().len(), 0);
}

#[test]
fn capabilities_and_runtime_share_recalled_context() {
    let dir = tempfile::tempdir().unwrap();
    let store: Arc<dyn rivora::Store> = Arc::new(LocalStore::open(dir.path()).unwrap());
    let rt = Arc::new(Runtime::new(Arc::clone(&store)));
    let caps = CapabilityService::new(Arc::clone(&rt));
    let s = scenario(rt.as_ref());

    let via_caps = caps
        .attach_recalled_context_from_source(s.c.id, s.a.id, None, "caps")
        .unwrap();
    let via_rt = rt.list_recalled_context(s.c.id).unwrap();
    assert_eq!(via_rt.len(), 1);
    assert_eq!(via_rt[0].id, via_caps.id);
}
