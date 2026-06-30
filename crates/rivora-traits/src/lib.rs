//! Core trait interfaces for Open Rivora.
//!
//! This crate defines the contract boundaries that every future
//! implementation will satisfy. It introduces zero provider-specific code,
//! zero business logic, and zero runtime behavior.
//!
//! # Traits
//!
//! | Trait | Purpose |
//! |---|---|
//! | [`connector::Connector`] | Read-only infrastructure source |
//! | [`inference::InferenceProvider`] | Inference / reasoning backend |
//! | [`storage::StorageProvider`] | Persistent storage backend |
//! | [`receipt::ReceiptRenderer`] | Renders reliability receipts |
//! | [`clock::Clock`] | Abstract time source |
//! | [`idgen::IdGenerator`] | Abstract identifier generation |
//! | [`logger::Logger`] | Abstract structured logging |
//!
//! # Design principles
//!
//! - **Portable**: no cloud-specific or framework-specific types.
//! - **Testable**: every trait has mock/fake implementations in
//!   `rivora-testing`.
//! - **Read-only by default**: `Connector` exposes no write methods by
//!   construction.
//! - **Deterministic**: `Clock` and `IdGenerator` enable reproducible tests.

pub mod clock;
pub mod connector;
pub mod health;
pub mod idgen;
pub mod inference;
pub mod logger;
pub mod receipt;
pub mod storage;

pub use health::HealthStatus;
