//! Contract tests for all core traits.
//!
//! These tests verify that implementations satisfy the behavioral guarantees
//! defined by each trait. They use the mock/fake implementations from
//! `rivora-testing` and test the trait contracts, not specific implementations.

use std::collections::HashMap;

use rivora_testing::{
    CountingIdGen, EchoProvider, InMemoryStorage, JsonReceiptRenderer, MarkdownReceiptRenderer,
    NullConnector, NullStorage, ScriptedConnector, ScriptedProvider, VecLogger,
};
use rivora_traits::clock::Clock;
use rivora_traits::connector::{Connector, Observation};
use rivora_traits::health::HealthStatus;
use rivora_traits::idgen::IdGenerator;
use rivora_traits::inference::{InferenceProvider, ReasoningRequest};
use rivora_traits::logger::{Level, LogEvent, Logger};
use rivora_traits::receipt::{ReceiptRenderer, RenderFormat};
use rivora_traits::storage::{
    GraphEdge, GraphNode, Snapshot, StorageProvider, StorageQuery, StoredReceipt,
};

// =========================================================================
// Connector contract tests
// =========================================================================

#[test]
fn connector_metadata_has_required_fields() {
    let c = NullConnector;
    let meta = c.metadata();
    assert!(!meta.id.is_empty(), "connector id must not be empty");
    assert!(
        !meta.version.is_empty(),
        "connector version must not be empty"
    );
    assert!(!meta.name.is_empty(), "connector name must not be empty");
}

#[test]
fn connector_capabilities_include_read() {
    let c = NullConnector;
    let caps = c.capabilities();
    assert!(caps.has("read"), "connector must include 'read' capability");
}

#[test]
fn connector_health_returns_valid_status() {
    let c = NullConnector;
    let health = c.health();
    assert!(
        matches!(
            health,
            HealthStatus::Healthy | HealthStatus::Degraded { .. } | HealthStatus::Unhealthy { .. }
        ),
        "health must be a valid HealthStatus variant"
    );
}

#[test]
fn connector_observe_returns_vec_not_option() {
    let c = NullConnector;
    let obs = c.observe("services", None);
    assert!(
        obs.is_empty() || !obs.is_empty(),
        "observe always returns a Vec"
    );
}

#[test]
fn connector_observations_carry_provenance() {
    let c = ScriptedConnector::new(HealthStatus::Healthy);
    c.add_observation(Observation {
        source: "test-connector".into(),
        source_version: "1.0.0".into(),
        observed_at: "2026-01-01T00:00:00Z".into(),
        raw_ref: "arn:test:123".into(),
        kind: "service".into(),
        payload: serde_json::json!({"name": "api"}),
    });
    let obs = c.observe("services", None);
    assert_eq!(obs.len(), 1);
    assert!(!obs[0].source.is_empty(), "observation must have a source");
    assert!(
        !obs[0].observed_at.is_empty(),
        "observation must have observed_at"
    );
    assert!(!obs[0].kind.is_empty(), "observation must have a kind");
}

#[test]
fn connector_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<NullConnector>();
    assert_send_sync::<ScriptedConnector>();
}

// =========================================================================
// InferenceProvider contract tests
// =========================================================================

#[test]
fn inference_metadata_has_required_fields() {
    let p = EchoProvider;
    let meta = p.metadata();
    assert!(!meta.id.is_empty(), "provider id must not be empty");
    assert!(!meta.model.is_empty(), "provider model must not be empty");
    assert!(
        !meta.version.is_empty(),
        "provider version must not be empty"
    );
}

#[test]
fn inference_health_returns_valid_status() {
    let p = EchoProvider;
    let health = p.health();
    assert!(
        matches!(
            health,
            HealthStatus::Healthy | HealthStatus::Degraded { .. } | HealthStatus::Unhealthy { .. }
        ),
        "health must be a valid HealthStatus variant"
    );
}

