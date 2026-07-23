//! v0.6 Execution Plan model, policy, approval, and storage tests.

use std::sync::Arc;

use rivora::domain::{
    default_accepted_input_types, evaluate_execution_policy, CapabilityRiskLevel,
    EngineeringLoopParticipation, ExecutionAction, ExecutionCapabilityDescriptor, ExecutionPlan,
    ExecutionPlanStatus, ExecutionPolicyDecisionKind, MockExecutionCapability, ObjectId,
    ProposalStatus, ProposalTransitionAuthority, Provenance,
};
use rivora::runtime::execution::CreateExecutionPlanRequest;
use rivora::runtime::proposal::CreateProposalRequest;
use rivora::storage::LocalStore;
use rivora::{CapabilityService, Confidence, ProposalCategory, ProposalPriority, Runtime, Store};

fn setup() -> (CapabilityService, rivora::InvestigationId, ObjectId) {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(LocalStore::open(dir.path()).unwrap());
    // Keep tempdir alive by leaking — tests are short-lived processes.
    std::mem::forget(dir);
    let runtime = Arc::new(Runtime::new(store));
    let mock = Arc::new(MockExecutionCapability::new());
    runtime.register_execution_capability(mock).unwrap();
    let caps = CapabilityService::new(runtime);
    let inv = caps
        .create_investigation("v0.6 model", None, "tester")
        .unwrap();
    let proposal = caps
        .create_improvement_proposal(
            inv.id,
            CreateProposalRequest {
                title: "Add label".into(),
                summary: "Label an issue".into(),
                rationale: "Track work".into(),
                category: ProposalCategory::Process,
                priority: ProposalPriority::Medium,
                confidence: Confidence::neutral(),
                supporting_evidence_ids: vec![],
                contradicting_evidence_ids: vec![],
                source_recommendation_ids: vec![],
                affected_components: vec![],
                affected_resources: vec![],
            },
            "tester",
        )
        .unwrap();
    // Draft → Proposed → UnderReview → Accepted
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
            "accept for later execution",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap();
    (caps, inv.id, accepted.id)
}

fn sample_actions() -> Vec<ExecutionAction> {
    vec![ExecutionAction {
        action_id: "a1".into(),
        action_name: "record_mutation".into(),
        inputs: serde_json::json!({
            "resource_key": "issue/1",
            "field": "label",
            "value": "bug"
        }),
        continue_on_failure: false,
    }]
}

#[test]
fn create_plan_requires_accepted_proposal() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(LocalStore::open(dir.path()).unwrap());
    let runtime = Arc::new(Runtime::new(store));
    runtime
        .register_execution_capability(Arc::new(MockExecutionCapability::new()))
        .unwrap();
    let caps = CapabilityService::new(runtime);
    let inv = caps.create_investigation("x", None, "t").unwrap();
    let draft = caps
        .create_improvement_proposal(
            inv.id,
            CreateProposalRequest {
                title: "t".into(),
                summary: "s".into(),
                rationale: "r".into(),
                category: ProposalCategory::Code,
                priority: ProposalPriority::Low,
                confidence: Confidence::none(),
                supporting_evidence_ids: vec![],
                contradicting_evidence_ids: vec![],
                source_recommendation_ids: vec![],
                affected_components: vec![],
                affected_resources: vec![],
            },
            "t",
        )
        .unwrap();
    let err = caps
        .create_execution_plan(
            inv.id,
            CreateExecutionPlanRequest {
                proposal_id: draft.id,
                capability_id: "mock.record".into(),
                target_system: "mock".into(),
                target_environment: "sandbox".into(),
                actions: sample_actions(),
                inputs: serde_json::json!({}),
                expected_effects: vec![],
                preconditions: vec![],
                supports_dry_run: true,
            },
            "t",
        )
        .unwrap_err();
    assert!(err.to_string().contains("accepted proposal"));
}

#[test]
fn plan_lifecycle_validate_approve_does_not_execute() {
    let (caps, inv, proposal_id) = setup();
    let plan = caps
        .create_execution_plan(
            inv,
            CreateExecutionPlanRequest {
                proposal_id,
                capability_id: "mock.record".into(),
                target_system: "mock".into(),
                target_environment: "sandbox".into(),
                actions: sample_actions(),
                inputs: serde_json::json!({}),
                expected_effects: vec![],
                preconditions: vec![],
                supports_dry_run: true,
            },
            "planner",
        )
        .unwrap();
    assert_eq!(plan.status, ExecutionPlanStatus::Draft);
    let ready = caps
        .validate_execution_plan(inv, plan.id, "planner", "scope ok")
        .unwrap();
    assert_eq!(ready.status, ExecutionPlanStatus::ReadyForReview);
    let (approved, approval) = caps
        .approve_execution_plan(
            inv,
            ready.id,
            "approver",
            "ship",
            vec![],
            vec![],
            None,
            true,
        )
        .unwrap();
    assert_eq!(approved.status, ExecutionPlanStatus::Approved);
    assert_eq!(approval.plan_id, approved.id);
    assert_eq!(approval.plan_revision_number, approved.revision_number);
    // No attempts yet — approval ≠ execution.
    let attempts = caps.list_execution_attempts(inv).unwrap();
    assert!(attempts.attempts.is_empty());
}

