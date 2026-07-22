//! Rivora Runtime — single source of engineering reasoning (RFC-005, RFC-014).

mod evaluation;
mod investigation;
mod knowledge;
/// Learning subsystem request types and Runtime methods.
pub mod learning;
mod memory;
/// Observation ingestion request types and Runtime methods.
pub mod observation;
mod recommendation;
mod verification;

use std::sync::Arc;

use crate::storage::Store;

/// Rivora Runtime entry point.
///
/// Interfaces never bypass the Runtime. All engineering reasoning lives here.
#[derive(Clone)]
pub struct Runtime {
    store: Arc<dyn Store>,
}

impl Runtime {
    /// Create a Runtime backed by the given store.
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self { store }
    }

    /// Access the underlying store (for read-only inspection in tests).
    pub fn store(&self) -> &Arc<dyn Store> {
        &self.store
    }
}

impl std::fmt::Debug for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runtime").finish_non_exhaustive()
    }
}
