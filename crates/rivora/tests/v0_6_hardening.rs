//! Release-hardening regressions for RFC-025, RFC-026, and RFC-027.
//!
//! These tests intentionally exercise the durable Runtime boundary. They do not
//! call connector mutation APIs directly and they keep every filesystem fixture
//! isolated in a temporary store.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use rivora::domain::{
    CapabilityExecutionResult, CapabilityExecutionStatus, CapabilityInvocation,
    CapabilityRiskLevel, CapabilityStateObservation, CapabilityStateQuery, CapabilityTarget,
    CapabilityVerificationStatus, DryRunResult, ExecutionAction, ExecutionAttemptStatus,
    ExecutionCapability, ExecutionCapabilityDescriptor, ExecutionPlanStatus,
    ExecutionPolicyDecision, ExecutionPolicyDecisionKind, ExecutionReceiptResult,
    ExecutionVerificationStatus, ExpectedEffect, MockExecutionCapability, ProposalStatus,
    ProposalTransitionAuthority, RetrySafety, RollbackMetadata,
};
use rivora::runtime::execution::{CreateExecutionPlanRequest, ReviseExecutionPlanRequest};
use rivora::runtime::proposal::CreateProposalRequest;
use rivora::storage::LocalStore;
use rivora::{
    CapabilityService, Confidence, InvestigationId, ObjectId, ProposalCategory, ProposalPriority,
    RivoraError, RivoraResult, Runtime, Store,
};
use serde_json::{json, Value};
use tempfile::TempDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbeMode {
    Success,
    Failed,
    Uncertain,
    MissingCredentials,
    PanicAfterReservation,
}

#[derive(Clone)]
struct ProbeCapability {
    mode: ProbeMode,
    store_root: PathBuf,
    execute_count: Arc<AtomicUsize>,
    observed: Arc<Mutex<HashMap<String, HashMap<String, String>>>>,
    repository: Arc<Mutex<String>>,
}

impl ProbeCapability {
    fn new(mode: ProbeMode, store_root: impl Into<PathBuf>) -> Self {
        Self {
            mode,
            store_root: store_root.into(),
            execute_count: Arc::new(AtomicUsize::new(0)),
            observed: Arc::new(Mutex::new(HashMap::new())),
            repository: Arc::new(Mutex::new("rivora".into())),
        }
    }

    fn execute_count(&self) -> usize {
        self.execute_count.load(Ordering::SeqCst)
    }

    fn set_repository(&self, repository: &str) {
        *self.repository.lock().expect("probe repository lock") = repository.into();
    }

    fn assert_started_attempt_is_durable(&self, request: &CapabilityInvocation) {
        let attempts_root = self.store_root.join("investigations");
        let mut found = false;
        for investigation in fs::read_dir(&attempts_root).expect("investigations directory") {
            let attempts = investigation
                .expect("investigation directory entry")
                .path()
                .join("execution_attempts");
            let Ok(entries) = fs::read_dir(attempts) else {
                continue;
            };
            for entry in entries {
                let path = entry.expect("attempt directory entry").path();
                if path.extension().and_then(|value| value.to_str()) != Some("json") {
                    continue;
                }
                let value: Value =
                    serde_json::from_slice(&fs::read(path).expect("read durable attempt"))
                        .expect("valid durable attempt");
                let invocation_key_prefix = request
                    .idempotency_key
                    .strip_suffix(&format!(";action={}", request.action_id))
                    .or_else(|| {
                        request
                            .idempotency_key
                            .strip_suffix(&format!(":{}", request.action_id))
                    })
                    .unwrap_or(&request.idempotency_key);
                if value.get("status") == Some(&Value::String("started".into()))
                    && value.get("idempotency_key")
                        == Some(&Value::String(invocation_key_prefix.into()))
                {
                    found = true;
                }
            }
        }
        assert!(
            found,
            "a Started attempt and idempotency reservation must be persisted before mutation"
        );
    }

    fn success_result(request: &CapabilityInvocation) -> CapabilityExecutionResult {
        CapabilityExecutionResult {
            status: CapabilityExecutionStatus::Success,
            request_summary: format!("probe {}", request.action_id),
            response_summary: "probe mutation accepted".into(),
            changed_resources: vec!["probe/resource".into()],
            unchanged_resources: vec![],
            external_identifiers: vec![format!("probe:{}", request.action_id)],
            warnings: vec![],
            rollback: RollbackMetadata::default(),
            verification_requirements: vec!["observe exact field".into()],
            evidence_refs: vec![format!("probe-observation:{}", request.action_id)],
            error: None,
            duplicate_suppressed: false,
        }
    }
}