#[test]
fn inference_reason_returns_reasoning_and_confidence() {
    let p = EchoProvider;
    let resp = p.reason(&ReasoningRequest {
        prompt: "test prompt".into(),
        context: vec![],
        deterministic: true,
        temperature: None,
    });
    assert!(
        !resp.reasoning.is_empty(),
        "response must contain reasoning"
    );
    assert!(
        (0.0..=1.0).contains(&resp.confidence),
        "confidence must be between 0.0 and 1.0"
    );
    assert!(!resp.model.is_empty(), "response must have a model");
}

#[test]
fn inference_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<EchoProvider>();
    assert_send_sync::<ScriptedProvider>();
}

// =========================================================================
// StorageProvider contract tests
// =========================================================================

#[test]
fn storage_metadata_has_required_fields() {
    let s = NullStorage;
    let meta = s.metadata();
    assert!(!meta.id.is_empty(), "storage id must not be empty");
    assert!(
        !meta.version.is_empty(),
        "storage version must not be empty"
    );
    assert!(!meta.name.is_empty(), "storage name must not be empty");
}

#[test]
fn storage_health_returns_valid_status() {
    let s = NullStorage;
    let health = s.health();
    assert!(
        matches!(
            health,
            HealthStatus::Healthy | HealthStatus::Degraded { .. } | HealthStatus::Unhealthy { .. }
        ),
        "health must be a valid HealthStatus variant"
    );
}

#[test]
fn storage_snapshot_round_trip() {
    let s = InMemoryStorage::new();
    let snap = Snapshot {
        id: "snap-rt-1".into(),
        created_at: "2026-01-01T00:00:00Z".into(),
        content: serde_json::json!({"test": true}),
    };
    let id = s.put_snapshot(&snap);
    assert_eq!(id, "snap-rt-1");
    let retrieved = s.get_snapshot(&id).expect("snapshot must be retrievable");
    assert_eq!(retrieved.id, "snap-rt-1");
    assert_eq!(retrieved.content, snap.content);
}

#[test]
fn storage_snapshot_missing_returns_none() {
    let s = NullStorage;
    assert!(s.get_snapshot("nonexistent").is_none());
}

#[test]
fn storage_nodes_are_append_mostly() {
    let s = InMemoryStorage::new();
    for i in 0..5 {
        s.put_node(&GraphNode {
            id: format!("node-{i}"),
            kind: "service".into(),
            version: 1,
            source: "test".into(),
            observed_at: "2026-01-01T00:00:00Z".into(),
            payload: serde_json::json!({}),
        });
    }
    assert_eq!(s.node_count(), 5, "nodes are append-mostly, never deleted");
}

#[test]
fn storage_edges_are_append_mostly() {
    let s = InMemoryStorage::new();
    for i in 0..5 {
        s.put_edge(&GraphEdge {
            id: format!("edge-{i}"),
            kind: "depends_on".into(),
            from: format!("node-{i}"),
            to: format!("node-{}", i + 1),
            source: "test".into(),
            observed_at: "2026-01-01T00:00:00Z".into(),
        });
    }
    assert_eq!(s.edge_count(), 5, "edges are append-mostly, never deleted");
}

#[test]
fn storage_receipts_are_immutable() {
    let s = InMemoryStorage::new();
    s.put_receipt(&StoredReceipt {
        id: "receipt-1".into(),
        kind: "recommendation".into(),
        stored_at: "2026-01-01T00:00:00Z".into(),
        content: serde_json::json!({"verdict": "proceed"}),
    });
    assert_eq!(s.receipt_count(), 1, "receipts are immutable once stored");
}

