//! The main [`MemoryRecord`] type and its lifecycle operations.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use rivora_types::NonEmptyString;

use crate::confidence::MemoryConfidence;
use crate::kind::MemoryKind;
use crate::metadata::{MemoryMetadata, MemoryTimestamps, MemoryVersion};
use crate::provenance::MemoryProvenance;
use crate::retention::MemoryRetention;
use crate::scope::MemoryScope;
use crate::source::MemorySource;
use crate::status::MemoryStatus;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub id: NonEmptyString,
    pub kind: MemoryKind,
    pub scope: MemoryScope,
    pub status: MemoryStatus,
    pub title: NonEmptyString,
    pub body: NonEmptyString,
    pub subject_refs: Vec<NonEmptyString>,
    pub graph_node_ids: Vec<String>,
    pub graph_edge_ids: Vec<String>,
    pub receipt_ids: Vec<String>,
    pub source: MemorySource,
    pub provenance: MemoryProvenance,
    pub confidence: MemoryConfidence,
    pub retention: MemoryRetention,
    pub timestamps: MemoryTimestamps,
    pub version: MemoryVersion,
    pub labels: BTreeMap<NonEmptyString, NonEmptyString>,
    pub metadata: MemoryMetadata,
    pub feedback_ids: Vec<String>,
}

