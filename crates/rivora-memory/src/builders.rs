//! Ergonomic builders for constructing memory entities.

use std::collections::BTreeMap;

use rivora_errors::RivoraError;
use rivora_types::NonEmptyString;

use crate::confidence::MemoryConfidence;
use crate::feedback::{FeedbackKind, FeedbackSource, FeedbackTargetType, HumanFeedback};
use crate::kind::MemoryKind;
use crate::metadata::{MemoryMetadata, MemoryTimestamps, MemoryVersion};
use crate::provenance::MemoryProvenance;
use crate::recall::MemoryRecallQuery;
use crate::record::MemoryRecord;
use crate::retention::{MemoryDecay, MemoryRetention, MemoryRetentionPolicy};
use crate::scope::MemoryScope;
use crate::source::MemorySource;
use crate::status::MemoryStatus;

#[derive(Debug, Clone, Default)]
pub struct MemoryRecordBuilder {
    id: Option<NonEmptyString>,
    kind: Option<MemoryKind>,
    scope: Option<MemoryScope>,
    status: Option<MemoryStatus>,
    title: Option<NonEmptyString>,
    body: Option<NonEmptyString>,
    subject_refs: Option<Vec<NonEmptyString>>,
    graph_node_ids: Option<Vec<String>>,
    graph_edge_ids: Option<Vec<String>>,
    receipt_ids: Option<Vec<String>>,
    source: Option<MemorySource>,
    provenance: Option<MemoryProvenance>,
    confidence: Option<MemoryConfidence>,
    retention: Option<MemoryRetention>,
    timestamps: Option<MemoryTimestamps>,
    version: Option<MemoryVersion>,
    labels: Option<BTreeMap<NonEmptyString, NonEmptyString>>,
    metadata: Option<MemoryMetadata>,
}