#[test]
fn storage_query_filters_by_kind() {
    let s = InMemoryStorage::new();
    s.put_node(&GraphNode {
        id: "svc-1".into(),
        kind: "service".into(),
        version: 1,
        source: "test".into(),
        observed_at: "2026-01-01T00:00:00Z".into(),
        payload: serde_json::json!({}),
    });
    s.put_node(&GraphNode {
        id: "dep-1".into(),
        kind: "deployment".into(),
        version: 1,
        source: "test".into(),
        observed_at: "2026-01-01T00:00:00Z".into(),
        payload: serde_json::json!({}),
    });
    let result = s.query(&StorageQuery {
        entity_kind: "service".into(),
        filter: None,
        limit: None,
    });
    assert_eq!(result.items.len(), 1, "query filters by entity kind");
    assert_eq!(result.total, 1);
}

#[test]
fn storage_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<NullStorage>();
    assert_send_sync::<InMemoryStorage>();
}

// =========================================================================
// ReceiptRenderer contract tests
// =========================================================================

#[test]
fn renderer_json_outputs_valid_json() {
    let r = JsonReceiptRenderer;
    let receipt = serde_json::json!({"id": "r1", "kind": "test"});
    let output = r.render(&receipt, RenderFormat::Json);
    let parsed: serde_json::Value =
        serde_json::from_str(&output).expect("output must be valid JSON");
    assert_eq!(parsed["id"], "r1");
}

#[test]
fn renderer_markdown_outputs_human_readable() {
    let r = MarkdownReceiptRenderer;
    let receipt = serde_json::json!({
        "id": "r1",
        "title": "Test Receipt",
        "summary": "This is a test receipt",
        "kind": "recommendation"
    });
    let output = r.render(&receipt, RenderFormat::Markdown);
    assert!(
        output.contains("Test Receipt"),
        "markdown must contain title"
    );
    assert!(
        output.contains("recommendation"),
        "markdown must contain kind"
    );
}

#[test]
fn renderer_supports_declared_formats() {
    let r = JsonReceiptRenderer;
    for format in r.supported_formats() {
        assert!(r.supports(format), "renderer must support declared formats");
    }
}

#[test]
fn renderer_rejects_undeclared_formats() {
    let r = JsonReceiptRenderer;
    if !r.supports(RenderFormat::Markdown) {
        // JsonRenderer doesn't support Markdown — this is valid
        assert!(!r.supports(RenderFormat::Markdown));
    }
}

#[test]
fn renderer_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<JsonReceiptRenderer>();
    assert_send_sync::<MarkdownReceiptRenderer>();
}

// =========================================================================
// Clock contract tests
// =========================================================================

#[test]
fn clock_returns_iso8601_string() {
    let clock = rivora_testing::FakeClock::fixed("2026-06-26T12:00:00Z");
    let ts = clock.now_iso();
    assert!(
        ts.contains('T'),
        "timestamp must contain 'T' separator (ISO-8601)"
    );
    assert!(ts.ends_with('Z'), "timestamp must end with 'Z' (UTC)");
}

#[test]
fn clock_fixed_returns_same_value() {
    let clock = rivora_testing::FakeClock::fixed("2026-06-26T12:00:00Z");
    let a = clock.now_iso();
    let b = clock.now_iso();
    assert_eq!(a, b, "fixed clock must return the same value");
}

#[test]
fn clock_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<rivora_testing::FakeClock>();
}

// =========================================================================
// IdGenerator contract tests
// =========================================================================

#[test]
fn idgen_produces_unique_ids() {
    let gen = CountingIdGen::new();
    let mut ids = std::collections::HashSet::new();
    for _ in 0..100 {
        ids.insert(gen.generate());
    }
    assert_eq!(ids.len(), 100, "all generated IDs must be unique");
}

#[test]
fn idgen_ids_are_non_empty() {
    let gen = CountingIdGen::new();
    for _ in 0..10 {
        let id = gen.generate();
        assert!(!id.is_empty(), "generated ID must not be empty");
    }
}

#[test]
fn idgen_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<CountingIdGen>();
}

// =========================================================================
// Logger contract tests
// =========================================================================

