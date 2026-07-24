//! Prove the Workspace library entrypoint is the shared launch path.

use std::io::IsTerminal;
use std::process::{Command, Stdio};

use tempfile::tempdir;

#[test]
fn run_workspace_smoke_uses_shared_entrypoint() {
    let dir = tempdir().unwrap();
    let result = rivora_workspace::run_workspace(rivora_workspace::WorkspaceLaunchConfig::smoke(
        dir.path().to_path_buf(),
    ));
    assert!(result.is_ok(), "smoke launch failed: {result:?}");
}

#[test]
fn run_workspace_interactive_requires_tty() {
    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        // Running inside an interactive agent session — skip process-level assertion.
        // The library still rejects when callers force non-TTY via process tests below.
        return;
    }
    let dir = tempdir().unwrap();
    let err = rivora_workspace::run_workspace(
        rivora_workspace::WorkspaceLaunchConfig::interactive(dir.path().to_path_buf()),
    )
    .expect_err("interactive launch without TTY must fail");
    assert!(
        err.contains("interactive Workspace requires a terminal"),
        "unexpected error: {err}"
    );
}

#[test]
fn binary_non_tty_matches_shared_policy() {
    let dir = tempdir().unwrap();
    let bin = env!("CARGO_BIN_EXE_rivora-workspace");
    let out = Command::new(bin)
        .args(["--data-dir", dir.path().to_str().unwrap()])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("interactive Workspace requires a terminal"),
        "binary must use shared TTY policy: {stderr}"
    );
}

#[test]
fn binary_version_is_release() {
    let bin = env!("CARGO_BIN_EXE_rivora-workspace");
    let out = Command::new(bin).arg("--version").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("0.10.0"), "expected 0.10.0, got: {stdout}");
}