impl ExecutionCapability for ProbeCapability {
    fn descriptor(&self) -> ExecutionCapabilityDescriptor {
        ExecutionCapabilityDescriptor {
            capability_id: "probe.mutate".into(),
            name: "Probe Mutate".into(),
            version: "1".into(),
            provider: "probe".into(),
            operation: "mutate".into(),
            risk_level: CapabilityRiskLevel::LowRiskWrite,
            mutating: true,
            supported_actions: vec!["mutate".into()],
            required_inputs: vec!["resource_key".into(), "field".into(), "value".into()],
            permissions: vec!["probe:write".into()],
            supports_dry_run: true,
            idempotency_behavior: "runtime durable reservation".into(),
            reversibility: "explicit inverse metadata only".into(),
            verification_method: "independent probe observation".into(),
            credential_requirements: match self.mode {
                ProbeMode::MissingCredentials => vec!["PROBE_TOKEN".into()],
                _ => vec![],
            },
            target_restrictions: vec![
                "provider=github".into(),
                "repository=rivora-dev/rivora".into(),
                "environment=sandbox".into(),
            ],
            failure_semantics: "timeouts are uncertain".into(),
            description: "Hardening test probe".into(),
            output_types: vec!["execution_result".into()],
            limitations: vec!["test-only probe".into()],
            engineering_loop: rivora::EngineeringLoopParticipation::execution_capability_default(),
            accepted_input_types: vec!["event".into()],
            provider_independent: true,
        }
    }

    fn target(&self, _environment: &str, inputs: &Value) -> RivoraResult<CapabilityTarget> {
        Ok(CapabilityTarget {
            provider: inputs["provider"].as_str().unwrap_or("github").into(),
            owner: inputs["owner"].as_str().map(str::to_string),
            repository: Some(
                self.repository
                    .lock()
                    .expect("probe repository lock")
                    .clone(),
            ),
            branch_or_ref: inputs["ref"].as_str().map(str::to_string),
        })
    }

    fn validate_preconditions(&self, request: &CapabilityInvocation) -> RivoraResult<()> {
        if self.mode == ProbeMode::MissingCredentials {
            return Err(RivoraError::precondition(
                "missing required credential PROBE_TOKEN",
            ));
        }
        if request.action_name != "mutate" {
            return Err(RivoraError::validation(format!(
                "unsupported action {}",
                request.action_name
            )));
        }
        for input in ["resource_key", "field", "value"] {
            let value = request.inputs.get(input).and_then(Value::as_str);
            if !matches!(value, Some(value) if !value.is_empty()) {
                return Err(RivoraError::validation(format!(
                    "required input `{input}` is missing"
                )));
            }
        }
        Ok(())
    }

    fn dry_run(&self, request: &CapabilityInvocation) -> RivoraResult<DryRunResult> {
        if self.mode == ProbeMode::MissingCredentials {
            return Err(RivoraError::precondition(
                "missing required credential PROBE_TOKEN",
            ));
        }
        Ok(DryRunResult {
            actions: vec![request.action_name.clone()],
            target: "rivora-dev/rivora".into(),
            expected_mutations: vec!["mutate probe resource".into()],
            required_permissions: vec!["probe:write".into()],
            current_state: Some("before".into()),
            predicted_state: Some("after".into()),
            risks: vec![],
            policy_decision: ExecutionPolicyDecision {
                decision: ExecutionPolicyDecisionKind::AllowedWithApproval,
                reasons: vec!["bounded test mutation".into()],
                risk_level: CapabilityRiskLevel::LowRiskWrite,
                dry_run_permitted: true,
                live_execution_permitted: true,
                evaluated_at: chrono::Utc::now(),
            },
            missing_preconditions: vec![],
            verification_steps: vec!["observe exact field".into()],
            rollback_options: vec![],
            simulated: true,
        })
    }

    fn execute(&self, request: &CapabilityInvocation) -> RivoraResult<CapabilityExecutionResult> {
        self.execute_count.fetch_add(1, Ordering::SeqCst);
        self.assert_started_attempt_is_durable(request);

        if self.mode == ProbeMode::MissingCredentials {
            return Err(RivoraError::precondition(
                "missing required credential PROBE_TOKEN",
            ));
        }
        if self.mode == ProbeMode::PanicAfterReservation {
            panic!("simulated process crash after durable Started reservation");
        }
        if self.mode == ProbeMode::Uncertain {
            return Ok(CapabilityExecutionResult {
                status: CapabilityExecutionStatus::Uncertain,
                request_summary: format!("probe {}", request.action_id),
                response_summary: "request timed out after transmission".into(),
                changed_resources: vec![],
                unchanged_resources: vec![],
                external_identifiers: vec![],
                warnings: vec!["completion is unknown".into()],
                rollback: RollbackMetadata::default(),
                verification_requirements: vec!["determine whether mutation occurred".into()],
                evidence_refs: vec![],
                error: Some("timeout after request transmission".into()),
                duplicate_suppressed: false,
            });
        }
        if self.mode == ProbeMode::Failed {
            return Ok(CapabilityExecutionResult {
                status: CapabilityExecutionStatus::Failed,
                request_summary: format!("probe {}", request.action_id),
                response_summary: "mutation was rejected".into(),
                changed_resources: vec![],
                unchanged_resources: vec!["probe/resource".into()],
                external_identifiers: vec![],
                warnings: vec![],
                rollback: RollbackMetadata::default(),
                verification_requirements: vec!["confirm mutation is absent".into()],
                evidence_refs: vec![],
                error: Some("definite external rejection".into()),
                duplicate_suppressed: false,
            });
        }

        let key = request
            .inputs
            .get("resource_key")
            .and_then(Value::as_str)
            .expect("validated resource_key")
            .to_string();
        let field = request
            .inputs
            .get("field")
            .and_then(Value::as_str)
            .expect("validated field")
            .to_string();
        let value = request
            .inputs
            .get("value")
            .and_then(Value::as_str)
            .expect("validated value")
            .to_string();
        self.observed
            .lock()
            .expect("probe observation lock")
            .entry(key)
            .or_default()
            .insert(field, value);
        Ok(Self::success_result(request))
    }

