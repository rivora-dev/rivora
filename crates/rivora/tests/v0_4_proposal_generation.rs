//! v0.4 Phase 2 — deterministic Proposal generation and comparison (RFC-021).

use std::sync::Arc;

use chrono::Utc;
use rivora::domain::{
    Confidence, EvidenceScope, Hypothesis, HypothesisStatus, ObservationKind, ProposalStatus,
    Provenance,
};
use rivora::{CapabilityService, LocalStore, ObjectId, Runtime};

fn ingest_failure(caps: &CapabilityService, id: rivora::InvestigationId, text: &str) {
    caps.ingest_observation(
        id,
        ObservationKind::CheckResult,
        text,
        serde_json::json!({"component": "connector", "status": "failed"}),
        "test-fixture",
        Utc::now(),
        None,
        "tester",
    )
    .unwrap();
    caps.derive_knowledge(id, "tester").unwrap();
}

#[test]
fn deterministic_generation_preserves_inputs_boundaries_and_source_objects() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Arc::new(Runtime::new(Arc::new(
        LocalStore::open(dir.path()).unwrap(),
    )));
    let caps = CapabilityService::new(Arc::clone(&runtime));

    let current = caps
        .create_investigation("Timezone validation failure", None, "tester")
        .unwrap();
    ingest_failure(
        &caps,
        current.id,
        "CI failed because connector timestamp lacks an explicit timezone",
    );
    caps.evaluate_investigation(current.id, "tester").unwrap();
    caps.verify_all(current.id, "tester").unwrap();
    caps.generate_recommendation(current.id, "tester").unwrap();

    let supporting = ObjectId::new();
    let contradicting = ObjectId::new();
    runtime
        .store()
        .append_hypothesis(&Hypothesis::new(
            current.id,
            "Timestamp parsing may be accepting ambiguous values.",
            HypothesisStatus::Inconclusive,
            Confidence::new(0.6),
            vec![supporting],
            vec![contradicting],
            Vec::new(),
            "test_unverified_v1",
            "unverified",
            1,
            Provenance::now("tester", "test"),
        ))
        .unwrap();

    let attached_source = caps
        .create_investigation("Prior successful timestamp validation", None, "tester")
        .unwrap();
    ingest_failure(
        &caps,
        attached_source.id,
        "Prior connector failure improved after timestamp validation",
    );
    let attached = caps
        .attach_recalled_context_from_source(
            current.id,
            attached_source.id,
            Some("relevant prior outcome".into()),
            "reviewer",
        )
        .unwrap();

    let dismissed_source = caps
        .create_investigation("Dismissed unrelated schema incident", None, "tester")
        .unwrap();
    ingest_failure(
        &caps,
        dismissed_source.id,
        "Unrelated schema migration failed",
    );
    let dismissed = caps
        .attach_recalled_context_from_source(
            current.id,
            dismissed_source.id,
            Some("candidate only".into()),
            "reviewer",
        )
        .unwrap();
    caps.dismiss_recalled_context(current.id, dismissed.id, "reviewer")
        .unwrap();

    let before = (
        runtime.store().list_observations(&current.id).unwrap(),
        runtime.store().list_memory(&current.id).unwrap(),
        runtime.store().list_knowledge(&current.id).unwrap(),
        runtime.store().list_evaluations(&current.id).unwrap(),
        runtime.store().list_verifications(&current.id).unwrap(),
        runtime.store().list_recommendations(&current.id).unwrap(),
        runtime.store().list_hypotheses(&current.id).unwrap(),
        runtime.store().list_recalled_context(&current.id).unwrap(),
    );

    let proposals = caps
        .generate_improvement_proposals(current.id, "runtime")
        .unwrap();
    assert!(proposals.len() >= 2);
    assert!(proposals.iter().all(|p| p.status == ProposalStatus::Draft));
    assert_eq!(
        proposals[0].alternative_group_id,
        proposals[1].alternative_group_id
    );
    assert_ne!(proposals[0].lineage_id, proposals[1].lineage_id);

    for proposal in &proposals {
        assert!(!proposal.generation_inputs.is_empty());
        assert!(proposal
            .supporting_evidence
            .iter()
            .any(|e| e.scope == EvidenceScope::Current));
        assert!(proposal
            .contradicting_evidence
            .iter()
            .any(|e| e.object_id == contradicting));
        assert!(proposal
            .related_investigation_ids
            .contains(&attached_source.id));
        assert!(!proposal
            .related_investigation_ids
            .contains(&dismissed_source.id));
        assert!(proposal
            .generation_inputs
            .iter()
            .any(|e| e.scope == EvidenceScope::Historical));
        assert!(attached.source_object_ids.iter().all(|id| proposal
            .generation_inputs
            .iter()
            .any(|e| e.object_id == *id)));
        assert!(!proposal.implementation_outline.is_empty());
        assert!(!proposal.test_strategy.is_empty());
        assert!(!proposal.verification_plan.claims.is_empty());
        assert!(!proposal.priority_explanation.is_empty());
        assert!(proposal
            .assumptions
            .iter()
            .any(|a| a.contains("Unverified hypothesis")));
    }

    let after = (
        runtime.store().list_observations(&current.id).unwrap(),
        runtime.store().list_memory(&current.id).unwrap(),
        runtime.store().list_knowledge(&current.id).unwrap(),
        runtime.store().list_evaluations(&current.id).unwrap(),
        runtime.store().list_verifications(&current.id).unwrap(),
        runtime.store().list_recommendations(&current.id).unwrap(),
        runtime.store().list_hypotheses(&current.id).unwrap(),
        runtime.store().list_recalled_context(&current.id).unwrap(),
    );
    assert_eq!(
        before, after,
        "Proposal generation must not mutate source objects"
    );
}

