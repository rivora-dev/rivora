//! Explicit Workspace application state.

use std::path::PathBuf;
use std::sync::Arc;

use rivora::domain::{InvestigationId, ObjectId};
use rivora::CapabilityService;

use crate::actions::{ActionAvailability, WorkspaceActionDescriptor};
use crate::conversation::ConversationState;
use crate::effects::TaskManager;
use crate::intent::execute::InvestigationListItem;
use crate::intent::{WorkspaceIntent, WorkspaceRoute};
use crate::persistence::{self, WorkspaceUiState};

/// Which region has keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WorkspaceFocus {
    Context,
    #[default]
    Composer,
    Conversation,
    Inspector,
    List,
}

/// Composer interaction mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ComposerMode {
    #[default]
    Prompt,
    Slash,
    Confirm,
    Busy,
}

/// Composer input state.
#[derive(Debug, Clone, Default)]
pub struct ComposerState {
    pub input: String,
    pub cursor: usize,
    pub mode: ComposerMode,
    pub history: Vec<String>,
    pub history_index: Option<usize>,
}

impl ComposerState {
    pub const MAX_LEN: usize = 8_192;

    pub fn insert(&mut self, ch: char) {
        if self.input.chars().count() >= Self::MAX_LEN {
            return;
        }
        if ch.is_control() && ch != '\n' {
            return;
        }
        let idx = self.byte_index();
        self.input.insert(idx, ch);
        self.cursor += 1;
        if self.input.starts_with('/') && self.mode == ComposerMode::Prompt {
            self.mode = ComposerMode::Slash;
        }
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let idx = self.byte_index_before();
        self.input.remove(idx);
        self.cursor -= 1;
        if self.input.is_empty() {
            self.mode = ComposerMode::Prompt;
        }
    }

    pub fn clear(&mut self) {
        self.input.clear();
        self.cursor = 0;
        if !matches!(self.mode, ComposerMode::Busy | ComposerMode::Confirm) {
            self.mode = ComposerMode::Prompt;
        }
    }

    pub fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_right(&mut self) {
        let len = self.input.chars().count();
        if self.cursor < len {
            self.cursor += 1;
        }
    }

    pub fn display_with_cursor(&self) -> String {
        let mut out = String::new();
        for (i, ch) in self.input.chars().enumerate() {
            if i == self.cursor {
                out.push('▌');
            }
            out.push(ch);
        }
        if self.cursor >= self.input.chars().count() {
            out.push('▌');
        }
        out
    }

    fn byte_index(&self) -> usize {
        self.input
            .char_indices()
            .nth(self.cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.input.len())
    }

    fn byte_index_before(&self) -> usize {
        self.input
            .char_indices()
            .nth(self.cursor - 1)
            .map(|(i, _)| i)
            .unwrap_or(0)
    }
}

/// Command palette (`/` or Ctrl+P).
#[derive(Debug, Clone, Default)]
pub struct CommandPaletteState {
    pub open: bool,
    pub global: bool,
    pub filter: String,
    pub selected: usize,
    pub filtered: Vec<(WorkspaceActionDescriptor, ActionAvailability)>,
}

/// Active investigation projection for the UI.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ActiveInvestigationState {
    pub id: InvestigationId,
    pub title: String,
    pub status: String,
}

/// Notification toast.
#[derive(Debug, Clone)]
pub struct Notification {
    pub kind: NotificationKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationKind {
    Success,
    Info,
    Warning,
    Error,
    Progress,
}

/// Modal overlay.
#[derive(Debug, Clone)]
pub enum WorkspaceModal {
    Confirm {
        title: String,
        body: String,
        pending: WorkspaceIntent,
    },
    Help,
    Error {
        title: String,
        body: String,
    },
}

/// Full application state.
pub struct WorkspaceApp {
    pub caps: Arc<CapabilityService>,
    pub data_dir: PathBuf,
    pub route: WorkspaceRoute,
    pub focus: WorkspaceFocus,
    pub composer: ComposerState,
    pub conversation: ConversationState,
    pub palette: CommandPaletteState,
    pub recent_investigations: Vec<InvestigationListItem>,
    pub list_items: Vec<InvestigationListItem>,
    pub list_selected: usize,
    pub context_selected: usize,
    pub active_investigation: Option<ActiveInvestigationState>,
    pub selected_proposal_id: Option<ObjectId>,
    pub selected_plan_id: Option<ObjectId>,
    pub inspector_text: String,
    pub inspector_visible: bool,
    pub panel_title: String,
    pub panel_lines: Vec<String>,
    pub notifications: Vec<Notification>,
    pub tasks: TaskManager,
    pub modal: Option<WorkspaceModal>,
    pub pending_intent: Option<WorkspaceIntent>,
    pub should_quit: bool,
    pub runtime_healthy: bool,
    pub ui_state: WorkspaceUiState,
}

impl WorkspaceApp {
    pub fn bootstrap(caps: Arc<CapabilityService>, data_dir: PathBuf) -> Result<Self, String> {
        let ui_state = persistence::load_ui_state(&data_dir);
        let runtime_healthy = caps.store_health().is_ok();
        let mut app = Self {
            caps,
            data_dir,
            route: WorkspaceRoute::Home,
            focus: WorkspaceFocus::Composer,
            composer: ComposerState {
                history: ui_state.command_history.clone(),
                ..ComposerState::default()
            },
            conversation: ConversationState::default(),
            palette: CommandPaletteState::default(),
            recent_investigations: Vec::new(),
            list_items: Vec::new(),
            list_selected: 0,
            context_selected: 0,
            active_investigation: None,
            selected_proposal_id: None,
            selected_plan_id: None,
            inspector_text: String::new(),
            inspector_visible: ui_state.inspector_visible,
            panel_title: String::new(),
            panel_lines: Vec::new(),
            notifications: Vec::new(),
            tasks: TaskManager::new(),
            modal: None,
            pending_intent: None,
            should_quit: false,
            runtime_healthy,
            ui_state,
        };
        app.refresh_recent();
        // Restore last active investigation when possible.
        if let Some(id_str) = app.ui_state.last_active_investigation_id.clone() {
            if let Ok(id) = id_str.parse::<InvestigationId>() {
                if let Ok(inv) = app.caps.open_investigation(id) {
                    app.set_active_investigation(inv.id, inv.title, inv.status.as_str());
                }
            }
        }
        Ok(app)
    }

