//! Phase 1 completion gate tests.

use std::sync::Arc;

use rivora::domain::{InvestigationStatus, Provenance};
use rivora::storage::{LocalStore, Store};
use rivora::{Investigation, Runtime};

fn runtime(dir: &std::path::Path) -> Runtime {
    let store = Arc::new(LocalStore::open(dir).unwrap());
    Runtime::new(store)
}

#[test]
fn create_persist_load_advance_complete_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());

    let inv = rt
        .create_investigation("Phase1 Investigation", Some("gate".into()), "tester")
        .unwrap();
    assert_eq!(inv.status, InvestigationStatus::Created);

    let loaded = rt.open_investigation(inv.id).unwrap();
    assert_eq!(loaded.id, inv.id);
    assert_eq!(loaded.title, "Phase1 Investigation");

    let mut current = inv;
    for expected in [
        InvestigationStatus::Collecting,
        InvestigationStatus::Understanding,
        InvestigationStatus::Evaluating,
        InvestigationStatus::Verifying,
        InvestigationStatus::Recommending,
        InvestigationStatus::Learning,
    ] {
        current = rt.advance_investigation(current.id, None).unwrap();
        assert_eq!(current.status, expected);
    }

    current = rt
        .complete_investigation(current.id, Some("done".into()))
        .unwrap();
    assert_eq!(current.status, InvestigationStatus::Completed);
    assert!(!current.transitions.is_empty());

    // History preserved after reopen
    let transition_count = current.transitions.len();
    current = rt.reopen_investigation(current.id, None).unwrap();
    assert_eq!(current.status, InvestigationStatus::Collecting);
    assert!(current.transitions.len() > transition_count);

    let reloaded = rt.open_investigation(current.id).unwrap();
    assert_eq!(reloaded.status, InvestigationStatus::Collecting);
    assert_eq!(reloaded.transitions.len(), current.transitions.len());
}

#[test]
fn invalid_lifecycle_transitions_fail_safely() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let inv = rt.create_investigation("x", None, "t").unwrap();

    let err = rt
        .transition_investigation(inv.id, InvestigationStatus::Evaluating, None)
        .unwrap_err();
    assert!(matches!(
        err,
        rivora::RivoraError::InvalidLifecycleTransition { .. }
    ));

    // Original state unchanged
    let loaded = rt.open_investigation(inv.id).unwrap();
    assert_eq!(loaded.status, InvestigationStatus::Created);
}

#[test]
fn engineering_objects_serialize() {
    let inv = Investigation::create("s", None, Provenance::now("a", "s")).unwrap();
    let json = serde_json::to_string_pretty(&inv).unwrap();
    let back: Investigation = serde_json::from_str(&json).unwrap();
    assert_eq!(inv.id, back.id);
    assert_eq!(inv.status, back.status);
}

#[test]
fn memory_append_only_foundation() {
    use chrono::Utc;
    use rivora::domain::{MemoryRecord, ObjectId, Observation, ObservationKind};

    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::open(dir.path()).unwrap();
    let inv = Investigation::create("m", None, Provenance::now("a", "s")).unwrap();
    store.save_investigation(&inv).unwrap();

    let obs = Observation::new(
        inv.id,
        ObservationKind::Event,
        "event",
        serde_json::json!({"n": 1}),
        "test",
        Utc::now(),
        Some("key-1".into()),
        Provenance::now("a", "s"),
    )
    .unwrap();
    store.append_observation(&obs).unwrap();

    let mem = MemoryRecord::from_observation(
        obs.id,
        inv.id,
        "event",
        Utc::now(),
        Provenance::now("a", "s"),
    );
    store.append_memory(&mem).unwrap();

    // Second append of same record fails
    assert!(store.append_memory(&mem).is_err());

    // Correction is a new record
    let correction = MemoryRecord::correction(
        ObjectId::new(),
        inv.id,
        "corrected event",
        mem.id,
        Utc::now(),
        Provenance::now("a", "s"),
    );
    store.append_memory(&correction).unwrap();
    let all = store.list_memory(&inv.id).unwrap();
    assert_eq!(all.len(), 2);
    assert!(all.iter().any(|m| m.corrects == Some(mem.id)));
}
