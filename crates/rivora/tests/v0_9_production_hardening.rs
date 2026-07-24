//! v0.9 Production Hardening — automated production scenarios and gates.
//!
//! Covers store lock, corruption isolation, payload limits, idempotent
//! replay, backup/restore, schema mismatch, concurrent open, and
//! architecture safety for production envelopes.

use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use rivora::domain::{
    CliExitCode, FailureClass, ObservationKind, OperatingEnvelope, OperatingProfile,
    PerformanceBudget, Provenance, ReplayContract, MAX_PAYLOAD_BYTES, STORE_SCHEMA_VERSION,
};
use rivora::storage::{LocalStore, Store};
use rivora::{CapabilityService, RivoraError, Runtime};
use serde_json::json;
use tempfile::tempdir;

fn caps_only(path: &std::path::Path) -> CapabilityService {
    let store = Arc::new(LocalStore::open(path).expect("open"));
    CapabilityService::new(Arc::new(Runtime::new(store)))
}

#[test]
fn store_manifest_is_written_on_open() {
    let dir = tempdir().unwrap();
    let store = LocalStore::open(dir.path()).unwrap();
    assert!(dir.path().join("store.json").exists());
    let health = store.health_report().unwrap();
    assert_eq!(health.schema_version, STORE_SCHEMA_VERSION);
    assert!(health.lock_held);
    assert!(health.is_healthy());
}

#[test]
fn same_process_store_open_is_reentrant() {
    let dir = tempdir().unwrap();
    let first = LocalStore::open(dir.path()).unwrap();
    let second = LocalStore::open(dir.path()).unwrap();
    assert!(first.lock_held() && second.lock_held());
    drop(second);
    assert!(dir.path().join(".rivora.lock").exists());
    drop(first);
    assert!(!dir.path().join(".rivora.lock").exists());
}

#[test]
fn store_locked_error_maps_to_lock_conflict_exit() {
    let err = RivoraError::store_locked("held by other process");
    assert_eq!(err.exit_code(), CliExitCode::LockConflict);
    assert_eq!(err.failure_class(), FailureClass::Blocked);
}

#[test]
fn stale_lock_recovery_then_open() {
    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join(".rivora.lock"),
        "pid=999999\ncreated_at=1\n",
    )
    .unwrap();
    assert!(LocalStore::recover_stale_lock(dir.path()).unwrap());
    let store = LocalStore::open(dir.path()).unwrap();
    assert!(store.lock_held());
}

#[test]
fn duplicate_ingestion_is_idempotent() {
    let dir = tempdir().unwrap();
    let caps = caps_only(dir.path());
    let inv = caps.create_investigation("dup", None, "test").unwrap();
    let (o1, m1, replay1) = caps
        .ingest_observation(
            inv.id,
            ObservationKind::Event,
            "same event",
            json!({"n": 1}),
            "test",
            Utc::now(),
            Some("dup-key-1".into()),
            "test",
        )
        .unwrap();
    assert!(!replay1);
    let (o2, m2, replay2) = caps
        .ingest_observation(
            inv.id,
            ObservationKind::Event,
            "same event again",
            json!({"n": 2}),
            "test",
            Utc::now(),
            Some("dup-key-1".into()),
            "test",
        )
        .unwrap();
    assert!(replay2);
    assert_eq!(o1.id, o2.id);
    assert_eq!(m1.id, m2.id);
    let memory = caps.recall_memory(inv.id).unwrap();
    assert_eq!(memory.len(), 1);
}

#[test]
fn oversized_payload_is_rejected() {
    let dir = tempdir().unwrap();
    let caps = caps_only(dir.path());
    let inv = caps.create_investigation("big", None, "test").unwrap();
    let big = "x".repeat(MAX_PAYLOAD_BYTES + 64);
    let err = caps
        .ingest_observation(
            inv.id,
            ObservationKind::Event,
            "too big",
            json!({ "blob": big }),
            "test",
            Utc::now(),
            None,
            "test",
        )
        .unwrap_err();
    assert!(matches!(err, RivoraError::PayloadTooLarge(_)));
    assert_eq!(err.exit_code(), CliExitCode::Validation);
}

#[test]
fn corrupt_observation_does_not_block_healthy_reads() {
    let dir = tempdir().unwrap();
    let store = LocalStore::open(dir.path()).unwrap();
    let inv = rivora::Investigation::create("c", None, Provenance::now("t", "t")).unwrap();
    store.save_investigation(&inv).unwrap();
    let obs = rivora::Observation::new(
        inv.id,
        ObservationKind::Event,
        "ok",
        json!({}),
        "test",
        Utc::now(),
        Some("k".into()),
        Provenance::now("t", "t"),
    )
    .unwrap();
    store.append_observation(&obs).unwrap();
    let bad = store
        .root()
        .join("investigations")
        .join(inv.id.to_string())
        .join("observations")
        .join("broken.json");
    std::fs::write(bad, "{nope").unwrap();
    let listed = store.list_observations(&inv.id).unwrap();
    assert_eq!(listed.len(), 1);
    let health = store.health_report().unwrap();
    assert!(!health.corrupt_records.is_empty());
    assert!(!health.is_healthy());
}

