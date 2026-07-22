//! Workspace smoke test via non-interactive --smoke flag.

use std::process::Command;

use tempfile::tempdir;

#[test]
fn workspace_smoke_completes_investigation() {
    let dir = tempdir().unwrap();
    let bin = env!("CARGO_BIN_EXE_rivora-workspace");
    let output = Command::new(bin)
        .args(["--data-dir", dir.path().to_str().unwrap(), "--smoke"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("workspace smoke ok"));
    assert!(stdout.contains("Proposal only — not applied, not implemented, not verified."));
    assert!(stdout.contains("Workspace Proposal"));
    assert!(stdout.contains("Workspace Proposal alternatives: 2"));
    assert!(stdout.contains("Ranking is guidance, not a guaranteed correct implementation."));
    assert!(stdout.contains("Verification Plan is proposed work; it was not executed."));
    assert!(!stdout.contains("Apply Proposal"));
}
