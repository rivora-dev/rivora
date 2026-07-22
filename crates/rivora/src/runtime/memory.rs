//! Memory engine (RFC-006).

use crate::domain::{InvestigationId, MemoryRecord, TimelineEntry};
use crate::error::RivoraResult;
use crate::runtime::Runtime;

impl Runtime {
    /// Recall Memory for an Investigation (chronological).
    pub fn recall_memory(&self, id: InvestigationId) -> RivoraResult<Vec<MemoryRecord>> {
        // Ensure investigation exists.
        let _ = self.store.load_investigation(&id)?;
        self.store.list_memory(&id)
    }

    /// Generate a chronological Investigation timeline from Memory.
    pub fn generate_timeline(&self, id: InvestigationId) -> RivoraResult<Vec<TimelineEntry>> {
        let _ = self.store.load_investigation(&id)?;
        self.store.timeline(&id)
    }
}
