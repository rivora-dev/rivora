//! Secret references.
//!
//! Configuration never stores secret *values* — only [`SecretRef`] references
//! to where a secret lives. This satisfies the "future secrets providers"
//! requirement without pulling in any external service: only the `Env` variant
//! is resolvable today; `Keychain` and `Vault` are explicit, error-returning
//! extension points for later phases.
//!
//! See SECURITY.md: credentials live in the engineer's secret store, never in
//! config, memory, the Context Graph, or logs.

use rivora_errors::RivoraError;
use rivora_types::NonEmptyString;
use serde::{Deserialize, Serialize};

/// A reference to a secret, never the secret itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SecretRef {
    /// Resolve from an environment variable.
    Env {
        /// Environment variable name.
        var: NonEmptyString,
    },
    /// Resolve from the OS keychain (future; not yet supported).
    Keychain {
        /// Keychain entry name.
        name: NonEmptyString,
    },
    /// Resolve from a vault (future; not yet supported).
    Vault {
        /// Vault path/key.
        path: NonEmptyString,
    },
}

impl SecretRef {
    /// Returns a human-readable description of *where* the secret lives.
    ///
    /// This never exposes the secret value and is safe to log.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Env { var } => format!("env var {}", var.as_str()),
            Self::Keychain { name } => {
                format!("OS keychain entry {} (not yet supported)", name.as_str())
            }
            Self::Vault { path } => format!("vault path {} (not yet supported)", path.as_str()),
        }
    }

    /// Resolves the secret value.
    ///
    /// Only [`SecretRef::Env`] is supported today. `Keychain` and `Vault`
    /// return a [`RivoraError::Secret`] indicating the provider is not yet
    /// available — they never touch an external service.
    ///
    /// # Errors
    /// Returns [`RivoraError::Secret`] if the provider is unsupported or the
    /// environment variable is missing.
    pub fn resolve(&self) -> Result<Secret, RivoraError> {
        match self {
            Self::Env { var } => {
                std::env::var(var.as_str())
                    .map(Secret::new)
                    .map_err(|e| RivoraError::Secret {
                        reason: format!("env var {} not available: {e}", var.as_str()),
                    })
            }
            Self::Keychain { name } => Err(RivoraError::Secret {
                reason: format!("keychain provider not yet supported for {name}"),
            }),
            Self::Vault { path } => Err(RivoraError::Secret {
                reason: format!("vault provider not yet supported for {path}"),
            }),
        }
    }
}

/// A resolved secret value.
///
/// Redacts itself in [`Debug`](std::fmt::Debug) and
/// [`Display`](std::fmt::Display) and deliberately does **not** implement
/// `Serialize`, so it cannot be accidentally persisted or embedded in a future
/// receipt. Use [`Secret::expose`] only at the point of use.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    /// Wraps a raw secret string. Only available within this crate.
    pub(crate) fn new(value: String) -> Self {
        Self(value)
    }

    /// Returns the raw secret. Use only at the point of use; never log or
    /// serialize the returned value.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("***")
    }
}

impl std::fmt::Display for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("***")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describe_never_exposes_value() {
        let env = SecretRef::Env {
            var: NonEmptyString::new("MY_TOKEN").unwrap(),
        };
        assert_eq!(env.describe(), "env var MY_TOKEN");
        let kc = SecretRef::Keychain {
            name: NonEmptyString::new("rivora").unwrap(),
        };
        assert!(kc.describe().contains("not yet supported"));
    }

    #[test]
    fn unsupported_providers_return_secret_error() {
        let kc = SecretRef::Keychain {
            name: NonEmptyString::new("rivora").unwrap(),
        };
        let err = kc.resolve().unwrap_err();
        assert_eq!(err.kind(), rivora_errors::ErrorKind::Secret);

        let vault = SecretRef::Vault {
            path: NonEmptyString::new("secret/data/rivora").unwrap(),
        };
        assert_eq!(
            vault.resolve().unwrap_err().kind(),
            rivora_errors::ErrorKind::Secret
        );
    }

    #[test]
    fn missing_env_var_returns_secret_error() {
        let env = SecretRef::Env {
            var: NonEmptyString::new("RIVORA_DEFINITELY_NOT_SET_X9").unwrap(),
        };
        let err = env.resolve().unwrap_err();
        assert_eq!(err.kind(), rivora_errors::ErrorKind::Secret);
        assert!(err.to_string().contains("RIVORA_DEFINITELY_NOT_SET_X9"));
    }

    #[test]
    fn secret_redacts_in_debug_and_display() {
        let s = Secret::new("super-secret-value".to_string());
        assert_eq!(format!("{s:?}"), "***");
        assert_eq!(format!("{s}"), "***");
        assert_eq!(s.expose(), "super-secret-value");
    }

    #[test]
    fn secret_ref_round_trips_through_json() {
        let env = SecretRef::Env {
            var: NonEmptyString::new("MY_TOKEN").unwrap(),
        };
        let json = serde_json::to_string(&env).unwrap();
        assert_eq!(json, r#"{"kind":"env","var":"MY_TOKEN"}"#);
        let back: SecretRef = serde_json::from_str(&json).unwrap();
        assert_eq!(back, env);
    }
}
