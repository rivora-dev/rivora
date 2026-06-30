//! In-memory index of memory records for recall operations.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use rivora_errors::RivoraError;
use rivora_types::NonEmptyString;

use crate::kind::MemoryKind;
use crate::metadata::MemoryMetadata;
use crate::recall::{MemoryRecallQuery, MemoryRecallResult};
use crate::record::MemoryRecord;
use crate::scope::MemoryScope;
use crate::snapshot::MemorySnapshot;
use crate::status::MemoryStatus;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MemoryIndex {
    pub metadata: MemoryMetadata,
    pub(crate) records: BTreeMap<String, MemoryRecord>,
}

impl MemoryIndex {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_metadata(metadata: MemoryMetadata) -> Self {
        Self {
            metadata,
            records: BTreeMap::new(),
        }
    }

    pub fn add_record(&mut self, record: MemoryRecord) -> Result<(), RivoraError> {
        let id = record.id.as_str().to_string();
        if self.records.contains_key(&id) {
            return Err(RivoraError::invalid_value(
                "record_id",
                format!("duplicate record id: {id}"),
            ));
        }
        self.records.insert(id, record);
        Ok(())
    }

    #[must_use]
    pub fn get_record(&self, id: &str) -> Option<&MemoryRecord> {
        self.records.get(id)
    }

    pub fn remove_record(&mut self, id: &str) -> Option<MemoryRecord> {
        self.records.remove(id)
    }

    #[must_use]
    pub fn records_by_kind(&self, kind: MemoryKind) -> Vec<&MemoryRecord> {
        self.records.values().filter(|r| r.kind == kind).collect()
    }

    #[must_use]
    pub fn records_by_scope(&self, scope: MemoryScope) -> Vec<&MemoryRecord> {
        self.records.values().filter(|r| r.scope == scope).collect()
    }

    #[must_use]
    pub fn records_by_status(&self, status: MemoryStatus) -> Vec<&MemoryRecord> {
        self.records
            .values()
            .filter(|r| r.status == status)
            .collect()
    }

    #[must_use]
    pub fn records_for_graph_node(&self, node_id: &str) -> Vec<&MemoryRecord> {
        self.records
            .values()
            .filter(|r| r.graph_node_ids.iter().any(|id| id.as_str() == node_id))
            .collect()
    }

    #[must_use]
    pub fn records_for_receipt(&self, receipt_id: &str) -> Vec<&MemoryRecord> {
        self.records
            .values()
            .filter(|r| r.receipt_ids.iter().any(|id| id.as_str() == receipt_id))
            .collect()
    }

