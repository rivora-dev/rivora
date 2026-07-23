//! v0.5 end-to-end — Implementation Records and Measured Learning Outcomes remain inert.

use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use rivora::domain::{
    ImplementationReference, ImplementationSource, MeasuredOutcomeStatus, ObservationKind,
    OutcomeClassification, OutcomeEvidenceRelation, ProposalStatus, ProposalTransitionAuthority,
};
use rivora::runtime::outcome::{CollectOutcomeEvidenceRequest, RecordImplementationRequest};
use rivora::{CapabilityService, LocalStore, ObjectId, Runtime};
use serde_json::json;

const ACTOR: &str = "v0.5-e2e";

fn assert_local_operation(started: Instant, operation: &str) {
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{operation} exceeded the local MVP baseline: {:?}",
        started.elapsed()
    );
}

#[test]
fn measured_learning_workflow_is_durable_traceable_and_never_applied() {
    let overall_started = Instant::now();
    let store_dir = tempfile::tempdir().unwrap();
    let runtime = Arc::new(Runtime::new(Arc::new(
        LocalStore::open(store_dir.path()).unwrap(),
    )));
    let caps = CapabilityService::new(Arc::clone(&runtime));

    // 1. Create investigation + observe + pipeline.
    let investigation = caps
        .create_investigation(
            "Config guard measured learning",
            Some("v0.5 full learning loop".into()),
            ACTOR,
        )
        .unwrap();
    caps.ingest_observation(
        investigation.id,
        ObservationKind::CheckResult,
        "Malformed configuration reaches the Runtime",
        json!({"component": "config", "conclusion": "failure"}),
        "e2e-fixture",
        Utc::now(),
        None,
        ACTOR,
    )
    .unwrap();
    let pipeline = caps.run_full_pipeline(investigation.id, ACTOR).unwrap();
    assert!(!pipeline.recommendations.is_empty());

    // 2. Create/accept proposal with success criteria (via generation).
    let generated = caps
        .generate_improvement_proposals(investigation.id, ACTOR)
        .unwrap();
    assert!(!generated.is_empty());
    let mut proposal = generated[0].clone();
    assert!(!proposal.success_criteria.is_empty());

    proposal = caps
        .update_improvement_proposal_status(
            investigation.id,
            proposal.id,
            ProposalStatus::Proposed,
            ACTOR,
            "submit for review",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap();
    proposal = caps
        .update_improvement_proposal_status(
            investigation.id,
            proposal.id,
            ProposalStatus::UnderReview,
            ACTOR,
            "begin review",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap();
    let accepted = caps
        .update_improvement_proposal_status(
            investigation.id,
            proposal.id,
            ProposalStatus::Accepted,
            ACTOR,
            "accept for possible external implementation only",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .unwrap();
    assert_eq!(accepted.status, ProposalStatus::Accepted);
    // Acceptance ≠ implementation ≠ verified.
    assert!(caps
        .list_implementation_records(investigation.id)
        .unwrap()
        .records
        .is_empty());
    assert!(caps
        .list_measured_learning_outcomes(investigation.id)
        .unwrap()
        .outcomes
        .is_empty());

    // 3. Record implementation with git commit ref.
    let impl_started = Instant::now();
    let implementation = caps
        .record_external_implementation(
            investigation.id,
            accepted.id,
            RecordImplementationRequest {
                source: ImplementationSource::GitCommit,
                summary: "Merged bounded config validation guard".into(),
                references: vec![ImplementationReference::CommitSha {
                    sha: "deadbeefcafebabe0123456789abcdef01234567".into(),
                }],
                implemented_at: Some(Utc::now()),
                observed_files: vec!["src/config.rs".into()],
                observed_components: vec!["config".into()],
                declared_scope: "config validation boundary only".into(),
            },
            ACTOR,
        )
        .unwrap();
    assert_local_operation(impl_started, "record implementation");
    let ready = caps
        .mark_implementation_ready(
            investigation.id,
            implementation.id,
            ACTOR,
            "commit reference is sufficient for evaluation",
        )
        .unwrap();

    // 4. Create measured outcome.
    let outcome = caps
        .create_measured_learning_outcome(investigation.id, accepted.id, ready.id, ACTOR)
        .unwrap();
    assert_eq!(outcome.status, MeasuredOutcomeStatus::Draft);
    assert_eq!(outcome.classification, OutcomeClassification::Pending);
    assert!(!outcome.expected_results.is_empty());
    assert!(!outcome.historical_learning_eligible);

    // 5. Add baseline + post-change evidence (+ supports for evaluation).
    let mut head = outcome;
    for expected in head.expected_results.clone() {
        head = caps
            .collect_outcome_evidence(
                investigation.id,
                head.id,
                CollectOutcomeEvidenceRequest {
                    object_id: ObjectId::new(),
                    relation: OutcomeEvidenceRelation::IsBaseline,
                    expected_result_id: Some(expected.id),
                    reason: None,
                },
                ACTOR,
            )
            .unwrap();
        head = caps
            .collect_outcome_evidence(
                investigation.id,
                head.id,
                CollectOutcomeEvidenceRequest {
                    object_id: ObjectId::new(),
                    relation: OutcomeEvidenceRelation::IsPostChange,
                    expected_result_id: Some(expected.id),
                    reason: None,
                },
                ACTOR,
            )
            .unwrap();
        head = caps
            .collect_outcome_evidence(
                investigation.id,
                head.id,
                CollectOutcomeEvidenceRequest {
                    object_id: ObjectId::new(),
                    relation: OutcomeEvidenceRelation::SupportsExpectedResult,
                    expected_result_id: Some(expected.id),
                    reason: Some("observed expected result after change".into()),
                },
                ACTOR,
            )
            .unwrap();
    }

    // 6. Evaluate.
    let eval_started = Instant::now();
    let evaluated = caps
        .evaluate_measured_learning_outcome(investigation.id, head.id, ACTOR)
        .unwrap();
    assert_local_operation(eval_started, "evaluate measured outcome");
    assert_eq!(evaluated.status, MeasuredOutcomeStatus::Evaluated);
    assert_ne!(evaluated.classification, OutcomeClassification::Pending);
    assert!(evaluated.evaluation_report.is_some());

    // 7. Verify with actor + reason.
    let verified = caps
        .verify_measured_learning_outcome(
            investigation.id,
            evaluated.id,
            "reviewer",
            "evidence supports the measured conclusion",
            false,
            None,
        )
        .unwrap();
    assert_eq!(verified.status, MeasuredOutcomeStatus::Verified);
    assert!(verified.historical_learning_eligible);
    assert!(verified.verification.is_some());
    assert_eq!(verified.verification.as_ref().unwrap().actor, "reviewer");

    // 8. Derive patterns.
    let patterns = caps.derive_learning_patterns(ACTOR).unwrap();
    assert!(!patterns.is_empty());

    // 9. Explain influence.
    let influence = caps
        .explain_historical_influence(investigation.id, accepted.id)
        .unwrap();
    assert!(!influence.explanation.is_empty());

    // Ranking may incorporate bounded pattern influence while keeping evidence primary.
    if let Ok(comparison) = caps.prioritize_improvement_proposals(investigation.id) {
        assert!(!comparison.ranked.is_empty());
        for ranked in &comparison.ranked {
            let historical = ranked
                .factors
                .iter()
                .find(|factor| factor.name == "historical_context");
            assert!(historical.is_some());
        }
    }

    // 10. Export.
    let markdown = caps
        .export_measured_learning_outcome_markdown(investigation.id, verified.id)
        .unwrap();
    assert!(markdown.contains("Measured Learning Outcome") || markdown.contains("Boundary"));
    let json = caps
        .export_measured_learning_outcome_json(investigation.id, verified.id)
        .unwrap();
    assert!(json.contains(&verified.id.to_string()));

    let trace = caps
        .trace_measured_learning_outcome(investigation.id, verified.id)
        .unwrap();
    assert_eq!(trace.proposal_id, accepted.id);
    assert_eq!(trace.implementation_record_id, ready.id);
    assert!(trace.explanation.contains("Accepted Proposal") || !trace.explanation.is_empty());

    // 11. Reload store in new Runtime — state persists.
    let verified_id = verified.id;
    let ready_id = ready.id;
    let accepted_id = accepted.id;
    let investigation_id = investigation.id;
    drop(caps);
    drop(runtime);

    let reopened_runtime = Arc::new(Runtime::new(Arc::new(
        LocalStore::open(store_dir.path()).unwrap(),
    )));
    let reopened = CapabilityService::new(reopened_runtime);
    let reloaded_outcome = reopened
        .get_measured_learning_outcome(investigation_id, verified_id)
        .unwrap();
    assert_eq!(reloaded_outcome.status, MeasuredOutcomeStatus::Verified);
    assert_eq!(reloaded_outcome.classification, verified.classification);
    let reloaded_impl = reopened
        .get_implementation_record(investigation_id, ready_id)
        .unwrap();
    assert_eq!(reloaded_impl.id, ready_id);
    let reloaded_proposal = reopened
        .get_improvement_proposal(investigation_id, accepted_id)
        .unwrap();
    assert_eq!(reloaded_proposal.status, ProposalStatus::Accepted);
    assert!(!reopened.list_learning_patterns().unwrap().is_empty());

    // 12. Acceptance ≠ implementation ≠ verified.
    assert_ne!(
        reloaded_proposal.status.as_str(),
        reloaded_impl.status.as_str()
    );
    assert_ne!(
        reloaded_impl.status.as_str(),
        reloaded_outcome.status.as_str()
    );
    assert_eq!(reloaded_proposal.status, ProposalStatus::Accepted);
    assert_eq!(reloaded_outcome.status, MeasuredOutcomeStatus::Verified);

    // Mixed/inconclusive path: create a second implementation without supporting evidence.
    let second_impl = reopened
        .record_external_implementation(
            investigation_id,
            accepted_id,
            RecordImplementationRequest {
                source: ImplementationSource::HumanDeclared,
                summary: "Partial follow-up without measured proof".into(),
                references: vec![ImplementationReference::HumanNote {
                    note: "incomplete rollout".into(),
                }],
                implemented_at: None,
                observed_files: Vec::new(),
                observed_components: Vec::new(),
                declared_scope: "partial".into(),
            },
            ACTOR,
        )
        .unwrap();
    let second_ready = reopened
        .mark_implementation_ready(
            investigation_id,
            second_impl.id,
            ACTOR,
            "ready for inconclusive measurement",
        )
        .unwrap();
    let inconclusive = reopened
        .create_measured_learning_outcome(investigation_id, accepted_id, second_ready.id, ACTOR)
        .unwrap();
    // Evaluate with insufficient evidence → inconclusive/not implemented class of results.
    let inconclusive_eval = reopened
        .evaluate_measured_learning_outcome(investigation_id, inconclusive.id, ACTOR)
        .unwrap();
    assert!(matches!(
        inconclusive_eval.status,
        MeasuredOutcomeStatus::Evaluated | MeasuredOutcomeStatus::UnderEvaluation
    ));
    assert!(matches!(
        inconclusive_eval.classification,
        OutcomeClassification::Inconclusive
            | OutcomeClassification::NotImplemented
            | OutcomeClassification::Unsuccessful
            | OutcomeClassification::Pending
            | OutcomeClassification::Mixed
    ));

    assert!(
        overall_started.elapsed() < Duration::from_secs(30),
        "the local v0.5 end-to-end workflow exceeded its MVP baseline: {:?}",
        overall_started.elapsed()
    );
}
