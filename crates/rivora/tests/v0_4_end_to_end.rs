//! v0.4 end-to-end — evidence-backed improvement proposals remain inert.

use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use rivora::domain::{
    EvidenceScope, ObservationKind, OutcomeDisposition, ProposalFeedbackCategory, ProposalStatus,
    ProposalTransitionAuthority, WorkflowStatus,
};
use rivora::runtime::proposal::RefineProposalRequest;
use rivora::{CapabilityService, InvestigationId, LocalStore, Runtime};
use rivora_connectors::github_actions::GitHubActionsConnector;
use rivora_connectors::local::LocalConnector;
use serde_json::{json, Value};

const ACTOR: &str = "v0.4-e2e";

fn ingest_normalized(
    caps: &CapabilityService,
    investigation_id: InvestigationId,
    observations: impl IntoIterator<Item = rivora_connectors::NormalizedObservation>,
) {
    for observation in observations {
        caps.ingest_observation(
            investigation_id,
            observation.kind,
            observation.summary,
            observation.payload,
            observation.source,
            observation.observed_at,
            observation.idempotency_key,
            ACTOR,
        )
        .unwrap();
    }
}

fn completed_prior_investigation(
    caps: &CapabilityService,
    title: &str,
    failure: &str,
    disposition: OutcomeDisposition,
    notes: &str,
) -> InvestigationId {
    let investigation = caps.create_investigation(title, None, ACTOR).unwrap();
    caps.ingest_observation(
        investigation.id,
        ObservationKind::CheckResult,
        failure,
        json!({"component": "connector", "conclusion": "failure"}),
        "historical-fixture",
        Utc::now(),
        None,
        ACTOR,
    )
    .unwrap();
    let pipeline = caps.run_full_pipeline(investigation.id, ACTOR).unwrap();
    assert!(!pipeline.verifications.is_empty());
    let outcome = caps
        .record_outcome(
            investigation.id,
            Some(pipeline.recommendations[0].id),
            disposition,
            notes,
            Some("recorded historical impact".into()),
            ACTOR,
        )
        .unwrap();
    assert_eq!(outcome.disposition, disposition);
    caps.runtime()
        .complete_investigation(investigation.id, Some("historical case closed".into()))
        .unwrap();
    investigation.id
}

fn source_snapshot(caps: &CapabilityService, investigation_id: InvestigationId) -> Value {
    let store = caps.runtime().store();
    json!({
        "investigation": caps.open_investigation(investigation_id).unwrap(),
        "observations": store.list_observations(&investigation_id).unwrap(),
        "memory": store.list_memory(&investigation_id).unwrap(),
        "knowledge": store.list_knowledge(&investigation_id).unwrap(),
        "evaluations": store.list_evaluations(&investigation_id).unwrap(),
        "verifications": store.list_verifications(&investigation_id).unwrap(),
        "recommendations": store.list_recommendations(&investigation_id).unwrap(),
        "learning": store.list_learning(&investigation_id).unwrap(),
        "hypotheses": store.list_hypotheses(&investigation_id).unwrap(),
        "recalled_context": store.list_recalled_context(&investigation_id).unwrap(),
    })
}

fn assert_local_operation(started: Instant, operation: &str) {
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{operation} exceeded the local MVP baseline: {:?}",
        started.elapsed()
    );
}

