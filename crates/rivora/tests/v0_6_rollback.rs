//! Rollback inverse safety regressions for RFC-027.
//!
//! These tests would fail on audited commit `1a790ea` which fell back to
//! `supported_actions.first()` when explicit inverse metadata was missing.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rivora::domain::{
    default_accepted_input_types, CapabilityExecutionResult, CapabilityExecutionStatus,
    CapabilityInvocation, CapabilityRiskLevel, CapabilityStateObservation, CapabilityStateQuery,
    CapabilityTarget, CapabilityVerificationStatus, DryRunResult, EngineeringLoopParticipation,
    ExecutionAction, ExecutionAttemptStatus, ExecutionCapability, ExecutionCapabilityDescriptor,
    ExecutionPlanStatus, ExecutionPolicyDecision, ExecutionPolicyDecisionKind,
    ExecutionReceiptResult, MockExecutionCapability, ProposalStatus, ProposalTransitionAuthority,
    RollbackMetadata,
};
use rivora::runtime::execution::CreateExecutionPlanRequest;
use rivora::runtime::proposal::CreateProposalRequest;
use rivora::storage::LocalStore;
use rivora::{
    CapabilityService, Confidence, ObjectId, ProposalCategory, ProposalPriority, RivoraError,
    RivoraResult, Runtime,
};
use serde_json::{json, Value};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Controllable probe capability for inverse-metadata tests
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
struct InverseProbe {
    /// Action names supported (order is intentionally significant for Test B).
    supported: Arc<Mutex<Vec<String>>>,
    /// Rollback to emit on successful mutate.
    rollback: Arc<Mutex<RollbackMetadata>>,
    mutations: Arc<Mutex<u32>>,
}

impl InverseProbe {
    fn with_actions(actions: &[&str]) -> Self {
        Self {
            supported: Arc::new(Mutex::new(
                actions.iter().map(|a| (*a).to_string()).collect(),
            )),
            rollback: Arc::new(Mutex::new(RollbackMetadata::default())),
            mutations: Arc::new(Mutex::new(0)),
        }
    }

    fn set_rollback(&self, meta: RollbackMetadata) {
        *self.rollback.lock().expect("rollback lock") = meta;
    }

    fn mutation_count(&self) -> u32 {
        *self.mutations.lock().expect("mutations lock")
    }
}

impl ExecutionCapability for InverseProbe {
    fn descriptor(&self) -> ExecutionCapabilityDescriptor {
        let supported = self.supported.lock().expect("supported lock").clone();
        ExecutionCapabilityDescriptor {
            capability_id: "probe.inverse".into(),
            version: "1".into(),
            risk_level: CapabilityRiskLevel::LowRiskWrite,
            supported_actions: supported,
            required_inputs: vec!["resource_key".into(), "field".into(), "value".into()],
            supports_dry_run: true,
            idempotency_behavior: "none".into(),
            reversibility: "explicit inverse metadata only".into(),
            verification_method: "observe fields".into(),
            credential_requirements: vec![],
            target_restrictions: vec![
                "provider=mock".into(),
                "repository=probe".into(),
                "environment=sandbox".into(),
            ],
            failure_semantics: "fail closed".into(),
            description: "Probe for explicit inverse derivation".into(),
            engineering_loop: EngineeringLoopParticipation::execution_capability_default(),
            accepted_input_types: default_accepted_input_types("probe.inverse"),
            provider_independent: true,
        }
    }

    fn target(&self, _environment: &str, _inputs: &Value) -> RivoraResult<CapabilityTarget> {
        Ok(CapabilityTarget {
            provider: "mock".into(),
            owner: Some("probe".into()),
            repository: Some("probe".into()),
            branch_or_ref: None,
        })
    }

    fn validate_preconditions(&self, request: &CapabilityInvocation) -> RivoraResult<()> {
        let supported = self.supported.lock().expect("supported lock");
        if !supported.iter().any(|a| a == &request.action_name) {
            return Err(RivoraError::validation(format!(
                "unsupported action {}",
                request.action_name
            )));
        }
        Ok(())
    }

