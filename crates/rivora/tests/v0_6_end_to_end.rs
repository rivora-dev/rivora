//! v0.6 end-to-end controlled execution scenarios.

use std::collections::HashMap;
use std::sync::Arc;

use rivora::domain::{
    ExecutionAction, ExecutionAttemptStatus, ExecutionPlanStatus, ExecutionPrecondition,
    ExecutionVerificationStatus, ExpectedEffect, MockExecutionCapability, ProposalStatus,
    ProposalTransitionAuthority, RetrySafety,
};
use rivora::runtime::execution::CreateExecutionPlanRequest;
use rivora::runtime::proposal::CreateProposalRequest;
use rivora::storage::LocalStore;
use rivora::{CapabilityService, Confidence, ProposalCategory, ProposalPriority, Runtime};
use tempfile::TempDir;

struct Env {
    _dir: TempDir,
    caps: CapabilityService,
    mock: Arc<MockExecutionCapability>,
    inv: rivora::InvestigationId,
    proposal_id: rivora::ObjectId,
}

fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(LocalStore::open(dir.path()).unwrap());
    let runtime = Arc::new(Runtime::new(store));
    let mock = Arc::new(MockExecutionCapability::new());
    runtime
        .register_execution_capability(Arc::clone(&mock) as Arc<dyn rivora::ExecutionCapability>);
    let caps = CapabilityService::new(runtime);
    let inv = caps
        .create_investigation("v0.6 e2e", None, "tester")
        .unwrap();
    let proposal = caps
        .create_improvement_proposal(
            inv.id,
            CreateProposalRequest {
                title: "Label tracking".into(),
                summary: "Add bug label".into(),
                rationale: "Visibility".into(),
                category: ProposalCategory::Process,
                priority: ProposalPriority::Medium,
                confidence: Confidence::neutral(),
                supporting_evidence_ids: vec![],
                contradicting_evidence_ids: vec![],
                source_recommendation_ids: vec![],
                affected_components: vec!["tracker".into()],
                affected_resources: vec!["issue/1".into()],
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
        mock,
        inv: inv.id,
        proposal_id: accepted.id,
    }
}

fn action(id: &str, key: &str, field: &str, value: &str) -> ExecutionAction {
    ExecutionAction {
        action_id: id.into(),
        action_name: "record_mutation".into(),
        inputs: serde_json::json!({
            "resource_key": key,
            "field": field,
            "value": value
        }),
        continue_on_failure: false,
    }
}

fn prepare_approved(
    env: &Env,
    actions: Vec<ExecutionAction>,
) -> (rivora::ExecutionPlan, rivora::ExecutionApproval) {
    let plan = env
        .caps
        .create_execution_plan(
            env.inv,
            CreateExecutionPlanRequest {
                proposal_id: env.proposal_id,
                capability_id: "mock.record".into(),
                target_system: "mock".into(),
                target_environment: "sandbox".into(),
                actions,
                inputs: serde_json::json!({}),
                expected_effects: vec![ExpectedEffect {
                    description: "label applied".into(),
                    resource_type: "issue".into(),
                    expected_fields: vec![("label".into(), "bug".into())],
                }],
                preconditions: vec![],
                supports_dry_run: true,
            },
            "planner",
        )
        .unwrap();
    let ready = env
        .caps
        .validate_execution_plan(env.inv, plan.id, "planner", "validated")
        .unwrap();
    env.caps
        .approve_execution_plan(
            env.inv,
            ready.id,
            "approver",
            "approved",
            vec![],
            vec![],
            None,
            true,
        )
        .unwrap()
}

