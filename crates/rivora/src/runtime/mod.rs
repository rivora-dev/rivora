//! Rivora Runtime — single source of engineering reasoning (RFC-005, RFC-014).

/// Explainable Engineering Assistance (RFC-019).
pub mod assistance;
/// Recalled Context, patterns, and historical trends (RFC-017).
pub mod context;
/// Local-first embedding abstraction for semantic recall (RFC-016).
pub mod embedding;
/// Capability Engineering Loop orchestration (RFC-028 / v0.7).
pub mod engineering_loop;
mod evaluation;
/// Controlled external execution (RFC-025/026/027).
pub mod execution;
/// Investigation Graph subsystem (RFC-015).
pub mod graph;
mod investigation;
mod knowledge;
/// Learning subsystem request types and Runtime methods.
pub mod learning;
mod memory;
/// Observation ingestion request types and Runtime methods.
pub mod observation;
/// Implementation Records, Measured Learning Outcomes, and Patterns (RFC-022/023/024).
pub mod outcome;
/// Improvement Proposal lifecycle and reasoning (RFC-020/RFC-021).
pub mod proposal;
mod recommendation;
/// Search and Recall subsystem (RFC-016).
pub mod search;
mod verification;
/// Composite Capabilities and Assisted Workflows (RFC-018).
pub mod workflow;

use std::sync::Arc;

use crate::domain::ExecutionCapabilityRegistry;
use crate::runtime::embedding::{EmbeddingProvider, TokenHashEmbedding};
use crate::storage::Store;

/// Rivora Runtime entry point.
///
/// Interfaces never bypass the Runtime. All engineering reasoning lives here.
#[derive(Clone)]
pub struct Runtime {
    store: Arc<dyn Store>,
    embedding: Arc<dyn EmbeddingProvider>,
    execution_registry: ExecutionCapabilityRegistry,
}

impl Runtime {
    /// Create a Runtime backed by the given store, using the
    /// deterministic local embedding baseline for semantic recall.
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self {
            store,
            embedding: Arc::new(TokenHashEmbedding::new()),
            execution_registry: ExecutionCapabilityRegistry::new(),
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
            execution_registry: ExecutionCapabilityRegistry::new(),
        }
    }

    /// Create a Runtime with a pre-populated execution capability registry.
    pub fn with_execution_registry(
        store: Arc<dyn Store>,
        execution_registry: ExecutionCapabilityRegistry,
    ) -> Self {
        Self {
            store,
            embedding: Arc::new(TokenHashEmbedding::new()),
            execution_registry,
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

    /// Local store health report (v0.9 production diagnostics).
    pub fn store_health(&self) -> crate::error::RivoraResult<crate::domain::StoreHealthReport> {
        self.store.health_report()
    }

    /// Sanitized diagnostic export (v0.9).
    pub fn diagnostic_export(&self) -> crate::error::RivoraResult<serde_json::Value> {
        self.store.diagnostic_export()
    }

    /// Backup the store to a destination directory.
    pub fn backup_store(
        &self,
        dest: impl AsRef<std::path::Path>,
    ) -> crate::error::RivoraResult<()> {
        self.store.backup_to(dest.as_ref())
    }

    /// Rebuild derived observation indexes from canonical records.
    pub fn rebuild_observation_indexes(&self) -> crate::error::RivoraResult<u64> {
        self.store.rebuild_observation_indexes()
    }
}

impl std::fmt::Debug for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runtime").finish_non_exhaustive()
    }
}
