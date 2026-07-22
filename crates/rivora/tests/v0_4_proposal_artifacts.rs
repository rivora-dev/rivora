//! v0.4 Phase 3 — Proposal artifacts, portfolio, traceability, and handoff (RFC-021).

use std::sync::Arc;

use chrono::Utc;
use rivora::domain::{
    Confidence, ObservationKind, ProposalCategory, ProposalFeedbackCategory, ProposalPriority,
    ProposalStatus, ProposalTransitionAuthority,
};
use rivora::runtime::proposal::{
    CreateProposalRequest, ProposalPortfolioFilter, RefineProposalRequest,
};
use rivora::{CapabilityService, LocalStore, Runtime};

fn setup() -> (
    tempfile::TempDir,
    Arc<Runtime>,
    CapabilityService,
    rivora::InvestigationId,
) {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Arc::new(Runtime::new(Arc::new(
        LocalStore::open(dir.path()).unwrap(),
    )));
    let caps = CapabilityService::new(Arc::clone(&runtime));
    let investigation = caps
        .create_investigation("Connector timestamp validation", None, "tester")
        .unwrap();
    (dir, runtime, caps, investigation.id)
}

fn seed_reasoning(caps: &CapabilityService, id: rivora::InvestigationId) {
    caps.ingest_observation(
        id,
        ObservationKind::CheckResult,
        "CI failed because a connector timestamp omitted its timezone",
        serde_json::json!({"component": "connector", "status": "failed"}),
        "test-fixture",
        Utc::now(),
        None,
        "tester",
    )
    .unwrap();
    caps.derive_knowledge(id, "tester").unwrap();
    caps.evaluate_investigation(id, "tester").unwrap();
    caps.verify_all(id, "tester").unwrap();
    caps.generate_recommendation(id, "tester").unwrap();
}

fn request(
    title: &str,
    category: ProposalCategory,
    priority: ProposalPriority,
) -> CreateProposalRequest {
    CreateProposalRequest {
        title: title.into(),
        summary: "Add deterministic validation and focused fixtures.".into(),
        rationale: "Current evidence shows malformed input reaches the Runtime.".into(),
        category,
        priority,
        confidence: Confidence::new(0.8),
        supporting_evidence_ids: Vec::new(),
        contradicting_evidence_ids: Vec::new(),
        source_recommendation_ids: Vec::new(),
        affected_components: Vec::new(),
        affected_resources: Vec::new(),
    }
}

fn sorted_ids(mut ids: Vec<rivora::ObjectId>) -> Vec<rivora::ObjectId> {
    ids.sort_by_key(ToString::to_string);
    ids
}