#[test]
fn stale_approval_after_revise_is_rejected() {
    let (caps, inv, proposal_id) = setup();
    let plan = caps
        .create_execution_plan(
            inv,
            CreateExecutionPlanRequest {
                proposal_id,
                capability_id: "mock.record".into(),
                target_system: "mock".into(),
                target_environment: "sandbox".into(),
                actions: sample_actions(),
                inputs: serde_json::json!({}),
                expected_effects: vec![],
                preconditions: vec![],
                supports_dry_run: true,
            },
            "planner",
        )
        .unwrap();
    let ready = caps
        .validate_execution_plan(inv, plan.id, "planner", "ok")
        .unwrap();
    let (approved, approval) = caps
        .approve_execution_plan(inv, ready.id, "approver", "ok", vec![], vec![], None, true)
        .unwrap();
    let revised = caps
        .revise_execution_plan(
            inv,
            approved.id,
            rivora::runtime::execution::ReviseExecutionPlanRequest {
                inputs: Some(serde_json::json!({"note": "changed"})),
                ..Default::default()
            },
            "planner",
            "change inputs",
        )
        .unwrap();
    assert_eq!(revised.status, ExecutionPlanStatus::Draft);
    // Old approval is invalidated.
    let err = caps
        .execute_plan(inv, revised.id, approval.id, "runner", "key-1", false)
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("invalidated")
            || msg.contains("stale")
            || msg.contains("approved")
            || msg.contains("precondition"),
        "unexpected: {msg}"
    );
}

#[test]
fn policy_denies_unknown_and_high_risk_capabilities() {
    let denied = evaluate_execution_policy(None, "nope", "sandbox", 1, true);
    assert_eq!(denied.decision, ExecutionPolicyDecisionKind::Denied);

    let high = ExecutionCapabilityDescriptor {
        capability_id: "dangerous".into(),
        name: "Dangerous Merge".into(),
        version: "1".into(),
        provider: "test".into(),
        operation: "merge".into(),
        risk_level: CapabilityRiskLevel::HighRiskWrite,
        mutating: true,
        supported_actions: vec!["merge".into()],
        required_inputs: vec![],
        permissions: vec![],
        supports_dry_run: false,
        idempotency_behavior: "none".into(),
        reversibility: "none".into(),
        verification_method: "none".into(),
        credential_requirements: vec![],
        target_restrictions: vec![],
        failure_semantics: "fail".into(),
        description: "denied".into(),
        output_types: vec![],
        limitations: vec!["policy denied".into()],
        engineering_loop: EngineeringLoopParticipation::execution_capability_default(),
        accepted_input_types: default_accepted_input_types("dangerous"),
        provider_independent: true,
    };
    let d = evaluate_execution_policy(Some(&high), "dangerous", "production", 1, false);
    assert_eq!(d.decision, ExecutionPolicyDecisionKind::Denied);
}

#[test]
fn storage_isolates_corrupt_execution_plans() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::open(dir.path()).unwrap();
    let inv = rivora::Investigation::create("t", None, Provenance::now("t", "t")).unwrap();
    store.save_investigation(&inv).unwrap();
    let plan = ExecutionPlan::draft(
        inv.id,
        ObjectId::new(),
        ObjectId::new(),
        1,
        "mock.record",
        "mock",
        "sandbox",
        sample_actions(),
        Provenance::now("t", "t"),
    )
    .unwrap();
    store.append_execution_plan(&plan).unwrap();
    let bad = dir
        .path()
        .join("investigations")
        .join(inv.id.to_string())
        .join("execution_plans")
        .join("corrupt.json");
    std::fs::write(&bad, "{not json").unwrap();
    let listing = store.list_execution_plans(&inv.id).unwrap();
    assert_eq!(listing.plans.len(), 1);
    assert!(!listing.diagnostics.is_empty());
}

#[test]
fn acceptance_never_creates_execution_plan() {
    let (caps, inv, _proposal_id) = setup();
    let listing = caps.list_execution_plans(inv).unwrap();
    assert!(listing.plans.is_empty());
}
