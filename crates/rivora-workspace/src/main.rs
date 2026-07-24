//! Rivora Workspace — primary interactive experience (RFC-003).
//!
//! Thin UI over `CapabilityService`. No Runtime reasoning is implemented here.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use chrono::Utc;
use clap::Parser;
use console::style;
use dialoguer::{Confirm, Input, Select};
use rivora::domain::{
    Confidence, ImplementationReference, ImplementationSource, ImprovementProposal, Investigation,
    InvestigationId, ObjectId, ObservationKind, OutcomeDisposition, OutcomeEvidenceRelation,
    ProposalCategory, ProposalFeedbackCategory, ProposalPriority, ProposalStatus,
    ProposalTransitionAuthority, RecommendationStatus,
};
use rivora::runtime::execution::CreateExecutionPlanRequest;
use rivora::runtime::outcome::{CollectOutcomeEvidenceRequest, RecordImplementationRequest};
use rivora::runtime::proposal::{
    CreateProposalRequest, ProposalPortfolioFilter, RefineProposalRequest,
};
use rivora::storage::LocalStore;
use rivora::{
    CapabilityService, ExecutionAction, ExecutionPlan, ExecutionPolicyDecision,
    MockExecutionCapability, Runtime,
};
use rivora_connectors::github_actions::GitHubActionsConnector;
use rivora_connectors::kubernetes::KubernetesConnector;
use rivora_connectors::local::LocalConnector;
use rivora_connectors::register_first_party_github_execution_capabilities;
use rivora_connectors::sentry::SentryConnector;

const EXECUTION_BOUNDARY: &str = "Execution Through External Systems — only explicitly approved, bounded capabilities; Proposal acceptance ≠ execution approval.";

#[derive(Debug, Parser)]
#[command(
    name = "rivora-workspace",
    version,
    about = "Rivora Workspace — interactive Investigations"
)]
struct Args {
    /// Data directory for local Runtime storage.
    #[arg(long, default_value = ".rivora/data")]
    data_dir: PathBuf,

    /// Run a single non-interactive demo workflow (for tests/CI).
    #[arg(long)]
    smoke: bool,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse();
    let caps = open_capabilities(&args.data_dir)?;

    if args.smoke {
        return smoke_workflow(&caps);
    }

    println!("{}", style("Rivora Workspace").bold().cyan());
    println!("Primary interactive experience for Investigations.\n");

    loop {
        let items = vec![
            "Create Investigation",
            "Open Investigation",
            "List Investigations",
            "Search Investigations",
            "Prior Outcomes",
            "Patterns",
            "Historical Trends",
            "Connectors",
            "Quit",
        ];
        let choice = Select::new()
            .with_prompt("Workspace")
            .items(&items)
            .default(0)
            .interact()
            .map_err(|e| e.to_string())?;

        match choice {
            0 => {
                let title: String = Input::new()
                    .with_prompt("Title")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let description: String = Input::new()
                    .with_prompt("Description (optional)")
                    .allow_empty(true)
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let desc = if description.trim().is_empty() {
                    None
                } else {
                    Some(description)
                };
                let inv = caps
                    .create_investigation(title, desc, "workspace")
                    .map_err(err)?;
                println!("{} Created {} [{}]", style("✓").green(), inv.id, inv.status);
                investigation_session(&caps, inv)?;
            }
            1 => {
                let id: String = Input::new()
                    .with_prompt("Investigation id")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let inv = caps
                    .open_investigation(id.parse().map_err(err)?)
                    .map_err(err)?;
                investigation_session(&caps, inv)?;
            }
            2 => {
                let ids = caps.list_investigations().map_err(err)?;
                if ids.is_empty() {
                    println!("No investigations yet.");
                } else {
                    let total = ids.len();
                    let limit = rivora::DEFAULT_LIST_LIMIT.min(total);
                    for id in ids.into_iter().take(limit) {
                        let inv = caps.open_investigation(id).map_err(err)?;
                        println!("  {}  [{}]  {}", inv.id, inv.status, inv.title);
                    }
                    if total > limit {
                        println!(
                            "  … showing {limit} of {total} (bounded for Workspace responsiveness)"
                        );
                    }
                }
            }
            3 => {
                let text: String = Input::new()
                    .with_prompt("Search text (optional)")
                    .allow_empty(true)
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let repository: String = Input::new()
                    .with_prompt("Repository filter (optional)")
                    .allow_empty(true)
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let query = rivora::runtime::search::SearchQuery {
                    text: if text.trim().is_empty() {
                        None
                    } else {
                        Some(text)
                    },
                    repository: if repository.trim().is_empty() {
                        None
                    } else {
                        Some(repository)
                    },
                    ..rivora::runtime::search::SearchQuery::default()
                };
                let results = caps.search_investigations(query).map_err(err)?;
                if results.is_empty() {
                    println!("No matching Investigations.");
                } else {
                    for r in &results {
                        println!(
                            "  {}  [{}]  {}  (score {:.2})",
                            r.investigation_id, r.status, r.title, r.score
                        );
                        println!("      {}", r.explanation);
                    }
                    let open: String = Input::new()
                        .with_prompt("Open result id (optional)")
                        .allow_empty(true)
                        .interact_text()
                        .map_err(|e| e.to_string())?;
                    if !open.trim().is_empty() {
                        let inv = caps
                            .open_investigation(open.trim().parse().map_err(err)?)
                            .map_err(err)?;
                        investigation_session(&caps, inv)?;
                    }
                }
            }
            4 => {
                let outcomes = caps
                    .recall_prior_outcomes(rivora::runtime::search::OutcomeFilter::default())
                    .map_err(err)?;
                if outcomes.is_empty() {
                    println!("No prior outcomes recorded.");
                } else {
                    for o in outcomes {
                        println!(
                            "  • [{}]  {}  [{}]  {}",
                            o.outcome.disposition.as_str(),
                            o.investigation_id,
                            o.investigation_title,
                            o.outcome.notes
                        );
                    }
                }
            }
            5 => {
                let patterns = caps.detect_patterns("workspace").map_err(err)?;
                if patterns.is_empty() {
                    println!("No patterns detected.");
                } else {
                    for p in patterns {
                        println!(
                            "  • [{}]  {}  ({} investigations)",
                            p.kind.as_str(),
                            p.signature,
                            p.occurrence_count
                        );
                        println!("      {}", p.description);
                    }
                }
            }
            6 => {
                let repository: String = Input::new()
                    .with_prompt("Repository filter (optional)")
                    .allow_empty(true)
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let filter = if repository.trim().is_empty() {
                    None
                } else {
                    Some(repository)
                };
                let trend = caps.summarize_historical_trend(filter).map_err(err)?;
                println!("{}", trend.summary);
                if !trend.top_failure_signatures.is_empty() {
                    println!("Top failure signatures:");
                    for item in trend.top_failure_signatures {
                        println!("  • {} ({})", item.label, item.count);
                    }
                }
            }
            7 => connector_session(&caps)?,
            _ => break,
        }
    }

    // Restore terminal-ish cleanliness.
    println!("Goodbye.");
    Ok(())
}

