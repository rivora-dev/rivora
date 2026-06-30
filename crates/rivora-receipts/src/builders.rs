//! Ergonomic builders for constructing receipts and their components.

use rivora_errors::RivoraError;
use rivora_types::NonEmptyString;

use crate::action::{ActionKind, ApprovalRequirement, HumanApproval, SuggestedAction};
use crate::confidence::Confidence;
use crate::evidence::{Evidence, EvidenceKind, EvidenceSource};
use crate::kind::ReceiptKind;
use crate::metadata::{
    AbilityRef, InferenceRef, ReceiptMetadata, ReceiptProvenance, ReceiptTimestamps, ReceiptVersion,
};
use crate::reasoning::ReasoningStep;
use crate::receipt::Receipt;
use crate::risk::{Risk, RiskLevel};
use crate::status::ReceiptStatus;
use crate::subject::{ReceiptSubject, ReceiptSummary};

/// A builder for [`Receipt`].
///
/// All required fields start as `None` and are validated in [`build`].
/// Optional fields default to sensible values: `metadata` defaults to empty,
/// `suggested_actions` defaults to an empty vector, and `inference` and
/// `ability` default to `None`.
///
/// When `inference` or `ability` are set, they are also propagated into the
/// corresponding fields on `provenance` during [`build`].
///
/// [`build`]: ReceiptBuilder::build
#[derive(Debug, Clone, Default)]
pub struct ReceiptBuilder {
    id: Option<crate::ReceiptId>,
    kind: Option<ReceiptKind>,
    status: Option<ReceiptStatus>,
    metadata: Option<ReceiptMetadata>,
    subject: Option<ReceiptSubject>,
    summary: Option<ReceiptSummary>,
    evidence: Option<Vec<Evidence>>,
    reasoning: Option<Vec<ReasoningStep>>,
    confidence: Option<Confidence>,
    risk: Option<Risk>,
    suggested_actions: Option<Vec<SuggestedAction>>,
    provenance: Option<ReceiptProvenance>,
    timestamps: Option<ReceiptTimestamps>,
    version: Option<ReceiptVersion>,
    inference: Option<InferenceRef>,
    ability: Option<AbilityRef>,
}