    fn observe_state(
        &self,
        query: &CapabilityStateQuery,
    ) -> RivoraResult<CapabilityStateObservation> {
        let key = query
            .inputs
            .get("resource_key")
            .and_then(Value::as_str)
            .unwrap_or("probe/resource")
            .to_string();
        let fields = self
            .observed
            .lock()
            .expect("probe observation lock")
            .get(&key)
            .cloned()
            .unwrap_or_default();
        Ok(CapabilityStateObservation {
            resource_key: key,
            observed: !fields.is_empty(),
            fields,
            summary: "independent probe observation".into(),
            verification_status: match self.mode {
                ProbeMode::Uncertain => CapabilityVerificationStatus::Inconclusive,
                ProbeMode::Failed => CapabilityVerificationStatus::Failed,
                _ => CapabilityVerificationStatus::Passed,
            },
            error: None,
        })
    }
}

struct Fixture {
    _temp: TempDir,
    root: PathBuf,
    store: Arc<LocalStore>,
    caps: CapabilityService,
    investigation_id: InvestigationId,
    proposal_id: ObjectId,
}

impl Fixture {
    fn with_capability(capability: Arc<dyn ExecutionCapability>) -> Self {
        let temp = tempfile::tempdir().expect("temporary store");
        let root = temp.path().to_path_buf();
        Self::at_root(temp, root, capability)
    }

    fn probe(mode: ProbeMode) -> (Self, ProbeCapability) {
        let temp = tempfile::tempdir().expect("temporary store");
        let root = temp.path().to_path_buf();
        let probe = ProbeCapability::new(mode, &root);
        let fixture = Self::at_root(temp, root, Arc::new(probe.clone()));
        (fixture, probe)
    }

    fn at_root(temp: TempDir, root: PathBuf, capability: Arc<dyn ExecutionCapability>) -> Self {
        let store = Arc::new(LocalStore::open(&root).expect("open temporary store"));
        let runtime = Arc::new(Runtime::new(store.clone()));
        runtime
            .register_execution_capability(capability)
            .expect("register execution capability");
        let caps = CapabilityService::new(runtime.clone());
        let investigation = caps
            .create_investigation("v0.6 release hardening", None, "test-author")
            .expect("create investigation");
        let proposal = caps
            .create_improvement_proposal(
                investigation.id,
                CreateProposalRequest {
                    title: "Perform bounded mutation".into(),
                    summary: "Exercise v0.6 execution invariants".into(),
                    rationale: "Release audit regression".into(),
                    category: ProposalCategory::Process,
                    priority: ProposalPriority::High,
                    confidence: Confidence::new(0.9),
                    supporting_evidence_ids: vec![],
                    contradicting_evidence_ids: vec![],
                    source_recommendation_ids: vec![],
                    affected_components: vec!["runtime".into()],
                    affected_resources: vec!["rivora-dev/rivora".into()],
                },
                "test-author",
            )
            .expect("create proposal");
        let proposed = caps
            .update_improvement_proposal_status(
                investigation.id,
                proposal.id,
                ProposalStatus::Proposed,
                "test-author",
                "submit",
                ProposalTransitionAuthority::ExternalCaller,
            )
            .expect("propose");
        let reviewing = caps
            .update_improvement_proposal_status(
                investigation.id,
                proposed.id,
                ProposalStatus::UnderReview,
                "reviewer",
                "review",
                ProposalTransitionAuthority::ExternalCaller,
            )
            .expect("review");
        let accepted = caps
            .update_improvement_proposal_status(
                investigation.id,
                reviewing.id,
                ProposalStatus::Accepted,
                "reviewer",
                "accept",
                ProposalTransitionAuthority::ExternalCaller,
            )
            .expect("accept");
        Self {
            _temp: temp,
            root,
            store,
            caps,
            investigation_id: investigation.id,
            proposal_id: accepted.id,
        }
    }

