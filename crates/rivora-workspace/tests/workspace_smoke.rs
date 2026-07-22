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
}
