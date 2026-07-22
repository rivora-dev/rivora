//! Investigation lifecycle states and transition rules (RFC-013).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::{RivoraError, RivoraResult};

use super::InvestigationId;

/// Investigation lifecycle status for v0.1.
///
/// Canonical progression:
/// ```text
/// Created → Collecting → Understanding → Evaluating → Verifying
/// → Recommending → Learning → Completed
/// ```
///
/// Completed Investigations may reopen into Collecting when new
/// Observations arrive. History is preserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvestigationStatus {
    /// Newly created Investigation with no activity yet.
    Created,
    /// Gathering Observations and Memory.
    Collecting,
    /// Deriving Knowledge from Memory.
    Understanding,
    /// Assessing significance of Knowledge.
    Evaluating,
    /// Validating conclusions with evidence.
    Verifying,
    /// Generating Recommendations.
    Recommending,
    /// Recording outcomes and learning.
    Learning,
    /// Engineering objective satisfied; history retained.
    Completed,
}

impl InvestigationStatus {
    /// Human-readable name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Collecting => "collecting",
            Self::Understanding => "understanding",
            Self::Evaluating => "evaluating",
            Self::Verifying => "verifying",
            Self::Recommending => "recommending",
            Self::Learning => "learning",
            Self::Completed => "completed",
        }
    }

    /// Whether the Investigation is open for reasoning work.
    pub fn is_active(self) -> bool {
        !matches!(self, Self::Completed)
    }

    /// Valid forward transitions from this status.
    pub fn valid_next(self) -> &'static [InvestigationStatus] {
        use InvestigationStatus::*;
        match self {
            Created => &[Collecting],
            Collecting => &[Understanding],
            Understanding => &[Evaluating],
            Evaluating => &[Verifying],
            Verifying => &[Recommending],
            Recommending => &[Learning],
            Learning => &[Completed],
            Completed => &[],
        }
    }

    /// Whether transitioning to `to` is allowed.
    ///
    /// Reopen (`Completed → Collecting`) is a special supported transition.
    pub fn can_transition_to(self, to: InvestigationStatus) -> bool {
        if self == to {
            return true;
        }
        if self == InvestigationStatus::Completed && to == InvestigationStatus::Collecting {
            return true;
        }
        // Allow advancing one step at a time.
        self.valid_next().contains(&to)
    }

    /// Validate a transition, returning an error if invalid.
    pub fn validate_transition(
        self,
        to: InvestigationStatus,
        investigation_id: InvestigationId,
    ) -> RivoraResult<()> {
        if self.can_transition_to(to) {
            Ok(())
        } else {
            Err(RivoraError::InvalidLifecycleTransition {
                investigation_id,
                from: self,
                to,
            })
        }
    }
}

impl fmt::Display for InvestigationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Record of a lifecycle transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleTransition {
    /// Status before the transition.
    pub from: InvestigationStatus,
    /// Status after the transition.
    pub to: InvestigationStatus,
    /// Optional reason.
    pub reason: Option<String>,
    /// When the transition occurred (RFC3339 string for simplicity in logs).
    pub at: chrono::DateTime<chrono::Utc>,
}

impl LifecycleTransition {
    /// Create a transition record for the current time.
    pub fn new(from: InvestigationStatus, to: InvestigationStatus, reason: Option<String>) -> Self {
        Self {
            from,
            to,
            reason,
            at: chrono::Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_forward_transitions() {
        use InvestigationStatus::*;
        let path = [
            Created,
            Collecting,
            Understanding,
            Evaluating,
            Verifying,
            Recommending,
            Learning,
            Completed,
        ];
        for window in path.windows(2) {
            assert!(
                window[0].can_transition_to(window[1]),
                "{} → {} should be valid",
                window[0],
                window[1]
            );
        }
    }

    #[test]
    fn invalid_skip_transitions_fail() {
        use InvestigationStatus::*;
        let id = InvestigationId::new();
        assert!(Created.validate_transition(Understanding, id).is_err());
        assert!(Collecting.validate_transition(Completed, id).is_err());
        assert!(Evaluating.validate_transition(Created, id).is_err());
    }

    #[test]
    fn reopen_completed_to_collecting() {
        use InvestigationStatus::*;
        let id = InvestigationId::new();
        assert!(Completed.can_transition_to(Collecting));
        assert!(Completed.validate_transition(Collecting, id).is_ok());
        assert!(Completed.validate_transition(Understanding, id).is_err());
    }

    #[test]
    fn same_status_is_allowed() {
        let status = InvestigationStatus::Collecting;
        assert!(status.can_transition_to(status));
    }
}