fn investigation_session(caps: &CapabilityService, mut inv: Investigation) -> Result<(), String> {
    loop {
        inv = caps.open_investigation(inv.id).map_err(err)?;
        println!(
            "\n{} {}  [{}]",
            style("Investigation").bold(),
            inv.id,
            style(inv.status.to_string()).yellow()
        );
        println!("  {}", inv.title);

        let actions = vec![
            "Status overview",
            "Related Investigations",
            "Observe (manual)",
            "Observe local project",
            "Review Observations / Memory",
            "Timeline",
            "Derive Knowledge",
            "Inspect Knowledge",
            "Evaluate",
            "Inspect Evaluations",
            "Verify",
            "Inspect Verification Receipts",
            "Recommend",
            "Inspect Recommendations",
            "Record outcome",
            "Complete Investigation",
            "Reopen Investigation",
            "Find Similar Investigations",
            "Recall Related Evidence",
            "Recalled Context",
            "Assistance (composites & reports)",
            "Improvement Proposals",
            "Learning Outcomes",
            "Execution (v0.6)",
            "Capability Engineering Loop (v0.8)",
            "Back",
        ];
        let choice = Select::new()
            .with_prompt("Action")
            .items(&actions)
            .default(0)
            .interact()
            .map_err(|e| e.to_string())?;

        match choice {
            0 => show_status(caps, inv.id)?,
            1 => relationship_session(caps, inv.id)?,
            2 => {
                let summary: String = Input::new()
                    .with_prompt("Summary")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let (obs, _mem, _) = caps
                    .ingest_observation(
                        inv.id,
                        ObservationKind::UserInput,
                        summary,
                        serde_json::json!({}),
                        "workspace",
                        Utc::now(),
                        None,
                        "workspace",
                    )
                    .map_err(err)?;
                println!("{} Ingested observation {}", style("✓").green(), obs.id);
            }
            3 => {
                let path: String = Input::new()
                    .with_prompt("Project path")
                    .default(".".into())
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let connector = LocalConnector::new(path);
                let observations = connector.observe().map_err(|e| e.to_string())?;
                for obs in observations {
                    let _ = caps
                        .ingest_observation(
                            inv.id,
                            obs.kind,
                            obs.summary,
                            obs.payload,
                            obs.source,
                            obs.observed_at,
                            obs.idempotency_key,
                            "workspace",
                        )
                        .map_err(err)?;
                }
                println!("{} Local observations ingested", style("✓").green());
            }
            4 => {
                let memory = caps.recall_memory(inv.id).map_err(err)?;
                if memory.is_empty() {
                    println!("No memory yet.");
                } else {
                    for m in memory {
                        println!("  • {}  {}", m.recorded_at.to_rfc3339(), m.summary);
                    }
                }
            }
            5 => {
                let timeline = caps.generate_timeline(inv.id).map_err(err)?;
                for e in timeline {
                    println!("  {}  [{}]  {}", e.at.to_rfc3339(), e.source, e.summary);
                }
            }
            6 => {
                let knowledge = caps.derive_knowledge(inv.id, "workspace").map_err(err)?;
                println!(
                    "{} Derived {} knowledge object(s)",
                    style("✓").green(),
                    knowledge.len()
                );
            }
            7 => {
                for k in caps.list_knowledge(inv.id).map_err(err)? {
                    println!("  • {:?}  {}", k.kind, k.summary);
                }
            }
            8 => {
                let evaluations = caps
                    .evaluate_investigation(inv.id, "workspace")
                    .map_err(err)?;
                println!(
                    "{} Produced {} evaluation(s)",
                    style("✓").green(),
                    evaluations.len()
                );
            }
            9 => {
                for e in caps.list_evaluations(inv.id).map_err(err)? {
                    println!(
                        "  • [{:?}/{}] {} — {}",
                        e.assessment_type,
                        e.severity.as_str(),
                        e.summary,
                        e.explanation
                    );
                }
            }
            10 => {
                let receipts = caps.verify_all(inv.id, "workspace").map_err(err)?;
                println!(
                    "{} Produced {} verification receipt(s)",
                    style("✓").green(),
                    receipts.len()
                );
            }
            11 => {
                for r in caps.list_verifications(inv.id).map_err(err)? {
                    println!("  • [{}] {} — {}", r.result.as_str(), r.subject, r.reason);
                }
            }
            12 => {
                let recs = caps
                    .generate_recommendation(inv.id, "workspace")
                    .map_err(err)?;
                for r in recs {
                    println!(
                        "{} Recommendation [{}]: {}",
                        style("✓").green(),
                        r.status.as_str(),
                        r.summary
                    );
                    println!("    {}", r.rationale);
                }
            }
            13 => {
                for r in caps.list_recommendations(inv.id).map_err(err)? {
                    println!(
                        "  • [{}] {} (confidence {:.0}%)",
                        r.status.as_str(),
                        r.summary,
                        r.confidence.value() * 100.0
                    );
                }
            }
            14 => {
                let recs = caps.list_recommendations(inv.id).map_err(err)?;
                let rec_id = recs.first().map(|r| r.id);
                let dispositions = [
                    OutcomeDisposition::Accepted,
                    OutcomeDisposition::Rejected,
                    OutcomeDisposition::Ignored,
                    OutcomeDisposition::Successful,
                    OutcomeDisposition::Unsuccessful,
                ];
                let labels: Vec<&str> = dispositions.iter().map(|d| d.as_str()).collect();
                let idx = Select::new()
                    .with_prompt("Disposition")
                    .items(&labels)
                    .default(0)
                    .interact()
                    .map_err(|e| e.to_string())?;
                let notes: String = Input::new()
                    .with_prompt("Notes")
                    .allow_empty(true)
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let outcome = caps
                    .record_outcome(inv.id, rec_id, dispositions[idx], notes, None, "workspace")
                    .map_err(err)?;
                println!("{} Recorded learning {}", style("✓").green(), outcome.id);
            }
            15 => {
                if Confirm::new()
                    .with_prompt("Complete this Investigation?")
                    .default(false)
                    .interact()
                    .map_err(|e| e.to_string())?
                {
                    inv = caps
                        .complete_investigation(inv.id, Some("completed in workspace".into()))
                        .map_err(err)?;
                    println!("{} Completed", style("✓").green());
                }
            }
            16 => {
                inv = caps
                    .reopen_investigation(inv.id, Some("reopened in workspace".into()))
                    .map_err(err)?;
                println!("{} Reopened ({})", style("✓").green(), inv.status);
            }
            17 => {
                let results = caps
                    .find_similar_investigations(inv.id, Some(10))
                    .map_err(err)?;
                if results.is_empty() {
                    println!("No similar Investigations found.");
                } else {
                    for r in &results {
                        println!(
                            "  • {}  [{}]  {}  (score {:.2})",
                            r.investigation_id, r.status, r.title, r.score
                        );
                        println!("      {}", r.explanation);
                    }
                }
            }
            18 => {
                let recalled = caps.recall_related_evidence(inv.id).map_err(err)?;
                if recalled.is_empty() {
                    println!("No related evidence. Refresh relationships first.");
                } else {
                    for r in &recalled {
                        println!(
                            "  • [{}] from {}",
                            r.relationship_kind.as_str(),
                            r.investigation_id
                        );
                        for e in &r.evidence {
                            println!("      - {}", e.description);
                        }
                    }
                }
            }
            19 => context_session(caps, inv.id)?,
            20 => assistance_session(caps, inv.id)?,
            21 => proposal_session(caps, inv.id)?,
            22 => learning_session(caps, inv.id)?,
            23 => execution_session(caps, inv.id)?,
            24 => lifecycle_session(caps, inv.id)?,
            _ => break,
        }
    }
    Ok(())
}

/// Assisted workflows and engineering assistance (RFC-018 / RFC-019).
fn assistance_session(caps: &CapabilityService, id: InvestigationId) -> Result<(), String> {
    loop {
        println!("\n{}", style("Assistance").bold());
        let actions = vec![
            "List composite intents",
            "Plan investigate workflow",
            "Run Investigate Engineering Problem",
            "Run Assess Deployment Readiness",
            "Run Explain Failure",
            "Generate Hypotheses",
            "Next Verification",
            "Forecast Risks",
            "Root-Cause Guidance",
            "Prioritize Recommendations",
            "Generate Engineering Report",
            "Summarize Investigation",
            "List Workflows",
            "Back",
        ];
        let choice = Select::new()
            .with_prompt("Assistance")
            .items(&actions)
            .default(0)
            .interact()
            .map_err(|e| e.to_string())?;
        match choice {
            0 => {
                for d in caps.list_composite_capabilities() {
                    println!("  • {} — {}", d.id, d.description);
                }
            }
            1 => {
                let wf = caps
                    .plan_workflow(id, "investigate_engineering_problem", "workspace")
                    .map_err(err)?;
                println!(
                    "{} Planned {} ({} steps)",
                    style("✓").green(),
                    wf.id,
                    wf.steps.len()
                );
                for s in &wf.steps {
                    println!("  {}. {}", s.index, s.capability);
                }
            }
            2 => {
                let wf = caps
                    .run_composite(id, "investigate_engineering_problem", "workspace")
                    .map_err(err)?;
                println!(
                    "{} {} status={}",
                    style("✓").green(),
                    wf.intent,
                    wf.status.as_str()
                );
                if let Some(s) = wf.summary {
                    println!("{s}");
                }
            }
            3 => {
                let wf = caps
                    .run_composite(id, "assess_deployment_readiness", "workspace")
                    .map_err(err)?;
                println!(
                    "{} readiness workflow status={}",
                    style("✓").green(),
                    wf.status.as_str()
                );
            }
            4 => {
                let wf = caps
                    .run_composite(id, "explain_failure", "workspace")
                    .map_err(err)?;
                println!(
                    "{} explain-failure status={}",
                    style("✓").green(),
                    wf.status.as_str()
                );
            }
            5 => {
                for h in caps.generate_hypotheses(id, "workspace").map_err(err)? {
                    println!("  {}. [{}] {}", h.rank, h.status.as_str(), h.statement);
                }
            }
            6 => {
                for s in caps
                    .recommend_next_verification(id, "workspace")
                    .map_err(err)?
                {
                    println!("  {}. {} — {}", s.rank, s.claim, s.method);
                }
            }
            7 => {
                let f = caps.forecast_risk(id, "workspace").map_err(err)?;
                println!("{}", f.summary);
                for item in f.items {
                    println!(
                        "  • {} [{}]: {}",
                        item.category.as_str(),
                        item.severity.as_str(),
                        item.mitigation
                    );
                }
            }
            8 => {
                let g = caps
                    .generate_root_cause_guidance(id, "workspace")
                    .map_err(err)?;
                println!("{}", g.guidance);
            }
            9 => {
                for r in caps
                    .prioritize_recommendations(id, "workspace")
                    .map_err(err)?
                {
                    println!("  {}. score={:.3} {}", r.rank, r.score, r.summary);
                }
            }
            10 => {
                let report = caps
                    .generate_engineering_report(id, "workspace")
                    .map_err(err)?;
                println!("{}", report.markdown);
            }
            11 => {
                let s = caps
                    .summarize_investigation_state(id, "workspace")
                    .map_err(err)?;
                println!("{}", s.summary);
            }
            12 => {
                for w in caps.list_workflows(id).map_err(err)? {
                    println!("  {}  {}  [{}]", w.id, w.intent, w.status.as_str());
                }
            }
            _ => break,
        }
    }
    Ok(())
}

