//! The [`StorageProvider`] trait — persistent storage backend.
//!
//! A storage provider represents any persistence layer. Examples include
//! SQLite, redb, Postgres, D1, filesystem, S3, and R2.
//!
//! Storage providers manage content-addressed snapshots, append-mostly
//! graph data (nodes and edges), immutable receipt logs, and read-only
//! queries.
//!
//! # Design principles
//!
//! - **Append-mostly**: nodes, edges, and receipts are never deleted or
//!   updated in place. Corrections create new versioned entries.
//! - **Content-addressed snapshots**: snapshots are identified by their
//!   content hash, making them idempotent and reproducible.
//! - **Atomic writes**: each `put_*` operation is atomic.
//! - **Portable**: no database-specific types; the trait uses only standard
//!   Rust and serde types.

use serde::{Deserialize, Serialize};

use crate::HealthStatus;

/// A content-addressed snapshot of system state.
///
/// Snapshots are immutable and identified by their content hash. They
/// capture a point-in-time view of the context graph for reproducibility
/// and receipt generation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Snapshot {
    /// Content-addressed identifier (hash of the snapshot content).
    pub id: String,
    /// ISO-8601 timestamp of when the snapshot was taken.
    pub created_at: String,
    /// The serialized snapshot content.
    pub content: serde_json::Value,
}

/// A node in the context graph.
///
/// Nodes represent entities (services, deployments, incidents, etc.)
/// observed by connectors.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GraphNode {
    /// Unique identifier for this node.
    pub id: String,
    /// The kind of entity (e.g. `"service"`, `"deployment"`).
    pub kind: String,
    /// Version of this node (nodes are versioned, not updated in place).
    pub version: u64,
    /// The connector that observed this node.
    pub source: String,
    /// ISO-8601 timestamp of when this version was observed.
    pub observed_at: String,
    /// The node's payload.
    pub payload: serde_json::Value,
}

/// A typed edge in the context graph.
///
/// Edges represent relationships between nodes (e.g. `"depends_on"`,
/// `"deployed_in"`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GraphEdge {
    /// Unique identifier for this edge.
    pub id: String,
    /// The kind of relationship (e.g. `"depends_on"`, `"owns"`).
    pub kind: String,
    /// Source node ID.
    pub from: String,
    /// Target node ID.
    pub to: String,
    /// The connector that observed this edge.
    pub source: String,
    /// ISO-8601 timestamp of when this edge was observed.
    pub observed_at: String,
}

/// An immutable reliability receipt.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StoredReceipt {
    /// Unique identifier for this receipt.
    pub id: String,
    /// The kind of receipt (e.g. `"recommendation"`, `"observation"`).
    pub kind: String,
    /// ISO-8601 timestamp of when this receipt was stored.
    pub stored_at: String,
    /// The serialized receipt content.
    pub content: serde_json::Value,
}

/// A read-only query against the storage backend.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StorageQuery {
    /// The entity kind to query (e.g. `"service"`, `"receipt"`).
    pub entity_kind: String,
    /// Optional filter expression (provider-defined).
    pub filter: Option<String>,
    /// Maximum number of results to return.
    pub limit: Option<u64>,
}

/// The result of a storage query.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QueryResult {
    /// The matching entities.
    pub items: Vec<serde_json::Value>,
    /// Total number of matching entities (may exceed `items.len()` if
    /// limited).
    pub total: u64,
}

/// Metadata describing a storage provider's identity and capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StorageMetadata {
    /// Unique identifier for the backend (e.g. `"sqlite"`, `"redb"`).
    pub id: String,
    /// Version of the backend.
    pub version: String,
    /// Human-readable name.
    pub name: String,
    /// Whether the backend supports atomic writes.
    pub atomic_writes: bool,
}