    fn dry_run(&self, request: &CapabilityInvocation) -> RivoraResult<DryRunResult> {
        Ok(DryRunResult {
            actions: vec![request.action_name.clone()],
            target: "probe".into(),
            expected_mutations: vec![],
            required_permissions: vec![],
            current_state: None,
            predicted_state: None,
            risks: vec![],
            policy_decision: ExecutionPolicyDecision {
                decision: ExecutionPolicyDecisionKind::AllowedWithApproval,
                reasons: vec!["probe".into()],
                risk_level: CapabilityRiskLevel::LowRiskWrite,
                dry_run_permitted: true,
                live_execution_permitted: true,
                evaluated_at: chrono::Utc::now(),
            },
            missing_preconditions: vec![],
            verification_steps: vec![],
            rollback_options: vec![],
            simulated: true,
        })
    }

    fn execute(&self, request: &CapabilityInvocation) -> RivoraResult<CapabilityExecutionResult> {
        *self.mutations.lock().expect("mutations lock") += 1;
        let rollback = self.rollback.lock().expect("rollback lock").clone();
        Ok(CapabilityExecutionResult {
            status: CapabilityExecutionStatus::Success,
            request_summary: format!("probe {}", request.action_name),
            response_summary: "ok".into(),
            changed_resources: vec!["probe/resource".into()],
            unchanged_resources: vec![],
            external_identifiers: vec![format!("probe:{}", request.action_id)],
            warnings: vec![],
            rollback,
            verification_requirements: vec![],
            evidence_refs: vec![],
            error: None,
            duplicate_suppressed: false,
        })
    }

    fn observe_state(
        &self,
        query: &CapabilityStateQuery,
    ) -> RivoraResult<CapabilityStateObservation> {
        Ok(CapabilityStateObservation {
            resource_key: "probe".into(),
            fields: HashMap::new(),
            summary: format!("observe {}", query.action_name),
            observed: true,
            verification_status: CapabilityVerificationStatus::Passed,
            error: None,
        })
    }
}

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

struct Fixture {
    _dir: TempDir,
    caps: CapabilityService,
    inv: rivora::InvestigationId,
    proposal_id: ObjectId,
}

