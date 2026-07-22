//! v0.2 Phase 2 — Search and Recall (RFC-016).

use std::path::Path;
use std::sync::Arc;

use chrono::Utc;
use rivora::domain::{
    Investigation, InvestigationId, InvestigationStatus, ObjectId, Observation, ObservationKind,
    OutcomeDisposition, RelationshipKind, VerificationResult,
};
use rivora::runtime::observation::IngestObservationRequest;
use rivora::runtime::search::{OutcomeFilter, RankingFactor, SearchQuery};
use rivora::storage::LocalStore;
use rivora::{RivoraError, Runtime};

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

/// Deterministic fixture set for search tests.
struct Fixture {
    a: Investigation, // repo acme/app, failing check build, successful outcome
    b: Investigation, // repo acme/app, failing check build
    c: Investigation, // repo other/lib, passing check
    d: Investigation, // unrelated note
}

fn fixture(rt: &Runtime) -> Fixture {
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
    rt.run_full_pipeline_for_test(b.id);

    let c = rt
        .create_investigation("Library release for other lib", None, "tester")
        .unwrap();
    ingest(
        rt,
        c.id,
        ObservationKind::Repository,
        "Repository metadata for `other/lib`",
        serde_json::json!({"full_name": "other/lib"}),
        "github",
    );
    ingest(
        rt,
        c.id,
        ObservationKind::CheckResult,
        "Check build passed",
        serde_json::json!({"name": "build", "conclusion": "success"}),
        "github",
    );
    rt.run_full_pipeline_for_test(c.id);

    let d = rt
        .create_investigation("Unrelated frontend polish", None, "tester")
        .unwrap();
    ingest(
        rt,
        d.id,
        ObservationKind::UserInput,
        "Tweak button spacing on the settings page",
        serde_json::json!({}),
        "cli",
    );

    Fixture { a, b, c, d }
}

/// Small helper: run the v0.1 pipeline via public Runtime methods and
/// return the recommendation ids.
trait PipelineForTest {
    fn run_full_pipeline_for_test(&self, id: InvestigationId) -> Vec<ObjectId>;
}

impl PipelineForTest for Runtime {
    fn run_full_pipeline_for_test(&self, id: InvestigationId) -> Vec<ObjectId> {
        self.derive_knowledge(id, "tester").unwrap();
        self.evaluate_investigation(id, "tester").unwrap();
        self.verify_all(id, "tester").unwrap();
        self.generate_recommendation(id, "tester")
            .unwrap()
            .into_iter()
            .map(|r| r.id)
            .collect()
    }
}

#[test]
fn exact_id_search_short_circuits() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let f = fixture(&rt);

    let results = rt
        .search_investigations(SearchQuery {
            investigation_id: Some(f.c.id),
            ..SearchQuery::default()
        })
        .unwrap();
    assert_eq!(results.len(), 1);
    let result = &results[0];
    assert_eq!(result.investigation_id, f.c.id);
    assert_eq!(result.score, 1.0);
    assert_eq!(result.matched_evidence[0].factor, RankingFactor::ExactId);
    assert!(!result.explanation.is_empty());
}

