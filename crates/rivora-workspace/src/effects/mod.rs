//! Background task model — keep the render loop non-blocking.
//!
//! Slow Runtime / Capability / connector work is moved off the render thread
//! onto a background worker. The render thread continues processing input and
//! redrawing while a task is in flight. Results are delivered back over an
//! mpsc channel and applied on the main update path (`poll_background`).
//!
//! Generation counters keep stale results from overwriting newer context:
//! every task captures the generation it was spawned under, and `poll` drops
//! any message whose generation no longer matches the current context.
//! Cancellation bumps the generation so a late worker result is discarded.
//!
//! The task infra exposes a public surface (task kinds, policies, labels,
//! test seams) used across the app layer, unit tests, and integration tests;
//! the module-level dead-code allowance keeps that surface from warning when
//! an individual accessor is only exercised from tests.
#![allow(dead_code)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use rivora::CapabilityService;

use crate::intent::{execute_intent, CancellationPolicy, IntentExecutionResult, WorkspaceIntent};

static TASK_SEQ: AtomicU64 = AtomicU64::new(1);

/// Test seam: artificial delay (ms) applied by workers spawned via
/// `spawn_intent` (the production path) before they call `execute_intent`.
/// Zero by default → no production impact. Tests set it to deterministically
/// observe busy state and cancellation without racing fast local-store work.
static TEST_DELAY_MS: AtomicU64 = AtomicU64::new(0);

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
    pub policy: CancellationPolicy,
}

/// Message from a background worker.
pub struct TaskMessage {
    pub id: TaskId,
    pub generation: u64,
    pub result: Result<IntentExecutionResult, String>,
}

/// Outcome of a cancellation request, used by the app layer to show honest
/// progress text. Cancelling never silently implies the underlying operation
/// stopped or rolled back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancellationOutcome {
    /// A read task was cancelled; its result will be discarded safely.
    CancelledRead,
    /// A mutating task was cancelled. The underlying Runtime operation may
    /// already have completed or may still complete on the worker thread;
    /// only the UI result is detached. Runtime idempotency remains
    /// authoritative for any replay.
    CancelledMutationMayHaveCompleted,
    /// No running task to cancel.
    NothingRunning,
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

    /// The cancellation policy of the active task, if any.
    pub fn active_policy(&self) -> Option<CancellationPolicy> {
        self.active.as_ref().map(|t| t.policy)
    }

    /// A short, honest label for the active task kind, used in progress
    /// notifications. Derived from the intent, never a fake percentage.
    pub fn active_label(&self) -> Option<&str> {
        self.active.as_ref().map(|t| t.kind.as_str())
    }

    /// Spawn intent execution on a background thread (production path).
    ///
    /// If a task is already running, the generation is bumped first so the
    /// older worker's late result is dropped (stale-result protection).
    pub fn spawn_intent(&mut self, caps: Arc<CapabilityService>, intent: WorkspaceIntent) {
        let policy = intent.cancellation_policy();
        let kind = busy_label(&intent).to_string();
        self.spawn_internal(
            policy,
            kind,
            Box::new(move || {
                // Test seam only; no-op in production (delay stays 0).
                let delay = TEST_DELAY_MS.load(Ordering::SeqCst);
                if delay > 0 {
                    thread::sleep(Duration::from_millis(delay));
                }
                execute_intent(&caps, &intent)
            }),
        );
    }

    /// Test seam: spawn a worker with a custom operation and explicit policy.
    /// Uses the same generation/active/id machinery as `spawn_intent` so
    /// tests exercise the real task lifecycle without a live Capability call.
    pub fn spawn_op(
        &mut self,
        kind: impl Into<String>,
        policy: CancellationPolicy,
        op: Box<dyn FnOnce() -> IntentExecutionResult + Send + 'static>,
    ) {
        self.spawn_internal(policy, kind.into(), op);
    }

    fn spawn_internal(
        &mut self,
        policy: CancellationPolicy,
        kind: String,
        op: Box<dyn FnOnce() -> IntentExecutionResult + Send + 'static>,
    ) {
        // Replace any running task: bump generation so the older worker's
        // late result is dropped (stale-result protection). A finished task
        // (Completed/Failed/Cancelled) is simply overwritten.
        if matches!(
            self.active.as_ref().map(|t| t.status),
            Some(TaskStatus::Running)
        ) {
            self.bump_generation();
        }
        let id = TaskId::next();
        let generation = self.generation;
        self.active = Some(WorkspaceTask {
            id,
            kind: kind.chars().take(64).collect(),
            status: TaskStatus::Running,
            started: Instant::now(),
            generation,
            policy,
        });
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(op));
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

    /// Non-blocking poll for completed tasks. Stale generations are dropped
    /// and return `None` so they cannot overwrite newer context.
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

    /// Cancel the active task. Bumps generation so any late worker result is
    /// dropped. Returns an outcome describing what was cancelled so the UI
    /// can report it honestly — cancellation never claims the underlying
    /// mutation stopped unless that is actually true.
    pub fn cancel_active(&mut self) -> CancellationOutcome {
        let Some(active) = &mut self.active else {
            return CancellationOutcome::NothingRunning;
        };
        if active.status != TaskStatus::Running {
            return CancellationOutcome::NothingRunning;
        }
        let policy = active.policy;
        active.status = TaskStatus::Cancelled;
        // Bump generation AFTER recording cancellation so the in-flight
        // worker's result (captured under the old generation) is dropped.
        self.bump_generation();
        match policy {
            CancellationPolicy::Immediate => CancellationOutcome::CancelledRead,
            CancellationPolicy::DetachResult => {
                CancellationOutcome::CancelledMutationMayHaveCompleted
            }
        }
    }
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Public accessor for the honest, deterministic progress label of an
/// intent (never a fake percentage).
pub fn busy_label_for(intent: &WorkspaceIntent) -> &'static str {
    busy_label(intent)
}

