//! CLI smoke tests for v0.7 Capability Engineering Loop commands.

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
fn capability_list_and_show_include_engineering_loop() {
    let dir = tempdir().unwrap();
    let data = dir.path().join("data");
    let bin = rivora_bin();

    let list = run_ok(
        &bin,
        &["--data-dir", data.to_str().unwrap(), "capability", "list"],
    );
    let text = String::from_utf8_lossy(&list.stdout);
    assert!(text.contains("mock.record"), "stdout={text}");
    assert!(text.contains("loop="), "stdout={text}");

    let desc = run_json(&bin, &data, &["capability", "show", "mock.record"]);
    assert_eq!(desc["capability_id"], "mock.record");
    assert_eq!(desc["engineering_loop"]["memory"], "supported");
    assert_eq!(desc["engineering_loop"]["learning"], "deferred");
}

#[test]
fn capability_lifecycle_end_to_end_json() {
    let dir = tempdir().unwrap();
    let data = dir.path().join("data");
    let bin = rivora_bin();

    let inv = run_json(
        &bin,
        &data,
        &["investigation", "create", "v0.7 cli lifecycle"],
    );
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
            "CI dispatch",
            "--summary",
            "run mock mutation",
            "--rationale",
            "lifecycle proof",
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
            "--action-input",
            r#"{"resource_key":"cli/loop","field":"ok","value":"1"}"#,
        ],
    );
    let plan_id = plan["id"].as_str().unwrap().to_string();

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
            "ready",
        ],
    );
    let ready_id = ready["id"].as_str().unwrap().to_string();

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
            "ok",
        ],
    );
    let approved_plan_id = approved["plan"]["id"].as_str().unwrap().to_string();
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
            &approved_plan_id,
            "--approval",
            &approval_id,
            "--idempotency-key",
            "cli-v07-1",
            "--confirm",
        ],
    );
    let attempt_id = attempt["id"].as_str().unwrap().to_string();

    let _ = run_json(
        &bin,
        &data,
        &[
            "execute",
            "verify",
            "--investigation",
            inv_id,
            "--attempt",
            &attempt_id,
        ],
    );

    let lifecycle = run_json(
        &bin,
        &data,
        &[
            "capability",
            "lifecycle",
            "--investigation",
            inv_id,
            "--attempt",
            &attempt_id,
        ],
    );
    assert_eq!(lifecycle["capability_id"], "mock.record");
    assert!(lifecycle["stages"].is_array());
    assert!(
        lifecycle["status"] == "completed" || lifecycle["status"] == "partial",
        "status={}",
        lifecycle["status"]
    );

    let trace = run_json(
        &bin,
        &data,
        &[
            "capability",
            "trace",
            "--investigation",
            inv_id,
            &attempt_id,
        ],
    );
    assert_eq!(trace["capability_id"], "mock.record");
    assert!(trace.get("run_id").is_some());

    let lifecycle2 = run_json(
        &bin,
        &data,
        &[
            "capability",
            "lifecycle",
            "--investigation",
            inv_id,
            "--attempt",
            &attempt_id,
        ],
    );
    assert_eq!(lifecycle["lineage_id"], lifecycle2["lineage_id"]);
}
