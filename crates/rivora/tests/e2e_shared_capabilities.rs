//! Shared Capability / Runtime consistency tests.

use std::sync::Arc;

use chrono::Utc;
use rivora::domain::{ObservationKind, OutcomeDisposition, RecommendationStatus};
use rivora::storage::LocalStore;
use rivora::{CapabilityService, Runtime};
use rivora_connectors::github::GitHubConnector;
use rivora_connectors::local::LocalConnector;

#[test]
fn capability_pipeline_shared_by_two_services() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(LocalStore::open(dir.path()).unwrap());
    let runtime = Arc::new(Runtime::new(store));

    let cli_caps = CapabilityService::new(Arc::clone(&runtime));
    let workspace_caps = CapabilityService::new(Arc::clone(&runtime));
    assert!(Arc::ptr_eq(cli_caps.runtime(), workspace_caps.runtime()));

    let inv = cli_caps
        .create_investigation("shared", None, "cli")
        .unwrap();

    let _ = workspace_caps
        .ingest_observation(
            inv.id,
            ObservationKind::Event,
            "shared observation",
            serde_json::json!({"error": "timeout"}),
            "workspace",
            Utc::now(),
            Some("shared-1".into()),
            "workspace",
        )
        .unwrap();

    let pipeline = cli_caps.run_full_pipeline(inv.id, "cli").unwrap();
    assert!(!pipeline.knowledge.is_empty());
    assert!(!pipeline.evaluations.is_empty());
    assert!(!pipeline.verifications.is_empty());
    assert!(!pipeline.recommendations.is_empty());
    assert_eq!(
        pipeline.recommendations[0].status,
        RecommendationStatus::Proposed
    );

    let outcome = workspace_caps
        .record_outcome(
            inv.id,
            Some(pipeline.recommendations[0].id),
            OutcomeDisposition::Successful,
            "worked",
            Some("latency improved".into()),
            "workspace",
        )
        .unwrap();
    assert_eq!(outcome.investigation_id, inv.id);

    let from_cli = cli_caps.list_learning(inv.id).unwrap();
    assert_eq!(from_cli.len(), 1);
}

#[test]
fn local_connector_observation_only_end_to_end() {
    let project = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(project.path().join(".rivora/events")).unwrap();
    std::fs::write(
        project.path().join(".rivora/events/fail.json"),
        r#"{"summary":"deploy failed","error":"rollback","idempotency_key":"evt-1"}"#,
    )
    .unwrap();

    let connector = LocalConnector::new(project.path());
    let observations = connector.observe().unwrap();
    assert!(!observations.is_empty());

    let data = tempfile::tempdir().unwrap();
    let caps = CapabilityService::new(Arc::new(Runtime::new(Arc::new(
        LocalStore::open(data.path()).unwrap(),
    ))));
    let inv = caps
        .create_investigation("local-conn", None, "test")
        .unwrap();
    for obs in observations {
        caps.ingest_observation(
            inv.id,
            obs.kind,
            obs.summary,
            obs.payload,
            obs.source,
            obs.observed_at,
            obs.idempotency_key,
            "test",
        )
        .unwrap();
    }
    assert!(!caps.recall_memory(inv.id).unwrap().is_empty());
    let pipeline = caps.run_full_pipeline(inv.id, "test").unwrap();
    assert!(!pipeline.recommendations.is_empty());
}

#[test]
fn github_fixture_connector_end_to_end() {
    let fixture = serde_json::json!({
        "repository": {"full_name": "acme/app"},
        "pull_request": {"number": 9, "title": "Fix crash", "body": "Closes #2"},
        "commits": [{"sha": "deadbeefcafebabe", "message": "fix crash"}],
        "checks": [{"name": "build", "conclusion": "failure"}],
        "issues": [{"number": 2, "title": "crash on start"}]
    });
    let observations = GitHubConnector::observe_from_fixture(&fixture).unwrap();
    assert!(observations.len() >= 4);

    let data = tempfile::tempdir().unwrap();
    let caps = CapabilityService::new(Arc::new(Runtime::new(Arc::new(
        LocalStore::open(data.path()).unwrap(),
    ))));
    let inv = caps.create_investigation("gh", None, "test").unwrap();
    for obs in observations {
        caps.ingest_observation(
            inv.id,
            obs.kind,
            obs.summary,
            obs.payload,
            obs.source,
            obs.observed_at,
            obs.idempotency_key,
            "test",
        )
        .unwrap();
    }
    let pipeline = caps.run_full_pipeline(inv.id, "test").unwrap();
    assert!(pipeline
        .knowledge
        .iter()
        .any(|k| matches!(k.kind, rivora::domain::KnowledgeKind::RiskSignal)));
    assert!(!pipeline.recommendations.is_empty());
}
