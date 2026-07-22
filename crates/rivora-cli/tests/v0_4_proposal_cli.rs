//! v0.4 Phase 1 Proposal CLI flows through the shared CapabilityService.

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
            "Proposal CLI",
        ],
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    value["id"].as_str().unwrap().to_string()
}

fn create_proposal(
    bin: &str,
    data: &std::path::Path,
    investigation: &str,
    title: &str,
) -> serde_json::Value {
    let output = run_ok(
        bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "proposal",
            "create",
            "--investigation",
            investigation,
            "--title",
            title,
            "--summary",
            "Add deterministic validation",
            "--rationale",
            "Verified failures require a bounded validation change",
            "--category",
            "reliability",
            "--priority",
            "high",
            "--confidence",
            "0.8",
        ],
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn proposal_cli_preserves_lifecycle_feedback_and_revisions() {
    let dir = tempdir().unwrap();
    let data = dir.path().join("data");
    let bin = rivora_bin();
    let investigation = create_investigation(&bin, &data);

    let created = create_proposal(
        &bin,
        &data,
        &investigation,
        "Validate deployment configuration",
    );
    let proposal_id = created["id"].as_str().unwrap().to_string();
    let lineage_id = created["lineage_id"].as_str().unwrap().to_string();
    assert_eq!(created["status"], "draft");
    assert_eq!(
        created["boundary"],
        "Proposal only — not applied, not implemented, not verified."
    );

    let listed = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "proposal",
            "list",
            "--investigation",
            &investigation,
        ],
    );
    let listed: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(listed["proposals"].as_array().unwrap().len(), 1);
    assert_eq!(
        listed["boundary"],
        "Proposal only — not applied, not implemented, not verified."
    );

    let shown = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "proposal",
            "show",
            &proposal_id,
            "--investigation",
            &investigation,
        ],
    );
    let shown = String::from_utf8_lossy(&shown.stdout);
    assert!(shown.contains("Proposal only — not applied, not implemented, not verified."));
    assert!(shown.contains("Validate deployment configuration"));

    let explained = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "proposal",
            "explain",
            &proposal_id,
            "--investigation",
            &investigation,
        ],
    );
    assert!(String::from_utf8_lossy(&explained.stdout)
        .contains("Proposal only — not applied, not implemented, not verified."));

    let feedback = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "proposal",
            "feedback",
            &proposal_id,
            "--investigation",
            &investigation,
            "--category",
            "too-broad",
            "--comment",
            "Limit this to deployment configuration",
        ],
    );
    let feedback: serde_json::Value = serde_json::from_slice(&feedback.stdout).unwrap();
    let feedback_id = feedback["id"].as_str().unwrap().to_string();
    assert_eq!(feedback["feedback"][0]["actor"], "cli");

    let refined = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "proposal",
            "refine",
            &feedback_id,
            "--investigation",
            &investigation,
            "--summary",
            "Validate deployment configuration only",
            "--affected-component",
            "deployment-config",
            "--test",
            "Add malformed configuration fixtures",
            "--reason",
            "Address explicit scope feedback",
        ],
    );
    let refined: serde_json::Value = serde_json::from_slice(&refined.stdout).unwrap();
    assert_eq!(refined["summary"], "Validate deployment configuration only");
    assert_eq!(refined["parent_proposal_id"], feedback_id);
    assert!(refined["revision_number"].as_u64().unwrap() >= 3);

    let revisions = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "proposal",
            "revisions",
            &lineage_id,
            "--investigation",
            &investigation,
        ],
    );
    let revisions: serde_json::Value = serde_json::from_slice(&revisions.stdout).unwrap();
    assert_eq!(revisions["proposals"].as_array().unwrap().len(), 3);

    let rejected = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "proposal",
            "reject",
            refined["id"].as_str().unwrap(),
            "--investigation",
            &investigation,
            "--reason",
            "Alternative has lower risk",
        ],
    );
    let rejected: serde_json::Value = serde_json::from_slice(&rejected.stdout).unwrap();
    assert_eq!(rejected["status"], "rejected");
    assert_eq!(rejected["transitions"][0]["actor"], "cli");
    assert_eq!(
        rejected["transitions"][0]["reason"],
        "Alternative has lower risk"
    );
}