impl MemoryRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        kind: MemoryKind,
        scope: MemoryScope,
        title: impl Into<String>,
        body: impl Into<String>,
        source: MemorySource,
        provenance: MemoryProvenance,
        confidence: MemoryConfidence,
        retention: MemoryRetention,
        timestamps: MemoryTimestamps,
        version: MemoryVersion,
    ) -> Result<Self, rivora_errors::RivoraError> {
        Ok(Self {
            id: NonEmptyString::new(id.into())?,
            kind,
            scope,
            status: MemoryStatus::Draft,
            title: NonEmptyString::new(title.into())?,
            body: NonEmptyString::new(body.into())?,
            subject_refs: Vec::new(),
            graph_node_ids: Vec::new(),
            graph_edge_ids: Vec::new(),
            receipt_ids: Vec::new(),
            source,
            provenance,
            confidence,
            retention,
            timestamps,
            version,
            labels: BTreeMap::new(),
            metadata: MemoryMetadata::default(),
            feedback_ids: Vec::new(),
        })
    }

    #[must_use]
    pub fn builder() -> crate::builders::MemoryRecordBuilder {
        crate::builders::MemoryRecordBuilder::new()
    }

    #[must_use]
    pub fn with_status(mut self, status: MemoryStatus) -> Self {
        self.status = status;
        self
    }

    #[must_use]
    pub fn with_subject_refs(mut self, refs: Vec<NonEmptyString>) -> Self {
        self.subject_refs = refs;
        self
    }

    #[must_use]
    pub fn with_graph_node_ids(mut self, ids: Vec<String>) -> Self {
        self.graph_node_ids = ids;
        self
    }

    #[must_use]
    pub fn with_graph_edge_ids(mut self, ids: Vec<String>) -> Self {
        self.graph_edge_ids = ids;
        self
    }

    #[must_use]
    pub fn with_receipt_ids(mut self, ids: Vec<String>) -> Self {
        self.receipt_ids = ids;
        self
    }

    #[must_use]
    pub fn with_labels(mut self, labels: BTreeMap<NonEmptyString, NonEmptyString>) -> Self {
        self.labels = labels;
        self
    }

    #[must_use]
    pub fn with_metadata(mut self, metadata: MemoryMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    #[must_use]
    pub fn with_feedback_ids(mut self, ids: Vec<String>) -> Self {
        self.feedback_ids = ids;
        self
    }

    pub fn activate(&mut self) {
        if matches!(self.status, MemoryStatus::Draft | MemoryStatus::Candidate) {
            self.status = MemoryStatus::Active;
        }
    }

    pub fn approve(&mut self) {
        if self.status == MemoryStatus::Candidate {
            self.status = MemoryStatus::Active;
        }
    }

    pub fn reject(&mut self, reason: &str) {
        if self.status == MemoryStatus::Candidate {
            self.status = MemoryStatus::Rejected;
            if let (Ok(key), Ok(value)) = (
                NonEmptyString::new("rejection_reason"),
                NonEmptyString::new(reason),
            ) {
                self.labels.insert(key, value);
            }
        }
    }

    pub fn correct(&mut self, correction: &str) {
        if matches!(
            self.status,
            MemoryStatus::Active | MemoryStatus::Candidate | MemoryStatus::Corrected
        ) {
            self.status = MemoryStatus::Corrected;
            if let (Ok(key), Ok(value)) = (
                NonEmptyString::new("correction_text"),
                NonEmptyString::new(correction),
            ) {
                self.labels.insert(key, value);
            }
        }
    }

    pub fn add_feedback(&mut self, feedback_id: &str) {
        self.feedback_ids.push(feedback_id.to_string());
    }

    pub fn supersede(&mut self, by_id: &str) {
        self.status = MemoryStatus::Superseded;
        if let (Ok(key), Ok(value)) = (
            NonEmptyString::new("superseded_by"),
            NonEmptyString::new(by_id),
        ) {
            self.labels.insert(key, value);
        }
    }

    pub fn expire(&mut self) {
        self.status = MemoryStatus::Expired;
    }

    pub fn archive(&mut self) {
        self.status = MemoryStatus::Archived;
    }

    pub fn invalidate(&mut self, reason: &str) {
        self.status = MemoryStatus::Invalid;
        if let (Ok(key), Ok(value)) = (
            NonEmptyString::new("invalid_reason"),
            NonEmptyString::new(reason),
        ) {
            self.labels.insert(key, value);
        }
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        self.status.is_active()
    }

    #[must_use]
    pub fn is_expired_at(&self, timestamp: &str) -> bool {
        if self.status.is_expired() {
            return true;
        }
        if let Some(expires_at) = &self.retention.expires_at {
            return timestamp >= expires_at.as_str();
        }
        false
    }

    #[must_use]
    pub fn requires_review_at(&self, timestamp: &str) -> bool {
        if let Some(review_after) = &self.retention.review_after {
            return timestamp >= review_after.as_str();
        }
        false
    }

    #[must_use]
    pub fn confidence_at(&self, _timestamp: &str) -> f64 {
        self.confidence.score
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use rivora_types::Version;

    #[test]
    fn record_rejects_empty_id() {
        let record = MemoryRecord::new(
            "",
            MemoryKind::Fact,
            MemoryScope::Organization,
            "title",
            "body",
            MemorySource::Human,
            fixtures::provenance(),
            fixtures::confidence(),
            fixtures::retention(),
            fixtures::timestamps(),
            fixtures::version(),
        );
        assert!(record.is_err());
    }

    #[test]
    fn record_rejects_empty_title() {
        let record = MemoryRecord::new(
            "mem-1",
            MemoryKind::Fact,
            MemoryScope::Organization,
            "",
            "body",
            MemorySource::Human,
            fixtures::provenance(),
            fixtures::confidence(),
            fixtures::retention(),
            fixtures::timestamps(),
            fixtures::version(),
        );
        assert!(record.is_err());
    }

    #[test]
    fn record_rejects_empty_body() {
        let record = MemoryRecord::new(
            "mem-1",
            MemoryKind::Fact,
            MemoryScope::Organization,
            "title",
            "",
            MemorySource::Human,
            fixtures::provenance(),
            fixtures::confidence(),
            fixtures::retention(),
            fixtures::timestamps(),
            fixtures::version(),
        );
        assert!(record.is_err());
    }

    #[test]
    fn record_accepts_valid_fields() {
        let record = MemoryRecord::new(
            "mem-1",
            MemoryKind::Fact,
            MemoryScope::Organization,
            "title",
            "body",
            MemorySource::Human,
            fixtures::provenance(),
            fixtures::confidence(),
            fixtures::retention(),
            fixtures::timestamps(),
            fixtures::version(),
        )
        .unwrap();
        assert_eq!(record.id.as_str(), "mem-1");
        assert_eq!(record.kind, MemoryKind::Fact);
        assert_eq!(record.scope, MemoryScope::Organization);
        assert_eq!(record.status, MemoryStatus::Draft);
        assert_eq!(record.title.as_str(), "title");
        assert_eq!(record.body.as_str(), "body");
    }

    #[test]
    fn activate_sets_draft_to_active() {
        let mut record = fixtures::organization_fact();
        record.status = MemoryStatus::Draft;
        record.activate();
        assert_eq!(record.status, MemoryStatus::Active);
    }

    #[test]
    fn activate_does_not_change_non_draft() {
        let mut record = fixtures::organization_fact();
        record.status = MemoryStatus::Archived;
        record.activate();
        assert_eq!(record.status, MemoryStatus::Archived);
    }

    #[test]
    fn supersede_sets_status_and_label() {
        let mut record = fixtures::organization_fact();
        record.supersede("mem-newer");
        assert_eq!(record.status, MemoryStatus::Superseded);
        let key = NonEmptyString::new("superseded_by").unwrap();
        assert_eq!(record.labels.get(&key).unwrap().as_str(), "mem-newer");
    }

    #[test]
    fn expire_sets_status() {
        let mut record = fixtures::organization_fact();
        record.expire();
        assert_eq!(record.status, MemoryStatus::Expired);
    }

    #[test]
    fn archive_sets_status() {
        let mut record = fixtures::organization_fact();
        record.archive();
        assert_eq!(record.status, MemoryStatus::Archived);
    }

    #[test]
    fn invalidate_sets_status_and_label() {
        let mut record = fixtures::organization_fact();
        record.invalidate("stale evidence");
        assert_eq!(record.status, MemoryStatus::Invalid);
        let key = NonEmptyString::new("invalid_reason").unwrap();
        assert_eq!(record.labels.get(&key).unwrap().as_str(), "stale evidence");
    }

    #[test]
    fn is_active_returns_true_only_for_active() {
        let mut record = fixtures::organization_fact();
        assert!(record.is_active());
        record.expire();
        assert!(!record.is_active());
    }

    #[test]
    fn is_expired_at_returns_true_when_status_is_expired() {
        let mut record = fixtures::organization_fact();
        record.expire();
        assert!(record.is_expired_at("2026-06-25T12:00:00Z"));
    }

    #[test]
    fn is_expired_at_returns_true_when_timestamp_past_expires_at() {
        let mut record = fixtures::expired_memory();
        record.status = MemoryStatus::Active;
        assert!(record.is_expired_at("2026-06-26T12:00:00Z"));
    }

    #[test]
    fn is_expired_at_returns_false_when_timestamp_before_expires_at() {
        let mut record = fixtures::expired_memory();
        record.status = MemoryStatus::Active;
        assert!(!record.is_expired_at("2026-06-19T12:00:00Z"));
    }

    #[test]
    fn is_expired_at_returns_false_when_no_expires_at() {
        let record = fixtures::organization_fact();
        assert!(!record.is_expired_at("2026-06-26T12:00:00Z"));
    }

    #[test]
    fn requires_review_at_returns_true_when_timestamp_past_review_after() {
        let retention = MemoryRetention::new(
            crate::retention::MemoryRetentionPolicy::ReviewRequired,
            "needs review",
        )
        .unwrap()
        .with_review_after("2026-07-25T12:00:00Z");
        let record = MemoryRecord::new(
            "mem-1",
            MemoryKind::Fact,
            MemoryScope::Organization,
            "title",
            "body",
            MemorySource::Human,
            fixtures::provenance(),
            fixtures::confidence(),
            retention,
            fixtures::timestamps(),
            fixtures::version(),
        )
        .unwrap();
        assert!(record.requires_review_at("2026-07-26T12:00:00Z"));
    }

    #[test]
    fn requires_review_at_returns_false_when_timestamp_before_review_after() {
        let retention = MemoryRetention::new(
            crate::retention::MemoryRetentionPolicy::ReviewRequired,
            "needs review",
        )
        .unwrap()
        .with_review_after("2026-07-25T12:00:00Z");
        let record = MemoryRecord::new(
            "mem-1",
            MemoryKind::Fact,
            MemoryScope::Organization,
            "title",
            "body",
            MemorySource::Human,
            fixtures::provenance(),
            fixtures::confidence(),
            retention,
            fixtures::timestamps(),
            fixtures::version(),
        )
        .unwrap();
        assert!(!record.requires_review_at("2026-07-24T12:00:00Z"));
    }

    #[test]
    fn requires_review_at_returns_false_when_no_review_after() {
        let record = fixtures::organization_fact();
        assert!(!record.requires_review_at("2026-07-26T12:00:00Z"));
    }

    #[test]
    fn confidence_at_returns_score_regardless_of_timestamp() {
        let record = fixtures::organization_fact();
        let score = record.confidence_at("2026-06-25T12:00:00Z");
        assert!((score - record.confidence.score).abs() < f64::EPSILON);
        let score_later = record.confidence_at("2099-01-01T00:00:00Z");
        assert!((score_later - record.confidence.score).abs() < f64::EPSILON);
    }

    #[test]
    fn record_round_trips_through_serde() {
        let record = fixtures::organization_fact();
        let json = serde_json::to_string(&record).unwrap();
        let back: MemoryRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back, record);
    }

    #[test]
    fn with_subject_refs_sets_refs() {
        let record = fixtures::organization_fact()
            .with_subject_refs(vec![NonEmptyString::new("org-1").unwrap()]);
        assert_eq!(record.subject_refs.len(), 1);
    }

    #[test]
    fn with_graph_node_ids_sets_ids() {
        let record = fixtures::organization_fact().with_graph_node_ids(vec!["node-1".to_string()]);
        assert_eq!(record.graph_node_ids.len(), 1);
    }

    #[test]
    fn with_receipt_ids_sets_ids() {
        let record = fixtures::organization_fact().with_receipt_ids(vec!["receipt_1".to_string()]);
        assert_eq!(record.receipt_ids.len(), 1);
    }

    #[test]
    fn version_uses_memory_field() {
        let record = fixtures::organization_fact();
        assert_eq!(
            record.version,
            crate::metadata::MemoryVersion::new(Version::new(1, 0, 0), 1)
        );
    }

    #[test]
    fn new_record_initializes_empty_feedback_ids() {
        let record = MemoryRecord::new(
            "mem-1",
            MemoryKind::Fact,
            MemoryScope::Organization,
            "title",
            "body",
            MemorySource::Human,
            fixtures::provenance(),
            fixtures::confidence(),
            fixtures::retention(),
            fixtures::timestamps(),
            fixtures::version(),
        )
        .unwrap();
        assert!(record.feedback_ids.is_empty());
    }

    #[test]
    fn approve_transitions_candidate_to_active() {
        let mut record = fixtures::candidate_memory();
        record.approve();
        assert_eq!(record.status, MemoryStatus::Active);
    }

    #[test]
    fn approve_does_not_change_non_candidate() {
        let mut record = fixtures::organization_fact();
        assert_eq!(record.status, MemoryStatus::Active);
        record.approve();
        assert_eq!(record.status, MemoryStatus::Active);
    }

    #[test]
    fn reject_sets_status_and_reason_label() {
        let mut record = fixtures::candidate_memory();
        record.reject("not enough evidence");
        assert_eq!(record.status, MemoryStatus::Rejected);
        let key = NonEmptyString::new("rejection_reason").unwrap();
        assert_eq!(
            record.labels.get(&key).unwrap().as_str(),
            "not enough evidence"
        );
    }

    #[test]
    fn reject_does_not_change_non_candidate() {
        let mut record = fixtures::organization_fact();
        record.reject("ignored");
        assert_eq!(record.status, MemoryStatus::Active);
        assert!(!record
            .labels
            .contains_key(&NonEmptyString::new("rejection_reason").unwrap()));
    }

    #[test]
    fn correct_sets_status_and_correction_label_from_active() {
        let mut record = fixtures::organization_fact();
        record.correct("corrected body");
        assert_eq!(record.status, MemoryStatus::Corrected);
        let key = NonEmptyString::new("correction_text").unwrap();
        assert_eq!(record.labels.get(&key).unwrap().as_str(), "corrected body");
    }

    #[test]
    fn correct_sets_status_and_correction_label_from_candidate() {
        let mut record = fixtures::candidate_memory();
        record.correct("corrected body");
        assert_eq!(record.status, MemoryStatus::Corrected);
        let key = NonEmptyString::new("correction_text").unwrap();
        assert_eq!(record.labels.get(&key).unwrap().as_str(), "corrected body");
    }

    #[test]
    fn correct_works_from_corrected() {
        let mut record = fixtures::corrected_memory();
        record.correct("refined correction");
        assert_eq!(record.status, MemoryStatus::Corrected);
        let key = NonEmptyString::new("correction_text").unwrap();
        assert_eq!(
            record.labels.get(&key).unwrap().as_str(),
            "refined correction"
        );
    }

    #[test]
    fn correct_does_not_change_terminal_status() {
        let mut record = fixtures::expired_memory();
        record.correct("ignored");
        assert_eq!(record.status, MemoryStatus::Expired);
        assert!(!record
            .labels
            .contains_key(&NonEmptyString::new("correction_text").unwrap()));
    }

    #[test]
    fn add_feedback_appends_id() {
        let mut record = fixtures::organization_fact();
        assert!(record.feedback_ids.is_empty());
        record.add_feedback("fb-1");
        record.add_feedback("fb-2");
        assert_eq!(
            record.feedback_ids,
            vec!["fb-1".to_string(), "fb-2".to_string()]
        );
    }

    #[test]
    fn activate_transitions_candidate_to_active() {
        let mut record = fixtures::candidate_memory();
        record.activate();
        assert_eq!(record.status, MemoryStatus::Active);
    }

    #[test]
    fn with_feedback_ids_sets_ids() {
        let record = fixtures::organization_fact()
            .with_feedback_ids(vec!["fb-1".to_string(), "fb-2".to_string()]);
        assert_eq!(record.feedback_ids.len(), 2);
    }
}
