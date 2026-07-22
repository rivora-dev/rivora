//! Knowledge objects — derived understanding (RFC-007).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{empty_metadata, Confidence, InvestigationId, Metadata, ObjectId, Provenance};

/// Knowledge derived from Investigation Memory.
///
/// Knowledge is never a second source of truth; Memory remains authoritative.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeObject {
    /// Stable object identifier.
    pub id: ObjectId,
    /// Primary Investigation.
    pub investigation_id: InvestigationId,
    /// Derived understanding summary.
    pub summary: String,
    /// Kind of derived knowledge.
    pub kind: KnowledgeKind,
    /// Memory records supporting this Knowledge.
    pub supporting_memory_ids: Vec<ObjectId>,
    /// Confidence in the derivation.
    pub confidence: Confidence,
    /// When Knowledge was derived.
    pub derived_at: DateTime<Utc>,
    /// Derivation metadata (method, rules used).
    pub derivation: DerivationMetadata,
    /// Provenance.
    pub provenance: Provenance,
    /// Metadata.
    pub metadata: Metadata,
}

/// Classification of derived Knowledge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeKind {
    /// High-level summary of Investigation Memory.
    Summary,
    /// Pattern detected across Memory.
    Pattern,
    /// Relationship between Memory records.
    Relationship,
    /// Risk-related understanding.
    RiskSignal,
    /// Activity classification.
    Activity,
}

impl KnowledgeKind {
    /// Stable string form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::Pattern => "pattern",
            Self::Relationship => "relationship",
            Self::RiskSignal => "risk_signal",
            Self::Activity => "activity",
        }
    }
}

/// How Knowledge was derived.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DerivationMetadata {
    /// Deterministic method identifier.
    pub method: String,
    /// Human-readable explanation of derivation.
    pub explanation: String,
}

impl KnowledgeObject {
    /// Construct a Knowledge object.
    pub fn new(
        investigation_id: InvestigationId,
        summary: impl Into<String>,
        kind: KnowledgeKind,
        supporting_memory_ids: Vec<ObjectId>,
        confidence: Confidence,
        derivation: DerivationMetadata,
        provenance: Provenance,
    ) -> Self {
        Self {
            id: ObjectId::new(),
            investigation_id,
            summary: summary.into(),
            kind,
            supporting_memory_ids,
            confidence,
            derived_at: Utc::now(),
            derivation,
            provenance,
            metadata: empty_metadata(),
        }
    }
}
