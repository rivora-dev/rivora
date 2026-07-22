//! v0.2 Phase 1 — Investigation Graph (RFC-015).

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use chrono::Utc;
use rivora::domain::{
    Confidence, ConfirmationState, DerivationMetadata, Investigation, InvestigationId,
    InvestigationRelationship, ObjectId, Observation, ObservationKind, Provenance,
    RelationshipEvidence, RelationshipKind,
};
use rivora::runtime::observation::IngestObservationRequest;
use rivora::storage::LocalStore;
use rivora::{CapabilityService, RivoraError, Runtime};

fn runtime(dir: &Path) -> Runtime {
    Runtime::new(Arc::new(LocalStore::open(dir).unwrap()))
}

fn ingest(
    rt: &Runtime,
    investigation_id: InvestigationId,
    kind: ObservationKind,
    summary: &str,
    payload: serde_json::Value,
    source: &str,
) -> Observation {
    rt.ingest_observation(IngestObservationRequest {
        investigation_id,
        kind,
        summary: summary.into(),
        payload,
        source: source.into(),
        observed_at: Utc::now(),
        idempotency_key: None,
        actor: "tester".into(),
    })
    .unwrap()
    .observation
}

/// Two investigations sharing a repository and a failing check (RFC-015 signals).
struct SharedPair {
    a: Investigation,
    b: Investigation,
    repo_a: Observation,
    repo_b: Observation,
    check_a: Observation,
    check_b: Observation,
}

fn shared_pair(rt: &Runtime) -> SharedPair {
    let a = rt
        .create_investigation("Incident alpha", None, "tester")
        .unwrap();
    let b = rt
        .create_investigation("Incident beta", None, "tester")
        .unwrap();
    let repo_a = ingest(
        rt,
        a.id,
        ObservationKind::Repository,
        "Repository metadata for `acme/app`",
        serde_json::json!({"full_name": "acme/app"}),
        "github",
    );
    let repo_b = ingest(
        rt,
        b.id,
        ObservationKind::Repository,
        "Repository metadata for `acme/app`",
        serde_json::json!({"full_name": "acme/app"}),
        "github",
    );
    let check_a = ingest(
        rt,
        a.id,
        ObservationKind::CheckResult,
        "Check build failed",
        serde_json::json!({"name": "build", "conclusion": "failure"}),
        "github",
    );
    let check_b = ingest(
        rt,
        b.id,
        ObservationKind::CheckResult,
        "Check build failed",
        serde_json::json!({"name": "build", "conclusion": "failure"}),
        "github",
    );
    SharedPair {
        a,
        b,
        repo_a,
        repo_b,
        check_a,
        check_b,
    }
}

fn relationship_kinds(rels: &[InvestigationRelationship]) -> HashSet<RelationshipKind> {
    rels.iter().map(|r| r.kind).collect()
}

fn relationship_ids(rels: &[InvestigationRelationship]) -> HashSet<ObjectId> {
    rels.iter().map(|r| r.id).collect()
}

#[test]
fn relationship_creation_and_serialization_round_trip() {
    let a = InvestigationId::new();
    let b = InvestigationId::new();
    let derivation = || DerivationMetadata {
        method: "shared_repository_v1".into(),
        explanation: "Compares normalized repository names.".into(),
    };

    // Endpoints are stored in canonical order regardless of argument order.
    let derived = InvestigationRelationship::derived(
        RelationshipKind::SharedRepository,
        b,
        a,
        Confidence::new(0.9),
        vec![RelationshipEvidence::new(
            "Both investigations observed repository `acme/app`",
            vec![ObjectId::new(), ObjectId::new()],
        )],
        derivation(),
        Provenance::now("tester", "runtime").with_capability("refresh_relationships"),
        "shared_repository|acme/app",
    );
    assert!(
        derived.source_investigation_id.to_string() < derived.target_investigation_id.to_string()
    );
    assert_eq!(derived.confirmation.state, ConfirmationState::Unconfirmed);
    assert!(derived.kind.is_derived());

    // Re-deriving over the same inputs reproduces the same identifier.
    let rebuilt = InvestigationRelationship::derived(
        RelationshipKind::SharedRepository,
        a,
        b,
        Confidence::new(0.9),
        vec![],
        derivation(),
        Provenance::now("tester", "runtime"),
        "shared_repository|acme/app",
    );
    assert_eq!(derived.id, rebuilt.id);

    // Serde round-trip (derived).
    let json = serde_json::to_string_pretty(&derived).unwrap();
    assert!(json.contains("\"kind\": \"shared_repository\""));
    let back: InvestigationRelationship = serde_json::from_str(&json).unwrap();
    assert_eq!(derived, back);

    // Explicit links keep the user's direction and start confirmed.
    let explicit = InvestigationRelationship::explicit(
        a,
        b,
        Some("same outage".into()),
        "oncall",
        Provenance::now("oncall", "runtime"),
    );
    assert_eq!(explicit.kind, RelationshipKind::ExplicitLink);
    assert_eq!(explicit.source_investigation_id, a);
    assert_eq!(explicit.target_investigation_id, b);
    assert_eq!(explicit.confirmation.state, ConfirmationState::Confirmed);
    assert_eq!(explicit.derivation.method, "explicit_link_v1");

    // Serde round-trip (explicit).
    let json = serde_json::to_string_pretty(&explicit).unwrap();
    assert!(json.contains("\"kind\": \"explicit_link\""));
    let back: InvestigationRelationship = serde_json::from_str(&json).unwrap();
    assert_eq!(explicit, back);
}

