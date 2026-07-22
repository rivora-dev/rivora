//! CLI smoke tests — process-level Capability invocation.

use std::process::Command;

use tempfile::tempdir;

fn rivora_bin() -> String {
    env!("CARGO_BIN_EXE_rivora").to_string()
}

fn run_ok(bin: &str, args: &[&str]) -> std::process::Output {
    let out = Command::new(bin).args(args).output().unwrap();
    assert!(
        out.status.success(),
        "rivora {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn create_investigation(bin: &str, data: &std::path::Path, title: &str) -> String {
    let out = run_ok(
        bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "investigation",
            "create",
            title,
        ],
    );
    let created: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    created["id"].as_str().unwrap().to_string()
}

#[test]
fn cli_investigation_graph_workflow() {
    let dir = tempdir().unwrap();
    let data = dir.path().join("data");
    let bin = rivora_bin();

    let a = create_investigation(&bin, &data, "CLI graph A");
    let b = create_investigation(&bin, &data, "CLI graph B");

    // Both investigations observe the same repository.
    for id in [&a, &b] {
        run_ok(
            &bin,
            &[
                "--data-dir",
                data.to_str().unwrap(),
                "observe",
                "--investigation",
                id,
                "--summary",
                "GitHub repository `acme/app`",
                "--kind",
                "repository",
                "--payload",
                r#"{"full_name":"acme/app"}"#,
            ],
        );
    }

    // Derive relationships for A.
    run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "investigation",
            "refresh-relationships",
            &a,
        ],
    );

    // A should now list B as related via shared repository.
    let related = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "investigation",
            "related",
            &a,
        ],
    );
    let related_json: serde_json::Value = serde_json::from_slice(&related.stdout).unwrap();
    let entries = related_json.as_array().unwrap();
    assert!(
        entries
            .iter()
            .any(|e| e["related"]["id"].as_str() == Some(b.as_str())),
        "expected B related to A: {related_json}"
    );
    let derived_id = entries
        .iter()
        .find(|e| e["relationship"]["kind"].as_str() == Some("shared_repository"))
        .map(|e| e["relationship"]["id"].as_str().unwrap().to_string())
        .expect("shared_repository relationship present");

    // Explanation answers why.
    let explained = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "investigation",
            "relationship",
            &derived_id,
        ],
    );
    let explanation = String::from_utf8_lossy(&explained.stdout);
    assert!(
        explanation.contains("acme/app"),
        "explanation cites evidence: {explanation}"
    );

    // Explicit link, confirm, then unlink.
    run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "investigation",
            "link",
            &a,
            &b,
            "--reason",
            "same incident",
        ],
    );
    let related = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "investigation",
            "related",
            &a,
        ],
    );
    let related_json: serde_json::Value = serde_json::from_slice(&related.stdout).unwrap();
    let link_id = related_json
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["relationship"]["kind"].as_str() == Some("explicit_link"))
        .map(|e| e["relationship"]["id"].as_str().unwrap().to_string())
        .expect("explicit link present");

    run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "investigation",
            "confirm-relationship",
            &link_id,
        ],
    );
    run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "investigation",
            "unlink",
            &link_id,
        ],
    );

    // Unlinking a derived relationship is rejected.
    let denied = Command::new(&bin)
        .args([
            "--data-dir",
            data.to_str().unwrap(),
            "investigation",
            "unlink",
            &derived_id,
        ])
        .output()
        .unwrap();
    assert!(
        !denied.status.success(),
        "derived unlink must fail: {}",
        String::from_utf8_lossy(&denied.stdout)
    );
}

