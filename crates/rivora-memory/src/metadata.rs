//! Metadata, timestamps, and version for memory entities.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use rivora_types::{NonEmptyString, Version};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct MemoryMetadata {
    pub tags: Vec<NonEmptyString>,
    pub labels: BTreeMap<NonEmptyString, NonEmptyString>,
    pub organization_id: Option<NonEmptyString>,
}

impl MemoryMetadata {
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
pub struct MemoryTimestamps {
    pub created_at: NonEmptyString,
    pub updated_at: Option<NonEmptyString>,
}

impl MemoryTimestamps {
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
pub struct MemoryVersion {
    pub schema: Version,
    pub memory: u64,
}

impl MemoryVersion {
    #[must_use]
    pub fn new(schema: Version, memory: u64) -> Self {
        Self { schema, memory }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_default_is_empty() {
        let m = MemoryMetadata::default();
        assert!(m.tags.is_empty());
        assert!(m.labels.is_empty());
        assert!(m.organization_id.is_none());
    }

    #[test]
    fn metadata_with_all_fields() {
        let m = MemoryMetadata::new()
            .with_tags(vec![NonEmptyString::new("production").unwrap()])
            .with_organization_id("org-1");
        assert_eq!(m.tags.len(), 1);
        assert_eq!(m.organization_id.as_ref().unwrap().as_str(), "org-1");
    }

    #[test]
    fn metadata_round_trips() {
        let m = MemoryMetadata::new()
            .with_tags(vec![NonEmptyString::new("payments").unwrap()])
            .with_organization_id("org-1");
        let json = serde_json::to_string(&m).unwrap();
        let back: MemoryMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(back, m);
    }

    #[test]
    fn timestamps_rejects_empty_created_at() {
        assert!(MemoryTimestamps::new("").is_err());
    }

    #[test]
    fn timestamps_round_trips() {
        let t = MemoryTimestamps::new("2026-06-25T12:00:00Z")
            .unwrap()
            .with_updated_at("2026-06-25T13:00:00Z");
        let json = serde_json::to_string(&t).unwrap();
        let back: MemoryTimestamps = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn memory_version_new() {
        let v = MemoryVersion::new(Version::new(1, 0, 0), 1);
        assert_eq!(v.schema, Version::new(1, 0, 0));
        assert_eq!(v.memory, 1);
    }

    #[test]
    fn memory_version_round_trips() {
        let v = MemoryVersion::new(Version::new(1, 0, 0), 5);
        let json = serde_json::to_string(&v).unwrap();
        let back: MemoryVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(back, v);
    }
}