#[test]
fn refresh_derives_relationships_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let pair = shared_pair(&rt);

    let rels = rt.refresh_relationships(pair.a.id, "tester").unwrap();
    let kinds = relationship_kinds(&rels);
    assert!(kinds.contains(&RelationshipKind::SharedRepository));
    assert!(kinds.contains(&RelationshipKind::RepeatedFailureSignature));
    assert!(kinds.contains(&RelationshipKind::SharedConnectorSource));

    for relationship in &rels {
        assert!(relationship.touches(pair.a.id));
        assert!(relationship.touches(pair.b.id));
        assert!(!relationship.evidence.is_empty());
        assert!(!relationship.derivation.method.is_empty());
        assert!(!relationship.derivation.explanation.is_empty());
        assert_eq!(
            relationship.confirmation.state,
            ConfirmationState::Unconfirmed
        );
        assert_eq!(
            relationship.provenance.capability.as_deref(),
            Some("refresh_relationships")
        );
        if relationship.kind.is_derived() {
            assert!(
                relationship.source_investigation_id.to_string()
                    < relationship.target_investigation_id.to_string(),
                "derived endpoints must be canonically ordered"
            );
        }
    }

    // Refreshing from the other side yields the same relationships.
    let from_b = rt.refresh_relationships(pair.b.id, "tester").unwrap();
    assert_eq!(relationship_ids(&rels), relationship_ids(&from_b));
}

#[test]
fn relationship_evidence_identifies_both_sides() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let pair = shared_pair(&rt);
    let rels = rt.refresh_relationships(pair.a.id, "tester").unwrap();

    let repo_rel = rels
        .iter()
        .find(|r| r.kind == RelationshipKind::SharedRepository)
        .unwrap();
    assert_eq!(repo_rel.evidence.len(), 1);
    let evidence = &repo_rel.evidence[0];
    assert!(evidence.description.contains("acme/app"));
    assert_eq!(evidence.object_ids.len(), 2);
    assert!(evidence.object_ids.contains(&pair.repo_a.id));
    assert!(evidence.object_ids.contains(&pair.repo_b.id));
    assert!((repo_rel.confidence.value() - 0.9).abs() < f64::EPSILON);

    let failure_rel = rels
        .iter()
        .find(|r| r.kind == RelationshipKind::RepeatedFailureSignature)
        .unwrap();
    assert!(failure_rel
        .evidence
        .iter()
        .any(|e| e.description.contains("check_result:build")));
    let evidence_ids: Vec<ObjectId> = failure_rel
        .evidence
        .iter()
        .flat_map(|e| e.object_ids.iter().copied())
        .collect();
    assert!(evidence_ids.contains(&pair.check_a.id));
    assert!(evidence_ids.contains(&pair.check_b.id));
}

#[test]
fn refresh_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let pair = shared_pair(&rt);

    let first = rt.refresh_relationships(pair.a.id, "tester").unwrap();
    assert!(!first.is_empty());
    let second = rt.refresh_relationships(pair.a.id, "tester").unwrap();
    assert_eq!(first, second, "re-refresh must reproduce identical records");

    // Cross-side refresh then re-refresh still reproduces the same set.
    rt.refresh_relationships(pair.b.id, "tester").unwrap();
    let third = rt.refresh_relationships(pair.a.id, "tester").unwrap();
    assert_eq!(first, third);
}

