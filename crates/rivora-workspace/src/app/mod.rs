//! Unified Workspace application loop and state.

pub mod state;
pub mod update;

pub use state::WorkspaceApp;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};
use rivora::CapabilityService;

use crate::terminal::TerminalGuard;
use crate::ui;

use self::update::handle_key;

/// Run the full-screen Unified Workspace until quit.
pub fn run_unified_workspace(
    caps: Arc<CapabilityService>,
    data_dir: PathBuf,
) -> Result<(), String> {
    let mut guard = TerminalGuard::enter()?;
    let mut app = WorkspaceApp::bootstrap(caps, data_dir)?;

    loop {
        // Apply completed background tasks before drawing.
        app.poll_background();

        guard
            .terminal_mut()
            .draw(|frame| ui::draw(frame, &app))
            .map_err(|e| format!("render: {e}"))?;

        if app.should_quit {
            break;
        }

        // Poll with timeout so background task results can refresh UI.
        if event::poll(Duration::from_millis(100)).map_err(|e| format!("poll: {e}"))? {
            match event::read().map_err(|e| format!("read event: {e}"))? {
                Event::Key(key)
                    if key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat =>
                {
                    handle_key(&mut app, key)?;
                }
                Event::Resize(_, _) => {
                    // Ratatui redraws next loop; nothing else required.
                }
                _ => {}
            }
        }
    }

    app.persist();
    guard.restore();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use rivora::storage::LocalStore;
    use rivora::{MockExecutionCapability, Runtime};
    use rivora_connectors::register_first_party_github_execution_capabilities;
    use tempfile::tempdir;

    fn test_app() -> WorkspaceApp {
        let dir = tempdir().unwrap();
        let store = LocalStore::open(dir.path()).unwrap();
        let runtime = Arc::new(Runtime::new(Arc::new(store)));
        runtime
            .register_execution_capability(Arc::new(MockExecutionCapability::new()))
            .unwrap();
        register_first_party_github_execution_capabilities(runtime.execution_registry()).unwrap();
        let caps = Arc::new(CapabilityService::new(runtime));
        let path = dir.path().to_path_buf();
        std::mem::forget(dir);
        WorkspaceApp::bootstrap(caps, path).unwrap()
    }

    #[test]
    fn slash_opens_palette() {
        let mut app = test_app();
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
        )
        .unwrap();
        assert!(app.palette.open);
        assert!(!app.palette.global);
    }

    #[test]
    fn ctrl_p_opens_global_palette() {
        let mut app = test_app();
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
        )
        .unwrap();
        assert!(app.palette.open);
        assert!(app.palette.global);
    }

    #[test]
    fn plain_english_create_flow_sets_pending_confirmation() {
        let mut app = test_app();
        for ch in "Investigate why deploy failed".chars() {
            handle_key(
                &mut app,
                KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
            )
            .unwrap();
        }
        handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
        // Either confirmation modal/pending or message asking confirm.
        assert!(
            app.pending_intent.is_some()
                || app.conversation.messages.len() >= 2
                || app.modal.is_some()
        );
    }
}
