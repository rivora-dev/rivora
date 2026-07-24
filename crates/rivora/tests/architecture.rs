//! Architecture boundary and invariant tests.

use std::path::PathBuf;
use std::sync::Arc;

use rivora::storage::LocalStore;
use rivora::{CapabilityService, Runtime};

/// CapabilityService and Runtime share the same store-backed Runtime instance.
#[test]
fn capabilities_coordinate_shared_runtime() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(LocalStore::open(dir.path()).unwrap());
    let runtime = Arc::new(Runtime::new(store));
    let caps = CapabilityService::new(Arc::clone(&runtime));

    let inv = caps.create_investigation("arch", None, "tester").unwrap();
    let opened = runtime.open_investigation(inv.id).unwrap();
    assert_eq!(opened.id, inv.id);

    // Same Runtime pointer for interface sharing proofs.
    assert!(Arc::ptr_eq(caps.runtime(), &runtime));
}

/// Source-level architecture checks for dependency direction.
#[test]
fn connectors_crate_does_not_import_reasoning_modules() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");
    let connectors_src = workspace_root.join("crates/rivora-connectors/src");
    if !connectors_src.exists() {
        // Connectors may not exist yet during early Phase 1; skip softly.
        return;
    }

    let mut stack = vec![connectors_src];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                let content = std::fs::read_to_string(&path).unwrap();
                for forbidden in [
                    "runtime::evaluation",
                    "runtime::verification",
                    "runtime::recommendation",
                    "runtime::learning",
                    "evaluate_investigation",
                    "generate_recommendation",
                    "verify_conclusion",
                    "record_outcome",
                    "generate_improvement_proposals",
                    "compare_improvement_proposals",
                    "update_improvement_proposal_status",
                    "apply_improvement_proposal",
                ] {
                    assert!(
                        !content.contains(forbidden),
                        "{} must not reference reasoning API `{forbidden}`",
                        path.display()
                    );
                }
            }
        }
    }
}

#[test]
fn cli_and_workspace_remain_thin() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");

    for crate_name in ["rivora-cli", "rivora-workspace"] {
        let src = workspace_root.join("crates").join(crate_name).join("src");
        if !src.exists() {
            continue;
        }
        let mut stack = vec![src];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                    let content = std::fs::read_to_string(&path).unwrap();
                    // Interfaces should call CapabilityService, not implement Memory rules.
                    for forbidden in [
                        "fn derive_knowledge",
                        "fn evaluate_investigation",
                        "fn generate_improvement_proposals",
                        "fn compare_improvement_proposals",
                        "fn update_improvement_proposal_status",
                        "append_proposal(",
                        ".runtime()",
                        ".store()",
                    ] {
                        assert!(
                            !content.contains(forbidden),
                            "{} must not own or bypass Capability behavior `{forbidden}`",
                            path.display()
                        );
                    }
                }
            }
        }
    }
}

/// Proposal generation and export may persist Rivora documents, but cannot apply changes.
#[test]
fn proposal_runtime_has_no_implementation_or_external_mutation_primitive() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(manifest_dir.join("src/runtime/proposal.rs")).unwrap();
    for forbidden in [
        "std::process::Command",
        "std::fs::write",
        "File::create",
        "OpenOptions",
        "fn apply_improvement_proposal",
        "fn invoke_coding_agent",
        "fn create_branch",
        "fn create_pull_request",
        "fn deploy",
    ] {
        assert!(
            !source.contains(forbidden),
            "Proposal Runtime must not contain mutation primitive `{forbidden}`"
        );
    }
}

