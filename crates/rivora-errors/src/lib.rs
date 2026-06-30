//! Shared error types for Open Rivora.
//!
//! Errors are **typed** (a closed [`RivoraError`] enum), **human readable**
//! (each variant renders an actionable message via [`std::fmt::Display`]),
//! **actionable** (messages name the offending value and what is expected),
//! and **consistent** (every error maps to a stable [`ErrorKind`] tag and can
//! be serialized to a small JSON object for future receipts).
//!
//! This crate is intentionally dependency-light: only `thiserror`, `serde`,
//! and `serde_json`. Provider/connector error variants belong to future
//! phases and are deliberately absent.

mod kind;

pub use kind::ErrorKind;

use std::io;
use thiserror::Error;

/// The canonical error type for the Open Rivora foundation.
///
/// Variants cover only the foundational domains (identifiers, versions,
/// values, configuration, secrets, I/O, serialization). Runtime provider
/// errors are introduced by later phases.
#[derive(Debug, Error)]
pub enum RivoraError {
    /// A typed identifier failed validation.
    ///
    /// `kind` names the identifier family (e.g. `"observation"`), `reason`
    /// explains why the value was rejected.
    #[error("invalid {kind} identifier: {reason}")]
    InvalidIdentifier { kind: &'static str, reason: String },

    /// A version string failed to parse.
    #[error("invalid version {input:?}: {reason}")]
    InvalidVersion { input: String, reason: String },

    /// A typed primitive value failed validation.
    #[error("invalid value for {field}: {reason}")]
    InvalidValue { field: &'static str, reason: String },

    /// A configuration value failed validation.
    #[error("invalid configuration: {reason}")]
    InvalidConfig { reason: String },

    /// A configuration file was expected but not found at `path`.
    #[error("configuration file not found: {path}")]
    ConfigNotFound { path: String },

    /// A configuration could not be loaded or parsed.
    #[error("could not load configuration: {reason}")]
    ConfigLoad { reason: String },

    /// A secret reference could not be resolved.
    #[error("secret could not be resolved: {reason}")]
    Secret { reason: String },

    /// An underlying I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// A (de)serialization failure.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// An unexpected, internal failure. Should be reported, not retried
    /// blindly.
    #[error("internal error: {reason}")]
    Internal { reason: String },

    /// A trait-level error from a provider implementation (connector,
    /// inference, storage, etc.). The `kind` string identifies the specific
    /// failure class (e.g. `"source_unavailable"`, `"rate_limited"`); the
    /// message explains what happened and what to do about it.
    #[error("provider error ({kind}): {message}")]
    Provider { kind: &'static str, message: String },

    /// A reliability receipt failed validation.
    ///
    /// The `reason` explains which invariant was violated and what the
    /// caller should fix.
    #[error("invalid receipt: {reason}")]
    Receipt { reason: String },
}

impl RivoraError {
    /// Returns the stable, payload-free category for this error.
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::InvalidIdentifier { .. } => ErrorKind::InvalidIdentifier,
            Self::InvalidVersion { .. } => ErrorKind::InvalidVersion,
            Self::InvalidValue { .. } => ErrorKind::InvalidValue,
            Self::InvalidConfig { .. } => ErrorKind::InvalidConfig,
            Self::ConfigNotFound { .. } => ErrorKind::ConfigNotFound,
            Self::ConfigLoad { .. } => ErrorKind::ConfigLoad,
            Self::Secret { .. } => ErrorKind::Secret,
            Self::Io(_) => ErrorKind::Io,
            Self::Serialization(_) => ErrorKind::Serialization,
            Self::Internal { .. } => ErrorKind::Internal,
            Self::Provider { .. } => ErrorKind::Provider,
            Self::Receipt { .. } => ErrorKind::Receipt,
        }
    }

    /// Convenience constructor for an identifier error.
    #[must_use]
    pub fn invalid_identifier(kind: &'static str, reason: impl Into<String>) -> Self {
        Self::InvalidIdentifier {
            kind,
            reason: reason.into(),
        }
    }

    /// Convenience constructor for a version error.
    #[must_use]
    pub fn invalid_version(input: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidVersion {
            input: input.into(),
            reason: reason.into(),
        }
    }

    /// Convenience constructor for a typed-value error.
    #[must_use]
    pub fn invalid_value(field: &'static str, reason: impl Into<String>) -> Self {
        Self::InvalidValue {
            field,
            reason: reason.into(),
        }
    }

    /// Convenience constructor for an internal error.
    #[must_use]
    pub fn internal(reason: impl Into<String>) -> Self {
        Self::Internal {
            reason: reason.into(),
        }
    }

    /// Convenience constructor for a provider error.
    #[must_use]
    pub fn provider(kind: &'static str, message: impl Into<String>) -> Self {
        Self::Provider {
            kind,
            message: message.into(),
        }
    }

    /// Convenience constructor for a receipt validation error.
    #[must_use]
    pub fn receipt(reason: impl Into<String>) -> Self {
        Self::Receipt {
            reason: reason.into(),
        }
    }
}

/// A [`Result`] alias carrying [`RivoraError`].
pub type Result<T> = std::result::Result<T, RivoraError>;

/// Serialize an error as a small, stable JSON object: `{"kind","message"}`.
///
/// This keeps error representations consistent across the CLI and future
/// receipts without leaking internal enum layout.
impl serde::Serialize for RivoraError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut st = serializer.serialize_struct("RivoraError", 2)?;
        st.serialize_field("kind", self.kind().as_str())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_messages_are_actionable() {
        let e = RivoraError::invalid_identifier("observation", "must not be empty");
        assert!(e.to_string().contains("observation"));
        assert!(e.to_string().contains("empty"));

        let e = RivoraError::invalid_version("1.2", "missing patch component");
        assert!(e.to_string().contains("1.2"));

        let e = RivoraError::ConfigNotFound {
            path: "/tmp/rivora.toml".to_string(),
        };
        assert!(e.to_string().contains("/tmp/rivora.toml"));
    }

