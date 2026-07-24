//! v0.10 Unified Workspace contracts: routing, intents, registry, terminal policy.

use std::process::{Command, Stdio};
use std::sync::Arc;

use rivora::storage::LocalStore;
use rivora::{CapabilityService, MockExecutionCapability, Runtime};
use rivora_connectors::register_first_party_github_execution_capabilities;
use rivora_workspace::actions::{action_registry, filter_actions, ActionContext};
use rivora_workspace::intent::{
    execute_intent, interpret_prompt, WorkspaceActionId, WorkspaceIntent,
};
use tempfile::tempdir;

fn caps() -> CapabilityService {
    let dir = tempdir().unwrap();
    let store = LocalStore::open(dir.path()).unwrap();
    let runtime = Arc::new(Runtime::new(Arc::new(store)));
    runtime
        .register_execution_capability(Arc::new(MockExecutionCapability::new()))
        .unwrap();
    register_first_party_github_execution_capabilities(runtime.execution_registry()).unwrap();
    std::mem::forget(dir);
    CapabilityService::new(runtime)
}

#[test]
fn version_is_0_10_0() {
    let bin = env!("CARGO_BIN_EXE_rivora-workspace");
    let out = Command::new(bin).arg("--version").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("0.10.0"), "expected 0.10.0, got: {stdout}");
}

#[test]
fn non_tty_refuses_unified_workspace() {
    let dir = tempdir().unwrap();
    let bin = env!("CARGO_BIN_EXE_rivora-workspace");
    let out = Command::new(bin)
        .args(["--data-dir", dir.path().to_str().unwrap()])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("interactive Workspace requires a terminal"),
        "{err}"
    );
}

#[test]
fn slash_and_ctrlp_share_registry_identity() {
    let a = action_registry();
    let b = action_registry();
    assert!(std::ptr::eq(a.as_ptr(), b.as_ptr()));
    assert!(a
        .iter()
        .any(|d| d.id == WorkspaceActionId::CreateInvestigation));
    assert!(a.iter().any(|d| d.id == WorkspaceActionId::Evaluate));
    assert!(a.iter().any(|d| d.id == WorkspaceActionId::Quit));
}

#[test]
fn apply_fix_language_never_creates_execution_plan_intent() {
    let id = rivora::domain::InvestigationId::new();
    let r = interpret_prompt("Run this fix in production now", Some(id));
    assert!(!matches!(
        r.intent,
        WorkspaceIntent::CreateExecutionPlan { .. }
    ));
}

#[test]
fn create_investigation_intent_uses_capabilities() {
    let caps = caps();
    let draft = rivora_workspace::intent::InvestigationDraft {
        title: "v0.10 flow".into(),
        description: Some("unified workspace".into()),
        suggested_sources: vec![],
    };
    let result = execute_intent(&caps, &WorkspaceIntent::CreateInvestigation { draft });
    match result {
        rivora_workspace::intent::IntentExecutionResult::InvestigationCreated { title, .. } => {
            assert_eq!(title, "v0.10 flow");
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn evaluate_disabled_without_context_in_registry() {
    let ctx = ActionContext {
        active_investigation: None,
        has_selected_proposal: false,
        has_selected_plan: false,
        filter: "evaluate",
    };
    let results = filter_actions("evaluate", ctx);
    let eval = results
        .iter()
        .find(|(d, _)| d.id == WorkspaceActionId::Evaluate)
        .expect("evaluate present");
    assert!(!eval.1.is_available());
}

#[test]
fn workspace_crate_does_not_depend_on_dialoguer() {
    let toml = include_str!("../Cargo.toml");
    assert!(
        !toml.contains("dialoguer"),
        "Unified Workspace must not depend on dialoguer"
    );
}