#[test]
fn backup_and_restore_preserves_records() {
    let dir = tempdir().unwrap();
    let backup_root = tempdir().unwrap();
    let store = LocalStore::open(dir.path()).unwrap();
    let inv = rivora::Investigation::create("b", None, Provenance::now("t", "t")).unwrap();
    store.save_investigation(&inv).unwrap();
    // Destination must be outside the store root (nested backups recurse).
    let backup = backup_root.path().join("backup-copy");
    store.backup_to(&backup).unwrap();
    drop(store);
    let restored = LocalStore::open(&backup).unwrap();
    let loaded = restored.load_investigation(&inv.id).unwrap();
    assert_eq!(loaded.title, "b");
}

#[test]
fn rebuild_indexes_from_canonical_records() {
    let dir = tempdir().unwrap();
    let store = LocalStore::open(dir.path()).unwrap();
    let inv = rivora::Investigation::create("idx", None, Provenance::now("t", "t")).unwrap();
    store.save_investigation(&inv).unwrap();
    let obs = rivora::Observation::new(
        inv.id,
        ObservationKind::Event,
        "indexed",
        json!({}),
        "test",
        Utc::now(),
        Some("idx-key".into()),
        Provenance::now("t", "t"),
    )
    .unwrap();
    store.append_observation(&obs).unwrap();
    // Drop index tree and rebuild.
    let index_dir = store
        .root()
        .join("investigations")
        .join(inv.id.to_string())
        .join("indexes");
    let _ = std::fs::remove_dir_all(&index_dir);
    let n = store.rebuild_observation_indexes().unwrap();
    assert_eq!(n, 1);
    let found = store
        .find_observation_by_idempotency(&inv.id, "idx-key")
        .unwrap()
        .expect("found via rebuilt index");
    assert_eq!(found.id, obs.id);
}

#[test]
fn old_store_without_manifest_opens_and_migrates() {
    let dir = tempdir().unwrap();
    // Simulate pre-v0.9 layout: investigations only.
    std::fs::create_dir_all(dir.path().join("investigations")).unwrap();
    let store = LocalStore::open(dir.path()).unwrap();
    assert!(dir.path().join("store.json").exists());
    let health = store.health_report().unwrap();
    assert_eq!(health.migration_status, "compatible");
    assert!(health.supported_prior_versions.iter().any(|v| v == "0.8"));
}

#[test]
fn diagnostic_export_is_sanitized_json() {
    let dir = tempdir().unwrap();
    let store = LocalStore::open(dir.path()).unwrap();
    let export = store.diagnostic_export().unwrap();
    assert_eq!(export["schema_version"], 1);
    assert!(export.get("health").is_some());
    assert!(export.get("operating_envelope").is_some());
    assert!(export.get("replay_contracts").is_some());
    let text = export.to_string();
    assert!(!text.contains("Bearer "));
    assert!(!text.contains("password="));
}

#[test]
fn operating_envelope_and_budgets_are_defined() {
    let small = OperatingEnvelope::small();
    let medium = OperatingEnvelope::medium();
    let large = OperatingEnvelope::large_supported();
    assert!(small.max_investigations_per_store < medium.max_investigations_per_store);
    assert!(medium.max_investigations_per_store < large.max_investigations_per_store);
    assert_eq!(medium.profile, OperatingProfile::Medium);
    let budgets = PerformanceBudget::v0_9_budgets();
    assert!(budgets.len() >= 15);
    for c in ReplayContract::v0_9_contracts() {
        assert!(!c.dry_run_suppresses_live);
        assert!(!c.retry_bypasses_authority);
    }
}

#[test]
fn error_exit_code_mapping_is_stable() {
    assert_eq!(
        RivoraError::validation("x").exit_code(),
        CliExitCode::Validation
    );
    assert_eq!(
        RivoraError::store_locked("x").exit_code(),
        CliExitCode::LockConflict
    );
    assert_eq!(RivoraError::timeout("x").exit_code(), CliExitCode::Timeout);
    assert_eq!(
        RivoraError::RateLimited("x".into()).exit_code(),
        CliExitCode::ProviderFailure
    );
    assert_eq!(RivoraError::partial("x").exit_code(), CliExitCode::Partial);
    assert_eq!(
        RivoraError::SchemaMismatch {
            found: 9,
            supported_max: 1
        }
        .exit_code(),
        CliExitCode::SchemaMismatch
    );
}

