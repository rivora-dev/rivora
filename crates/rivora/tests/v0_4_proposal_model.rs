//! v0.4 Phase 1 — Improvement Proposal model and storage (RFC-020).

use std::fs;

use chrono::{TimeZone, Utc};
use rivora::domain::{
    Confidence, EvidenceReference, EvidenceScope, ImprovementProposal, ProposalCategory,
    ProposalGenerationMethod, ProposalPriority, ProposalStatus, ProposalTransitionAuthority,
    Provenance,
};
use rivora::storage::{LocalStore, Store};
use rivora::{InvestigationId, ObjectId, RivoraError};

fn generated_proposal(investigation_id: InvestigationId) -> ImprovementProposal {
    let mut proposal = ImprovementProposal::generated(
        investigation_id,
        "Validate deployment configuration",
        "Reject malformed deployment configuration before use.",
        "Current verification evidence shows malformed configuration reaches the Runtime.",
        ProposalCategory::Configuration,
        ProposalPriority::High,
        Confidence::new(0.85),
        ProposalGenerationMethod::Deterministic,
        Provenance::now("runtime", "proposal_generator"),
    )
    .unwrap();
    proposal.supporting_evidence = vec![EvidenceReference {
        object_id: ObjectId::new(),
        scope: EvidenceScope::Current,
    }];
    proposal.contradicting_evidence = vec![EvidenceReference {
        object_id: ObjectId::new(),
        scope: EvidenceScope::Historical,
    }];
    proposal
}

#[test]
fn generated_proposal_is_a_distinct_draft_with_scoped_evidence() {
    let investigation_id = InvestigationId::new();
    let proposal = generated_proposal(investigation_id);

    assert_eq!(proposal.investigation_id, investigation_id);
    assert_eq!(proposal.status, ProposalStatus::Draft);
    assert_eq!(proposal.lineage_id, proposal.id);
    assert_eq!(proposal.revision_number, 1);
    assert_eq!(proposal.parent_proposal_id, None);
    assert_eq!(
        proposal.supporting_evidence[0].scope,
        EvidenceScope::Current
    );
    assert_eq!(
        proposal.contradicting_evidence[0].scope,
        EvidenceScope::Historical
    );

    let json = serde_json::to_string(&proposal).unwrap();
    let decoded: ImprovementProposal = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, proposal);
}