#[test]
fn comparison_exposes_factors_and_prefers_bounded_verifiable_change() {
    let dir = tempfile::tempdir().unwrap();
    let caps = CapabilityService::new(Arc::new(Runtime::new(Arc::new(
        LocalStore::open(dir.path()).unwrap(),
    ))));
    let inv = caps
        .create_investigation("Repeated connector timeout", None, "tester")
        .unwrap();
    ingest_failure(&caps, inv.id, "Connector timeout failed verification");
    caps.evaluate_investigation(inv.id, "tester").unwrap();
    let proposals = caps
        .generate_proposal_alternatives(inv.id, "runtime")
        .unwrap();
    let comparison = caps
        .compare_improvement_proposals(inv.id, proposals.iter().map(|p| p.id).collect())
        .unwrap();

    assert_eq!(comparison.ranked.len(), proposals.len());
    assert_eq!(comparison.ranked[0].rank, 1);
    assert!(comparison.ranked.iter().all(|ranked| {
        !ranked.factors.is_empty()
            && ranked
                .factors
                .iter()
                .all(|factor| !factor.explanation.is_empty())
    }));
    assert!(comparison.explanation.contains("not guaranteed"));
    assert!(!caps
        .generate_proposal_verification_plan(inv.id, proposals[0].id)
        .unwrap()
        .tests
        .is_empty());
    assert!(!caps
        .generate_proposal_implementation_outline(inv.id, proposals[0].id)
        .unwrap()
        .is_empty());
    assert!(caps
        .explain_improvement_proposal_provenance(inv.id, proposals[0].id)
        .unwrap()
        .contains("labeled historical"));
}

#[test]
fn propose_engineering_improvement_composite_is_bounded_and_never_accepts() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Arc::new(Runtime::new(Arc::new(
        LocalStore::open(dir.path()).unwrap(),
    )));
    let caps = CapabilityService::new(Arc::clone(&runtime));
    let inv = caps
        .create_investigation("Bounded proposal composite", None, "tester")
        .unwrap();
    ingest_failure(
        &caps,
        inv.id,
        "CI failed at configuration validation boundary",
    );
    caps.evaluate_investigation(inv.id, "tester").unwrap();

    let definition = caps
        .list_composite_capabilities()
        .into_iter()
        .find(|definition| definition.id == "propose_engineering_improvement")
        .expect("v0.4 composite definition");
    assert_eq!(
        definition.core_capabilities,
        vec![
            "recall_proposal_inputs",
            "generate_improvement_proposals",
            "compare_improvement_proposals",
            "summarize_proposal_ranking",
        ]
    );

    let before = (
        runtime.store().list_memory(&inv.id).unwrap(),
        runtime.store().list_knowledge(&inv.id).unwrap(),
        runtime.store().list_evaluations(&inv.id).unwrap(),
    );
    let workflow = caps
        .run_composite(inv.id, "propose_engineering_improvement", "tester")
        .unwrap();
    assert_eq!(workflow.status.as_str(), "completed");
    assert!(workflow
        .steps
        .iter()
        .all(|step| !step.capability.contains("accept") && !step.capability.contains("apply")));
    let proposals = caps.list_improvement_proposals(inv.id).unwrap().proposals;
    assert!(proposals.len() >= 2);
    assert!(proposals
        .iter()
        .all(|proposal| proposal.status == ProposalStatus::Draft));
    let after = (
        runtime.store().list_memory(&inv.id).unwrap(),
        runtime.store().list_knowledge(&inv.id).unwrap(),
        runtime.store().list_evaluations(&inv.id).unwrap(),
    );
    assert_eq!(before, after);
}
