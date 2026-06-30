//! Mock and fake implementations of the core traits for testing.
//!
//! These implementations are intentionally simple and deterministic. They
//! enable contract tests and integration tests without network access or
//! filesystem side effects.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use rivora_traits::connector::{CapabilitySet, Connector, ConnectorMetadata, Observation};
use rivora_traits::health::HealthStatus;
use rivora_traits::idgen::IdGenerator;
use rivora_traits::inference::{
    InferenceMetadata, InferenceProvider, ReasoningRequest, ReasoningResponse,
};
use rivora_traits::logger::{Level, LogEvent, Logger};
use rivora_traits::receipt::{ReceiptRenderer, RenderFormat};
use rivora_traits::storage::{
    GraphEdge, GraphNode, QueryResult, Snapshot, StorageMetadata, StorageProvider, StorageQuery,
    StoredReceipt,
};

// ---------------------------------------------------------------------------
// NullConnector
// ---------------------------------------------------------------------------

/// A connector that returns empty observations and always reports healthy.
///
/// Useful as a default mock when you need a `Connector` implementation but
/// don't care about the observations it returns.
pub struct NullConnector;

impl Connector for NullConnector {
    fn metadata(&self) -> ConnectorMetadata {
        ConnectorMetadata {
            id: "null".into(),
            version: "0.1.0".into(),
            name: "Null Connector".into(),
        }
    }

    fn capabilities(&self) -> CapabilitySet {
        CapabilitySet::new(vec!["read".into()])
    }

    fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }

    fn observe(&self, _scope: &str, _since: Option<&str>) -> Vec<Observation> {
        vec![]
    }
}

// ---------------------------------------------------------------------------
// ScriptedConnector
// ---------------------------------------------------------------------------

/// A connector that returns scripted observations and health status.
///
/// Use `ScriptedConnector::new()` to build, then `.add_observation()` to
/// configure what it returns.
pub struct ScriptedConnector {
    observations: Mutex<Vec<Observation>>,
    health: HealthStatus,
}

impl ScriptedConnector {
    /// Creates a new scripted connector with the given health status.
    #[must_use]
    pub fn new(health: HealthStatus) -> Self {
        Self {
            observations: Mutex::new(vec![]),
            health,
        }
    }

    /// Adds an observation that will be returned by `observe()`.
    pub fn add_observation(&self, obs: Observation) {
        self.observations.lock().unwrap().push(obs);
    }
}

impl Connector for ScriptedConnector {
    fn metadata(&self) -> ConnectorMetadata {
        ConnectorMetadata {
            id: "scripted".into(),
            version: "0.1.0".into(),
            name: "Scripted Connector".into(),
        }
    }

    fn capabilities(&self) -> CapabilitySet {
        CapabilitySet::new(vec!["read".into()])
    }

    fn health(&self) -> HealthStatus {
        self.health.clone()
    }

    fn observe(&self, _scope: &str, _since: Option<&str>) -> Vec<Observation> {
        self.observations.lock().unwrap().clone()
    }
}

// ---------------------------------------------------------------------------
// EchoProvider
// ---------------------------------------------------------------------------

/// An inference provider that echoes the prompt back as reasoning with
/// confidence 1.0.
pub struct EchoProvider;

impl InferenceProvider for EchoProvider {
    fn metadata(&self) -> InferenceMetadata {
        InferenceMetadata {
            id: "echo".into(),
            model: "echo-1".into(),
            version: "0.1.0".into(),
            deterministic: true,
        }
    }

    fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }

    fn reason(&self, request: &ReasoningRequest) -> ReasoningResponse {
        ReasoningResponse {
            reasoning: format!("Echo: {}", request.prompt),
            confidence: 1.0,
            tokens_used: None,
            model: "echo-1".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// ScriptedProvider
// ---------------------------------------------------------------------------

/// An inference provider that returns a scripted response.
pub struct ScriptedProvider {
    response: Mutex<Option<ReasoningResponse>>,
    health: HealthStatus,
}

impl ScriptedProvider {
    /// Creates a new scripted provider with the given health and response.
    #[must_use]
    pub fn new(health: HealthStatus, response: ReasoningResponse) -> Self {
        Self {
            response: Mutex::new(Some(response)),
            health,
        }
    }
}

impl InferenceProvider for ScriptedProvider {
    fn metadata(&self) -> InferenceMetadata {
        InferenceMetadata {
            id: "scripted".into(),
            model: "scripted-1".into(),
            version: "0.1.0".into(),
            deterministic: true,
        }
    }

    fn health(&self) -> HealthStatus {
        self.health.clone()
    }

    fn reason(&self, _request: &ReasoningRequest) -> ReasoningResponse {
        self.response
            .lock()
            .unwrap()
            .take()
            .unwrap_or_else(|| ReasoningResponse {
                reasoning: "no response configured".into(),
                confidence: 0.0,
                tokens_used: None,
                model: "scripted-1".into(),
            })
    }
}

// ---------------------------------------------------------------------------
// NullStorage
// ---------------------------------------------------------------------------

/// A storage provider that stores nothing and always returns empty results.
///
/// Useful as a default mock when you need a `StorageProvider` but don't
/// care about persistence.
pub struct NullStorage;

impl StorageProvider for NullStorage {
    fn metadata(&self) -> StorageMetadata {
        StorageMetadata {
            id: "null".into(),
            version: "0.1.0".into(),
            name: "Null Storage".into(),
            atomic_writes: true,
        }
    }

    fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }

    fn put_snapshot(&self, snapshot: &Snapshot) -> String {
        snapshot.id.clone()
    }

    fn get_snapshot(&self, _id: &str) -> Option<Snapshot> {
        None
    }

    fn put_node(&self, _node: &GraphNode) {}
    fn put_edge(&self, _edge: &GraphEdge) {}
    fn put_receipt(&self, _receipt: &StoredReceipt) {}

    fn query(&self, _query: &StorageQuery) -> QueryResult {
        QueryResult {
            items: vec![],
            total: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// InMemoryStorage
// ---------------------------------------------------------------------------

/// A storage provider that stores everything in memory.
///
/// Useful for integration tests that need to verify storage interactions
/// without touching the filesystem.
pub struct InMemoryStorage {
    snapshots: Mutex<HashMap<String, Snapshot>>,
    nodes: Mutex<Vec<GraphNode>>,
    edges: Mutex<Vec<GraphEdge>>,
    receipts: Mutex<Vec<StoredReceipt>>,
}

impl InMemoryStorage {
    /// Creates a new empty in-memory storage.
    #[must_use]
    pub fn new() -> Self {
        Self {
            snapshots: Mutex::new(HashMap::new()),
            nodes: Mutex::new(vec![]),
            edges: Mutex::new(vec![]),
            receipts: Mutex::new(vec![]),
        }
    }

    /// Returns the number of stored snapshots.
    #[must_use]
    pub fn snapshot_count(&self) -> usize {
        self.snapshots.lock().unwrap().len()
    }

    /// Returns the number of stored nodes.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.lock().unwrap().len()
    }

    /// Returns the number of stored edges.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edges.lock().unwrap().len()
    }

    /// Returns the number of stored receipts.
    #[must_use]
    pub fn receipt_count(&self) -> usize {
        self.receipts.lock().unwrap().len()
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageProvider for InMemoryStorage {
    fn metadata(&self) -> StorageMetadata {
        StorageMetadata {
            id: "in-memory".into(),
            version: "0.1.0".into(),
            name: "In-Memory Storage".into(),
            atomic_writes: true,
        }
    }

    fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }

    fn put_snapshot(&self, snapshot: &Snapshot) -> String {
        let id = snapshot.id.clone();
        self.snapshots
            .lock()
            .unwrap()
            .insert(id.clone(), snapshot.clone());
        id
    }

    fn get_snapshot(&self, id: &str) -> Option<Snapshot> {
        self.snapshots.lock().unwrap().get(id).cloned()
    }

    fn put_node(&self, node: &GraphNode) {
        self.nodes.lock().unwrap().push(node.clone());
    }

    fn put_edge(&self, edge: &GraphEdge) {
        self.edges.lock().unwrap().push(edge.clone());
    }

    fn put_receipt(&self, receipt: &StoredReceipt) {
        self.receipts.lock().unwrap().push(receipt.clone());
    }

    fn query(&self, query: &StorageQuery) -> QueryResult {
        let nodes = self.nodes.lock().unwrap();
        let items: Vec<serde_json::Value> = nodes
            .iter()
            .filter(|n| n.kind == query.entity_kind)
            .map(|n| serde_json::to_value(n).unwrap())
            .take(query.limit.unwrap_or(u64::MAX) as usize)
            .collect();
        let total = nodes.iter().filter(|n| n.kind == query.entity_kind).count() as u64;
        QueryResult { items, total }
    }
}

// ---------------------------------------------------------------------------
// JsonReceiptRenderer
// ---------------------------------------------------------------------------

/// A receipt renderer that outputs JSON.
pub struct JsonReceiptRenderer;

impl ReceiptRenderer for JsonReceiptRenderer {
    fn render(&self, receipt: &serde_json::Value, _format: RenderFormat) -> String {
        serde_json::to_string_pretty(receipt).unwrap()
    }

    fn supported_formats(&self) -> Vec<RenderFormat> {
        vec![RenderFormat::Json]
    }
}

// ---------------------------------------------------------------------------
// MarkdownReceiptRenderer
// ---------------------------------------------------------------------------

/// A receipt renderer that outputs simple Markdown.
pub struct MarkdownReceiptRenderer;

impl ReceiptRenderer for MarkdownReceiptRenderer {
    fn render(&self, receipt: &serde_json::Value, _format: RenderFormat) -> String {
        let mut md = String::new();
        if let Some(title) = receipt.get("title").and_then(|v| v.as_str()) {
            md.push_str(&format!("## {title}\n\n"));
        }
        if let Some(summary) = receipt.get("summary").and_then(|v| v.as_str()) {
            md.push_str(&format!("{summary}\n\n"));
        }
        if let Some(kind) = receipt.get("kind").and_then(|v| v.as_str()) {
            md.push_str(&format!("**Kind:** {kind}\n"));
        }
        if let Some(id) = receipt.get("id").and_then(|v| v.as_str()) {
            md.push_str(&format!("**ID:** `{id}`\n"));
        }
        md
    }

    fn supported_formats(&self) -> Vec<RenderFormat> {
        vec![RenderFormat::Markdown]
    }
}

// ---------------------------------------------------------------------------
// CountingIdGen
// ---------------------------------------------------------------------------

/// An ID generator that produces sequential IDs (`id-0`, `id-1`, ...).
pub struct CountingIdGen {
    counter: AtomicU64,
}

impl CountingIdGen {
    /// Creates a new counting ID generator starting at 0.
    #[must_use]
    pub fn new() -> Self {
        Self {
            counter: AtomicU64::new(0),
        }
    }

    /// Creates a new counting ID generator starting at the given value.
    #[must_use]
    pub fn starting_at(start: u64) -> Self {
        Self {
            counter: AtomicU64::new(start),
        }
    }
}

impl Default for CountingIdGen {
    fn default() -> Self {
        Self::new()
    }
}

impl IdGenerator for CountingIdGen {
    fn generate(&self) -> String {
        let n = self.counter.fetch_add(1, Ordering::Relaxed);
        format!("id-{n}")
    }
}

// ---------------------------------------------------------------------------
// VecLogger
// ---------------------------------------------------------------------------

/// A logger that collects log events in a `Vec` for test assertions.
pub struct VecLogger {
    events: Mutex<Vec<LogEvent>>,
}

impl VecLogger {
    /// Creates a new empty logger.
    #[must_use]
    pub fn new() -> Self {
        Self {
            events: Mutex::new(vec![]),
        }
    }

    /// Returns a clone of all logged events.
    #[must_use]
    pub fn events(&self) -> Vec<LogEvent> {
        self.events.lock().unwrap().clone()
    }

    /// Returns the number of logged events.
    #[must_use]
    pub fn event_count(&self) -> usize {
        self.events.lock().unwrap().len()
    }

    /// Returns `true` if any event matches the given level and contains the
    /// given message substring.
    #[must_use]
    pub fn has_event(&self, level: Level, message_contains: &str) -> bool {
        self.events
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.level == level && e.message.contains(message_contains))
    }
}

impl Default for VecLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl Logger for VecLogger {
    fn log(&self, event: LogEvent) {
        self.events.lock().unwrap().push(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_connector_is_healthy() {
        let c = NullConnector;
        assert!(c.health().is_healthy());
        assert_eq!(c.metadata().id, "null");
        assert!(c.observe("anything", None).is_empty());
    }

    #[test]
    fn scripted_connector_returns_observations() {
        let c = ScriptedConnector::new(HealthStatus::Healthy);
        c.add_observation(Observation {
            source: "test".into(),
            source_version: "0.1.0".into(),
            observed_at: "2026-01-01T00:00:00Z".into(),
            raw_ref: "ref-1".into(),
            kind: "service".into(),
            payload: serde_json::json!({}),
        });
        let obs = c.observe("services", None);
        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0].kind, "service");
    }

    #[test]
    fn echo_provider_echoes_prompt() {
        let p = EchoProvider;
        let resp = p.reason(&ReasoningRequest {
            prompt: "hello".into(),
            context: vec![],
            deterministic: true,
            temperature: None,
        });
        assert_eq!(resp.reasoning, "Echo: hello");
        assert_eq!(resp.confidence, 1.0);
    }

    #[test]
    fn null_storage_is_healthy() {
        let s = NullStorage;
        assert!(s.health().is_healthy());
        assert!(s.get_snapshot("anything").is_none());
    }

    #[test]
    fn in_memory_storage_stores_and_retrieves() {
        let s = InMemoryStorage::new();
        let snap = Snapshot {
            id: "snap-1".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            content: serde_json::json!({"key": "value"}),
        };
        let returned_id = s.put_snapshot(&snap);
        assert_eq!(returned_id, "snap-1");
        assert_eq!(s.snapshot_count(), 1);
        let retrieved = s.get_snapshot("snap-1").unwrap();
        assert_eq!(retrieved.id, "snap-1");
    }

    #[test]
    fn in_memory_storage_nodes_and_edges() {
        let s = InMemoryStorage::new();
        s.put_node(&GraphNode {
            id: "n1".into(),
            kind: "service".into(),
            version: 1,
            source: "test".into(),
            observed_at: "2026-01-01T00:00:00Z".into(),
            payload: serde_json::json!({}),
        });
        assert_eq!(s.node_count(), 1);
        s.put_edge(&GraphEdge {
            id: "e1".into(),
            kind: "depends_on".into(),
            from: "n1".into(),
            to: "n2".into(),
            source: "test".into(),
            observed_at: "2026-01-01T00:00:00Z".into(),
        });
        assert_eq!(s.edge_count(), 1);
    }

    #[test]
    fn json_renderer_outputs_json() {
        let r = JsonReceiptRenderer;
        let receipt = serde_json::json!({"id": "r1", "title": "Test"});
        let output = r.render(&receipt, RenderFormat::Json);
        assert!(output.contains("r1"));
        assert!(output.contains("Test"));
    }

    #[test]
    fn markdown_renderer_outputs_markdown() {
        let r = MarkdownReceiptRenderer;
        let receipt = serde_json::json!({
            "id": "r1",
            "title": "Test Receipt",
            "summary": "This is a test",
            "kind": "recommendation"
        });
        let output = r.render(&receipt, RenderFormat::Markdown);
        assert!(output.contains("## Test Receipt"));
        assert!(output.contains("This is a test"));
        assert!(output.contains("recommendation"));
    }

    #[test]
    fn counting_id_gen_sequential() {
        let gen = CountingIdGen::new();
        assert_eq!(gen.generate(), "id-0");
        assert_eq!(gen.generate(), "id-1");
    }

    #[test]
    fn counting_id_gen_starting_at() {
        let gen = CountingIdGen::starting_at(10);
        assert_eq!(gen.generate(), "id-10");
        assert_eq!(gen.generate(), "id-11");
    }

    #[test]
    fn vec_logger_collects_events() {
        let logger = VecLogger::new();
        logger.log(LogEvent {
            level: Level::Info,
            message: "server started".into(),
            fields: HashMap::new(),
        });
        assert_eq!(logger.event_count(), 1);
        assert!(logger.has_event(Level::Info, "server started"));
        assert!(!logger.has_event(Level::Error, "server started"));
    }
}
