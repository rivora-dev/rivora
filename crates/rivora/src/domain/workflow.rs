//! Assisted Workflows — durable Composite Capability executions (RFC-018).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{empty_metadata, InvestigationId, Metadata, ObjectId, Provenance};

/// Overall status of an Assisted Workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStatus {
    /// Plan created; no steps executed yet.
    Planned,
    /// At least one step is running or ready to continue.
    Running,
    /// All steps finished successfully.
    Completed,
    /// Some steps finished; others failed or await confirmation.
    PartiallyCompleted,
    /// Unrecoverable failure stopped progress.
    Failed,
    /// Explicitly cancelled by a human or interface.
    Cancelled,
}

impl WorkflowStatus {
    /// Stable string form.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::PartiallyCompleted => "partially_completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

/// Status of a single workflow step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStepStatus {
    /// Not yet started.
    Planned,
    /// Currently executing.
    Running,
    /// Finished successfully.
    Completed,
    /// Failed; may be retriable.
    Failed,
    /// Skipped with an explanation.
    Skipped,
    /// Cancelled before execution.
    Cancelled,
}

impl WorkflowStepStatus {
    /// Stable string form.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
            Self::Cancelled => "cancelled",
        }
    }
}

/// One step in an Assisted Workflow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowStep {
    /// Zero-based index in the plan.
    pub index: u32,
    /// Stable step slug within the composite (e.g. `recall_memory`).
    pub step_id: String,
    /// Core Capability invoked.
    pub capability: String,
    /// Human-readable intent of the step.
    pub description: String,
    /// Step status.
    pub status: WorkflowStepStatus,
    /// Whether human confirmation is required before execution.
    pub confirmation_required: bool,
    /// Whether confirmation has been granted.
    pub confirmation_granted: bool,
    /// Object ids produced by this step.
    pub output_refs: Vec<ObjectId>,
    /// Evidence object ids consulted.
    pub evidence_refs: Vec<ObjectId>,
    /// Structured notes (counts, summaries, decision reasons).
    pub notes: String,
    /// Failure details when status is Failed.
    pub failure: Option<String>,
    /// Skip reason when status is Skipped.
    pub skip_reason: Option<String>,
    /// When the step started.
    pub started_at: Option<DateTime<Utc>>,
    /// When the step finished.
    pub completed_at: Option<DateTime<Utc>>,
}

impl WorkflowStep {
    /// Create a planned step.
    pub fn planned(
        index: u32,
        step_id: impl Into<String>,
        capability: impl Into<String>,
        description: impl Into<String>,
        confirmation_required: bool,
    ) -> Self {
        Self {
            index,
            step_id: step_id.into(),
            capability: capability.into(),
            description: description.into(),
            status: WorkflowStepStatus::Planned,
            confirmation_required,
            confirmation_granted: false,
            output_refs: Vec::new(),
            evidence_refs: Vec::new(),
            notes: String::new(),
            failure: None,
            skip_reason: None,
            started_at: None,
            completed_at: None,
        }
    }
}

/// Durable Assisted Workflow execution record (RFC-018).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistedWorkflow {
    /// Stable workflow identifier.
    pub id: ObjectId,
    /// Primary Investigation.
    pub investigation_id: InvestigationId,
    /// Composite Capability slug (intent).
    pub intent: String,
    /// Human-readable intent description.
    pub intent_description: String,
    /// Overall status.
    pub status: WorkflowStatus,
    /// Ordered steps.
    pub steps: Vec<WorkflowStep>,
    /// Final summary after completion or partial success.
    pub summary: Option<String>,
    /// Cancellation reason when cancelled.
    pub cancellation_reason: Option<String>,
    /// When the workflow was planned.
    pub planned_at: DateTime<Utc>,
    /// When execution started.
    pub started_at: Option<DateTime<Utc>>,
    /// When the workflow reached a terminal or partial terminal state.
    pub completed_at: Option<DateTime<Utc>>,
    /// Provenance.
    pub provenance: Provenance,
    /// Metadata.
    pub metadata: Metadata,
}

impl AssistedWorkflow {
    /// Construct a planned workflow.
    pub fn planned(
        investigation_id: InvestigationId,
        intent: impl Into<String>,
        intent_description: impl Into<String>,
        steps: Vec<WorkflowStep>,
        provenance: Provenance,
    ) -> Self {
        Self {
            id: ObjectId::new(),
            investigation_id,
            intent: intent.into(),
            intent_description: intent_description.into(),
            status: WorkflowStatus::Planned,
            steps,
            summary: None,
            cancellation_reason: None,
            planned_at: Utc::now(),
            started_at: None,
            completed_at: None,
            provenance,
            metadata: empty_metadata(),
        }
    }
}

/// Catalog entry describing a Composite Capability definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompositeCapabilityDefinition {
    /// Stable slug.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Intent description.
    pub description: String,
    /// Core Capability slugs coordinated, in order.
    pub core_capabilities: Vec<String>,
}