    fn create_plan(
        &self,
        capability_id: &str,
        actions: Vec<ExecutionAction>,
    ) -> RivoraResult<rivora::ExecutionPlan> {
        self.caps.create_execution_plan(
            self.investigation_id,
            CreateExecutionPlanRequest {
                proposal_id: self.proposal_id,
                capability_id: capability_id.into(),
                target_system: if capability_id.starts_with("mock.") {
                    "mock".into()
                } else {
                    "github".into()
                },
                target_environment: "sandbox".into(),
                actions,
                inputs: json!({
                    "provider": "github",
                    "owner": "rivora-dev",
                    "repository": "rivora",
                }),
                expected_effects: vec![ExpectedEffect {
                    description: "exact field mutation".into(),
                    resource_type: "probe".into(),
                    expected_fields: vec![("label".into(), "bug".into())],
                }],
                preconditions: vec![],
                supports_dry_run: true,
            },
            "planner",
        )
    }

    fn approve(
        &self,
        plan: rivora::ExecutionPlan,
    ) -> (rivora::ExecutionPlan, rivora::ExecutionApproval) {
        let ready = self
            .caps
            .validate_execution_plan(self.investigation_id, plan.id, "planner", "validated")
            .expect("validate plan");
        self.caps
            .approve_execution_plan(
                self.investigation_id,
                ready.id,
                "approver",
                "approve exact target and revision",
                vec![],
                vec![],
                None,
                true,
            )
            .expect("approve plan")
    }

    fn plan_path(&self, plan_id: ObjectId) -> PathBuf {
        self.root
            .join("investigations")
            .join(self.investigation_id.to_string())
            .join("execution_plans")
            .join(format!("{plan_id}.json"))
    }

    fn execution_dir(&self, name: &str) -> PathBuf {
        self.root
            .join("investigations")
            .join(self.investigation_id.to_string())
            .join(name)
    }
}

