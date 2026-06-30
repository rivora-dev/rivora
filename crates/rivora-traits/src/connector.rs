//! The [`Connector`] trait — read-only infrastructure source.
//!
//! A connector represents any external system that Open Rivora can observe
//! without mutating. Examples include AWS, GitHub, Kubernetes, Cloudflare,
//! Datadog, and Prometheus.
//!
//! Connectors expose their capabilities and metadata, stream typed
//! [`Observation`]s, and provide a read-only health probe. They never
//! perform write operations by construction.
//!
//! # Design principles
//!
//! - **Read-only**: no `put`, `delete`, or `update` methods exist on the trait.
//! - **Async streaming**: observations are delivered as a `Vec` (a bounded
//!   snapshot), not an unbounded `Stream`, to keep the trait portable and
//!   testable.
//! - **Provenance-bearing**: every observation carries a source identifier,
//!   version, and observed-at timestamp.
//! - **Portable**: no cloud-specific types; provider crates define their own
//!   associated types.

use serde::{Deserialize, Serialize};

use crate::HealthStatus;

/// A read-only observation from an infrastructure source.
///
/// Every observation carries provenance metadata so the engine can trace
/// lineage and compute confidence.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Observation {
    /// The connector that produced this observation.
    pub source: String,
    /// The version of the connector that produced this observation.
    pub source_version: String,
    /// ISO-8601 timestamp of when the observation was made.
    pub observed_at: String,
    /// A provider-defined reference to the raw data (e.g. an ARN, SHA, URL).
    pub raw_ref: String,
    /// The kind of entity observed (e.g. `"service"`, `"deployment"`).
    pub kind: String,
    /// The provider-defined payload. Serialized as JSON.
    pub payload: serde_json::Value,
}

/// Metadata describing a connector's identity and version.
///
/// Returned by [`Connector::metadata`] to allow the engine to track which
/// connector produced which observations.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConnectorMetadata {
    /// Unique identifier for the connector (e.g. `"aws"`, `"github"`).
    pub id: String,
    /// Semantic version of the connector implementation.
    pub version: String,
    /// Human-readable name.
    pub name: String,
}

/// The set of capabilities a connector supports.
///
/// Capabilities are strings to allow extensibility without breaking the trait.
/// Every connector must include `"read"`; no write capabilities exist on this
/// trait by construction.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CapabilitySet {
    /// The capabilities this connector supports (e.g. `["read", "metrics"]`).
    pub capabilities: Vec<String>,
}

impl CapabilitySet {
    /// Creates a new capability set from a list of capability strings.
    #[must_use]
    pub fn new(capabilities: Vec<String>) -> Self {
        Self { capabilities }
    }

    /// Returns `true` if the set includes the given capability.
    #[must_use]
    pub fn has(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| c == capability)
    }
}

/// A read-only infrastructure source.
///
/// # Examples
///
/// ```rust
/// use rivora_traits::connector::{Connector, ConnectorMetadata, CapabilitySet, Observation};
/// use rivora_traits::HealthStatus;
///
/// struct FakeConnector;
///
/// impl Connector for FakeConnector {
///     fn metadata(&self) -> ConnectorMetadata {
///         ConnectorMetadata {
///             id: "fake".into(),
///             version: "0.1.0".into(),
///             name: "Fake Connector".into(),
///         }
///     }
///
///     fn capabilities(&self) -> CapabilitySet {
///         CapabilitySet::new(vec!["read".into()])
///     }
///
///     fn health(&self) -> HealthStatus {
///         HealthStatus::Healthy
///     }
///
///     fn observe(&self, _scope: &str, _since: Option<&str>) -> Vec<Observation> {
///         vec![]
///     }
/// }
///
/// let c = FakeConnector;
/// assert_eq!(c.metadata().id, "fake");
/// assert!(c.capabilities().has("read"));
/// assert!(c.health().is_healthy());
/// assert!(c.observe("services", None).is_empty());
/// ```
pub trait Connector: Send + Sync {
    /// Returns metadata identifying this connector.
    fn metadata(&self) -> ConnectorMetadata;

    /// Returns the set of capabilities this connector supports.
    ///
    /// Must always include `"read"`. No write capabilities exist by
    /// construction — the `Connector` trait is read-only.
    fn capabilities(&self) -> CapabilitySet;

    /// Returns the current health status of the connector.
    fn health(&self) -> HealthStatus;

    /// Returns observations for the given scope, optionally filtered to
    /// observations made after `since` (an ISO-8601 timestamp).
    ///
    /// The `scope` parameter is a provider-defined string (e.g. `"services"`,
    /// `"deployments"`) that filters the observation stream. An empty scope
    /// returns all observations.
    ///
    /// Returns a bounded `Vec` of observations. An empty vec indicates no new
    /// observations — this is not an error.
    fn observe(&self, scope: &str, since: Option<&str>) -> Vec<Observation>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_set_has_checks_membership() {
        let caps = CapabilitySet::new(vec!["read".into(), "metrics".into()]);
        assert!(caps.has("read"));
        assert!(caps.has("metrics"));
        assert!(!caps.has("write"));
    }

    #[test]
    fn capability_set_empty() {
        let caps = CapabilitySet::new(vec![]);
        assert!(!caps.has("read"));
    }

    #[test]
    fn observation_fields_are_accessible() {
        let obs = Observation {
            source: "aws".into(),
            source_version: "0.1.0".into(),
            observed_at: "2026-01-01T00:00:00Z".into(),
            raw_ref: "arn:aws:ecs:us-east-1:123456:service/prod".into(),
            kind: "service".into(),
            payload: serde_json::json!({"name": "api-gateway"}),
        };
        assert_eq!(obs.source, "aws");
        assert_eq!(obs.kind, "service");
        assert_eq!(obs.payload["name"], "api-gateway");
    }

    #[test]
    fn observation_round_trips_through_serde() {
        let obs = Observation {
            source: "github".into(),
            source_version: "0.1.0".into(),
            observed_at: "2026-06-26T12:00:00Z".into(),
            raw_ref: "repo/org/main".into(),
            kind: "repository".into(),
            payload: serde_json::json!({"stars": 42}),
        };
        let json = serde_json::to_string(&obs).unwrap();
        let back: Observation = serde_json::from_str(&json).unwrap();
        assert_eq!(back, obs);
    }
}
