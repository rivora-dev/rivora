//! Background task model — keep the render loop non-blocking.
#![allow(dead_code)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use rivora::CapabilityService;

use crate::intent::{execute_intent, IntentExecutionResult, WorkspaceIntent};

static TASK_SEQ: AtomicU64 = AtomicU64::new(1);

/// Stable task identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(u64);

impl TaskId {
    fn next() -> Self {
        Self(TASK_SEQ.fetch_add(1, Ordering::SeqCst))
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "task-{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// In-flight or finished Workspace task.
#[derive(Debug)]
pub struct WorkspaceTask {
    pub id: TaskId,
    pub kind: String,
    pub status: TaskStatus,
    pub started: Instant,
    pub generation: u64,
}

/// Message from a background worker.
pub struct TaskMessage {
    pub id: TaskId,
    pub generation: u64,
    pub result: Result<IntentExecutionResult, String>,
}

/// Manages background Capability work.
pub struct TaskManager {
    tx: Sender<TaskMessage>,
    rx: Receiver<TaskMessage>,
    pub active: Option<WorkspaceTask>,
    /// Bumped when context changes so stale results are dropped.
    pub generation: u64,
}

impl TaskManager {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx,
            active: None,
            generation: 0,
        }
    }

    pub fn is_busy(&self) -> bool {
        matches!(
            self.active.as_ref().map(|t| t.status),
            Some(TaskStatus::Running)
        )
    }

    pub fn bump_generation(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    /// Spawn intent execution on a background thread.
    pub fn spawn_intent(&mut self, caps: Arc<CapabilityService>, intent: WorkspaceIntent) {
        let id = TaskId::next();
        let generation = self.generation;
        let kind = format!("{intent:?}");
        self.active = Some(WorkspaceTask {
            id,
            kind: kind.chars().take(64).collect(),
            status: TaskStatus::Running,
            started: Instant::now(),
            generation,
        });
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                execute_intent(&caps, &intent)
            }));
            let mapped = match result {
                Ok(r) => Ok(r),
                Err(_) => Err("Background task panicked".into()),
            };
            let _ = tx.send(TaskMessage {
                id,
                generation,
                result: mapped,
            });
        });
    }

    /// Non-blocking poll for completed tasks.
    pub fn poll(&mut self) -> Option<TaskMessage> {
        match self.rx.try_recv() {
            Ok(msg) => {
                if let Some(active) = &mut self.active {
                    if active.id == msg.id {
                        active.status = if msg.result.is_ok() {
                            TaskStatus::Completed
                        } else {
                            TaskStatus::Failed
                        };
                    }
                }
                // Drop stale generations.
                if msg.generation != self.generation {
                    return None;
                }
                Some(msg)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => None,
        }
    }

    pub fn cancel_active(&mut self) {
        self.bump_generation();
        if let Some(active) = &mut self.active {
            active.status = TaskStatus::Cancelled;
        }
    }
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_generation_dropped() {
        let mut tm = TaskManager::new();
        tm.generation = 1;
        let (tx, rx) = mpsc::channel();
        tm.tx = tx;
        tm.rx = rx;
        // Simulate completion with old generation.
        tm.tx
            .send(TaskMessage {
                id: TaskId(1),
                generation: 0,
                result: Ok(IntentExecutionResult::Quit),
            })
            .unwrap();
        assert!(tm.poll().is_none());
    }
}