#[test]
fn markdown_and_structured_artifacts_are_complete_redacted_deterministic_and_durable() {
    let (dir, _runtime, caps, id) = setup();
    seed_reasoning(&caps, id);
    let generated = caps
        .generate_improvement_proposals(id, "runtime")
        .unwrap()
        .remove(0);
    let secret = "artifact-super-secret-value";
    let github_token = "github_pat_11AA22BB33CC44DD55";
    let aws_access_key = "AKIAIOSFODNN7EXAMPLE";
    let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0In0.signature";
    let private_key = "-----BEGIN PRIVATE KEY----- private-material -----END PRIVATE KEY-----";
    let feedback = caps
        .add_improvement_proposal_feedback(
            id,
            generated.id,
            ProposalFeedbackCategory::TooBroad,
            "narrow scope before export",
            "reviewer",
        )
        .unwrap();
    let refined = caps
        .refine_improvement_proposal(
            id,
            feedback.id,
            RefineProposalRequest {
                summary: Some(format!("Validate the boundary with token={secret}")),
                rationale: Some(format!(
                    "A password={secret}, {github_token}, {aws_access_key}, and {jwt} must never appear in export"
                )),
                affected_components: Some(vec![
                    "connector".into(),
                    "https://user:password@example.test/private".into(),
                ]),
                test_strategy: Some(vec![
                    "Add malformed timestamp fixtures".into(),
                    r#"Never emit JSON {"token":"json-secret-value"}"#.into(),
                    private_key.into(),
                ]),
                ..RefineProposalRequest::default()
            },
            "reviewer",
            "add export-safety coverage",
        )
        .unwrap();

    let proposals_dir = dir
        .path()
        .join("investigations")
        .join(id.to_string())
        .join("proposals");
    std::fs::write(
        proposals_dir.join(format!("token={secret}.json")),
        b"{not valid json",
    )
    .unwrap();

    let first = caps
        .generate_proposal_artifact(id, refined.id, "exporter")
        .unwrap();
    let second = caps
        .generate_proposal_artifact(id, refined.id, "exporter")
        .unwrap();

    assert_ne!(first.id, second.id, "each durable export is a new artifact");
    assert_eq!(first.markdown, second.markdown);
    assert_eq!(first.proposal_id, refined.id);
    assert_eq!(first.proposal.id, refined.id);
    assert_eq!(first.revision_diagnostics.len(), 1);
    assert!(first
        .markdown
        .contains("revision history may be incomplete"));
    assert_eq!(
        first.boundary,
        "Proposal only — not applied, not implemented, not verified."
    );
    let markdown_lower = first.markdown.to_lowercase();
    for required in [
        &refined.title,
        "Status",
        "Priority",
        "Summary",
        "Problem statement",
        "Supporting evidence",
        "Contradicting evidence",
        "Historical context",
        "Assumptions",
        "Affected components",
        "Proposed change",
        "Alternatives considered",
        "Implementation outline",
        "Test strategy",
        "Verification Plan",
        "Risks",
        "Success criteria",
        "Expected impact",
        "Unresolved questions",
        "Provenance",
        "Revision history",
        "Proposal only",
    ] {
        assert!(
            markdown_lower.contains(&required.to_lowercase()),
            "Markdown artifact must contain {required:?}"
        );
    }
    for sensitive in [
        secret,
        github_token,
        aws_access_key,
        jwt,
        "user:password",
        "json-secret-value",
        "private-material",
    ] {
        assert!(!first.markdown.contains(sensitive));
    }
    assert!(first.markdown.contains("[REDACTED]"));
    assert!(first.markdown.contains(&generated.id.to_string()));
    assert!(first.markdown.contains("Parent snapshot"));
    assert!(first.markdown.contains("add export-safety coverage"));
    assert_eq!(
        first.markdown.matches("narrow scope before export").count(),
        1
    );

    let structured = serde_json::to_string_pretty(&first).unwrap();
    assert!(!structured.contains(secret));
    assert!(structured.contains("[REDACTED]"));
    let decoded: rivora::domain::ProposalArtifact = serde_json::from_str(&structured).unwrap();
    assert_eq!(decoded, first);

    let artifacts_dir = dir
        .path()
        .join("investigations")
        .join(id.to_string())
        .join("proposal_artifacts");
    std::fs::write(artifacts_dir.join("corrupt.json"), b"{not valid json").unwrap();
    let mut foreign = first.clone();
    foreign.id = rivora::ObjectId::new();
    foreign.investigation_id = rivora::InvestigationId::new();
    std::fs::write(
        artifacts_dir.join(format!("{}.json", foreign.id)),
        serde_json::to_vec_pretty(&foreign).unwrap(),
    )
    .unwrap();

    drop(caps);
    let reopened = CapabilityService::new(Arc::new(Runtime::new(Arc::new(
        LocalStore::open(dir.path()).unwrap(),
    ))));
    let artifacts = reopened.list_proposal_artifacts(id).unwrap();
    assert_eq!(artifacts.artifacts.len(), 2);
    assert_eq!(artifacts.diagnostics.len(), 2);
    assert_eq!(artifacts.artifacts[0].markdown, first.markdown);
    assert_eq!(artifacts.artifacts[1].markdown, second.markdown);
}

