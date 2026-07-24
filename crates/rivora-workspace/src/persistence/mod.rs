//! Versioned Workspace UI state — additive, corruption-isolated.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const STATE_VERSION: u32 = 1;
const STATE_FILE: &str = "workspace_ui_state.json";

/// Persistable Workspace preferences and recent context.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceUiState {
    pub version: u32,
    pub recent_investigation_ids: Vec<String>,
    pub last_active_investigation_id: Option<String>,
    pub command_history: Vec<String>,
    pub dismissed_onboarding: bool,
    pub inspector_visible: bool,
}

impl WorkspaceUiState {
    pub fn new() -> Self {
        Self {
            version: STATE_VERSION,
            recent_investigation_ids: Vec::new(),
            last_active_investigation_id: None,
            command_history: Vec::new(),
            dismissed_onboarding: false,
            inspector_visible: true,
        }
    }
}

/// Load UI state; missing or corrupt files yield defaults without failing Runtime.
pub fn load_ui_state(data_dir: &Path) -> WorkspaceUiState {
    let path = state_path(data_dir);
    match fs::read_to_string(&path) {
        Ok(raw) => match serde_json::from_str::<WorkspaceUiState>(&raw) {
            Ok(mut s) => {
                if s.version == 0 {
                    s.version = STATE_VERSION;
                }
                // Bound history growth.
                if s.command_history.len() > 100 {
                    let drain = s.command_history.len() - 100;
                    s.command_history.drain(0..drain);
                }
                if s.recent_investigation_ids.len() > 50 {
                    let drain = s.recent_investigation_ids.len() - 50;
                    s.recent_investigation_ids.drain(0..drain);
                }
                s
            }
            Err(_) => WorkspaceUiState::new(),
        },
        Err(_) => WorkspaceUiState::new(),
    }
}

/// Persist UI state. Failure is non-fatal for the Workspace session.
pub fn save_ui_state(data_dir: &Path, state: &WorkspaceUiState) {
    let path = state_path(data_dir);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(raw) = serde_json::to_string_pretty(state) {
        let _ = fs::write(path, raw);
    }
}

fn state_path(data_dir: &Path) -> PathBuf {
    data_dir.join(STATE_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn missing_state_is_default() {
        let dir = tempdir().unwrap();
        let s = load_ui_state(dir.path());
        assert_eq!(s.version, STATE_VERSION);
        assert!(s.recent_investigation_ids.is_empty());
    }

    #[test]
    fn corrupt_state_does_not_panic() {
        let dir = tempdir().unwrap();
        fs::write(state_path(dir.path()), "{not json").unwrap();
        let s = load_ui_state(dir.path());
        assert!(s.recent_investigation_ids.is_empty());
    }

    #[test]
    fn round_trip() {
        let dir = tempdir().unwrap();
        let mut s = WorkspaceUiState::new();
        s.command_history.push("evaluate".into());
        s.dismissed_onboarding = true;
        save_ui_state(dir.path(), &s);
        let loaded = load_ui_state(dir.path());
        assert!(loaded.dismissed_onboarding);
        assert_eq!(loaded.command_history, vec!["evaluate".to_string()]);
    }
}
