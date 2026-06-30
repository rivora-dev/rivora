//! Deterministic memory snapshots.

use serde::{Deserialize, Serialize};

use crate::index::MemoryIndex;
use crate::metadata::{MemoryMetadata, MemoryTimestamps, MemoryVersion};
use crate::record::MemoryRecord;
use crate::MemoryId;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemorySnapshot {
    pub id: MemoryId,
    pub metadata: MemoryMetadata,
    pub records: Vec<MemoryRecord>,
    pub timestamps: MemoryTimestamps,
    pub version: MemoryVersion,
}

impl MemorySnapshot {
    #[must_use]
    pub fn from_index(index: &MemoryIndex) -> Self {
        let mut records: Vec<MemoryRecord> = index.records.values().cloned().collect();
        records.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        Self {
            id: MemoryId::new_unchecked("memory-snapshot"),
            metadata: index.metadata.clone(),
            records,
            timestamps: MemoryTimestamps::new("2026-06-25T12:00:00Z").unwrap(),
            version: MemoryVersion::new(rivora_types::Version::new(1, 0, 0), 1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    #[test]
    fn snapshot_from_empty_index() {
        let index = fixtures::empty_index();
        let snap = MemorySnapshot::from_index(&index);
        assert_eq!(snap.id.as_str(), "memory-snapshot");
        assert!(snap.records.is_empty());
    }

    #[test]
    fn snapshot_from_sample_index_has_records() {
        let index = fixtures::sample_index();
        let snap = MemorySnapshot::from_index(&index);
        assert_eq!(snap.records.len(), index.record_count());
    }

    #[test]
    fn snapshot_records_sorted_by_id() {
        let index = fixtures::sample_index();
        let snap = MemorySnapshot::from_index(&index);
        let ids: Vec<&str> = snap.records.iter().map(|r| r.id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn snapshot_round_trips_through_serde() {
        let index = fixtures::sample_index();
        let snap = MemorySnapshot::from_index(&index);
        let json = serde_json::to_string(&snap).unwrap();
        let back: MemorySnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(back, snap);
    }

    #[test]
    fn snapshot_is_deterministic() {
        let index = fixtures::sample_index();
        let snap1 = MemorySnapshot::from_index(&index);
        let snap2 = MemorySnapshot::from_index(&index);
        assert_eq!(snap1, snap2);
    }

    #[test]
    fn snapshot_preserves_metadata() {
        let index = fixtures::empty_index();
        let snap = MemorySnapshot::from_index(&index);
        assert_eq!(snap.metadata, index.metadata);
    }
}