fn probe_action(id: &str) -> ExecutionAction {
    ExecutionAction {
        action_id: id.into(),
        action_name: "mutate".into(),
        inputs: json!({
            "resource_key": "probe/resource",
            "field": "label",
            "value": "bug",
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

fn json_files(path: &Path) -> usize {
    fs::read_dir(path)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| {
                    entry.path().extension().and_then(|value| value.to_str()) == Some("json")
                })
                .count()
        })
        .unwrap_or(0)
}

#[test]
fn approval_binds_exact_repository_and_rejects_repository_drift() {
    let (fixture, probe) = Fixture::probe(ProbeMode::Success);
    let plan = fixture
        .create_plan("probe.mutate", vec![probe_action("a1")])
        .expect("create plan");
    let (approved, approval) = fixture.approve(plan);
    assert_eq!(approval.target_snapshot, approved.target_snapshot);

    let path = fixture.plan_path(approved.id);
    let mut raw: Value =
        serde_json::from_slice(&fs::read(&path).expect("read approved plan")).expect("plan json");
    let snapshot = raw
        .get_mut("target_snapshot")
        .expect("plan must durably contain target_snapshot");
    assert_eq!(
        snapshot.get("provider").and_then(Value::as_str),
        Some("github")
    );
    assert_eq!(
        snapshot.get("owner").and_then(Value::as_str),
        Some("rivora-dev")
    );
    assert_eq!(
        snapshot.get("repository").and_then(Value::as_str),
        Some("rivora")
    );
    assert_eq!(
        snapshot.get("environment").and_then(Value::as_str),
        Some("sandbox")
    );
    assert_eq!(
        snapshot.get("capability_id").and_then(Value::as_str),
        Some("probe.mutate")
    );
    let expected_plan_id = approved.id.to_string();
    assert_eq!(
        snapshot.get("plan_id").and_then(Value::as_str),
        Some(expected_plan_id.as_str())
    );
    assert_eq!(
        snapshot.get("plan_revision_number").and_then(Value::as_u64),
        Some(u64::from(approved.revision_number))
    );
    probe.set_repository("different-repository");

    let error = fixture
        .caps
        .execute_plan(
            fixture.investigation_id,
            approved.id,
            approval.id,
            "runner",
            "repo-drift",
            false,
        )
        .expect_err("repository drift must invalidate approval");
    assert!(
        error.to_string().contains("target")
            || error.to_string().contains("repository")
            || error.to_string().contains("approval"),
        "unexpected repository drift error: {error}"
    );
}

#[test]
fn approval_binds_exact_environment_and_rejects_environment_drift() {
    let (fixture, _probe) = Fixture::probe(ProbeMode::Success);
    let plan = fixture
        .create_plan("probe.mutate", vec![probe_action("a1")])
        .expect("create plan");
    let (approved, approval) = fixture.approve(plan);

    let path = fixture.plan_path(approved.id);
    let mut raw: Value =
        serde_json::from_slice(&fs::read(&path).expect("read approved plan")).expect("plan json");
    raw["target_environment"] = Value::String("production".into());
    raw["target_snapshot"]["environment"] = Value::String("production".into());
    fs::write(
        &path,
        serde_json::to_vec_pretty(&raw).expect("serialize drift"),
    )
    .expect("write drifted plan");

    fixture
        .caps
        .execute_plan(
            fixture.investigation_id,
            approved.id,
            approval.id,
            "runner",
            "environment-drift",
            false,
        )
        .expect_err("environment drift must invalidate approval");
}

#[test]
fn invalid_actions_and_inputs_are_rejected_before_approval() {
    let (fixture, _probe) = Fixture::probe(ProbeMode::Success);
    let invalid_cases = vec![
        vec![ExecutionAction {
            action_name: "unsupported".into(),
            ..probe_action("unsupported")
        }],
        vec![probe_action("duplicate"), probe_action("duplicate")],
        vec![probe_action("duplicate-a"), probe_action("duplicate-b")],
        vec![ExecutionAction {
            inputs: json!({
                "resource_key": "probe/resource",
                "field": "label",
            }),
            ..probe_action("missing-input")
        }],
    ];

    for actions in invalid_cases {
        match fixture.create_plan("probe.mutate", actions) {
            Ok(plan) => {
                fixture
                    .caps
                    .validate_execution_plan(
                        fixture.investigation_id,
                        plan.id,
                        "planner",
                        "must reject invalid plan",
                    )
                    .expect_err("invalid plan reached ReadyForReview");
            }
            Err(error) => {
                assert!(
                    error.to_string().contains("action")
                        || error.to_string().contains("input")
                        || error.to_string().contains("duplicate"),
                    "unexpected validation error: {error}"
                );
            }
        }
    }

    assert_eq!(
        json_files(&fixture.execution_dir("execution_approvals")),
        0,
        "approval must never exist for an invalid plan"
    );
}

#[test]
fn missing_credentials_block_before_execution_adapter_mutation() {
    let (fixture, probe) = Fixture::probe(ProbeMode::MissingCredentials);
    let plan = fixture
        .create_plan("probe.mutate", vec![probe_action("a1")])
        .expect("create plan");
    let (approved, approval) = fixture.approve(plan);
    let attempt = fixture
        .caps
        .execute_plan(
            fixture.investigation_id,
            approved.id,
            approval.id,
            "runner",
            "missing-credentials",
            false,
        )
        .expect("missing credentials produce a durable blocked attempt");
    assert_eq!(attempt.status, ExecutionAttemptStatus::Blocked);
    assert!(attempt
        .errors
        .iter()
        .any(|error| error.contains("PROBE_TOKEN")));
    assert_eq!(probe.execute_count(), 0);
    let head = fixture
        .caps
        .list_execution_plan_revisions(fixture.investigation_id, approved.lineage_id)
        .expect("list plan revisions")
        .plans
        .pop()
        .expect("plan head");
    assert_eq!(
        head.status,
        ExecutionPlanStatus::Approved,
        "credential preflight must block before the Plan becomes Executing"
    );
}

#[test]
fn duplicate_capability_registration_is_rejected() {
    let temp = tempfile::tempdir().expect("temporary store");
    let store = Arc::new(LocalStore::open(temp.path()).expect("open temporary store"));
    let runtime = Runtime::new(store);
    runtime
        .register_execution_capability(Arc::new(ProbeCapability::new(
            ProbeMode::Success,
            temp.path(),
        )))
        .expect("first registration");
    let error = runtime
        .register_execution_capability(Arc::new(ProbeCapability::new(
            ProbeMode::Success,
            temp.path(),
        )))
        .expect_err("duplicate capability id must be rejected");
    assert!(error.to_string().contains("already registered"));
}

#[test]
fn started_attempt_and_idempotency_reservation_are_durable_before_mutation() {
    let (fixture, probe) = Fixture::probe(ProbeMode::Success);
    let plan = fixture
        .create_plan("probe.mutate", vec![probe_action("a1")])
        .expect("create plan");
    let (approved, approval) = fixture.approve(plan);

    let attempt = fixture
        .caps
        .execute_plan(
            fixture.investigation_id,
            approved.id,
            approval.id,
            "runner",
            "started-before-mutation",
            false,
        )
        .expect("execute plan");
    assert_eq!(probe.execute_count(), 1);
    assert_eq!(attempt.status, ExecutionAttemptStatus::Completed);
}

#[test]
fn crash_after_started_reservation_is_safely_suppressed_after_restart() {
    let (fixture, crashing_probe) = Fixture::probe(ProbeMode::PanicAfterReservation);
    let plan = fixture
        .create_plan("probe.mutate", vec![probe_action("a1")])
        .expect("create plan");
    let (approved, approval) = fixture.approve(plan);

    let crashed = catch_unwind(AssertUnwindSafe(|| {
        fixture.caps.execute_plan(
            fixture.investigation_id,
            approved.id,
            approval.id,
            "runner",
            "crash-reservation",
            false,
        )
    }));
    assert!(
        crashed.is_err(),
        "test adapter must simulate a process crash"
    );
    assert_eq!(crashing_probe.execute_count(), 1);
    let before_restart = fixture
        .caps
        .list_execution_attempts(fixture.investigation_id)
        .expect("Started reservation remains readable");
    assert!(before_restart
        .attempts
        .iter()
        .any(|attempt| attempt.status == ExecutionAttemptStatus::Started));

    let restarted_store = Arc::new(LocalStore::open(&fixture.root).expect("reopen store"));
    let restarted_runtime = Arc::new(Runtime::new(restarted_store));
    let restarted_probe = ProbeCapability::new(ProbeMode::Success, &fixture.root);
    restarted_runtime
        .register_execution_capability(Arc::new(restarted_probe.clone()))
        .expect("register restarted capability");
    let restarted = CapabilityService::new(restarted_runtime);
    let duplicate = restarted
        .execute_plan(
            fixture.investigation_id,
            approved.id,
            approval.id,
            "runner",
            "crash-reservation",
            false,
        )
        .expect("reservation suppresses mutation after restart");
    assert_eq!(
        duplicate.status,
        ExecutionAttemptStatus::DuplicateSuppressed
    );
    assert_eq!(restarted_probe.execute_count(), 0);
    let recovered = restarted
        .list_execution_attempts(fixture.investigation_id)
        .expect("list recovered attempt");
    assert!(recovered.attempts.iter().any(|attempt| {
        attempt.status == ExecutionAttemptStatus::PartiallyCompleted
            && attempt.uncertain_actions == vec!["a1"]
            && attempt.retry_safety == RetrySafety::Unsafe
    }));
}

#[test]
fn timeouts_are_uncertain_and_can_never_verify_as_passed() {
    let (fixture, _probe) = Fixture::probe(ProbeMode::Uncertain);
    let plan = fixture
        .create_plan("probe.mutate", vec![probe_action("a1")])
        .expect("create plan");
    let (approved, approval) = fixture.approve(plan);

    let attempt = fixture
        .caps
        .execute_plan(
            fixture.investigation_id,
            approved.id,
            approval.id,
            "runner",
            "uncertain-timeout",
            false,
        )
        .expect("uncertain outcome remains a durable attempt");
    assert_eq!(attempt.uncertain_actions, vec!["a1"]);
    assert!(attempt.failed_actions.is_empty());
    assert!(attempt.completed_actions.is_empty());
    assert_eq!(
        attempt.retry_safety,
        RetrySafety::Unsafe,
        "uncertain completion must refuse retry without a new revision and approval"
    );

    let receipts = fixture
        .caps
        .list_execution_receipts(fixture.investigation_id)
        .expect("list receipts");
    assert_eq!(receipts.receipts.len(), 1);
    assert_eq!(
        receipts.receipts[0].result_status,
        ExecutionReceiptResult::Uncertain
    );

    let verification = fixture
        .caps
        .verify_execution_attempt(fixture.investigation_id, attempt.id, "verifier")
        .expect("record an inconclusive or failed verification");
    assert_ne!(
        verification.status,
        ExecutionVerificationStatus::Passed,
        "failed or uncertain execution must never verify as passed"
    );
}

#[test]
fn failed_attempt_can_never_verify_as_passed() {
    let (fixture, _probe) = Fixture::probe(ProbeMode::Failed);
    let plan = fixture
        .create_plan("probe.mutate", vec![probe_action("a1")])
        .expect("create plan");
    let (approved, approval) = fixture.approve(plan);
    let attempt = fixture
        .caps
        .execute_plan(
            fixture.investigation_id,
            approved.id,
            approval.id,
            "runner",
            "definite-failure",
            false,
        )
        .expect("failed outcome remains durable");
    assert_eq!(attempt.status, ExecutionAttemptStatus::Failed);
    assert_eq!(attempt.failed_actions, vec!["a1"]);

    let verification = fixture
        .caps
        .verify_execution_attempt(fixture.investigation_id, attempt.id, "verifier")
        .expect("record failed execution verification");
    assert_ne!(
        verification.status,
        ExecutionVerificationStatus::Passed,
        "a definite failed attempt must never verify as passed"
    );
}

#[test]
fn duplicate_suppression_is_durable_across_runtime_restart() {
    let (fixture, first_probe) = Fixture::probe(ProbeMode::Success);
    let plan = fixture
        .create_plan("probe.mutate", vec![probe_action("a1")])
        .expect("create plan");
    let (approved, approval) = fixture.approve(plan);
    let first = fixture
        .caps
        .execute_plan(
            fixture.investigation_id,
            approved.id,
            approval.id,
            "runner",
            "durable-idempotency",
            false,
        )
        .expect("first execution");
    assert_eq!(first_probe.execute_count(), 1);

    let restarted_store = Arc::new(LocalStore::open(&fixture.root).expect("reopen store"));
    let restarted_runtime = Arc::new(Runtime::new(restarted_store));
    let restarted_probe = ProbeCapability::new(ProbeMode::Success, &fixture.root);
    restarted_runtime
        .register_execution_capability(Arc::new(restarted_probe.clone()))
        .expect("register restarted capability");
    let restarted = CapabilityService::new(restarted_runtime);
    let duplicate = restarted
        .execute_plan(
            fixture.investigation_id,
            approved.id,
            approval.id,
            "runner",
            "durable-idempotency",
            false,
        )
        .expect("durably suppress duplicate");

    assert_ne!(
        duplicate.id, first.id,
        "suppression must be represented by its own audit record"
    );
    assert_eq!(
        duplicate.status,
        ExecutionAttemptStatus::DuplicateSuppressed
    );
    assert_eq!(restarted_probe.execute_count(), 0);
    let attempts = restarted
        .list_execution_attempts(fixture.investigation_id)
        .expect("list attempts");
    let lineages: HashSet<_> = attempts
        .attempts
        .iter()
        .map(rivora::ExecutionAttempt::lineage_id)
        .collect();
    assert_eq!(
        lineages.len(),
        2,
        "one original lineage and one suppression record must be durable"
    );
    assert_eq!(
        attempts
            .attempts
            .iter()
            .filter(|candidate| { candidate.status == ExecutionAttemptStatus::DuplicateSuppressed })
            .count(),
        1
    );
}

#[test]
fn rollback_plan_contains_explicit_inverses_in_reverse_completion_order() {
    let mock = Arc::new(MockExecutionCapability::new());
    let fixture = Fixture::with_capability(Arc::clone(&mock) as Arc<dyn ExecutionCapability>);
    let mut before = HashMap::new();
    before.insert("label".into(), "old".into());
    before.insert("state".into(), "open".into());
    mock.seed_resource("issue/1", before);
    let plan = fixture
        .create_plan(
            "mock.record",
            vec![
                mock_action("a1", "label", "bug"),
                mock_action("a2", "state", "closed"),
            ],
        )
        .expect("create plan");
    let (approved, approval) = fixture.approve(plan);
    let attempt = fixture
        .caps
        .execute_plan(
            fixture.investigation_id,
            approved.id,
            approval.id,
            "runner",
            "two-action-rollback",
            false,
        )
        .expect("execute plan");
    assert_eq!(attempt.completed_actions, vec!["a1", "a2"]);

    let rollback = fixture
        .caps
        .create_rollback_plan(fixture.investigation_id, attempt.id, "rollback-planner")
        .expect("derive explicit rollback draft");
    assert_eq!(rollback.status, ExecutionPlanStatus::Draft);
    assert_eq!(rollback.actions.len(), 2);
    let inverse_fields: Vec<_> = rollback
        .actions
        .iter()
        .map(|action| {
            (
                action.inputs["field"].as_str().unwrap(),
                action.inputs["value"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(inverse_fields, vec![("state", "open"), ("label", "old")]);
    let rollback_ready = fixture
        .caps
        .validate_execution_plan(
            fixture.investigation_id,
            rollback.id,
            "rollback-reviewer",
            "explicit inverses and target reviewed",
        )
        .expect("rollback draft must pass normal validation");
    assert_eq!(rollback_ready.status, ExecutionPlanStatus::ReadyForReview);
    assert!(
        fixture
            .caps
            .list_execution_attempts(fixture.investigation_id)
            .expect("list attempts")
            .attempts
            .iter()
            .all(|candidate| candidate.plan_id != rollback.id),
        "rollback generation must not execute automatically"
    );
}

#[test]
fn trace_has_structured_learning_ids_and_duplicate_linking_is_prevented() {
    let mock = Arc::new(MockExecutionCapability::new());
    let fixture = Fixture::with_capability(Arc::clone(&mock) as Arc<dyn ExecutionCapability>);
    let plan = fixture
        .create_plan("mock.record", vec![mock_action("a1", "label", "bug")])
        .expect("create plan");
    let (approved, approval) = fixture.approve(plan);
    let attempt = fixture
        .caps
        .execute_plan(
            fixture.investigation_id,
            approved.id,
            approval.id,
            "runner",
            "trace-linkage",
            false,
        )
        .expect("execute plan");
    let verification = fixture
        .caps
        .verify_execution_attempt(fixture.investigation_id, attempt.id, "verifier")
        .expect("verify");
    assert_eq!(verification.status, ExecutionVerificationStatus::Passed);

    let first = fixture
        .caps
        .link_execution_to_implementation(
            fixture.investigation_id,
            attempt.id,
            "runner",
            "bounded execution",
        )
        .expect("link implementation");
    let duplicate_error = fixture
        .caps
        .link_execution_to_implementation(
            fixture.investigation_id,
            attempt.id,
            "runner",
            "bounded execution",
        )
        .expect_err("duplicate implementation linkage must be rejected");
    assert!(
        duplicate_error.to_string().contains("already linked"),
        "unexpected duplicate-link error: {duplicate_error}"
    );
    assert_eq!(
        fixture
            .caps
            .list_implementation_records(fixture.investigation_id)
            .expect("list implementations")
            .records
            .len(),
        1
    );

    let outcome = fixture
        .caps
        .create_measured_learning_outcome(
            fixture.investigation_id,
            fixture.proposal_id,
            first.id,
            "learner",
        )
        .expect("create measured outcome");
    let trace = fixture
        .caps
        .trace_execution(fixture.investigation_id, approved.id)
        .expect("trace execution");
    assert_eq!(trace.implementation_record_id, Some(first.id));
    assert_eq!(trace.measured_outcome_id, Some(outcome.id));
}

#[test]
fn revised_plan_preserves_visible_supersession() {
    let (fixture, _probe) = Fixture::probe(ProbeMode::Success);
    let plan = fixture
        .create_plan("probe.mutate", vec![probe_action("a1")])
        .expect("create plan");
    let (approved, _approval) = fixture.approve(plan);
    let revised = fixture
        .caps
        .revise_execution_plan(
            fixture.investigation_id,
            approved.id,
            ReviseExecutionPlanRequest {
                inputs: Some(json!({
                    "provider": "github",
                    "owner": "rivora-dev",
                    "repository": "rivora",
                    "reason": "revision",
                })),
                ..Default::default()
            },
            "planner",
            "revise plan",
        )
        .expect("revise approved plan");
    let revisions = fixture
        .caps
        .list_execution_plan_revisions(fixture.investigation_id, approved.lineage_id)
        .expect("list revisions");

    assert_eq!(revised.status, ExecutionPlanStatus::Draft);
    assert!(
        revisions.plans.iter().any(|candidate| {
            candidate.status == ExecutionPlanStatus::Superseded
                && candidate.superseding_plan_id == Some(revised.id)
        }),
        "the superseded revision and its successor must remain visible"
    );
}

#[test]
fn repeated_verification_creates_monotonic_revisions() {
    let mock = Arc::new(MockExecutionCapability::new());
    let fixture = Fixture::with_capability(Arc::clone(&mock) as Arc<dyn ExecutionCapability>);
    let plan = fixture
        .create_plan("mock.record", vec![mock_action("a1", "label", "bug")])
        .expect("create plan");
    let (approved, approval) = fixture.approve(plan);
    let attempt = fixture
        .caps
        .execute_plan(
            fixture.investigation_id,
            approved.id,
            approval.id,
            "runner",
            "verification-revisions",
            false,
        )
        .expect("execute");
    let first = fixture
        .caps
        .verify_execution_attempt(fixture.investigation_id, attempt.id, "verifier")
        .expect("first verification");
    let second = fixture
        .caps
        .verify_execution_attempt(fixture.investigation_id, attempt.id, "verifier")
        .expect("second verification");
    assert_eq!(first.revision, 1);
    assert_eq!(second.revision, 2);
    assert_ne!(first.id, second.id);
}

#[test]
fn corrupt_approval_and_verification_do_not_hide_valid_records() {
    let mock = Arc::new(MockExecutionCapability::new());
    let fixture = Fixture::with_capability(Arc::clone(&mock) as Arc<dyn ExecutionCapability>);
    let plan = fixture
        .create_plan("mock.record", vec![mock_action("a1", "label", "bug")])
        .expect("create plan");
    let (approved, approval) = fixture.approve(plan);
    let attempt = fixture
        .caps
        .execute_plan(
            fixture.investigation_id,
            approved.id,
            approval.id,
            "runner",
            "corruption-isolation",
            false,
        )
        .expect("execute");
    let verification = fixture
        .caps
        .verify_execution_attempt(fixture.investigation_id, attempt.id, "verifier")
        .expect("verify");

    fs::write(
        fixture
            .execution_dir("execution_approvals")
            .join("corrupt.json"),
        "{not json",
    )
    .expect("write corrupt approval");
    fs::write(
        fixture
            .execution_dir("execution_verifications")
            .join("corrupt.json"),
        "{not json",
    )
    .expect("write corrupt verification");

    let approvals = fixture
        .store
        .list_execution_approvals(&fixture.investigation_id)
        .expect("approval corruption is isolated");
    assert!(
        approvals
            .approvals
            .iter()
            .any(|candidate| candidate.id == approval.id),
        "valid approval must remain visible"
    );
    assert_eq!(
        approvals.diagnostics.len(),
        1,
        "corrupt approval must produce a visible diagnostic"
    );
    let verifications = fixture
        .store
        .list_execution_verifications(&fixture.investigation_id)
        .expect("verification corruption is isolated");
    assert!(
        verifications
            .verifications
            .iter()
            .any(|candidate| candidate.id == verification.id),
        "valid verification must remain visible"
    );
    assert_eq!(
        verifications.diagnostics.len(),
        1,
        "corrupt verification must produce a visible diagnostic"
    );

    let trace = fixture
        .caps
        .trace_execution(fixture.investigation_id, approved.id)
        .expect("trace valid records despite corrupt siblings");
    assert!(trace.approval_ids.contains(&approval.id));
    assert!(trace.verification_ids.contains(&verification.id));
}