impl ReceiptBuilder {
    /// Creates a new, empty `ReceiptBuilder`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the receipt id. Accepts a `&str`, `String`, or `ReceiptId`.
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(rivora_core::ReceiptId::new_unchecked(id.into()));
        self
    }

    #[must_use]
    pub fn kind(mut self, kind: ReceiptKind) -> Self {
        self.kind = Some(kind);
        self
    }

    #[must_use]
    pub fn status(mut self, status: ReceiptStatus) -> Self {
        self.status = Some(status);
        self
    }

    #[must_use]
    pub fn metadata(mut self, metadata: ReceiptMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    #[must_use]
    pub fn subject(mut self, subject: ReceiptSubject) -> Self {
        self.subject = Some(subject);
        self
    }

    #[must_use]
    pub fn summary(mut self, summary: ReceiptSummary) -> Self {
        self.summary = Some(summary);
        self
    }

    #[must_use]
    pub fn evidence(mut self, evidence: Vec<Evidence>) -> Self {
        self.evidence = Some(evidence);
        self
    }

    #[must_use]
    pub fn reasoning(mut self, reasoning: Vec<ReasoningStep>) -> Self {
        self.reasoning = Some(reasoning);
        self
    }

    #[must_use]
    pub fn confidence(mut self, confidence: Confidence) -> Self {
        self.confidence = Some(confidence);
        self
    }

    #[must_use]
    pub fn risk(mut self, risk: Risk) -> Self {
        self.risk = Some(risk);
        self
    }

    #[must_use]
    pub fn suggested_actions(mut self, actions: Vec<SuggestedAction>) -> Self {
        self.suggested_actions = Some(actions);
        self
    }

    #[must_use]
    pub fn provenance(mut self, provenance: ReceiptProvenance) -> Self {
        self.provenance = Some(provenance);
        self
    }

    #[must_use]
    pub fn timestamps(mut self, timestamps: ReceiptTimestamps) -> Self {
        self.timestamps = Some(timestamps);
        self
    }

    #[must_use]
    pub fn version(mut self, version: ReceiptVersion) -> Self {
        self.version = Some(version);
        self
    }

    #[must_use]
    pub fn inference(mut self, inference: InferenceRef) -> Self {
        self.inference = Some(inference);
        self
    }

    #[must_use]
    pub fn ability(mut self, ability: AbilityRef) -> Self {
        self.ability = Some(ability);
        self
    }

    /// Builds a [`Receipt`] from the configured fields.
    ///
    /// # Errors
    ///
    /// Returns an error if any required field is missing.
    pub fn build(self) -> Result<Receipt, RivoraError> {
        let id = self
            .id
            .ok_or_else(|| RivoraError::receipt("id is required"))?;
        let kind = self
            .kind
            .ok_or_else(|| RivoraError::receipt("kind is required"))?;
        let status = self
            .status
            .ok_or_else(|| RivoraError::receipt("status is required"))?;
        let metadata = self.metadata.unwrap_or_default();
        let subject = self
            .subject
            .ok_or_else(|| RivoraError::receipt("subject is required"))?;
        let summary = self
            .summary
            .ok_or_else(|| RivoraError::receipt("summary is required"))?;
        let evidence = self
            .evidence
            .ok_or_else(|| RivoraError::receipt("evidence is required"))?;
        let reasoning = self
            .reasoning
            .ok_or_else(|| RivoraError::receipt("reasoning is required"))?;
        let confidence = self
            .confidence
            .ok_or_else(|| RivoraError::receipt("confidence is required"))?;
        let risk = self
            .risk
            .ok_or_else(|| RivoraError::receipt("risk is required"))?;
        let suggested_actions = self.suggested_actions.unwrap_or_default();
        let mut provenance = self
            .provenance
            .ok_or_else(|| RivoraError::receipt("provenance is required"))?;
        let timestamps = self
            .timestamps
            .ok_or_else(|| RivoraError::receipt("timestamps is required"))?;
        let version = self
            .version
            .ok_or_else(|| RivoraError::receipt("version is required"))?;
        let inference = self.inference;
        let ability = self.ability;

        if let Some(ref inf) = inference {
            provenance.inference = Some(inf.clone());
        }
        if let Some(ref ab) = ability {
            provenance.ability = Some(ab.clone());
        }

        Ok(Receipt {
            id,
            kind,
            status,
            metadata,
            subject,
            summary,
            evidence,
            reasoning,
            confidence,
            risk,
            suggested_actions,
            provenance,
            timestamps,
            version,
            inference,
            ability,
        })
    }
}

/// A builder for [`Evidence`].
///
/// Wraps [`Evidence::new`] and provides fluent setters for the optional
/// `raw_ref` and `metadata` fields.
#[derive(Debug, Clone, Default)]
pub struct EvidenceBuilder {
    kind: Option<EvidenceKind>,
    source: Option<EvidenceSource>,
    title: Option<String>,
    description: Option<String>,
    observed_at: Option<String>,
    confidence_contribution: Option<f64>,
    raw_ref: Option<String>,
    metadata: Option<serde_json::Value>,
}

