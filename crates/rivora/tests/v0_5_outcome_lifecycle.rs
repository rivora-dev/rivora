//! v0.5 Phase 1–2 — Measured Learning Outcome lifecycle, evaluation, verify (RFC-022/023/024).

use std::sync::Arc;

use rivora::domain::{
    ImplementationReference, ImplementationSource, MeasuredOutcomeStatus, OutcomeClassification,
    OutcomeEvidenceRelation, ProposalCategory, ProposalGenerationMethod, ProposalPriority,
    Provenance,
};
use rivora::runtime::outcome::{CollectOutcomeEvidenceRequest, RecordImplementationRequest};
use rivora::storage::LocalStore;
use rivora::{CapabilityService, Confidence, ImprovementProposal, ObjectId, Runtime};

struct Fixture {
    _dir: tempfile::TempDir,
    caps: CapabilityService,
    inv_id: rivora::InvestigationId,
    proposal_id: ObjectId,
    impl_id: ObjectId,
}

fn setup() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(LocalStore::open(dir.path()).unwrap());
    let runtime = Arc::new(Runtime::new(store));
    let caps = CapabilityService::new(runtime);
    let inv = caps
        .create_investigation("v0.5 outcome", None, "tester")
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
    proposal.verification_plan.success_criteria =
        vec!["Verification observes rejected payloads".into()];
    caps.runtime().store().append_proposal(&proposal).unwrap();

    let record = caps
        .record_external_implementation(
            inv.id,
            proposal.id,
            RecordImplementationRequest {
                source: ImplementationSource::PullRequest,
                summary: "Merged config guard PR".into(),
                references: vec![ImplementationReference::PullRequest {
                    reference: "99".into(),
                }],
                implemented_at: None,
                observed_files: vec!["src/config.rs".into()],
                observed_components: vec!["config".into()],
                declared_scope: "config validation".into(),
            },
            "engineer",
        )
        .unwrap();
    let ready = caps
        .mark_implementation_ready(inv.id, record.id, "engineer", "ready")
        .unwrap();

    Fixture {
        _dir: dir,
        caps,
        inv_id: inv.id,
        proposal_id: proposal.id,
        impl_id: ready.id,
    }
}

#[test]
fn create_outcome_seeds_expected_results_from_proposal() {
    let fx = setup();
    let outcome = fx
        .caps
        .create_measured_learning_outcome(fx.inv_id, fx.proposal_id, fx.impl_id, "engineer")
        .unwrap();

    assert_eq!(outcome.status, MeasuredOutcomeStatus::Draft);
    assert_eq!(outcome.classification, OutcomeClassification::Pending);
    assert_eq!(outcome.proposal_id, fx.proposal_id);
    assert_eq!(outcome.implementation_record_id, fx.impl_id);
    assert!(!outcome.expected_results.is_empty());
    assert!(!outcome.historical_learning_eligible);

    let loaded = fx
        .caps
        .get_measured_learning_outcome(fx.inv_id, outcome.id)
        .unwrap();
    assert_eq!(loaded.id, outcome.id);
}

