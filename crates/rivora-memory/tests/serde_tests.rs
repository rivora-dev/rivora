use rivora_memory::fixtures;
use rivora_memory::{
    MemoryIndex, MemoryKind, MemoryRecord, MemoryRetentionPolicy, MemoryScope, MemorySnapshot,
    MemoryStatus,
};

#[test]
fn record_round_trips_through_json() {
    let record = fixtures::organization_fact();
    let json = serde_json::to_string(&record).unwrap();
    let back: MemoryRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(back, record);
}

#[test]
fn snapshot_round_trips_through_json() {
    let index = fixtures::sample_index();
    let snapshot = index.snapshot();
    let json = serde_json::to_string(&snapshot).unwrap();
    let back: MemorySnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(back, snapshot);
}

#[test]
fn all_fixture_records_round_trip() {
    let fixtures: Vec<(&str, MemoryRecord)> = vec![
        ("organization_fact", fixtures::organization_fact()),
        (
            "service_relationship_memory",
            fixtures::service_relationship_memory(),
        ),
        (
            "incident_learning_memory",
            fixtures::incident_learning_memory(),
        ),
        (
            "deployment_learning_memory",
            fixtures::deployment_learning_memory(),
        ),
        (
            "receipt_learning_memory",
            fixtures::receipt_learning_memory(),
        ),
        (
            "ability_learning_memory",
            fixtures::ability_learning_memory(),
        ),
        ("expired_memory", fixtures::expired_memory()),
        ("superseded_memory", fixtures::superseded_memory()),
        ("invalid_memory", fixtures::invalid_memory()),
    ];
    for (name, record) in &fixtures {
        let json = serde_json::to_string(record).unwrap();
        let back: MemoryRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back, *record, "round-trip mismatch for {name}");
    }
}

#[test]
fn memory_kind_serializes_as_snake_case() {
    let cases = [
        (MemoryKind::Fact, "\"fact\""),
        (MemoryKind::Pattern, "\"pattern\""),
        (MemoryKind::Preference, "\"preference\""),
        (MemoryKind::Convention, "\"convention\""),
        (MemoryKind::IncidentLearning, "\"incident_learning\""),
        (MemoryKind::DeploymentLearning, "\"deployment_learning\""),
        (MemoryKind::ServiceRelationship, "\"service_relationship\""),
        (MemoryKind::OperationalNote, "\"operational_note\""),
        (MemoryKind::RunbookKnowledge, "\"runbook_knowledge\""),
        (MemoryKind::TeamKnowledge, "\"team_knowledge\""),
        (MemoryKind::RiskKnowledge, "\"risk_knowledge\""),
        (MemoryKind::ReceiptLearning, "\"receipt_learning\""),
        (MemoryKind::AbilityLearning, "\"ability_learning\""),
        (MemoryKind::Unknown, "\"unknown\""),
    ];
    for (kind, expected) in cases {
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(
            json, expected,
            "MemoryKind::{kind:?} serialized unexpectedly"
        );
    }
}

#[test]
fn memory_status_serializes_as_snake_case() {
    let cases = [
        (MemoryStatus::Draft, "\"draft\""),
        (MemoryStatus::Active, "\"active\""),
        (MemoryStatus::Superseded, "\"superseded\""),
        (MemoryStatus::Expired, "\"expired\""),
        (MemoryStatus::Archived, "\"archived\""),
        (MemoryStatus::Invalid, "\"invalid\""),
    ];
    for (status, expected) in cases {
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(
            json, expected,
            "MemoryStatus::{status:?} serialized unexpectedly"
        );
    }
}

#[test]
fn memory_scope_serializes_as_snake_case() {
    let cases = [
        (MemoryScope::Organization, "\"organization\""),
        (MemoryScope::Team, "\"team\""),
        (MemoryScope::Service, "\"service\""),
        (MemoryScope::Environment, "\"environment\""),
        (MemoryScope::Repository, "\"repository\""),
        (MemoryScope::Incident, "\"incident\""),
        (MemoryScope::Deployment, "\"deployment\""),
        (MemoryScope::Ability, "\"ability\""),
        (MemoryScope::Global, "\"global\""),
        (MemoryScope::Unknown, "\"unknown\""),
    ];
    for (scope, expected) in cases {
        let json = serde_json::to_string(&scope).unwrap();
        assert_eq!(
            json, expected,
            "MemoryScope::{scope:?} serialized unexpectedly"
        );
    }
}

#[test]
fn memory_retention_policy_serializes_as_snake_case() {
    let cases = [
        (MemoryRetentionPolicy::Permanent, "\"permanent\""),
        (
            MemoryRetentionPolicy::UntilSuperseded,
            "\"until_superseded\"",
        ),
        (MemoryRetentionPolicy::TimeBound, "\"time_bound\""),
        (MemoryRetentionPolicy::ReviewRequired, "\"review_required\""),
        (MemoryRetentionPolicy::Ephemeral, "\"ephemeral\""),
        (MemoryRetentionPolicy::Unknown, "\"unknown\""),
    ];
    for (policy, expected) in cases {
        let json = serde_json::to_string(&policy).unwrap();
        assert_eq!(
            json, expected,
            "MemoryRetentionPolicy::{policy:?} serialized unexpectedly"
        );
    }
}

#[test]
fn snapshot_ordering_is_deterministic() {
    let index = fixtures::sample_index();
    let snapshot = index.snapshot();
    let ids: Vec<&str> = snapshot.records.iter().map(|r| r.id.as_str()).collect();
    let mut sorted_ids = ids.clone();
    sorted_ids.sort();
    assert_eq!(ids, sorted_ids);
}

#[test]
fn invalid_memory_still_serializes() {
    let record = fixtures::invalid_memory();
    let json = serde_json::to_string(&record).unwrap();
    let back: MemoryRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(back, record);
    assert_eq!(back.id.as_str(), "mem-fixture-invalid");
}

#[test]
fn index_round_trips_through_json() {
    let index = fixtures::sample_index();
    let json = serde_json::to_string(&index).unwrap();
    let back: MemoryIndex = serde_json::from_str(&json).unwrap();
    assert_eq!(back, index);
}