#[test]
fn coding_agent_handoff_is_complete_bounded_text_and_never_invokes_an_agent() {
    let (_dir, _runtime, caps, id) = setup();
    seed_reasoning(&caps, id);
    let proposal = caps
        .generate_improvement_proposals(id, "runtime")
        .unwrap()
        .remove(0);

    let handoff = caps.generate_coding_agent_handoff(id, proposal.id).unwrap();
    let handoff_lower = handoff.to_lowercase();
    for required in [
        &proposal.title,
        "Repository context",
        "Affected subsystem",
        "Relevant RFCs",
        "Architectural invariants",
        "Bounded implementation objective",
        "Out of scope",
        "Likely modules and files",
        "Tests to write",
        "Verification Plan",
        "Compatibility requirements",
        "Safety boundaries",
        "Acceptance criteria",
        "This is an implementation proposal.",
        "Review repository state and current code before acting.",
        "Do not treat suggested files or implementation details as authoritative without inspecting the repository.",
        "Do not exceed the approved Proposal scope.",
    ] {
        assert!(
            handoff_lower.contains(&required.to_lowercase()),
            "coding-agent handoff must contain {required:?}"
        );
    }
    assert!(handoff.contains("does not invoke a coding agent"));
    assert!(handoff.contains("not applied"));
}

