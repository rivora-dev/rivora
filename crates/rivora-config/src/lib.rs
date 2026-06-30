//! Configuration loading (files + environment) and validation for Open Rivora.
//!
//! Layering is **defaults < file < environment**. Files are TOML
//! (`rivora.toml`); environment overrides use the `RIVORA_` prefix with `__`
//! as the nesting separator. Only validation is performed here — no external
//! services are contacted. Secrets are referenced, never stored (see
//! [`secret::SecretRef`]).
//!
//! The configuration surface is deliberately foundation-only (organization,
//! storage, logging). Connector and inference sections arrive in later phases.

pub mod config;
pub mod loader;
pub mod secret;
pub mod validation;

pub use config::{Config, OrganizationSection, StorageSection};
pub use secret::{Secret, SecretRef};
pub use validation::validate;
