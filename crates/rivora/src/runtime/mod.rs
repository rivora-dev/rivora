//! Rivora Runtime — single source of engineering reasoning (RFC-005, RFC-014).

/// Explainable Engineering Assistance (RFC-019).
pub mod assistance;
/// Recalled Context, patterns, and historical trends (RFC-017).
pub mod context;
/// Local-first embedding abstraction for semantic recall (RFC-016).
pub mod embedding;
mod evaluation;
/// Investigation Graph subsystem (RFC-015).
pub mod graph;
mod investigation;
mod knowledge;
/// Learning subsystem request types and Runtime methods.
pub mod learning;
mod memory;
/// Observation ingestion request types and Runtime methods.
pub mod observation;
mod recommendation;
/// Search and Recall subsystem (RFC-016).
pub mod search;
mod verification;
/// Composite Capabilities and Assisted Workflows (RFC-018).
pub mod workflow;

use std::sync::Arc;

use crate::runtime::embedding::{EmbeddingProvider, TokenHashEmbedding};
use crate::storage::Store;

/// Rivora Runtime entry point.
///
/// Interfaces never bypass the Runtime. All engineering reasoning lives here.
#[derive(Clone)]
pub struct Runtime {
    store: Arc<dyn Store>,
    embedding: Arc<dyn EmbeddingProvider>,
}

impl Runtime {
    /// Create a Runtime backed by the given store, using the
    /// deterministic local embedding baseline for semantic recall.
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self {
            store,
            embedding: Arc::new(TokenHashEmbedding::new()),
        }
    }

    /// Create a Runtime with a custom embedding provider (RFC-016).
    ///
    /// The provider must be deterministic and local-first; the Runtime
    /// never depends on a mandatory external AI provider.
    pub fn with_embedding_provider(
        store: Arc<dyn Store>,
        provider: Arc<dyn EmbeddingProvider>,
    ) -> Self {
        Self {
            store,
            embedding: provider,
        }
    }

    /// Access the underlying store (for read-only inspection in tests).
    pub fn store(&self) -> &Arc<dyn Store> {
        &self.store
    }

    /// Access the configured embedding provider.
    pub fn embedding(&self) -> &Arc<dyn EmbeddingProvider> {
        &self.embedding
    }
}

impl std::fmt::Debug for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runtime").finish_non_exhaustive()
    }
}
