//! Structured errors for Rivora Runtime operations.

use thiserror::Error;

use crate::domain::{CliExitCode, FailureClass, InvestigationId, InvestigationStatus, ObjectId};

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

    /// Exclusive store lock is held by another process.
    #[error("store lock conflict: {0}")]
    StoreLocked(String),

    /// On-disk store schema is incompatible with this build.
    #[error("schema mismatch: found {found}, supported max {supported_max}")]
    SchemaMismatch {
        /// Schema version found on disk.
        found: u32,
        /// Maximum schema version supported by this build.
        supported_max: u32,
    },

    /// Corrupt or unreadable store record that cannot be isolated for this path.
    #[error("corrupt record at {path}: {message}")]
    CorruptRecord {
        /// Path of the corrupt record.
        path: String,
        /// Sanitized error detail.
        message: String,
    },

    /// Operation timed out.
    #[error("timeout: {0}")]
    Timeout(String),

    /// External provider rate limit.
    #[error("rate limited: {0}")]
    RateLimited(String),

    /// Authentication / authorization failure against an external provider.
    #[error("authentication failure: {0}")]
    AuthFailure(String),

    /// External provider / API failure.
    #[error("provider failure: {0}")]
    ProviderFailure(String),

    /// Payload exceeds the supported envelope.
    #[error("payload too large: {0}")]
    PayloadTooLarge(String),

    /// Policy denied the operation (execution authority).
    #[error("policy denial: {0}")]
    PolicyDenial(String),

    /// Independent Verification reported failure.
    #[error("verification failure: {0}")]
    VerificationFailure(String),

    /// Partial completion — some stages finished, overall work incomplete.
    #[error("partial completion: {0}")]
    PartialCompletion(String),

    /// Operation is unsupported in this configuration or build.
    #[error("unsupported: {0}")]
    Unsupported(String),

    /// Concurrent write / idempotency conflict.
    #[error("conflict: {0}")]
    Conflict(String),
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

    /// Create a precondition failure.
    pub fn precondition(message: impl Into<String>) -> Self {
        Self::Precondition(message.into())
    }

    /// Create a store-locked error.
    pub fn store_locked(message: impl Into<String>) -> Self {
        Self::StoreLocked(message.into())
    }

    /// Create a timeout error.
    pub fn timeout(message: impl Into<String>) -> Self {
        Self::Timeout(message.into())
    }

    /// Create a provider failure.
    pub fn provider_failure(message: impl Into<String>) -> Self {
        Self::ProviderFailure(message.into())
    }

    /// Create an auth failure.
    pub fn auth_failure(message: impl Into<String>) -> Self {
        Self::AuthFailure(message.into())
    }

    /// Create a payload-too-large error.
    pub fn payload_too_large(message: impl Into<String>) -> Self {
        Self::PayloadTooLarge(message.into())
    }

    /// Create a partial-completion error.
    pub fn partial(message: impl Into<String>) -> Self {
        Self::PartialCompletion(message.into())
    }

    /// Create an unsupported error.
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::Unsupported(message.into())
    }

    /// Create a conflict error.
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict(message.into())
    }

    /// Map this error to a stable CLI exit code.
    pub fn exit_code(&self) -> CliExitCode {
        match self {
            Self::Validation(_) | Self::InvalidLifecycleTransition { .. } => {
                CliExitCode::Validation
            }
            Self::InvestigationNotFound(_) | Self::ObjectNotFound(_) => CliExitCode::NotFound,
            Self::Unsupported(_) => CliExitCode::Unsupported,
            Self::StoreLocked(_) => CliExitCode::LockConflict,
            Self::OperationNotAllowed { .. }
            | Self::Precondition(_)
            | Self::Conflict(_)
            | Self::DuplicateObservation(_) => CliExitCode::Blocked,
            Self::PartialCompletion(_) => CliExitCode::Partial,
            Self::ProviderFailure(_) | Self::RateLimited(_) => CliExitCode::ProviderFailure,
            Self::AuthFailure(_) => CliExitCode::AuthFailure,
            Self::Timeout(_) => CliExitCode::Timeout,
            Self::CorruptRecord { .. } => CliExitCode::CorruptStore,
            Self::SchemaMismatch { .. } => CliExitCode::SchemaMismatch,
            Self::PolicyDenial(_) => CliExitCode::PolicyDenial,
            Self::VerificationFailure(_) => CliExitCode::VerificationFailure,
            Self::PayloadTooLarge(_) => CliExitCode::Validation,
            Self::Storage(_) | Self::Serialization(_) => CliExitCode::Internal,
        }
    }

    /// Production failure classification for recovery guidance.
    pub fn failure_class(&self) -> FailureClass {
        match self {
            Self::Timeout(_) | Self::RateLimited(_) => FailureClass::Retryable,
            Self::StoreLocked(_) | Self::Conflict(_) | Self::OperationNotAllowed { .. } => {
                FailureClass::Blocked
            }
            Self::PolicyDenial(_) => FailureClass::Blocked,
            Self::PartialCompletion(_) => FailureClass::Partial,
            Self::Unsupported(_) => FailureClass::Unsupported,
            Self::VerificationFailure(_) => FailureClass::Failed,
            Self::CorruptRecord { .. } | Self::SchemaMismatch { .. } => {
                FailureClass::RequiresManualReview
            }
            Self::AuthFailure(_) => FailureClass::NonRetryable,
            Self::Validation(_)
            | Self::InvestigationNotFound(_)
            | Self::ObjectNotFound(_)
            | Self::InvalidLifecycleTransition { .. }
            | Self::DuplicateObservation(_)
            | Self::PayloadTooLarge(_)
            | Self::Precondition(_) => FailureClass::NonRetryable,
            Self::ProviderFailure(_) => FailureClass::Retryable,
            Self::Storage(_) | Self::Serialization(_) => FailureClass::RequiresManualReview,
        }
    }

    /// Whether automated retry may be appropriate after an explicit user request.
    pub fn is_retryable(&self) -> bool {
        matches!(self.failure_class(), FailureClass::Retryable)
    }

    /// Machine-readable error code string for JSON diagnostics.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvestigationNotFound(_) => "investigation_not_found",
            Self::ObjectNotFound(_) => "object_not_found",
            Self::InvalidLifecycleTransition { .. } => "invalid_lifecycle_transition",
            Self::Validation(_) => "validation",
            Self::Storage(_) => "storage",
            Self::Serialization(_) => "serialization",
            Self::DuplicateObservation(_) => "duplicate_observation",
            Self::OperationNotAllowed { .. } => "operation_not_allowed",
            Self::Precondition(_) => "precondition",
            Self::StoreLocked(_) => "store_locked",
            Self::SchemaMismatch { .. } => "schema_mismatch",
            Self::CorruptRecord { .. } => "corrupt_record",
            Self::Timeout(_) => "timeout",
            Self::RateLimited(_) => "rate_limited",
            Self::AuthFailure(_) => "auth_failure",
            Self::ProviderFailure(_) => "provider_failure",
            Self::PayloadTooLarge(_) => "payload_too_large",
            Self::PolicyDenial(_) => "policy_denial",
            Self::VerificationFailure(_) => "verification_failure",
            Self::PartialCompletion(_) => "partial_completion",
            Self::Unsupported(_) => "unsupported",
            Self::Conflict(_) => "conflict",
        }
    }

    /// Structured JSON value for CLI `--json` error output (secrets must already be absent).
    pub fn to_json_value(&self) -> serde_json::Value {
        serde_json::json!({
            "error": true,
            "code": self.code(),
            "message": self.to_string(),
            "exit_code": self.exit_code().code(),
            "failure_class": self.failure_class().as_str(),
            "retryable": self.is_retryable(),
        })
    }
}