#[test]
fn proposal_acceptance_is_explicit_and_reason_is_required() {
    let dir = tempdir().unwrap();
    let data = dir.path().join("data");
    let bin = rivora_bin();
    let investigation = create_investigation(&bin, &data);
    let created = create_proposal(&bin, &data, &investigation, "Bounded candidate");
    let proposal_id = created["id"].as_str().unwrap();

    let missing_reason = run(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "proposal",
            "accept",
            proposal_id,
            "--investigation",
            &investigation,
        ],
    );
    assert!(!missing_reason.status.success());

    let proposed = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "proposal",
            "status",
            proposal_id,
            "--investigation",
            &investigation,
            "--status",
            "proposed",
            "--reason",
            "Human submitted the Draft",
        ],
    );
    let proposed: serde_json::Value = serde_json::from_slice(&proposed.stdout).unwrap();
    let proposed_id = proposed["id"].as_str().unwrap();
    let review = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "proposal",
            "status",
            proposed_id,
            "--investigation",
            &investigation,
            "--status",
            "under-review",
            "--reason",
            "Human review started",
        ],
    );
    let review: serde_json::Value = serde_json::from_slice(&review.stdout).unwrap();
    let review_id = review["id"].as_str().unwrap();

    let accepted = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "proposal",
            "accept",
            review_id,
            "--investigation",
            &investigation,
            "--reason",
            "Approved for possible later implementation",
        ],
    );
    let accepted: serde_json::Value = serde_json::from_slice(&accepted.stdout).unwrap();
    assert_eq!(accepted["status"], "accepted");
    assert_eq!(accepted["transitions"][2]["actor"], "cli");
    assert!(accepted.get("implemented").is_none());
}

#[test]
fn proposal_cli_exposes_durable_defer_withdraw_and_supersede_actions() {
    let dir = tempdir().unwrap();
    let data = dir.path().join("data");
    let bin = rivora_bin();
    let investigation = create_investigation(&bin, &data);

    let deferred = create_proposal(&bin, &data, &investigation, "Deferred candidate");
    let deferred = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "proposal",
            "defer",
            deferred["id"].as_str().unwrap(),
            "--investigation",
            &investigation,
            "--reason",
            "Wait for stronger evidence",
        ],
    );
    let deferred: serde_json::Value = serde_json::from_slice(&deferred.stdout).unwrap();
    assert_eq!(deferred["status"], "deferred");

    let withdrawn = create_proposal(&bin, &data, &investigation, "Withdrawn candidate");
    let withdrawn = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "proposal",
            "withdraw",
            withdrawn["id"].as_str().unwrap(),
            "--investigation",
            &investigation,
            "--reason",
            "Owner withdrew the candidate",
        ],
    );
    let withdrawn: serde_json::Value = serde_json::from_slice(&withdrawn.stdout).unwrap();
    assert_eq!(withdrawn["status"], "withdrawn");

    let original = create_proposal(&bin, &data, &investigation, "Original candidate");
    let replacement = create_proposal(&bin, &data, &investigation, "Replacement candidate");
    let superseded = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "proposal",
            "supersede",
            original["id"].as_str().unwrap(),
            "--investigation",
            &investigation,
            "--replacement",
            replacement["id"].as_str().unwrap(),
            "--reason",
            "Replacement has a smaller scope",
        ],
    );
    let superseded: serde_json::Value = serde_json::from_slice(&superseded.stdout).unwrap();
    assert_eq!(superseded["status"], "superseded");
    assert_eq!(superseded["superseding_proposal_id"], replacement["id"]);

    let help = run_ok(&bin, &["proposal", "--help"]);
    assert!(!String::from_utf8_lossy(&help.stdout).contains("apply"));
}

#[test]
fn proposal_cli_generates_compares_prioritizes_and_explains_plans() {
    let dir = tempdir().unwrap();
    let data = dir.path().join("data");
    let bin = rivora_bin();
    let investigation = create_investigation(&bin, &data);

    run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "observe",
            "--investigation",
            &investigation,
            "--summary",
            "Deployment validation failed repeatedly",
            "--kind",
            "check_result",
            "--payload",
            r#"{"conclusion":"failure","component":"deployment-config"}"#,
        ],
    );

    let generated = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "proposal",
            "generate",
            "--investigation",
            &investigation,
        ],
    );
    let generated: serde_json::Value = serde_json::from_slice(&generated.stdout).unwrap();
    assert_eq!(generated["proposals"].as_array().unwrap().len(), 2);
    assert_eq!(
        generated["boundary"],
        "Proposal only — not applied, not implemented, not verified."
    );
    let first = generated["proposals"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let second = generated["proposals"][1]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(generated["proposals"][0]["status"], "draft");
    assert_eq!(
        generated["proposals"][0]["generation_method"],
        "deterministic"
    );

    let compared = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "proposal",
            "compare",
            "--investigation",
            &investigation,
            &first,
            &second,
        ],
    );
    let compared: serde_json::Value = serde_json::from_slice(&compared.stdout).unwrap();
    assert_eq!(compared["ranked"].as_array().unwrap().len(), 2);
    assert!(!compared["ranked"][0]["factors"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(compared["explanation"]
        .as_str()
        .unwrap()
        .contains("not guaranteed correct"));
    assert_eq!(
        compared["boundary"],
        "Proposal only — not applied, not implemented, not verified."
    );

    let prioritized = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "proposal",
            "prioritize",
            "--investigation",
            &investigation,
        ],
    );
    let prioritized: serde_json::Value = serde_json::from_slice(&prioritized.stdout).unwrap();
    assert_eq!(prioritized["ranked"].as_array().unwrap().len(), 2);

    let verification = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "proposal",
            "verification-plan",
            &first,
            "--investigation",
            &investigation,
        ],
    );
    let verification: serde_json::Value = serde_json::from_slice(&verification.stdout).unwrap();
    assert!(!verification["claims"].as_array().unwrap().is_empty());
    assert_eq!(
        verification["boundary"],
        "Proposal only — not applied, not implemented, not verified."
    );

    let implementation = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "proposal",
            "implementation-plan",
            &first,
            "--investigation",
            &investigation,
        ],
    );
    let implementation: serde_json::Value = serde_json::from_slice(&implementation.stdout).unwrap();
    assert!(!implementation["outline"].as_array().unwrap().is_empty());
    assert_eq!(
        implementation["boundary"],
        "Proposal only — not applied, not implemented, not verified."
    );

    let provenance = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "proposal",
            "provenance",
            &first,
            "--investigation",
            &investigation,
        ],
    );
    let provenance: serde_json::Value = serde_json::from_slice(&provenance.stdout).unwrap();
    assert!(provenance["provenance"]
        .as_str()
        .unwrap()
        .contains("current"));
    assert!(provenance["provenance"]
        .as_str()
        .unwrap()
        .contains("labeled historical"));
    assert_eq!(
        provenance["boundary"],
        "Proposal only — not applied, not implemented, not verified."
    );
}