/// Focused Improvement Proposal experience (RFC-020).
fn proposal_session(caps: &CapabilityService, id: InvestigationId) -> Result<(), String> {
    loop {
        println!("\n{}", style("Workspace Proposals").bold());
        println!("Proposal only — not applied, not implemented, not verified.");
        let actions = vec![
            "List Proposals",
            "Create explicit Proposal",
            "Inspect Proposal",
            "Submit Draft and mark under review",
            "Accept Proposal",
            "Reject Proposal",
            "Defer Proposal",
            "Withdraw Proposal",
            "Add feedback",
            "Refine Proposal",
            "Revision history",
            "Supersede Proposal",
            "Generate Proposal alternatives",
            "Compare selected Proposals",
            "Rank latest Proposals",
            "Inspect implementation and Verification Plans",
            "Explain generation provenance",
            "Export Proposal as Markdown",
            "Export Proposal as structured JSON",
            "Generate coding-agent handoff",
            "View Proposal portfolio",
            "Trace Proposal evidence",
            "Back",
        ];
        let choice = Select::new()
            .with_prompt("Proposals")
            .items(&actions)
            .default(0)
            .interact()
            .map_err(|e| e.to_string())?;
        match choice {
            0 => {
                let listing = caps.list_improvement_proposals(id).map_err(err)?;
                if listing.proposals.is_empty() {
                    println!("No Improvement Proposals.");
                } else {
                    for proposal in &listing.proposals {
                        println!(
                            "  {}  [{} / {}]  {}  (revision {})",
                            proposal.id,
                            proposal.status.as_str(),
                            proposal.priority.as_str(),
                            proposal.title,
                            proposal.revision_number,
                        );
                    }
                }
                if !listing.diagnostics.is_empty() {
                    println!(
                        "{} {} corrupted Proposal record(s) were isolated.",
                        style("Warning:").yellow(),
                        listing.diagnostics.len()
                    );
                }
            }
            1 => {
                let title: String = Input::new()
                    .with_prompt("Title")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let summary: String = Input::new()
                    .with_prompt("Concise summary")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let rationale: String = Input::new()
                    .with_prompt("Evidence-backed rationale")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let categories = [
                    ProposalCategory::Code,
                    ProposalCategory::Configuration,
                    ProposalCategory::Testing,
                    ProposalCategory::Reliability,
                    ProposalCategory::Performance,
                    ProposalCategory::Security,
                    ProposalCategory::Observability,
                    ProposalCategory::Infrastructure,
                    ProposalCategory::DeveloperExperience,
                    ProposalCategory::Process,
                    ProposalCategory::Documentation,
                ];
                let category_labels: Vec<_> =
                    categories.iter().map(|value| value.as_str()).collect();
                let category = Select::new()
                    .with_prompt("Category")
                    .items(&category_labels)
                    .default(0)
                    .interact()
                    .map_err(|e| e.to_string())?;
                let priorities = [
                    ProposalPriority::Critical,
                    ProposalPriority::High,
                    ProposalPriority::Medium,
                    ProposalPriority::Low,
                    ProposalPriority::Exploratory,
                ];
                let priority_labels: Vec<_> =
                    priorities.iter().map(|value| value.as_str()).collect();
                let priority = Select::new()
                    .with_prompt("Priority")
                    .items(&priority_labels)
                    .default(2)
                    .interact()
                    .map_err(|e| e.to_string())?;
                let proposal = caps
                    .create_improvement_proposal(
                        id,
                        CreateProposalRequest {
                            title,
                            summary,
                            rationale,
                            category: categories[category],
                            priority: priorities[priority],
                            confidence: Confidence::neutral(),
                            supporting_evidence_ids: Vec::new(),
                            contradicting_evidence_ids: Vec::new(),
                            source_recommendation_ids: Vec::new(),
                            affected_components: Vec::new(),
                            affected_resources: Vec::new(),
                        },
                        "workspace",
                    )
                    .map_err(err)?;
                println!("{}", proposal_details(&proposal));
            }
            2 => {
                let proposal = get_workspace_proposal(caps, id)?;
                println!("{}", proposal_details(&proposal));
                println!("Supporting evidence:");
                if proposal.supporting_evidence.is_empty() {
                    println!("  none recorded");
                } else {
                    for evidence in &proposal.supporting_evidence {
                        println!("  • {} [{}]", evidence.object_id, evidence.scope.as_str());
                    }
                }
                println!("Contradicting evidence:");
                if proposal.contradicting_evidence.is_empty() {
                    println!("  none recorded");
                } else {
                    for evidence in &proposal.contradicting_evidence {
                        println!("  • {} [{}]", evidence.object_id, evidence.scope.as_str());
                    }
                }
                println!("Assumptions: {}", proposal.assumptions.join("; "));
                println!("Risks:");
                for risk in &proposal.risks {
                    println!(
                        "  • [{}] {} — {}",
                        risk.severity.as_str(),
                        risk.description,
                        risk.mitigation
                    );
                }
                println!("Implementation outline:");
                for step in &proposal.implementation_outline {
                    println!("  • {step}");
                }
                println!("Test strategy:");
                for test in &proposal.test_strategy {
                    println!("  • {test}");
                }
                println!("Verification claims:");
                for claim in &proposal.verification_plan.claims {
                    println!("  • {claim}");
                }
            }
            3 => transition_workspace_proposal(
                caps,
                id,
                ProposalStatus::UnderReview,
                "Reason for starting review",
                false,
            )?,
            4 => transition_workspace_proposal(
                caps,
                id,
                ProposalStatus::Accepted,
                "Reason for acceptance",
                true,
            )?,
            5 => transition_workspace_proposal(
                caps,
                id,
                ProposalStatus::Rejected,
                "Reason for rejection",
                false,
            )?,
            6 => transition_workspace_proposal(
                caps,
                id,
                ProposalStatus::Deferred,
                "Reason for deferral",
                false,
            )?,
            7 => transition_workspace_proposal(
                caps,
                id,
                ProposalStatus::Withdrawn,
                "Reason for withdrawal",
                false,
            )?,
            8 => {
                let proposal_id = input_object_id("Proposal id")?;
                let categories = [
                    ProposalFeedbackCategory::TooBroad,
                    ProposalFeedbackCategory::TooRisky,
                    ProposalFeedbackCategory::TooExpensive,
                    ProposalFeedbackCategory::InsufficientEvidence,
                    ProposalFeedbackCategory::WrongComponent,
                    ProposalFeedbackCategory::MissingAlternative,
                    ProposalFeedbackCategory::MissingTest,
                    ProposalFeedbackCategory::ViolatesArchitecture,
                    ProposalFeedbackCategory::ShouldSplit,
                    ProposalFeedbackCategory::ShouldCombine,
                    ProposalFeedbackCategory::NeedsVerification,
                    ProposalFeedbackCategory::Other,
                ];
                let labels: Vec<_> = categories.iter().map(|value| value.as_str()).collect();
                let category = Select::new()
                    .with_prompt("Feedback category")
                    .items(&labels)
                    .default(0)
                    .interact()
                    .map_err(|e| e.to_string())?;
                let comment: String = Input::new()
                    .with_prompt("Feedback")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let proposal = caps
                    .add_improvement_proposal_feedback(
                        id,
                        proposal_id,
                        categories[category],
                        comment,
                        "workspace",
                    )
                    .map_err(err)?;
                println!("{}", proposal_details(&proposal));
            }
            9 => {
                let proposal_id = input_object_id("Proposal id")?;
                let title = optional_input("Replacement title (optional)")?;
                let summary = optional_input("Replacement summary (optional)")?;
                let rationale = optional_input("Replacement rationale (optional)")?;
                let components = optional_csv_input("Affected components (comma-separated)")?;
                let tests = optional_csv_input("Test strategy (comma-separated)")?;
                let reason: String = Input::new()
                    .with_prompt("Reason for refinement")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let proposal = caps
                    .refine_improvement_proposal(
                        id,
                        proposal_id,
                        RefineProposalRequest {
                            title,
                            summary,
                            rationale,
                            affected_components: components,
                            test_strategy: tests,
                        },
                        "workspace",
                        reason,
                    )
                    .map_err(err)?;
                println!("{}", proposal_details(&proposal));
            }
            10 => {
                let lineage_id = input_object_id("Proposal lineage id")?;
                let listing = caps
                    .list_improvement_proposal_revisions(id, lineage_id)
                    .map_err(err)?;
                for proposal in listing.proposals {
                    println!(
                        "  revision {}  {}  [{}]  {}",
                        proposal.revision_number,
                        proposal.id,
                        proposal.status.as_str(),
                        proposal.title,
                    );
                }
            }
            11 => {
                let proposal_id = input_object_id("Proposal id")?;
                let replacement_id = input_object_id("Replacement Proposal id")?;
                let reason: String = Input::new()
                    .with_prompt("Reason for supersession")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let proposal = caps
                    .supersede_improvement_proposal(
                        id,
                        proposal_id,
                        replacement_id,
                        "workspace",
                        reason,
                    )
                    .map_err(err)?;
                println!("{}", proposal_details(&proposal));
            }
            12 => {
                let proposals = caps
                    .generate_proposal_alternatives(id, "workspace")
                    .map_err(err)?;
                println!("Workspace Proposal alternatives: {}", proposals.len());
                for proposal in proposals {
                    println!(
                        "  {}  [{} / {}]  {}\n    {}",
                        proposal.id,
                        proposal.status.as_str(),
                        proposal.priority.as_str(),
                        proposal.title,
                        proposal.summary,
                    );
                }
                println!(
                    "Alternatives are uncertain candidates; none is guaranteed correct or selected for implementation."
                );
                println!("Proposal only — not applied, not implemented, not verified.");
            }
            13 => {
                let proposal_ids: String = Input::new()
                    .with_prompt("Proposal ids to compare (comma-separated, at least two)")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let proposal_ids = proposal_ids
                    .split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|value| value.parse().map_err(err))
                    .collect::<Result<Vec<ObjectId>, String>>()?;
                let comparison = caps
                    .compare_improvement_proposals(id, proposal_ids)
                    .map_err(err)?;
                print_workspace_comparison(&comparison);
            }
            14 => {
                let comparison = caps.prioritize_improvement_proposals(id).map_err(err)?;
                print_workspace_comparison(&comparison);
            }
            15 => {
                let proposal_id = input_object_id("Proposal id")?;
                let outline = caps
                    .generate_proposal_implementation_outline(id, proposal_id)
                    .map_err(err)?;
                let plan = caps
                    .generate_proposal_verification_plan(id, proposal_id)
                    .map_err(err)?;
                println!("Expected implementation scope:");
                print_workspace_lines(&outline);
                println!("Verification claims:");
                print_workspace_lines(&plan.claims);
                println!("Verification tests and fixtures:");
                print_workspace_lines(&plan.tests);
                println!("Verification checks:");
                print_workspace_lines(&plan.checks);
                println!("Success criteria:");
                print_workspace_lines(&plan.success_criteria);
                println!("Failure criteria:");
                print_workspace_lines(&plan.failure_criteria);
                println!("Verification Plan is proposed work; it was not executed.");
                println!("Proposal only — not applied, not implemented, not verified.");
            }
            16 => {
                let proposal_id = input_object_id("Proposal id")?;
                let explanation = caps
                    .explain_improvement_proposal_provenance(id, proposal_id)
                    .map_err(err)?;
                println!("{explanation}");
            }
            17 => {
                let proposal_id = input_object_id("Proposal id")?;
                let artifact = caps
                    .generate_proposal_artifact(id, proposal_id, "workspace")
                    .map_err(err)?;
                println!("{}", artifact.markdown);
            }
            18 => {
                let proposal_id = input_object_id("Proposal id")?;
                let artifact = caps
                    .generate_proposal_artifact(id, proposal_id, "workspace")
                    .map_err(err)?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&artifact).map_err(|error| error.to_string())?
                );
            }
            19 => {
                let proposal_id = input_object_id("Proposal id")?;
                let handoff = caps
                    .generate_coding_agent_handoff(id, proposal_id)
                    .map_err(err)?;
                println!("{handoff}");
            }
            20 => {
                let proposals = caps
                    .proposal_portfolio(id, ProposalPortfolioFilter::default())
                    .map_err(err)?;
                if proposals.is_empty() {
                    println!("No matching Improvement Proposals.");
                } else {
                    for proposal in proposals {
                        println!(
                            "  {}  [{} / {} / {}]  {}  (revision {})",
                            proposal.id,
                            proposal.status.as_str(),
                            proposal.priority.as_str(),
                            proposal.category.as_str(),
                            proposal.title,
                            proposal.revision_number,
                        );
                    }
                }
                println!("Proposal only — not applied, not implemented, not verified.");
            }
            21 => {
                let proposal_id = input_object_id("Proposal id")?;
                let trace = caps
                    .trace_improvement_proposal(id, proposal_id)
                    .map_err(err)?;
                println!(
                    "Observation ({}) → Memory ({}) → Knowledge ({}) → Evaluation ({}) → Verification ({}) → Recommendation ({}) → Improvement Proposal {}",
                    trace.observation_ids.len(),
                    trace.memory_ids.len(),
                    trace.knowledge_ids.len(),
                    trace.evaluation_ids.len(),
                    trace.verification_ids.len(),
                    trace.recommendation_ids.len(),
                    trace.proposal_id,
                );
                println!("{}", trace.explanation);
                println!("Proposal only — not applied, not implemented, not verified.");
            }
            _ => break,
        }
    }
    Ok(())
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

