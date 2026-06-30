//! Domain-agnostic typed primitives for Open Rivora.
//!
//! This crate provides the reusable *mechanism* layer: a type-tagged
//! identifier ([`TypedId`]), a semantic version ([`Version`]), and a
//! non-empty text primitive ([`NonEmptyString`]). Domain *vocabulary* (the
//! named identifiers like `ObservationId` and `ReceiptId`) lives in
//! `rivora-core`; this crate deliberately carries no domain knowledge.
//!
//! All primitives serialize cleanly (as strings), validate on construction
//! and on deserialize, and funnel validation errors through [`rivora_errors`].

pub mod id;
pub mod text;
pub mod version;

pub use id::{IdTag, TypedId, MAX_ID_LEN};
pub use text::{NonEmptyString, MAX_TEXT_LEN};
pub use version::Version;

/// Convenience re-exports for the most common primitives.
pub mod prelude {
    pub use crate::id::{IdTag, TypedId};
    pub use crate::text::NonEmptyString;
    pub use crate::version::Version;
}