/// Honest, deterministic progress labels for the busy indicator. Never a
/// fake percentage.
fn busy_label(intent: &WorkspaceIntent) -> &'static str {
    match intent {
        WorkspaceIntent::SearchInvestigations { .. } => "Searching…",
        WorkspaceIntent::OpenInvestigation { .. } => "Loading investigation…",
        WorkspaceIntent::ListInvestigations => "Listing investigations…",
        WorkspaceIntent::RunEvaluation { .. } => "Evaluating…",
        WorkspaceIntent::RunVerification { .. } => "Verifying…",
        WorkspaceIntent::GenerateRecommendation { .. } => "Generating recommendation…",
        WorkspaceIntent::CreateProposal { .. } => "Preparing proposal…",
        WorkspaceIntent::CreateInvestigation { .. } => "Creating investigation…",
        WorkspaceIntent::AddObservation { .. } => "Recording observation…",
        WorkspaceIntent::AgentHandoff { .. } => "Preparing agent handoff…",
        WorkspaceIntent::ShowLearning { .. } => "Loading learning…",
        WorkspaceIntent::ReviewProposals { .. } => "Loading proposals…",
        WorkspaceIntent::ReviewExecutions { .. } => "Loading executions…",
        WorkspaceIntent::ShowPriorOutcomes
        | WorkspaceIntent::ShowPatterns
        | WorkspaceIntent::ShowHistoricalTrends
        | WorkspaceIntent::ShowDoctor => "Loading…",
        WorkspaceIntent::CreateExecutionPlan { .. } => "Preparing execution plan…",
        _ => "Working…",
    }
}