/// Outcome Runtime records and evaluates external work but never applies or mutates systems.
#[test]
fn outcome_runtime_has_no_apply_or_coding_agent_invocation() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(manifest_dir.join("src/runtime/outcome.rs")).unwrap();
    for forbidden in [
        "std::process::Command",
        "std::fs::write",
        "File::create",
        "OpenOptions",
        "fn apply_improvement_proposal",
        "fn apply_implementation",
        "fn invoke_coding_agent",
        "fn create_branch",
        "fn create_pull_request",
        "fn deploy",
        "fn mutate_external",
        "fn execute_patch",
    ] {
        assert!(
            !source.contains(forbidden),
            "Outcome Runtime must not contain mutation primitive `{forbidden}`"
        );
    }
    // Measured outcomes must remain distinct from proposal acceptance.
    assert!(
        !source.contains("ProposalStatus::Accepted")
            || source.contains("Accepted Proposal")
            || source.contains("accepted does not"),
        "Outcome Runtime should not treat proposal acceptance as measured success without explicit boundary language"
    );
    assert!(
        source.contains("Accepted Proposal")
            || source.contains("never applies")
            || source.contains("does not prove"),
        "Outcome Runtime must document that acceptance ≠ measured outcome"
    );
}

/// v0.1 Recommendation LearningOutcome remains distinct from MeasuredLearningOutcome.
#[test]
fn measured_outcome_is_not_recommendation_disposition_learning_outcome() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let learning = std::fs::read_to_string(manifest_dir.join("src/domain/learning.rs")).unwrap();
    let outcome = std::fs::read_to_string(manifest_dir.join("src/domain/outcome.rs")).unwrap();
    assert!(
        learning.contains("struct LearningOutcome"),
        "v0.1 LearningOutcome must remain"
    );
    assert!(
        outcome.contains("struct MeasuredLearningOutcome"),
        "v0.5 MeasuredLearningOutcome must exist as a distinct type"
    );
    assert!(
        !outcome.contains("enum OutcomeDisposition"),
        "Measured outcomes must not reuse Recommendation disposition enum as their classification"
    );
}

/// Runtime never guesses rollback inverses from supported_actions order.
#[test]
fn rollback_derivation_never_uses_supported_actions_first() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let runtime = std::fs::read_to_string(manifest_dir.join("src/runtime/execution.rs")).unwrap();
    let create_fn = runtime
        .find("pub fn create_rollback_plan")
        .expect("create_rollback_plan must exist");
    let next_fn = runtime[create_fn + 1..]
        .find("\n    pub fn ")
        .map(|offset| create_fn + 1 + offset)
        .unwrap_or(runtime.len());
    let body = &runtime[create_fn..next_fn];
    for forbidden in [
        "supported_actions.first()",
        "supported_actions.first(",
        ".supported_actions\n                    .first(",
        "descriptor().supported_actions.first",
    ] {
        assert!(
            !body.contains(forbidden),
            "create_rollback_plan must never guess inverses via `{forbidden}`"
        );
    }
    assert!(
        body.contains("inverse_action_name"),
        "create_rollback_plan must require explicit inverse_action_name from receipts"
    );
    assert!(
        body.contains("Draft") || body.contains("status"),
        "rollback plans must remain draft-only"
    );
}

/// v0.6 execution requires explicit approval; proposal acceptance must not execute.
#[test]
fn execution_requires_approval_and_not_proposal_acceptance() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let execution = std::fs::read_to_string(manifest_dir.join("src/runtime/execution.rs")).unwrap();
    assert!(
        execution.contains("approve_execution_plan") && execution.contains("execute_plan"),
        "execution runtime must expose approve and execute as separate operations"
    );
    assert!(
        execution.contains("Accepted") && execution.contains("execution plans require an accepted"),
        "plans require accepted proposals but acceptance alone must not execute"
    );
    assert!(
        !execution.contains("std::process::Command"),
        "execution runtime must not spawn arbitrary shell commands"
    );
    // No autonomous loop primitives.
    for forbidden in [
        "loop {",
        "schedule_execution",
        "auto_remediate",
        "daemon",
        "hidden_retry",
    ] {
        assert!(
            !execution.contains(forbidden),
            "execution runtime must not contain autonomous primitive `{forbidden}`"
        );
    }
}

