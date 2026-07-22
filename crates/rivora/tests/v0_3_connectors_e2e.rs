//! v0.3 Phase 2 — connector fixtures feed assistance workflows.

use std::sync::Arc;

use rivora::storage::LocalStore;
use rivora::{CapabilityService, Runtime};
use rivora_connectors::github_actions::GitHubActionsConnector;
use rivora_connectors::kubernetes::KubernetesConnector;
use rivora_connectors::sentry::SentryConnector;
use serde_json::json;

#[test]
fn three_connector_categories_ingest_and_assist() {
    let dir = tempfile::tempdir().unwrap().keep();
    let store = Arc::new(LocalStore::open(&dir).unwrap());
    let caps = CapabilityService::new(Arc::new(Runtime::new(store)));
    let inv = caps
        .create_investigation("connector assist", None, "tester")
        .unwrap();

    let actions = GitHubActionsConnector::observe_from_fixture(&json!({
        "repository": "acme/app",
        "workflow_runs": [{
            "id": 9,
            "name": "Deploy",
            "status": "completed",
            "conclusion": "failure",
            "event": "push",
            "updated_at": "2026-01-01T00:00:00Z",
            "jobs": [{"id": 1, "name": "deploy", "conclusion": "failure"}]
        }]
    }))
    .unwrap();
    let k8s = KubernetesConnector::observe_from_fixture(&json!({
        "namespace": "prod",
        "items": [{
            "kind": "Pod",
            "metadata": {"name": "api"},
            "status": {"phase": "Failed", "containerStatuses": [{"ready": false}]}
        }]
    }))
    .unwrap();
    let sentry = SentryConnector::observe_from_fixture(&json!({
        "organization": "acme",
        "project": "api",
        "issues": [{
            "id": "55",
            "title": "panic in handler",
            "level": "error",
            "count": "3",
            "lastSeen": "2026-01-01T01:00:00Z"
        }]
    }))
    .unwrap();

    for obs in actions.into_iter().chain(k8s).chain(sentry) {
        caps.ingest_observation(
            inv.id,
            obs.kind,
            obs.summary,
            obs.payload,
            obs.source,
            obs.observed_at,
            obs.idempotency_key,
            "tester",
        )
        .unwrap();
    }

    // Idempotent re-ingest of same keys returns replay.
    let again = GitHubActionsConnector::observe_from_fixture(&json!({
        "repository": "acme/app",
        "workflow_runs": [{
            "id": 9,
            "name": "Deploy",
            "status": "completed",
            "conclusion": "failure",
            "event": "push",
            "updated_at": "2026-01-01T00:00:00Z"
        }]
    }))
    .unwrap();
    let (_o, _m, replay) = caps
        .ingest_observation(
            inv.id,
            again[0].kind.clone(),
            again[0].summary.clone(),
            again[0].payload.clone(),
            again[0].source.clone(),
            again[0].observed_at,
            again[0].idempotency_key.clone(),
            "tester",
        )
        .unwrap();
    assert!(replay);

    let wf = caps
        .run_composite(inv.id, "explain_failure", "tester")
        .unwrap();
    assert!(wf
        .steps
        .iter()
        .any(|s| s.status == rivora::domain::WorkflowStepStatus::Completed));
    let readiness = caps.assess_deployment_readiness(inv.id, "tester").unwrap();
    assert!(!readiness.dimensions.is_empty());
    let report = caps.generate_engineering_report(inv.id, "tester").unwrap();
    assert!(!report.markdown.is_empty());
}

#[test]
fn connector_status_reports_are_read_only() {
    let a = GitHubActionsConnector::new("a/b").status();
    let k = KubernetesConnector::new("default").status();
    let s = SentryConnector::new("o", "p").status();
    assert!(a.read_only && k.read_only && s.read_only);
    assert_eq!(a.category, "ci");
    assert_eq!(k.category, "infrastructure");
    assert_eq!(s.category, "observability");
}
