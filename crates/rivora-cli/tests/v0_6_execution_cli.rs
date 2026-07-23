//! CLI smoke for v0.6 execution workflows (mock capability).

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

fn latest_proposal_id(bin: &str, data: &std::path::Path, investigation: &str) -> String {
    let listed = run_json(
        bin,
        data,
        &["proposal", "list", "--investigation", investigation],
    );
    listed["proposals"][0]["id"]
        .as_str()
        .expect("latest proposal id")
        .to_string()
}

fn transition(
    bin: &str,
    data: &std::path::Path,
    investigation: &str,
    status: &str,
    reason: &str,
) -> String {
    let current = latest_proposal_id(bin, data, investigation);
    let value = run_json(
        bin,
        data,
        &[
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
    value["id"].as_str().unwrap().to_string()
}

#[test]
fn cli_execution_plan_preview_approve_dry_run() {
    let dir = tempdir().unwrap();
    let data = dir.path().join("data");
    let bin = rivora_bin();

    let inv = run_json(&bin, &data, &["investigation", "create", "exec cli"]);
    let inv_id = inv["id"].as_str().unwrap();

    run_json(
        &bin,
        &data,
        &[
            "proposal",
            "create",
            "--investigation",
            inv_id,
            "--title",
            "Label issue",
            "--summary",
            "Add bug label",
            "--rationale",
            "Track",
            "--category",
            "process",
        ],
    );
    transition(&bin, &data, inv_id, "proposed", "submit");
    transition(&bin, &data, inv_id, "under-review", "review");
    let proposal_id = {
        let current = latest_proposal_id(&bin, &data, inv_id);
        let accepted = run_json(
            &bin,
            &data,
            &[
                "proposal",
                "accept",
                &current,
                "--investigation",
                inv_id,
                "--reason",
                "accept",
            ],
        );
        accepted["id"].as_str().unwrap().to_string()
    };

    let caps = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "execute",
            "capabilities",
        ],
    );
    let text = String::from_utf8_lossy(&caps.stdout);
    assert!(text.contains("mock.record"));
    assert!(text.contains("Execution Through External Systems"));

    let plan = run_json(
        &bin,
        &data,
        &[
            "execute",
            "plan",
            "--investigation",
            inv_id,
            "--proposal",
            &proposal_id,
            "--capability",
            "mock.record",
            "--action",
            "record_mutation",
            "--inputs",
            r#"{"resource_key":"issue/1","field":"label","value":"bug"}"#,
        ],
    );
    let plan_id = plan["id"].as_str().unwrap().to_string();
    assert_eq!(plan["status"], "draft");

    let ready = run_json(
        &bin,
        &data,
        &[
            "execute",
            "validate",
            "--investigation",
            inv_id,
            "--plan",
            &plan_id,
            "--reason",
            "ok",
        ],
    );
    let ready_id = ready["id"].as_str().unwrap().to_string();

    let _preview = run_json(
        &bin,
        &data,
        &[
            "execute",
            "preview",
            "--investigation",
            inv_id,
            "--plan",
            &ready_id,
        ],
    );

    let approved = run_json(
        &bin,
        &data,
        &[
            "execute",
            "approve",
            "--investigation",
            inv_id,
            "--plan",
            &ready_id,
            "--reason",
            "approved for dry-run path",
        ],
    );
    let live_plan_id = approved["plan"]["id"].as_str().unwrap().to_string();
    let approval_id = approved["approval"]["id"].as_str().unwrap().to_string();

    let attempt = run_json(
        &bin,
        &data,
        &[
            "execute",
            "run",
            "--investigation",
            inv_id,
            "--plan",
            &live_plan_id,
            "--approval",
            &approval_id,
            "--idempotency-key",
            "cli-dry-1",
            "--dry-run",
        ],
    );
    assert_eq!(attempt["dry_run"], true);
    assert_eq!(attempt["status"], "completed");

    let trace = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "execute",
            "trace",
            "--investigation",
            inv_id,
            "--plan",
            &live_plan_id,
        ],
    );
    let trace_text = String::from_utf8_lossy(&trace.stdout);
    assert!(
        trace_text.contains("Proposal Accepted") || trace_text.contains("Execution"),
        "{trace_text}"
    );
}