#[test]
fn successful_bounded_execution_through_verification_and_implementation() {
    let env = env();
    let (plan, approval) = prepare_approved(&env, vec![action("a1", "issue/1", "label", "bug")]);
    let preview = env.caps.preview_execution_plan(env.inv, plan.id).unwrap();
    assert!(preview.simulated);
    assert!(!preview.expected_mutations.is_empty());

    let attempt = env
        .caps
        .execute_plan(
            env.inv,
            plan.id,
            approval.id,
            "runner",
            "idem-success-1",
            false,
        )
        .unwrap();
    assert_eq!(attempt.status, ExecutionAttemptStatus::Completed);
    assert!(!attempt.receipt_ids.is_empty());
    assert_eq!(
        env.mock
            .get_resource("issue/1")
            .unwrap()
            .get("label")
            .unwrap(),
        "bug"
    );

    let verification = env
        .caps
        .verify_execution_attempt(env.inv, attempt.id, "verifier")
        .unwrap();
    assert_eq!(verification.status, ExecutionVerificationStatus::Passed);

    let head = env
        .caps
        .list_execution_plan_revisions(env.inv, plan.lineage_id)
        .unwrap()
        .plans
        .pop()
        .unwrap();
    assert_eq!(head.status, ExecutionPlanStatus::Verified);

    let impl_rec = env
        .caps
        .link_execution_to_implementation(
            env.inv,
            attempt.id,
            "runner",
            "applied via mock execution",
        )
        .unwrap();
    assert_eq!(impl_rec.proposal_id, env.proposal_id);

    let outcome = env
        .caps
        .create_measured_learning_outcome(env.inv, env.proposal_id, impl_rec.id, "learner")
        .unwrap();
    assert!(!outcome.expected_results.is_empty());

    let trace = env.caps.trace_execution(env.inv, plan.id).unwrap();
    assert!(trace.explanation.contains("Proposal Accepted"));
    assert!(!trace.attempt_ids.is_empty());
}

#[test]
fn failed_precondition_blocks_mutation() {
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
                actions: vec![action("a1", "issue/9", "label", "bug")],
                inputs: serde_json::json!({}),
                expected_effects: vec![],
                preconditions: vec![ExecutionPrecondition {
                    id: "issue-exists".into(),
                    description: "issue must exist".into(),
                    satisfied: Some(false),
                    detail: Some("issue 9 missing".into()),
                }],
                supports_dry_run: true,
            },
            "planner",
        )
        .unwrap();
    let ready = env
        .caps
        .validate_execution_plan(env.inv, plan.id, "planner", "ok")
        .unwrap();
    let (approved, approval) = env
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
    let attempt = env
        .caps
        .execute_plan(
            env.inv,
            approved.id,
            approval.id,
            "runner",
            "idem-precond",
            false,
        )
        .unwrap();
    assert_eq!(attempt.status, ExecutionAttemptStatus::Blocked);
    assert!(env.mock.get_resource("issue/9").is_none());
}

#[test]
fn partial_failure_records_both_actions() {
    let env = env();
    env.mock
        .set_fail_action_names(vec!["record_mutation".into()]);
    // First action uses fail_mutation name pattern via second action.
    env.mock.set_fail_action_names(vec![]);
    let actions = vec![
        action("a1", "issue/1", "label", "bug"),
        ExecutionAction {
            action_id: "a2".into(),
            action_name: "fail_mutation".into(),
            inputs: serde_json::json!({
                "resource_key": "issue/1",
                "field": "status",
                "value": "closed"
            }),
            continue_on_failure: false,
        },
    ];
    let (plan, approval) = prepare_approved(&env, actions);
    // Override expected fields for multi-action
    let attempt = env
        .caps
        .execute_plan(
            env.inv,
            plan.id,
            approval.id,
            "runner",
            "idem-partial",
            false,
        )
        .unwrap();
    assert_eq!(attempt.status, ExecutionAttemptStatus::PartiallyCompleted);
    assert_eq!(attempt.completed_actions, vec!["a1".to_string()]);
    assert_eq!(attempt.failed_actions, vec!["a2".to_string()]);
    assert_eq!(attempt.retry_safety, RetrySafety::Unsafe);
    assert!(env
        .mock
        .get_resource("issue/1")
        .unwrap()
        .contains_key("label"));
    assert!(!env
        .mock
        .get_resource("issue/1")
        .unwrap()
        .contains_key("status"));
}

#[test]
fn idempotent_retry_suppresses_duplicate_mutation() {
    let env = env();
    let (plan, approval) = prepare_approved(&env, vec![action("a1", "issue/2", "label", "bug")]);
    let first = env
        .caps
        .execute_plan(env.inv, plan.id, approval.id, "runner", "same-key", false)
        .unwrap();
    assert_eq!(first.status, ExecutionAttemptStatus::Completed);

    // One-time approval consumed; re-approve for retry demonstration at capability layer:
    // Attempt-level idempotency returns the same attempt without re-mutation.
    let second = env
        .caps
        .execute_plan(env.inv, plan.id, approval.id, "runner", "same-key", false)
        .unwrap();
    assert_eq!(second.id, first.id);
    assert_eq!(
        env.mock
            .get_resource("issue/2")
            .unwrap()
            .get("label")
            .unwrap(),
        "bug"
    );
}

