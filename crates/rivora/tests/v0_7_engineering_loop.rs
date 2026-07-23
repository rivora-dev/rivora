//! v0.7 Capability Engineering Loop integration tests (RFC-028).

use std::sync::Arc;

use rivora::domain::{
    EngineeringLoopStage, ExecutionAction, ExecutionAttemptStatus, ExpectedEffect,
    LifecycleParticipation, LifecycleRunStatus, LifecycleStageStatus, MockExecutionCapability,
    ObservationKind, ProposalStatus, ProposalTransitionAuthority,
};
use rivora::runtime::execution::CreateExecutionPlanRequest;
use rivora::runtime::proposal::CreateProposalRequest;
use rivora::storage::LocalStore;
use rivora::{CapabilityService, Confidence, ProposalCategory, ProposalPriority, Runtime};
use tempfile::TempDir;

struct Env {
    _dir: TempDir,
    caps: CapabilityService,
    inv: rivora::InvestigationId,
    proposal_id: rivora::ObjectId,
}

fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(LocalStore::open(dir.path()).unwrap());
    let runtime = Arc::new(Runtime::new(store));
    let mock = Arc::new(MockExecutionCapability::new());
    runtime
        .register_execution_capability(Arc::clone(&mock) as Arc<dyn rivora::ExecutionCapability>)
        .unwrap();
    let caps = CapabilityService::new(runtime);
    let inv = caps
        .create_investigation("v0.7 loop", None, "tester")
        .unwrap();
    let proposal = caps
        .create_improvement_proposal(
            inv.id,
            CreateProposalRequest {
                title: "Dispatch CI".into(),
                summary: "Run workflow".into(),
                rationale: "Need CI".into(),
                category: ProposalCategory::Process,
                priority: ProposalPriority::Medium,
                confidence: Confidence::neutral(),
                supporting_evidence_ids: vec![],
                contradicting_evidence_ids: vec![],
                source_recommendation_ids: vec![],
                affected_components: vec!["ci".into()],
                affected_resources: vec!["workflow".into()],
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

fn approved_attempt(env: &Env) -> rivora::ExecutionAttempt {
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
                        "resource_key": "ci/main",
                        "field": "status",
                        "value": "green"
                    }),
                    continue_on_failure: false,
                }],
                inputs: serde_json::json!({}),
                expected_effects: vec![ExpectedEffect {
                    description: "status green".into(),
                    resource_type: "resource".into(),
                    expected_fields: vec![("status".into(), "green".into())],
                }],
                preconditions: vec![],
                supports_dry_run: true,
            },
            "planner",
        )
        .unwrap();
    let ready = env
        .caps
        .validate_execution_plan(env.inv, plan.id, "planner", "ok")
        .unwrap();
    let (plan, approval) = env
        .caps
        .approve_execution_plan(
            env.inv,
            ready.id,
            "approver",
            "ok",
            vec![],
            vec![],
            None,
            true,
        )
        .unwrap();
    env.caps
        .execute_plan(
            env.inv,
            plan.id,
            approval.id,
            "executor",
            "v0.7-e2e-key",
            false,
        )
        .unwrap()
}

#[test]
fn every_registered_capability_declares_engineering_loop() {
    let env = env();
    let list = env.caps.list_execution_capabilities();
    assert!(!list.is_empty());
    for desc in list {
        // Explicit participation for every stage (Default is Deferred, never silent None).
        let _ = desc.engineering_loop.memory.as_str();
        let _ = desc.engineering_loop.evaluation.as_str();
        let _ = desc.engineering_loop.verification.as_str();
        let _ = desc.engineering_loop.improvement.as_str();
        let _ = desc.engineering_loop.learning.as_str();
        assert!(!desc.capability_id.is_empty());
    }
    let mock = env.caps.show_execution_capability("mock.record").unwrap();
    assert_eq!(
        mock.engineering_loop.memory,
        LifecycleParticipation::Supported
    );
    assert_eq!(
        mock.engineering_loop.learning,
        LifecycleParticipation::Deferred
    );
    assert!(mock.provider_independent);
    assert!(mock
        .accepted_input_types
        .iter()
        .any(|t| t == "execution_result" || t == "event"));
}

#[test]
fn full_lifecycle_vertical_slice_for_mock_execution() {
    let env = env();
    let attempt = approved_attempt(&env);
    assert_eq!(attempt.status, ExecutionAttemptStatus::Completed);

    let verification = env
        .caps
        .verify_execution_attempt(env.inv, attempt.id, "verifier")
        .unwrap();
    assert_eq!(
        verification.status.as_str(),
        "passed",
        "mock independent verification should pass"
    );

    let run = env
        .caps
        .run_capability_lifecycle_for_attempt(env.inv, attempt.id, "tester")
        .unwrap();

    assert_eq!(run.capability_id, "mock.record");
    assert_eq!(run.attempt_id, Some(attempt.id));
    assert!(matches!(
        run.status,
        LifecycleRunStatus::Completed | LifecycleRunStatus::Partial
    ));

    let memory = run.stage(EngineeringLoopStage::Memory).unwrap();
    assert_eq!(memory.status, LifecycleStageStatus::Completed);
    assert!(!memory.artifact_ids.is_empty());

    let evaluation = run.stage(EngineeringLoopStage::Evaluation).unwrap();
    assert_eq!(evaluation.status, LifecycleStageStatus::Completed);

    let verification_stage = run.stage(EngineeringLoopStage::Verification).unwrap();
    assert_eq!(verification_stage.status, LifecycleStageStatus::Completed);

    let improvement = run.stage(EngineeringLoopStage::Improvement).unwrap();
    assert_eq!(improvement.status, LifecycleStageStatus::Deferred);

    let learning = run.stage(EngineeringLoopStage::Learning).unwrap();
    assert_eq!(learning.status, LifecycleStageStatus::Deferred);

    // Memory append-only growth
    let memories = env.caps.recall_memory(env.inv).unwrap();
    assert!(memories.iter().any(|m| m.summary.contains("mock.record")));

    // Evaluations and verification receipts created via Runtime
    let evals = env.caps.list_evaluations(env.inv).unwrap();
    assert!(!evals.is_empty());
    let verifs = env.caps.list_verifications(env.inv).unwrap();
    assert!(!verifs.is_empty());
}