/// v0.6 authority is bound to an immutable target and Started is durable before mutation.
#[test]
fn execution_authority_and_durability_are_runtime_owned() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let domain = std::fs::read_to_string(manifest_dir.join("src/domain/execution.rs")).unwrap();
    let runtime = std::fs::read_to_string(manifest_dir.join("src/runtime/execution.rs")).unwrap();

    assert!(
        domain.contains("struct TargetSnapshot")
            && domain.contains("pub target_snapshot: Option<TargetSnapshot>"),
        "Plans, Approvals, and Attempts must preserve immutable target authority"
    );

    let persist_started = runtime
        .find("append_execution_attempt(&started)")
        .expect("Runtime must persist a Started Attempt");
    let invoke_adapter = runtime
        .find("cap.execute(&invocation)")
        .expect("Runtime must own bounded adapter invocation");
    assert!(
        persist_started < invoke_adapter,
        "Runtime must durably persist Started before invoking an external mutation"
    );
}

/// CLI and Workspace must not call GitHub mutation APIs directly.
#[test]
fn interfaces_do_not_call_github_mutation_http_directly() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");
    for crate_name in ["rivora-cli", "rivora-workspace"] {
        let src = workspace_root.join("crates").join(crate_name).join("src");
        let mut stack = vec![src];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                    let content = std::fs::read_to_string(&path).unwrap();
                    for forbidden in [
                        "api.github.com",
                        ".post(\"/repos",
                        "create_pull_request(",
                        "reqwest::blocking",
                    ] {
                        assert!(
                            !content.contains(forbidden),
                            "{} must not call external mutation APIs directly (`{forbidden}`)",
                            path.display()
                        );
                    }
                }
            }
        }
    }
}

/// Observation connectors remain free of write methods; execution is a separate module.
#[test]
fn observation_connectors_remain_read_only_separate_from_execution() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");
    for file in [
        "github.rs",
        "github_actions.rs",
        "local.rs",
        "kubernetes.rs",
        "sentry.rs",
    ] {
        let path = workspace_root
            .join("crates/rivora-connectors/src")
            .join(file);
        let content = std::fs::read_to_string(&path).unwrap();
        for forbidden in [".post(", ".patch(", ".put(", ".delete("] {
            assert!(
                !content.contains(forbidden),
                "{} observation connector must remain read-only (found `{forbidden}`)",
                path.display()
            );
        }
    }
    let execution =
        std::fs::read_to_string(workspace_root.join("crates/rivora-connectors/src/execution.rs"))
            .unwrap();
    assert!(
        execution.contains("ExecutionCapability"),
        "execution adapters must implement ExecutionCapability"
    );
    assert!(
        execution.contains("github.issue.comment")
            || execution.contains("GitHubIssueCommentCapability"),
        "bounded GitHub write capabilities must be declared"
    );
}

/// High-risk and prohibited capabilities are denied by centralized policy.
/// Capabilities must not write Engineering Loop artifacts outside Runtime orchestration.
#[test]
fn execution_capabilities_do_not_write_loop_artifacts_directly() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");
    let connectors_src = workspace_root.join("crates/rivora-connectors/src");
    let mut stack = vec![connectors_src];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                let content = std::fs::read_to_string(&path).unwrap();
                for forbidden in [
                    "append_memory(",
                    "append_evaluation(",
                    "append_verification(",
                    "append_learning(",
                    "generate_improvement_proposals(",
                    "append_measured_learning_outcome(",
                    "process_lifecycle_contributions(",
                ] {
                    assert!(
                        !content.contains(forbidden),
                        "{} must not call Runtime loop write API `{forbidden}`",
                        path.display()
                    );
                }
            }
        }
    }
}

