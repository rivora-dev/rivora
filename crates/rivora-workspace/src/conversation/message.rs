//! Typed conversation messages — projection, not domain state.
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::intent::WorkspaceIntent;

/// Message identifier (Workspace-local, not an Engineering Object id).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkspaceMessageId(Uuid);

impl WorkspaceMessageId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for WorkspaceMessageId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for WorkspaceMessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceMessageRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageStatus {
    Pending,
    Complete,
    Failed,
    Cancelled,
}

/// Message content variants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageContent {
    Text { text: String },
    ObjectCard { title: String, body: String },
    Confirmation { title: String, body: String },
    Progress { text: String },
    Warning { text: String },
    Error { title: String, summary: String },
}

/// Conversation message projecting typed operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceMessage {
    pub id: WorkspaceMessageId,
    pub role: WorkspaceMessageRole,
    pub content: MessageContent,
    pub created_at: DateTime<Utc>,
    pub references: Vec<String>,
    pub intent_summary: Option<String>,
    pub status: MessageStatus,
}

impl WorkspaceMessage {
    pub fn user_text(text: impl Into<String>) -> Self {
        Self {
            id: WorkspaceMessageId::new(),
            role: WorkspaceMessageRole::User,
            content: MessageContent::Text {
                text: sanitize(text.into()),
            },
            created_at: Utc::now(),
            references: vec![],
            intent_summary: None,
            status: MessageStatus::Complete,
        }
    }

    pub fn assistant_text(text: impl Into<String>) -> Self {
        Self {
            id: WorkspaceMessageId::new(),
            role: WorkspaceMessageRole::Assistant,
            content: MessageContent::Text {
                text: sanitize(text.into()),
            },
            created_at: Utc::now(),
            references: vec![],
            intent_summary: None,
            status: MessageStatus::Complete,
        }
    }

    pub fn assistant_card(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            id: WorkspaceMessageId::new(),
            role: WorkspaceMessageRole::Assistant,
            content: MessageContent::ObjectCard {
                title: sanitize(title.into()),
                body: sanitize(body.into()),
            },
            created_at: Utc::now(),
            references: vec![],
            intent_summary: None,
            status: MessageStatus::Complete,
        }
    }

    pub fn confirmation(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            id: WorkspaceMessageId::new(),
            role: WorkspaceMessageRole::Assistant,
            content: MessageContent::Confirmation {
                title: sanitize(title.into()),
                body: sanitize(body.into()),
            },
            created_at: Utc::now(),
            references: vec![],
            intent_summary: None,
            status: MessageStatus::Pending,
        }
    }

    pub fn error(title: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            id: WorkspaceMessageId::new(),
            role: WorkspaceMessageRole::System,
            content: MessageContent::Error {
                title: sanitize(title.into()),
                summary: sanitize(summary.into()),
            },
            created_at: Utc::now(),
            references: vec![],
            intent_summary: None,
            status: MessageStatus::Failed,
        }
    }

    pub fn with_refs(mut self, refs: Vec<String>) -> Self {
        self.references = refs;
        self
    }

    pub fn with_intent(mut self, intent: &WorkspaceIntent) -> Self {
        self.intent_summary = Some(format!("{intent:?}"));
        self
    }

    /// Flatten to display lines for the conversation pane.
    pub fn lines(&self) -> Vec<String> {
        let prefix = match self.role {
            WorkspaceMessageRole::User => "You",
            WorkspaceMessageRole::Assistant => "Rivora",
            WorkspaceMessageRole::System => "System",
        };
        match &self.content {
            MessageContent::Text { text } => text
                .lines()
                .enumerate()
                .map(|(i, line)| {
                    if i == 0 {
                        format!("{prefix}: {line}")
                    } else {
                        format!("  {line}")
                    }
                })
                .collect(),
            MessageContent::ObjectCard { title, body } => {
                let mut out = vec![format!("{prefix}: [{title}]")];
                for line in body.lines() {
                    out.push(format!("  {line}"));
                }
                if !self.references.is_empty() {
                    out.push(format!("  refs: {}", self.references.join(", ")));
                }
                out
            }
            MessageContent::Confirmation { title, body } => {
                let mut out = vec![
                    format!("{prefix}: {title}"),
                    "  Confirm? [y] yes  [n] cancel".into(),
                ];
                for line in body.lines() {
                    out.push(format!("  {line}"));
                }
                out
            }
            MessageContent::Progress { text } => vec![format!("{prefix}: … {text}")],
            MessageContent::Warning { text } => vec![format!("{prefix}: ! {text}")],
            MessageContent::Error { title, summary } => {
                vec![format!("{prefix}: {title}"), format!("  {summary}")]
            }
        }
    }
}

/// Conversation state held by the Workspace application.
#[derive(Debug, Clone, Default)]
pub struct ConversationState {
    pub messages: Vec<WorkspaceMessage>,
    pub scroll: u16,
}

impl ConversationState {
    pub fn push(&mut self, msg: WorkspaceMessage) {
        self.messages.push(msg);
        // Keep a bounded projection in memory.
        const MAX: usize = 500;
        if self.messages.len() > MAX {
            let drain = self.messages.len() - MAX;
            self.messages.drain(0..drain);
        }
    }

    pub fn clear_scroll_to_end(&mut self) {
        self.scroll = 0;
    }
}

fn sanitize(s: String) -> String {
    s.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_control_chars() {
        let m = WorkspaceMessage::user_text("hello\u{0007}world");
        if let MessageContent::Text { text } = m.content {
            assert!(!text.contains('\u{0007}'));
            assert!(text.contains("helloworld") || text.contains("hello"));
        } else {
            panic!("expected text");
        }
    }
}