#[test]
fn micro_benchmarks_within_budgets() {
    let dir = tempdir().unwrap();
    let store_path = dir.path();

    let t0 = Instant::now();
    let store = LocalStore::open(store_path).unwrap();
    // Gate against max budgets (not tight targets) to absorb host variance.
    let open_ms = t0.elapsed().as_millis() as u64;
    assert!(open_ms < 250, "store_open budget exceeded: {open_ms}ms");

    let inv = rivora::Investigation::create("bench", None, Provenance::now("t", "t")).unwrap();
    let t1 = Instant::now();
    store.save_investigation(&inv).unwrap();
    let write_ms = t1.elapsed().as_millis() as u64;
    assert!(
        write_ms < 250,
        "persistence_write budget exceeded: {write_ms}ms"
    );

    let t2 = Instant::now();
    let _ = store.load_investigation(&inv.id).unwrap();
    let read_ms = t2.elapsed().as_millis() as u64;
    assert!(
        read_ms < 250,
        "persistence_read budget exceeded: {read_ms}ms"
    );

    drop(store);
    let caps = caps_only(store_path);
    let inv = caps
        .create_investigation("ingest-bench", None, "t")
        .unwrap();
    let t3 = Instant::now();
    caps.ingest_observation(
        inv.id,
        ObservationKind::Event,
        "bench",
        json!({"ok": true}),
        "bench",
        Utc::now(),
        Some("bench-key".into()),
        "t",
    )
    .unwrap();
    let ingest_ms = t3.elapsed().as_millis() as u64;
    assert!(ingest_ms < 250, "ingestion budget exceeded: {ingest_ms}ms");

    let t4 = Instant::now();
    let (_, _, replay) = caps
        .ingest_observation(
            inv.id,
            ObservationKind::Event,
            "bench",
            json!({"ok": true}),
            "bench",
            Utc::now(),
            Some("bench-key".into()),
            "t",
        )
        .unwrap();
    assert!(replay);
    let dup_ms = t4.elapsed().as_millis() as u64;
    assert!(
        dup_ms < 250,
        "duplicate_ingestion budget exceeded: {dup_ms}ms"
    );

    let t5 = Instant::now();
    let _ = caps.store_health().unwrap();
    let health_ms = t5.elapsed().as_millis() as u64;
    assert!(
        health_ms < 2_000,
        "diagnostic health budget exceeded: {health_ms}ms"
    );
}

#[test]
fn production_scenario_large_investigation_bounded() {
    let dir = tempdir().unwrap();
    let caps = caps_only(dir.path());
    let inv = caps.create_investigation("large", None, "test").unwrap();
    // Medium-small synthetic load within CI budget.
    for i in 0..50 {
        caps.ingest_observation(
            inv.id,
            ObservationKind::Event,
            format!("event {i}"),
            json!({"i": i}),
            "bench",
            Utc::now(),
            Some(format!("large-key-{i}")),
            "test",
        )
        .unwrap();
    }
    let t0 = Instant::now();
    let memory = caps.recall_memory(inv.id).unwrap();
    let load_ms = t0.elapsed().as_millis() as u64;
    assert_eq!(memory.len(), 50);
    assert!(
        load_ms < 3_000,
        "large investigation load exceeded budget: {load_ms}ms"
    );
}

#[test]
fn connector_redaction_and_batch_bounds() {
    use rivora_connectors::{bound_batch, max_event_batch_size, redact_json, sanitize_error};

    let mut v = json!({
        "token": "sekrit",
        "nested": { "password": "p", "ok": 1 }
    });
    redact_json(&mut v);
    assert_eq!(v["token"], "[redacted]");
    assert_eq!(v["nested"]["password"], "[redacted]");
    let s = sanitize_error("Authorization: Bearer abc123xyz failed");
    assert!(!s.contains("abc123xyz"));
    let items: Vec<u32> = (0..(max_event_batch_size() as u32 + 20)).collect();
    assert_eq!(bound_batch(items).len(), max_event_batch_size());
}

#[test]
fn search_results_are_bounded_by_default() {
    let dir = tempdir().unwrap();
    let caps = caps_only(dir.path());
    for i in 0..15 {
        caps.create_investigation(format!("s-{i}"), None, "t")
            .unwrap();
    }
    let results = caps
        .search_investigations(rivora::runtime::search::SearchQuery {
            text: Some("s-".into()),
            ..Default::default()
        })
        .unwrap();
    assert!(results.len() <= rivora::DEFAULT_LIST_LIMIT);
}