/// Every ExecutionCapabilityDescriptor construction in connectors declares loop fields.
#[test]
fn registered_capabilities_expose_engineering_loop_participation() {
    use rivora::domain::MockExecutionCapability;
    use rivora::ExecutionCapability;
    let mock = MockExecutionCapability::new();
    let desc = mock.descriptor();
    assert_eq!(
        desc.engineering_loop.memory,
        rivora::LifecycleParticipation::Supported
    );
    assert_eq!(
        desc.engineering_loop.learning,
        rivora::LifecycleParticipation::Deferred
    );
    assert!(!desc.accepted_input_types.is_empty());
    assert!(desc.provider_independent);
    assert!(desc.is_complete(), "mock.record must be v0.8-complete");
    assert!(!desc.name.is_empty());
    assert!(!desc.provider.is_empty());
    assert!(!desc.operation.is_empty());
}

/// v0.8: first-party catalog, complete descriptors, and no connector health reasoning.
#[test]
fn v0_8_capability_coverage_architecture_gates() {
    use rivora::domain::{
        build_capability_coverage_report, first_party_connector_coverage, MockExecutionCapability,
        FIRST_PARTY_CONNECTOR_IDS, FIRST_PARTY_EXECUTION_CAPABILITY_IDS,
    };
    use rivora::ExecutionCapability;
    use rivora_connectors::{
        register_github_execution_capabilities, DEFAULT_GITHUB_EXECUTION_REPO,
    };
    use std::sync::Arc;

    assert_eq!(FIRST_PARTY_EXECUTION_CAPABILITY_IDS.len(), 6);
    assert_eq!(FIRST_PARTY_CONNECTOR_IDS.len(), 5);
    assert_eq!(first_party_connector_coverage().len(), 5);

    let registry = rivora::ExecutionCapabilityRegistry::new();
    registry
        .register(Arc::new(MockExecutionCapability::new()) as Arc<dyn ExecutionCapability>)
        .unwrap();
    register_github_execution_capabilities(&registry, DEFAULT_GITHUB_EXECUTION_REPO, None).unwrap();
    let descriptors = registry.list();
    assert_eq!(descriptors.len(), 6);
    for desc in &descriptors {
        assert!(
            desc.is_complete(),
            "{} incomplete: {:?}",
            desc.capability_id,
            desc.completeness_gaps()
        );
        assert!(desc.lifecycle_fully_declared());
        assert!(!desc.accepted_input_types.is_empty());
        assert_eq!(
            desc.engineering_loop.memory,
            rivora::LifecycleParticipation::Supported
        );
        assert_eq!(
            desc.engineering_loop.evaluation,
            rivora::LifecycleParticipation::Supported
        );
        assert_eq!(
            desc.engineering_loop.verification,
            rivora::LifecycleParticipation::Supported
        );
        assert_eq!(
            desc.engineering_loop.improvement,
            rivora::LifecycleParticipation::Deferred
        );
        assert_eq!(
            desc.engineering_loop.learning,
            rivora::LifecycleParticipation::Deferred
        );
    }
    let report = build_capability_coverage_report(&descriptors);
    assert!(report.all_first_party_registered);
    assert!(report.all_descriptors_complete);
    assert!(report.all_lifecycle_declared);
    assert!(report.gaps.is_empty(), "gaps: {:?}", report.gaps);

    // Observation connectors must not invent health classifications.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");
    let k8s =
        std::fs::read_to_string(workspace_root.join("crates/rivora-connectors/src/kubernetes.rs"))
            .unwrap();
    assert!(
        !k8s.contains("health=healthy") && !k8s.contains("\"unhealthy\""),
        "kubernetes connector must not classify health"
    );
    let local =
        std::fs::read_to_string(workspace_root.join("crates/rivora-connectors/src/local.rs"))
            .unwrap();
    assert!(
        !local.contains("indicates_failure"),
        "local connector must not invent failure conclusions"
    );
}

