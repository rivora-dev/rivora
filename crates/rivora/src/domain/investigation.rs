//! Investigation aggregate root (RFC-004, RFC-013).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{RivoraError, RivoraResult};

use super::{
    empty_metadata, InvestigationId, InvestigationStatus, LifecycleTransition, Metadata, Provenance,
};

/// Primary unit of engineering understanding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Investigation {
    /// Stable identifier.
    pub id: InvestigationId,
    /// Human-readable title.
    pub title: String,
    /// Optional description of the engineering question.
    pub description: Option<String>,
    /// Current lifecycle status.
    pub status: InvestigationStatus,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
    /// Provenance for creation.
    pub provenance: Provenance,
    /// Free-form metadata.
    pub metadata: Metadata,
    /// Recorded lifecycle transitions (append-only history).
    pub transitions: Vec<LifecycleTransition>,
}

impl Investigation {
    /// Create a new Investigation in `Created` status.
    pub fn create(
        title: impl Into<String>,
        description: Option<String>,
        provenance: Provenance,
    ) -> RivoraResult<Self> {
        let title = title.into().trim().to_string();
        if title.is_empty() {
            return Err(RivoraError::validation(
                "investigation title must not be empty",
            ));
        }
        let now = provenance.created_at;
        Ok(Self {
            id: InvestigationId::new(),
            title,
            description,
            status: InvestigationStatus::Created,
            created_at: now,
            updated_at: now,
            provenance,
            metadata: empty_metadata(),
            transitions: Vec::new(),
        })
    }

    /// Transition to a new status when allowed.
    pub fn transition_to(
        &mut self,
        to: InvestigationStatus,
        reason: Option<String>,
    ) -> RivoraResult<()> {
        self.status.validate_transition(to, self.id)?;
        if self.status != to {
            self.transitions
                .push(LifecycleTransition::new(self.status, to, reason));
            self.status = to;
            self.updated_at = Utc::now();
        }
        Ok(())
    }

    /// Advance one step along the canonical lifecycle.
    pub fn advance(&mut self, reason: Option<String>) -> RivoraResult<InvestigationStatus> {
        let next = self.status.valid_next().first().copied().ok_or_else(|| {
            RivoraError::OperationNotAllowed {
                status: self.status,
                message: "investigation cannot advance further".into(),
            }
        })?;
        self.transition_to(next, reason)?;
        Ok(self.status)
    }

    /// Complete the Investigation (must be in Learning).
    pub fn complete(&mut self, reason: Option<String>) -> RivoraResult<()> {
        self.transition_to(InvestigationStatus::Completed, reason)
    }

    /// Reopen a completed Investigation into Collecting.
    pub fn reopen(&mut self, reason: Option<String>) -> RivoraResult<()> {
        self.transition_to(
            InvestigationStatus::Collecting,
            reason.or_else(|| Some("reopened with new observations".into())),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Investigation {
        Investigation::create(
            "CI failure on main",
            Some("Investigate recent failures".into()),
            Provenance::now("tester", "test"),
        )
        .expect("create")
    }

    #[test]
    fn create_starts_in_created() {
        let inv = sample();
        assert_eq!(inv.status, InvestigationStatus::Created);
        assert!(inv.transitions.is_empty());
    }

    #[test]
    fn empty_title_rejected() {
        let err = Investigation::create("  ", None, Provenance::now("t", "t")).unwrap_err();
        assert!(matches!(err, RivoraError::Validation(_)));
    }

    #[test]
    fn full_lifecycle_and_reopen() {
        let mut inv = sample();
        for expected in [
            InvestigationStatus::Collecting,
            InvestigationStatus::Understanding,
            InvestigationStatus::Evaluating,
            InvestigationStatus::Verifying,
            InvestigationStatus::Recommending,
            InvestigationStatus::Learning,
            InvestigationStatus::Completed,
        ] {
            inv.advance(None).unwrap();
            assert_eq!(inv.status, expected);
        }
        inv.reopen(None).unwrap();
        assert_eq!(inv.status, InvestigationStatus::Collecting);
        assert!(inv.transitions.len() >= 8);
        // History preserved
        assert_eq!(inv.transitions[0].to, InvestigationStatus::Collecting);
    }

    #[test]
    fn cannot_complete_from_created() {
        let mut inv = sample();
        assert!(inv.complete(None).is_err());
    }
}