    pub fn route_label(&self) -> &'static str {
        match self.route {
            WorkspaceRoute::Home => "Home",
            WorkspaceRoute::Investigation => "Investigation",
            WorkspaceRoute::Search => "Search",
            WorkspaceRoute::ProposalReview => "Proposals",
            WorkspaceRoute::ExecutionReview => "Execution",
            WorkspaceRoute::Connectors => "Connectors",
            WorkspaceRoute::Doctor => "Doctor",
            WorkspaceRoute::Learning => "Learning",
            WorkspaceRoute::Settings => "Settings",
            WorkspaceRoute::Help => "Help",
        }
    }

    pub fn notify(&mut self, kind: NotificationKind, text: impl Into<String>) {
        self.notifications.push(Notification {
            kind,
            text: text.into(),
        });
        if self.notifications.len() > 8 {
            let drain = self.notifications.len() - 8;
            self.notifications.drain(0..drain);
        }
    }

    pub fn set_active_investigation(&mut self, id: InvestigationId, title: String, status: &str) {
        self.active_investigation = Some(ActiveInvestigationState {
            id,
            title: title.clone(),
            status: status.to_string(),
        });
        self.ui_state.last_active_investigation_id = Some(id.to_string());
        let id_str = id.to_string();
        self.ui_state
            .recent_investigation_ids
            .retain(|x| x != &id_str);
        self.ui_state.recent_investigation_ids.insert(0, id_str);
        self.inspector_text = format!(
            "Investigation\n{title}\nStatus: {status}\nId: {id}\n\nTimeline, evidence, and loop artifacts load through Capabilities."
        );
        self.refresh_recent();
        self.load_timeline_into_inspector(id);
    }

    pub fn load_timeline_into_inspector(&mut self, id: InvestigationId) {
        if let Ok(entries) = self.caps.generate_timeline(id) {
            let mut text = self.inspector_text.clone();
            text.push_str("\n\nTimeline:\n");
            for e in entries.iter().take(20) {
                text.push_str(&format!("• {}\n", e.summary));
            }
            self.inspector_text = text;
        }
    }

    pub fn refresh_recent(&mut self) {
        let ids = self.caps.list_investigations().unwrap_or_default();
        let mut items = Vec::new();
        for id in ids.into_iter().take(30) {
            if let Ok(inv) = self.caps.open_investigation(id) {
                items.push(InvestigationListItem {
                    id: inv.id,
                    title: inv.title,
                    status: inv.status.as_str().to_string(),
                    updated_at: inv.updated_at.to_rfc3339(),
                    score: None,
                });
            }
        }
        items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        self.recent_investigations = items;
    }

    pub fn persist(&self) {
        let mut state = self.ui_state.clone();
        state.command_history = self.composer.history.clone();
        state.inspector_visible = self.inspector_visible;
        state.version = 1;
        persistence::save_ui_state(&self.data_dir, &state);
    }

    pub fn active_id(&self) -> Option<InvestigationId> {
        self.active_investigation.as_ref().map(|i| i.id)
    }

    pub fn poll_background(&mut self) {
        if let Some(msg) = self.tasks.poll() {
            match msg.result {
                Ok(result) => crate::app::update::apply_result(self, result),
                Err(e) => {
                    self.composer.mode = ComposerMode::Prompt;
                    self.notify(NotificationKind::Error, e.clone());
                    self.conversation
                        .push(crate::conversation::WorkspaceMessage::error(
                            "Task failed",
                            e,
                        ));
                }
            }
        }
        if !self.tasks.is_busy() && matches!(self.composer.mode, ComposerMode::Busy) {
            self.composer.mode = ComposerMode::Prompt;
        }
    }
}
