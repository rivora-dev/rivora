//! Rivora Workspace binary — thin CLI wrapper over the shared Workspace launcher.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use rivora_workspace::{run_workspace, WorkspaceLaunchConfig};

#[derive(Debug, Parser)]
#[command(
    name = "rivora-workspace",
    version,
    about = "Rivora Unified Workspace — conversation-first Investigations"
)]
struct Args {
    /// Data directory for local Runtime storage.
    #[arg(long, default_value = ".rivora/data")]
    data_dir: PathBuf,

    /// Run a single non-interactive demo workflow (for tests/CI).
    #[arg(long)]
    smoke: bool,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let config = WorkspaceLaunchConfig {
        data_dir: args.data_dir,
        smoke: args.smoke,
    };
    match run_workspace(config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::from(1)
        }
    }
}
