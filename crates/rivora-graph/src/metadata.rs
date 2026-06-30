//! Metadata, timestamps, and version for graph entities.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use rivora_types::{NonEmptyString, Version};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct NodeMetadata {
    pub labels: BTreeMap<NonEmptyString, NonEmptyString>,
}

impl NodeMetadata {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_labels(mut self, labels: BTreeMap<NonEmptyString, NonEmptyString>) -> Self {
        self.labels = labels;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct EdgeMetadata {
    pub labels: BTreeMap<NonEmptyString, NonEmptyString>,
}

impl EdgeMetadata {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_labels(mut self, labels: BTreeMap<NonEmptyString, NonEmptyString>) -> Self {
        self.labels = labels;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct GraphMetadata {
    pub tags: Vec<NonEmptyString>,
    pub labels: BTreeMap<NonEmptyString, NonEmptyString>,
    pub organization_id: Option<NonEmptyString>,
}

impl GraphMetadata {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_tags(mut self, tags: Vec<NonEmptyString>) -> Self {
        self.tags = tags;
        self
    }

    #[must_use]
    pub fn with_labels(mut self, labels: BTreeMap<NonEmptyString, NonEmptyString>) -> Self {
        self.labels = labels;
        self
    }

    #[must_use]
    pub fn with_organization_id(mut self, organization_id: impl Into<String>) -> Self {
        self.organization_id = Some(NonEmptyString::new(organization_id.into()).unwrap());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GraphTimestamps {
    pub created_at: NonEmptyString,
    pub updated_at: Option<NonEmptyString>,
}

impl GraphTimestamps {
    pub fn new(created_at: impl Into<String>) -> Result<Self, rivora_errors::RivoraError> {
        Ok(Self {
            created_at: NonEmptyString::new(created_at.into())?,
            updated_at: None,
        })
    }

    #[must_use]
    pub fn with_updated_at(mut self, updated_at: impl Into<String>) -> Self {
        self.updated_at = Some(NonEmptyString::new(updated_at.into()).unwrap());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GraphVersion {
    pub schema: Version,
    pub graph: u64,
}

impl GraphVersion {
    #[must_use]
    pub fn new(schema: Version, graph: u64) -> Self {
        Self { schema, graph }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_metadata_default_is_empty() {
        let m = NodeMetadata::default();
        assert!(m.labels.is_empty());
    }

    #[test]
    fn node_metadata_round_trips() {
        let mut labels = BTreeMap::new();
        labels.insert(
            NonEmptyString::new("env").unwrap(),
            NonEmptyString::new("prod").unwrap(),
        );
        let m = NodeMetadata::new().with_labels(labels);
        let json = serde_json::to_string(&m).unwrap();
        let back: NodeMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(back, m);
    }

    #[test]
    fn edge_metadata_default_is_empty() {
        let m = EdgeMetadata::default();
        assert!(m.labels.is_empty());
    }

    #[test]
    fn edge_metadata_round_trips() {
        let mut labels = BTreeMap::new();
        labels.insert(
            NonEmptyString::new("weight").unwrap(),
            NonEmptyString::new("high").unwrap(),
        );
        let m = EdgeMetadata::new().with_labels(labels);
        let json = serde_json::to_string(&m).unwrap();
        let back: EdgeMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(back, m);
    }

    #[test]
    fn graph_metadata_default_is_empty() {
        let m = GraphMetadata::default();
        assert!(m.tags.is_empty());
        assert!(m.labels.is_empty());
        assert!(m.organization_id.is_none());
    }

    #[test]
    fn graph_metadata_with_all_fields() {
        let m = GraphMetadata::new()
            .with_tags(vec![NonEmptyString::new("production").unwrap()])
            .with_organization_id("org-1");
        assert_eq!(m.tags.len(), 1);
        assert_eq!(m.organization_id.as_ref().unwrap().as_str(), "org-1");
    }

    #[test]
    fn graph_metadata_round_trips() {
        let m = GraphMetadata::new()
            .with_tags(vec![NonEmptyString::new("payments").unwrap()])
            .with_organization_id("org-1");
        let json = serde_json::to_string(&m).unwrap();
        let back: GraphMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(back, m);
    }

    #[test]
    fn timestamps_rejects_empty_created_at() {
        assert!(GraphTimestamps::new("").is_err());
    }

    #[test]
    fn timestamps_round_trips() {
        let t = GraphTimestamps::new("2026-06-25T12:00:00Z")
            .unwrap()
            .with_updated_at("2026-06-25T13:00:00Z");
        let json = serde_json::to_string(&t).unwrap();
        let back: GraphTimestamps = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn graph_version_new() {
        let v = GraphVersion::new(Version::new(1, 0, 0), 1);
        assert_eq!(v.schema, Version::new(1, 0, 0));
        assert_eq!(v.graph, 1);
    }

    #[test]
    fn graph_version_round_trips() {
        let v = GraphVersion::new(Version::new(1, 0, 0), 5);
        let json = serde_json::to_string(&v).unwrap();
        let back: GraphVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(back, v);
    }
}
