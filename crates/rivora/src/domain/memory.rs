//! Memory records — append-only durable facts (RFC-006).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{empty_metadata, Confidence, InvestigationId, Metadata, ObjectId, Provenance};

/// Durable Memory record derived from an Observation.
///
/// Memory is append-only. Corrections create new records.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryRecord {
    /// Stable object identifier.
    pub id: ObjectId,
    /// Primary Investigation.
    pub investigation_id: InvestigationId,
    /// Observation that produced this Memory record.
    pub observation_id: ObjectId,
    /// Factual summary of what happened.
    pub summary: String,
    /// When the fact was recorded into Memory.
    pub recorded_at: DateTime<Utc>,
    /// Optional pointer to a prior Memory record this corrects.
    pub corrects: Option<ObjectId>,
    /// Confidence.
    pub confidence: Confidence,
    /// Provenance.
    pub provenance: Provenance,
    /// Metadata.
    pub metadata: Metadata,
}

impl MemoryRecord {
    /// Create a Memory record from an Observation.
    pub fn from_observation(
        observation_id: ObjectId,
        investigation_id: InvestigationId,
        summary: impl Into<String>,
        recorded_at: DateTime<Utc>,
        provenance: Provenance,
    ) -> Self {
        Self {
            id: ObjectId::new(),
            investigation_id,
            observation_id,
            summary: summary.into(),
            recorded_at,
            corrects: None,
            confidence: Confidence::certain(),
            provenance,
            metadata: empty_metadata(),
        }
    }

    /// Create a correction Memory record that references a prior record.
    pub fn correction(
        observation_id: ObjectId,
        investigation_id: InvestigationId,
        summary: impl Into<String>,
        corrects: ObjectId,
        recorded_at: DateTime<Utc>,
        provenance: Provenance,
    ) -> Self {
        Self {
            id: ObjectId::new(),
            investigation_id,
            observation_id,
            summary: summary.into(),
            recorded_at,
            corrects: Some(corrects),
            confidence: Confidence::certain(),
            provenance,
            metadata: empty_metadata(),
        }
    }
}

/// Chronological Investigation timeline entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineEntry {
    /// Memory record identifier.
    pub memory_id: ObjectId,
    /// Observation identifier.
    pub observation_id: ObjectId,
    /// Event timestamp used for ordering.
    pub at: DateTime<Utc>,
    /// Summary text.
    pub summary: String,
    /// Source system.
    pub source: String,
}
