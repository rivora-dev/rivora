//! Recall queries and results for the context memory model.

use serde::{Deserialize, Serialize};

use rivora_types::NonEmptyString;

use crate::kind::MemoryKind;
use crate::record::MemoryRecord;
use crate::scope::MemoryScope;
use crate::status::MemoryStatus;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MemoryRecallQuery {
    pub kind: Option<MemoryKind>,
    pub scope: Option<MemoryScope>,
    pub status: Option<MemoryStatus>,
    pub subject_ref: Option<NonEmptyString>,
    pub graph_node_id: Option<String>,
    pub receipt_id: Option<String>,
    pub min_confidence: Option<f64>,
    pub include_expired: bool,
    pub limit: Option<usize>,
}

impl MemoryRecallQuery {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn builder() -> crate::builders::MemoryRecallQueryBuilder {
        crate::builders::MemoryRecallQueryBuilder::new()
    }

    #[must_use]
    pub fn with_kind(mut self, kind: MemoryKind) -> Self {
        self.kind = Some(kind);
        self
    }

    #[must_use]
    pub fn with_scope(mut self, scope: MemoryScope) -> Self {
        self.scope = Some(scope);
        self
    }

    #[must_use]
    pub fn with_status(mut self, status: MemoryStatus) -> Self {
        self.status = Some(status);
        self
    }

    #[must_use]
    pub fn with_subject_ref(mut self, subject_ref: impl Into<String>) -> Self {
        self.subject_ref = NonEmptyString::new(subject_ref.into()).ok();
        self
    }

    #[must_use]
    pub fn with_graph_node_id(mut self, graph_node_id: impl Into<String>) -> Self {
        self.graph_node_id = Some(graph_node_id.into());
        self
    }

    #[must_use]
    pub fn with_receipt_id(mut self, receipt_id: impl Into<String>) -> Self {
        self.receipt_id = Some(receipt_id.into());
        self
    }

    #[must_use]
    pub fn with_min_confidence(mut self, min_confidence: f64) -> Self {
        self.min_confidence = Some(min_confidence);
        self
    }

    #[must_use]
    pub fn with_include_expired(mut self, include_expired: bool) -> Self {
        self.include_expired = include_expired;
        self
    }

    #[must_use]
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryRecallResult {
    pub records: Vec<MemoryRecord>,
    pub total_count: usize,
    pub query: MemoryRecallQuery,
    pub generated_at: NonEmptyString,
}

impl MemoryRecallResult {
    pub fn new(
        query: MemoryRecallQuery,
        records: Vec<MemoryRecord>,
        generated_at: impl Into<String>,
    ) -> Result<Self, rivora_errors::RivoraError> {
        let total_count = records.len();
        Ok(Self {
            records,
            total_count,
            query,
            generated_at: NonEmptyString::new(generated_at.into())?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_new_defaults_to_none_and_false() {
        let q = MemoryRecallQuery::new();
        assert!(q.kind.is_none());
        assert!(q.scope.is_none());
        assert!(q.status.is_none());
        assert!(q.subject_ref.is_none());
        assert!(q.graph_node_id.is_none());
        assert!(q.receipt_id.is_none());
        assert!(q.min_confidence.is_none());
        assert!(!q.include_expired);
        assert!(q.limit.is_none());
    }

    #[test]
    fn query_default_matches_new() {
        assert_eq!(MemoryRecallQuery::new(), MemoryRecallQuery::default());
    }

    #[test]
    fn query_with_kind_sets_kind() {
        let q = MemoryRecallQuery::new().with_kind(MemoryKind::Fact);
        assert_eq!(q.kind, Some(MemoryKind::Fact));
    }

    #[test]
    fn query_with_scope_sets_scope() {
        let q = MemoryRecallQuery::new().with_scope(MemoryScope::Service);
        assert_eq!(q.scope, Some(MemoryScope::Service));
    }

    #[test]
    fn query_with_status_sets_status() {
        let q = MemoryRecallQuery::new().with_status(MemoryStatus::Active);
        assert_eq!(q.status, Some(MemoryStatus::Active));
    }

    #[test]
    fn query_with_subject_ref_sets_ref() {
        let q = MemoryRecallQuery::new().with_subject_ref("org-1");
        assert_eq!(q.subject_ref.as_ref().unwrap().as_str(), "org-1");
    }

    #[test]
    fn query_with_graph_node_id_sets_id() {
        let q = MemoryRecallQuery::new().with_graph_node_id("node-1");
        assert_eq!(q.graph_node_id.as_deref(), Some("node-1"));
    }

    #[test]
    fn query_with_receipt_id_sets_id() {
        let q = MemoryRecallQuery::new().with_receipt_id("receipt_1");
        assert_eq!(q.receipt_id.as_deref(), Some("receipt_1"));
    }

    #[test]
    fn query_with_min_confidence_sets_value() {
        let q = MemoryRecallQuery::new().with_min_confidence(0.5);
        assert_eq!(q.min_confidence, Some(0.5));
    }

    #[test]
    fn query_with_include_expired_sets_true() {
        let q = MemoryRecallQuery::new().with_include_expired(true);
        assert!(q.include_expired);
    }

    #[test]
    fn query_with_limit_sets_value() {
        let q = MemoryRecallQuery::new().with_limit(10);
        assert_eq!(q.limit, Some(10));
    }

    #[test]
    fn query_round_trips_through_serde() {
        let q = MemoryRecallQuery::new()
            .with_kind(MemoryKind::Fact)
            .with_scope(MemoryScope::Organization)
            .with_min_confidence(0.5)
            .with_limit(10);
        let json = serde_json::to_string(&q).unwrap();
        let back: MemoryRecallQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(back, q);
    }

    #[test]
    fn result_new_sets_total_count_from_records() {
        let query = MemoryRecallQuery::new();
        let records = vec![crate::fixtures::organization_fact()];
        let result = MemoryRecallResult::new(query, records, "2026-06-25T12:00:00Z").unwrap();
        assert_eq!(result.total_count, 1);
        assert_eq!(result.records.len(), 1);
        assert_eq!(result.generated_at.as_str(), "2026-06-25T12:00:00Z");
    }

    #[test]
    fn result_new_rejects_empty_generated_at() {
        let query = MemoryRecallQuery::new();
        assert!(MemoryRecallResult::new(query, Vec::new(), "").is_err());
    }

    #[test]
    fn result_round_trips_through_serde() {
        let query = MemoryRecallQuery::new().with_kind(MemoryKind::Fact);
        let records = vec![crate::fixtures::organization_fact()];
        let result = MemoryRecallResult::new(query, records, "2026-06-25T12:00:00Z").unwrap();
        let json = serde_json::to_string(&result).unwrap();
        let back: MemoryRecallResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back, result);
    }
}