#[test]
fn structured_filters_constrain_results() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let f = fixture(&rt);

    // Repository filter.
    let results = rt
        .search_investigations(SearchQuery {
            repository: Some("acme/app".into()),
            ..SearchQuery::default()
        })
        .unwrap();
    let ids: Vec<InvestigationId> = results.iter().map(|r| r.investigation_id).collect();
    assert!(ids.contains(&f.a.id) && ids.contains(&f.b.id));
    assert!(!ids.contains(&f.c.id) && !ids.contains(&f.d.id));

    // Verification result filter: all of A, B, C pass verification.
    let results = rt
        .search_investigations(SearchQuery {
            verification_result: Some(VerificationResult::Pass),
            ..SearchQuery::default()
        })
        .unwrap();
    let ids: Vec<InvestigationId> = results.iter().map(|r| r.investigation_id).collect();
    assert!(ids.contains(&f.a.id) && ids.contains(&f.b.id) && ids.contains(&f.c.id));
    assert!(!ids.contains(&f.d.id));

    // Outcome filter.
    let results = rt
        .search_investigations(SearchQuery {
            outcome: Some(OutcomeDisposition::Successful),
            ..SearchQuery::default()
        })
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].investigation_id, f.a.id);
    assert!(results[0]
        .matched_evidence
        .iter()
        .any(|e| e.factor == RankingFactor::OutcomeMatch));

    // Connector source filter.
    let results = rt
        .search_investigations(SearchQuery {
            connector_source: Some("cli".into()),
            ..SearchQuery::default()
        })
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].investigation_id, f.d.id);

    // Status filter: everything is past Created.
    let results = rt
        .search_investigations(SearchQuery {
            status: Some(InvestigationStatus::Created),
            ..SearchQuery::default()
        })
        .unwrap();
    assert!(results.is_empty());

    // Date range filter: after = now excludes everything created earlier.
    let results = rt
        .search_investigations(SearchQuery {
            created_after: Some(Utc::now() + chrono::Duration::hours(1)),
            ..SearchQuery::default()
        })
        .unwrap();
    assert!(results.is_empty());

    // Relationship kind filter after refresh.
    rt.refresh_relationships(f.a.id, "tester").unwrap();
    let results = rt
        .search_investigations(SearchQuery {
            relationship_kind: Some(RelationshipKind::SharedRepository),
            ..SearchQuery::default()
        })
        .unwrap();
    let ids: Vec<InvestigationId> = results.iter().map(|r| r.investigation_id).collect();
    assert!(ids.contains(&f.a.id) && ids.contains(&f.b.id));
    assert!(!ids.contains(&f.d.id));

    // File filter.
    ingest(
        &rt,
        f.a.id,
        ObservationKind::ChangedFiles,
        "2 changed file(s)",
        serde_json::json!({"files": ["src/db.rs", "src/lib.rs"]}),
        "local",
    );
    let results = rt
        .search_investigations(SearchQuery {
            file: Some("src/db.rs".into()),
            ..SearchQuery::default()
        })
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].investigation_id, f.a.id);
}

#[test]
fn text_search_ranks_by_token_overlap() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let f = fixture(&rt);

    let results = rt
        .search_investigations(SearchQuery {
            text: Some("acme build failed".into()),
            ..SearchQuery::default()
        })
        .unwrap();
    assert!(!results.is_empty());
    let ids: Vec<InvestigationId> = results.iter().map(|r| r.investigation_id).collect();
    assert!(ids.contains(&f.a.id) || ids.contains(&f.b.id));
    // A and B outrank everything else.
    let top_two: Vec<InvestigationId> = ids.iter().take(2).copied().collect();
    assert!(top_two.contains(&f.a.id) && top_two.contains(&f.b.id));
    // Every result explains itself.
    for result in &results {
        assert!(!result.explanation.is_empty());
        assert!(!result.matched_evidence.is_empty());
        assert!(result.explanation.contains("Score"));
        assert!(result
            .matched_evidence
            .iter()
            .any(|e| e.factor == RankingFactor::TextOverlap));
    }
}

#[test]
fn semantic_factor_is_deterministic() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let f = fixture(&rt);
    let _ = f;

    let query = || SearchQuery {
        text: Some("deploy regression rollback".into()),
        ..SearchQuery::default()
    };
    let first = rt.search_investigations(query()).unwrap();
    let second = rt.search_investigations(query()).unwrap();
    assert_eq!(first.len(), second.len());
    for (a, b) in first.iter().zip(second.iter()) {
        assert_eq!(a.investigation_id, b.investigation_id);
        assert_eq!(a.score, b.score, "scores must be deterministic");
    }
    assert!(first.iter().any(|r| r
        .matched_evidence
        .iter()
        .any(|e| e.factor == RankingFactor::SemanticSimilarity)));
}