#[test]
fn lifecycle_transitions_create_immutable_successor_snapshots() {
    let original = generated_proposal(InvestigationId::new());
    let at = Utc.with_ymd_and_hms(2026, 7, 22, 12, 0, 0).unwrap();

    let proposed = original
        .transitioned(
            ProposalStatus::Proposed,
            "reviewer",
            "ready for review",
            at,
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap();

    assert_eq!(original.status, ProposalStatus::Draft);
    assert_eq!(original.revision_number, 1);
    assert_eq!(proposed.status, ProposalStatus::Proposed);
    assert_ne!(proposed.id, original.id);
    assert_eq!(proposed.lineage_id, original.lineage_id);
    assert_eq!(proposed.parent_proposal_id, Some(original.id));
    assert_eq!(proposed.revision_number, 2);
    assert_eq!(proposed.transitions.len(), 1);
    assert_eq!(proposed.transitions[0].actor, "reviewer");
    assert_eq!(proposed.transitions[0].reason, "ready for review");
    assert_eq!(proposed.transitions[0].at, at);
}

#[test]
fn acceptance_requires_an_explicit_external_caller_and_never_means_implemented() {
    let draft = generated_proposal(InvestigationId::new());
    let at = Utc::now();
    let proposed = draft
        .transitioned(
            ProposalStatus::Proposed,
            "reviewer",
            "review candidate",
            at,
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap()
        .transitioned(
            ProposalStatus::UnderReview,
            "reviewer",
            "begin review",
            at,
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap();

    let err = proposed
        .transitioned(
            ProposalStatus::Accepted,
            "runtime",
            "ranked first",
            at,
            ProposalTransitionAuthority::Runtime,
        )
        .unwrap_err();
    assert!(matches!(err, RivoraError::Validation(_)));

    let accepted = proposed
        .transitioned(
            ProposalStatus::Accepted,
            "sergio",
            "approved for later implementation",
            at,
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap();
    assert_eq!(accepted.status, ProposalStatus::Accepted);
    assert_eq!(accepted.status.as_str(), "accepted");
    assert!(!accepted.status.as_str().contains("implemented"));
    assert!(accepted
        .transitioned(
            ProposalStatus::Rejected,
            "sergio",
            "changed mind",
            at,
            ProposalTransitionAuthority::ExternalCaller,
        )
        .is_err());
}

#[test]
fn transition_requires_actor_and_reason_and_rejects_invalid_edges() {
    let proposal = generated_proposal(InvestigationId::new());
    let at = Utc::now();

    assert!(proposal
        .transitioned(
            ProposalStatus::Proposed,
            "",
            "review",
            at,
            ProposalTransitionAuthority::ExternalCaller,
        )
        .is_err());
    assert!(proposal
        .transitioned(
            ProposalStatus::Proposed,
            "reviewer",
            "",
            at,
            ProposalTransitionAuthority::ExternalCaller,
        )
        .is_err());
    assert!(proposal
        .transitioned(
            ProposalStatus::Accepted,
            "reviewer",
            "skip review",
            at,
            ProposalTransitionAuthority::ExternalCaller,
        )
        .is_err());
}

#[test]
fn proposal_storage_is_lazy_missing_safe_and_append_only() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::open(dir.path()).unwrap();
    let investigation_id = InvestigationId::new();
    let proposals_dir = dir
        .path()
        .join("investigations")
        .join(investigation_id.to_string())
        .join("proposals");

    let empty = store.list_proposals(&investigation_id).unwrap();
    assert!(empty.proposals.is_empty());
    assert!(empty.diagnostics.is_empty());
    assert!(!proposals_dir.exists());

    let proposal = generated_proposal(investigation_id);
    store.append_proposal(&proposal).unwrap();
    assert!(proposals_dir.exists());
    assert!(store.append_proposal(&proposal).is_err());

    let loaded = store
        .load_proposal(&investigation_id, &proposal.id)
        .unwrap();
    assert_eq!(loaded, proposal);
}

#[test]
fn proposal_listing_is_deterministic_and_isolates_corruption_with_diagnostics() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::open(dir.path()).unwrap();
    let investigation_id = InvestigationId::new();
    let first = generated_proposal(investigation_id);
    let second = generated_proposal(investigation_id);
    store.append_proposal(&second).unwrap();
    store.append_proposal(&first).unwrap();

    let proposals_dir = dir
        .path()
        .join("investigations")
        .join(investigation_id.to_string())
        .join("proposals");
    fs::write(proposals_dir.join("corrupt.json"), b"{ definitely not json").unwrap();

    let listing = store.list_proposals(&investigation_id).unwrap();
    assert_eq!(listing.proposals.len(), 2);
    assert_eq!(listing.diagnostics.len(), 1);
    assert!(listing.diagnostics[0].path.ends_with("corrupt.json"));

    let mut expected = listing.proposals.clone();
    expected.sort_by(|a, b| {
        a.created_at
            .cmp(&b.created_at)
            .then_with(|| a.id.to_string().cmp(&b.id.to_string()))
    });
    assert_eq!(listing.proposals, expected);
}

#[test]
fn proposal_storage_enforces_investigation_ownership_and_lists_revisions() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::open(dir.path()).unwrap();
    let owner = InvestigationId::new();
    let other = InvestigationId::new();
    let original = generated_proposal(owner);
    let successor = original
        .transitioned(
            ProposalStatus::Proposed,
            "reviewer",
            "ready",
            Utc::now(),
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap();
    store.append_proposal(&successor).unwrap();
    store.append_proposal(&original).unwrap();

    assert!(matches!(
        store.load_proposal(&other, &original.id).unwrap_err(),
        RivoraError::ObjectNotFound(_)
    ));
    assert!(store.list_proposals(&other).unwrap().proposals.is_empty());

    let revisions = store
        .list_proposal_revisions(&owner, &original.lineage_id)
        .unwrap();
    assert!(revisions.diagnostics.is_empty());
    assert_eq!(revisions.proposals, vec![original, successor]);
}