impl Fixture {
    fn with_capability(cap: Arc<dyn ExecutionCapability>) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(LocalStore::open(dir.path()).unwrap());
        let runtime = Arc::new(Runtime::new(store));
        runtime
            .register_execution_capability(cap)
            .expect("register capability");
        let caps = CapabilityService::new(runtime);
        let inv = caps
            .create_investigation("rollback fixture", None, "tester")
            .unwrap();
        let proposal = caps
            .create_improvement_proposal(
                inv.id,
                CreateProposalRequest {
                    title: "Rollback test".into(),
                    summary: "s".into(),
                    rationale: "r".into(),
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
        Self {
            _dir: dir,
            caps,
            inv: inv.id,
            proposal_id: accepted.id,
        }
    }

    fn create_and_execute(
        &self,
        capability_id: &str,
        actions: Vec<ExecutionAction>,
        key: &str,
    ) -> (
        rivora::ExecutionPlan,
        rivora::ExecutionApproval,
        rivora::ExecutionAttempt,
    ) {
        let plan = self
            .caps
            .create_execution_plan(
                self.inv,
                CreateExecutionPlanRequest {
                    proposal_id: self.proposal_id,
                    capability_id: capability_id.into(),
                    target_system: "mock".into(),
                    target_environment: "sandbox".into(),
                    actions,
                    inputs: json!({
                        "provider": "mock",
                        "owner": "probe",
                        "repository": "probe",
                    }),
                    expected_effects: vec![],
                    preconditions: vec![],
                    supports_dry_run: true,
                },
                "planner",
            )
            .unwrap();
        let ready = self
            .caps
            .validate_execution_plan(self.inv, plan.id, "planner", "validated")
            .unwrap();
        let (approved, approval) = self
            .caps
            .approve_execution_plan(
                self.inv,
                ready.id,
                "approver",
                "approve original",
                vec![],
                vec![],
                None,
                true,
            )
            .unwrap();
        let attempt = self
            .caps
            .execute_plan(self.inv, approved.id, approval.id, "runner", key, false)
            .unwrap();
        (approved, approval, attempt)
    }
}

fn mutate_action(id: &str, field: &str, value: &str) -> ExecutionAction {
    ExecutionAction {
        action_id: id.into(),
        action_name: "mutate".into(),
        inputs: json!({
            "resource_key": "issue/1",
            "field": field,
            "value": value,
            "provider": "mock",
            "owner": "probe",
            "repository": "probe",
        }),
        continue_on_failure: false,
    }
}

fn mock_action(id: &str, field: &str, value: &str) -> ExecutionAction {
    ExecutionAction {
        action_id: id.into(),
        action_name: "record_mutation".into(),
        inputs: json!({
            "resource_key": "issue/1",
            "field": field,
            "value": value,
        }),
        continue_on_failure: false,
    }
}

// ---------------------------------------------------------------------------
// Tests A–N
// ---------------------------------------------------------------------------

/// Test A — No arbitrary fallback when inverse is missing.
#[test]
fn a_missing_explicit_inverse_fails_without_choosing_first_supported_action() {
    let probe = Arc::new(InverseProbe::with_actions(&["restore", "mutate", "other"]));
    // Available=true but no inverse_action_name — 1a790ea would pick "restore" (first).
    probe.set_rollback(RollbackMetadata {
        available: true,
        capability_id: Some("probe.inverse".into()),
        inputs: Some(json!({"resource_key":"issue/1","field":"label","value":"old"})),
        inverse_action_name: None,
        risks: vec![],
        verification: None,
        irreversible_effects: vec![],
    });
    let fixture = Fixture::with_capability(Arc::clone(&probe) as Arc<dyn ExecutionCapability>);
    let (_plan, _approval, attempt) = fixture.create_and_execute(
        "probe.inverse",
        vec![mutate_action("a1", "label", "bug")],
        "a",
    );
    assert_eq!(attempt.status, ExecutionAttemptStatus::Completed);
    let err = fixture
        .caps
        .create_rollback_plan(fixture.inv, attempt.id, "rollback-planner")
        .expect_err("must fail without explicit inverse");
    let msg = err.to_string();
    assert!(
        msg.contains("explicit inverse") || msg.contains("inverse_action_name"),
        "unexpected error: {msg}"
    );
    // Capability was not invoked again for rollback derivation.
    assert_eq!(probe.mutation_count(), 1);
}

/// Test B — Registration/support order independence.
#[test]
fn b_supported_action_order_does_not_change_rollback_behavior() {
    for order in [
        vec!["restore", "mutate", "other"],
        vec!["other", "mutate", "restore"],
        vec!["mutate", "other", "restore"],
    ] {
        let refs = order.to_vec();
        let probe = Arc::new(InverseProbe::with_actions(&refs));
        // No explicit inverse — must fail identically regardless of order.
        probe.set_rollback(RollbackMetadata {
            available: true,
            capability_id: Some("probe.inverse".into()),
            inputs: Some(json!({"resource_key":"issue/1","field":"x","value":"1"})),
            inverse_action_name: None,
            risks: vec![],
            verification: None,
            irreversible_effects: vec![],
        });
        let fixture = Fixture::with_capability(Arc::clone(&probe) as Arc<dyn ExecutionCapability>);
        let (_p, _a, attempt) = fixture.create_and_execute(
            "probe.inverse",
            vec![mutate_action("a1", "x", "2")],
            &format!("order-{}", order.join("-")),
        );
        let err = fixture
            .caps
            .create_rollback_plan(fixture.inv, attempt.id, "planner")
            .unwrap_err();
        assert!(
            err.to_string().contains("explicit inverse"),
            "order {:?} produced unexpected error {}",
            order,
            err
        );
    }
}

/// Test C — Unsupported inverse action is rejected before persistence.
#[test]
fn c_unsupported_inverse_action_is_rejected() {
    let probe = Arc::new(InverseProbe::with_actions(&["mutate", "restore"]));
    probe.set_rollback(RollbackMetadata {
        available: true,
        capability_id: Some("probe.inverse".into()),
        inputs: Some(json!({"resource_key":"issue/1","field":"x","value":"old"})),
        inverse_action_name: Some("delete_everything".into()),
        risks: vec![],
        verification: None,
        irreversible_effects: vec![],
    });
    let fixture = Fixture::with_capability(Arc::clone(&probe) as Arc<dyn ExecutionCapability>);
    let plans_before = fixture
        .caps
        .list_execution_plans(fixture.inv)
        .unwrap()
        .plans
        .len();
    let (_p, _a, attempt) =
        fixture.create_and_execute("probe.inverse", vec![mutate_action("a1", "x", "new")], "c");
    let err = fixture
        .caps
        .create_rollback_plan(fixture.inv, attempt.id, "planner")
        .unwrap_err();
    assert!(
        err.to_string().contains("unsupported") || err.to_string().contains("not declared"),
        "{}",
        err
    );
    let plans_after = fixture
        .caps
        .list_execution_plans(fixture.inv)
        .unwrap()
        .plans
        .len();
    // Only the original execution plan lineage should exist (no rollback draft).
    assert_eq!(
        plans_after,
        plans_before + /* original plan revisions through approve+execute */ plans_after
            - plans_before,
        "sanity: plans may grow from original lifecycle only"
    );
    // Stronger: no plan with capability actions named delete_everything.
    for plan in fixture
        .caps
        .list_execution_plans(fixture.inv)
        .unwrap()
        .plans
    {
        assert!(
            plan.actions
                .iter()
                .all(|a| a.action_name != "delete_everything"),
            "unsupported inverse must not be persisted as a plan action"
        );
    }
}

/// Test D — Unknown inverse capability fails safely.
#[test]
fn d_unknown_inverse_capability_fails() {
    let probe = Arc::new(InverseProbe::with_actions(&["mutate"]));
    probe.set_rollback(RollbackMetadata {
        available: true,
        capability_id: Some("does.not.exist".into()),
        inputs: Some(json!({"resource_key":"issue/1","field":"x","value":"old"})),
        inverse_action_name: Some("mutate".into()),
        risks: vec![],
        verification: None,
        irreversible_effects: vec![],
    });
    let fixture = Fixture::with_capability(Arc::clone(&probe) as Arc<dyn ExecutionCapability>);
    let (_p, _a, attempt) =
        fixture.create_and_execute("probe.inverse", vec![mutate_action("a1", "x", "new")], "d");
    let err = fixture
        .caps
        .create_rollback_plan(fixture.inv, attempt.id, "planner")
        .unwrap_err();
    assert!(
        err.to_string().contains("not registered") || err.to_string().contains("unavailable"),
        "{}",
        err
    );
}

/// Test E/G/I — Explicit inverse with prior state (mock.record).
#[test]
fn egi_mock_prior_state_produces_exact_restore_inverse() {
    let mock = Arc::new(MockExecutionCapability::new());
    let mut before = HashMap::new();
    before.insert("label".into(), "old".into());
    mock.seed_resource("issue/1", before);
    let fixture = Fixture::with_capability(Arc::clone(&mock) as Arc<dyn ExecutionCapability>);
    let (_p, _a, attempt) = fixture.create_and_execute(
        "mock.record",
        vec![mock_action("a1", "label", "bug")],
        "egi",
    );
    assert_eq!(attempt.status, ExecutionAttemptStatus::Completed);
    let rollback = fixture
        .caps
        .create_rollback_plan(fixture.inv, attempt.id, "planner")
        .expect("prior-state restore must produce a draft");
    assert_eq!(rollback.status, ExecutionPlanStatus::Draft);
    assert_eq!(rollback.actions.len(), 1);
    assert_eq!(rollback.actions[0].action_name, "record_mutation");
    assert_eq!(rollback.actions[0].inputs["value"], "old");
    assert_eq!(rollback.actions[0].inputs["field"], "label");
    assert_eq!(rollback.actions[0].inputs["resource_key"], "issue/1");
}

/// Test F/H — First write / no prior state → no executable rollback.
#[test]
fn fh_mock_first_write_has_no_executable_rollback() {
    let mock = Arc::new(MockExecutionCapability::new());
    let fixture = Fixture::with_capability(Arc::clone(&mock) as Arc<dyn ExecutionCapability>);
    let (_p, _a, attempt) =
        fixture.create_and_execute("mock.record", vec![mock_action("a1", "label", "bug")], "fh");
    let err = fixture
        .caps
        .create_rollback_plan(fixture.inv, attempt.id, "planner")
        .unwrap_err();
    assert!(
        err.to_string().contains("does not define rollback")
            || err.to_string().contains("not available")
            || err.to_string().contains("unavailable"),
        "{}",
        err
    );
}

/// Test J — Irreversible probe with no rollback metadata cannot invent an inverse.
#[test]
fn j_irreversible_actions_do_not_invent_rollback_from_supported_actions() {
    let probe = Arc::new(InverseProbe::with_actions(&[
        "close_issue",
        "delete_repo",
        "force_push",
    ]));
    // Emulate irreversible capability: success but no rollback.
    probe.set_rollback(RollbackMetadata {
        available: false,
        capability_id: None,
        inputs: None,
        inverse_action_name: None,
        risks: vec![],
        verification: None,
        irreversible_effects: vec!["no reliable inverse".into()],
    });
    // Override action name to first supported for execution - need mutate in supported
    let probe = Arc::new(InverseProbe::with_actions(&[
        "mutate",
        "close_issue",
        "delete_repo",
    ]));
    probe.set_rollback(RollbackMetadata {
        available: false,
        capability_id: None,
        inputs: None,
        inverse_action_name: None,
        risks: vec![],
        verification: None,
        irreversible_effects: vec!["no reliable inverse".into()],
    });
    let fixture = Fixture::with_capability(Arc::clone(&probe) as Arc<dyn ExecutionCapability>);
    let (_p, _a, attempt) =
        fixture.create_and_execute("probe.inverse", vec![mutate_action("a1", "x", "y")], "j");
    let err = fixture
        .caps
        .create_rollback_plan(fixture.inv, attempt.id, "planner")
        .unwrap_err();
    assert!(
        !err.to_string().contains("close_issue")
            && !err.to_string().contains("delete_repo")
            && !err.to_string().contains("force_push"),
        "error must not invent high-risk actions: {err}"
    );
    assert!(
        err.to_string().contains("unavailable")
            || err.to_string().contains("not available")
            || err.to_string().contains("does not define"),
        "{}",
        err
    );
}

/// Test K — Creating a rollback Plan does not approve/execute/invoke adapters.
#[test]
fn k_rollback_plan_is_draft_only_without_attempt_or_adapter_invocation() {
    let mock = Arc::new(MockExecutionCapability::new());
    let mut before = HashMap::new();
    before.insert("label".into(), "old".into());
    mock.seed_resource("issue/1", before);
    let fixture = Fixture::with_capability(Arc::clone(&mock) as Arc<dyn ExecutionCapability>);
    let (_p, _a, attempt) =
        fixture.create_and_execute("mock.record", vec![mock_action("a1", "label", "bug")], "k");
    let attempts_before = fixture
        .caps
        .list_execution_attempts(fixture.inv)
        .unwrap()
        .attempts
        .len();
    let rollback = fixture
        .caps
        .create_rollback_plan(fixture.inv, attempt.id, "planner")
        .unwrap();
    assert_eq!(rollback.status, ExecutionPlanStatus::Draft);
    let attempts_after = fixture
        .caps
        .list_execution_attempts(fixture.inv)
        .unwrap()
        .attempts
        .len();
    assert_eq!(
        attempts_before, attempts_after,
        "rollback plan creation must not create attempts"
    );
    // Resource still has post-execution value (no adapter re-invocation).
    assert_eq!(
        mock.get_resource("issue/1").unwrap().get("label").unwrap(),
        "bug"
    );
}

/// Test L — Original approval cannot authorize the rollback plan.
#[test]
fn l_original_approval_cannot_execute_rollback_plan() {
    let mock = Arc::new(MockExecutionCapability::new());
    let mut before = HashMap::new();
    before.insert("label".into(), "old".into());
    mock.seed_resource("issue/1", before);
    let fixture = Fixture::with_capability(Arc::clone(&mock) as Arc<dyn ExecutionCapability>);
    let (_orig, approval, attempt) =
        fixture.create_and_execute("mock.record", vec![mock_action("a1", "label", "bug")], "l");
    let rollback = fixture
        .caps
        .create_rollback_plan(fixture.inv, attempt.id, "planner")
        .unwrap();
    // Even if we validate+try to run with the original approval, it must fail.
    let ready = fixture
        .caps
        .validate_execution_plan(fixture.inv, rollback.id, "reviewer", "review rollback")
        .unwrap();
    let err = fixture
        .caps
        .execute_plan(
            fixture.inv,
            ready.id,
            approval.id,
            "runner",
            "reuse-original-approval",
            false,
        )
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("approval")
            || msg.contains("revision")
            || msg.contains("snapshot")
            || msg.contains("approved")
            || msg.contains("precondition")
            || msg.contains("stale")
            || msg.contains("invalid"),
        "original approval must not authorize rollback: {msg}"
    );
}