#[test]
fn unsafe_retry_classification_after_partial() {
    let env = env();
    let actions = vec![
        action("a1", "issue/3", "label", "bug"),
        ExecutionAction {
            action_id: "a2".into(),
            action_name: "fail_mutation".into(),
            inputs: serde_json::json!({
                "resource_key": "issue/3",
                "field": "x",
                "value": "y"
            }),
            continue_on_failure: false,
        },
    ];
    let (plan, approval) = prepare_approved(&env, actions);
    let attempt = env
        .caps
        .execute_plan(
            env.inv,
            plan.id,
            approval.id,
            "runner",
            "idem-unsafe",
            false,
        )
        .unwrap();
    let safety = env.caps.classify_retry_safety(env.inv, attempt.id).unwrap();
    assert_eq!(safety, RetrySafety::Unsafe);
}

#[test]
fn policy_denial_unknown_capability() {
    let env = env();
    let plan = env
        .caps
        .create_execution_plan(
            env.inv,
            CreateExecutionPlanRequest {
                proposal_id: env.proposal_id,
                capability_id: "github.merge".into(),
                target_system: "github".into(),
                target_environment: "production".into(),
                actions: vec![action("a1", "pr/1", "merged", "true")],
                inputs: serde_json::json!({}),
                expected_effects: vec![],
                preconditions: vec![],
                supports_dry_run: false,
            },
            "planner",
        )
        .unwrap();
    let err = env
        .caps
        .validate_execution_plan(env.inv, plan.id, "planner", "try")
        .unwrap_err();
    assert!(
        err.to_string().contains("not registered") || err.to_string().contains("denied"),
        "{}",
        err
    );
}

#[test]
fn verification_failure_when_observed_state_mismatches() {
    let env = env();
    env.mock.set_lie_success(true);
    let (plan, approval) = prepare_approved(&env, vec![action("a1", "issue/4", "label", "bug")]);
    let attempt = env
        .caps
        .execute_plan(env.inv, plan.id, approval.id, "runner", "idem-lie", false)
        .unwrap();
    assert_eq!(attempt.status, ExecutionAttemptStatus::Completed);
    let verification = env
        .caps
        .verify_execution_attempt(env.inv, attempt.id, "verifier")
        .unwrap();
    assert_eq!(verification.status, ExecutionVerificationStatus::Failed);
    assert!(!verification.contradictions.is_empty());
}

#[test]
fn dry_run_does_not_mutate() {
    let env = env();
    let (plan, approval) = prepare_approved(&env, vec![action("a1", "issue/5", "label", "bug")]);
    let attempt = env
        .caps
        .execute_plan(env.inv, plan.id, approval.id, "runner", "idem-dry", true)
        .unwrap();
    assert!(attempt.dry_run);
    assert_eq!(attempt.status, ExecutionAttemptStatus::Completed);
    assert!(env.mock.get_resource("issue/5").is_none());
}

#[test]
fn dry_run_idempotency_does_not_suppress_live_execution() {
    let env = env();
    let (plan, approval) = prepare_approved(&env, vec![action("a1", "issue/6", "label", "bug")]);
    let dry = env
        .caps
        .execute_plan(env.inv, plan.id, approval.id, "runner", "shared-key", true)
        .unwrap();
    assert!(dry.dry_run);
    assert!(env.mock.get_resource("issue/6").is_none());

    let live = env
        .caps
        .execute_plan(env.inv, plan.id, approval.id, "runner", "shared-key", false)
        .unwrap();
    assert!(!live.dry_run);
    assert_ne!(live.id, dry.id);
    assert_eq!(live.status, ExecutionAttemptStatus::Completed);
    assert_eq!(
        env.mock
            .get_resource("issue/6")
            .unwrap()
            .get("label")
            .unwrap(),
        "bug"
    );
}

#[test]
fn list_capabilities_includes_mock() {
    let env = env();
    let list = env.caps.list_execution_capabilities();
    assert!(list.iter().any(|c| c.capability_id == "mock.record"));
    let desc = env.caps.show_execution_capability("mock.record").unwrap();
    assert_eq!(desc.risk_level, rivora::CapabilityRiskLevel::LowRiskWrite);
}

#[test]
fn seed_precondition_with_resource() {
    let env = env();
    let mut fields = HashMap::new();
    fields.insert("exists".into(), "true".into());
    env.mock.seed_resource("issue/1", fields);
    assert!(env.mock.get_resource("issue/1").is_some());
}