/// A persistent storage backend.
///
/// # Examples
///
/// ```rust
/// use rivora_traits::storage::{
///     StorageProvider, StorageMetadata, Snapshot, GraphNode, GraphEdge,
///     StoredReceipt, StorageQuery, QueryResult,
/// };
/// use rivora_traits::HealthStatus;
///
/// struct NullStorage;
///
/// impl StorageProvider for NullStorage {
///     fn metadata(&self) -> StorageMetadata {
///         StorageMetadata {
///             id: "null".into(),
///             version: "0.1.0".into(),
///             name: "Null Storage".into(),
///             atomic_writes: true,
///         }
///     }
///
///     fn health(&self) -> HealthStatus {
///         HealthStatus::Healthy
///     }
///
///     fn put_snapshot(&self, snapshot: &Snapshot) -> String {
///         snapshot.id.clone()
///     }
///
///     fn get_snapshot(&self, id: &str) -> Option<Snapshot> {
///         None
///     }
///
///     fn put_node(&self, _node: &GraphNode) {}
///     fn put_edge(&self, _edge: &GraphEdge) {}
///     fn put_receipt(&self, _receipt: &StoredReceipt) {}
///
///     fn query(&self, _query: &StorageQuery) -> QueryResult {
///         QueryResult { items: vec![], total: 0 }
///     }
/// }
///
/// let s = NullStorage;
/// assert!(s.health().is_healthy());
/// assert_eq!(s.metadata().id, "null");
/// ```
pub trait StorageProvider: Send + Sync {
    /// Returns metadata identifying this storage backend.
    fn metadata(&self) -> StorageMetadata;

    /// Returns the current health status of the storage backend.
    fn health(&self) -> HealthStatus;

    /// Stores a content-addressed snapshot and returns its ID.
    ///
    /// Snapshots are idempotent: storing the same snapshot twice returns
    /// the same ID without error.
    fn put_snapshot(&self, snapshot: &Snapshot) -> String;

    /// Retrieves a snapshot by its content-addressed ID.
    ///
    /// Returns `None` if the snapshot does not exist.
    fn get_snapshot(&self, id: &str) -> Option<Snapshot>;

    /// Appends a node to the context graph.
    fn put_node(&self, node: &GraphNode);

    /// Appends an edge to the context graph.
    fn put_edge(&self, edge: &GraphEdge);

    /// Appends an immutable receipt to the receipt log.
    fn put_receipt(&self, receipt: &StoredReceipt);

    /// Executes a read-only query against the storage backend.
    fn query(&self, query: &StorageQuery) -> QueryResult;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_round_trips_through_serde() {
        let snap = Snapshot {
            id: "abc123".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            content: serde_json::json!({"services": []}),
        };
        let json = serde_json::to_string(&snap).unwrap();
        let back: Snapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(back, snap);
    }

    #[test]
    fn graph_node_round_trips_through_serde() {
        let node = GraphNode {
            id: "svc-1".into(),
            kind: "service".into(),
            version: 1,
            source: "aws".into(),
            observed_at: "2026-01-01T00:00:00Z".into(),
            payload: serde_json::json!({"name": "api"}),
        };
        let json = serde_json::to_string(&node).unwrap();
        let back: GraphNode = serde_json::from_str(&json).unwrap();
        assert_eq!(back, node);
    }

    #[test]
    fn graph_edge_round_trips_through_serde() {
        let edge = GraphEdge {
            id: "edge-1".into(),
            kind: "depends_on".into(),
            from: "svc-a".into(),
            to: "svc-b".into(),
            source: "k8s".into(),
            observed_at: "2026-01-01T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&edge).unwrap();
        let back: GraphEdge = serde_json::from_str(&json).unwrap();
        assert_eq!(back, edge);
    }

    #[test]
    fn query_result_round_trips_through_serde() {
        let qr = QueryResult {
            items: vec![
                serde_json::json!({"id": "1"}),
                serde_json::json!({"id": "2"}),
            ],
            total: 10,
        };
        let json = serde_json::to_string(&qr).unwrap();
        let back: QueryResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.items.len(), 2);
        assert_eq!(back.total, 10);
    }
}