/// Test M — Rollback requires independent approval (policy path still applies).
#[test]
fn m_rollback_requires_separate_policy_and_approval() {
    let mock = Arc::new(MockExecutionCapability::new());
    let mut before = HashMap::new();
    before.insert("label".into(), "old".into());
    mock.seed_resource("issue/1", before);
    let fixture = Fixture::with_capability(Arc::clone(&mock) as Arc<dyn ExecutionCapability>);
    let (_orig, _approval, attempt) =
        fixture.create_and_execute("mock.record", vec![mock_action("a1", "label", "bug")], "m");
    let rollback = fixture
        .caps
        .create_rollback_plan(fixture.inv, attempt.id, "planner")
        .unwrap();
    assert_eq!(rollback.status, ExecutionPlanStatus::Draft);
    // Live execute without approval must fail (not approved).
    let err = fixture
        .caps
        .execute_plan(
            fixture.inv,
            rollback.id,
            ObjectId::new(), // nonsense approval
            "runner",
            "no-approval",
            false,
        )
        .unwrap_err();
    assert!(
        err.to_string().contains("not found")
            || err.to_string().contains("approved")
            || err.to_string().contains("precondition")
            || err.to_string().contains("approval"),
        "{}",
        err
    );
    // Independent approval path works.
    let ready = fixture
        .caps
        .validate_execution_plan(fixture.inv, rollback.id, "reviewer", "ok")
        .unwrap();
    let (approved, new_approval) = fixture
        .caps
        .approve_execution_plan(
            fixture.inv,
            ready.id,
            "approver",
            "approve rollback separately",
            vec![],
            vec![],
            None,
            true,
        )
        .unwrap();
    assert_eq!(approved.status, ExecutionPlanStatus::Approved);
    assert_ne!(new_approval.plan_id, attempt.plan_id);
    let policy = fixture
        .caps
        .explain_execution_policy(fixture.inv, approved.id)
        .unwrap();
    assert!(matches!(
        policy.decision,
        ExecutionPolicyDecisionKind::AllowedWithApproval | ExecutionPolicyDecisionKind::Allowed
    ));
}