impl MemoryRecordBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = NonEmptyString::new(id.into()).ok();
        self
    }

    #[must_use]
    pub fn kind(mut self, kind: MemoryKind) -> Self {
        self.kind = Some(kind);
        self
    }

    #[must_use]
    pub fn scope(mut self, scope: MemoryScope) -> Self {
        self.scope = Some(scope);
        self
    }

    #[must_use]
    pub fn status(mut self, status: MemoryStatus) -> Self {
        self.status = Some(status);
        self
    }

    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = NonEmptyString::new(title.into()).ok();
        self
    }

    #[must_use]
    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = NonEmptyString::new(body.into()).ok();
        self
    }

    #[must_use]
    pub fn subject_refs(mut self, refs: Vec<NonEmptyString>) -> Self {
        self.subject_refs = Some(refs);
        self
    }

    #[must_use]
    pub fn graph_node_ids(mut self, ids: Vec<String>) -> Self {
        self.graph_node_ids = Some(ids);
        self
    }

    #[must_use]
    pub fn graph_edge_ids(mut self, ids: Vec<String>) -> Self {
        self.graph_edge_ids = Some(ids);
        self
    }

    #[must_use]
    pub fn receipt_ids(mut self, ids: Vec<String>) -> Self {
        self.receipt_ids = Some(ids);
        self
    }

    #[must_use]
    pub fn source(mut self, source: MemorySource) -> Self {
        self.source = Some(source);
        self
    }

    #[must_use]
    pub fn provenance(mut self, provenance: MemoryProvenance) -> Self {
        self.provenance = Some(provenance);
        self
    }

    #[must_use]
    pub fn confidence(mut self, confidence: MemoryConfidence) -> Self {
        self.confidence = Some(confidence);
        self
    }

    #[must_use]
    pub fn retention(mut self, retention: MemoryRetention) -> Self {
        self.retention = Some(retention);
        self
    }

    #[must_use]
    pub fn timestamps(mut self, timestamps: MemoryTimestamps) -> Self {
        self.timestamps = Some(timestamps);
        self
    }

    #[must_use]
    pub fn version(mut self, version: MemoryVersion) -> Self {
        self.version = Some(version);
        self
    }

    #[must_use]
    pub fn labels(mut self, labels: BTreeMap<NonEmptyString, NonEmptyString>) -> Self {
        self.labels = Some(labels);
        self
    }

    #[must_use]
    pub fn metadata(mut self, metadata: MemoryMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn build(self) -> Result<MemoryRecord, RivoraError> {
        let id = self
            .id
            .ok_or_else(|| RivoraError::invalid_value("record_id", "id is required"))?;
        let kind = self
            .kind
            .ok_or_else(|| RivoraError::invalid_value("record_kind", "kind is required"))?;
        let scope = self
            .scope
            .ok_or_else(|| RivoraError::invalid_value("record_scope", "scope is required"))?;
        let title = self
            .title
            .ok_or_else(|| RivoraError::invalid_value("record_title", "title is required"))?;
        let body = self
            .body
            .ok_or_else(|| RivoraError::invalid_value("record_body", "body is required"))?;
        let source = self
            .source
            .ok_or_else(|| RivoraError::invalid_value("record_source", "source is required"))?;
        let provenance = self.provenance.ok_or_else(|| {
            RivoraError::invalid_value("record_provenance", "provenance is required")
        })?;
        let confidence = self.confidence.ok_or_else(|| {
            RivoraError::invalid_value("record_confidence", "confidence is required")
        })?;
        let retention = self.retention.ok_or_else(|| {
            RivoraError::invalid_value("record_retention", "retention is required")
        })?;
        let timestamps = self.timestamps.ok_or_else(|| {
            RivoraError::invalid_value("record_timestamps", "timestamps is required")
        })?;
        let version = self
            .version
            .ok_or_else(|| RivoraError::invalid_value("record_version", "version is required"))?;
        Ok(MemoryRecord {
            id,
            kind,
            scope,
            status: self.status.unwrap_or(MemoryStatus::Draft),
            title,
            body,
            subject_refs: self.subject_refs.unwrap_or_default(),
            graph_node_ids: self.graph_node_ids.unwrap_or_default(),
            graph_edge_ids: self.graph_edge_ids.unwrap_or_default(),
            receipt_ids: self.receipt_ids.unwrap_or_default(),
            source,
            provenance,
            confidence,
            retention,
            timestamps,
            version,
            labels: self.labels.unwrap_or_default(),
            metadata: self.metadata.unwrap_or_default(),
            feedback_ids: Vec::new(),
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct MemoryProvenanceBuilder {
    source: Option<NonEmptyString>,
    source_version: Option<NonEmptyString>,
    observed_at: Option<NonEmptyString>,
    learned_at: Option<NonEmptyString>,
    graph_id: Option<String>,
    graph_node_ids: Option<Vec<String>>,
    graph_edge_ids: Option<Vec<String>>,
    receipt_id: Option<String>,
    connector_ref: Option<String>,
    inference_ref: Option<String>,
    ability_ref: Option<String>,
    human_ref: Option<String>,
    raw_ref: Option<String>,
}

impl MemoryProvenanceBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = NonEmptyString::new(source.into()).ok();
        self
    }

    #[must_use]
    pub fn source_version(mut self, source_version: impl Into<String>) -> Self {
        self.source_version = NonEmptyString::new(source_version.into()).ok();
        self
    }

    #[must_use]
    pub fn observed_at(mut self, observed_at: impl Into<String>) -> Self {
        self.observed_at = NonEmptyString::new(observed_at.into()).ok();
        self
    }

    #[must_use]
    pub fn learned_at(mut self, learned_at: impl Into<String>) -> Self {
        self.learned_at = NonEmptyString::new(learned_at.into()).ok();
        self
    }

    #[must_use]
    pub fn graph_id(mut self, graph_id: impl Into<String>) -> Self {
        self.graph_id = Some(graph_id.into());
        self
    }

    #[must_use]
    pub fn graph_node_ids(mut self, ids: Vec<String>) -> Self {
        self.graph_node_ids = Some(ids);
        self
    }

    #[must_use]
    pub fn graph_edge_ids(mut self, ids: Vec<String>) -> Self {
        self.graph_edge_ids = Some(ids);
        self
    }

    #[must_use]
    pub fn receipt_id(mut self, receipt_id: impl Into<String>) -> Self {
        self.receipt_id = Some(receipt_id.into());
        self
    }

    #[must_use]
    pub fn connector_ref(mut self, connector_ref: impl Into<String>) -> Self {
        self.connector_ref = Some(connector_ref.into());
        self
    }

    #[must_use]
    pub fn inference_ref(mut self, inference_ref: impl Into<String>) -> Self {
        self.inference_ref = Some(inference_ref.into());
        self
    }

    #[must_use]
    pub fn ability_ref(mut self, ability_ref: impl Into<String>) -> Self {
        self.ability_ref = Some(ability_ref.into());
        self
    }

    #[must_use]
    pub fn human_ref(mut self, human_ref: impl Into<String>) -> Self {
        self.human_ref = Some(human_ref.into());
        self
    }

    #[must_use]
    pub fn raw_ref(mut self, raw_ref: impl Into<String>) -> Self {
        self.raw_ref = Some(raw_ref.into());
        self
    }

    pub fn build(self) -> Result<MemoryProvenance, RivoraError> {
        let source = self
            .source
            .ok_or_else(|| RivoraError::invalid_value("provenance_source", "source is required"))?;
        let source_version = self.source_version.ok_or_else(|| {
            RivoraError::invalid_value("provenance_source_version", "source_version is required")
        })?;
        let observed_at = self.observed_at.ok_or_else(|| {
            RivoraError::invalid_value("provenance_observed_at", "observed_at is required")
        })?;
        let learned_at = self.learned_at.ok_or_else(|| {
            RivoraError::invalid_value("provenance_learned_at", "learned_at is required")
        })?;
        Ok(MemoryProvenance {
            source,
            source_version,
            observed_at,
            learned_at,
            graph_id: self.graph_id,
            graph_node_ids: self.graph_node_ids.unwrap_or_default(),
            graph_edge_ids: self.graph_edge_ids.unwrap_or_default(),
            receipt_id: self.receipt_id,
            connector_ref: self.connector_ref,
            inference_ref: self.inference_ref,
            ability_ref: self.ability_ref,
            human_ref: self.human_ref,
            raw_ref: self.raw_ref,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct MemoryConfidenceBuilder {
    score: Option<f64>,
    explanation: Option<NonEmptyString>,
    contributing_factors: Option<Vec<NonEmptyString>>,
    limiting_factors: Option<Vec<NonEmptyString>>,
    last_evaluated_at: Option<NonEmptyString>,
}

impl MemoryConfidenceBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn score(mut self, score: f64) -> Self {
        self.score = Some(score);
        self
    }

    #[must_use]
    pub fn explanation(mut self, explanation: impl Into<String>) -> Self {
        self.explanation = NonEmptyString::new(explanation.into()).ok();
        self
    }

    #[must_use]
    pub fn contributing_factors(mut self, factors: Vec<NonEmptyString>) -> Self {
        self.contributing_factors = Some(factors);
        self
    }

    #[must_use]
    pub fn limiting_factors(mut self, factors: Vec<NonEmptyString>) -> Self {
        self.limiting_factors = Some(factors);
        self
    }

    #[must_use]
    pub fn last_evaluated_at(mut self, last_evaluated_at: impl Into<String>) -> Self {
        self.last_evaluated_at = NonEmptyString::new(last_evaluated_at.into()).ok();
        self
    }

    pub fn build(self) -> Result<MemoryConfidence, RivoraError> {
        let score = self
            .score
            .ok_or_else(|| RivoraError::invalid_value("confidence_score", "score is required"))?;
        let explanation = self.explanation.ok_or_else(|| {
            RivoraError::invalid_value("confidence_explanation", "explanation is required")
        })?;
        let last_evaluated_at = self.last_evaluated_at.ok_or_else(|| {
            RivoraError::invalid_value(
                "confidence_last_evaluated_at",
                "last_evaluated_at is required",
            )
        })?;
        Ok(MemoryConfidence {
            score,
            level: crate::confidence::MemoryConfidenceLevel::from_score(score),
            explanation,
            contributing_factors: self.contributing_factors.unwrap_or_default(),
            limiting_factors: self.limiting_factors.unwrap_or_default(),
            last_evaluated_at,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct MemoryRetentionBuilder {
    policy: Option<MemoryRetentionPolicy>,
    expires_at: Option<NonEmptyString>,
    review_after: Option<NonEmptyString>,
    max_age_days: Option<u64>,
    decay: Option<MemoryDecay>,
    reason: Option<NonEmptyString>,
}

impl MemoryRetentionBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn policy(mut self, policy: MemoryRetentionPolicy) -> Self {
        self.policy = Some(policy);
        self
    }

    #[must_use]
    pub fn expires_at(mut self, expires_at: impl Into<String>) -> Self {
        self.expires_at = NonEmptyString::new(expires_at.into()).ok();
        self
    }

    #[must_use]
    pub fn review_after(mut self, review_after: impl Into<String>) -> Self {
        self.review_after = NonEmptyString::new(review_after.into()).ok();
        self
    }

    #[must_use]
    pub fn max_age_days(mut self, max_age_days: u64) -> Self {
        self.max_age_days = Some(max_age_days);
        self
    }

    #[must_use]
    pub fn decay(mut self, decay: MemoryDecay) -> Self {
        self.decay = Some(decay);
        self
    }

    #[must_use]
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = NonEmptyString::new(reason.into()).ok();
        self
    }

    pub fn build(self) -> Result<MemoryRetention, RivoraError> {
        let policy = self
            .policy
            .ok_or_else(|| RivoraError::invalid_value("retention_policy", "policy is required"))?;
        let reason = self
            .reason
            .ok_or_else(|| RivoraError::invalid_value("retention_reason", "reason is required"))?;
        Ok(MemoryRetention {
            policy,
            expires_at: self.expires_at,
            review_after: self.review_after,
            max_age_days: self.max_age_days,
            decay: self.decay.unwrap_or(MemoryDecay::None),
            reason,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct MemoryRecallQueryBuilder {
    kind: Option<MemoryKind>,
    scope: Option<MemoryScope>,
    status: Option<MemoryStatus>,
    subject_ref: Option<NonEmptyString>,
    graph_node_id: Option<String>,
    receipt_id: Option<String>,
    min_confidence: Option<f64>,
    include_expired: bool,
    limit: Option<usize>,
}

impl MemoryRecallQueryBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn kind(mut self, kind: MemoryKind) -> Self {
        self.kind = Some(kind);
        self
    }

    #[must_use]
    pub fn scope(mut self, scope: MemoryScope) -> Self {
        self.scope = Some(scope);
        self
    }

    #[must_use]
    pub fn status(mut self, status: MemoryStatus) -> Self {
        self.status = Some(status);
        self
    }

    #[must_use]
    pub fn subject_ref(mut self, subject_ref: impl Into<String>) -> Self {
        self.subject_ref = NonEmptyString::new(subject_ref.into()).ok();
        self
    }

    #[must_use]
    pub fn graph_node_id(mut self, graph_node_id: impl Into<String>) -> Self {
        self.graph_node_id = Some(graph_node_id.into());
        self
    }

    #[must_use]
    pub fn receipt_id(mut self, receipt_id: impl Into<String>) -> Self {
        self.receipt_id = Some(receipt_id.into());
        self
    }

    #[must_use]
    pub fn min_confidence(mut self, min_confidence: f64) -> Self {
        self.min_confidence = Some(min_confidence);
        self
    }

    #[must_use]
    pub fn include_expired(mut self, include_expired: bool) -> Self {
        self.include_expired = include_expired;
        self
    }

    #[must_use]
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn build(self) -> Result<MemoryRecallQuery, RivoraError> {
        Ok(MemoryRecallQuery {
            kind: self.kind,
            scope: self.scope,
            status: self.status,
            subject_ref: self.subject_ref,
            graph_node_id: self.graph_node_id,
            receipt_id: self.receipt_id,
            min_confidence: self.min_confidence,
            include_expired: self.include_expired,
            limit: self.limit,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct HumanFeedbackBuilder {
    id: Option<NonEmptyString>,
    target_id: Option<NonEmptyString>,
    target_type: Option<FeedbackTargetType>,
    actor: Option<NonEmptyString>,
    source: Option<FeedbackSource>,
    kind: Option<FeedbackKind>,
    note: Option<NonEmptyString>,
    correction_text: Option<NonEmptyString>,
    confidence_adjustment: Option<f64>,
    timestamp: Option<NonEmptyString>,
}

impl HumanFeedbackBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = NonEmptyString::new(id.into()).ok();
        self
    }

    #[must_use]
    pub fn target_id(mut self, target_id: impl Into<String>) -> Self {
        self.target_id = NonEmptyString::new(target_id.into()).ok();
        self
    }

    #[must_use]
    pub fn target_type(mut self, target_type: FeedbackTargetType) -> Self {
        self.target_type = Some(target_type);
        self
    }

    #[must_use]
    pub fn actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = NonEmptyString::new(actor.into()).ok();
        self
    }

    #[must_use]
    pub fn source(mut self, source: FeedbackSource) -> Self {
        self.source = Some(source);
        self
    }

    #[must_use]
    pub fn kind(mut self, kind: FeedbackKind) -> Self {
        self.kind = Some(kind);
        self
    }

    #[must_use]
    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.note = NonEmptyString::new(note.into()).ok();
        self
    }

    #[must_use]
    pub fn correction_text(mut self, correction_text: impl Into<String>) -> Self {
        self.correction_text = NonEmptyString::new(correction_text.into()).ok();
        self
    }

    #[must_use]
    pub fn confidence_adjustment(mut self, confidence_adjustment: f64) -> Self {
        self.confidence_adjustment = Some(confidence_adjustment);
        self
    }

    #[must_use]
    pub fn timestamp(mut self, timestamp: impl Into<String>) -> Self {
        self.timestamp = NonEmptyString::new(timestamp.into()).ok();
        self
    }

    pub fn build(self) -> Result<HumanFeedback, RivoraError> {
        let id = self
            .id
            .ok_or_else(|| RivoraError::invalid_value("feedback_id", "id is required"))?;
        let target_id = self.target_id.ok_or_else(|| {
            RivoraError::invalid_value("feedback_target_id", "target_id is required")
        })?;
        let target_type = self.target_type.ok_or_else(|| {
            RivoraError::invalid_value("feedback_target_type", "target_type is required")
        })?;
        let actor = self
            .actor
            .ok_or_else(|| RivoraError::invalid_value("feedback_actor", "actor is required"))?;
        let source = self
            .source
            .ok_or_else(|| RivoraError::invalid_value("feedback_source", "source is required"))?;
        let kind = self
            .kind
            .ok_or_else(|| RivoraError::invalid_value("feedback_kind", "kind is required"))?;
        let timestamp = self.timestamp.ok_or_else(|| {
            RivoraError::invalid_value("feedback_timestamp", "timestamp is required")
        })?;
        Ok(HumanFeedback {
            id,
            target_id,
            target_type,
            actor,
            source,
            kind,
            note: self.note,
            correction_text: self.correction_text,
            confidence_adjustment: self.confidence_adjustment,
            timestamp,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::kind::MemoryKind;
    use crate::retention::{MemoryDecay, MemoryRetentionPolicy};
    use crate::scope::MemoryScope;
    use crate::source::MemorySource;
    use crate::status::MemoryStatus;

    #[test]
    fn record_builder_succeeds() {
        let record = MemoryRecordBuilder::new()
            .id("mem-1")
            .kind(MemoryKind::Fact)
            .scope(MemoryScope::Organization)
            .title("title")
            .body("body")
            .source(MemorySource::Human)
            .provenance(fixtures::provenance())
            .confidence(fixtures::confidence())
            .retention(fixtures::retention())
            .timestamps(fixtures::timestamps())
            .version(fixtures::version())
            .build()
            .unwrap();
        assert_eq!(record.id.as_str(), "mem-1");
        assert_eq!(record.kind, MemoryKind::Fact);
        assert_eq!(record.status, MemoryStatus::Draft);
    }

    #[test]
    fn record_builder_requires_id() {
        let err = MemoryRecordBuilder::new()
            .kind(MemoryKind::Fact)
            .scope(MemoryScope::Organization)
            .title("title")
            .body("body")
            .source(MemorySource::Human)
            .provenance(fixtures::provenance())
            .confidence(fixtures::confidence())
            .retention(fixtures::retention())
            .timestamps(fixtures::timestamps())
            .version(fixtures::version())
            .build()
            .unwrap_err();
        assert!(err.to_string().contains("id"));
    }

    #[test]
    fn record_builder_requires_kind() {
        let err = MemoryRecordBuilder::new()
            .id("mem-1")
            .scope(MemoryScope::Organization)
            .title("title")
            .body("body")
            .source(MemorySource::Human)
            .provenance(fixtures::provenance())
            .confidence(fixtures::confidence())
            .retention(fixtures::retention())
            .timestamps(fixtures::timestamps())
            .version(fixtures::version())
            .build()
            .unwrap_err();
        assert!(err.to_string().contains("kind"));
    }

    #[test]
    fn record_builder_with_status() {
        let record = MemoryRecordBuilder::new()
            .id("mem-1")
            .kind(MemoryKind::Fact)
            .scope(MemoryScope::Organization)
            .status(MemoryStatus::Active)
            .title("title")
            .body("body")
            .source(MemorySource::Human)
            .provenance(fixtures::provenance())
            .confidence(fixtures::confidence())
            .retention(fixtures::retention())
            .timestamps(fixtures::timestamps())
            .version(fixtures::version())
            .build()
            .unwrap();
        assert_eq!(record.status, MemoryStatus::Active);
    }

    #[test]
    fn record_builder_with_collections() {
        let record = MemoryRecordBuilder::new()
            .id("mem-1")
            .kind(MemoryKind::Fact)
            .scope(MemoryScope::Organization)
            .title("title")
            .body("body")
            .source(MemorySource::Human)
            .provenance(fixtures::provenance())
            .confidence(fixtures::confidence())
            .retention(fixtures::retention())
            .timestamps(fixtures::timestamps())
            .version(fixtures::version())
            .subject_refs(vec![NonEmptyString::new("org-1").unwrap()])
            .graph_node_ids(vec!["node-1".to_string()])
            .graph_edge_ids(vec!["edge-1".to_string()])
            .receipt_ids(vec!["receipt_1".to_string()])
            .build()
            .unwrap();
        assert_eq!(record.subject_refs.len(), 1);
        assert_eq!(record.graph_node_ids.len(), 1);
        assert_eq!(record.graph_edge_ids.len(), 1);
        assert_eq!(record.receipt_ids.len(), 1);
    }

    #[test]
    fn provenance_builder_succeeds() {
        let p = MemoryProvenanceBuilder::new()
            .source("connector")
            .source_version("0.1.0")
            .observed_at("2026-06-25T12:00:00Z")
            .learned_at("2026-06-25T12:00:00Z")
            .build()
            .unwrap();
        assert_eq!(p.source.as_str(), "connector");
    }

    #[test]
    fn provenance_builder_requires_source() {
        let err = MemoryProvenanceBuilder::new()
            .source_version("0.1.0")
            .observed_at("2026-06-25T12:00:00Z")
            .learned_at("2026-06-25T12:00:00Z")
            .build()
            .unwrap_err();
        assert!(err.to_string().contains("source"));
    }

    #[test]
    fn provenance_builder_with_optional_fields() {
        let p = MemoryProvenanceBuilder::new()
            .source("connector")
            .source_version("0.1.0")
            .observed_at("2026-06-25T12:00:00Z")
            .learned_at("2026-06-25T12:00:00Z")
            .receipt_id("receipt_1")
            .graph_id("graph-1")
            .build()
            .unwrap();
        assert_eq!(p.receipt_id.as_deref(), Some("receipt_1"));
        assert_eq!(p.graph_id.as_deref(), Some("graph-1"));
    }

    #[test]
    fn confidence_builder_succeeds() {
        let c = MemoryConfidenceBuilder::new()
            .score(0.85)
            .explanation("strong evidence")
            .last_evaluated_at("2026-06-25T12:00:00Z")
            .build()
            .unwrap();
        assert!((c.score - 0.85).abs() < f64::EPSILON);
        assert_eq!(c.level, crate::confidence::MemoryConfidenceLevel::High);
    }

    #[test]
    fn confidence_builder_requires_score() {
        let err = MemoryConfidenceBuilder::new()
            .explanation("explanation")
            .last_evaluated_at("2026-06-25T12:00:00Z")
            .build()
            .unwrap_err();
        assert!(err.to_string().contains("score"));
    }

    #[test]
    fn confidence_builder_with_factors() {
        let c = MemoryConfidenceBuilder::new()
            .score(0.5)
            .explanation("moderate")
            .last_evaluated_at("2026-06-25T12:00:00Z")
            .contributing_factors(vec![NonEmptyString::new("stable pattern").unwrap()])
            .limiting_factors(vec![NonEmptyString::new("small sample").unwrap()])
            .build()
            .unwrap();
        assert_eq!(c.contributing_factors.len(), 1);
        assert_eq!(c.limiting_factors.len(), 1);
    }

    #[test]
    fn retention_builder_succeeds() {
        let r = MemoryRetentionBuilder::new()
            .policy(MemoryRetentionPolicy::Permanent)
            .reason("fixture")
            .build()
            .unwrap();
        assert_eq!(r.policy, MemoryRetentionPolicy::Permanent);
        assert_eq!(r.decay, MemoryDecay::None);
    }

    #[test]
    fn retention_builder_requires_policy() {
        let err = MemoryRetentionBuilder::new()
            .reason("fixture")
            .build()
            .unwrap_err();
        assert!(err.to_string().contains("policy"));
    }

    #[test]
    fn retention_builder_requires_reason() {
        let err = MemoryRetentionBuilder::new()
            .policy(MemoryRetentionPolicy::Permanent)
            .build()
            .unwrap_err();
        assert!(err.to_string().contains("reason"));
    }

    #[test]
    fn retention_builder_with_optional_fields() {
        let r = MemoryRetentionBuilder::new()
            .policy(MemoryRetentionPolicy::TimeBound)
            .reason("time-bound")
            .expires_at("2026-06-25T12:00:00Z")
            .review_after("2026-07-25T12:00:00Z")
            .max_age_days(30)
            .decay(MemoryDecay::Linear)
            .build()
            .unwrap();
        assert!(r.expires_at.is_some());
        assert!(r.review_after.is_some());
        assert_eq!(r.max_age_days, Some(30));
        assert_eq!(r.decay, MemoryDecay::Linear);
    }

    #[test]
    fn recall_query_builder_succeeds() {
        let q = MemoryRecallQueryBuilder::new()
            .kind(MemoryKind::Fact)
            .scope(MemoryScope::Organization)
            .status(MemoryStatus::Active)
            .min_confidence(0.5)
            .limit(10)
            .build()
            .unwrap();
        assert_eq!(q.kind, Some(MemoryKind::Fact));
        assert_eq!(q.scope, Some(MemoryScope::Organization));
        assert_eq!(q.status, Some(MemoryStatus::Active));
        assert_eq!(q.min_confidence, Some(0.5));
        assert_eq!(q.limit, Some(10));
        assert!(!q.include_expired);
    }

    #[test]
    fn recall_query_builder_with_include_expired() {
        let q = MemoryRecallQueryBuilder::new()
            .include_expired(true)
            .build()
            .unwrap();
        assert!(q.include_expired);
    }

    #[test]
    fn record_builder_method_on_record() {
        let record = MemoryRecord::builder()
            .id("mem-1")
            .kind(MemoryKind::Fact)
            .scope(MemoryScope::Organization)
            .title("title")
            .body("body")
            .source(MemorySource::Human)
            .provenance(fixtures::provenance())
            .confidence(fixtures::confidence())
            .retention(fixtures::retention())
            .timestamps(fixtures::timestamps())
            .version(fixtures::version())
            .build()
            .unwrap();
        assert_eq!(record.id.as_str(), "mem-1");
    }

    #[test]
    fn provenance_builder_method() {
        let p = MemoryProvenance::builder()
            .source("connector")
            .source_version("0.1.0")
            .observed_at("2026-06-25T12:00:00Z")
            .learned_at("2026-06-25T12:00:00Z")
            .build()
            .unwrap();
        assert_eq!(p.source.as_str(), "connector");
    }

    #[test]
    fn confidence_builder_method() {
        let c = MemoryConfidence::builder()
            .score(0.85)
            .explanation("strong")
            .last_evaluated_at("2026-06-25T12:00:00Z")
            .build()
            .unwrap();
        assert!((c.score - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn retention_builder_method() {
        let r = MemoryRetention::builder()
            .policy(MemoryRetentionPolicy::Permanent)
            .reason("fixture")
            .build()
            .unwrap();
        assert_eq!(r.policy, MemoryRetentionPolicy::Permanent);
    }

    #[test]
    fn recall_query_builder_method() {
        let q = MemoryRecallQuery::builder()
            .kind(MemoryKind::Fact)
            .build()
            .unwrap();
        assert_eq!(q.kind, Some(MemoryKind::Fact));
    }

    #[test]
    fn feedback_builder_succeeds() {
        let feedback = HumanFeedbackBuilder::new()
            .id("fb-1")
            .target_id("mem-1")
            .target_type(FeedbackTargetType::Memory)
            .actor("actor-1")
            .source(FeedbackSource::Human)
            .kind(FeedbackKind::Approved)
            .timestamp("2026-06-25T12:00:00Z")
            .build()
            .unwrap();
        assert_eq!(feedback.id.as_str(), "fb-1");
        assert_eq!(feedback.target_id.as_str(), "mem-1");
        assert_eq!(feedback.target_type, FeedbackTargetType::Memory);
        assert_eq!(feedback.actor.as_str(), "actor-1");
        assert_eq!(feedback.source, FeedbackSource::Human);
        assert_eq!(feedback.kind, FeedbackKind::Approved);
        assert_eq!(feedback.timestamp.as_str(), "2026-06-25T12:00:00Z");
    }

    #[test]
    fn feedback_builder_with_optional_fields() {
        let feedback = HumanFeedbackBuilder::new()
            .id("fb-1")
            .target_id("mem-1")
            .target_type(FeedbackTargetType::Memory)
            .actor("actor-1")
            .source(FeedbackSource::Slack)
            .kind(FeedbackKind::Corrected)
            .note("note text")
            .correction_text("corrected body")
            .confidence_adjustment(0.75)
            .timestamp("2026-06-25T12:00:00Z")
            .build()
            .unwrap();
        assert_eq!(feedback.note.as_ref().unwrap().as_str(), "note text");
        assert_eq!(
            feedback.correction_text.as_ref().unwrap().as_str(),
            "corrected body"
        );
        assert!((feedback.confidence_adjustment.unwrap() - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn feedback_builder_requires_id() {
        let err = HumanFeedbackBuilder::new()
            .target_id("mem-1")
            .target_type(FeedbackTargetType::Memory)
            .actor("actor-1")
            .source(FeedbackSource::Human)
            .kind(FeedbackKind::Approved)
            .timestamp("2026-06-25T12:00:00Z")
            .build()
            .unwrap_err();
        assert!(err.to_string().contains("id"));
    }

    #[test]
    fn feedback_builder_requires_target_id() {
        let err = HumanFeedbackBuilder::new()
            .id("fb-1")
            .target_type(FeedbackTargetType::Memory)
            .actor("actor-1")
            .source(FeedbackSource::Human)
            .kind(FeedbackKind::Approved)
            .timestamp("2026-06-25T12:00:00Z")
            .build()
            .unwrap_err();
        assert!(err.to_string().contains("target_id"));
    }

    #[test]
    fn feedback_builder_requires_target_type() {
        let err = HumanFeedbackBuilder::new()
            .id("fb-1")
            .target_id("mem-1")
            .actor("actor-1")
            .source(FeedbackSource::Human)
            .kind(FeedbackKind::Approved)
            .timestamp("2026-06-25T12:00:00Z")
            .build()
            .unwrap_err();
        assert!(err.to_string().contains("target_type"));
    }

    #[test]
    fn feedback_builder_requires_actor() {
        let err = HumanFeedbackBuilder::new()
            .id("fb-1")
            .target_id("mem-1")
            .target_type(FeedbackTargetType::Memory)
            .source(FeedbackSource::Human)
            .kind(FeedbackKind::Approved)
            .timestamp("2026-06-25T12:00:00Z")
            .build()
            .unwrap_err();
        assert!(err.to_string().contains("actor"));
    }

    #[test]
    fn feedback_builder_requires_source() {
        let err = HumanFeedbackBuilder::new()
            .id("fb-1")
            .target_id("mem-1")
            .target_type(FeedbackTargetType::Memory)
            .actor("actor-1")
            .kind(FeedbackKind::Approved)
            .timestamp("2026-06-25T12:00:00Z")
            .build()
            .unwrap_err();
        assert!(err.to_string().contains("source"));
    }

    #[test]
    fn feedback_builder_requires_kind() {
        let err = HumanFeedbackBuilder::new()
            .id("fb-1")
            .target_id("mem-1")
            .target_type(FeedbackTargetType::Memory)
            .actor("actor-1")
            .source(FeedbackSource::Human)
            .timestamp("2026-06-25T12:00:00Z")
            .build()
            .unwrap_err();
        assert!(err.to_string().contains("kind"));
    }

    #[test]
    fn feedback_builder_requires_timestamp() {
        let err = HumanFeedbackBuilder::new()
            .id("fb-1")
            .target_id("mem-1")
            .target_type(FeedbackTargetType::Memory)
            .actor("actor-1")
            .source(FeedbackSource::Human)
            .kind(FeedbackKind::Approved)
            .build()
            .unwrap_err();
        assert!(err.to_string().contains("timestamp"));
    }

    #[test]
    fn feedback_builder_method_on_human_feedback() {
        let feedback = HumanFeedback::builder()
            .id("fb-1")
            .target_id("mem-1")
            .target_type(FeedbackTargetType::Memory)
            .actor("actor-1")
            .source(FeedbackSource::Cli)
            .kind(FeedbackKind::Rejected)
            .timestamp("2026-06-25T12:00:00Z")
            .build()
            .unwrap();
        assert_eq!(feedback.id.as_str(), "fb-1");
        assert_eq!(feedback.kind, FeedbackKind::Rejected);
    }
}
