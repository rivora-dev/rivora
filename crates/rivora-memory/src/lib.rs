//! # rivora-memory
//!
//! Typed Context Memory model for Open Rivora.
//!
//! The context memory model captures durable, organization-specific knowledge
//! that adaptive workflows learn over time. Each [`MemoryRecord`] is a typed,
//! provenance-tracked, confidence-scored unit of memory that can be recalled,
//! validated, and surfaced to engineers.
//!
//! ## Schema
//!
//! A [`MemoryIndex`] is the top-level type. It contains:
//!
//! - Graph-level [`MemoryMetadata`]
//! - A set of [`MemoryRecord`]s keyed by ID
//!
//! Each [`MemoryRecord`] carries:
//!
//! - A unique `id` (`NonEmptyString`)
//! - A [`MemoryKind`], [`MemoryScope`], and [`MemoryStatus`]
//! - A title and body
//! - Subject references, graph node/edge references, and receipt references
//! - A [`MemorySource`], [`MemoryProvenance`], [`MemoryConfidence`], and
//!   [`MemoryRetention`]
//! - [`MemoryTimestamps`] and [`MemoryVersion`]
//! - Record-level labels and [`MemoryMetadata`]
//!
//! ## Validation
//!
//! All records and indices are validated before being surfaced. See the
//! [`validation`] module for the canonical validation rules.

pub mod builders;
pub mod confidence;
pub mod feedback;
pub mod fixtures;
pub mod index;
pub mod kind;
pub mod metadata;
pub mod provenance;
pub mod recall;
pub mod record;
pub mod retention;
pub mod scope;
pub mod snapshot;
pub mod source;
pub mod status;
pub mod validation;

pub use confidence::{MemoryConfidence, MemoryConfidenceLevel};
pub use feedback::{FeedbackKind, FeedbackSource, FeedbackTargetType, HumanFeedback};
pub use index::MemoryIndex;
pub use kind::MemoryKind;
pub use metadata::{MemoryMetadata, MemoryTimestamps, MemoryVersion};
pub use provenance::MemoryProvenance;
pub use recall::{MemoryRecallQuery, MemoryRecallResult};
pub use record::MemoryRecord;
pub use retention::{MemoryDecay, MemoryRetention, MemoryRetentionPolicy};
pub use scope::MemoryScope;
pub use snapshot::MemorySnapshot;
pub use source::MemorySource;
pub use status::MemoryStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Memory;

impl rivora_types::IdTag for Memory {
    const KIND: &'static str = "memory";
}

pub type MemoryId = rivora_types::TypedId<Memory>;

#[cfg(test)]
mod tests {
    use super::*;
    use rivora_types::IdTag;

    #[test]
    fn memory_id_tag_kind_is_memory() {
        assert_eq!(Memory::KIND, "memory");
    }

    #[test]
    fn memory_id_new_accepts_valid_value() {
        let id = MemoryId::new("mem-1").unwrap();
        assert_eq!(id.as_str(), "mem-1");
    }

    #[test]
    fn memory_id_new_rejects_empty() {
        assert!(MemoryId::new("").is_err());
    }

    #[test]
    fn memory_id_new_unchecked_bypasses_validation() {
        let id = MemoryId::new_unchecked("mem-fixture");
        assert_eq!(id.as_str(), "mem-fixture");
    }

    #[test]
    fn memory_id_round_trips_through_serde() {
        let id = MemoryId::new("mem-1").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"mem-1\"");
        let back: MemoryId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn memory_id_display_is_inner_value() {
        let id = MemoryId::new("mem-9").unwrap();
        assert_eq!(id.to_string(), "mem-9");
    }

    #[test]
    fn memory_id_debug_is_kinded() {
        let id = MemoryId::new("mem-9").unwrap();
        assert_eq!(format!("{id:?}"), "memory(\"mem-9\")");
    }
}
