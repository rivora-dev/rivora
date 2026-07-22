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