#[test]
fn find_similar_ranks_shared_evidence_first() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let f = fixture(&rt);

    let similar = rt.find_similar_investigations(f.a.id, None).unwrap();
    assert!(!similar.is_empty());
    assert_eq!(
        similar[0].investigation_id, f.b.id,
        "B shares repository and failure signature with A: {similar:?}"
    );
    let b_result = &similar[0];
    let factors: Vec<RankingFactor> = b_result.matched_evidence.iter().map(|e| e.factor).collect();
    assert!(factors.contains(&RankingFactor::SharedRepository));
    assert!(factors.contains(&RankingFactor::FailureSignatureMatch));
    assert!(b_result.explanation.contains("acme/app"));

    // D shares nothing with A and must not appear.
    assert!(!similar.iter().any(|r| r.investigation_id == f.d.id));
    // The context investigation never appears in its own results.
    assert!(!similar.iter().any(|r| r.investigation_id == f.a.id));
}

#[test]
fn confirmed_relationship_boosts_similarity_score() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let f = fixture(&rt);

    rt.refresh_relationships(f.a.id, "tester").unwrap();
    let before = rt.find_similar_investigations(f.a.id, None).unwrap();
    let b_before = before
        .iter()
        .find(|r| r.investigation_id == f.b.id)
        .unwrap()
        .score;

    let relationship = rt
        .list_relationships(f.a.id)
        .unwrap()
        .into_iter()
        .find(|r| r.kind == RelationshipKind::SharedRepository)
        .unwrap();
    rt.confirm_relationship(relationship.id, "tester").unwrap();

    let after = rt.find_similar_investigations(f.a.id, None).unwrap();
    let b_after = after.iter().find(|r| r.investigation_id == f.b.id).unwrap();
    assert!(b_after.score >= b_before);
    assert!(b_after.relationship.is_some());
}

#[test]
fn no_result_queries_return_empty() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let _f = fixture(&rt);

    let results = rt
        .search_investigations(SearchQuery {
            text: Some("zzqqx".into()),
            ..SearchQuery::default()
        })
        .unwrap();
    assert!(results.is_empty());
}

#[test]
fn empty_query_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let err = rt
        .search_investigations(SearchQuery::default())
        .unwrap_err();
    assert!(matches!(err, RivoraError::Validation(_)));
}

#[test]
fn explain_search_result_returns_explained_match() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let f = fixture(&rt);

    let explained = rt
        .explain_search_result(
            f.a.id,
            SearchQuery {
                repository: Some("acme/app".into()),
                ..SearchQuery::default()
            },
        )
        .unwrap();
    assert_eq!(explained.investigation_id, f.a.id);
    assert!(explained.explanation.contains("acme/app"));

    let err = rt
        .explain_search_result(
            f.c.id,
            SearchQuery {
                repository: Some("acme/app".into()),
                ..SearchQuery::default()
            },
        )
        .unwrap_err();
    assert!(matches!(err, RivoraError::Precondition(_)));
}

#[test]
fn recall_related_evidence_cites_relationships() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let f = fixture(&rt);

    rt.refresh_relationships(f.a.id, "tester").unwrap();
    let recalled = rt.recall_related_evidence(f.a.id).unwrap();
    assert!(!recalled.is_empty());
    let from_b = recalled
        .iter()
        .find(|r| r.investigation_id == f.b.id)
        .expect("evidence recalled from B");
    assert_eq!(from_b.relationship_kind, RelationshipKind::SharedRepository);
    assert!(!from_b.explanation.is_empty());
    assert!(!from_b.evidence.is_empty());
    assert!(from_b.evidence.iter().all(|e| !e.object_ids.is_empty()));
}

#[test]
fn recall_prior_outcomes_filters() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let f = fixture(&rt);

    // Disposition filter.
    let outcomes = rt
        .recall_prior_outcomes(OutcomeFilter {
            disposition: Some(OutcomeDisposition::Successful),
            ..OutcomeFilter::default()
        })
        .unwrap();
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].investigation_id, f.a.id);
    assert!(outcomes[0].recommendation_summary.is_some());

    // Repository filter.
    let outcomes = rt
        .recall_prior_outcomes(OutcomeFilter {
            repository: Some("acme/app".into()),
            ..OutcomeFilter::default()
        })
        .unwrap();
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].investigation_id, f.a.id);

    // Similar-to filter uses stored relationships.
    rt.refresh_relationships(f.b.id, "tester").unwrap();
    let outcomes = rt
        .recall_prior_outcomes(OutcomeFilter {
            similar_to: Some(f.b.id),
            ..OutcomeFilter::default()
        })
        .unwrap();
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].investigation_id, f.a.id);
}

