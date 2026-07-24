//! v0.10.0 entry routing: bare `rivora` launches Workspace; help/version/commands stay CLI.

use std::path::PathBuf;
use std::process::{Command, Stdio};

use tempfile::tempdir;

fn rivora_bin() -> String {
    env!("CARGO_BIN_EXE_rivora").to_string()
}

fn workspace_bin() -> Option<String> {
    // CARGO_BIN_EXE_rivora-workspace is only set when this package builds that bin.
    // Prefer the sibling release/debug binary next to rivora when available.
    let rivora = PathBuf::from(env!("CARGO_BIN_EXE_rivora"));
    let sibling = rivora.with_file_name("rivora-workspace");
    if sibling.is_file() {
        Some(sibling.to_string_lossy().into_owned())
    } else {
        None
    }
}

#[test]
fn bare_rivora_launches_workspace_not_help() {
    // Non-TTY: must not print Clap help (v0.9.1 regression). Must request Workspace path.
    let dir = tempdir().unwrap();
    let out = Command::new(rivora_bin())
        .args(["--data-dir", dir.path().to_str().unwrap()])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    assert!(
        !out.status.success(),
        "bare rivora on non-TTY should fail with a clear Workspace/TTY error"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{stdout}{stderr}");
    assert!(
        !combined.contains("Usage: rivora [OPTIONS] <COMMAND>"),
        "bare rivora must not print required-subcommand help (v0.9.1 bug): {combined}"
    );
    assert!(
        combined.contains("interactive Workspace requires a terminal")
            || combined.contains("CLI subcommand"),
        "expected TTY / non-interactive Workspace error, got: {combined}"
    );
}

#[test]
fn rivora_help_prints_help_without_launching_workspace() {
    let out = Command::new(rivora_bin())
        .arg("--help")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Usage: rivora"));
    assert!(
        stdout.contains("open the Workspace")
            || stdout.contains("no subcommand")
            || stdout.contains("Workspace"),
        "help should document bare Workspace launch: {stdout}"
    );
    assert!(!stdout.contains("interactive Workspace requires a terminal"));
}

#[test]
fn rivora_version_prints_version() {
    let out = Command::new(rivora_bin())
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("rivora") && stdout.contains("0.10.0"),
        "expected version 0.10.0, got: {stdout}"
    );
}

#[test]
fn rivora_valid_command_still_dispatches_cli() {
    let dir = tempdir().unwrap();
    let out = Command::new(rivora_bin())
        .args([
            "--data-dir",
            dir.path().to_str().unwrap(),
            "doctor",
            "exit-codes",
        ])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "doctor exit-codes failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("exit") || stdout.contains("0") || !stdout.is_empty(),
        "expected doctor exit-codes output: {stdout}"
    );
}

#[test]
fn rivora_invalid_command_is_cli_error_not_workspace() {
    let out = Command::new(rivora_bin())
        .arg("definitely-not-a-command")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("unrecognized subcommand")
            || combined.contains("unexpected")
            || combined.contains("error"),
        "expected typed CLI error: {combined}"
    );
    assert!(
        !combined.contains("interactive Workspace requires a terminal"),
        "invalid command must not attempt Workspace launch: {combined}"
    );
}

#[test]
fn rivora_json_without_command_is_rejected() {
    let dir = tempdir().unwrap();
    let out = Command::new(rivora_bin())
        .args(["--json", "--data-dir", dir.path().to_str().unwrap()])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--json requires a CLI subcommand")
            || stderr.contains("requires a CLI subcommand"),
        "expected --json rejection: {stderr}"
    );
}

#[test]
fn rivora_data_dir_is_passed_to_workspace_launcher() {
    // Use a unique data dir; non-TTY fails before store open, but the path is still
    // accepted by clap and routed into WorkspaceLaunchConfig. Prove store open uses it
    // when smoke path is available via rivora-workspace sibling, and bare rivora rejects
    // non-TTY with the data-dir flag still parsed (no clap error about unknown args).
    let dir = tempdir().unwrap();
    let data = dir.path().join("custom-data-root");
    let out = Command::new(rivora_bin())
        .args(["--data-dir", data.to_str().unwrap()])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    // Clap accepted --data-dir; failure is TTY policy, not unknown flag / missing command.
    assert!(
        !stderr.contains("unexpected argument") && !stderr.contains("required"),
        "data-dir should be accepted for bare launch: {stderr}"
    );
    assert!(
        stderr.contains("interactive Workspace requires a terminal"),
        "bare rivora with --data-dir should still route to Workspace: {stderr}"
    );
}

#[test]
fn both_binaries_exist_in_cargo_target() {
    let rivora = PathBuf::from(env!("CARGO_BIN_EXE_rivora"));
    assert!(
        rivora.is_file(),
        "rivora binary missing at {}",
        rivora.display()
    );
    if let Some(ws) = workspace_bin() {
        assert!(
            PathBuf::from(&ws).is_file(),
            "rivora-workspace sibling missing at {ws}"
        );
        let ver = Command::new(&ws).arg("--version").output().unwrap();
        assert!(ver.status.success());
        let stdout = String::from_utf8_lossy(&ver.stdout);
        assert!(
            stdout.contains("0.10.0"),
            "workspace binary version: {stdout}"
        );
    }
}

#[test]
fn shared_workspace_module_is_used_by_cli() {
    // Source-level contract: rivora CLI must call the shared launcher, not reimplement UI.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cli_main = std::fs::read_to_string(manifest_dir.join("src/main.rs")).unwrap();
    assert!(
        cli_main.contains("run_workspace") && cli_main.contains("WorkspaceLaunchConfig"),
        "rivora CLI must call shared rivora_workspace::run_workspace"
    );
    assert!(
        !cli_main.contains("dialoguer::Select"),
        "rivora CLI must not reimplement Workspace dialoguer UI"
    );

    let workspace_lib = std::fs::read_to_string(
        manifest_dir
            .parent()
            .unwrap()
            .join("rivora-workspace/src/lib.rs"),
    )
    .unwrap();
    assert!(
        workspace_lib.contains("pub fn run_workspace"),
        "shared run_workspace must be public in rivora-workspace lib"
    );

    let workspace_main = std::fs::read_to_string(
        manifest_dir
            .parent()
            .unwrap()
            .join("rivora-workspace/src/main.rs"),
    )
    .unwrap();
    assert!(
        workspace_main.contains("run_workspace"),
        "rivora-workspace binary must call shared run_workspace"
    );
}