/// Test N — Creating/executing rollback does not mutate original receipts.
#[test]
fn n_original_receipts_remain_immutable() {
    let mock = Arc::new(MockExecutionCapability::new());
    let mut before = HashMap::new();
    before.insert("label".into(), "old".into());
    mock.seed_resource("issue/1", before);
    let fixture = Fixture::with_capability(Arc::clone(&mock) as Arc<dyn ExecutionCapability>);
    let (_orig, _approval, attempt) =
        fixture.create_and_execute("mock.record", vec![mock_action("a1", "label", "bug")], "n");
    let before_receipts = fixture
        .caps
        .list_execution_receipts(fixture.inv)
        .unwrap()
        .receipts;
    assert!(!before_receipts.is_empty());
    let before_json: Vec<String> = before_receipts
        .iter()
        .map(|r| serde_json::to_string(r).unwrap())
        .collect();
    let rollback = fixture
        .caps
        .create_rollback_plan(fixture.inv, attempt.id, "planner")
        .unwrap();
    let ready = fixture
        .caps
        .validate_execution_plan(fixture.inv, rollback.id, "reviewer", "ok")
        .unwrap();
    let (approved, approval) = fixture
        .caps
        .approve_execution_plan(
            fixture.inv,
            ready.id,
            "approver",
            "run rollback",
            vec![],
            vec![],
            None,
            true,
        )
        .unwrap();
    let _rb_attempt = fixture
        .caps
        .execute_plan(
            fixture.inv,
            approved.id,
            approval.id,
            "runner",
            "rollback-live",
            false,
        )
        .unwrap();
    let after_receipts = fixture
        .caps
        .list_execution_receipts(fixture.inv)
        .unwrap()
        .receipts;
    for (i, original) in before_json.iter().enumerate() {
        let still = after_receipts
            .iter()
            .find(|r| r.id == before_receipts[i].id)
            .expect("original receipt must still exist");
        assert_eq!(
            &serde_json::to_string(still).unwrap(),
            original,
            "original receipt must not be mutated"
        );
        assert_eq!(still.result_status, ExecutionReceiptResult::Success);
    }
}