#[test]
fn proposal_portfolio_filters_latest_revisions_without_collapsing_lineages() {
    let (_dir, _runtime, caps, id) = setup();
    seed_reasoning(&caps, id);
    let source_recommendation = caps
        .runtime()
        .store()
        .list_recommendations(&id)
        .unwrap()
        .remove(0);
    let generated = caps.generate_improvement_proposals(id, "runtime").unwrap();

    let accepted = caps
        .create_improvement_proposal(
            id,
            request(
                "Accept explicit validation",
                ProposalCategory::Reliability,
                ProposalPriority::High,
            ),
            "engineer",
        )
        .unwrap();
    let accepted = caps
        .update_improvement_proposal_status(
            id,
            accepted.id,
            ProposalStatus::Proposed,
            "reviewer",
            "submit explicit draft",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap();
    let accepted = caps
        .update_improvement_proposal_status(
            id,
            accepted.id,
            ProposalStatus::UnderReview,
            "reviewer",
            "begin review",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap();
    let accepted = caps
        .update_improvement_proposal_status(
            id,
            accepted.id,
            ProposalStatus::Accepted,
            "reviewer",
            "accepted as a proposal only",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap();

    let rejected = caps
        .create_improvement_proposal(
            id,
            request(
                "Reject broad migration",
                ProposalCategory::Testing,
                ProposalPriority::Medium,
            ),
            "engineer",
        )
        .unwrap();
    let rejected = caps
        .update_improvement_proposal_status(
            id,
            rejected.id,
            ProposalStatus::Rejected,
            "reviewer",
            "scope exceeds verified need",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap();

    let unresolved = caps
        .create_improvement_proposal(
            id,
            request(
                "Validate the connector boundary",
                ProposalCategory::Reliability,
                ProposalPriority::High,
            ),
            "engineer",
        )
        .unwrap();
    let unresolved_latest = caps
        .refine_improvement_proposal(
            id,
            unresolved.id,
            RefineProposalRequest {
                title: Some("Validate the connector timestamp boundary".into()),
                affected_components: Some(vec!["connector".into()]),
                ..RefineProposalRequest::default()
            },
            "reviewer",
            "identify the affected component",
        )
        .unwrap();

    let all = caps
        .proposal_portfolio(id, ProposalPortfolioFilter::default())
        .unwrap();
    assert_eq!(all.len(), generated.len() + 3);
    assert!(all
        .iter()
        .any(|proposal| proposal.id == unresolved_latest.id));
    assert!(!all.iter().any(|proposal| proposal.id == unresolved.id));

    let accepted_only = caps
        .proposal_portfolio(
            id,
            ProposalPortfolioFilter {
                status: Some(ProposalStatus::Accepted),
                ..ProposalPortfolioFilter::default()
            },
        )
        .unwrap();
    assert_eq!(accepted_only.len(), 1);
    assert_eq!(accepted_only[0].id, accepted.id);

    let rejected_only = caps
        .proposal_portfolio(
            id,
            ProposalPortfolioFilter {
                status: Some(ProposalStatus::Rejected),
                ..ProposalPortfolioFilter::default()
            },
        )
        .unwrap();
    assert_eq!(rejected_only.len(), 1);
    assert_eq!(rejected_only[0].id, rejected.id);

    let unresolved_high = caps
        .proposal_portfolio(
            id,
            ProposalPortfolioFilter {
                unresolved_high_priority: true,
                ..ProposalPortfolioFilter::default()
            },
        )
        .unwrap();
    assert!(unresolved_high
        .iter()
        .any(|proposal| proposal.id == unresolved_latest.id));
    assert!(!unresolved_high
        .iter()
        .any(|proposal| proposal.id == accepted.id));

    let component = caps
        .proposal_portfolio(
            id,
            ProposalPortfolioFilter {
                affected_component: Some("connector".into()),
                ..ProposalPortfolioFilter::default()
            },
        )
        .unwrap();
    assert!(component
        .iter()
        .any(|proposal| proposal.id == unresolved_latest.id));

    let from_recommendation = caps
        .proposal_portfolio(
            id,
            ProposalPortfolioFilter {
                source_recommendation_id: Some(source_recommendation.id),
                ..ProposalPortfolioFilter::default()
            },
        )
        .unwrap();
    assert_eq!(from_recommendation.len(), generated.len());
    assert!(from_recommendation.iter().all(|proposal| proposal
        .source_recommendation_ids
        .contains(&source_recommendation.id)));
}

#[test]
fn trace_and_manual_external_reference_are_inert_durable_and_preserve_sources() {
    let (_dir, runtime, caps, id) = setup();
    seed_reasoning(&caps, id);
    let proposal = caps
        .generate_improvement_proposals(id, "runtime")
        .unwrap()
        .remove(0);

    let before = (
        runtime.store().list_observations(&id).unwrap(),
        runtime.store().list_memory(&id).unwrap(),
        runtime.store().list_knowledge(&id).unwrap(),
        runtime.store().list_evaluations(&id).unwrap(),
        runtime.store().list_verifications(&id).unwrap(),
        runtime.store().list_recommendations(&id).unwrap(),
        runtime.store().list_learning(&id).unwrap(),
    );

    let trace = caps.trace_improvement_proposal(id, proposal.id).unwrap();
    assert_eq!(
        trace.observation_ids,
        sorted_ids(before.0.iter().map(|object| object.id).collect())
    );
    assert_eq!(
        trace.memory_ids,
        sorted_ids(before.1.iter().map(|object| object.id).collect())
    );
    assert_eq!(
        trace.knowledge_ids,
        sorted_ids(before.2.iter().map(|object| object.id).collect())
    );
    assert_eq!(
        trace.evaluation_ids,
        sorted_ids(before.3.iter().map(|object| object.id).collect())
    );
    assert_eq!(
        trace.verification_ids,
        sorted_ids(before.4.iter().map(|object| object.id).collect())
    );
    assert_eq!(
        trace.recommendation_ids,
        sorted_ids(before.5.iter().map(|object| object.id).collect())
    );
    assert_eq!(trace.proposal_id, proposal.id);
    assert_eq!(trace.external_implementation_reference, None);
    assert!(trace
        .explanation
        .contains("accepted does not mean implemented"));
    assert!(trace.explanation.contains("not a verified outcome"));

    let reference = "manual-reference: commit abc123";
    let revised = caps
        .record_external_implementation_reference(id, proposal.id, reference, "reviewer")
        .unwrap();
    assert_ne!(revised.id, proposal.id);
    assert_eq!(revised.parent_proposal_id, Some(proposal.id));
    assert_eq!(revised.revision_number, proposal.revision_number + 1);
    assert_eq!(
        revised.external_implementation_reference.as_deref(),
        Some(reference)
    );
    assert_eq!(proposal.external_implementation_reference, None);

    let revised_trace = caps.trace_improvement_proposal(id, revised.id).unwrap();
    assert_eq!(
        revised_trace.external_implementation_reference.as_deref(),
        Some(reference)
    );
    let revisions = caps
        .list_improvement_proposal_revisions(id, proposal.lineage_id)
        .unwrap();
    assert_eq!(revisions.proposals.len(), 2);
    assert_eq!(revisions.proposals[0], proposal);
    assert_eq!(revisions.proposals[1], revised);

    let after = (
        runtime.store().list_observations(&id).unwrap(),
        runtime.store().list_memory(&id).unwrap(),
        runtime.store().list_knowledge(&id).unwrap(),
        runtime.store().list_evaluations(&id).unwrap(),
        runtime.store().list_verifications(&id).unwrap(),
        runtime.store().list_recommendations(&id).unwrap(),
        runtime.store().list_learning(&id).unwrap(),
    );
    assert_eq!(before, after, "Phase 3 operations must not mutate sources");
}
