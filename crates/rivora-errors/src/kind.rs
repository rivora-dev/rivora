//! Programmatic error classification.
//!
//! [`ErrorKind`] is a payload-free discriminant that lets callers branch on
//! *what* went wrong without inspecting error messages. It is stable across
//! releases and serializes as a lowercase string tag.

use serde::{Deserialize, Serialize};

/// A stable, payload-free category for a [`crate::RivoraError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// A typed identifier failed validation (empty, too long, bad characters).
    InvalidIdentifier,
    /// A version string failed to parse as semantic version.
    InvalidVersion,
    /// A typed primitive value failed validation (e.g. empty where non-empty).
    InvalidValue,
    /// A configuration value failed validation.
    InvalidConfig,
    /// A configuration file was expected but not found.
    ConfigNotFound,
    /// A configuration could not be loaded or parsed.
    ConfigLoad,
    /// A secret reference could not be resolved (e.g. missing env var).
    Secret,
    /// An underlying I/O failure.
    Io,
    /// A (de)serialization failure.
    Serialization,
    /// An unexpected, internal failure that should be reported, not retried.
    Internal,
    /// A trait-level error from a provider (connector, inference, storage, etc.).
    /// The `kind` field on the error variant identifies the specific failure
    /// (e.g. `"source_unavailable"`, `"rate_limited"`).
    Provider,
    /// A reliability receipt failed validation.
    Receipt,
}

impl ErrorKind {
    /// Stable lowercase string tag for the kind.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidIdentifier => "invalid_identifier",
            Self::InvalidVersion => "invalid_version",
            Self::InvalidValue => "invalid_value",
            Self::InvalidConfig => "invalid_config",
            Self::ConfigNotFound => "config_not_found",
            Self::ConfigLoad => "config_load",
            Self::Secret => "secret",
            Self::Io => "io",
            Self::Serialization => "serialization",
            Self::Internal => "internal",
            Self::Provider => "provider",
            Self::Receipt => "receipt",
        }
    }
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_is_lowercase_and_stable() {
        assert_eq!(ErrorKind::InvalidIdentifier.as_str(), "invalid_identifier");
        assert_eq!(ErrorKind::ConfigNotFound.as_str(), "config_not_found");
        assert_eq!(ErrorKind::Io.as_str(), "io");
    }

    #[test]
    fn kind_serializes_as_lowercase_tag() {
        let json = serde_json::to_string(&ErrorKind::Secret).unwrap();
        assert_eq!(json, "\"secret\"");
    }

    #[test]
    fn kind_round_trips_through_serde() {
        let back: ErrorKind = serde_json::from_str("\"invalid_version\"").unwrap();
        assert_eq!(back, ErrorKind::InvalidVersion);
    }
}
