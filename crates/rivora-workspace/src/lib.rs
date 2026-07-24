//! Rivora Workspace — primary interactive experience (RFC-003, Unified Workspace v0.10).
//!
//! Thin presentation layer over [`CapabilityService`]. No Runtime reasoning is
//! implemented here. Conversation is a projection over typed intents and durable
//! engineering objects — never the persistence model or authority source.
//!
//! Both the `rivora-workspace` binary and bare `rivora` (no subcommand) launch
//! through [`run_workspace`] so the interactive entrypoint cannot drift.

pub mod actions;
mod agent_handoff;
mod app;
mod conversation;
mod effects;
mod error_view;
pub mod intent;
mod launch;
mod persistence;
mod smoke;
mod terminal;
mod ui;

use std::sync::Arc;

pub use launch::{open_capabilities, WorkspaceLaunchConfig};
pub use terminal::is_interactive_terminal;

/// Test-only surface exposing Workspace internals so integration tests can
/// assert the async / confirmation / terminal contracts without leaking
/// impl details into the stable public API. `#[doc(hidden)]` keeps it out of
/// generated docs; production code must not depend on it.
#[doc(hidden)]
pub mod testing {
    pub use crate::app::state::{
        ActiveInvestigationState, CommandPaletteState, ComposerMode, Notification,
        NotificationKind, WorkspaceApp, WorkspaceFocus, WorkspaceModal,
    };
    pub use crate::app::update::{
        apply_result, cancel_pending_for_test, confirm_pending_for_test, handle_key,
        request_confirmation_for_test,
    };
    pub use crate::conversation::{MessageContent, WorkspaceMessage};
    pub use crate::effects::{
        busy_label_for, set_test_task_delay_ms, CancellationOutcome, TaskId, TaskManager,
        TaskStatus, WorkspaceTask,
    };
    pub use crate::intent::{
        CancellationPolicy, IntentExecutionMode, IntentExecutionResult, WorkspaceIntent,
        WorkspaceRoute,
    };
    pub use crate::terminal::TerminalGuard;
}

use app::run_unified_workspace;
use smoke::smoke_workflow;

/// Shared Workspace entrypoint for `rivora` and `rivora-workspace`.
pub fn run_workspace(config: WorkspaceLaunchConfig) -> Result<(), String> {
    let caps = open_capabilities(&config.data_dir)?;
    if config.smoke {
        return smoke_workflow(&caps);
    }
    ensure_interactive_terminal()?;
    run_unified_workspace(Arc::new(caps), config.data_dir)
}

fn ensure_interactive_terminal() -> Result<(), String> {
    if is_interactive_terminal() {
        return Ok(());
    }
    Err("interactive Workspace requires a terminal (TTY). \
         Use a CLI subcommand for non-interactive environments \
         (for example: `rivora --help` or `rivora doctor health`)."
        .to_string())
}

/// Map a displayable error into a Workspace string (CLI compatibility).
pub fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[cfg(test)]
mod architecture_tests {
    /// Workspace crate must not implement Runtime reasoning engines.
    #[test]
    fn workspace_stays_thin_over_capabilities() {
        let lib = include_str!("lib.rs");
        assert!(
            lib.contains("CapabilityService"),
            "Workspace must route through CapabilityService"
        );
        // lib root is the shared launcher only; CapabilityService is the boundary.
        assert!(
            lib.contains("run_unified_workspace") || lib.contains("smoke_workflow"),
            "lib root must launch unified workspace or smoke"
        );
    }
}