#[test]
fn explicit_link_create_list_and_unlink() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let a = rt
        .create_investigation("Link alpha", None, "tester")
        .unwrap();
    let b = rt
        .create_investigation("Link beta", None, "tester")
        .unwrap();

    // Self-link and missing investigations are rejected.
    let err = rt
        .link_investigations(a.id, a.id, None, "tester")
        .unwrap_err();
    assert!(matches!(err, RivoraError::Validation(_)));
    let err = rt
        .link_investigations(a.id, InvestigationId::new(), None, "tester")
        .unwrap_err();
    assert!(matches!(err, RivoraError::InvestigationNotFound(_)));

    let link = rt
        .link_investigations(a.id, b.id, Some("same outage".into()), "oncall")
        .unwrap();
    assert_eq!(link.kind, RelationshipKind::ExplicitLink);
    assert_eq!(link.source_investigation_id, a.id);
    assert_eq!(link.target_investigation_id, b.id);
    assert_eq!(link.confirmation.state, ConfirmationState::Confirmed);
    assert_eq!(link.confirmation.actor.as_deref(), Some("oncall"));
    assert_eq!(
        link.provenance.capability.as_deref(),
        Some("link_investigations")
    );

    // Linking the same pair again (either direction) is idempotent.
    let again = rt
        .link_investigations(b.id, a.id, None, "someone-else")
        .unwrap();
    assert_eq!(again.id, link.id);

    // Visible from both sides.
    assert!(rt
        .list_relationships(a.id)
        .unwrap()
        .iter()
        .any(|r| r.id == link.id));
    assert!(rt
        .list_relationships(b.id)
        .unwrap()
        .iter()
        .any(|r| r.id == link.id));

    // Related investigations load the opposite side.
    let related = rt.list_related_investigations(a.id).unwrap();
    assert_eq!(related.len(), 1);
    assert_eq!(related[0].related.id, b.id);
    assert_eq!(related[0].relationship.id, link.id);

    // Explicit links survive refresh untouched.
    rt.refresh_relationships(a.id, "tester").unwrap();
    assert!(rt
        .list_relationships(a.id)
        .unwrap()
        .iter()
        .any(|r| r.id == link.id));

    // Unlink removes it.
    rt.unlink_investigation(link.id, "oncall").unwrap();
    assert!(rt.list_relationships(a.id).unwrap().is_empty());
    let err = rt.explain_relationship(link.id).unwrap_err();
    assert!(matches!(err, RivoraError::ObjectNotFound(_)));
}

#[test]
fn unlink_derived_relationship_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let pair = shared_pair(&rt);
    let rels = rt.refresh_relationships(pair.a.id, "tester").unwrap();
    let derived = rels.iter().find(|r| r.kind.is_derived()).unwrap();

    let err = rt.unlink_investigation(derived.id, "tester").unwrap_err();
    assert!(matches!(err, RivoraError::Precondition(_)));
    // The derived relationship remains in place.
    assert!(rt
        .list_relationships(pair.a.id)
        .unwrap()
        .iter()
        .any(|r| r.id == derived.id));
}

#[test]
fn explain_relationship_answers_why() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let pair = shared_pair(&rt);
    let rels = rt.refresh_relationships(pair.a.id, "tester").unwrap();
    let repo_rel = rels
        .iter()
        .find(|r| r.kind == RelationshipKind::SharedRepository)
        .unwrap();

    let explanation = rt.explain_relationship(repo_rel.id).unwrap();
    assert_eq!(explanation.relationship.id, repo_rel.id);
    assert!(explanation.explanation.contains("shared_repository"));
    assert!(explanation.explanation.contains("acme/app"));
    assert!(explanation.explanation.contains("shared_repository_v1"));
    assert!(explanation.explanation.contains("unconfirmed"));

    let err = rt.explain_relationship(ObjectId::new()).unwrap_err();
    assert!(matches!(err, RivoraError::ObjectNotFound(_)));
}

