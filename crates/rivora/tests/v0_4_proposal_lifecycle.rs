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
        supporting_evidence_ids: Vec::new(),
        contradicting_evidence_ids: Vec::new(),
        source_recommendation_ids: Vec::new(),
        affected_components: Vec::new(),
        affected_resources: Vec::new(),
    }
}

#[test]
fn capabilities_create_get_list_and_explain_distinct_proposals() {
    let (_dir, _runtime, caps, id) = setup();
    let proposal = caps
        .create_improvement_proposal(id, request("Validate connector timestamps"), "engineer")
        .unwrap();

    assert_eq!(proposal.status, ProposalStatus::Draft);
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
fn explicit_creation_can_preserve_validated_evidence_and_scope() {
    use chrono::Utc;
    use rivora::domain::ObservationKind;

    let (_dir, _runtime, caps, id) = setup();
    let observation = caps
        .ingest_observation(
            id,
            ObservationKind::CheckResult,
            "Configuration validation failed",
            serde_json::json!({"component": "configuration"}),
            "fixture",
            Utc::now(),
            None,
            "engineer",
        )
        .unwrap();
    let workflow = caps
        .run_composite(id, "investigate_engineering_problem", "engineer")
        .unwrap();
    let mut evidence_backed = request("Validate configuration schema");
    evidence_backed.supporting_evidence_ids = vec![observation.0.id, workflow.id];
    evidence_backed.affected_components = vec!["configuration".into()];
    evidence_backed.affected_resources = vec!["config/schema.json".into()];
    let proposal = caps
        .create_improvement_proposal(id, evidence_backed, "engineer")
        .unwrap();
    assert!(proposal
        .supporting_evidence
        .iter()
        .any(|evidence| evidence.object_id == observation.0.id));
    assert!(proposal
        .supporting_evidence
        .iter()
        .any(|evidence| evidence.object_id == workflow.id));
    assert_eq!(proposal.affected_components, vec!["configuration"]);
    assert_eq!(proposal.affected_resources, vec!["config/schema.json"]);
    assert_eq!(proposal.status, ProposalStatus::Proposed);

    let mut foreign = request("Invalid evidence");
    foreign.supporting_evidence_ids = vec![rivora::ObjectId::new()];
    assert!(caps
        .create_improvement_proposal(id, foreign, "engineer")
        .is_err());
}

#[test]
fn explicit_status_actions_preserve_revisions_and_terminal_decisions() {
    let (_dir, _runtime, caps, id) = setup();
    let original = caps
        .create_improvement_proposal(id, request("Bound configuration schema"), "engineer")
        .unwrap();
    let proposed = caps
        .update_improvement_proposal_status(
            id,
            original.id,
            ProposalStatus::Proposed,
            "reviewer",
            "submit evidence-free draft for review",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap();
    let review = caps
        .update_improvement_proposal_status(
            id,
            proposed.id,
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
    assert_eq!(revisions.proposals.len(), 4);
    assert_eq!(revisions.proposals[3].transitions.len(), 3);
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

#[test]
fn historical_snapshots_cannot_branch_and_terminal_content_cannot_be_refined() {
    let (_dir, _runtime, caps, id) = setup();
    let original = caps
        .create_improvement_proposal(id, request("Single revision head"), "engineer")
        .unwrap();
    let refined = caps
        .refine_improvement_proposal(
            id,
            original.id,
            RefineProposalRequest {
                summary: Some("First preserved refinement".into()),
                ..RefineProposalRequest::default()
            },
            "reviewer",
            "first refinement",
        )
        .unwrap();

    assert!(caps
        .refine_improvement_proposal(
            id,
            original.id,
            RefineProposalRequest {
                summary: Some("Competing branch".into()),
                ..RefineProposalRequest::default()
            },
            "reviewer",
            "must not branch",
        )
        .is_err());
    assert!(caps
        .add_improvement_proposal_feedback(
            id,
            original.id,
            ProposalFeedbackCategory::Other,
            "stale feedback",
            "reviewer",
        )
        .is_err());
    assert!(caps
        .update_improvement_proposal_status(
            id,
            original.id,
            ProposalStatus::Deferred,
            "reviewer",
            "stale transition",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .is_err());

    let proposed = caps
        .update_improvement_proposal_status(
            id,
            refined.id,
            ProposalStatus::Proposed,
            "reviewer",
            "submit latest content",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap();
    let review = caps
        .update_improvement_proposal_status(
            id,
            proposed.id,
            ProposalStatus::UnderReview,
            "reviewer",
            "review latest content",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap();
    let accepted = caps
        .update_improvement_proposal_status(
            id,
            review.id,
            ProposalStatus::Accepted,
            "reviewer",
            "accept latest content only",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap();
    assert!(caps
        .refine_improvement_proposal(
            id,
            accepted.id,
            RefineProposalRequest {
                summary: Some("Changed after acceptance".into()),
                ..RefineProposalRequest::default()
            },
            "reviewer",
            "must require a new Proposal",
        )
        .is_err());
    assert!(caps
        .add_improvement_proposal_feedback(
            id,
            accepted.id,
            ProposalFeedbackCategory::Other,
            "must not alter terminal content",
            "reviewer",
        )
        .is_err());

    let referenced = caps
        .record_external_implementation_reference(
            id,
            accepted.id,
            "manual-reference: commit abc123",
            "reviewer",
        )
        .unwrap();
    assert!(caps
        .record_external_implementation_reference(
            id,
            accepted.id,
            "manual-reference: commit stale456",
            "reviewer",
        )
        .is_err());
    let revisions = caps
        .list_improvement_proposal_revisions(id, original.lineage_id)
        .unwrap();
    assert_eq!(revisions.proposals.last().unwrap().id, referenced.id);
    assert!(revisions
        .proposals
        .windows(2)
        .all(|window| window[1].revision_number == window[0].revision_number + 1));
}