impl EvidenceBuilder {
    /// Creates a new, empty `EvidenceBuilder`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn kind(mut self, kind: EvidenceKind) -> Self {
        self.kind = Some(kind);
        self
    }

    #[must_use]
    pub fn source(mut self, source: EvidenceSource) -> Self {
        self.source = Some(source);
        self
    }

    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    #[must_use]
    pub fn observed_at(mut self, observed_at: impl Into<String>) -> Self {
        self.observed_at = Some(observed_at.into());
        self
    }

    #[must_use]
    pub fn confidence_contribution(mut self, confidence_contribution: f64) -> Self {
        self.confidence_contribution = Some(confidence_contribution);
        self
    }

    #[must_use]
    pub fn raw_ref(mut self, raw_ref: impl Into<String>) -> Self {
        self.raw_ref = Some(raw_ref.into());
        self
    }

    #[must_use]
    pub fn metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Builds an [`Evidence`] from the configured fields.
    ///
    /// # Errors
    ///
    /// Returns an error if any required field is missing, or if the
    /// underlying [`Evidence::new`] validation fails (e.g. confidence out of
    /// range or empty strings).
    pub fn build(self) -> Result<Evidence, RivoraError> {
        let mut evidence = Evidence::new(
            self.kind
                .ok_or_else(|| RivoraError::invalid_value("kind", "is required"))?,
            self.source
                .ok_or_else(|| RivoraError::invalid_value("source", "is required"))?,
            self.title
                .ok_or_else(|| RivoraError::invalid_value("title", "is required"))?,
            self.description
                .ok_or_else(|| RivoraError::invalid_value("description", "is required"))?,
            self.observed_at
                .ok_or_else(|| RivoraError::invalid_value("observed_at", "is required"))?,
            self.confidence_contribution.ok_or_else(|| {
                RivoraError::invalid_value("confidence_contribution", "is required")
            })?,
        )?;
        if let Some(raw_ref) = self.raw_ref {
            evidence = evidence.with_raw_ref(raw_ref);
        }
        if let Some(metadata) = self.metadata {
            evidence = evidence.with_metadata(metadata);
        }
        Ok(evidence)
    }
}

/// A builder for [`ReasoningStep`].
///
/// Wraps [`ReasoningStep::new`] and provides a fluent setter for the optional
/// `input_evidence` field.
#[derive(Debug, Clone, Default)]
pub struct ReasoningStepBuilder {
    step: Option<u32>,
    title: Option<String>,
    explanation: Option<String>,
    output_conclusion: Option<String>,
    confidence_impact: Option<f64>,
    input_evidence: Option<Vec<NonEmptyString>>,
}

impl ReasoningStepBuilder {
    /// Creates a new, empty `ReasoningStepBuilder`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn step(mut self, step: u32) -> Self {
        self.step = Some(step);
        self
    }

    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    #[must_use]
    pub fn explanation(mut self, explanation: impl Into<String>) -> Self {
        self.explanation = Some(explanation.into());
        self
    }

    #[must_use]
    pub fn output_conclusion(mut self, output_conclusion: impl Into<String>) -> Self {
        self.output_conclusion = Some(output_conclusion.into());
        self
    }

    #[must_use]
    pub fn confidence_impact(mut self, confidence_impact: f64) -> Self {
        self.confidence_impact = Some(confidence_impact);
        self
    }

    #[must_use]
    pub fn input_evidence(mut self, input_evidence: Vec<NonEmptyString>) -> Self {
        self.input_evidence = Some(input_evidence);
        self
    }

    /// Builds a [`ReasoningStep`] from the configured fields.
    ///
    /// # Errors
    ///
    /// Returns an error if any required field is missing, or if the
    /// underlying [`ReasoningStep::new`] validation fails.
    pub fn build(self) -> Result<ReasoningStep, RivoraError> {
        let mut step = ReasoningStep::new(
            self.step
                .ok_or_else(|| RivoraError::invalid_value("step", "is required"))?,
            self.title
                .ok_or_else(|| RivoraError::invalid_value("title", "is required"))?,
            self.explanation
                .ok_or_else(|| RivoraError::invalid_value("explanation", "is required"))?,
            self.output_conclusion
                .ok_or_else(|| RivoraError::invalid_value("output_conclusion", "is required"))?,
            self.confidence_impact
                .ok_or_else(|| RivoraError::invalid_value("confidence_impact", "is required"))?,
        )?;
        if let Some(input_evidence) = self.input_evidence {
            step = step.with_input_evidence(input_evidence);
        }
        Ok(step)
    }
}

/// A builder for [`SuggestedAction`].
///
/// Wraps [`SuggestedAction::new`] and provides fluent setters for `scope`,
/// `approval`, `human_approval`, and `rollback_strategy`.
#[derive(Debug, Clone, Default)]
pub struct SuggestedActionBuilder {
    kind: Option<ActionKind>,
    title: Option<String>,
    description: Option<String>,
    expected_outcome: Option<String>,
    risk_level: Option<RiskLevel>,
    scope: Option<Vec<NonEmptyString>>,
    approval: Option<ApprovalRequirement>,
    human_approval: Option<HumanApproval>,
    rollback_strategy: Option<String>,
}

