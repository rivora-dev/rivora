//! Non-interactive Workspace smoke workflow (CI / `--smoke`).
//!
//! Exercises Capability paths without the full-screen Unified Workspace.

use chrono::Utc;
use rivora::domain::ObjectId;
use rivora::domain::{
    Confidence, ImprovementProposal, ObservationKind, OutcomeDisposition, ProposalCategory,
    ProposalFeedbackCategory, ProposalPriority, ProposalStatus, ProposalTransitionAuthority,
    RecommendationStatus,
};
use rivora::runtime::execution::CreateExecutionPlanRequest;
use rivora::runtime::proposal::{
    CreateProposalRequest, ProposalPortfolioFilter, RefineProposalRequest,
};
use rivora::{CapabilityService, ExecutionAction, ExecutionPlan, ExecutionPolicyDecision};
use rivora_connectors::github_actions::GitHubActionsConnector;
use rivora_connectors::kubernetes::KubernetesConnector;
use rivora_connectors::sentry::SentryConnector;

use crate::err;

fn proposal_details(proposal: &ImprovementProposal) -> String {
    let implementation = proposal
        .external_implementation_reference
        .as_deref()
        .map(|reference| format!("manually referenced as {reference}; not verified"))
        .unwrap_or_else(|| "not recorded".into());
    format!(
        "Workspace Proposal {} revision {} [{} / {}]\n  {}\n  {}\n  implemented externally: {}\n  verified outcome: not established by Proposal state\nProposal only — not applied, not implemented, not verified.",
        proposal.id,
        proposal.revision_number,
        proposal.status.as_str(),
        proposal.priority.as_str(),
        proposal.title,
        proposal.summary,
        implementation,
    )
}

fn print_workspace_comparison(comparison: &rivora::domain::ProposalComparison) {
    for ranked in &comparison.ranked {
        println!(
            "{}. {} score={:.3}",
            ranked.rank, ranked.proposal_id, ranked.score
        );
        for factor in &ranked.factors {
            println!(
                "    {} weight={:.2} contribution={:.3} — {}",
                factor.name, factor.weight, factor.contribution, factor.explanation
            );
        }
        println!("    {}", ranked.explanation);
    }
    println!("{}", comparison.explanation);
    println!("Ranking is guidance, not a guaranteed correct implementation.");
    println!("Proposal only — not applied, not implemented, not verified.");
}

#[allow(dead_code)]
fn print_workspace_lines(lines: &[String]) {
    if lines.is_empty() {
        println!("  none specified");
    } else {
        for line in lines {
            println!("  • {line}");
        }
    }
}

fn live_execution_confirmation(
    plan: &ExecutionPlan,
    policy: &ExecutionPolicyDecision,
    approval_id: ObjectId,
) -> String {
    let bound_target = plan
        .target_snapshot
        .as_ref()
        .map(|target| {
            format!(
                "provider={} owner={} repository={} ref={} environment={}",
                target.provider,
                target.owner.as_deref().unwrap_or("-"),
                target.repository.as_deref().unwrap_or("-"),
                target.branch_or_ref.as_deref().unwrap_or("-"),
                target.environment
            )
        })
        .unwrap_or_else(|| "unbound (validation required)".into());
    format!(
        "Live execution review\n  plan snapshot: {} (lineage {}, revision {})\n  target: {}:{}\n  bound target: {}\n  capability: {}\n  actions: {}\n  risk: {}\n  policy: {} (live={})\n  approval: {}\n  authority check: Runtime will revalidate target binding, scope, expiration, and one-time use before mutation.\n  API success will still require independent verification.",
        plan.id,
        plan.lineage_id,
        plan.revision_number,
        plan.target_system,
        plan.target_environment,
        bound_target,
        plan.capability_id,
        plan.actions
            .iter()
            .map(|action| format!("{}:{}", action.action_id, action.action_name))
            .collect::<Vec<_>>()
            .join(", "),
        policy.risk_level.as_str(),
        policy.decision.as_str(),
        policy.live_execution_permitted,
        approval_id
    )
}

