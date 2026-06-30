//! Shared testing infrastructure for Open Rivora.
//!
//! Reusable across every future crate so tests stay consistent and do not
//! reinvent fixtures, temp filesystems, snapshots, property tests, or mocks.
//!
//! - [`fixtures`]: sample domain values (typed IDs, versions, a `rivora.toml`).
//! - [`tempfs`]: disposable temp directories and workspaces ([`tempfile`]).
//! - [`snapshot`]: golden snapshots ([`insta`]) with deterministic settings.
//! - [`property`]: [`proptest`] strategies for the foundational primitives.
//! - [`mock`]: deterministic mocks such as [`mock::FakeClock`].
//! - [`mock_traits`]: fake/mock implementations of the core traits from
//!   `rivora-traits` ([`NullConnector`], [`EchoProvider`], [`NullStorage`],
//!   etc.).
//!
//! This crate is intended to be used as a `[dev-dependencies]` entry by other
//! crates.

pub mod fixtures;
pub mod mock;
pub mod mock_traits;
pub mod property;
pub mod snapshot;
pub mod tempfs;

pub use fixtures::{
    sample_config_toml, sample_id, sample_organization_id, sample_schema_version,
    sample_service_id, sample_version,
};
pub use mock::FakeClock;
pub use mock_traits::{
    CountingIdGen, EchoProvider, InMemoryStorage, JsonReceiptRenderer, MarkdownReceiptRenderer,
    NullConnector, NullStorage, ScriptedConnector, ScriptedProvider, VecLogger,
};
pub use tempfs::{temp_dir, TempWorkspace};
