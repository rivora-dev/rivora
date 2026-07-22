//! Observation — immutable engineering event (RFC-004).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{RivoraError, RivoraResult};

use super::{empty_metadata, Confidence, InvestigationId, Metadata, ObjectId, Provenance};

/// Kind of engineering observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationKind {
    /// Generic event.
    Event,
    /// Repository metadata.
    Repository,
    /// Git commit.
    Commit,
    /// Git branch or status.
    GitStatus,
    /// Changed files snapshot.
    ChangedFiles,
    /// Pull request metadata.
    PullRequest,
    /// CI / workflow check result.
    CheckResult,
    /// Test output.
    TestOutput,
    /// Linked issue.
    Issue,
    /// User-supplied note or file event.
    UserInput,
    /// Local structured event file.
    LocalEvent,
    /// CI / CD workflow run.
    WorkflowRun,
    /// Infrastructure resource or deployment state.
    Infrastructure,
    /// Observability error, alert, or anomaly.
    Observability,
    /// Other / extension kind.
    Other(String),
}

impl ObservationKind {
    /// Display name.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Event => "event",
            Self::Repository => "repository",
            Self::Commit => "commit",
            Self::GitStatus => "git_status",
            Self::ChangedFiles => "changed_files",
            Self::PullRequest => "pull_request",
            Self::CheckResult => "check_result",
            Self::TestOutput => "test_output",
            Self::Issue => "issue",
            Self::UserInput => "user_input",
            Self::LocalEvent => "local_event",
            Self::WorkflowRun => "workflow_run",
            Self::Infrastructure => "infrastructure",
            Self::Observability => "observability",
            Self::Other(s) => s.as_str(),
        }
    }
}

/// Immutable recorded engineering event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    /// Stable object identifier.
    pub id: ObjectId,
    /// Primary Investigation this Observation belongs to.
    pub investigation_id: InvestigationId,
    /// Classification of the event.
    pub kind: ObservationKind,
    /// Short human-readable summary.
    pub summary: String,
    /// Structured payload (normalized source data).
    pub payload: serde_json::Value,
    /// Source system (e.g. `local`, `github`, `cli`).
    pub source: String,
    /// When the event occurred in the source system.
    pub observed_at: DateTime<Utc>,
    /// Optional key for idempotent ingestion.
    pub idempotency_key: Option<String>,
    /// Confidence that the observation is accurate.
    pub confidence: Confidence,
    /// Provenance.
    pub provenance: Provenance,
    /// Free-form metadata.
    pub metadata: Metadata,
}

impl Observation {
    /// Build a validated Observation.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        investigation_id: InvestigationId,
        kind: ObservationKind,
        summary: impl Into<String>,
        payload: serde_json::Value,
        source: impl Into<String>,
        observed_at: DateTime<Utc>,
        idempotency_key: Option<String>,
        provenance: Provenance,
    ) -> RivoraResult<Self> {
        let summary = summary.into().trim().to_string();
        if summary.is_empty() {
            return Err(RivoraError::validation(
                "observation summary must not be empty",
            ));
        }
        let source = source.into().trim().to_string();
        if source.is_empty() {
            return Err(RivoraError::validation(
                "observation source must not be empty",
            ));
        }
        Ok(Self {
            id: ObjectId::new(),
            investigation_id,
            kind,
            summary,
            payload,
            source,
            observed_at,
            idempotency_key,
            confidence: Confidence::certain(),
            provenance,
            metadata: empty_metadata(),
        })
    }
}