#[test]
fn collect_evidence_evaluate_and_verify() {
    let fx = setup();
    let outcome = fx
        .caps
        .create_measured_learning_outcome(fx.inv_id, fx.proposal_id, fx.impl_id, "engineer")
        .unwrap();
    let expected_id = outcome.expected_results[0].id;

    let with_baseline = fx
        .caps
        .collect_outcome_evidence(
            fx.inv_id,
            outcome.id,
            CollectOutcomeEvidenceRequest {
                object_id: ObjectId::new(),
                relation: OutcomeEvidenceRelation::IsBaseline,
                expected_result_id: Some(expected_id),
                reason: None,
            },
            "engineer",
        )
        .unwrap();
    assert_eq!(
        with_baseline.status,
        MeasuredOutcomeStatus::EvidenceCollection
    );

    let with_post = fx
        .caps
        .collect_outcome_evidence(
            fx.inv_id,
            with_baseline.id,
            CollectOutcomeEvidenceRequest {
                object_id: ObjectId::new(),
                relation: OutcomeEvidenceRelation::IsPostChange,
                expected_result_id: Some(expected_id),
                reason: None,
            },
            "engineer",
        )
        .unwrap();

    // Support each expected result.
    let mut head = with_post;
    for expected in head.expected_results.clone() {
        head = fx
            .caps
            .collect_outcome_evidence(
                fx.inv_id,
                head.id,
                CollectOutcomeEvidenceRequest {
                    object_id: ObjectId::new(),
                    relation: OutcomeEvidenceRelation::SupportsExpectedResult,
                    expected_result_id: Some(expected.id),
                    reason: Some("observed as expected".into()),
                },
                "engineer",
            )
            .unwrap();
    }

    let evaluated = fx
        .caps
        .evaluate_measured_learning_outcome(fx.inv_id, head.id, "runtime")
        .unwrap();
    assert_eq!(evaluated.status, MeasuredOutcomeStatus::Evaluated);
    assert_eq!(evaluated.classification, OutcomeClassification::Successful);
    assert!(evaluated.evaluation_report.is_some());
    assert!(
        evaluated
            .evaluation_report
            .as_ref()
            .unwrap()
            .verification_ready
    );

    // Verify requires actor + reason; cannot auto-verify by confidence alone.
    let err = fx
        .caps
        .verify_measured_learning_outcome(fx.inv_id, evaluated.id, "", "looks good", false, None)
        .unwrap_err();
    assert!(err.to_string().contains("validation") || err.to_string().contains("required"));

    let verified = fx
        .caps
        .verify_measured_learning_outcome(
            fx.inv_id,
            evaluated.id,
            "reviewer",
            "evidence supports successful outcome",
            false,
            None,
        )
        .unwrap();
    assert_eq!(verified.status, MeasuredOutcomeStatus::Verified);
    assert!(verified.historical_learning_eligible);
    assert!(verified.verification.is_some());
    assert_eq!(verified.verification.as_ref().unwrap().actor, "reviewer");

    // Verified revisions are immutable for content revise.
    let err = fx
        .caps
        .revise_measured_learning_outcome(
            fx.inv_id,
            verified.id,
            Default::default(),
            "reviewer",
            "try edit",
        )
        .unwrap_err();
    assert!(err.to_string().contains("cannot revise") || err.to_string().contains("validation"));

    // Reload persistence via a second store handle on the same root.
    let path = fx._dir.path().to_path_buf();
    let store = Arc::new(LocalStore::open(&path).unwrap());
    let runtime = Arc::new(Runtime::new(store));
    let caps = CapabilityService::new(runtime);
    let reloaded = caps
        .get_measured_learning_outcome(verified.investigation_id, verified.id)
        .unwrap();
    assert_eq!(reloaded.status, MeasuredOutcomeStatus::Verified);
    assert_eq!(reloaded.classification, OutcomeClassification::Successful);
    // Keep fixture alive so the temp directory is not deleted mid-assert.
    let _keep = fx;
}

#[test]
fn verify_requires_evaluated_status() {
    let fx = setup();
    let outcome = fx
        .caps
        .create_measured_learning_outcome(fx.inv_id, fx.proposal_id, fx.impl_id, "engineer")
        .unwrap();
    let err = fx
        .caps
        .verify_measured_learning_outcome(
            fx.inv_id,
            outcome.id,
            "reviewer",
            "too early",
            false,
            None,
        )
        .unwrap_err();
    assert!(err.to_string().contains("Evaluated") || err.to_string().contains("validation"));
}

