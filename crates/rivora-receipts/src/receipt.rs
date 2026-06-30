//! The top-level [`Receipt`] type and its associated convenience methods.

use serde::{Deserialize, Serialize};

use crate::action::SuggestedAction;
use crate::builders::ReceiptBuilder;
use crate::confidence::Confidence;
use crate::evidence::Evidence;
use crate::kind::ReceiptKind;
use crate::metadata::{
    AbilityRef, InferenceRef, ReceiptMetadata, ReceiptProvenance, ReceiptTimestamps, ReceiptVersion,
};
use crate::reasoning::ReasoningStep;
use crate::risk::Risk;
use crate::status::ReceiptStatus;
use crate::subject::{ReceiptSubject, ReceiptSummary};

/// A typed reliability receipt.
///
/// A receipt is the canonical, validated artifact that explains *why* Open
/// Rivora reached a conclusion. Receipts are immutable once validated.
///
/// # Validation
///
/// Not every `Receipt` value is necessarily valid. Use
/// [`crate::validation::validate_receipt`] to check that all invariants
/// hold. Invalid receipts must not be surfaced to engineers.
///
/// # Example
///
/// ```rust
/// use rivora_receipts::{
///     ActionKind, ApprovalRequirement, Confidence, Evidence, EvidenceKind, EvidenceSource,
///     Receipt, ReceiptKind, ReceiptStatus, ReceiptSubject, ReceiptSummary, ReasoningStep,
///     ReceiptProvenance, ReceiptTimestamps, ReceiptVersion, Risk, RiskLevel,
///     SuggestedAction,
/// };
/// use rivora_types::{NonEmptyString, Version};
///
/// let evidence = vec![Evidence::new(
///     EvidenceKind::Metric,
///     EvidenceSource {
///         provider: NonEmptyString::new("aws").unwrap(),
///         version: NonEmptyString::new("0.1.0").unwrap(),
///     },
///     "CPU spike",
///     "CPU exceeded 90% for 5 minutes",
///     "2026-06-25T12:00:00Z",
///     0.8,
/// )
/// .unwrap()];
///
/// let reasoning = vec![ReasoningStep::new(
///     1,
///     "Detect anomaly",
///     "CPU exceeded threshold",
///     "Likely deploy-induced",
///     0.3,
/// )
/// .unwrap()];
///
/// let receipt = Receipt::builder()
///     .id("receipt_test_1")
///     .kind(ReceiptKind::IncidentExplanation)
///     .status(ReceiptStatus::Draft)
///     .subject(ReceiptSubject::new("service", "svc-1", "api-gateway").unwrap())
///     .summary(ReceiptSummary::new("Latency spike", "Latency increased 3x").unwrap())
///     .evidence(evidence)
///     .reasoning(reasoning)
///     .confidence(Confidence::new(0.85, "method-v1", "Limited data").unwrap())
///     .risk(Risk::new(RiskLevel::Medium, "Service degradation possible").unwrap())
///     .provenance(ReceiptProvenance::new("adaptive-engine", "0.1.0").unwrap())
///     .timestamps(ReceiptTimestamps::new("2026-06-25T12:00:00Z").unwrap())
///     .version(ReceiptVersion::new(Version::new(1, 0, 0)))
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Receipt {
    /// Unique receipt identifier.
    pub id: crate::ReceiptId,
    /// The kind of receipt.
    pub kind: ReceiptKind,
    /// The lifecycle status.
    pub status: ReceiptStatus,
    /// Structured metadata (tags, labels).
    pub metadata: ReceiptMetadata,
    /// What the receipt is about.
    pub subject: ReceiptSubject,
    /// A human-readable summary.
    pub summary: ReceiptSummary,
    /// At least one piece of evidence. A receipt with zero evidence is
    /// invalid.
    pub evidence: Vec<Evidence>,
    /// Ordered reasoning steps. A receipt with zero reasoning is invalid.
    pub reasoning: Vec<ReasoningStep>,
    /// Typed confidence value.
    pub confidence: Confidence,
    /// Risk assessment.
    pub risk: Risk,
    /// Proposed actions. May be empty.
    pub suggested_actions: Vec<SuggestedAction>,
    /// Provenance — who or what produced this receipt.
    pub provenance: ReceiptProvenance,
    /// Timestamps.
    pub timestamps: ReceiptTimestamps,
    /// Schema and API version.
    pub version: ReceiptVersion,
    /// Optional reference to the inference provider that contributed.
    /// Convenience accessor for `provenance.inference`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub inference: Option<InferenceRef>,
    /// Optional reference to the Ability that produced this receipt.
    /// Convenience accessor for `provenance.ability`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub ability: Option<AbilityRef>,
}

impl Receipt {
    /// Creates a new [`ReceiptBuilder`] for constructing a receipt.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rivora_receipts::{Receipt, ReceiptKind, ReceiptStatus};
    /// let builder = Receipt::builder()
    ///     .id("receipt_1")
    ///     .kind(ReceiptKind::Observation)
    ///     .status(ReceiptStatus::Draft);
    /// ```
    #[must_use]
    pub fn builder() -> ReceiptBuilder {
        ReceiptBuilder::new()
    }

    /// Returns `true` if this receipt has at least one piece of evidence.
    #[must_use]
    pub fn has_evidence(&self) -> bool {
        !self.evidence.is_empty()
    }

    /// Returns `true` if this receipt has at least one reasoning step.
    #[must_use]
    pub fn has_reasoning(&self) -> bool {
        !self.reasoning.is_empty()
    }

    /// Returns `true` if this receipt has any mutating suggested actions.
    #[must_use]
    pub fn has_mutating_actions(&self) -> bool {
        self.suggested_actions
            .iter()
            .any(|a| a.mutates_infrastructure)
    }
}