#[test]
fn improvement_proposal_workflow_is_durable_traceable_and_never_applied() {
    let overall_started = Instant::now();
    let store_dir = tempfile::tempdir().unwrap();
    let observed_repository = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(observed_repository.path().join(".rivora/events")).unwrap();
    let source_path = observed_repository.path().join("connector.rs");
    let event_path = observed_repository
        .path()
        .join(".rivora/events/timestamp-failure.json");
    let source_before = b"pub fn parse_timestamp(input: &str) -> &str { input }\n".to_vec();
    let event_before = br#"{
        "summary": "connector timestamp validation failed",
        "error": "timestamp has no explicit timezone",
        "idempotency_key": "v04-local-failure"
    }"#
    .to_vec();
    std::fs::write(&source_path, &source_before).unwrap();
    std::fs::write(&event_path, &event_before).unwrap();

    let runtime = Arc::new(Runtime::new(Arc::new(
        LocalStore::open(store_dir.path()).unwrap(),
    )));
    let caps = CapabilityService::new(Arc::clone(&runtime));

    let successful_prior = completed_prior_investigation(
        &caps,
        "Prior connector timezone failure",
        "Connector validation failed before timezone checks were added",
        OutcomeDisposition::Successful,
        "Targeted timezone validation prevented recurrence",
    );
    let unsuccessful_prior = completed_prior_investigation(
        &caps,
        "Prior broad connector migration",
        "Broad connector migration failed compatibility verification",
        OutcomeDisposition::Unsuccessful,
        "The broad migration was too risky and did not resolve the failure",
    );
    let successful_prior_before = source_snapshot(&caps, successful_prior);
    let unsuccessful_prior_before = source_snapshot(&caps, unsuccessful_prior);

    let active = caps
        .create_investigation(
            "Connector timestamps require deterministic validation",
            None,
            ACTOR,
        )
        .unwrap();

    let local_connector = LocalConnector::new(observed_repository.path());
    ingest_normalized(&caps, active.id, local_connector.observe().unwrap());

    let actions_connector = GitHubActionsConnector::new("acme/connectors");
    assert!(actions_connector.status().read_only);
    let actions = GitHubActionsConnector::observe_from_fixture(&json!({
        "repository": "acme/connectors",
        "workflow_runs": [{
            "id": 404,
            "name": "Connector validation",
            "status": "completed",
            "conclusion": "failure",
            "event": "push",
            "updated_at": "2026-07-22T12:00:00Z",
            "jobs": [{
                "id": 405,
                "name": "timestamp-fixtures",
                "conclusion": "failure"
            }]
        }]
    }))
    .unwrap();
    ingest_normalized(&caps, active.id, actions);

    let attached = caps
        .attach_recalled_context_from_source(
            active.id,
            successful_prior,
            Some("targeted validation succeeded in a related incident".into()),
            ACTOR,
        )
        .unwrap();
    let dismissed = caps
        .attach_recalled_context_from_source(
            active.id,
            unsuccessful_prior,
            Some("consider the broad migration history".into()),
            ACTOR,
        )
        .unwrap();
    let dismissed = caps
        .dismiss_recalled_context(active.id, dismissed.id, ACTOR)
        .unwrap();

    let assistance = caps
        .run_composite(active.id, "investigate_engineering_problem", ACTOR)
        .unwrap();
    assert!(matches!(
        assistance.status,
        WorkflowStatus::Completed | WorkflowStatus::PartiallyCompleted
    ));
    let hypotheses = caps.generate_hypotheses(active.id, ACTOR).unwrap();
    assert!(!hypotheses.is_empty());
    assert_eq!(hypotheses[0].rank, 1);
    assert!(hypotheses
        .iter()
        .all(|hypothesis| !hypothesis.verification_summary.is_empty()));
    let receipts = caps.verify_all(active.id, ACTOR).unwrap();
    assert!(!receipts.is_empty());
    let recommendations = caps.generate_recommendation(active.id, ACTOR).unwrap();
    assert!(!recommendations.is_empty());

    let current_sources_before_proposals = source_snapshot(&caps, active.id);
    let proposal_started = Instant::now();
    let proposal_workflow = caps
        .run_composite(active.id, "propose_engineering_improvement", ACTOR)
        .unwrap();
    assert_local_operation(proposal_started, "Proposal generation");
    assert_eq!(proposal_workflow.status, WorkflowStatus::Completed);
    assert!(proposal_workflow
        .steps
        .iter()
        .all(|step| !step.capability.contains("apply") && !step.capability.contains("accept")));
    assert_eq!(
        current_sources_before_proposals,
        source_snapshot(&caps, active.id),
        "the Proposal composite must not mutate source Engineering Objects"
    );

    let generated = caps
        .list_improvement_proposals(active.id)
        .unwrap()
        .proposals;
    assert_eq!(generated.len(), 2);
    assert!(generated
        .iter()
        .all(|proposal| proposal.status == ProposalStatus::Draft));
    assert!(generated.iter().all(|proposal| {
        proposal
            .related_investigation_ids
            .contains(&successful_prior)
            && !proposal
                .related_investigation_ids
                .contains(&unsuccessful_prior)
            && proposal
                .generation_inputs
                .iter()
                .any(|input| input.scope == EvidenceScope::Historical)
            && attached.source_object_ids.iter().all(|source_id| {
                proposal
                    .generation_inputs
                    .iter()
                    .any(|input| input.object_id == *source_id)
            })
            && dismissed.source_object_ids.iter().all(|source_id| {
                !proposal
                    .generation_inputs
                    .iter()
                    .any(|input| input.object_id == *source_id)
            })
    }));
    assert!(generated.iter().all(|proposal| {
        !proposal.supporting_evidence.is_empty()
            && serde_json::to_value(proposal)
                .unwrap()
                .get("contradicting_evidence")
                .is_some()
            && !proposal.risks.is_empty()
    }));

    let comparison_started = Instant::now();
    let comparison = caps
        .compare_improvement_proposals(active.id, generated.iter().map(|p| p.id).collect())
        .unwrap();
    assert_local_operation(comparison_started, "Proposal comparison");
    assert_eq!(comparison.ranked.len(), 2);
    assert!(comparison.ranked.iter().all(|ranked| {
        !ranked.factors.is_empty()
            && ranked
                .factors
                .iter()
                .all(|factor| !factor.name.is_empty() && !factor.explanation.is_empty())
    }));
    assert!(comparison.ranked.iter().all(|ranked| ranked
        .factors
        .iter()
        .find(|factor| factor.name == "historical_context")
        .is_some_and(|factor| factor.explanation.contains("1 successful, 0 unsuccessful"))));
    for proposal in &generated {
        assert!(!caps
            .generate_proposal_implementation_outline(active.id, proposal.id)
            .unwrap()
            .is_empty());
        let plan = caps
            .generate_proposal_verification_plan(active.id, proposal.id)
            .unwrap();
        assert!(!plan.claims.is_empty());
        assert!(!plan.tests.is_empty());
        assert!(!plan.success_criteria.is_empty());
    }

    let original = generated[0].clone();
    let feedback_revision = caps
        .add_improvement_proposal_feedback(
            active.id,
            original.id,
            ProposalFeedbackCategory::TooBroad,
            "Limit the Proposal to the connector timestamp boundary",
            "human-reviewer",
        )
        .unwrap();
    let refined = caps
        .refine_improvement_proposal(
            active.id,
            feedback_revision.id,
            RefineProposalRequest {
                title: Some("Validate connector timestamps at the ingestion boundary".into()),
                summary: Some(
                    "Reject timestamps without explicit timezones and add focused fixtures.".into(),
                ),
                affected_components: Some(vec!["connector-ingestion".into()]),
                test_strategy: Some(vec![
                    "Add malformed, boundary, and timezone-offset timestamp fixtures".into(),
                ]),
                ..RefineProposalRequest::default()
            },
            "human-reviewer",
            "refine the Proposal from explicit too-broad feedback",
        )
        .unwrap();
    let revisions = caps
        .list_improvement_proposal_revisions(active.id, original.lineage_id)
        .unwrap()
        .proposals;
    assert_eq!(revisions.len(), 3);
    assert_eq!(revisions[0], original);
    assert!(revisions
        .iter()
        .any(|revision| revision.id == feedback_revision.id));
    assert!(refined
        .feedback
        .iter()
        .any(|feedback| feedback.category == ProposalFeedbackCategory::TooBroad));

    let rejected = caps
        .update_improvement_proposal_status(
            active.id,
            generated[1].id,
            ProposalStatus::Rejected,
            "human-reviewer",
            "broader alternative exceeds the verified need",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap();
    let proposed = caps
        .update_improvement_proposal_status(
            active.id,
            refined.id,
            ProposalStatus::Proposed,
            "human-reviewer",
            "submit the bounded revision for review",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap();
    let under_review = caps
        .update_improvement_proposal_status(
            active.id,
            proposed.id,
            ProposalStatus::UnderReview,
            "human-reviewer",
            "begin explicit review",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap();
    let accepted = caps
        .update_improvement_proposal_status(
            active.id,
            under_review.id,
            ProposalStatus::Accepted,
            "human-reviewer",
            "accept as a Proposal only; implementation remains external",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap();
    assert_eq!(accepted.status, ProposalStatus::Accepted);
    assert_eq!(accepted.external_implementation_reference, None);
    assert!(caps.list_learning(active.id).unwrap().is_empty());

    let artifact_started = Instant::now();
    let artifact = caps
        .generate_proposal_artifact(active.id, accepted.id, "human-exporter")
        .unwrap();
    assert_local_operation(artifact_started, "Proposal artifact export");
    assert!(artifact.markdown.contains(&accepted.title));
    assert!(artifact.markdown.contains("Proposal only"));
    assert!(artifact.markdown.contains("not implemented"));
    let structured = serde_json::to_string_pretty(&artifact).unwrap();
    let structured_round_trip: rivora::domain::ProposalArtifact =
        serde_json::from_str(&structured).unwrap();
    assert_eq!(structured_round_trip, artifact);

    let handoff_started = Instant::now();
    let handoff = caps
        .generate_coding_agent_handoff(active.id, accepted.id)
        .unwrap();
    assert_local_operation(handoff_started, "Coding-agent handoff generation");
    assert!(handoff.contains("This is an implementation proposal."));
    assert!(handoff.contains("Do not exceed the approved Proposal scope."));
    assert!(handoff.contains("does not invoke a coding agent"));

    let trace = caps
        .trace_improvement_proposal(active.id, accepted.id)
        .unwrap();
    assert_eq!(trace.proposal_id, accepted.id);
    assert!(!trace.observation_ids.is_empty());
    assert!(!trace.memory_ids.is_empty());
    assert!(!trace.knowledge_ids.is_empty());
    assert!(!trace.evaluation_ids.is_empty());
    assert!(!trace.verification_ids.is_empty());
    assert!(!trace.recommendation_ids.is_empty());
    assert!(trace.explanation.contains("not a verified outcome"));

    assert_eq!(
        current_sources_before_proposals,
        source_snapshot(&caps, active.id),
        "feedback, status, comparison, and export must not mutate source objects"
    );
    assert_eq!(std::fs::read(&source_path).unwrap(), source_before);
    assert_eq!(std::fs::read(&event_path).unwrap(), event_before);
    assert_eq!(
        source_snapshot(&caps, successful_prior),
        successful_prior_before
    );
    assert_eq!(
        source_snapshot(&caps, unsuccessful_prior),
        unsuccessful_prior_before
    );

    drop(caps);
    drop(runtime);
    let reopened_runtime = Arc::new(Runtime::new(Arc::new(
        LocalStore::open(store_dir.path()).unwrap(),
    )));
    let reopened = CapabilityService::new(reopened_runtime);
    let latest = reopened
        .list_improvement_proposals(active.id)
        .unwrap()
        .proposals;
    assert_eq!(latest.len(), 2);
    assert!(latest
        .iter()
        .any(|proposal| proposal.id == accepted.id && proposal.status == ProposalStatus::Accepted));
    assert!(latest
        .iter()
        .any(|proposal| proposal.id == rejected.id && proposal.status == ProposalStatus::Rejected));
    let reopened_revisions = reopened
        .list_improvement_proposal_revisions(active.id, original.lineage_id)
        .unwrap()
        .proposals;
    assert!(reopened_revisions
        .iter()
        .any(|revision| revision.id == original.id));
    assert!(reopened_revisions
        .iter()
        .any(|revision| revision.id == accepted.id));
    assert!(reopened_revisions
        .iter()
        .any(|revision| !revision.feedback.is_empty()));
    let reloaded_artifacts = reopened.list_proposal_artifacts(active.id).unwrap();
    assert_eq!(reloaded_artifacts.artifacts, vec![artifact.clone()]);
    assert!(reloaded_artifacts.diagnostics.is_empty());
    assert_eq!(
        reopened
            .generate_coding_agent_handoff(active.id, accepted.id)
            .unwrap(),
        handoff
    );
    assert_eq!(std::fs::read(&source_path).unwrap(), source_before);
    assert_eq!(std::fs::read(&event_path).unwrap(), event_before);
    assert_eq!(
        source_snapshot(&reopened, successful_prior),
        successful_prior_before
    );
    assert_eq!(
        source_snapshot(&reopened, unsuccessful_prior),
        unsuccessful_prior_before
    );
    assert!(
        overall_started.elapsed() < Duration::from_secs(30),
        "the local v0.4 end-to-end workflow exceeded its MVP baseline: {:?}",
        overall_started.elapsed()
    );
}