fn print_workspace_lines(lines: &[String]) {
    if lines.is_empty() {
        println!("  none specified");
    } else {
        for line in lines {
            println!("  • {line}");
        }
    }
}

fn input_object_id(prompt: &str) -> Result<ObjectId, String> {
    let value: String = Input::new()
        .with_prompt(prompt)
        .interact_text()
        .map_err(|e| e.to_string())?;
    value.parse().map_err(err)
}

fn get_workspace_proposal(
    caps: &CapabilityService,
    investigation_id: InvestigationId,
) -> Result<ImprovementProposal, String> {
    let proposal_id = input_object_id("Proposal id")?;
    caps.get_improvement_proposal(investigation_id, proposal_id)
        .map_err(err)
}

fn transition_workspace_proposal(
    caps: &CapabilityService,
    investigation_id: InvestigationId,
    status: ProposalStatus,
    prompt: &str,
    confirm_acceptance: bool,
) -> Result<(), String> {
    let proposal_id = input_object_id("Proposal id")?;
    if confirm_acceptance
        && !Confirm::new()
            .with_prompt(
                "Accept this Proposal for possible later implementation? This does not apply it.",
            )
            .default(false)
            .interact()
            .map_err(|e| e.to_string())?
    {
        println!("Acceptance cancelled.");
        return Ok(());
    }
    let reason: String = Input::new()
        .with_prompt(prompt)
        .interact_text()
        .map_err(|e| e.to_string())?;
    let current = caps
        .get_improvement_proposal(investigation_id, proposal_id)
        .map_err(err)?;
    let proposal_id =
        if status == ProposalStatus::UnderReview && current.status == ProposalStatus::Draft {
            let proposed = caps
                .update_improvement_proposal_status(
                    investigation_id,
                    proposal_id,
                    ProposalStatus::Proposed,
                    "workspace",
                    "explicitly submit Draft for review",
                    ProposalTransitionAuthority::ExternalCaller,
                )
                .map_err(err)?;
            println!("Draft explicitly submitted as Proposed.");
            proposed.id
        } else {
            proposal_id
        };
    let proposal = caps
        .update_improvement_proposal_status(
            investigation_id,
            proposal_id,
            status,
            "workspace",
            reason,
            ProposalTransitionAuthority::ExternalCaller,
        )
        .map_err(err)?;
    println!("{}", proposal_details(&proposal));
    Ok(())
}

fn optional_input(prompt: &str) -> Result<Option<String>, String> {
    let value: String = Input::new()
        .with_prompt(prompt)
        .allow_empty(true)
        .interact_text()
        .map_err(|e| e.to_string())?;
    Ok((!value.trim().is_empty()).then_some(value))
}

fn optional_csv_input(prompt: &str) -> Result<Option<Vec<String>>, String> {
    Ok(optional_input(prompt)?.map(|value| {
        value
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect()
    }))
}

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

/// Connector status and fixture ingest (read-only).
fn connector_session(caps: &CapabilityService) -> Result<(), String> {
    loop {
        println!("\n{}", style("Connectors").bold());
        let actions = vec![
            "List connector status",
            "Test GitHub Actions config",
            "Test Kubernetes config",
            "Test Sentry config",
            "Back",
        ];
        let choice = Select::new()
            .with_prompt("Connectors")
            .items(&actions)
            .default(0)
            .interact()
            .map_err(|e| e.to_string())?;
        match choice {
            0 => {
                for c in [
                    GitHubActionsConnector::new("owner/repo").status(),
                    KubernetesConnector::new("default").status(),
                    SentryConnector::new("org", "project").status(),
                ] {
                    println!(
                        "  {} [{}] configured={} read_only={} — {}",
                        c.id, c.category, c.configured, c.read_only, c.details
                    );
                }
            }
            1 => {
                let msg = GitHubActionsConnector::new("owner/repo")
                    .test_configuration()
                    .map_err(|e| e.to_string())?;
                println!("{} {msg}", style("✓").green());
            }
            2 => {
                let msg = KubernetesConnector::new("default")
                    .test_configuration()
                    .map_err(|e| e.to_string())?;
                println!("{} {msg}", style("✓").green());
            }
            3 => {
                let msg = SentryConnector::new("org", "project")
                    .test_configuration()
                    .map_err(|e| e.to_string())?;
                println!("{} {msg}", style("✓").green());
            }
            _ => break,
        }
    }
    // Keep caps referenced for future ingest UI; suppress unused warning.
    let _ = caps.list_investigations().map_err(err)?;
    Ok(())
}