#[test]
fn v0_9_production_hardening_architecture_gates() {
    use rivora::domain::{
        PerformanceBudget, ReplayContract, STORE_SCHEMA_VERSION, STORE_SCHEMA_VERSION_MAX,
    };
    use rivora::{LocalStore, RivoraError};

    // Store manifest + schema gate.
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::open(dir.path()).unwrap();
    assert!(dir.path().join("store.json").exists());
    assert!(store.lock_held());
    let health = store.health_report().unwrap();
    assert_eq!(health.schema_version, STORE_SCHEMA_VERSION);
    assert!(health.schema_version <= STORE_SCHEMA_VERSION_MAX);

    // Replay contracts must never allow dry-run to suppress live or bypass authority.
    for c in ReplayContract::v0_9_contracts() {
        assert!(!c.dry_run_suppresses_live, "{}", c.operation);
        assert!(!c.retry_bypasses_authority, "{}", c.operation);
    }

    // Performance budgets must cover required production scenarios.
    let names: Vec<_> = PerformanceBudget::v0_9_budgets()
        .into_iter()
        .map(|b| b.scenario)
        .collect();
    for required in [
        "cli_startup",
        "store_open",
        "ingestion",
        "duplicate_ingestion",
        "lifecycle_run",
        "search",
        "diagnostic_export",
    ] {
        assert!(
            names.iter().any(|n| n == required),
            "missing budget {required}"
        );
    }

    // Structured errors expose stable exit codes and failure classes.
    assert_eq!(
        RivoraError::store_locked("x").exit_code(),
        rivora::CliExitCode::LockConflict
    );
    assert_eq!(
        RivoraError::partial("x").exit_code(),
        rivora::CliExitCode::Partial
    );
    assert!(!RivoraError::validation("x").is_retryable());
    assert!(RivoraError::timeout("x").is_retryable());

    // Connector resilience module must exist and forbid reasoning imports (covered elsewhere).
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");
    let resilience = workspace_root.join("crates/rivora-connectors/src/resilience.rs");
    assert!(
        resilience.exists(),
        "v0.9 requires connector resilience helpers"
    );
    let content = std::fs::read_to_string(&resilience).unwrap();
    assert!(content.contains("redact_json"));
    assert!(content.contains("http_client"));
    assert!(content.contains("max_response_bytes"));
}

#[test]
fn policy_denies_high_risk_and_prohibited() {
    use rivora::domain::{
        default_accepted_input_types, evaluate_execution_policy, CapabilityRiskLevel,
        EngineeringLoopParticipation, ExecutionCapabilityDescriptor, ExecutionPolicyDecisionKind,
    };
    let prohibited = ExecutionCapabilityDescriptor {
        capability_id: "force_push".into(),
        name: "Force Push".into(),
        version: "1".into(),
        provider: "test".into(),
        operation: "force_push".into(),
        risk_level: CapabilityRiskLevel::Prohibited,
        mutating: true,
        supported_actions: vec!["force_push".into()],
        required_inputs: vec![],
        permissions: vec![],
        supports_dry_run: false,
        idempotency_behavior: "none".into(),
        reversibility: "none".into(),
        verification_method: "none".into(),
        credential_requirements: vec![],
        target_restrictions: vec![],
        failure_semantics: "denied".into(),
        description: "prohibited".into(),
        output_types: vec![],
        limitations: vec!["policy denied".into()],
        engineering_loop: EngineeringLoopParticipation::execution_capability_default(),
        accepted_input_types: default_accepted_input_types("force_push"),
        provider_independent: true,
    };
    let d = evaluate_execution_policy(Some(&prohibited), "force_push", "production", 1, false);
    assert_eq!(d.decision, ExecutionPolicyDecisionKind::Denied);
    let high = ExecutionCapabilityDescriptor {
        risk_level: CapabilityRiskLevel::HighRiskWrite,
        capability_id: "merge".into(),
        ..prohibited
    };
    let d2 = evaluate_execution_policy(Some(&high), "merge", "production", 1, false);
    assert_eq!(d2.decision, ExecutionPolicyDecisionKind::Denied);
}