/// Set the test-only artificial delay (ms) applied by `spawn_intent`
/// workers before invoking `execute_intent`. Zero in production.
pub fn set_test_task_delay_ms(ms: u64) {
    TEST_DELAY_MS.store(ms, Ordering::SeqCst);
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

    #[test]
    fn fresh_generation_applied() {
        let mut tm = TaskManager::new();
        tm.generation = 2;
        let (tx, rx) = mpsc::channel();
        tm.tx = tx;
        tm.rx = rx;
        tm.tx
            .send(TaskMessage {
                id: TaskId(1),
                generation: 2,
                result: Ok(IntentExecutionResult::Quit),
            })
            .unwrap();
        assert!(tm.poll().is_some());
    }

    #[test]
    fn cancel_active_drops_late_result() {
        set_test_task_delay_ms(0);
        let mut tm = TaskManager::new();
        let (tx, rx) = mpsc::channel();
        tm.tx = tx.clone();
        tm.rx = rx;
        // Simulate a running read task.
        tm.active = Some(WorkspaceTask {
            id: TaskId(7),
            kind: "read".into(),
            status: TaskStatus::Running,
            started: Instant::now(),
            generation: 0,
            policy: CancellationPolicy::Immediate,
        });
        let outcome = tm.cancel_active();
        assert_eq!(outcome, CancellationOutcome::CancelledRead);
        // Late worker result arrives under the old (cancelled) generation.
        tx.send(TaskMessage {
            id: TaskId(7),
            generation: 0,
            result: Ok(IntentExecutionResult::Quit),
        })
        .unwrap();
        assert!(tm.poll().is_none(), "late cancelled result must be dropped");
    }

    #[test]
    fn cancel_mutation_reports_may_have_completed() {
        let mut tm = TaskManager::new();
        tm.active = Some(WorkspaceTask {
            id: TaskId(8),
            kind: "write".into(),
            status: TaskStatus::Running,
            started: Instant::now(),
            generation: 0,
            policy: CancellationPolicy::DetachResult,
        });
        let outcome = tm.cancel_active();
        assert_eq!(
            outcome,
            CancellationOutcome::CancelledMutationMayHaveCompleted
        );
        assert!(!tm.is_busy(), "cancelled task is no longer busy");
    }

    #[test]
    fn cancel_with_no_running_task_is_nothing() {
        let mut tm = TaskManager::new();
        assert_eq!(tm.cancel_active(), CancellationOutcome::NothingRunning);
        tm.active = Some(WorkspaceTask {
            id: TaskId(9),
            kind: "done".into(),
            status: TaskStatus::Completed,
            started: Instant::now(),
            generation: 0,
            policy: CancellationPolicy::Immediate,
        });
        assert_eq!(tm.cancel_active(), CancellationOutcome::NothingRunning);
    }

    #[test]
    fn replacing_running_task_bumps_generation() {
        let mut tm = TaskManager::new();
        tm.spawn_op(
            "a",
            CancellationPolicy::Immediate,
            Box::new(|| {
                thread::sleep(Duration::from_millis(200));
                IntentExecutionResult::Quit
            }),
        );
        let first_gen = tm.generation;
        // Replace while first still running.
        tm.spawn_op(
            "b",
            CancellationPolicy::Immediate,
            Box::new(|| IntentExecutionResult::Quit),
        );
        assert_eq!(tm.generation, first_gen + 1, "generation bumped on replace");
        assert_eq!(tm.active.as_ref().unwrap().kind, "b");
    }

    #[test]
    fn fresh_replaces_stale_under_real_workers() {
        // Worker A is slow (returns last); worker B is fast (returns first).
        // B is spawned while A is running → generation bumps → A dropped.
        let mut tm = TaskManager::new();
        tm.spawn_op(
            "a-slow",
            CancellationPolicy::Immediate,
            Box::new(|| {
                thread::sleep(Duration::from_millis(120));
                IntentExecutionResult::Info {
                    title: "A".into(),
                    body: "stale".into(),
                    route: None,
                }
            }),
        );
        let gen_a = tm.active.as_ref().unwrap().generation;
        // Let B start while a-slow still running.
        tm.spawn_op(
            "b-fast",
            CancellationPolicy::Immediate,
            Box::new(|| IntentExecutionResult::Info {
                title: "B".into(),
                body: "fresh".into(),
                route: None,
            }),
        );
        let gen_b = tm.active.as_ref().unwrap().generation;
        assert_eq!(gen_b, gen_a + 1);
        // Drain until quiet; B must be applied and A dropped.
        let mut applied: Vec<&str> = Vec::new();
        let deadline = Instant::now() + Duration::from_millis(500);
        while Instant::now() < deadline {
            if let Some(msg) = tm.poll() {
                if let Ok(IntentExecutionResult::Info { title, .. }) = &msg.result {
                    applied.push(match title.as_str() {
                        "A" => "A",
                        "B" => "B",
                        _ => "?",
                    });
                }
            } else {
                thread::sleep(Duration::from_millis(5));
            }
        }
        assert!(
            applied.contains(&"B"),
            "fresh B must be applied: {applied:?}"
        );
        assert!(
            !applied.contains(&"A"),
            "stale A must be dropped: {applied:?}"
        );
    }

    #[test]
    fn stale_error_does_not_replace_success() {
        let mut tm = TaskManager::new();
        tm.spawn_op(
            "success-fast",
            CancellationPolicy::Immediate,
            Box::new(|| IntentExecutionResult::Info {
                title: "ok".into(),
                body: "".into(),
                route: None,
            }),
        );
        let gen_ok = tm.generation;
        // Now spawn a slow failing task that replaces nothing (first already done).
        // Wait for the first to finish, then start the slow-failing one and the
        // generation must advance so its error is dropped.
        thread::sleep(Duration::from_millis(20));
        let _ = tm.poll(); // apply success
        tm.spawn_op(
            "slow-err",
            CancellationPolicy::Immediate,
            Box::new(|| {
                thread::sleep(Duration::from_millis(80));
                IntentExecutionResult::Error(crate::error_view::WorkspaceErrorView {
                    title: "boom".into(),
                    summary: "stale".into(),
                    details: None,
                    code: None,
                    retry: crate::error_view::RetryGuidance::SafeToRetry,
                    actions: vec![],
                })
            }),
        );
        // immediately cancel-style bump: bump generation so the slow error is stale.
        let _ = gen_ok;
        tm.bump_generation();
        let deadline = Instant::now() + Duration::from_millis(200);
        while Instant::now() < deadline {
            if let Some(msg) = tm.poll() {
                assert!(
                    !matches!(msg.result, Ok(IntentExecutionResult::Error(..))),
                    "stale error must not surface"
                );
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn active_label_is_honest() {
        assert_eq!(
            busy_label(&WorkspaceIntent::RunEvaluation {
                investigation_id: rivora::domain::InvestigationId::new()
            }),
            "Evaluating…"
        );
        assert_eq!(
            busy_label(&WorkspaceIntent::SearchInvestigations { query: "x".into() }),
            "Searching…"
        );
    }
}
