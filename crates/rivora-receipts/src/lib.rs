//! # rivora-receipts
//!
//! Typed reliability receipts for Open Rivora.
//!
//! A reliability receipt is the canonical, validated artifact that explains
//! *why* Open Rivora reached a conclusion. Every recommendation, explanation,
//! and Ability conclusion produces a receipt. Receipts are mandatory,
//! immutable, and reproducible.
//!
//! ## Schema
//!
//! A [`Receipt`] is the top-level type. It contains:
//!
//! - A unique [`ReceiptId`] (a [`TypedId<Receipt>`](rivora_core::ReceiptId))
//! - A [`ReceiptKind`] discriminator
//! - A [`ReceiptStatus`] lifecycle state
//! - Structured metadata, subject, and summary
//! - At least one [`Evidence`] reference
//! - At least one ordered [`ReasoningStep`]
//! - A typed [`Confidence`] value
//! - A [`Risk`] assessment
//! - Optional [`SuggestedAction`]s
//! - Provenance, timestamps, and version
//!
//! ## Validation
//!
//! All receipts are validated before being surfaced. See the
//! [`validation`] module for the canonical validation rules.
//!
//! ## Rendering
//!
//! The [`renderers`] module provides JSON and Markdown renderers that
//! implement the [`ReceiptRenderer`](rivora_traits::receipt::ReceiptRenderer)
//! trait from `rivora-traits`.

pub mod action;
pub mod builders;
pub mod confidence;
pub mod evidence;
pub mod fixtures;
pub mod kind;
pub mod metadata;
pub mod reasoning;
pub mod renderers;
pub mod risk;
pub mod status;
pub mod subject;
pub mod validation;

mod receipt;

pub use action::{ActionKind, ApprovalRequirement, HumanApproval, SuggestedAction};
pub use confidence::{Confidence, ConfidenceLevel};
pub use evidence::{Evidence, EvidenceKind, EvidenceSource};
pub use kind::ReceiptKind;
pub use metadata::{
    AbilityRef, InferenceRef, ReceiptMetadata, ReceiptProvenance, ReceiptTimestamps, ReceiptVersion,
};
pub use reasoning::ReasoningStep;
pub use receipt::Receipt;
pub use renderers::{JsonRenderer, MarkdownRenderer};
pub use risk::{Risk, RiskLevel};
pub use status::ReceiptStatus;
pub use subject::{ReceiptSubject, ReceiptSummary};

/// The canonical receipt ID type. Alias for [`rivora_core::ReceiptId`].
pub type ReceiptId = rivora_core::ReceiptId;
