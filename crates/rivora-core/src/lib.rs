//! Foundational domain types and logging setup for Open Rivora.
//!
//! `rivora-core` is the shared domain *vocabulary* layer: named, typed
//! identifiers and version kinds built on [`rivora_types`] primitives, plus a
//! single structured-logging entry point. It contains **no business logic**,
//! **no provider traits**, and **no I/O** beyond installing a tracing
//! subscriber. Per the architecture, the core depends on nothing
//! provider-specific; provider crates will depend upward on this crate.
//!
//! What is deliberately absent (later phases): the `Connector`,
//! `InferenceProvider`, and `Storage` traits; receipt schema/validation; the
//! context graph; the adaptive engine; any runtime behavior.

pub mod id;
pub mod logging;
pub mod version;

pub use id::{
    Ability, AbilityId, Context, ContextId, Deployment, DeploymentId, Incident, IncidentId,
    Observation, ObservationId, Organization, OrganizationId, Receipt, ReceiptId, Service,
    ServiceId,
};
pub use logging::{init_logging, init_logging_default, LoggingConfig, LoggingFormat};
pub use version::{AbilityVersion, ConnectorVersion, SchemaVersion, TypedVersion};

// Re-export the most-used primitives so downstream crates can depend on
// `rivora-core` alone for the common foundation surface.
pub use rivora_types::{IdTag, NonEmptyString, TypedId, Version};