#[test]
fn lifecycle_replay_is_idempotent() {
    let env = env();
    let attempt = approved_attempt(&env);
    let _ = env
        .caps
        .verify_execution_attempt(env.inv, attempt.id, "verifier")
        .unwrap();
    let first = env
        .caps
        .run_capability_lifecycle_for_attempt(env.inv, attempt.id, "tester")
        .unwrap();
    let second = env
        .caps
        .run_capability_lifecycle_for_attempt(env.inv, attempt.id, "tester")
        .unwrap();
    assert_eq!(first.lineage_id, second.lineage_id);
    // Head revision should be the same completed snapshot lineage.
    assert_eq!(first.id, second.id);

    let mem_count = env.caps.recall_memory(env.inv).unwrap().len();
    let third = env
        .caps
        .run_capability_lifecycle_for_attempt(env.inv, attempt.id, "tester")
        .unwrap();
    assert_eq!(third.lineage_id, first.lineage_id);
    assert_eq!(env.caps.recall_memory(env.inv).unwrap().len(), mem_count);
}

#[test]
fn routing_is_typed_and_deterministic() {
    let env = env();
    let (obs, _, _) = env
        .caps
        .ingest_observation(
            env.inv,
            ObservationKind::WorkflowRun,
            "workflow run failed",
            serde_json::json!({"status": "failure"}),
            "github_actions",
            chrono::Utc::now(),
            Some("wf-1".into()),
            "tester",
        )
        .unwrap();
    let decision = env
        .caps
        .route_observations_to_capabilities(env.inv, &[obs.id])
        .unwrap();
    // mock.record accepts "event" not workflow_run by default — may be unsupported or match if types overlap
    // For workflow_run, github_actions.workflow_dispatch is not registered in this env.
    // mock accepts event, mutation_request, execution_result — not workflow_run → unsupported.
    assert!(
        decision.unsupported
            || decision
                .matches
                .iter()
                .all(|m| m.capability_id == "mock.record")
    );

    let (obs2, _, _) = env
        .caps
        .ingest_observation(
            env.inv,
            ObservationKind::Event,
            "mutation requested",
            serde_json::json!({}),
            "test",
            chrono::Utc::now(),
            Some("evt-1".into()),
            "tester",
        )
        .unwrap();
    let decision2 = env
        .caps
        .route_observations_to_capabilities(env.inv, &[obs2.id])
        .unwrap();
    assert!(!decision2.unsupported);
    assert_eq!(decision2.matches.len(), 1);
    assert_eq!(decision2.matches[0].capability_id, "mock.record");
    assert!(!decision2.ambiguous);

    // Deterministic: same input → same order
    let decision3 = env
        .caps
        .route_observations_to_capabilities(env.inv, &[obs2.id])
        .unwrap();
    assert_eq!(decision2.matches, decision3.matches);
}

#[test]
fn lifecycle_trace_exposes_lineage() {
    let env = env();
    let attempt = approved_attempt(&env);
    let _ = env
        .caps
        .verify_execution_attempt(env.inv, attempt.id, "verifier")
        .unwrap();
    let run = env
        .caps
        .run_capability_lifecycle_for_attempt(env.inv, attempt.id, "tester")
        .unwrap();
    let trace = env
        .caps
        .trace_capability_lifecycle(env.inv, &attempt.id.to_string())
        .unwrap();
    assert_eq!(trace.capability_id, "mock.record");
    assert_eq!(trace.run_id, Some(run.id));
    assert!(!trace.stages.is_empty());
    assert!(trace.explanation.contains("Engineering Loop") || !trace.explanation.is_empty());
}

#[test]
fn old_store_without_lifecycle_runs_still_opens() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(LocalStore::open(dir.path()).unwrap());
    let runtime = Arc::new(Runtime::new(store));
    let caps = CapabilityService::new(runtime);
    let inv = caps.create_investigation("legacy", None, "t").unwrap();
    // No lifecycle_runs directory yet.
    let listing = caps.list_lifecycle_runs(inv.id).unwrap();
    assert!(listing.runs.is_empty());
    assert!(listing.diagnostics.is_empty());
}

#[test]
fn contribution_validation_rejects_undeclared_supported() {
    use rivora::domain::{
        CapabilityLifecycleContributions, ContributionIdentity, EngineeringLoopParticipation,
        MemoryContribution, StageContribution,
    };
    let identity = ContributionIdentity::new("x", "i", rivora::InvestigationId::new(), "a", "k");
    let contributions = CapabilityLifecycleContributions {
        identity,
        memory: StageContribution::Supported {
            value: MemoryContribution {
                summary: "x".into(),
                observation_id: None,
                confidence: 1.0,
                evidence_ids: vec![],
            },
        },
        evaluation: StageContribution::Deferred { reason: "d".into() },
        verification: StageContribution::Deferred { reason: "d".into() },
        improvement: StageContribution::Deferred { reason: "d".into() },
        learning: StageContribution::Deferred { reason: "d".into() },
    };
    let participation = EngineeringLoopParticipation::default();
    assert!(contributions.validate_against(&participation).is_err());
}
