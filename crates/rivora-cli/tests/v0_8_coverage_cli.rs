//! CLI smoke tests for v0.8 Capability Coverage commands.

use std::process::Command;

use tempfile::tempdir;

fn rivora_bin() -> String {
    env!("CARGO_BIN_EXE_rivora").to_string()
}

fn run_ok(bin: &str, args: &[&str]) -> std::process::Output {
    let out = Command::new(bin).args(args).output().unwrap();
    assert!(
        out.status.success(),
        "rivora {} failed:\nstdout={}\nstderr={}",
        args.join(" "),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn run_json(bin: &str, data: &std::path::Path, args: &[&str]) -> serde_json::Value {
    let mut full = vec!["--data-dir", data.to_str().unwrap(), "--json"];
    full.extend_from_slice(args);
    let out = run_ok(bin, &full);
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn capability_coverage_reports_all_first_party() {
    let dir = tempdir().unwrap();
    let data = dir.path().join("data");
    let bin = rivora_bin();

    let report = run_json(&bin, &data, &["capability", "coverage"]);
    assert_eq!(report["first_party_expected"], 6);
    assert_eq!(report["first_party_registered"], 6);
    assert_eq!(report["all_first_party_registered"], true);
    assert_eq!(report["all_descriptors_complete"], true);
    assert_eq!(report["all_lifecycle_declared"], true);
    let gaps = report["gaps"].as_array().unwrap();
    assert!(gaps.is_empty(), "gaps={gaps:?}");
    let capabilities = report["capabilities"].as_array().unwrap();
    assert_eq!(capabilities.len(), 6);
    let connectors = report["connectors"].as_array().unwrap();
    assert_eq!(connectors.len(), 5);

    let text = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "capability",
            "coverage",
        ],
    );
    let stdout = String::from_utf8_lossy(&text.stdout);
    assert!(stdout.contains("mock.record"), "stdout={stdout}");
    assert!(stdout.contains("github.issue.comment"), "stdout={stdout}");
    assert!(
        stdout.contains("github_actions.workflow_dispatch"),
        "stdout={stdout}"
    );
}

#[test]
fn capability_list_includes_github_adapters_without_env() {
    let dir = tempdir().unwrap();
    let data = dir.path().join("data");
    let bin = rivora_bin();
    let list = run_json(&bin, &data, &["capability", "list"]);
    let ids: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["capability_id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"mock.record"));
    assert!(ids.contains(&"github.issue.comment"));
    assert!(ids.contains(&"github.pull_request.create_draft"));
    assert!(ids.contains(&"github_actions.workflow_dispatch"));

    let show = run_json(&bin, &data, &["capability", "show", "github.issue.label"]);
    assert_eq!(show["name"], "GitHub Issue Label");
    assert_eq!(show["provider"], "github");
    assert_eq!(show["operation"], "label");
    assert_eq!(show["mutating"], true);
    assert!(!show["permissions"].as_array().unwrap().is_empty());
    assert!(!show["limitations"].as_array().unwrap().is_empty());
    assert_eq!(show["engineering_loop"]["memory"], "supported");
    assert_eq!(show["engineering_loop"]["learning"], "deferred");
}

#[test]
fn capability_show_unknown_id_fails_actionably() {
    let dir = tempdir().unwrap();
    let data = dir.path().join("data");
    let bin = rivora_bin();
    let out = Command::new(&bin)
        .args([
            "--data-dir",
            data.to_str().unwrap(),
            "capability",
            "show",
            "does.not.exist",
        ])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        err.contains("unknown") || err.contains("does.not.exist"),
        "err={err}"
    );
}