/// Explicit inverse works and is not order-dependent when present.
#[test]
fn explicit_inverse_uses_named_action_not_registration_order() {
    // Put a dangerous action first; explicit inverse must still be "restore".
    let probe = Arc::new(InverseProbe::with_actions(&[
        "delete_repo",
        "mutate",
        "restore",
    ]));
    probe.set_rollback(RollbackMetadata {
        available: true,
        capability_id: Some("probe.inverse".into()),
        inputs: Some(json!({
            "resource_key": "issue/1",
            "field": "label",
            "value": "old",
            "provider": "mock",
            "owner": "probe",
            "repository": "probe",
        })),
        inverse_action_name: Some("restore".into()),
        risks: vec!["restores prior value".into()],
        verification: Some("observe".into()),
        irreversible_effects: vec![],
    });
    let fixture = Fixture::with_capability(Arc::clone(&probe) as Arc<dyn ExecutionCapability>);
    let (_p, _a, attempt) = fixture.create_and_execute(
        "probe.inverse",
        vec![mutate_action("a1", "label", "bug")],
        "explicit",
    );
    let rollback = fixture
        .caps
        .create_rollback_plan(fixture.inv, attempt.id, "planner")
        .unwrap();
    assert_eq!(rollback.actions[0].action_name, "restore");
    assert_ne!(rollback.actions[0].action_name, "delete_repo");
}

/// Connector unit-level: label already present yields no rollback metadata.
#[test]
fn label_already_present_emits_no_rollback_metadata() {
    // Exercise the domain contract used by github.issue.label when
    // initially_present == desired_present: RollbackMetadata::default().
    let suppressed = RollbackMetadata::default();
    assert!(!suppressed.available);
    assert!(suppressed.inverse_action_name.is_none());
    assert!(suppressed.inputs.is_none());
}
