//! v0.5 Phase 1 — Implementation Record model, storage, and lifecycle (RFC-022).

use std::fs;
use std::sync::Arc;

use chrono::Utc;
use rivora::domain::{
    ImplementationReference, ImplementationSource, ImplementationStatus, ImprovementProposal,
    ProposalCategory, ProposalGenerationMethod, ProposalPriority, Provenance,
};
use rivora::runtime::outcome::{RecordImplementationRequest, ReviseImplementationRequest};
use rivora::storage::{LocalStore, Store};
use rivora::{CapabilityService, Confidence, ObjectId, Runtime};

struct Fixture {
    _dir: tempfile::TempDir,
    caps: CapabilityService,
    inv_id: rivora::InvestigationId,
    proposal_id: ObjectId,
}

fn setup() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(LocalStore::open(dir.path()).unwrap());
    let runtime = Arc::new(Runtime::new(store));
    let caps = CapabilityService::new(runtime);
    let inv = caps
        .create_investigation("v0.5 impl", None, "tester")
        .unwrap();
    let mut proposal = ImprovementProposal::generated(
        inv.id,
        "Add config guard",
        "Reject malformed config before use",
        "Malformed configuration reaches the Runtime",
        ProposalCategory::Configuration,
        ProposalPriority::High,
        Confidence::new(0.85),
        ProposalGenerationMethod::Human,
        Provenance::now("tester", "test"),
    )
    .unwrap();
    proposal.success_criteria = vec!["Malformed config is rejected".into()];
    caps.runtime().store().append_proposal(&proposal).unwrap();
    Fixture {
        _dir: dir,
        caps,
        inv_id: inv.id,
        proposal_id: proposal.id,
    }
}

#[test]
fn create_implementation_record_with_typed_references() {
    let fx = setup();
    let record = fx
        .caps
        .record_external_implementation(
            fx.inv_id,
            fx.proposal_id,
            RecordImplementationRequest {
                source: ImplementationSource::PullRequest,
                summary: "Merged PR that adds config validation".into(),
                references: vec![
                    ImplementationReference::PullRequest {
                        reference: "https://example.com/pr/12".into(),
                    },
                    ImplementationReference::CommitSha {
                        sha: "deadbeef".into(),
                    },
                ],
                implemented_at: Some(Utc::now()),
                observed_files: vec!["src/config.rs".into()],
                observed_components: vec!["config".into()],
                declared_scope: "config validation only".into(),
            },
            "engineer",
        )
        .unwrap();

    assert_eq!(record.status, ImplementationStatus::Reported);
    assert_eq!(record.revision_number, 1);
    assert_eq!(record.lineage_id, record.id);
    assert_eq!(record.proposal_id, fx.proposal_id);
    assert_eq!(record.references.len(), 2);
    assert_eq!(record.actor, "engineer");

    let loaded = fx
        .caps
        .get_implementation_record(fx.inv_id, record.id)
        .unwrap();
    assert_eq!(loaded, record);

    let listing = fx.caps.list_implementation_records(fx.inv_id).unwrap();
    assert_eq!(listing.records.len(), 1);
    assert!(listing.diagnostics.is_empty());
}

#[test]
fn revise_creates_immutable_successor() {
    let fx = setup();
    let original = fx
        .caps
        .record_external_implementation(
            fx.inv_id,
            fx.proposal_id,
            RecordImplementationRequest {
                source: ImplementationSource::HumanDeclared,
                summary: "Deployed manually".into(),
                references: vec![],
                implemented_at: None,
                observed_files: vec![],
                observed_components: vec![],
                declared_scope: String::new(),
            },
            "engineer",
        )
        .unwrap();

    let revised = fx
        .caps
        .revise_implementation_record(
            fx.inv_id,
            original.id,
            ReviseImplementationRequest {
                summary: Some("Deployed manually with runbook".into()),
                declared_scope: Some("production only".into()),
                ..Default::default()
            },
            "engineer",
            "clarify scope",
        )
        .unwrap();

    assert_ne!(revised.id, original.id);
    assert_eq!(revised.lineage_id, original.lineage_id);
    assert_eq!(revised.parent_record_id, Some(original.id));
    assert_eq!(revised.revision_number, 2);
    assert_eq!(revised.summary, "Deployed manually with runbook");

    let prior = fx
        .caps
        .get_implementation_record(fx.inv_id, original.id)
        .unwrap();
    assert_eq!(prior.summary, "Deployed manually");
    assert_eq!(prior.revision_number, 1);

    let revisions = fx
        .caps
        .list_implementation_revisions(fx.inv_id, original.lineage_id)
        .unwrap();
    assert_eq!(revisions.records.len(), 2);
}

