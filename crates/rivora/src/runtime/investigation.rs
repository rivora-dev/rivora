//! Investigation Manager (RFC-005, RFC-013).

use crate::domain::{Investigation, InvestigationId, InvestigationStatus, Provenance};
use crate::error::{RivoraError, RivoraResult};
use crate::runtime::Runtime;

impl Runtime {
    /// Create and persist a new Investigation.
    pub fn create_investigation(
        &self,
        title: impl Into<String>,
        description: Option<String>,
        actor: impl Into<String>,
    ) -> RivoraResult<Investigation> {
        let provenance = Provenance::now(actor, "runtime").with_capability("create_investigation");
        let investigation = Investigation::create(title, description, provenance)?;
        self.store.save_investigation(&investigation)?;
        Ok(investigation)
    }

    /// Load an Investigation by id.
    pub fn open_investigation(&self, id: InvestigationId) -> RivoraResult<Investigation> {
        self.store.load_investigation(&id)
    }

    /// List known Investigation ids.
    pub fn list_investigations(&self) -> RivoraResult<Vec<InvestigationId>> {
        self.store.list_investigations()
    }

    /// Advance Investigation lifecycle by one valid step.
    pub fn advance_investigation(
        &self,
        id: InvestigationId,
        reason: Option<String>,
    ) -> RivoraResult<Investigation> {
        let mut inv = self.store.load_investigation(&id)?;
        inv.advance(reason)?;
        self.store.save_investigation(&inv)?;
        Ok(inv)
    }

    /// Transition Investigation to an explicit status when allowed.
    pub fn transition_investigation(
        &self,
        id: InvestigationId,
        to: InvestigationStatus,
        reason: Option<String>,
    ) -> RivoraResult<Investigation> {
        let mut inv = self.store.load_investigation(&id)?;
        inv.transition_to(to, reason)?;
        self.store.save_investigation(&inv)?;
        Ok(inv)
    }

    /// Complete an Investigation (must currently be in Learning).
    pub fn complete_investigation(
        &self,
        id: InvestigationId,
        reason: Option<String>,
    ) -> RivoraResult<Investigation> {
        let mut inv = self.store.load_investigation(&id)?;
        if inv.status != InvestigationStatus::Learning {
            return Err(RivoraError::OperationNotAllowed {
                status: inv.status,
                message: "complete requires Learning status".into(),
            });
        }
        inv.complete(reason)?;
        self.store.save_investigation(&inv)?;
        Ok(inv)
    }

    /// Reopen a completed Investigation into Collecting.
    pub fn reopen_investigation(
        &self,
        id: InvestigationId,
        reason: Option<String>,
    ) -> RivoraResult<Investigation> {
        let mut inv = self.store.load_investigation(&id)?;
        if inv.status != InvestigationStatus::Completed {
            return Err(RivoraError::OperationNotAllowed {
                status: inv.status,
                message: "only completed investigations can be reopened".into(),
            });
        }
        inv.reopen(reason)?;
        self.store.save_investigation(&inv)?;
        Ok(inv)
    }

    /// Ensure Investigation is at least in the given status (advance as needed).
    pub(crate) fn ensure_status_at_least(
        &self,
        id: InvestigationId,
        target: InvestigationStatus,
    ) -> RivoraResult<Investigation> {
        let mut inv = self.store.load_investigation(&id)?;
        if inv.status == InvestigationStatus::Completed {
            return Err(RivoraError::OperationNotAllowed {
                status: inv.status,
                message: "investigation is completed; reopen before continuing".into(),
            });
        }

        // Advance along the path until we reach target or pass it.
        let order = [
            InvestigationStatus::Created,
            InvestigationStatus::Collecting,
            InvestigationStatus::Understanding,
            InvestigationStatus::Evaluating,
            InvestigationStatus::Verifying,
            InvestigationStatus::Recommending,
            InvestigationStatus::Learning,
            InvestigationStatus::Completed,
        ];
        let target_idx = order.iter().position(|s| *s == target).unwrap_or(0);
        let mut current_idx = order.iter().position(|s| *s == inv.status).unwrap_or(0);

        while current_idx < target_idx {
            inv.advance(Some(format!("auto-advance toward {target}")))?;
            current_idx += 1;
        }
        self.store.save_investigation(&inv)?;
        Ok(inv)
    }
}