#[test]
fn relationships_persist_across_store_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let pair = shared_pair(&rt);
    rt.refresh_relationships(pair.a.id, "tester").unwrap();
    rt.link_investigations(pair.a.id, pair.b.id, Some("tracked".into()), "oncall")
        .unwrap();
    let before = rt.list_relationships(pair.a.id).unwrap();
    assert!(!before.is_empty());

    let reopened = runtime(dir.path());
    let after = reopened.list_relationships(pair.a.id).unwrap();
    assert_eq!(before, after);

    // Individual loads survive as well.
    for relationship in &after {
        let explained = reopened.explain_relationship(relationship.id).unwrap();
        assert_eq!(explained.relationship, *relationship);
    }
}

fn investigation_snapshot(dir: &Path, id: InvestigationId) -> (Vec<u8>, Vec<Vec<u8>>) {
    let base = dir.join("investigations").join(id.to_string());
    let investigation = std::fs::read(base.join("investigation.json")).unwrap();
    let mut memory_paths: Vec<_> = std::fs::read_dir(base.join("memory"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    memory_paths.sort();
    let memory = memory_paths
        .into_iter()
        .map(|path| std::fs::read(path).unwrap())
        .collect();
    (investigation, memory)
}

#[test]
fn graph_operations_never_modify_investigations() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let pair = shared_pair(&rt);
    let before_a = investigation_snapshot(dir.path(), pair.a.id);
    let before_b = investigation_snapshot(dir.path(), pair.b.id);
    let memory_a: Vec<ObjectId> = rt
        .recall_memory(pair.a.id)
        .unwrap()
        .into_iter()
        .map(|m| m.id)
        .collect();

    rt.link_investigations(pair.a.id, pair.b.id, Some("same outage".into()), "oncall")
        .unwrap();
    rt.refresh_relationships(pair.a.id, "tester").unwrap();
    rt.refresh_relationships(pair.b.id, "tester").unwrap();

    assert_eq!(before_a, investigation_snapshot(dir.path(), pair.a.id));
    assert_eq!(before_b, investigation_snapshot(dir.path(), pair.b.id));
    let memory_after: Vec<ObjectId> = rt
        .recall_memory(pair.a.id)
        .unwrap()
        .into_iter()
        .map(|m| m.id)
        .collect();
    assert_eq!(memory_a, memory_after);
}

#[test]
fn graph_rebuilds_reproducibly_with_fresh_runtime() {
    let dir = tempfile::tempdir().unwrap();
    let rt1 = runtime(dir.path());
    let pair = shared_pair(&rt1);
    let before_ab = relationship_ids(&rt1.refresh_relationships(pair.a.id, "tester").unwrap());

    // A third investigation shares the same artifacts.
    let c = rt1
        .create_investigation("Incident gamma", None, "tester")
        .unwrap();
    ingest(
        &rt1,
        c.id,
        ObservationKind::Repository,
        "Repository metadata for `acme/app`",
        serde_json::json!({"full_name": "acme/app"}),
        "github",
    );
    ingest(
        &rt1,
        c.id,
        ObservationKind::CheckResult,
        "Check build failed",
        serde_json::json!({"name": "build", "conclusion": "failure"}),
        "github",
    );

    // A fresh Runtime over the same directory derives the same graph.
    let rt2 = runtime(dir.path());
    let first = rt2.refresh_relationships(c.id, "tester").unwrap();
    let second = rt2.refresh_relationships(c.id, "tester").unwrap();
    assert_eq!(relationship_ids(&first), relationship_ids(&second));
    assert!(relationship_kinds(&first).contains(&RelationshipKind::SharedRepository));
    let counterparties: HashSet<InvestigationId> =
        first.iter().map(|r| r.other_end(c.id).unwrap()).collect();
    assert!(counterparties.contains(&pair.a.id));
    assert!(counterparties.contains(&pair.b.id));

    // Yet another fresh Runtime reproduces the same derived identifiers.
    let rt3 = runtime(dir.path());
    let third = rt3.refresh_relationships(c.id, "tester").unwrap();
    assert_eq!(relationship_ids(&first), relationship_ids(&third));

    // Pre-existing derived relationships are untouched by the rebuild.
    let a_rels = rt3.refresh_relationships(pair.a.id, "tester").unwrap();
    assert!(relationship_ids(&a_rels).is_superset(&before_ab));
}

#[test]
fn confirmation_survives_refresh() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let pair = shared_pair(&rt);
    let rels = rt.refresh_relationships(pair.a.id, "tester").unwrap();
    let target = rels
        .iter()
        .find(|r| r.kind == RelationshipKind::SharedRepository)
        .unwrap();

    let confirmed = rt.confirm_relationship(target.id, "reviewer").unwrap();
    assert_eq!(confirmed.confirmation.state, ConfirmationState::Confirmed);
    assert_eq!(confirmed.confirmation.actor.as_deref(), Some("reviewer"));
    assert!(confirmed.confirmation.at.is_some());

    let refreshed = rt.refresh_relationships(pair.a.id, "tester").unwrap();
    let after = refreshed.iter().find(|r| r.id == target.id).unwrap();
    assert_eq!(after.confirmation.state, ConfirmationState::Confirmed);
    assert_eq!(after.confirmation.actor.as_deref(), Some("reviewer"));
    // Origin is preserved; content is still re-derived.
    assert_eq!(after.provenance, confirmed.provenance);
    assert_eq!(after.created_at, confirmed.created_at);
    assert!(!after.evidence.is_empty());
}