#[test]
fn proposal_cli_exports_handoff_portfolio_and_trace_through_capabilities() {
    let dir = tempdir().unwrap();
    let data = dir.path().join("data");
    let bin = rivora_bin();
    let investigation = create_investigation(&bin, &data);
    let created = create_proposal(
        &bin,
        &data,
        &investigation,
        "Export bounded deployment validation",
    );
    let proposal_id = created["id"].as_str().unwrap();

    let markdown = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "proposal",
            "export",
            proposal_id,
            "--investigation",
            &investigation,
            "--format",
            "markdown",
        ],
    );
    let markdown = String::from_utf8_lossy(&markdown.stdout);
    assert!(markdown.contains("# Improvement Proposal: Export bounded deployment validation"));
    assert!(markdown.contains("Proposal only — not applied, not implemented, not verified."));

    let structured = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "proposal",
            "export",
            proposal_id,
            "--investigation",
            &investigation,
            "--format",
            "json",
        ],
    );
    let structured: serde_json::Value = serde_json::from_slice(&structured.stdout).unwrap();
    assert_eq!(structured["proposal_id"], proposal_id);
    assert_eq!(structured["proposal"]["title"], created["title"]);
    assert_eq!(
        structured["boundary"],
        "Proposal only — not applied, not implemented, not verified."
    );

    let handoff = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "proposal",
            "handoff",
            proposal_id,
            "--investigation",
            &investigation,
        ],
    );
    let handoff = String::from_utf8_lossy(&handoff.stdout);
    assert!(handoff.contains("This is an implementation proposal."));
    assert!(handoff.contains("Do not exceed the approved Proposal scope."));
    assert!(handoff.contains("Proposal only — not applied, not implemented, not verified."));

    let portfolio = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "proposal",
            "portfolio",
            "--investigation",
            &investigation,
            "--priority",
            "high",
            "--category",
            "reliability",
            "--affected-component",
            "runtime",
        ],
    );
    let portfolio: serde_json::Value = serde_json::from_slice(&portfolio.stdout).unwrap();
    assert_eq!(portfolio["proposals"].as_array().unwrap().len(), 0);
    assert_eq!(
        portfolio["boundary"],
        "Proposal only — not applied, not implemented, not verified."
    );

    let portfolio = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "proposal",
            "portfolio",
            "--investigation",
            &investigation,
            "--priority",
            "high",
            "--category",
            "reliability",
        ],
    );
    let portfolio: serde_json::Value = serde_json::from_slice(&portfolio.stdout).unwrap();
    assert_eq!(portfolio["proposals"].as_array().unwrap().len(), 1);
    assert_eq!(portfolio["proposals"][0]["id"], proposal_id);

    let trace = run_ok(
        &bin,
        &[
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "proposal",
            "trace",
            proposal_id,
            "--investigation",
            &investigation,
        ],
    );
    let trace: serde_json::Value = serde_json::from_slice(&trace.stdout).unwrap();
    assert_eq!(trace["proposal_id"], proposal_id);
    assert_eq!(
        trace["boundary"],
        "Proposal only — not applied, not implemented, not verified."
    );

    let help = run_ok(&bin, &["proposal", "export", "--help"]);
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(!help.contains("output-path"));
    assert!(!help.contains("apply"));
}