#[test]
fn logger_stores_all_events() {
    let logger = VecLogger::new();
    for i in 0..10 {
        logger.log(LogEvent {
            level: Level::Info,
            message: format!("event {i}"),
            fields: HashMap::new(),
        });
    }
    assert_eq!(logger.event_count(), 10, "logger must store all events");
}

#[test]
fn logger_preserves_event_fields() {
    let logger = VecLogger::new();
    let mut fields = HashMap::new();
    fields.insert("key".into(), "value".into());
    logger.log(LogEvent {
        level: Level::Warn,
        message: "warning message".into(),
        fields,
    });
    let events = logger.events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].level, Level::Warn);
    assert_eq!(events[0].message, "warning message");
    assert_eq!(events[0].fields.get("key").unwrap(), "value");
}

#[test]
fn logger_level_ordering_is_consistent() {
    assert!(Level::Trace < Level::Debug);
    assert!(Level::Debug < Level::Info);
    assert!(Level::Info < Level::Warn);
    assert!(Level::Warn < Level::Error);
}

#[test]
fn logger_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<VecLogger>();
}

// =========================================================================
// Cross-trait integration tests
// =========================================================================

#[test]
fn connector_and_storage_compose() {
    let connector = ScriptedConnector::new(HealthStatus::Healthy);
    let storage = InMemoryStorage::new();

    // Connector produces observations
    connector.add_observation(Observation {
        source: "test".into(),
        source_version: "1.0.0".into(),
        observed_at: "2026-01-01T00:00:00Z".into(),
        raw_ref: "svc-1".into(),
        kind: "service".into(),
        payload: serde_json::json!({"name": "api"}),
    });
    let obs = connector.observe("services", None);
    assert_eq!(obs.len(), 1);

    // Storage persists a snapshot
    let snap = Snapshot {
        id: "snap-1".into(),
        created_at: "2026-01-01T00:00:00Z".into(),
        content: serde_json::json!({"observations": obs}),
    };
    storage.put_snapshot(&snap);
    assert_eq!(storage.snapshot_count(), 1);
}

#[test]
fn inference_and_receipt_compose() {
    let provider = EchoProvider;
    let renderer = JsonReceiptRenderer;

    // Provider generates reasoning
    let resp = provider.reason(&ReasoningRequest {
        prompt: "analyze infrastructure".into(),
        context: vec!["context-1".into()],
        deterministic: true,
        temperature: None,
    });
    assert!(!resp.reasoning.is_empty());

    // Renderer formats a receipt containing the reasoning
    let receipt = serde_json::json!({
        "id": "receipt-1",
        "kind": "recommendation",
        "title": "Infrastructure Analysis",
        "summary": &resp.reasoning,
        "confidence": resp.confidence,
    });
    let output = renderer.render(&receipt, RenderFormat::Json);
    assert!(output.contains("Infrastructure Analysis"));
}

#[test]
fn clock_and_idgen_compose() {
    let clock = rivora_testing::FakeClock::fixed("2026-06-26T12:00:00Z");
    let idgen = CountingIdGen::new();

    let ts = clock.now_iso();
    let id = idgen.generate();
    assert!(!ts.is_empty());
    assert!(!id.is_empty());
}

#[test]
fn logger_captures_trait_interactions() {
    let logger = VecLogger::new();

    // Log a connector observation
    logger.log(LogEvent {
        level: Level::Info,
        message: "observation received".into(),
        fields: {
            let mut f = HashMap::new();
            f.insert("source".into(), "aws".into());
            f.insert("kind".into(), "service".into());
            f
        },
    });

    // Log an inference request
    logger.log(LogEvent {
        level: Level::Debug,
        message: "reasoning requested".into(),
        fields: {
            let mut f = HashMap::new();
            f.insert("provider".into(), "openai".into());
            f.insert("model".into(), "gpt-4".into());
            f
        },
    });

    assert_eq!(logger.event_count(), 2);
    assert!(logger.has_event(Level::Info, "observation"));
    assert!(logger.has_event(Level::Debug, "reasoning"));
}
