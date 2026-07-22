//! Rivora connectors — observe, normalize, produce Observations (RFC-012).
//!
//! Connectors never evaluate, verify, recommend, or learn.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod github;
pub mod github_actions;
pub mod kubernetes;
pub mod local;
pub mod sentry;

pub use github_actions::ConnectorStatusReport;

use rivora::domain::{ObservationKind, Provenance};
use thiserror::Error;

/// Connector errors.
#[derive(Debug, Error)]
pub enum ConnectorError {
    /// I/O failure while observing.
    #[error("io error: {0}")]
    Io(String),
    /// External API failure.
    #[error("api error: {0}")]
    Api(String),
    /// Invalid connector configuration.
    #[error("config error: {0}")]
    Config(String),
    /// Normalization failure.
    #[error("normalization error: {0}")]
    Normalize(String),
}

/// Result type for connectors.
pub type ConnectorResult<T> = Result<T, ConnectorError>;

/// Normalized observation ready for Runtime ingestion.
///
/// Connectors stop here — they do not write Memory or reason.
#[derive(Debug, Clone)]
pub struct NormalizedObservation {
    /// Observation kind.
    pub kind: ObservationKind,
    /// Summary of what happened.
    pub summary: String,
    /// Structured payload.
    pub payload: serde_json::Value,
    /// Source system (`local`, `github`, ...).
    pub source: String,
    /// When the event occurred.
    pub observed_at: chrono::DateTime<chrono::Utc>,
    /// Optional idempotency key.
    pub idempotency_key: Option<String>,
    /// Provenance for the observation.
    pub provenance: Provenance,
}

impl NormalizedObservation {
    /// Helper constructor.
    pub fn new(
        kind: ObservationKind,
        summary: impl Into<String>,
        payload: serde_json::Value,
        source: impl Into<String>,
        observed_at: chrono::DateTime<chrono::Utc>,
        idempotency_key: Option<String>,
        actor: impl Into<String>,
    ) -> Self {
        let source = source.into();
        let provenance = Provenance::now(actor, source.clone());
        Self {
            kind,
            summary: summary.into(),
            payload,
            source,
            observed_at,
            idempotency_key,
            provenance,
        }
    }
}
