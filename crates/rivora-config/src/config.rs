//! Configuration model.
//!
//! The configuration is intentionally small and foundation-only: organization
//! identity, local-first storage, and logging. Connector and inference
//! sections belong to future phases and are deliberately absent.

use rivora_core::LoggingConfig;
use rivora_types::NonEmptyString;
use serde::{Deserialize, Serialize};

/// The root configuration object loaded from `rivora.toml` (or YAML) plus
/// environment overrides.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Config {
    /// Organization identity (owner of local memory).
    #[serde(default)]
    pub organization: OrganizationSection,
    /// Local-first storage configuration.
    #[serde(default)]
    pub storage: StorageSection,
    /// Structured logging configuration (reuses `rivora_core::LoggingConfig`).
    #[serde(default)]
    pub logging: LoggingConfig,
}

/// Organization section.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct OrganizationSection {
    /// Stable organization identifier.
    #[serde(default)]
    pub id: Option<NonEmptyString>,
    /// Human-readable organization name.
    #[serde(default)]
    pub name: Option<NonEmptyString>,
}

/// Storage section. The default backend is embedded and local; no remote
/// service is ever required for local operation (ADR-0007).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct StorageSection {
    /// Backend name (e.g. `redb`, `sqlite`). Provider crates are feature-gated.
    #[serde(default)]
    pub backend: Option<NonEmptyString>,
    /// Filesystem path for the embedded store.
    #[serde(default)]
    pub path: Option<NonEmptyString>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_empty_with_info_logging() {
        let cfg = Config::default();
        assert!(cfg.organization.id.is_none());
        assert!(cfg.organization.name.is_none());
        assert!(cfg.storage.backend.is_none());
        assert!(cfg.storage.path.is_none());
        assert_eq!(cfg.logging.level, "info");
    }

    #[test]
    fn config_round_trips_through_json() {
        let cfg = Config {
            organization: OrganizationSection {
                id: Some(NonEmptyString::new("org-acme").unwrap()),
                name: Some(NonEmptyString::new("Acme").unwrap()),
            },
            storage: StorageSection {
                backend: Some(NonEmptyString::new("redb").unwrap()),
                path: Some(NonEmptyString::new("./.rivora/store").unwrap()),
            },
            logging: LoggingConfig::default(),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cfg);
    }
}
