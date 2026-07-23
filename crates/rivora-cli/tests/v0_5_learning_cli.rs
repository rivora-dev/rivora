//! v0.5 Phase 3 Measured Learning Outcome CLI flows through CapabilityService.

use std::process::{Command, Output};

use tempfile::tempdir;

fn rivora_bin() -> String {
    env!("CARGO_BIN_EXE_rivora").to_string()
}

fn run(bin: &str, args: &[&str]) -> Output {
    Command::new(bin).args(args).output().unwrap()
}

fn run_ok(bin: &str, args: &[&str]) -> Output {
    let output = run(bin, args);
    assert!(
        output.status.success(),
        "rivora {} failed: stdout={} stderr={}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    output
}

fn create_investigation(bin: &str, data: &std::path::Path) -> String {
    let output = run_ok(
        bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "investigation",
            "create",
            "Learning CLI",
        ],
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    value["id"].as_str().unwrap().to_string()
}

fn latest_proposal_id(bin: &str, data: &std::path::Path, investigation: &str) -> String {
    let listed = run_ok(
        bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "proposal",
            "list",
            "--investigation",
            investigation,
        ],
    );
    let listed: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    listed["proposals"][0]["id"]
        .as_str()
        .expect("latest proposal id")
        .to_string()
}

fn transition_latest(
    bin: &str,
    data: &std::path::Path,
    investigation: &str,
    status: &str,
    reason: &str,
) -> String {
    let current = latest_proposal_id(bin, data, investigation);
    let output = run_ok(
        bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "proposal",
            "status",
            &current,
            "--investigation",
            investigation,
            "--status",
            status,
            "--reason",
            reason,
        ],
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    value["id"].as_str().unwrap().to_string()
}

fn setup_accepted_proposal(bin: &str, data: &std::path::Path, investigation: &str) -> String {
    run_ok(
        bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "observe",
            "--investigation",
            investigation,
            "--summary",
            "Config validation failed for malformed timestamps",
            "--kind",
            "check_result",
            "--payload",
            r#"{"component":"config","conclusion":"failure"}"#,
        ],
    );
    run_ok(
        bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "pipeline",
            "--investigation",
            investigation,
        ],
    );
    run_ok(
        bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "proposal",
            "generate",
            "--investigation",
            investigation,
        ],
    );

    transition_latest(bin, data, investigation, "proposed", "submit for review");
    transition_latest(bin, data, investigation, "under-review", "begin review");
    let current = latest_proposal_id(bin, data, investigation);
    let accepted = run_ok(
        bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "proposal",
            "accept",
            &current,
            "--investigation",
            investigation,
            "--reason",
            "accept for external implementation only",
        ],
    );
    let accepted: serde_json::Value = serde_json::from_slice(&accepted.stdout).unwrap();
    accepted["id"].as_str().unwrap().to_string()
}