    #[test]
    fn kind_maps_each_variant() {
        assert_eq!(
            RivoraError::invalid_identifier("x", "y").kind(),
            ErrorKind::InvalidIdentifier
        );
        assert_eq!(
            RivoraError::invalid_version("x", "y").kind(),
            ErrorKind::InvalidVersion
        );
        assert_eq!(
            RivoraError::invalid_value("f", "y").kind(),
            ErrorKind::InvalidValue
        );
        assert_eq!(
            RivoraError::InvalidConfig {
                reason: "x".to_string()
            }
            .kind(),
            ErrorKind::InvalidConfig
        );
        assert_eq!(
            RivoraError::ConfigNotFound {
                path: "x".to_string()
            }
            .kind(),
            ErrorKind::ConfigNotFound
        );
        assert_eq!(
            RivoraError::ConfigLoad {
                reason: "x".to_string()
            }
            .kind(),
            ErrorKind::ConfigLoad
        );
        assert_eq!(
            RivoraError::Secret {
                reason: "x".to_string()
            }
            .kind(),
            ErrorKind::Secret
        );
        assert_eq!(RivoraError::Io(io::Error::other("x")).kind(), ErrorKind::Io);
        assert_eq!(
            RivoraError::Internal {
                reason: "x".to_string()
            }
            .kind(),
            ErrorKind::Internal
        );
        assert_eq!(
            RivoraError::provider("source_unavailable", "AWS timeout").kind(),
            ErrorKind::Provider
        );
    }

    #[test]
    fn from_io_error_converts() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "missing");
        let rivora: RivoraError = io_err.into();
        assert_eq!(rivora.kind(), ErrorKind::Io);
    }

    #[test]
    fn serializes_to_kind_and_message_object() {
        let e = RivoraError::ConfigNotFound {
            path: "/x.toml".to_string(),
        };
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["kind"], "config_not_found");
        assert_eq!(json["message"], e.to_string());
    }

    #[test]
    fn provider_error_display_includes_kind_and_message() {
        let e = RivoraError::provider("rate_limited", "try again in 30s");
        let msg = e.to_string();
        assert!(msg.contains("rate_limited"));
        assert!(msg.contains("try again in 30s"));
    }

    #[test]
    fn provider_error_serializes_correctly() {
        let e = RivoraError::provider("permission_denied", "missing read scope");
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["kind"], "provider");
        assert!(json["message"]
            .as_str()
            .unwrap()
            .contains("permission_denied"));
    }

    #[test]
    fn receipt_error_display_includes_reason() {
        let e = RivoraError::receipt("evidence must not be empty");
        let msg = e.to_string();
        assert!(msg.contains("invalid receipt"));
        assert!(msg.contains("evidence must not be empty"));
        assert_eq!(e.kind(), ErrorKind::Receipt);
    }
}
