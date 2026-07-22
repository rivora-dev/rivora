//! CLI smoke tests — process-level Capability invocation.

use std::process::Command;

use tempfile::tempdir;

fn rivora_bin() -> String {
    env!("CARGO_BIN_EXE_rivora").to_string()
}

#[test]
fn cli_full_investigation_workflow() {
    let dir = tempdir().unwrap();
    let data = dir.path().join("data");
    let bin = rivora_bin();

    let create = Command::new(&bin)
        .args([
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "investigation",
            "create",
            "CLI workflow",
            "--description",
            "smoke",
        ])
        .output()
        .unwrap();
    assert!(
        create.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&create.stderr)
    );
    let created: serde_json::Value = serde_json::from_slice(&create.stdout).unwrap();
    let id = created["id"].as_str().unwrap();

    let observe = Command::new(&bin)
        .args([
            "--data-dir",
            data.to_str().unwrap(),
            "observe",
            "--investigation",
            id,
            "--summary",
            "CI failed",
            "--kind",
            "check_result",
            "--payload",
            r#"{"status":"failure","error":"boom"}"#,
            "--idempotency-key",
            "cli-1",
        ])
        .output()
        .unwrap();
    assert!(
        observe.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&observe.stderr)
    );

    for cmd in ["knowledge", "evaluate", "verify", "recommend"] {
        let out = Command::new(&bin)
            .args([
                "--data-dir",
                data.to_str().unwrap(),
                cmd,
                "--investigation",
                id,
            ])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{cmd} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    // Fetch recommendation id via show/json pipeline
    let recs = Command::new(&bin)
        .args([
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "recommend",
            "--investigation",
            id,
        ])
        .output()
        .unwrap();
    assert!(recs.status.success());
    let rec_json: serde_json::Value = serde_json::from_slice(&recs.stdout).unwrap();
    let rec_id = rec_json[0]["id"].as_str().unwrap();

    let learn = Command::new(&bin)
        .args([
            "--data-dir",
            data.to_str().unwrap(),
            "learn",
            "--investigation",
            id,
            "--recommendation",
            rec_id,
            "--disposition",
            "accepted",
            "--notes",
            "cli accepted",
        ])
        .output()
        .unwrap();
    assert!(
        learn.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&learn.stderr)
    );

    let complete = Command::new(&bin)
        .args([
            "--data-dir",
            data.to_str().unwrap(),
            "investigation",
            "complete",
            id,
        ])
        .output()
        .unwrap();
    assert!(
        complete.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&complete.stderr)
    );

    let reopen = Command::new(&bin)
        .args([
            "--data-dir",
            data.to_str().unwrap(),
            "investigation",
            "reopen",
            id,
        ])
        .output()
        .unwrap();
    assert!(
        reopen.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&reopen.stderr)
    );
}