#[test]
fn learning_cli_records_measures_and_learns_without_apply() {
    let dir = tempdir().unwrap();
    let data = dir.path().join("data");
    let bin = rivora_bin();
    let investigation = create_investigation(&bin, &data);
    let proposal_id = setup_accepted_proposal(&bin, &data, &investigation);

    let recorded_out = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "implementation",
            "record",
            "--investigation",
            &investigation,
            "--proposal",
            &proposal_id,
            "--source",
            "git-commit",
            "--summary",
            "Merged bounded config guard",
            "--commit-sha",
            "abc123def456",
            "--observed-file",
            "src/config.rs",
            "--observed-component",
            "config",
            "--declared-scope",
            "config validation only",
            "--actor",
            "engineer",
        ],
    );
    let recorded_text = String::from_utf8_lossy(&recorded_out.stdout);
    assert!(!recorded_text.to_lowercase().contains("\"apply\""));
    let recorded: serde_json::Value = serde_json::from_slice(&recorded_out.stdout).unwrap();
    let impl_id = recorded["id"].as_str().unwrap().to_string();
    assert_eq!(recorded["status"], "reported");
    assert_eq!(
        recorded["boundary"],
        "Measured Learning Outcome — external implementation recorded, never auto-applied; verified only with explicit actor+reason."
    );

    let ready = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "implementation",
            "ready",
            "--investigation",
            &investigation,
            "--implementation",
            &impl_id,
            "--reason",
            "linked commit is enough for evaluation",
            "--actor",
            "engineer",
        ],
    );
    let ready: serde_json::Value = serde_json::from_slice(&ready.stdout).unwrap();
    let ready_id = ready["id"].as_str().unwrap().to_string();
    assert_eq!(ready["status"], "ready_for_evaluation");

    let listed_impls = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "implementation",
            "list",
            "--investigation",
            &investigation,
        ],
    );
    let listed_impls = String::from_utf8_lossy(&listed_impls.stdout);
    assert!(listed_impls.contains(&ready_id) || listed_impls.contains(&impl_id));
    assert!(listed_impls.contains("never auto-applied"));

    let created = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "learn",
            "create",
            "--investigation",
            &investigation,
            "--proposal",
            &proposal_id,
            "--implementation",
            &ready_id,
            "--actor",
            "engineer",
        ],
    );
    let created: serde_json::Value = serde_json::from_slice(&created.stdout).unwrap();
    let mut outcome_id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["status"], "draft");
    assert_eq!(created["classification"], "pending");
    let expected_ids: Vec<String> = created["expected_results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_string())
        .collect();
    assert!(!expected_ids.is_empty());

    for expected in &expected_ids {
        let baseline = run_ok(
            &bin,
            &[
                "--data-dir",
                data.to_str().unwrap(),
                "--json",
                "learn",
                "evidence-add",
                "--investigation",
                &investigation,
                "--outcome",
                &outcome_id,
                "--evidence",
                "00000000-0000-4000-8000-000000000001",
                "--relation",
                "baseline",
                "--expected-result",
                expected,
                "--actor",
                "engineer",
            ],
        );
        let baseline: serde_json::Value = serde_json::from_slice(&baseline.stdout).unwrap();
        outcome_id = baseline["id"].as_str().unwrap().to_string();

        let post = run_ok(
            &bin,
            &[
                "--data-dir",
                data.to_str().unwrap(),
                "--json",
                "learn",
                "evidence-add",
                "--investigation",
                &investigation,
                "--outcome",
                &outcome_id,
                "--evidence",
                "00000000-0000-4000-8000-000000000002",
                "--relation",
                "post-change",
                "--expected-result",
                expected,
                "--actor",
                "engineer",
            ],
        );
        let post: serde_json::Value = serde_json::from_slice(&post.stdout).unwrap();
        outcome_id = post["id"].as_str().unwrap().to_string();

        let support = run_ok(
            &bin,
            &[
                "--data-dir",
                data.to_str().unwrap(),
                "--json",
                "learn",
                "evidence-add",
                "--investigation",
                &investigation,
                "--outcome",
                &outcome_id,
                "--evidence",
                "00000000-0000-4000-8000-000000000003",
                "--relation",
                "supports",
                "--expected-result",
                expected,
                "--reason",
                "observed expected behavior",
                "--actor",
                "engineer",
            ],
        );
        let support: serde_json::Value = serde_json::from_slice(&support.stdout).unwrap();
        outcome_id = support["id"].as_str().unwrap().to_string();
    }

    let evaluated = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "learn",
            "evaluate",
            "--investigation",
            &investigation,
            "--outcome",
            &outcome_id,
            "--actor",
            "runtime",
        ],
    );
    let evaluated: serde_json::Value = serde_json::from_slice(&evaluated.stdout).unwrap();
    outcome_id = evaluated["id"].as_str().unwrap().to_string();
    assert_eq!(evaluated["status"], "evaluated");

    let shown = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "learn",
            "show",
            "--investigation",
            &investigation,
            "--outcome",
            &outcome_id,
        ],
    );
    let shown = String::from_utf8_lossy(&shown.stdout);
    assert!(shown.contains("Measured Outcome"));
    assert!(shown.contains("never auto-applied"));
    assert!(!shown.to_lowercase().contains("applied change"));

    let verified = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "learn",
            "verify",
            "--investigation",
            &investigation,
            "--outcome",
            &outcome_id,
            "--actor",
            "reviewer",
            "--reason",
            "evidence supports successful outcome",
        ],
    );
    let verified: serde_json::Value = serde_json::from_slice(&verified.stdout).unwrap();
    outcome_id = verified["id"].as_str().unwrap().to_string();
    assert_eq!(verified["status"], "verified");
    assert_eq!(verified["historical_learning_eligible"], true);

    let patterns = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "learn",
            "patterns",
            "--derive",
            "--actor",
            "runtime",
        ],
    );
    let patterns: serde_json::Value = serde_json::from_slice(&patterns.stdout).unwrap();
    assert!(
        patterns.as_array().map(|a| !a.is_empty()).unwrap_or(false)
            || patterns
                .as_object()
                .map(|o| o.contains_key("boundary"))
                .unwrap_or(false)
    );

    let influence = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "learn",
            "influence",
            "--investigation",
            &investigation,
            "--proposal",
            &proposal_id,
        ],
    );
    let influence = String::from_utf8_lossy(&influence.stdout);
    assert!(!influence.is_empty());
    assert!(influence.contains("never auto-applied") || influence.contains("primary"));

    let export = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "learn",
            "export",
            "--investigation",
            &investigation,
            "--outcome",
            &outcome_id,
            "--format",
            "markdown",
        ],
    );
    let export = String::from_utf8_lossy(&export.stdout);
    assert!(export.contains("Measured Learning Outcome") || export.contains("Measured"));
    assert!(!export.to_lowercase().contains("auto-applied"));

    let trace = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "learn",
            "trace",
            "--investigation",
            &investigation,
            "--outcome",
            &outcome_id,
        ],
    );
    let trace = String::from_utf8_lossy(&trace.stdout);
    assert!(trace.contains("Proposal"));
    assert!(trace.contains("Implementation"));
    assert!(trace.contains("never auto-applied"));

    // v0.1 disposition path remains available under record-outcome.
    let recs = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "recommend",
            "--investigation",
            &investigation,
        ],
    );
    let recs: serde_json::Value = serde_json::from_slice(&recs.stdout).unwrap();
    if let Some(rec_id) = recs
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|item| item["id"].as_str())
    {
        run_ok(
            &bin,
            &[
                "--data-dir",
                data.to_str().unwrap(),
                "record-outcome",
                "--investigation",
                &investigation,
                "--recommendation",
                rec_id,
                "--disposition",
                "successful",
                "--notes",
                "legacy disposition still works",
            ],
        );
    }
}