fn print_lifecycle_run(run: &rivora::CapabilityLifecycleRun) {
    println!(
        "Engineering Loop {} rev {} [{}]",
        run.id,
        run.revision_number,
        run.status.as_str()
    );
    println!(
        "  capability={} invocation={}",
        run.capability_id, run.invocation_id
    );
    for stage in &run.stages {
        let artifacts = if stage.artifact_ids.is_empty() {
            String::new()
        } else {
            format!(
                " artifacts=[{}]",
                stage
                    .artifact_ids
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        println!(
            "  {:<12}  {:<14}  ({}){}{}",
            stage.stage.as_str(),
            stage.status.as_str(),
            stage.participation.as_str(),
            stage
                .detail
                .as_ref()
                .map(|d| format!("  {d}"))
                .unwrap_or_default(),
            artifacts
        );
    }
    println!("  {}", run.explanation);
}

pub(crate) fn smoke_workflow(caps: &CapabilityService) -> Result<(), String> {
    let inv = caps
        .create_investigation("Workspace smoke", Some("automated".into()), "workspace")
        .map_err(err)?;
    let _ = caps
        .ingest_observation(
            inv.id,
            ObservationKind::CheckResult,
            "CI failed in workspace smoke",
            serde_json::json!({"status": "failure", "error": "boom"}),
            "workspace",
            Utc::now(),
            Some("workspace-smoke-1".into()),
            "workspace",
        )
        .map_err(err)?;
    let _ = caps
        .ingest_observation(
            inv.id,
            ObservationKind::Repository,
            "Local repository `smoke`",
            serde_json::json!({"name": "smoke"}),
            "workspace",
            Utc::now(),
            Some("workspace-smoke-repo-1".into()),
            "workspace",
        )
        .map_err(err)?;
    let pipeline = caps.run_full_pipeline(inv.id, "workspace").map_err(err)?;
    assert!(!pipeline.recommendations.is_empty());
    assert_eq!(
        pipeline.recommendations[0].status,
        RecommendationStatus::Proposed
    );
    let _ = caps
        .record_outcome(
            inv.id,
            Some(pipeline.recommendations[0].id),
            OutcomeDisposition::Accepted,
            "smoke accepted",
            None,
            "workspace",
        )
        .map_err(err)?;
    let done = caps
        .complete_investigation(inv.id, Some("smoke complete".into()))
        .map_err(err)?;

    // Investigation Graph: a second investigation over the same
    // repository must be discoverable as related (RFC-015).
    let other = caps
        .create_investigation(
            "Workspace smoke related",
            Some("automated".into()),
            "workspace",
        )
        .map_err(err)?;
    let _ = caps
        .ingest_observation(
            other.id,
            ObservationKind::Repository,
            "Local repository `smoke`",
            serde_json::json!({"name": "smoke"}),
            "workspace",
            Utc::now(),
            Some("workspace-smoke-repo-2".into()),
            "workspace",
        )
        .map_err(err)?;
    let relationships = caps
        .refresh_relationships(done.id, "workspace")
        .map_err(err)?;
    assert!(!relationships.is_empty());
    let related = caps.list_related_investigations(done.id).map_err(err)?;
    assert!(
        related.iter().any(|r| r.related.id == other.id),
        "expected related investigation in workspace smoke"
    );
    let explanation = caps
        .explain_relationship(related[0].relationship.id)
        .map_err(err)?;
    assert!(!explanation.explanation.is_empty());

    // Search and Recall: the completed investigation is searchable and
    // similar investigations are explainable (RFC-016).
    let results = caps
        .search_investigations(rivora::runtime::search::SearchQuery {
            text: Some("smoke repository".into()),
            ..rivora::runtime::search::SearchQuery::default()
        })
        .map_err(err)?;
    assert!(results.iter().all(|r| !r.explanation.is_empty()));
    let similar = caps
        .find_similar_investigations(other.id, Some(5))
        .map_err(err)?;
    assert!(
        similar.iter().any(|r| r.investigation_id == done.id),
        "expected completed investigation as similar in workspace smoke"
    );

    // Recalled Context: attach historical intelligence without rewriting
    // the source Investigation (RFC-017).
    let context = caps
        .attach_recalled_context_from_source(
            other.id,
            done.id,
            Some("workspace smoke prior context".into()),
            "workspace",
        )
        .map_err(err)?;
    assert_eq!(
        context.state,
        rivora::domain::RecalledContextState::Attached
    );
    let listed = caps.list_recalled_context(other.id).map_err(err)?;
    assert_eq!(listed.len(), 1);
    assert!(caps.list_recalled_context(done.id).map_err(err)?.is_empty());

    let patterns = caps.detect_patterns("workspace").map_err(err)?;
    assert!(
        !patterns.is_empty(),
        "expected patterns from shared repository smoke data"
    );
    let trend = caps.summarize_historical_trend(None).map_err(err)?;
    assert!(trend.investigation_count >= 2);
    assert!(!trend.summary.is_empty());

    // v0.3 Engineering Assistance: composites, hypotheses, report (RFC-018/019).
    let assist_inv = caps
        .create_investigation("Workspace smoke assist", None, "workspace")
        .map_err(err)?;
    let _ = caps
        .ingest_observation(
            assist_inv.id,
            ObservationKind::WorkflowRun,
            "CI workflow failed in smoke assist",
            serde_json::json!({"conclusion": "failure"}),
            "github_actions",
            Utc::now(),
            Some("workspace-smoke-assist-ci".into()),
            "workspace",
        )
        .map_err(err)?;
    let wf = caps
        .run_composite(assist_inv.id, "explain_failure", "workspace")
        .map_err(err)?;
    assert!(
        matches!(
            wf.status,
            rivora::domain::WorkflowStatus::Completed
                | rivora::domain::WorkflowStatus::PartiallyCompleted
        ),
        "assist workflow status={}",
        wf.status.as_str()
    );
    let hyps = caps
        .generate_hypotheses(assist_inv.id, "workspace")
        .map_err(err)?;
    assert!(!hyps.is_empty());
    let readiness = caps
        .assess_deployment_readiness(assist_inv.id, "workspace")
        .map_err(err)?;
    assert!(!readiness.dimensions.is_empty());
    let report = caps
        .generate_engineering_report(assist_inv.id, "workspace")
        .map_err(err)?;
    assert!(!report.markdown.is_empty());

    // v0.4 Improvement Proposals: Workspace uses the same Capabilities and
    // preserves feedback, refinement, lifecycle provenance, and boundaries.
    let proposal = caps
        .create_improvement_proposal(
            assist_inv.id,
            CreateProposalRequest {
                title: "Validate workflow fixtures".into(),
                summary: "Add deterministic validation for malformed workflow fixtures".into(),
                rationale: "The current Investigation contains a failed workflow observation"
                    .into(),
                category: ProposalCategory::Reliability,
                priority: ProposalPriority::High,
                confidence: Confidence::new(0.8),
                supporting_evidence_ids: Vec::new(),
                contradicting_evidence_ids: Vec::new(),
                source_recommendation_ids: Vec::new(),
                affected_components: Vec::new(),
                affected_resources: Vec::new(),
            },
            "workspace",
        )
        .map_err(err)?;
    assert_eq!(proposal.status, ProposalStatus::Draft);
    let feedback = caps
        .add_improvement_proposal_feedback(
            assist_inv.id,
            proposal.id,
            ProposalFeedbackCategory::TooBroad,
            "Limit the first revision to workflow fixtures",
            "workspace",
        )
        .map_err(err)?;
    let refined = caps
        .refine_improvement_proposal(
            assist_inv.id,
            feedback.id,
            RefineProposalRequest {
                summary: Some("Validate malformed workflow fixtures".into()),
                affected_components: Some(vec!["workflow fixtures".into()]),
                test_strategy: Some(vec!["Add malformed fixture cases".into()]),
                ..RefineProposalRequest::default()
            },
            "workspace",
            "address explicit scope feedback",
        )
        .map_err(err)?;
    let proposed = caps
        .update_improvement_proposal_status(
            assist_inv.id,
            refined.id,
            ProposalStatus::Proposed,
            "workspace",
            "explicitly submit smoke Draft",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .map_err(err)?;
    let under_review = caps
        .update_improvement_proposal_status(
            assist_inv.id,
            proposed.id,
            ProposalStatus::UnderReview,
            "workspace",
            "explicit smoke review",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .map_err(err)?;
    let accepted = caps
        .update_improvement_proposal_status(
            assist_inv.id,
            under_review.id,
            ProposalStatus::Accepted,
            "workspace",
            "explicit smoke acceptance for possible later implementation",
            ProposalTransitionAuthority::ExternalCaller,
        )
        .map_err(err)?;
    assert_eq!(accepted.status, ProposalStatus::Accepted);
    assert_eq!(
        caps.list_improvement_proposal_revisions(assist_inv.id, proposal.lineage_id)
            .map_err(err)?
            .proposals
            .len(),
        6
    );
    assert_eq!(
        caps.list_improvement_proposals(assist_inv.id)
            .map_err(err)?
            .proposals[0]
            .id,
        accepted.id
    );
    println!("{}", proposal_details(&accepted));

    let execution_plan = caps
        .create_execution_plan(
            assist_inv.id,
            CreateExecutionPlanRequest {
                proposal_id: accepted.id,
                capability_id: "mock.record".into(),
                target_system: "mock".into(),
                target_environment: "sandbox".into(),
                actions: vec![ExecutionAction {
                    action_id: "a1".into(),
                    action_name: "record_mutation".into(),
                    inputs: serde_json::json!({
                        "resource_key": "workspace/smoke",
                        "field": "status",
                        "value": "reviewed"
                    }),
                    continue_on_failure: false,
                }],
                inputs: serde_json::json!({}),
                expected_effects: vec![],
                preconditions: vec![],
                supports_dry_run: true,
            },
            "workspace",
        )
        .map_err(err)?;
    let execution_plan = caps
        .validate_execution_plan(
            assist_inv.id,
            execution_plan.id,
            "workspace",
            "smoke validation",
        )
        .map_err(err)?;
    let (execution_plan, approval) = caps
        .approve_execution_plan(
            assist_inv.id,
            execution_plan.id,
            "workspace",
            "smoke approval review",
            vec![],
            vec![],
            None,
            true,
        )
        .map_err(err)?;
    let policy = caps
        .explain_execution_policy(assist_inv.id, execution_plan.id)
        .map_err(err)?;
    println!(
        "{}",
        live_execution_confirmation(&execution_plan, &policy, approval.id)
    );
    let revisions = caps
        .list_execution_plan_revisions(assist_inv.id, execution_plan.lineage_id)
        .map_err(err)?;
    println!(
        "Workspace Execution plan revisions: {}",
        revisions.plans.len()
    );
    let cancelled = caps
        .cancel_execution_plan(
            assist_inv.id,
            execution_plan.id,
            "workspace",
            "smoke cancellation",
        )
        .map_err(err)?;
    println!(
        "Workspace Execution cancellation: {}",
        cancelled.status.as_str()
    );

    // v0.7 Engineering Loop vertical slice (mock capability).
    let loop_plan = caps
        .create_execution_plan(
            assist_inv.id,
            CreateExecutionPlanRequest {
                proposal_id: accepted.id,
                capability_id: "mock.record".into(),
                target_system: "mock".into(),
                target_environment: "sandbox".into(),
                actions: vec![ExecutionAction {
                    action_id: "loop1".into(),
                    action_name: "record_mutation".into(),
                    inputs: serde_json::json!({
                        "resource_key": "workspace/lifecycle",
                        "field": "status",
                        "value": "looped"
                    }),
                    continue_on_failure: false,
                }],
                inputs: serde_json::json!({}),
                expected_effects: vec![],
                preconditions: vec![],
                supports_dry_run: true,
            },
            "workspace",
        )
        .map_err(err)?;
    let loop_plan = caps
        .validate_execution_plan(
            assist_inv.id,
            loop_plan.id,
            "workspace",
            "lifecycle smoke validation",
        )
        .map_err(err)?;
    let (loop_plan, loop_approval) = caps
        .approve_execution_plan(
            assist_inv.id,
            loop_plan.id,
            "workspace",
            "lifecycle smoke approval",
            vec![],
            vec![],
            None,
            true,
        )
        .map_err(err)?;
    let attempt = caps
        .execute_plan(
            assist_inv.id,
            loop_plan.id,
            loop_approval.id,
            "workspace",
            "workspace-lifecycle-smoke",
            false,
        )
        .map_err(err)?;
    let _ = caps
        .verify_execution_attempt(assist_inv.id, attempt.id, "workspace")
        .map_err(err)?;
    let lifecycle = caps
        .run_capability_lifecycle_for_attempt(assist_inv.id, attempt.id, "workspace")
        .map_err(err)?;
    print_lifecycle_run(&lifecycle);
    let replay = caps
        .run_capability_lifecycle_for_attempt(assist_inv.id, attempt.id, "workspace")
        .map_err(err)?;
    assert_eq!(
        replay.lineage_id, lifecycle.lineage_id,
        "lifecycle replay must be idempotent"
    );
    println!(
        "Workspace Engineering Loop: {} [{}]",
        lifecycle.id,
        lifecycle.status.as_str()
    );
    let caps_listed = caps.list_execution_capabilities();
    assert!(
        caps_listed.iter().any(|c| {
            c.capability_id == "mock.record"
                && c.engineering_loop.memory == rivora::LifecycleParticipation::Supported
                && c.engineering_loop.learning == rivora::LifecycleParticipation::Deferred
                && c.is_complete()
        }),
        "mock.record must declare Engineering Loop participation with complete descriptor"
    );
    let coverage = caps.capability_coverage_report();
    assert!(
        coverage.all_first_party_registered,
        "v0.8 workspace must register all first-party capabilities: {}",
        coverage.gaps.join("; ")
    );
    assert!(
        coverage.all_descriptors_complete,
        "all first-party descriptors must be complete: {}",
        coverage.gaps.join("; ")
    );
    println!("Workspace Capability coverage: {}", coverage.summary);
    println!("Workspace Capability Engineering Loop surface verified.");

    let alternatives = caps
        .generate_proposal_alternatives(assist_inv.id, "workspace")
        .map_err(err)?;
    assert_eq!(alternatives.len(), 2);
    assert!(alternatives
        .iter()
        .all(|proposal| proposal.status == ProposalStatus::Draft));
    println!("Workspace Proposal alternatives: {}", alternatives.len());
    let comparison = caps
        .compare_improvement_proposals(
            assist_inv.id,
            alternatives.iter().map(|proposal| proposal.id).collect(),
        )
        .map_err(err)?;
    assert_eq!(comparison.ranked.len(), 2);
    assert!(comparison
        .ranked
        .iter()
        .all(|ranked| !ranked.factors.is_empty()));
    print_workspace_comparison(&comparison);
    let prioritized = caps
        .prioritize_improvement_proposals(assist_inv.id)
        .map_err(err)?;
    assert!(prioritized.ranked.len() >= 2);
    let plan = caps
        .generate_proposal_verification_plan(assist_inv.id, alternatives[0].id)
        .map_err(err)?;
    assert!(!plan.claims.is_empty());
    let outline = caps
        .generate_proposal_implementation_outline(assist_inv.id, alternatives[0].id)
        .map_err(err)?;
    assert!(!outline.is_empty());
    let provenance = caps
        .explain_improvement_proposal_provenance(assist_inv.id, alternatives[0].id)
        .map_err(err)?;
    assert!(provenance.contains("current"));
    assert!(provenance.contains("labeled historical"));
    println!("Verification Plan is proposed work; it was not executed.");
    println!("{provenance}");

    let artifact = caps
        .generate_proposal_artifact(assist_inv.id, alternatives[0].id, "workspace")
        .map_err(err)?;
    println!("Workspace Proposal Markdown artifact:");
    println!("{}", artifact.markdown);
    println!("Workspace Proposal structured artifact:");
    println!(
        "{}",
        serde_json::to_string_pretty(&artifact).map_err(|error| error.to_string())?
    );
    let handoff = caps
        .generate_coding_agent_handoff(assist_inv.id, alternatives[0].id)
        .map_err(err)?;
    println!("Workspace coding-agent handoff:");
    println!("{handoff}");
    let portfolio = caps
        .proposal_portfolio(
            assist_inv.id,
            ProposalPortfolioFilter {
                status: Some(ProposalStatus::Draft),
                ..ProposalPortfolioFilter::default()
            },
        )
        .map_err(err)?;
    println!("Workspace Proposal portfolio: {}", portfolio.len());
    let trace = caps
        .trace_improvement_proposal(assist_inv.id, alternatives[0].id)
        .map_err(err)?;
    println!(
        "Workspace Proposal trace: Observation ({}) → Memory ({}) → Knowledge ({}) → Evaluation ({}) → Verification ({}) → Recommendation ({}) → Improvement Proposal {}",
        trace.observation_ids.len(),
        trace.memory_ids.len(),
        trace.knowledge_ids.len(),
        trace.evaluation_ids.len(),
        trace.verification_ids.len(),
        trace.recommendation_ids.len(),
        trace.proposal_id,
    );
    println!("{}", trace.explanation);
    let _ = GitHubActionsConnector::new("owner/repo").status();
    let _ = KubernetesConnector::new("default").status();
    let _ = SentryConnector::new("org", "project").status();

    println!(
        "workspace smoke ok: investigation {} status {}",
        done.id, done.status
    );
    Ok(())
}