impl SuggestedActionBuilder {
    /// Creates a new, empty `SuggestedActionBuilder`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn kind(mut self, kind: ActionKind) -> Self {
        self.kind = Some(kind);
        self
    }

    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    #[must_use]
    pub fn expected_outcome(mut self, expected_outcome: impl Into<String>) -> Self {
        self.expected_outcome = Some(expected_outcome.into());
        self
    }

    #[must_use]
    pub fn risk_level(mut self, risk_level: RiskLevel) -> Self {
        self.risk_level = Some(risk_level);
        self
    }

    #[must_use]
    pub fn scope(mut self, scope: Vec<NonEmptyString>) -> Self {
        self.scope = Some(scope);
        self
    }

    #[must_use]
    pub fn approval(mut self, approval: ApprovalRequirement) -> Self {
        self.approval = Some(approval);
        self
    }

    #[must_use]
    pub fn human_approval(mut self, human_approval: HumanApproval) -> Self {
        self.human_approval = Some(human_approval);
        self
    }

    #[must_use]
    pub fn rollback_strategy(mut self, rollback_strategy: impl Into<String>) -> Self {
        self.rollback_strategy = Some(rollback_strategy.into());
        self
    }

    /// Builds a [`SuggestedAction`] from the configured fields.
    ///
    /// # Errors
    ///
    /// Returns an error if any required field is missing, or if the
    /// underlying [`SuggestedAction::new`] validation fails.
    pub fn build(self) -> Result<SuggestedAction, RivoraError> {
        let mut action = SuggestedAction::new(
            self.kind
                .ok_or_else(|| RivoraError::invalid_value("kind", "is required"))?,
            self.title
                .ok_or_else(|| RivoraError::invalid_value("title", "is required"))?,
            self.description
                .ok_or_else(|| RivoraError::invalid_value("description", "is required"))?,
            self.expected_outcome
                .ok_or_else(|| RivoraError::invalid_value("expected_outcome", "is required"))?,
            self.risk_level
                .ok_or_else(|| RivoraError::invalid_value("risk_level", "is required"))?,
        )?;
        if let Some(scope) = self.scope {
            action = action.with_scope(scope);
        }
        if let Some(approval) = self.approval {
            action = action.with_approval(approval);
        }
        if let Some(human_approval) = self.human_approval {
            action = action.with_human_approval(human_approval);
        }
        if let Some(rollback_strategy) = self.rollback_strategy {
            action = action.with_rollback_strategy(rollback_strategy);
        }
        Ok(action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::{EvidenceKind, EvidenceSource};
    use crate::metadata::{
        AbilityRef, InferenceRef, ReceiptProvenance, ReceiptTimestamps, ReceiptVersion,
    };
    use rivora_core::ReceiptId;
    use rivora_types::{NonEmptyString, Version};
    use std::collections::BTreeMap;

    fn evidence_source() -> EvidenceSource {
        EvidenceSource {
            provider: NonEmptyString::new("aws").unwrap(),
            version: NonEmptyString::new("0.1.0").unwrap(),
        }
    }

    fn sample_evidence() -> Evidence {
        Evidence::new(
            EvidenceKind::Metric,
            evidence_source(),
            "CPU spike",
            "CPU exceeded 90% for 5 minutes",
            "2026-06-25T12:00:00Z",
            0.8,
        )
        .unwrap()
    }

    fn sample_reasoning() -> ReasoningStep {
        ReasoningStep::new(
            1,
            "Detect anomaly",
            "CPU exceeded threshold",
            "Likely deploy-induced",
            0.3,
        )
        .unwrap()
    }

    fn minimal_builder() -> ReceiptBuilder {
        ReceiptBuilder::new()
            .id("receipt_test_1")
            .kind(ReceiptKind::IncidentExplanation)
            .status(ReceiptStatus::Draft)
            .subject(ReceiptSubject::new("service", "svc-1", "api-gateway").unwrap())
            .summary(ReceiptSummary::new("Latency spike", "Latency increased 3x").unwrap())
            .evidence(vec![sample_evidence()])
            .reasoning(vec![sample_reasoning()])
            .confidence(Confidence::new(0.85, "method-v1", "Limited data").unwrap())
            .risk(Risk::new(RiskLevel::Low, "Minor risk").unwrap())
            .provenance(ReceiptProvenance::new("adaptive-engine", "0.1.0").unwrap())
            .timestamps(ReceiptTimestamps::new("2026-06-25T12:00:00Z").unwrap())
            .version(ReceiptVersion::new(Version::new(1, 0, 0)))
    }

    // --- ReceiptBuilder tests ---

    #[test]
    fn build_minimal_receipt() {
        let receipt = minimal_builder().build().unwrap();
        assert_eq!(receipt.id.as_str(), "receipt_test_1");
        assert_eq!(receipt.kind, ReceiptKind::IncidentExplanation);
        assert_eq!(receipt.status, ReceiptStatus::Draft);
        assert!(receipt.has_evidence());
        assert!(receipt.has_reasoning());
        assert!(!receipt.has_mutating_actions());
        assert!(receipt.metadata.tags.is_empty());
        assert!(receipt.metadata.labels.is_empty());
        assert!(receipt.suggested_actions.is_empty());
        assert!(receipt.inference.is_none());
        assert!(receipt.ability.is_none());
        assert!(receipt.provenance.inference.is_none());
        assert!(receipt.provenance.ability.is_none());
    }

    #[test]
    fn build_missing_id_fails() {
        let mut builder = minimal_builder();
        builder.id = None;
        let err = builder.build().unwrap_err();
        assert!(err.to_string().contains("id"));
    }

    #[test]
    fn build_missing_evidence_fails() {
        let mut builder = minimal_builder();
        builder.evidence = None;
        let err = builder.build().unwrap_err();
        assert!(err.to_string().contains("evidence"));
    }

    #[test]
    fn build_with_all_fields() {
        let inference =
            InferenceRef::new("anthropic", "claude-opus-4", "20250101", 0.0, "req-1").unwrap();
        let ability = AbilityRef::new("payment-validator", "1.0.0", "approved").unwrap();

        let mut labels = BTreeMap::new();
        labels.insert(
            NonEmptyString::new("env").unwrap(),
            NonEmptyString::new("prod").unwrap(),
        );
        let metadata = ReceiptMetadata::new()
            .with_tags(vec![NonEmptyString::new("payments").unwrap()])
            .with_labels(labels);

        let action = SuggestedAction::new(
            ActionKind::Rollback,
            "Rollback payment-service",
            "Rollback to v1.2.3",
            "Latency returns to normal",
            RiskLevel::Medium,
        )
        .unwrap()
        .with_rollback_strategy("redeploy prior image");

        let receipt = ReceiptBuilder::new()
            .id(ReceiptId::new_unchecked("receipt_all_fields"))
            .kind(ReceiptKind::Recommendation)
            .status(ReceiptStatus::Valid)
            .metadata(metadata)
            .subject(ReceiptSubject::new("service", "svc-1", "api-gateway").unwrap())
            .summary(ReceiptSummary::new("Latency spike", "Latency increased 3x").unwrap())
            .evidence(vec![sample_evidence()])
            .reasoning(vec![sample_reasoning()])
            .confidence(Confidence::new(0.9, "method-v2", "Well-supported").unwrap())
            .risk(Risk::new(RiskLevel::Medium, "Service degradation possible").unwrap())
            .suggested_actions(vec![action])
            .provenance(ReceiptProvenance::new("adaptive-engine", "0.1.0").unwrap())
            .timestamps(ReceiptTimestamps::new("2026-06-25T12:00:00Z").unwrap())
            .version(ReceiptVersion::new(Version::new(1, 0, 0)).with_api("v1"))
            .inference(inference.clone())
            .ability(ability.clone())
            .build()
            .unwrap();

        assert_eq!(receipt.id.as_str(), "receipt_all_fields");
        assert_eq!(receipt.kind, ReceiptKind::Recommendation);
        assert_eq!(receipt.status, ReceiptStatus::Valid);
        assert_eq!(receipt.metadata.tags.len(), 1);
        assert_eq!(receipt.metadata.labels.len(), 1);
        assert_eq!(receipt.suggested_actions.len(), 1);
        assert!(receipt.has_mutating_actions());
        assert_eq!(receipt.inference.as_ref(), Some(&inference));
        assert_eq!(receipt.ability.as_ref(), Some(&ability));
        assert_eq!(receipt.provenance.inference.as_ref(), Some(&inference));
        assert_eq!(receipt.provenance.ability.as_ref(), Some(&ability));
        assert!(receipt.version.api.is_some());
    }

    #[test]
    fn build_without_optional_fields_uses_defaults() {
        let receipt = minimal_builder().build().unwrap();
        assert!(receipt.metadata.tags.is_empty());
        assert!(receipt.suggested_actions.is_empty());
        assert!(receipt.inference.is_none());
        assert!(receipt.ability.is_none());
    }

    #[test]
    fn build_missing_kind_fails() {
        let mut builder = minimal_builder();
        builder.kind = None;
        assert!(builder.build().is_err());
    }

    #[test]
    fn build_missing_subject_fails() {
        let mut builder = minimal_builder();
        builder.subject = None;
        assert!(builder.build().is_err());
    }

    #[test]
    fn build_missing_provenance_fails() {
        let mut builder = minimal_builder();
        builder.provenance = None;
        assert!(builder.build().is_err());
    }

    #[test]
    fn id_setter_accepts_string() {
        let receipt = ReceiptBuilder::new()
            .id("receipt_str".to_string())
            .kind(ReceiptKind::Observation)
            .status(ReceiptStatus::Draft)
            .subject(ReceiptSubject::new("service", "svc-1", "api-gateway").unwrap())
            .summary(ReceiptSummary::new("Title", "Description").unwrap())
            .evidence(vec![sample_evidence()])
            .reasoning(vec![sample_reasoning()])
            .confidence(Confidence::new(0.5, "method-v1", "Uncertain").unwrap())
            .risk(Risk::new(RiskLevel::Low, "Low risk").unwrap())
            .provenance(ReceiptProvenance::new("adaptive-engine", "0.1.0").unwrap())
            .timestamps(ReceiptTimestamps::new("2026-06-25T12:00:00Z").unwrap())
            .version(ReceiptVersion::new(Version::new(1, 0, 0)))
            .build()
            .unwrap();
        assert_eq!(receipt.id.as_str(), "receipt_str");
    }

    #[test]
    fn id_setter_accepts_receipt_id() {
        let receipt = ReceiptBuilder::new()
            .id(ReceiptId::new_unchecked("receipt_typed"))
            .kind(ReceiptKind::Observation)
            .status(ReceiptStatus::Draft)
            .subject(ReceiptSubject::new("service", "svc-1", "api-gateway").unwrap())
            .summary(ReceiptSummary::new("Title", "Description").unwrap())
            .evidence(vec![sample_evidence()])
            .reasoning(vec![sample_reasoning()])
            .confidence(Confidence::new(0.5, "method-v1", "Uncertain").unwrap())
            .risk(Risk::new(RiskLevel::Low, "Low risk").unwrap())
            .provenance(ReceiptProvenance::new("adaptive-engine", "0.1.0").unwrap())
            .timestamps(ReceiptTimestamps::new("2026-06-25T12:00:00Z").unwrap())
            .version(ReceiptVersion::new(Version::new(1, 0, 0)))
            .build()
            .unwrap();
        assert_eq!(receipt.id.as_str(), "receipt_typed");
    }

    #[test]
    fn receipt_builder_default_is_empty() {
        let builder = ReceiptBuilder::default();
        assert!(builder.id.is_none());
        assert!(builder.kind.is_none());
        assert!(builder.status.is_none());
        assert!(builder.metadata.is_none());
        assert!(builder.evidence.is_none());
        assert!(builder.reasoning.is_none());
        assert!(builder.confidence.is_none());
        assert!(builder.risk.is_none());
        assert!(builder.suggested_actions.is_none());
        assert!(builder.provenance.is_none());
        assert!(builder.timestamps.is_none());
        assert!(builder.version.is_none());
        assert!(builder.inference.is_none());
        assert!(builder.ability.is_none());
    }

    // --- EvidenceBuilder tests ---

    #[test]
    fn evidence_builder_with_all_fields() {
        let evidence = EvidenceBuilder::new()
            .kind(EvidenceKind::Metric)
            .source(evidence_source())
            .title("CPU spike")
            .description("CPU exceeded 90%")
            .observed_at("2026-06-25T12:00:00Z")
            .confidence_contribution(0.8)
            .raw_ref("arn:aws:ecs:us-east-1:123:service/api")
            .metadata(serde_json::json!({"region": "us-east-1"}))
            .build()
            .unwrap();

        assert_eq!(evidence.kind, EvidenceKind::Metric);
        assert_eq!(evidence.confidence_contribution, 0.8);
        assert_eq!(
            evidence.raw_ref.as_deref(),
            Some("arn:aws:ecs:us-east-1:123:service/api")
        );
        assert!(evidence.metadata.is_some());
        assert_eq!(evidence.metadata.unwrap()["region"], "us-east-1");
    }

    #[test]
    fn evidence_builder_without_optional_fields() {
        let evidence = EvidenceBuilder::new()
            .kind(EvidenceKind::Observation)
            .source(evidence_source())
            .title("title")
            .description("description")
            .observed_at("2026-01-01T00:00:00Z")
            .confidence_contribution(0.5)
            .build()
            .unwrap();

        assert!(evidence.raw_ref.is_none());
        assert!(evidence.metadata.is_none());
    }

    #[test]
    fn evidence_builder_missing_kind_fails() {
        let result = EvidenceBuilder::new()
            .source(evidence_source())
            .title("title")
            .description("description")
            .observed_at("2026-01-01T00:00:00Z")
            .confidence_contribution(0.5)
            .build();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("kind"));
    }

    #[test]
    fn evidence_builder_missing_source_fails() {
        let result = EvidenceBuilder::new()
            .kind(EvidenceKind::Metric)
            .title("title")
            .description("description")
            .observed_at("2026-01-01T00:00:00Z")
            .confidence_contribution(0.5)
            .build();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("source"));
    }

    #[test]
    fn evidence_builder_validates_confidence_range() {
        let result = EvidenceBuilder::new()
            .kind(EvidenceKind::Metric)
            .source(evidence_source())
            .title("title")
            .description("description")
            .observed_at("2026-01-01T00:00:00Z")
            .confidence_contribution(1.5)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn evidence_builder_default_is_empty() {
        let builder = EvidenceBuilder::default();
        assert!(builder.kind.is_none());
        assert!(builder.source.is_none());
        assert!(builder.raw_ref.is_none());
        assert!(builder.metadata.is_none());
    }

    // --- ReasoningStepBuilder tests ---

    #[test]
    fn reasoning_step_builder_with_all_fields() {
        let step = ReasoningStepBuilder::new()
            .step(1)
            .title("Detect anomaly")
            .explanation("CPU exceeded threshold")
            .output_conclusion("Likely deploy-induced")
            .confidence_impact(0.3)
            .input_evidence(vec![
                NonEmptyString::new("ev-1").unwrap(),
                NonEmptyString::new("ev-2").unwrap(),
            ])
            .build()
            .unwrap();

        assert_eq!(step.step, 1);
        assert_eq!(step.confidence_impact, 0.3);
        assert_eq!(step.input_evidence.len(), 2);
    }

    #[test]
    fn reasoning_step_builder_without_input_evidence() {
        let step = ReasoningStepBuilder::new()
            .step(1)
            .title("Detect")
            .explanation("explanation")
            .output_conclusion("conclusion")
            .confidence_impact(0.0)
            .build()
            .unwrap();

        assert!(step.input_evidence.is_empty());
    }

    #[test]
    fn reasoning_step_builder_missing_step_fails() {
        let result = ReasoningStepBuilder::new()
            .title("Detect")
            .explanation("explanation")
            .output_conclusion("conclusion")
            .confidence_impact(0.0)
            .build();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("step"));
    }

    #[test]
    fn reasoning_step_builder_validates_step_zero() {
        let result = ReasoningStepBuilder::new()
            .step(0)
            .title("Detect")
            .explanation("explanation")
            .output_conclusion("conclusion")
            .confidence_impact(0.0)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn reasoning_step_builder_validates_confidence_impact_range() {
        let result = ReasoningStepBuilder::new()
            .step(1)
            .title("Detect")
            .explanation("explanation")
            .output_conclusion("conclusion")
            .confidence_impact(2.0)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn reasoning_step_builder_default_is_empty() {
        let builder = ReasoningStepBuilder::default();
        assert!(builder.step.is_none());
        assert!(builder.title.is_none());
        assert!(builder.input_evidence.is_none());
    }

    // --- SuggestedActionBuilder tests ---

    #[test]
    fn suggested_action_builder_with_all_fields() {
        let human_approval = HumanApproval::new(
            ApprovalRequirement::Required,
            "team-lead",
            "mutating action on production",
        )
        .unwrap();

        let action = SuggestedActionBuilder::new()
            .kind(ActionKind::Rollback)
            .title("Rollback payment-service")
            .description("Rollback to v1.2.3")
            .expected_outcome("Latency returns to normal")
            .risk_level(RiskLevel::Medium)
            .scope(vec![NonEmptyString::new("payment-service").unwrap()])
            .approval(ApprovalRequirement::Required)
            .human_approval(human_approval.clone())
            .rollback_strategy("redeploy prior image")
            .build()
            .unwrap();

        assert_eq!(action.kind, ActionKind::Rollback);
        assert!(action.mutates_infrastructure);
        assert_eq!(action.scope.len(), 1);
        assert_eq!(action.approval, ApprovalRequirement::Required);
        assert_eq!(action.human_approval.as_ref(), Some(&human_approval));
        assert!(action.rollback_strategy.is_some());
    }

    #[test]
    fn suggested_action_builder_without_optional_fields() {
        let action = SuggestedActionBuilder::new()
            .kind(ActionKind::Read)
            .title("View logs")
            .description("Show service logs")
            .expected_outcome("Logs displayed")
            .risk_level(RiskLevel::Low)
            .build()
            .unwrap();

        assert!(!action.mutates_infrastructure);
        assert!(action.scope.is_empty());
        assert_eq!(action.approval, ApprovalRequirement::NotRequired);
        assert!(action.human_approval.is_none());
        assert!(action.rollback_strategy.is_none());
    }

    #[test]
    fn suggested_action_builder_missing_kind_fails() {
        let result = SuggestedActionBuilder::new()
            .title("View logs")
            .description("Show service logs")
            .expected_outcome("Logs displayed")
            .risk_level(RiskLevel::Low)
            .build();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("kind"));
    }

    #[test]
    fn suggested_action_builder_missing_risk_level_fails() {
        let result = SuggestedActionBuilder::new()
            .kind(ActionKind::Read)
            .title("View logs")
            .description("Show service logs")
            .expected_outcome("Logs displayed")
            .build();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("risk_level"));
    }

    #[test]
    fn suggested_action_builder_mutating_gets_required_approval() {
        let action = SuggestedActionBuilder::new()
            .kind(ActionKind::Scale)
            .title("Scale up")
            .description("Increase replicas")
            .expected_outcome("More capacity")
            .risk_level(RiskLevel::Medium)
            .build()
            .unwrap();

        assert!(action.mutates_infrastructure);
        assert_eq!(action.approval, ApprovalRequirement::Required);
    }

    #[test]
    fn suggested_action_builder_default_is_empty() {
        let builder = SuggestedActionBuilder::default();
        assert!(builder.kind.is_none());
        assert!(builder.title.is_none());
        assert!(builder.risk_level.is_none());
        assert!(builder.scope.is_none());
        assert!(builder.approval.is_none());
        assert!(builder.human_approval.is_none());
        assert!(builder.rollback_strategy.is_none());
    }
}
