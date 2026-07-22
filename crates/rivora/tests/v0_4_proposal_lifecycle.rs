//! v0.4 Phase 1 — Runtime and Capability Proposal lifecycle (RFC-020).

use std::sync::Arc;

use rivora::domain::{
    Confidence, ProposalCategory, ProposalFeedbackCategory, ProposalPriority, ProposalStatus,
    ProposalTransitionAuthority,
};
use rivora::runtime::proposal::{CreateProposalRequest, RefineProposalRequest};
use rivora::{CapabilityService, LocalStore, Runtime};

fn setup() -> (
    tempfile::TempDir,
    Arc<Runtime>,
    CapabilityService,
    rivora::InvestigationId,
) {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(LocalStore::open(dir.path()).unwrap());
    let runtime = Arc::new(Runtime::new(store));
    let caps = CapabilityService::new(Arc::clone(&runtime));
    let inv = caps
        .create_investigation("proposal lifecycle", None, "tester")
        .unwrap();
    (dir, runtime, caps, inv.id)
}

fn request(title: &str) -> CreateProposalRequest {
    CreateProposalRequest {
        title: title.into(),
        summary: "Add deterministic validation and focused fixtures.".into(),
        rationale: "A verified failure shows malformed input reaches the Runtime.".into(),
        category: ProposalCategory::Reliability,
        priority: ProposalPriority::High,
        confidence: Confidence::new(0.8),
    }
}

#[test]
fn capabilities_create_get_list_and_explain_distinct_proposals() {
    let (_dir, _runtime, caps, id) = setup();
    let proposal = caps
        .create_improvement_proposal(id, request("Validate connector timestamps"), "engineer")
        .unwrap();

    assert_eq!(proposal.status, ProposalStatus::Proposed);
    assert!(proposal.source_recommendation_ids.is_empty());
    let loaded = caps.get_improvement_proposal(id, proposal.id).unwrap();
    assert_eq!(loaded, proposal);
    let listed = caps.list_improvement_proposals(id).unwrap();
    assert_eq!(listed.proposals, vec![proposal.clone()]);
    let explanation = caps.explain_improvement_proposal(id, proposal.id).unwrap();
    assert!(explanation.contains("Proposal only"));
    assert!(explanation.contains("not applied"));
    assert!(explanation.contains("not implemented"));
    assert!(explanation.contains("not verified"));
}

#[test]
fn explicit_status_actions_preserve_revisions_and_terminal_decisions() {
    let (_dir, _runtime, caps, id) = setup();
    let original = caps
        .create_improvement_proposal(id, request("Bound configuration schema"), "engineer")
        .unwrap();
    let review = caps
        .update_improvement_proposal_status(
            id,
            original.id,
            ProposalStatus::UnderReview,
            "reviewer",
            "begin review",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap();
    let accepted = caps
        .update_improvement_proposal_status(
            id,
            review.id,
            ProposalStatus::Accepted,
            "reviewer",
            "approved as a proposal only",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap();

    assert_eq!(
        caps.get_improvement_proposal(id, original.id).unwrap(),
        original
    );
    assert_eq!(accepted.status, ProposalStatus::Accepted);
    let revisions = caps
        .list_improvement_proposal_revisions(id, accepted.lineage_id)
        .unwrap();
    assert_eq!(revisions.proposals.len(), 3);
    assert_eq!(revisions.proposals[2].transitions.len(), 2);
    assert!(caps
        .update_improvement_proposal_status(
            id,
            accepted.id,
            ProposalStatus::Rejected,
            "reviewer",
            "invalid terminal move",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .is_err());
}

#[test]
fn feedback_refinement_and_superseding_preserve_original_content() {
    let (_dir, _runtime, caps, id) = setup();
    let original = caps
        .create_improvement_proposal(id, request("Broad schema migration"), "engineer")
        .unwrap();
    let with_feedback = caps
        .add_improvement_proposal_feedback(
            id,
            original.id,
            ProposalFeedbackCategory::TooBroad,
            "Limit the first change to connector timestamps.",
            "reviewer",
        )
        .unwrap();
    let refined = caps
        .refine_improvement_proposal(
            id,
            with_feedback.id,
            RefineProposalRequest {
                title: Some("Validate connector timestamps".into()),
                test_strategy: Some(vec!["Add malformed and timezone boundary fixtures".into()]),
                ..RefineProposalRequest::default()
            },
            "reviewer",
            "narrow scope after feedback",
        )
        .unwrap();
    let replacement = caps
        .create_improvement_proposal(id, request("Shared timestamp validator"), "engineer")
        .unwrap();
    let superseded = caps
        .supersede_improvement_proposal(
            id,
            refined.id,
            replacement.id,
            "reviewer",
            "prefer the smaller shared validator",
        )
        .unwrap();

    assert_eq!(original.title, "Broad schema migration");
    assert_eq!(refined.title, "Validate connector timestamps");
    assert_eq!(refined.feedback.len(), 1);
    assert_eq!(superseded.status, ProposalStatus::Superseded);
    assert_eq!(superseded.superseding_proposal_id, Some(replacement.id));
}

#[test]
fn proposal_lifecycle_does_not_mutate_source_engineering_objects() {
    let (_dir, runtime, caps, id) = setup();
    let before = (
        runtime.store().list_observations(&id).unwrap(),
        runtime.store().list_memory(&id).unwrap(),
        runtime.store().list_knowledge(&id).unwrap(),
        runtime.store().list_recommendations(&id).unwrap(),
        runtime.store().list_learning(&id).unwrap(),
    );
    let proposal = caps
        .create_improvement_proposal(id, request("No mutation"), "engineer")
        .unwrap();
    let _ = caps
        .update_improvement_proposal_status(
            id,
            proposal.id,
            ProposalStatus::Deferred,
            "reviewer",
            "wait for evidence",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap();
    let after = (
        runtime.store().list_observations(&id).unwrap(),
        runtime.store().list_memory(&id).unwrap(),
        runtime.store().list_knowledge(&id).unwrap(),
        runtime.store().list_recommendations(&id).unwrap(),
        runtime.store().list_learning(&id).unwrap(),
    );
    assert_eq!(before, after);
}