#[test]
fn cli_search_and_recall_workflow() {
    let dir = tempdir().unwrap();
    let data = dir.path().join("data");
    let bin = rivora_bin();

    let a = create_investigation(&bin, &data, "Deploy regression in acme app");
    let b = create_investigation(&bin, &data, "CI build broken in acme app");

    for (id, key) in [(&a, "a"), (&b, "b")] {
        run_ok(
            &bin,
            &[
                "--data-dir",
                data.to_str().unwrap(),
                "observe",
                "--investigation",
                id,
                "--summary",
                "GitHub repository `acme/app`",
                "--kind",
                "repository",
                "--payload",
                r#"{"full_name":"acme/app"}"#,
                "--idempotency-key",
                &format!("repo-{key}"),
            ],
        );
        run_ok(
            &bin,
            &[
                "--data-dir",
                data.to_str().unwrap(),
                "observe",
                "--investigation",
                id,
                "--summary",
                "Check build failed",
                "--kind",
                "check_result",
                "--payload",
                r#"{"name":"build","conclusion":"failure"}"#,
                "--idempotency-key",
                &format!("check-{key}"),
            ],
        );
        run_ok(
            &bin,
            &[
                "--data-dir",
                data.to_str().unwrap(),
                "pipeline",
                "--investigation",
                id,
            ],
        );
    }

    // Record a successful outcome on A.
    let recs = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "recommend",
            "--investigation",
            &a,
        ],
    );
    let rec_json: serde_json::Value = serde_json::from_slice(&recs.stdout).unwrap();
    let rec_id = rec_json[0]["id"].as_str().unwrap();
    run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "learn",
            "--investigation",
            &a,
            "--recommendation",
            rec_id,
            "--disposition",
            "successful",
            "--notes",
            "rollback resolved it",
        ],
    );

    // Text search finds A and B.
    let search = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "search",
            "acme build failed",
        ],
    );
    let results: serde_json::Value = serde_json::from_slice(&search.stdout).unwrap();
    let ids: Vec<&str> = results
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| r["investigation_id"].as_str())
        .collect();
    assert!(ids.contains(&a.as_str()) && ids.contains(&b.as_str()));
    for result in results.as_array().unwrap() {
        assert!(result["explanation"].as_str().unwrap().len() > 10);
        assert!(!result["matched_evidence"].as_array().unwrap().is_empty());
    }

    // Structured search: repository + outcome filters.
    let search = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "search",
            "--repository",
            "acme/app",
            "--outcome",
            "successful",
        ],
    );
    let results: serde_json::Value = serde_json::from_slice(&search.stdout).unwrap();
    assert_eq!(results.as_array().unwrap().len(), 1);
    assert_eq!(results[0]["investigation_id"].as_str(), Some(a.as_str()));

    // Explain a search result.
    let explained = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "search",
            "--repository",
            "acme/app",
            "--explain",
            &a,
        ],
    );
    assert!(String::from_utf8_lossy(&explained.stdout).contains("acme/app"));

    // Similar investigations for B rank A first.
    let similar = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "investigation",
            "similar",
            &b,
        ],
    );
    let results: serde_json::Value = serde_json::from_slice(&similar.stdout).unwrap();
    assert_eq!(
        results.as_array().unwrap()[0]["investigation_id"].as_str(),
        Some(a.as_str())
    );

    // Recall: memory (v0.1), related evidence, prior outcomes.
    run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "recall",
            "--investigation",
            &a,
        ],
    );
    run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "investigation",
            "refresh-relationships",
            &a,
        ],
    );
    let evidence = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "recall",
            "--investigation",
            &a,
            "--evidence",
        ],
    );
    assert!(String::from_utf8_lossy(&evidence.stdout).contains("shared_repository"));

    let outcomes = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "recall",
            "--outcome",
            "successful",
        ],
    );
    let outcomes_json: serde_json::Value = serde_json::from_slice(&outcomes.stdout).unwrap();
    assert_eq!(outcomes_json.as_array().unwrap().len(), 1);
    assert_eq!(
        outcomes_json[0]["investigation_id"].as_str(),
        Some(a.as_str())
    );
}

#[test]
fn cli_recalled_context_patterns_and_trends_workflow() {
    let dir = tempdir().unwrap();
    let data = dir.path().join("data");
    let bin = rivora_bin();

    let a = create_investigation(&bin, &data, "CLI context A acme");
    let b = create_investigation(&bin, &data, "CLI context B acme");
    let c = create_investigation(&bin, &data, "CLI context C current");

    for (id, key) in [(&a, "a"), (&b, "b"), (&c, "c")] {
        run_ok(
            &bin,
            &[
                "--data-dir",
                data.to_str().unwrap(),
                "observe",
                "--investigation",
                id,
                "--summary",
                "GitHub repository `acme/app`",
                "--kind",
                "repository",
                "--payload",
                r#"{"full_name":"acme/app"}"#,
                "--idempotency-key",
                &format!("ctx-repo-{key}"),
            ],
        );
        run_ok(
            &bin,
            &[
                "--data-dir",
                data.to_str().unwrap(),
                "observe",
                "--investigation",
                id,
                "--summary",
                "Check build failed",
                "--kind",
                "check_result",
                "--payload",
                r#"{"name":"build","conclusion":"failure"}"#,
                "--idempotency-key",
                &format!("ctx-check-{key}"),
            ],
        );
        run_ok(
            &bin,
            &[
                "--data-dir",
                data.to_str().unwrap(),
                "pipeline",
                "--investigation",
                id,
            ],
        );
    }

    // Attach historical context from A into C.
    let attached = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "investigation",
            "context-attach",
            &c,
            "--source",
            &a,
            "--reason",
            "prior acme failure",
        ],
    );
    let ctx: serde_json::Value = serde_json::from_slice(&attached.stdout).unwrap();
    assert_eq!(ctx["state"].as_str(), Some("attached"));
    assert_eq!(ctx["investigation_id"].as_str(), Some(c.as_str()));
    assert_eq!(ctx["source_investigation_id"].as_str(), Some(a.as_str()));
    let context_id = ctx["id"].as_str().unwrap().to_string();

    // List context.
    let listed = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "investigation",
            "context",
            &c,
        ],
    );
    let contexts: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(contexts.as_array().unwrap().len(), 1);

    // Patterns and trends.
    let patterns = run_ok(
        &bin,
        &["--data-dir", data.to_str().unwrap(), "--json", "patterns"],
    );
    let patterns_json: serde_json::Value = serde_json::from_slice(&patterns.stdout).unwrap();
    assert!(!patterns_json.as_array().unwrap().is_empty());

    let trends = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "trends",
            "--repository",
            "acme/app",
        ],
    );
    let trend: serde_json::Value = serde_json::from_slice(&trends.stdout).unwrap();
    assert!(trend["investigation_count"].as_u64().unwrap() >= 3);

    // Dismiss does not delete history; source investigation still lists empty context.
    run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "investigation",
            "context-dismiss",
            &c,
            &context_id,
        ],
    );
    let source_ctx = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "investigation",
            "context",
            &a,
        ],
    );
    let source_json: serde_json::Value = serde_json::from_slice(&source_ctx.stdout).unwrap();
    assert!(source_json.as_array().unwrap().is_empty());
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