#[test]
fn search_results_survive_runtime_restart() {
    let dir = tempfile::tempdir().unwrap();
    let scores: Vec<(InvestigationId, f64)> = {
        let rt = runtime(dir.path());
        let _f = fixture(&rt);
        rt.search_investigations(SearchQuery {
            text: Some("acme build".into()),
            ..SearchQuery::default()
        })
        .unwrap()
        .into_iter()
        .map(|r| (r.investigation_id, r.score))
        .collect()
    };

    // Fresh Runtime over the same directory.
    let rt = runtime(dir.path());
    let reloaded: Vec<(InvestigationId, f64)> = rt
        .search_investigations(SearchQuery {
            text: Some("acme build".into()),
            ..SearchQuery::default()
        })
        .unwrap()
        .into_iter()
        .map(|r| (r.investigation_id, r.score))
        .collect();
    assert_eq!(scores, reloaded);
}

#[test]
fn search_never_modifies_investigations() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let f = fixture(&rt);
    rt.refresh_relationships(f.a.id, "tester").unwrap();

    let snapshot = || {
        let mut files: Vec<(std::path::PathBuf, Vec<u8>)> = Vec::new();
        for entry in std::fs::read_dir(dir.path().join("investigations")).unwrap() {
            let inv_dir = entry.unwrap().path();
            for sub in std::fs::read_dir(&inv_dir).unwrap() {
                let sub = sub.unwrap().path();
                if sub.is_dir() {
                    for file in std::fs::read_dir(&sub).unwrap() {
                        let file = file.unwrap().path();
                        files.push((file.clone(), std::fs::read(&file).unwrap()));
                    }
                } else {
                    files.push((sub.clone(), std::fs::read(&sub).unwrap()));
                }
            }
        }
        files.sort();
        files
    };

    let before = snapshot();
    let _ = rt.search_investigations(SearchQuery {
        text: Some("acme build".into()),
        ..SearchQuery::default()
    });
    let _ = rt.find_similar_investigations(f.a.id, None);
    let _ = rt.recall_related_evidence(f.a.id);
    let _ = rt.recall_prior_outcomes(OutcomeFilter::default());
    assert_eq!(before, snapshot(), "search and recall must be read-only");
}

#[test]
fn search_performance_baseline_mvp_dataset() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());

    // 50 investigations, 3 observations each: realistic MVP dataset.
    let mut ids = Vec::new();
    for index in 0..50 {
        let inv = rt
            .create_investigation(
                format!("Investigation {index} for acme app"),
                None,
                "tester",
            )
            .unwrap();
        ingest(
            &rt,
            inv.id,
            ObservationKind::Repository,
            "Repository metadata for `acme/app`",
            serde_json::json!({"full_name": "acme/app"}),
            "github",
        );
        ingest(
            &rt,
            inv.id,
            ObservationKind::CheckResult,
            &format!("Check build {} failed", index % 5),
            serde_json::json!({"name": "build", "conclusion": "failure"}),
            "github",
        );
        ingest(
            &rt,
            inv.id,
            ObservationKind::Commit,
            "Commit abc1234: fix build",
            serde_json::json!({"sha": format!("abc{index:04}")}),
            "github",
        );
        ids.push(inv.id);
    }

    let start = std::time::Instant::now();
    let results = rt
        .search_investigations(SearchQuery {
            text: Some("acme build".into()),
            limit: Some(10),
            ..SearchQuery::default()
        })
        .unwrap();
    let search_elapsed = start.elapsed();
    assert_eq!(results.len(), 10);

    let start = std::time::Instant::now();
    let similar = rt.find_similar_investigations(ids[0], Some(5)).unwrap();
    let similar_elapsed = start.elapsed();
    assert_eq!(similar.len(), 5);

    println!(
        "performance baseline: search {search_elapsed:?} over 50 investigations, \
         similar {similar_elapsed:?}"
    );
    assert!(
        search_elapsed < std::time::Duration::from_secs(10),
        "search too slow: {search_elapsed:?}"
    );
    assert!(
        similar_elapsed < std::time::Duration::from_secs(10),
        "similar too slow: {similar_elapsed:?}"
    );
}
