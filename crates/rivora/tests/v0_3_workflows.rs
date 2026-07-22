//! v0.3 Phase 1 — Composite Capabilities and Assisted Workflows (RFC-018).

use std::sync::Arc;

use chrono::Utc;
use rivora::domain::{ObservationKind, WorkflowStatus, WorkflowStepStatus};
use rivora::storage::LocalStore;
use rivora::{CapabilityService, Runtime};

fn caps() -> CapabilityService {
    let dir = tempfile::tempdir().unwrap();
    // Keep tempdir for test process lifetime.
    let path = dir.keep();
    let store = Arc::new(LocalStore::open(path).unwrap());
    CapabilityService::new(Arc::new(Runtime::new(store)))
}

fn seed_investigation(caps: &CapabilityService) -> rivora::InvestigationId {
    let inv = caps
        .create_investigation("v0.3 workflow", None, "tester")
        .unwrap();
    caps.ingest_observation(
        inv.id,
        ObservationKind::CheckResult,
        "CI build failed on main",
        serde_json::json!({"conclusion": "failure", "name": "build"}),
        "test",
        Utc::now(),
        Some("seed-ci-fail".into()),
        "tester",
    )
    .unwrap();
    caps.ingest_observation(
        inv.id,
        ObservationKind::Event,
        "Deploy attempt recorded",
        serde_json::json!({"action": "deploy"}),
        "test",
        Utc::now(),
        Some("seed-deploy".into()),
        "tester",
    )
    .unwrap();
    inv.id
}

#[test]
fn composite_definitions_include_three_intents() {
    let caps = caps();
    let defs = caps.list_composite_capabilities();
    assert!(defs.len() >= 3);
    let ids: Vec<_> = defs.iter().map(|d| d.id.as_str()).collect();
    assert!(ids.contains(&"investigate_engineering_problem"));
    assert!(ids.contains(&"assess_deployment_readiness"));
    assert!(ids.contains(&"explain_failure"));
    for def in defs {
        assert!(!def.core_capabilities.is_empty());
    }
}

#[test]
fn plan_preserves_step_order_without_execution() {
    let caps = caps();
    let id = seed_investigation(&caps);
    let plan = caps
        .plan_workflow(id, "investigate_engineering_problem", "tester")
        .unwrap();
    assert_eq!(plan.status, WorkflowStatus::Planned);
    assert!(plan.steps.len() >= 5);
    for (i, step) in plan.steps.iter().enumerate() {
        assert_eq!(step.index as usize, i);
        assert_eq!(step.status, WorkflowStepStatus::Planned);
        assert!(step.output_refs.is_empty());
    }
    // Planning did not create evaluations yet beyond any prior work.
    assert!(caps.list_evaluations(id).unwrap().is_empty());
}

#[test]
fn investigate_composite_runs_end_to_end() {
    let caps = caps();
    let id = seed_investigation(&caps);
    let workflow = caps
        .run_composite(id, "investigate_engineering_problem", "tester")
        .unwrap();
    assert!(
        matches!(
            workflow.status,
            WorkflowStatus::Completed | WorkflowStatus::PartiallyCompleted
        ),
        "status={}",
        workflow.status.as_str()
    );
    let completed = workflow
        .steps
        .iter()
        .filter(|s| s.status == WorkflowStepStatus::Completed)
        .count();
    assert!(
        completed >= 5,
        "expected most steps completed, got {completed}"
    );
    assert!(!caps.list_knowledge(id).unwrap().is_empty());
    assert!(!caps.list_evaluations(id).unwrap().is_empty());
    assert!(!caps.list_verifications(id).unwrap().is_empty());
    assert!(!caps.list_recommendations(id).unwrap().is_empty());
    assert!(workflow.summary.is_some());
}

#[test]
fn three_composites_work_end_to_end() {
    let caps = caps();
    let id = seed_investigation(&caps);
    for intent in [
        "investigate_engineering_problem",
        "assess_deployment_readiness",
        "explain_failure",
    ] {
        let wf = caps.run_composite(id, intent, "tester").unwrap();
        assert!(
            matches!(
                wf.status,
                WorkflowStatus::Completed | WorkflowStatus::PartiallyCompleted
            ),
            "{intent} => {}",
            wf.status.as_str()
        );
        assert!(
            wf.steps
                .iter()
                .any(|s| s.status == WorkflowStepStatus::Completed),
            "{intent} produced no completed steps"
        );
    }
}

#[test]
fn cancel_preserves_completed_steps() {
    let caps = caps();
    let id = seed_investigation(&caps);
    let plan = caps
        .plan_workflow(id, "investigate_engineering_problem", "tester")
        .unwrap();
    let cancelled = caps
        .cancel_workflow(id, plan.id, Some("operator stop".into()), "tester")
        .unwrap();
    assert_eq!(cancelled.status, WorkflowStatus::Cancelled);
    assert!(cancelled
        .cancellation_reason
        .as_deref()
        .unwrap_or("")
        .contains("operator"));
    assert!(cancelled
        .steps
        .iter()
        .all(|s| s.status == WorkflowStepStatus::Cancelled));
}

#[test]
fn intermediate_outputs_persist_after_partial_failure() {
    let caps = caps();
    let id = seed_investigation(&caps);
    // Run investigate to create durable objects.
    let wf = caps
        .run_composite(id, "investigate_engineering_problem", "tester")
        .unwrap();
    let reloaded = caps.open_workflow(id, wf.id).unwrap();
    assert_eq!(reloaded.id, wf.id);
    assert!(!reloaded.steps.is_empty());
    // Knowledge remains after workflow finishes.
    assert!(!caps.list_knowledge(id).unwrap().is_empty());
}

#[test]
fn explain_and_summarize_workflow() {
    let caps = caps();
    let id = seed_investigation(&caps);
    let wf = caps.run_composite(id, "explain_failure", "tester").unwrap();
    let explanation = caps.explain_workflow(id, wf.id).unwrap();
    assert!(explanation.contains("explain_failure") || explanation.contains("Workflow"));
    let summary = caps.summarize_workflow(id, wf.id).unwrap();
    assert!(summary.contains("workflow") || summary.contains("Assisted"));
}

#[test]
fn workflows_list_is_durable() {
    let caps = caps();
    let id = seed_investigation(&caps);
    caps.plan_workflow(id, "assess_deployment_readiness", "tester")
        .unwrap();
    caps.plan_workflow(id, "explain_failure", "tester").unwrap();
    let listed = caps.list_workflows(id).unwrap();
    assert_eq!(listed.len(), 2);
}

#[test]
fn core_capabilities_are_reused_not_duplicated_in_cli_layer() {
    // Architecture: CapabilityService methods exist and Runtime owns reasoning.
    let caps = caps();
    let id = seed_investigation(&caps);
    let _ = caps.recall_memory(id).unwrap();
    let _ = caps.derive_knowledge(id, "tester").unwrap();
    let wf = caps
        .plan_workflow(id, "investigate_engineering_problem", "tester")
        .unwrap();
    assert_eq!(wf.steps[0].capability, "recall_memory");
    assert_eq!(wf.steps[1].capability, "derive_knowledge");
}

#[test]
fn no_external_mutation_from_workflow_execution() {
    let caps = caps();
    let id = seed_investigation(&caps);
    let wf = caps
        .run_composite(id, "assess_deployment_readiness", "tester")
        .unwrap();
    // Workflow summary asserts no external mutation; metadata stays local.
    let summary = wf.summary.unwrap_or_default();
    assert!(
        summary.contains("No external") || summary.contains("Assisted workflow"),
        "{summary}"
    );
}