#[test]
fn acceptance_is_not_implementation_or_outcome() {
    // Architectural distinction: creating a proposal does not create implementation or outcome.
    let fx = setup();
    let listing = fx.caps.list_implementation_records(fx.inv_id).unwrap();
    // setup creates one implementation lineage explicitly (with a ready revision).
    let lineages: std::collections::HashSet<_> =
        listing.records.iter().map(|r| r.lineage_id).collect();
    assert_eq!(lineages.len(), 1);
    assert!(listing
        .records
        .iter()
        .any(|r| r.status.as_str() == "ready_for_evaluation"));

    // No Measured Learning Outcomes until explicitly created.
    let outcomes = fx.caps.list_measured_learning_outcomes(fx.inv_id).unwrap();
    assert!(outcomes.outcomes.is_empty());
}

#[test]
fn derive_patterns_from_verified_outcomes() {
    let fx = setup();
    let outcome = fx
        .caps
        .create_measured_learning_outcome(fx.inv_id, fx.proposal_id, fx.impl_id, "engineer")
        .unwrap();
    let mut head = outcome;
    for expected in head.expected_results.clone() {
        head = fx
            .caps
            .collect_outcome_evidence(
                fx.inv_id,
                head.id,
                CollectOutcomeEvidenceRequest {
                    object_id: ObjectId::new(),
                    relation: OutcomeEvidenceRelation::IsBaseline,
                    expected_result_id: Some(expected.id),
                    reason: None,
                },
                "engineer",
            )
            .unwrap();
        head = fx
            .caps
            .collect_outcome_evidence(
                fx.inv_id,
                head.id,
                CollectOutcomeEvidenceRequest {
                    object_id: ObjectId::new(),
                    relation: OutcomeEvidenceRelation::IsPostChange,
                    expected_result_id: Some(expected.id),
                    reason: None,
                },
                "engineer",
            )
            .unwrap();
        head = fx
            .caps
            .collect_outcome_evidence(
                fx.inv_id,
                head.id,
                CollectOutcomeEvidenceRequest {
                    object_id: ObjectId::new(),
                    relation: OutcomeEvidenceRelation::SupportsExpectedResult,
                    expected_result_id: Some(expected.id),
                    reason: Some("ok".into()),
                },
                "engineer",
            )
            .unwrap();
    }
    let evaluated = fx
        .caps
        .evaluate_measured_learning_outcome(fx.inv_id, head.id, "runtime")
        .unwrap();
    let verified = fx
        .caps
        .verify_measured_learning_outcome(
            fx.inv_id,
            evaluated.id,
            "reviewer",
            "verified for learning",
            false,
            None,
        )
        .unwrap();
    assert!(verified.historical_learning_eligible);

    let patterns = fx.caps.derive_learning_patterns("runtime").unwrap();
    assert!(!patterns.is_empty());
    let listed = fx.caps.list_learning_patterns().unwrap();
    assert_eq!(listed.len(), patterns.len());

    let influence = fx
        .caps
        .explain_historical_influence(fx.inv_id, fx.proposal_id)
        .unwrap();
    assert!(!influence.explanation.is_empty());

    let md = fx
        .caps
        .export_measured_learning_outcome_markdown(fx.inv_id, verified.id)
        .unwrap();
    assert!(md.contains("Measured Learning Outcome"));
    assert!(md.contains("Boundary"));

    let retired = fx
        .caps
        .retire_learning_pattern(patterns[0].id, "reviewer", "no longer applicable")
        .unwrap();
    assert_eq!(retired.status.as_str(), "retired");
}

#[test]
fn trace_explains_boundaries() {
    let fx = setup();
    let outcome = fx
        .caps
        .create_measured_learning_outcome(fx.inv_id, fx.proposal_id, fx.impl_id, "engineer")
        .unwrap();
    let trace = fx
        .caps
        .trace_measured_learning_outcome(fx.inv_id, outcome.id)
        .unwrap();
    assert_eq!(trace.proposal_id, fx.proposal_id);
    assert_eq!(trace.implementation_record_id, fx.impl_id);
    assert!(trace.explanation.contains("Accepted Proposal"));
}