/// Recalled Context sub-loop: suggest, attach, dismiss, and inspect
/// historical context for the current Investigation (RFC-017).
fn context_session(caps: &CapabilityService, id: InvestigationId) -> Result<(), String> {
    loop {
        let contexts = caps.list_recalled_context(id).map_err(err)?;
        println!("\n{}", style("Recalled Context").bold());
        if contexts.is_empty() {
            println!("  No recalled context yet.");
        } else {
            for c in &contexts {
                println!(
                    "  • [{}]  {}  from {}  ({})",
                    c.state.as_str(),
                    c.id,
                    c.source_investigation_id,
                    c.origin.as_str()
                );
                println!("      reason: {}", c.reason);
                println!("      {}", c.evidence_summary);
            }
        }

        let actions = vec![
            "Suggest from related / similar",
            "Attach source Investigation",
            "Attach suggested context",
            "Dismiss context",
            "Open source Investigation",
            "Back",
        ];
        let choice = Select::new()
            .with_prompt("Context")
            .items(&actions)
            .default(0)
            .interact()
            .map_err(|e| e.to_string())?;

        match choice {
            0 => {
                let suggested = caps
                    .suggest_recalled_context(id, "workspace")
                    .map_err(err)?;
                println!(
                    "{} {} context record(s)",
                    style("✓").green(),
                    suggested.len()
                );
            }
            1 => {
                let source: String = Input::new()
                    .with_prompt("Source Investigation id")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let reason: String = Input::new()
                    .with_prompt("Reason (optional)")
                    .allow_empty(true)
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let reason = if reason.trim().is_empty() {
                    None
                } else {
                    Some(reason)
                };
                let context = caps
                    .attach_recalled_context_from_source(
                        id,
                        source.parse().map_err(err)?,
                        reason,
                        "workspace",
                    )
                    .map_err(err)?;
                println!(
                    "{} Attached {} (historical; not current fact)",
                    style("✓").green(),
                    context.id
                );
            }
            2 => {
                let context_id: String = Input::new()
                    .with_prompt("Recalled Context id")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let context = caps
                    .attach_recalled_context(id, context_id.parse().map_err(err)?, "workspace")
                    .map_err(err)?;
                println!("{} Attached {}", style("✓").green(), context.id);
            }
            3 => {
                let context_id: String = Input::new()
                    .with_prompt("Recalled Context id")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let context = caps
                    .dismiss_recalled_context(id, context_id.parse().map_err(err)?, "workspace")
                    .map_err(err)?;
                println!("{} Dismissed {}", style("✓").green(), context.id);
            }
            4 => {
                let source: String = Input::new()
                    .with_prompt("Source Investigation id")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let inv = caps
                    .open_investigation(source.parse().map_err(err)?)
                    .map_err(err)?;
                investigation_session(caps, inv)?;
            }
            _ => break,
        }
    }
    Ok(())
}

/// Related-Investigations sub-loop: list, explain, link, and curate
/// relationships for the current Investigation (RFC-015).
fn relationship_session(caps: &CapabilityService, id: InvestigationId) -> Result<(), String> {
    loop {
        let related = caps.list_related_investigations(id).map_err(err)?;
        println!("\n{}", style("Related Investigations").bold());
        if related.is_empty() {
            println!("  No related Investigations.");
        } else {
            for r in &related {
                println!(
                    "  • [{}]  {}  [{}]  {}  ({:.0}%, {})",
                    r.relationship.kind.as_str(),
                    r.related.id,
                    r.related.status,
                    r.related.title,
                    r.relationship.confidence.value() * 100.0,
                    r.relationship.confirmation.state.as_str()
                );
                println!("      relationship {}", r.relationship.id);
            }
        }

        let actions = vec![
            "Refresh relationships",
            "Link Investigation",
            "Explain relationship",
            "Confirm relationship",
            "Dismiss relationship",
            "Unlink explicit link",
            "Open related Investigation",
            "Back",
        ];
        let choice = Select::new()
            .with_prompt("Relationships")
            .items(&actions)
            .default(0)
            .interact()
            .map_err(|e| e.to_string())?;

        match choice {
            0 => {
                let relationships = caps.refresh_relationships(id, "workspace").map_err(err)?;
                println!(
                    "{} {} relationship(s)",
                    style("✓").green(),
                    relationships.len()
                );
            }
            1 => {
                let target: String = Input::new()
                    .with_prompt("Target Investigation id")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let reason: String = Input::new()
                    .with_prompt("Reason (optional)")
                    .allow_empty(true)
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let reason = if reason.trim().is_empty() {
                    None
                } else {
                    Some(reason)
                };
                let relationship = caps
                    .link_investigations(id, target.parse().map_err(err)?, reason, "workspace")
                    .map_err(err)?;
                println!("{} Linked ({})", style("✓").green(), relationship.id);
            }
            2 => {
                let rel: String = Input::new()
                    .with_prompt("Relationship id")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let explanation = caps
                    .explain_relationship(rel.parse().map_err(err)?)
                    .map_err(err)?;
                println!("{}", explanation.explanation);
            }
            3 => {
                let rel: String = Input::new()
                    .with_prompt("Relationship id")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let relationship = caps
                    .confirm_relationship(rel.parse().map_err(err)?, "workspace")
                    .map_err(err)?;
                println!("{} Confirmed {}", style("✓").green(), relationship.id);
            }
            4 => {
                let rel: String = Input::new()
                    .with_prompt("Relationship id")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let relationship = caps
                    .dismiss_relationship(rel.parse().map_err(err)?, "workspace")
                    .map_err(err)?;
                println!("{} Dismissed {}", style("✓").green(), relationship.id);
            }
            5 => {
                let rel: String = Input::new()
                    .with_prompt("Relationship id")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                caps.unlink_investigation(rel.parse().map_err(err)?, "workspace")
                    .map_err(err)?;
                println!("{} Unlinked", style("✓").green());
            }
            6 => {
                let target: String = Input::new()
                    .with_prompt("Investigation id")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let inv = caps
                    .open_investigation(target.parse().map_err(err)?)
                    .map_err(err)?;
                investigation_session(caps, inv)?;
            }
            _ => break,
        }
    }
    Ok(())
}

fn show_status(caps: &CapabilityService, id: InvestigationId) -> Result<(), String> {
    let inv = caps.open_investigation(id).map_err(err)?;
    println!("Status: {}", inv.status);
    println!("Memory: {}", caps.recall_memory(id).map_err(err)?.len());
    println!("Knowledge: {}", caps.list_knowledge(id).map_err(err)?.len());
    println!(
        "Evaluations: {}",
        caps.list_evaluations(id).map_err(err)?.len()
    );
    println!(
        "Verifications: {}",
        caps.list_verifications(id).map_err(err)?.len()
    );
    println!(
        "Recommendations: {}",
        caps.list_recommendations(id).map_err(err)?.len()
    );
    println!("Learning: {}", caps.list_learning(id).map_err(err)?.len());
    println!(
        "Recalled context: {}",
        caps.list_recalled_context(id).map_err(err)?.len()
    );
    println!(
        "Execution plans: {}",
        caps.list_execution_plans(id).map_err(err)?.plans.len()
    );
    let lifecycle = caps.list_lifecycle_runs(id).map_err(err)?;
    println!("Engineering Loop runs: {}", lifecycle.runs.len());
    if let Some(latest) = lifecycle.runs.last() {
        println!(
            "  latest loop: {} [{}] {}",
            latest.id,
            latest.status.as_str(),
            latest.capability_id
        );
    }
    Ok(())
}