#[test]
fn dismissed_relationships_are_hidden_from_related_list() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let pair = shared_pair(&rt);
    let rels = rt.refresh_relationships(pair.a.id, "tester").unwrap();
    let target = rels.iter().find(|r| r.kind.is_derived()).unwrap().id;

    // Visible before dismissal.
    assert!(rt
        .list_related_investigations(pair.a.id)
        .unwrap()
        .iter()
        .any(|r| r.relationship.id == target));

    let dismissed = rt.dismiss_relationship(target, "reviewer").unwrap();
    assert_eq!(dismissed.confirmation.state, ConfirmationState::Dismissed);
    assert_eq!(dismissed.confirmation.actor.as_deref(), Some("reviewer"));

    // Hidden from related investigations, still listed as a relationship.
    assert!(!rt
        .list_related_investigations(pair.a.id)
        .unwrap()
        .iter()
        .any(|r| r.relationship.id == target));
    let all = rt.list_relationships(pair.a.id).unwrap();
    let stored = all.iter().find(|r| r.id == target).unwrap();
    assert_eq!(stored.confirmation.state, ConfirmationState::Dismissed);
    // The other investigation remains reachable via remaining relationships.
    assert!(rt
        .list_related_investigations(pair.a.id)
        .unwrap()
        .iter()
        .any(|r| r.related.id == pair.b.id));
}

#[test]
fn capabilities_delegate_graph_operations_to_runtime() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(LocalStore::open(dir.path()).unwrap());
    let rt = Arc::new(Runtime::new(store));
    let caps = CapabilityService::new(rt);

    let a = caps
        .create_investigation("Cap alpha", None, "tester")
        .unwrap();
    let b = caps
        .create_investigation("Cap beta", None, "tester")
        .unwrap();
    for id in [a.id, b.id] {
        caps.ingest_observation(
            id,
            ObservationKind::Repository,
            "Repository metadata for `acme/app`",
            serde_json::json!({"full_name": "acme/app"}),
            "github",
            Utc::now(),
            None,
            "tester",
        )
        .unwrap();
    }

    let rels = caps.refresh_relationships(a.id, "tester").unwrap();
    assert!(relationship_kinds(&rels).contains(&RelationshipKind::SharedRepository));

    let link = caps
        .link_investigations(a.id, b.id, Some("cap link".into()), "oncall")
        .unwrap();
    assert!(caps
        .list_relationships(a.id)
        .unwrap()
        .iter()
        .any(|r| r.id == link.id));
    assert!(caps
        .list_related_investigations(b.id)
        .unwrap()
        .iter()
        .any(|r| r.related.id == a.id));

    let explanation = caps.explain_relationship(rels[0].id).unwrap();
    assert!(!explanation.explanation.is_empty());

    let confirmed = caps.confirm_relationship(rels[0].id, "reviewer").unwrap();
    assert_eq!(confirmed.confirmation.state, ConfirmationState::Confirmed);
    let dismissed = caps.dismiss_relationship(rels[0].id, "reviewer").unwrap();
    assert_eq!(dismissed.confirmation.state, ConfirmationState::Dismissed);

    caps.unlink_investigation(link.id, "oncall").unwrap();
    assert!(caps
        .list_relationships(a.id)
        .unwrap()
        .iter()
        .all(|r| r.id != link.id));
}