#[test]
fn lifecycle_evidence_ready_withdraw() {
    let fx = setup();
    let record = fx
        .caps
        .record_external_implementation(
            fx.inv_id,
            fx.proposal_id,
            RecordImplementationRequest {
                source: ImplementationSource::GitCommit,
                summary: "shipped".into(),
                references: vec![ImplementationReference::CommitSha { sha: "abc".into() }],
                implemented_at: None,
                observed_files: vec![],
                observed_components: vec![],
                declared_scope: String::new(),
            },
            "engineer",
        )
        .unwrap();

    let evidence_id = ObjectId::new();
    let linked = fx
        .caps
        .link_implementation_evidence(
            fx.inv_id,
            record.id,
            vec![evidence_id],
            "engineer",
            "linked observation",
        )
        .unwrap();
    assert_eq!(linked.status, ImplementationStatus::EvidenceLinked);
    assert!(linked.evidence_ids.contains(&evidence_id));

    let ready = fx
        .caps
        .mark_implementation_ready(fx.inv_id, linked.id, "engineer", "ready for outcome")
        .unwrap();
    assert_eq!(ready.status, ImplementationStatus::ReadyForEvaluation);

    let withdrawn = fx
        .caps
        .withdraw_implementation(fx.inv_id, ready.id, "engineer", "false report")
        .unwrap();
    assert_eq!(withdrawn.status, ImplementationStatus::Withdrawn);

    let err = fx
        .caps
        .revise_implementation_record(
            fx.inv_id,
            withdrawn.id,
            ReviseImplementationRequest {
                summary: Some("nope".into()),
                ..Default::default()
            },
            "engineer",
            "try",
        )
        .unwrap_err();
    assert!(err.to_string().contains("cannot revise") || err.to_string().contains("validation"));
}

#[test]
fn invalid_transition_is_rejected() {
    let fx = setup();
    let record = fx
        .caps
        .record_external_implementation(
            fx.inv_id,
            fx.proposal_id,
            RecordImplementationRequest {
                source: ImplementationSource::Other,
                summary: "done".into(),
                references: vec![],
                implemented_at: None,
                observed_files: vec![],
                observed_components: vec![],
                declared_scope: String::new(),
            },
            "engineer",
        )
        .unwrap();

    let err = fx
        .caps
        .withdraw_implementation(fx.inv_id, record.id, "", "reason")
        .unwrap_err();
    assert!(err.to_string().contains("validation") || err.to_string().contains("required"));
}

#[test]
fn serialization_round_trip_and_corruption_isolation() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::open(dir.path()).unwrap();
    let inv = rivora::Investigation::create("iso", None, Provenance::now("t", "t")).unwrap();
    store.save_investigation(&inv).unwrap();

    let proposal_id = ObjectId::new();
    let record = rivora::ImplementationRecord::reported(
        inv.id,
        proposal_id,
        proposal_id,
        1,
        "engineer",
        ImplementationSource::Deployment,
        "deployed",
        Provenance::now("engineer", "test"),
    )
    .unwrap();
    store.append_implementation_record(&record).unwrap();

    let json = serde_json::to_string(&record).unwrap();
    let decoded: rivora::ImplementationRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.id, record.id);

    let corrupt_path = dir
        .path()
        .join("investigations")
        .join(inv.id.to_string())
        .join("implementations")
        .join("corrupt.json");
    fs::write(&corrupt_path, "{not json").unwrap();

    let listing = store.list_implementation_records(&inv.id).unwrap();
    assert_eq!(listing.records.len(), 1);
    assert_eq!(listing.diagnostics.len(), 1);
    assert!(listing.diagnostics[0].path.contains("corrupt"));
}

#[test]
fn multiple_implementations_per_proposal_allowed() {
    let fx = setup();
    let a = fx
        .caps
        .record_external_implementation(
            fx.inv_id,
            fx.proposal_id,
            RecordImplementationRequest {
                source: ImplementationSource::GitCommit,
                summary: "first attempt".into(),
                references: vec![],
                implemented_at: None,
                observed_files: vec![],
                observed_components: vec![],
                declared_scope: String::new(),
            },
            "engineer",
        )
        .unwrap();
    let b = fx
        .caps
        .record_external_implementation(
            fx.inv_id,
            fx.proposal_id,
            RecordImplementationRequest {
                source: ImplementationSource::PullRequest,
                summary: "second attempt".into(),
                references: vec![],
                implemented_at: None,
                observed_files: vec![],
                observed_components: vec![],
                declared_scope: String::new(),
            },
            "engineer",
        )
        .unwrap();
    assert_ne!(a.lineage_id, b.lineage_id);
    assert_eq!(a.proposal_id, b.proposal_id);
    let listing = fx.caps.list_implementation_records(fx.inv_id).unwrap();
    assert_eq!(listing.records.len(), 2);
}
