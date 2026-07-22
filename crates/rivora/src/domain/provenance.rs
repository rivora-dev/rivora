//! Provenance and shared metadata for Engineering Objects.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::ObjectId;

/// Confidence score in the inclusive range `[0.0, 1.0]`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Confidence(f64);

impl Confidence {
    /// Create a confidence value, clamping into `[0.0, 1.0]`.
    pub fn new(value: f64) -> Self {
        Self(value.clamp(0.0, 1.0))
    }

    /// Certainty (1.0).
    pub fn certain() -> Self {
        Self(1.0)
    }

    /// Neutral confidence (0.5).
    pub fn neutral() -> Self {
        Self(0.5)
    }

    /// Zero confidence.
    pub fn none() -> Self {
        Self(0.0)
    }

    /// Inner value.
    pub fn value(self) -> f64 {
        self.0
    }
}

impl Default for Confidence {
    fn default() -> Self {
        Self::neutral()
    }
}

/// Provenance for an Engineering Object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    /// Actor that created the object (user, connector, capability, runtime).
    pub actor: String,
    /// Source system or subsystem.
    pub source: String,
    /// Optional capability that produced the object.
    pub capability: Option<String>,
    /// Creation timestamp (UTC).
    pub created_at: DateTime<Utc>,
    /// Optional model identifier when AI assisted.
    pub model: Option<String>,
    /// Object schema version.
    pub version: u32,
    /// Supporting evidence object identifiers.
    pub supporting_evidence: Vec<ObjectId>,
}

impl Provenance {
    /// Create provenance with the current timestamp.
    pub fn now(actor: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            actor: actor.into(),
            source: source.into(),
            capability: None,
            created_at: Utc::now(),
            model: None,
            version: 1,
            supporting_evidence: Vec::new(),
        }
    }

    /// Attach a capability name.
    pub fn with_capability(mut self, capability: impl Into<String>) -> Self {
        self.capability = Some(capability.into());
        self
    }

    /// Attach supporting evidence identifiers.
    pub fn with_evidence(mut self, evidence: Vec<ObjectId>) -> Self {
        self.supporting_evidence = evidence;
        self
    }
}

/// Free-form metadata map.
pub type Metadata = serde_json::Map<String, serde_json::Value>;

/// Empty metadata map.
pub fn empty_metadata() -> Metadata {
    serde_json::Map::new()
}
