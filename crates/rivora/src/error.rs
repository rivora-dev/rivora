//! Structured errors for Rivora Runtime operations.

use thiserror::Error;

use crate::domain::{InvestigationId, InvestigationStatus, ObjectId};

/// Result type used across Rivora.
pub type RivoraResult<T> = Result<T, RivoraError>;

/// Structured error type for all Rivora operations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RivoraError {
    /// Requested Investigation was not found.
    #[error("investigation not found: {0}")]
    InvestigationNotFound(InvestigationId),

    /// Requested Engineering Object was not found.
    #[error("object not found: {0}")]
    ObjectNotFound(ObjectId),

    /// Invalid lifecycle transition attempted.
    #[error("invalid lifecycle transition for investigation {investigation_id}: {from} → {to}")]
    InvalidLifecycleTransition {
        /// Investigation that failed to transition.
        investigation_id: InvestigationId,
        /// Current status.
        from: InvestigationStatus,
        /// Requested status.
        to: InvestigationStatus,
    },

    /// Validation failure for domain input.
    #[error("validation error: {0}")]
    Validation(String),

    /// Persistence failure.
    #[error("storage error: {0}")]
    Storage(String),

    /// Serialization or deserialization failure.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Duplicate ingestion rejected by idempotency.
    #[error("duplicate observation for idempotency key: {0}")]
    DuplicateObservation(String),

    /// Operation not allowed in the current Investigation state.
    #[error("operation not allowed in status {status}: {message}")]
    OperationNotAllowed {
        /// Current Investigation status.
        status: InvestigationStatus,
        /// Human-readable explanation.
        message: String,
    },

    /// Generic precondition failure.
    #[error("precondition failed: {0}")]
    Precondition(String),
}

impl RivoraError {
    /// Create a validation error.
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }

    /// Create a storage error.
    pub fn storage(message: impl Into<String>) -> Self {
        Self::Storage(message.into())
    }

    /// Create a serialization error.
    pub fn serialization(message: impl Into<String>) -> Self {
        Self::Serialization(message.into())
    }
}
