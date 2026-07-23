//! v0.8 Capability Coverage — inventory, multi-capability routing, lifecycle consistency.

use std::sync::Arc;

use rivora::domain::{
    build_capability_coverage_report, default_accepted_input_types, EngineeringLoopStage,
    ExecutionAction, ExecutionCapability, ExpectedEffect, MockExecutionCapability, ObservationKind,
    ProposalStatus, ProposalTransitionAuthority, FIRST_PARTY_EXECUTION_CAPABILITY_IDS,
};
use rivora::runtime::execution::CreateExecutionPlanRequest;
use rivora::runtime::proposal::CreateProposalRequest;
use rivora::storage::LocalStore;
use rivora::{CapabilityService, Confidence, ProposalCategory, ProposalPriority, Runtime};
use rivora_connectors::{register_github_execution_capabilities, DEFAULT_GITHUB_EXECUTION_REPO};
use tempfile::TempDir;

struct Env {
    _dir: TempDir,
    caps: CapabilityService,
    inv: rivora::InvestigationId,
    proposal_id: rivora::ObjectId,
}

fn open_full_registry() -> CapabilityService {
    let dir = tempfile::tempdir().unwrap().keep();
    let store = Arc::new(LocalStore::open(&dir).unwrap());
    let runtime = Arc::new(Runtime::new(store));
    runtime
        .register_execution_capability(Arc::new(MockExecutionCapability::new()))
        .unwrap();
    register_github_execution_capabilities(
        runtime.execution_registry(),
        DEFAULT_GITHUB_EXECUTION_REPO,
        None,
    )
    .unwrap();
    CapabilityService::new(runtime)
}

fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(LocalStore::open(dir.path()).unwrap());
    let runtime = Arc::new(Runtime::new(store));
    runtime
        .register_execution_capability(
            Arc::new(MockExecutionCapability::new()) as Arc<dyn ExecutionCapability>
        )
        .unwrap();
    register_github_execution_capabilities(
        runtime.execution_registry(),
        DEFAULT_GITHUB_EXECUTION_REPO,
        None,
    )
    .unwrap();
    let caps = CapabilityService::new(runtime);
    let inv = caps
        .create_investigation("v0.8 coverage", None, "tester")
        .unwrap();
    let proposal = caps
        .create_improvement_proposal(
            inv.id,
            CreateProposalRequest {
                title: "Coverage mutation".into(),
                summary: "Record mock state".into(),
                rationale: "Prove first-party loop".into(),
                category: ProposalCategory::Process,
                priority: ProposalPriority::Medium,
                confidence: Confidence::neutral(),
                supporting_evidence_ids: vec![],
                contradicting_evidence_ids: vec![],
                source_recommendation_ids: vec![],
                affected_components: vec!["coverage".into()],
                affected_resources: vec!["mock".into()],
            },
            "tester",
        )
        .unwrap();
    let proposed = caps
        .update_improvement_proposal_status(
            inv.id,
            proposal.id,
            ProposalStatus::Proposed,
            "tester",
            "submit",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap();
    let review = caps
        .update_improvement_proposal_status(
            inv.id,
            proposed.id,
            ProposalStatus::UnderReview,
            "tester",
            "review",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap();
    let accepted = caps
        .update_improvement_proposal_status(
            inv.id,
            review.id,
            ProposalStatus::Accepted,
            "tester",
            "accept",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap();
    Env {
        _dir: dir,
        caps,
        inv: inv.id,
        proposal_id: accepted.id,
    }
}

#[test]
fn all_first_party_capabilities_registered_with_complete_descriptors() {
    let caps = open_full_registry();
    let list = caps.list_execution_capabilities();
    assert_eq!(list.len(), FIRST_PARTY_EXECUTION_CAPABILITY_IDS.len());
    let ids: Vec<_> = list.iter().map(|c| c.capability_id.as_str()).collect();
    for expected in FIRST_PARTY_EXECUTION_CAPABILITY_IDS {
        assert!(
            ids.contains(expected),
            "missing first-party capability {expected}"
        );
    }
    for desc in &list {
        assert!(
            desc.is_complete(),
            "{} gaps: {:?}",
            desc.capability_id,
            desc.completeness_gaps()
        );
        assert!(!desc.version.is_empty());
        assert!(!desc.accepted_input_types.is_empty());
        assert_eq!(
            desc.accepted_input_types,
            default_accepted_input_types(&desc.capability_id)
        );
        assert!(desc.lifecycle_fully_declared());
        assert_eq!(desc.engineering_loop.memory.as_str(), "supported");
        assert_eq!(desc.engineering_loop.learning.as_str(), "deferred");
    }
    let report = caps.capability_coverage_report();
    assert!(report.all_first_party_registered);
    assert!(report.all_descriptors_complete);
    assert!(report.all_lifecycle_declared);
    assert!(report.gaps.is_empty(), "{:?}", report.gaps);
    assert_eq!(report.connectors.len(), 5);
}

#[test]
fn routing_zero_one_many_is_deterministic_across_first_party() {
    let caps = open_full_registry();
    let inv = caps
        .create_investigation("routing coverage", None, "tester")
        .unwrap();

    let (infra, _, _) = caps
        .ingest_observation(
            inv.id,
            ObservationKind::Infrastructure,
            "k8s pod phase=Running",
            serde_json::json!({"canonical_type": "infrastructure", "phase": "Running"}),
            "kubernetes",
            chrono::Utc::now(),
            Some("k8s-1".into()),
            "tester",
        )
        .unwrap();
    let zero = caps
        .route_observations_to_capabilities(inv.id, &[infra.id])
        .unwrap();
    assert!(zero.unsupported);
    assert!(zero.matches.is_empty());
    assert!(!zero.ambiguous);

    let (wf, _, _) = caps
        .ingest_observation(
            inv.id,
            ObservationKind::WorkflowRun,
            "workflow failed",
            serde_json::json!({"canonical_type": "workflow_run", "name": "CI"}),
            "github_actions",
            chrono::Utc::now(),
            Some("wf-1".into()),
            "tester",
        )
        .unwrap();
    let one = caps
        .route_observations_to_capabilities(inv.id, &[wf.id])
        .unwrap();
    assert!(!one.unsupported);
    assert!(!one.ambiguous);
    assert_eq!(one.matches.len(), 1);
    assert_eq!(
        one.matches[0].capability_id,
        "github_actions.workflow_dispatch"
    );

    let (issue, _, _) = caps
        .ingest_observation(
            inv.id,
            ObservationKind::Issue,
            "issue opened",
            serde_json::json!({"canonical_type": "issue", "number": 1}),
            "github",
            chrono::Utc::now(),
            Some("issue-1".into()),
            "tester",
        )
        .unwrap();
    let many = caps
        .route_observations_to_capabilities(inv.id, &[issue.id])
        .unwrap();
    assert!(!many.unsupported);
    assert!(many.ambiguous);
    assert!(many.matches.len() >= 3);
    let ordered: Vec<_> = many
        .matches
        .iter()
        .map(|m| m.capability_id.as_str())
        .collect();
    let mut sorted = ordered.clone();
    sorted.sort();
    assert_eq!(ordered, sorted, "matches must be sorted by capability_id");
    let many2 = caps
        .route_observations_to_capabilities(inv.id, &[issue.id])
        .unwrap();
    assert_eq!(many.matches, many2.matches);
}

#[test]
fn mock_lifecycle_produces_capability_aware_memory_and_deferred_learning() {
    let env = env();
    let plan = env
        .caps
        .create_execution_plan(
            env.inv,
            CreateExecutionPlanRequest {
                proposal_id: env.proposal_id,
                capability_id: "mock.record".into(),
                target_system: "mock".into(),
                target_environment: "sandbox".into(),
                actions: vec![ExecutionAction {
                    action_id: "a1".into(),
                    action_name: "record_mutation".into(),
                    inputs: serde_json::json!({
                        "resource_key": "coverage/resource",
                        "field": "state",
                        "value": "done"
                    }),
                    continue_on_failure: false,
                }],
                inputs: serde_json::json!({}),
                expected_effects: vec![ExpectedEffect {
                    description: "state done".into(),
                    resource_type: "resource".into(),
                    expected_fields: vec![("state".into(), "done".into())],
                }],
                preconditions: vec![],
                supports_dry_run: true,
            },
            "tester",
        )
        .unwrap();
    let plan = env
        .caps
        .validate_execution_plan(env.inv, plan.id, "tester", "validate")
        .unwrap();
    let (plan, approval) = env
        .caps
        .approve_execution_plan(
            env.inv,
            plan.id,
            "tester",
            "approve",
            vec![],
            vec![],
            None,
            true,
        )
        .unwrap();
    let attempt = env
        .caps
        .execute_plan(
            env.inv,
            plan.id,
            approval.id,
            "tester",
            "coverage-key-1",
            false,
        )
        .unwrap();
    let _ = env
        .caps
        .verify_execution_attempt(env.inv, attempt.id, "tester")
        .unwrap();
    let run = env
        .caps
        .run_capability_lifecycle_for_attempt(env.inv, attempt.id, "tester")
        .unwrap();
    assert_eq!(run.capability_id, "mock.record");
    let memory_stage = run.stage(EngineeringLoopStage::Memory).unwrap();
    assert_eq!(memory_stage.status.as_str(), "completed");
    let learning_stage = run.stage(EngineeringLoopStage::Learning).unwrap();
    assert_eq!(learning_stage.status.as_str(), "deferred");
    let memories = env.caps.recall_memory(env.inv).unwrap();
    assert!(
        memories.iter().any(|m| {
            m.summary.contains("Mock resource mutation")
                || m.summary.contains("mock.record")
                || m.summary.contains("coverage/resource")
        }),
        "expected capability-aware memory facts, got {:?}",
        memories.iter().map(|m| &m.summary).collect::<Vec<_>>()
    );
    let replay = env
        .caps
        .run_capability_lifecycle_for_attempt(env.inv, attempt.id, "tester")
        .unwrap();
    assert_eq!(replay.lineage_id, run.lineage_id);
}

#[test]
fn coverage_report_flags_missing_first_party_when_only_mock_registered() {
    let dir = tempfile::tempdir().unwrap().keep();
    let store = Arc::new(LocalStore::open(&dir).unwrap());
    let runtime = Arc::new(Runtime::new(store));
    runtime
        .register_execution_capability(Arc::new(MockExecutionCapability::new()))
        .unwrap();
    let report = build_capability_coverage_report(&runtime.list_execution_capabilities());
    assert!(!report.all_first_party_registered);
    assert_eq!(report.first_party_registered, 1);
    assert!(report
        .gaps
        .iter()
        .any(|g| g.contains("github.issue.comment")));
}

#[test]
fn unique_capability_ids_and_show_round_trip() {
    let caps = open_full_registry();
    let list = caps.list_execution_capabilities();
    let mut ids: Vec<_> = list.iter().map(|c| c.capability_id.clone()).collect();
    let before = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), before);
    for id in ids {
        let shown = caps.show_execution_capability(&id).unwrap();
        assert_eq!(shown.capability_id, id);
        assert!(shown.is_complete());
    }
}

#[test]
fn all_github_descriptors_share_standard_lifecycle_contract() {
    let caps = open_full_registry();
    for id in [
        "github.issue.comment",
        "github.issue.label",
        "github.issue.create",
        "github.pull_request.create_draft",
        "github_actions.workflow_dispatch",
    ] {
        let desc = caps.show_execution_capability(id).unwrap();
        assert!(desc.mutating);
        assert!(desc.supports_dry_run);
        assert!(!desc.permissions.is_empty());
        assert!(!desc.output_types.is_empty());
        assert!(!desc.limitations.is_empty());
        assert!(desc.provider_independent);
        assert_eq!(desc.engineering_loop.verification.as_str(), "supported");
        assert_eq!(desc.engineering_loop.improvement.as_str(), "deferred");
    }
}
