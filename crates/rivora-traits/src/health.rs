//! Common health status type shared by all provider traits.

use serde::{Deserialize, Serialize};

/// The health status of a provider (connector, inference backend, storage, etc.).
///
/// Returned by the `health()` method on every provider trait to enable
/// readiness probes and dependency checks.
///
/// # Examples
///
/// ```rust
/// use rivora_traits::HealthStatus;
///
/// let h = HealthStatus::Healthy;
/// assert!(h.is_healthy());
///
/// let h = HealthStatus::Degraded { reason: "slow".into() };
/// assert!(!h.is_healthy());
/// assert_eq!(h.reason(), Some("slow"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    /// The provider is operating normally.
    Healthy,
    /// The provider is operational but impaired.
    Degraded { reason: String },
    /// The provider is not operational.
    Unhealthy { reason: String },
}

impl HealthStatus {
    /// Returns `true` if the status is [`Healthy`](HealthStatus::Healthy).
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy)
    }

    /// Returns the human-readable reason for a non-healthy status, if any.
    #[must_use]
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Healthy => None,
            Self::Degraded { reason } | Self::Unhealthy { reason } => Some(reason),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthy_is_healthy() {
        assert!(HealthStatus::Healthy.is_healthy());
        assert_eq!(HealthStatus::Healthy.reason(), None);
    }

    #[test]
    fn degraded_is_not_healthy() {
        let h = HealthStatus::Degraded {
            reason: "latency high".into(),
        };
        assert!(!h.is_healthy());
        assert_eq!(h.reason(), Some("latency high"));
    }

    #[test]
    fn unhealthy_is_not_healthy() {
        let h = HealthStatus::Unhealthy {
            reason: "connection refused".into(),
        };
        assert!(!h.is_healthy());
        assert_eq!(h.reason(), Some("connection refused"));
    }

    #[test]
    fn serializes_as_snake_case() {
        let json = serde_json::to_string(&HealthStatus::Healthy).unwrap();
        assert_eq!(json, "\"healthy\"");
    }

    #[test]
    fn round_trips_through_serde() {
        let status = HealthStatus::Degraded {
            reason: "test".into(),
        };
        let json = serde_json::to_string(&status).unwrap();
        let back: HealthStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, status);
    }
}