/// Non-interactive end-to-end workflow for CI/smoke tests.
fn smoke_workflow(caps: &CapabilityService) -> Result<(), String> {
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

const LEARNING_BOUNDARY: &str = "Measured Learning Outcome — external implementation recorded, never auto-applied; verified only with explicit actor+reason.";

/// Focused Measured Learning Outcome experience (RFC-022/023/024).
fn learning_session(caps: &CapabilityService, id: InvestigationId) -> Result<(), String> {
    loop {
        println!("\n{}", style("Workspace Learning Outcomes").bold());
        println!("{LEARNING_BOUNDARY}");
        let actions = vec![
            "List Measured Learning Outcomes",
            "Record Implementation",
            "Create Measured Outcome",
            "Add Outcome Evidence",
            "Evaluate Outcome",
            "Show Outcome Detail",
            "Verify Outcome",
            "Trace Outcome",
            "Derive Learning Patterns",
            "List Learning Patterns",
            "Show Learning Pattern",
            "Explain Historical Influence",
            "Export Outcome Markdown",
            "Back",
        ];
        let choice = Select::new()
            .with_prompt("Learning Outcomes")
            .items(&actions)
            .default(0)
            .interact()
            .map_err(|e| e.to_string())?;
        match choice {
            0 => {
                let listing = caps.list_measured_learning_outcomes(id).map_err(err)?;
                if listing.outcomes.is_empty() {
                    println!("No Measured Learning Outcomes.");
                } else {
                    for outcome in &listing.outcomes {
                        println!(
                            "  {}  [{} / {}]  proposal {}  impl {}  (revision {})",
                            outcome.id,
                            outcome.status.as_str(),
                            outcome.classification.as_str(),
                            outcome.proposal_id,
                            outcome.implementation_record_id,
                            outcome.revision_number,
                        );
                    }
                }
            }
            1 => {
                let proposal = input_object_id("Proposal id")?;
                let summary: String = Input::new()
                    .with_prompt("Implementation summary")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let commit: String = Input::new()
                    .with_prompt("Commit SHA (optional)")
                    .allow_empty(true)
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let mut references = Vec::new();
                if !commit.trim().is_empty() {
                    references.push(ImplementationReference::CommitSha {
                        sha: commit.trim().into(),
                    });
                }
                let record = caps
                    .record_external_implementation(
                        id,
                        proposal,
                        RecordImplementationRequest {
                            source: if references.is_empty() {
                                ImplementationSource::HumanDeclared
                            } else {
                                ImplementationSource::GitCommit
                            },
                            summary,
                            references,
                            implemented_at: None,
                            observed_files: Vec::new(),
                            observed_components: Vec::new(),
                            declared_scope: String::new(),
                        },
                        "workspace",
                    )
                    .map_err(err)?;
                println!(
                    "{} Implementation {} [{}]",
                    style("✓").green(),
                    record.id,
                    record.status.as_str()
                );
                println!("{LEARNING_BOUNDARY}");
            }
            2 => {
                let proposal = input_object_id("Proposal id")?;
                let implementation = input_object_id("Implementation id")?;
                let outcome = caps
                    .create_measured_learning_outcome(id, proposal, implementation, "workspace")
                    .map_err(err)?;
                println!(
                    "{} Measured Outcome {} [{} / {}]",
                    style("✓").green(),
                    outcome.id,
                    outcome.status.as_str(),
                    outcome.classification.as_str()
                );
                println!("{LEARNING_BOUNDARY}");
            }
            3 => {
                let outcome = input_object_id("Outcome id")?;
                let evidence = input_object_id("Evidence object id")?;
                let relations = [
                    OutcomeEvidenceRelation::IsBaseline,
                    OutcomeEvidenceRelation::IsPostChange,
                    OutcomeEvidenceRelation::SupportsExpectedResult,
                    OutcomeEvidenceRelation::ContradictsExpectedResult,
                    OutcomeEvidenceRelation::IndicatesRegression,
                    OutcomeEvidenceRelation::ConfirmsImplementation,
                    OutcomeEvidenceRelation::IsInconclusive,
                ];
                let labels: Vec<_> = relations.iter().map(|r| r.as_str()).collect();
                let idx = Select::new()
                    .with_prompt("Evidence relation")
                    .items(&labels)
                    .default(0)
                    .interact()
                    .map_err(|e| e.to_string())?;
                let expected: String = Input::new()
                    .with_prompt("Expected result id (optional)")
                    .allow_empty(true)
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let reason: String = Input::new()
                    .with_prompt("Reason (optional)")
                    .allow_empty(true)
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let expected_result_id = if expected.trim().is_empty() {
                    None
                } else {
                    Some(expected.trim().parse().map_err(err)?)
                };
                let updated = caps
                    .collect_outcome_evidence(
                        id,
                        outcome,
                        CollectOutcomeEvidenceRequest {
                            object_id: evidence,
                            relation: relations[idx],
                            expected_result_id,
                            reason: (!reason.trim().is_empty()).then_some(reason),
                        },
                        "workspace",
                    )
                    .map_err(err)?;
                println!(
                    "{} Outcome {} now [{}] with {} evidence link(s)",
                    style("✓").green(),
                    updated.id,
                    updated.status.as_str(),
                    updated.evidence_links.len()
                );
            }
            4 => {
                let outcome = input_object_id("Outcome id")?;
                let evaluated = caps
                    .evaluate_measured_learning_outcome(id, outcome, "workspace")
                    .map_err(err)?;
                println!(
                    "{} Evaluated {} as {} (confidence {:.0}%)",
                    style("✓").green(),
                    evaluated.id,
                    evaluated.classification.as_str(),
                    evaluated.confidence.value() * 100.0
                );
                if let Some(report) = &evaluated.evaluation_report {
                    println!(
                        "  verification_ready={} method={}",
                        report.verification_ready, report.method
                    );
                }
                println!("{LEARNING_BOUNDARY}");
            }
            5 => {
                let outcome_id = input_object_id("Outcome id")?;
                let outcome = caps
                    .get_measured_learning_outcome(id, outcome_id)
                    .map_err(err)?;
                println!(
                    "Measured Outcome {} revision {} [{} / {}]",
                    outcome.id,
                    outcome.revision_number,
                    outcome.status.as_str(),
                    outcome.classification.as_str()
                );
                println!("  proposal: {}", outcome.proposal_id);
                println!("  implementation: {}", outcome.implementation_record_id);
                println!(
                    "  confidence: {:.0}%  historical learning eligible: {}",
                    outcome.confidence.value() * 100.0,
                    outcome.historical_learning_eligible
                );
                println!("  expected results: {}", outcome.expected_results.len());
                for expected in &outcome.expected_results {
                    println!(
                        "    • {} [{}]",
                        expected.description,
                        expected.kind.as_str()
                    );
                }
                println!("  assessments: {}", outcome.assessments.len());
                for assessment in &outcome.assessments {
                    println!(
                        "    • {} → {}",
                        assessment.expected_result_id,
                        assessment.kind.as_str()
                    );
                }
                println!("  regressions: {}", outcome.regressions.len());
                for regression in &outcome.regressions {
                    println!(
                        "    • [{}] {}",
                        regression.severity.as_str(),
                        regression.description
                    );
                }
                println!("{LEARNING_BOUNDARY}");
            }
            6 => {
                let outcome = input_object_id("Outcome id")?;
                let actor: String = Input::new()
                    .with_prompt("Actor (required)")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let reason: String = Input::new()
                    .with_prompt("Reason (required)")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                if actor.trim().is_empty() || reason.trim().is_empty() {
                    println!(
                        "{} Verification requires explicit non-empty actor and reason.",
                        style("!").yellow()
                    );
                    continue;
                }
                let verified = caps
                    .verify_measured_learning_outcome(
                        id,
                        outcome,
                        actor.trim(),
                        reason.trim(),
                        false,
                        None,
                    )
                    .map_err(err)?;
                println!(
                    "{} Verified {} as {}",
                    style("✓").green(),
                    verified.id,
                    verified.classification.as_str()
                );
                println!("{LEARNING_BOUNDARY}");
            }
            7 => {
                let outcome = input_object_id("Outcome id")?;
                let trace = caps
                    .trace_measured_learning_outcome(id, outcome)
                    .map_err(err)?;
                println!(
                    "Proposal {} → Implementation {} → Measured Outcome {}",
                    trace.proposal_id, trace.implementation_record_id, trace.outcome_id
                );
                println!("  classification: {}", trace.classification.as_str());
                println!("  status: {}", trace.status.as_str());
                println!("  {}", trace.explanation);
                println!("{LEARNING_BOUNDARY}");
            }
            8 => {
                let patterns = caps.derive_learning_patterns("workspace").map_err(err)?;
                if patterns.is_empty() {
                    println!("No Learning Patterns derived.");
                } else {
                    for pattern in &patterns {
                        println!(
                            "  {}  [{}]  {}  (confidence {:.0}%)",
                            pattern.id,
                            pattern.status.as_str(),
                            pattern.signature,
                            pattern.confidence.value() * 100.0
                        );
                    }
                }
                println!("{LEARNING_BOUNDARY}");
            }
            9 => {
                let patterns = caps.list_learning_patterns().map_err(err)?;
                if patterns.is_empty() {
                    println!("No Learning Patterns.");
                } else {
                    for pattern in &patterns {
                        println!(
                            "  {}  [{}]  {}",
                            pattern.id,
                            pattern.status.as_str(),
                            pattern.title
                        );
                    }
                }
            }
            10 => {
                let pattern_id = input_object_id("Pattern id")?;
                let pattern = caps.get_learning_pattern(pattern_id).map_err(err)?;
                println!("Pattern {} [{}]", pattern.id, pattern.status.as_str());
                println!("  title: {}", pattern.title);
                println!("  signature: {}", pattern.signature);
                println!("  confidence: {:.0}%", pattern.confidence.value() * 100.0);
                println!(
                    "  supporting: {}  contradicting: {}",
                    pattern.supporting_outcome_ids.len(),
                    pattern.contradicting_outcome_ids.len()
                );
                println!("{LEARNING_BOUNDARY}");
            }
            11 => {
                let proposal = input_object_id("Proposal id")?;
                let influence = caps
                    .explain_historical_influence(id, proposal)
                    .map_err(err)?;
                println!("{}", influence.explanation);
                for item in &influence.patterns_considered {
                    println!(
                        "  • pattern {}  {}  magnitude={:.3}",
                        item.pattern_id, item.direction, item.magnitude
                    );
                }
                println!("{LEARNING_BOUNDARY}");
            }
            12 => {
                let outcome = input_object_id("Outcome id")?;
                let markdown = caps
                    .export_measured_learning_outcome_markdown(id, outcome)
                    .map_err(err)?;
                println!("{markdown}");
            }
            _ => break,
        }
    }
    Ok(())
}

/// Controlled external execution surface (RFC-025/026/027).
fn execution_session(caps: &CapabilityService, id: InvestigationId) -> Result<(), String> {
    loop {
        println!("\n{}", style("Execution (v0.6)").bold());
        println!("{EXECUTION_BOUNDARY}");
        let actions = vec![
            "List execution capabilities",
            "List execution plans",
            "Create plan from accepted proposal (mock)",
            "Validate plan",
            "Preview plan (dry-run)",
            "Approve plan",
            "Run plan (dry-run)",
            "Run plan (live, requires confirm)",
            "List attempts",
            "Verify attempt",
            "Trace plan",
            "Explain policy",
            "List plan revisions",
            "Cancel plan",
            "Create rollback plan",
            "List receipts",
            "Export receipt JSON",
            "Run Engineering Loop for attempt",
            "Back",
        ];
        let choice = Select::new()
            .with_prompt("Execution")
            .items(&actions)
            .default(0)
            .interact()
            .map_err(|e| e.to_string())?;
        match choice {
            0 => {
                for c in caps.list_execution_capabilities() {
                    println!(
                        "  • {} [{}] dry_run={}  loop M/E/V/I/L={}/{}/{}/{}/{}",
                        c.capability_id,
                        c.risk_level.as_str(),
                        c.supports_dry_run,
                        c.engineering_loop.memory.as_str(),
                        c.engineering_loop.evaluation.as_str(),
                        c.engineering_loop.verification.as_str(),
                        c.engineering_loop.improvement.as_str(),
                        c.engineering_loop.learning.as_str(),
                    );
                }
            }
            1 => {
                let listing = caps.list_execution_plans(id).map_err(err)?;
                if listing.plans.is_empty() {
                    println!("No execution plans.");
                }
                for p in listing.plans {
                    println!(
                        "  • {} rev {} [{}] {}",
                        p.id,
                        p.revision_number,
                        p.status.as_str(),
                        p.capability_id
                    );
                }
                for diagnostic in listing.diagnostics {
                    println!(
                        "  {} corrupt plan isolated: {} ({})",
                        style("!").yellow(),
                        diagnostic.path,
                        diagnostic.error
                    );
                }
            }
            2 => {
                let proposal: String = Input::new()
                    .with_prompt("Accepted proposal id")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let value: String = Input::new()
                    .with_prompt("Mock field value")
                    .default("workspace".into())
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let plan = caps
                    .create_execution_plan(
                        id,
                        CreateExecutionPlanRequest {
                            proposal_id: proposal.parse().map_err(err)?,
                            capability_id: "mock.record".into(),
                            target_system: "mock".into(),
                            target_environment: "sandbox".into(),
                            actions: vec![ExecutionAction {
                                action_id: "a1".into(),
                                action_name: "record_mutation".into(),
                                inputs: serde_json::json!({
                                    "resource_key": "workspace/demo",
                                    "field": "label",
                                    "value": value
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
                println!(
                    "Created plan {} (draft). Validate and approve before live run.",
                    plan.id
                );
            }
            3 => {
                let plan: String = Input::new()
                    .with_prompt("Plan id")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let reason: String = Input::new()
                    .with_prompt("Reason")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let plan = caps
                    .validate_execution_plan(id, plan.parse().map_err(err)?, "workspace", reason)
                    .map_err(err)?;
                println!("Plan {} → {}", plan.id, plan.status.as_str());
            }
            4 => {
                let plan: String = Input::new()
                    .with_prompt("Plan id")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let preview = caps
                    .preview_execution_plan(id, plan.parse().map_err(err)?)
                    .map_err(err)?;
                println!(
                    "Preview: {} | policy={} | simulated={}",
                    preview.target,
                    preview.policy_decision.decision.as_str(),
                    preview.simulated
                );
            }
            5 => {
                let plan: String = Input::new()
                    .with_prompt("Plan id")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let reason: String = Input::new()
                    .with_prompt("Approval reason")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let (plan, approval) = caps
                    .approve_execution_plan(
                        id,
                        plan.parse().map_err(err)?,
                        "workspace",
                        reason,
                        vec![],
                        vec![],
                        None,
                        true,
                    )
                    .map_err(err)?;
                println!(
                    "Approved plan {} rev {} with approval {}",
                    plan.id, plan.revision_number, approval.id
                );
            }
            6 => {
                let plan: String = Input::new()
                    .with_prompt("Plan id")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let approval: String = Input::new()
                    .with_prompt("Approval id")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let key: String = Input::new()
                    .with_prompt("Idempotency key")
                    .default(format!("ws-dry-{}", chrono::Utc::now().timestamp()))
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let attempt = caps
                    .execute_plan(
                        id,
                        plan.parse().map_err(err)?,
                        approval.parse().map_err(err)?,
                        "workspace",
                        key,
                        true,
                    )
                    .map_err(err)?;
                println!(
                    "Dry-run attempt {} [{}]",
                    attempt.id,
                    attempt.status.as_str()
                );
            }
            7 => {
                let plan: String = Input::new()
                    .with_prompt("Plan id")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let approval: String = Input::new()
                    .with_prompt("Approval id")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let key: String = Input::new()
                    .with_prompt("Idempotency key")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let plan_id = plan.parse().map_err(err)?;
                let approval_id = approval.parse().map_err(err)?;
                let snapshot = caps.get_execution_plan(id, plan_id).map_err(err)?;
                let policy = caps.explain_execution_policy(id, plan_id).map_err(err)?;
                println!(
                    "{}",
                    live_execution_confirmation(&snapshot, &policy, approval_id)
                );
                let confirm = Confirm::new()
                    .with_prompt("Execute this exact approved target?")
                    .default(false)
                    .interact()
                    .map_err(|e| e.to_string())?;
                if !confirm {
                    println!("Live execution cancelled before mutation.");
                    continue;
                }
                let attempt = caps
                    .execute_plan(id, plan_id, approval_id, "workspace", key, false)
                    .map_err(err)?;
                println!(
                    "Live attempt {} [{}] completed={:?} failed={:?} skipped={:?} uncertain={:?}",
                    attempt.id,
                    attempt.status.as_str(),
                    attempt.completed_actions,
                    attempt.failed_actions,
                    attempt.skipped_actions,
                    attempt.uncertain_actions
                );
                println!(
                    "  retry_safety={} next={}",
                    attempt.retry_safety.as_str(),
                    attempt.recommended_next_action.as_deref().unwrap_or("-")
                );
            }
            8 => {
                let listing = caps.list_execution_attempts(id).map_err(err)?;
                for a in listing.attempts {
                    println!(
                        "  • {} [{}] plan={} completed={:?} failed={:?} skipped={:?} uncertain={:?} retry={}",
                        a.id,
                        a.status.as_str(),
                        a.plan_id,
                        a.completed_actions,
                        a.failed_actions,
                        a.skipped_actions,
                        a.uncertain_actions,
                        a.retry_safety.as_str()
                    );
                }
                for diagnostic in listing.diagnostics {
                    println!(
                        "  {} corrupt attempt isolated: {} ({})",
                        style("!").yellow(),
                        diagnostic.path,
                        diagnostic.error
                    );
                }
            }
            9 => {
                let attempt: String = Input::new()
                    .with_prompt("Attempt id")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let v = caps
                    .verify_execution_attempt(id, attempt.parse().map_err(err)?, "workspace")
                    .map_err(err)?;
                println!(
                    "Verification {} [{}] contradictions={}",
                    v.id,
                    v.status.as_str(),
                    v.contradictions.len()
                );
            }
            10 => {
                let plan: String = Input::new()
                    .with_prompt("Plan id")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let trace = caps
                    .trace_execution(id, plan.parse().map_err(err)?)
                    .map_err(err)?;
                println!("{}", trace.explanation);
                println!(
                    "approvals={} attempts={} receipts={} verifications={}",
                    trace.approval_ids.len(),
                    trace.attempt_ids.len(),
                    trace.receipt_ids.len(),
                    trace.verification_ids.len()
                );
                println!(
                    "implementation_record={} measured_outcome={}",
                    trace
                        .implementation_record_id
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| "-".into()),
                    trace
                        .measured_outcome_id
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| "-".into())
                );
            }
            11 => {
                let plan: String = Input::new()
                    .with_prompt("Plan id")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let policy = caps
                    .explain_execution_policy(id, plan.parse().map_err(err)?)
                    .map_err(err)?;
                println!(
                    "{} risk={} reasons={}",
                    policy.decision.as_str(),
                    policy.risk_level.as_str(),
                    policy.reasons.join("; ")
                );
            }
            12 => {
                let plan = input_object_id("Any plan snapshot id")?;
                let snapshot = caps.get_execution_plan(id, plan).map_err(err)?;
                let listing = caps
                    .list_execution_plan_revisions(id, snapshot.lineage_id)
                    .map_err(err)?;
                for revision in listing.plans {
                    println!(
                        "  • rev {} {} [{}] target={}:{}",
                        revision.revision_number,
                        revision.id,
                        revision.status.as_str(),
                        revision.target_system,
                        revision.target_environment
                    );
                }
                for diagnostic in listing.diagnostics {
                    println!(
                        "  {} corrupt plan revision isolated: {} ({})",
                        style("!").yellow(),
                        diagnostic.path,
                        diagnostic.error
                    );
                }
            }
            13 => {
                let plan = input_object_id("Plan id")?;
                let reason: String = Input::new()
                    .with_prompt("Cancellation reason")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let cancelled = caps
                    .cancel_execution_plan(id, plan, "workspace", reason)
                    .map_err(err)?;
                println!(
                    "{} Cancelled plan {} rev {} [{}]",
                    style("✓").green(),
                    cancelled.id,
                    cancelled.revision_number,
                    cancelled.status.as_str()
                );
            }
            14 => {
                let attempt = input_object_id("Attempt id")?;
                let rollback = caps
                    .create_rollback_plan(id, attempt, "workspace")
                    .map_err(err)?;
                println!(
                    "{} Rollback plan {} rev {} [{}]",
                    style("✓").green(),
                    rollback.id,
                    rollback.revision_number,
                    rollback.status.as_str()
                );
                println!(
                    "  capability={} target={}:{} actions={}",
                    rollback.capability_id,
                    rollback.target_system,
                    rollback.target_environment,
                    rollback
                        .actions
                        .iter()
                        .map(|action| action.action_name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                println!("  This is a separate draft. Validate and approve it before execution.");
            }
            15 => {
                let listing = caps.list_execution_receipts(id).map_err(err)?;
                if listing.receipts.is_empty() {
                    println!("No execution receipts.");
                }
                for receipt in listing.receipts {
                    println!(
                        "  • {} [{}] attempt={} action={} capability={}",
                        receipt.id,
                        receipt.result_status.as_str(),
                        receipt.attempt_id,
                        receipt.action_name,
                        receipt.capability_id
                    );
                }
                for diagnostic in listing.diagnostics {
                    println!(
                        "  {} corrupt receipt isolated: {} ({})",
                        style("!").yellow(),
                        diagnostic.path,
                        diagnostic.error
                    );
                }
            }
            16 => {
                let receipt = input_object_id("Receipt id")?;
                let json = caps.export_execution_receipt(id, receipt).map_err(err)?;
                println!("{json}");
            }
            17 => {
                let attempt = input_object_id("Attempt id")?;
                let run = caps
                    .run_capability_lifecycle_for_attempt(id, attempt, "workspace")
                    .map_err(err)?;
                print_lifecycle_run(&run);
            }
            _ => break,
        }
    }
    Ok(())
}

/// Capability Engineering Loop inspection and coverage (v0.8 / RFC-028).
fn lifecycle_session(caps: &CapabilityService, id: InvestigationId) -> Result<(), String> {
    loop {
        println!("\n{}", style("Capability Engineering Loop (v0.8)").bold());
        println!(
            "Connectors provide normalized facts. Capabilities contribute typed context. Runtime owns reasoning."
        );
        let actions = vec![
            "List registered Capabilities",
            "Show Capability descriptor + loop stages",
            "Capability coverage / health",
            "List Engineering Loop runs",
            "Show Engineering Loop run",
            "Trace invocation / attempt",
            "Run Engineering Loop for attempt",
            "Back",
        ];
        let choice = Select::new()
            .with_prompt("Engineering Loop")
            .items(&actions)
            .default(0)
            .interact()
            .map_err(|e| e.to_string())?;
        match choice {
            0 => {
                for c in caps.list_execution_capabilities() {
                    println!(
                        "  • {} ({}) v{} [{}] complete={}",
                        c.capability_id,
                        c.display_name(),
                        c.version,
                        c.risk_level.as_str(),
                        c.is_complete()
                    );
                    println!(
                        "      provider={} operation={} Memory={} Evaluation={} Verification={} Improvement={} Learning={}",
                        c.provider,
                        c.operation,
                        c.engineering_loop.memory.as_str(),
                        c.engineering_loop.evaluation.as_str(),
                        c.engineering_loop.verification.as_str(),
                        c.engineering_loop.improvement.as_str(),
                        c.engineering_loop.learning.as_str(),
                    );
                }
            }
            1 => {
                let cap_id: String = Input::new()
                    .with_prompt("Capability id")
                    .default("mock.record".into())
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let desc = caps.show_execution_capability(&cap_id).map_err(err)?;
                println!("{} — {}", desc.display_name(), desc.description);
                println!(
                    "  provider={} operation={} mutating={} risk={}",
                    desc.provider,
                    desc.operation,
                    desc.mutating,
                    desc.risk_level.as_str()
                );
                println!(
                    "  accepted_input_types=[{}] output_types=[{}] provider_independent={} complete={}",
                    desc.accepted_input_types.join(", "),
                    desc.output_types.join(", "),
                    desc.provider_independent,
                    desc.is_complete()
                );
                println!(
                    "  Engineering Loop: M={} E={} V={} I={} L={}",
                    desc.engineering_loop.memory.as_str(),
                    desc.engineering_loop.evaluation.as_str(),
                    desc.engineering_loop.verification.as_str(),
                    desc.engineering_loop.improvement.as_str(),
                    desc.engineering_loop.learning.as_str(),
                );
                if !desc.limitations.is_empty() {
                    println!("  limitations:");
                    for lim in &desc.limitations {
                        println!("    - {lim}");
                    }
                }
            }
            2 => {
                let report = caps.capability_coverage_report();
                println!("{}", report.summary);
                println!(
                    "  first_party={}/{}  descriptors_complete={}  lifecycle_declared={}",
                    report.first_party_registered,
                    report.first_party_expected,
                    report.all_descriptors_complete,
                    report.all_lifecycle_declared
                );
                for c in &report.capabilities {
                    println!(
                        "  • {}  complete={}  loop=[M:{} E:{} V:{} I:{} L:{}]",
                        c.capability_id,
                        c.descriptor_complete,
                        c.memory,
                        c.evaluation,
                        c.verification,
                        c.improvement,
                        c.learning
                    );
                }
                println!("Connectors:");
                for conn in &report.connectors {
                    println!(
                        "  • {}  kinds=[{}]  fixture={}",
                        conn.connector_id,
                        conn.emitted_kinds.join(", "),
                        conn.fixture_support
                    );
                }
                if !report.gaps.is_empty() {
                    println!("Gaps:");
                    for g in &report.gaps {
                        println!("  - {g}");
                    }
                }
            }
            3 => {
                let listing = caps.list_lifecycle_runs(id).map_err(err)?;
                if listing.runs.is_empty() {
                    println!("No Engineering Loop runs.");
                }
                for run in listing.runs {
                    println!(
                        "  • {} rev {} [{}] {} inv={}",
                        run.id,
                        run.revision_number,
                        run.status.as_str(),
                        run.capability_id,
                        run.invocation_id
                    );
                }
                for d in listing.diagnostics {
                    println!(
                        "  {} corrupt lifecycle run isolated: {} ({})",
                        style("!").yellow(),
                        d.path,
                        d.error
                    );
                }
            }
            4 => {
                let run_id = input_object_id("Lifecycle run id")?;
                let run = caps.get_lifecycle_run(id, run_id).map_err(err)?;
                print_lifecycle_run(&run);
            }
            5 => {
                let inv_id: String = Input::new()
                    .with_prompt("Attempt / invocation / run id")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                let trace = caps.trace_capability_lifecycle(id, &inv_id).map_err(err)?;
                println!("  capability={}", trace.capability_id);
                println!("  invocation={}", trace.invocation_id);
                println!(
                    "  status={}",
                    trace
                        .status
                        .map(|s| s.as_str().to_string())
                        .unwrap_or_else(|| "none".into())
                );
                for stage in &trace.stages {
                    println!(
                        "    {}  {}  {}",
                        stage.stage.as_str(),
                        stage.status.as_str(),
                        stage.detail.clone().unwrap_or_default()
                    );
                }
                println!("  {}", trace.explanation);
            }
            6 => {
                let attempt = input_object_id("Attempt id")?;
                let run = caps
                    .run_capability_lifecycle_for_attempt(id, attempt, "workspace")
                    .map_err(err)?;
                print_lifecycle_run(&run);
            }
            _ => break,
        }
    }
    Ok(())
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

fn open_capabilities(data_dir: &PathBuf) -> Result<CapabilityService, String> {
    let store = LocalStore::open(data_dir).map_err(err)?;
    let runtime = Arc::new(Runtime::new(Arc::new(store)));
    runtime
        .register_execution_capability(Arc::new(MockExecutionCapability::new()))
        .map_err(err)?;
    // v0.8: always register first-party GitHub adapters for Capability Coverage.
    register_first_party_github_execution_capabilities(runtime.execution_registry())
        .map_err(err)?;
    Ok(CapabilityService::new(runtime))
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