    #[must_use]
    pub fn recall(&self, query: &MemoryRecallQuery) -> MemoryRecallResult {
        let mut filtered: Vec<&MemoryRecord> = self.records.values().collect();

        if let Some(kind) = query.kind {
            filtered.retain(|r| r.kind == kind);
        }
        if let Some(scope) = query.scope {
            filtered.retain(|r| r.scope == scope);
        }
        if let Some(status) = query.status {
            filtered.retain(|r| r.status == status);
        }
        if let Some(subject_ref) = &query.subject_ref {
            filtered.retain(|r| r.subject_refs.iter().any(|s| s == subject_ref));
        }
        if let Some(node_id) = &query.graph_node_id {
            filtered.retain(|r| r.graph_node_ids.iter().any(|id| id == node_id));
        }
        if let Some(receipt_id) = &query.receipt_id {
            filtered.retain(|r| r.receipt_ids.iter().any(|id| id == receipt_id));
        }
        if let Some(min_confidence) = query.min_confidence {
            filtered.retain(|r| r.confidence.score >= min_confidence);
        }
        if !query.include_expired {
            filtered.retain(|r| !r.status.is_expired());
        }

        filtered.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));

        let total_count = filtered.len();
        let records: Vec<MemoryRecord> = if let Some(limit) = query.limit {
            filtered.into_iter().take(limit).cloned().collect()
        } else {
            filtered.into_iter().cloned().collect()
        };

        MemoryRecallResult {
            records,
            total_count,
            query: query.clone(),
            generated_at: NonEmptyString::new("2026-06-25T12:00:00Z").unwrap(),
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> MemorySnapshot {
        MemorySnapshot::from_index(self)
    }

    pub fn validate(&self) -> Result<(), RivoraError> {
        crate::validation::validate_index(self)
    }

    #[must_use]
    pub fn record_count(&self) -> usize {
        self.records.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::kind::MemoryKind;
    use crate::recall::MemoryRecallQuery;
    use crate::scope::MemoryScope;
    use crate::status::MemoryStatus;

    #[test]
    fn new_creates_empty_index() {
        let index = MemoryIndex::new();
        assert_eq!(index.record_count(), 0);
    }

    #[test]
    fn with_metadata_sets_metadata() {
        let metadata = MemoryMetadata::new().with_organization_id("org-1");
        let index = MemoryIndex::with_metadata(metadata.clone());
        assert_eq!(index.metadata, metadata);
        assert_eq!(index.record_count(), 0);
    }

    #[test]
    fn add_record_succeeds() {
        let mut index = MemoryIndex::new();
        assert!(index.add_record(fixtures::organization_fact()).is_ok());
        assert_eq!(index.record_count(), 1);
    }

    #[test]
    fn add_record_rejects_duplicate() {
        let mut index = MemoryIndex::new();
        index.add_record(fixtures::organization_fact()).unwrap();
        let err = index.add_record(fixtures::organization_fact()).unwrap_err();
        assert!(err.to_string().contains("duplicate"));
    }

    #[test]
    fn get_record_returns_some_for_existing() {
        let mut index = MemoryIndex::new();
        index.add_record(fixtures::organization_fact()).unwrap();
        assert!(index.get_record("mem-fixture-org-fact").is_some());
    }

    #[test]
    fn get_record_returns_none_for_missing() {
        let index = MemoryIndex::new();
        assert!(index.get_record("missing").is_none());
    }

    #[test]
    fn remove_record_returns_removed() {
        let mut index = MemoryIndex::new();
        index.add_record(fixtures::organization_fact()).unwrap();
        let removed = index.remove_record("mem-fixture-org-fact");
        assert!(removed.is_some());
        assert_eq!(index.record_count(), 0);
    }

    #[test]
    fn remove_record_returns_none_for_missing() {
        let mut index = MemoryIndex::new();
        assert!(index.remove_record("missing").is_none());
    }

    #[test]
    fn records_by_kind_filters_correctly() {
        let index = fixtures::sample_index();
        let facts = index.records_by_kind(MemoryKind::Fact);
        assert!(!facts.is_empty());
        let learnings = index.records_by_kind(MemoryKind::IncidentLearning);
        assert!(!learnings.is_empty());
        assert!(index.records_by_kind(MemoryKind::Unknown).is_empty());
    }

    #[test]
    fn records_by_scope_filters_correctly() {
        let index = fixtures::sample_index();
        let org = index.records_by_scope(MemoryScope::Organization);
        assert!(!org.is_empty());
        assert!(index.records_by_scope(MemoryScope::Unknown).is_empty());
    }

    #[test]
    fn records_by_status_filters_correctly() {
        let index = fixtures::sample_index();
        let active = index.records_by_status(MemoryStatus::Active);
        assert!(!active.is_empty());
        let expired = index.records_by_status(MemoryStatus::Expired);
        assert!(!expired.is_empty());
        assert!(index.records_by_status(MemoryStatus::Draft).is_empty());
    }

    #[test]
    fn records_for_graph_node_filters_correctly() {
        let mut index = MemoryIndex::new();
        let mut record = fixtures::organization_fact();
        record.graph_node_ids = vec!["node-org-1".to_string()];
        index.add_record(record).unwrap();
        let results = index.records_for_graph_node("node-org-1");
        assert_eq!(results.len(), 1);
        assert!(index.records_for_graph_node("missing").is_empty());
    }

    #[test]
    fn records_for_receipt_filters_correctly() {
        let index = fixtures::sample_index();
        let results = index.records_for_receipt("receipt-fixture-001");
        assert!(!results.is_empty());
        assert!(index.records_for_receipt("missing").is_empty());
    }

    #[test]
    fn recall_filters_by_kind() {
        let index = fixtures::sample_index();
        let query = MemoryRecallQuery::new().with_kind(MemoryKind::Fact);
        let result = index.recall(&query);
        for record in &result.records {
            assert_eq!(record.kind, MemoryKind::Fact);
        }
    }

    #[test]
    fn recall_filters_by_scope() {
        let index = fixtures::sample_index();
        let query = MemoryRecallQuery::new().with_scope(MemoryScope::Service);
        let result = index.recall(&query);
        for record in &result.records {
            assert_eq!(record.scope, MemoryScope::Service);
        }
    }

    #[test]
    fn recall_filters_by_status() {
        let index = fixtures::sample_index();
        let query = MemoryRecallQuery::new().with_status(MemoryStatus::Active);
        let result = index.recall(&query);
        for record in &result.records {
            assert_eq!(record.status, MemoryStatus::Active);
        }
    }

    #[test]
    fn recall_excludes_expired_by_default() {
        let index = fixtures::sample_index();
        let query = MemoryRecallQuery::new();
        let result = index.recall(&query);
        for record in &result.records {
            assert!(!record.status.is_expired());
        }
    }

    #[test]
    fn recall_includes_expired_when_flag_set() {
        let index = fixtures::sample_index();
        let query = MemoryRecallQuery::new().with_include_expired(true);
        let result = index.recall(&query);
        let has_expired = result.records.iter().any(|r| r.status.is_expired());
        assert!(has_expired);
    }

    #[test]
    fn recall_filters_by_min_confidence() {
        let index = fixtures::sample_index();
        let query = MemoryRecallQuery::new().with_min_confidence(0.9);
        let result = index.recall(&query);
        for record in &result.records {
            assert!(record.confidence.score >= 0.9);
        }
    }

    #[test]
    fn recall_respects_limit() {
        let index = fixtures::sample_index();
        let total = index.record_count();
        let query = MemoryRecallQuery::new()
            .with_include_expired(true)
            .with_limit(2);
        let result = index.recall(&query);
        assert!(result.records.len() <= 2);
        assert_eq!(result.total_count, total);
    }

    #[test]
    fn recall_sorts_by_id() {
        let index = fixtures::sample_index();
        let query = MemoryRecallQuery::new().with_include_expired(true);
        let result = index.recall(&query);
        let ids: Vec<&str> = result.records.iter().map(|r| r.id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn recall_filters_by_subject_ref() {
        let mut index = MemoryIndex::new();
        let mut record = fixtures::organization_fact();
        record.subject_refs = vec![NonEmptyString::new("org-1").unwrap()];
        index.add_record(record).unwrap();
        let query = MemoryRecallQuery::new().with_subject_ref("org-1");
        let result = index.recall(&query);
        assert_eq!(result.records.len(), 1);
    }

    #[test]
    fn recall_filters_by_graph_node_id() {
        let mut index = MemoryIndex::new();
        let mut record = fixtures::organization_fact();
        record.graph_node_ids = vec!["node-org-1".to_string()];
        index.add_record(record).unwrap();
        let query = MemoryRecallQuery::new().with_graph_node_id("node-org-1");
        let result = index.recall(&query);
        assert_eq!(result.records.len(), 1);
    }

    #[test]
    fn recall_filters_by_receipt_id() {
        let index = fixtures::sample_index();
        let query = MemoryRecallQuery::new().with_receipt_id("receipt-fixture-001");
        let result = index.recall(&query);
        for record in &result.records {
            assert!(record
                .receipt_ids
                .iter()
                .any(|id| id == "receipt-fixture-001"));
        }
    }

    #[test]
    fn snapshot_creates_deterministic_snapshot() {
        let index = fixtures::sample_index();
        let snap = index.snapshot();
        assert_eq!(snap.records.len(), index.record_count());
    }

    #[test]
    fn validate_delegates_to_validation_module() {
        let index = fixtures::sample_index();
        assert!(index.validate().is_ok());
    }

    #[test]
    fn record_count_returns_count() {
        let index = fixtures::sample_index();
        assert!(index.record_count() > 0);
    }

    #[test]
    fn index_round_trips_through_serde() {
        let index = fixtures::sample_index();
        let json = serde_json::to_string(&index).unwrap();
        let back: MemoryIndex = serde_json::from_str(&json).unwrap();
        assert_eq!(back, index);
    }
}
