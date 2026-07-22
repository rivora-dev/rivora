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
    Investigation, InvestigationId, ObservationKind, OutcomeDisposition, RecommendationStatus,
};
use rivora::storage::LocalStore;
use rivora::{CapabilityService, Runtime};
use rivora_connectors::local::LocalConnector;

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
                    for id in ids {
                        let inv = caps.open_investigation(id).map_err(err)?;
                        println!("  {}  [{}]  {}", inv.id, inv.status, inv.title);
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

    println!(
        "workspace smoke ok: investigation {} status {}",
        done.id, done.status
    );
    Ok(())
}

fn open_capabilities(data_dir: &PathBuf) -> Result<CapabilityService, String> {
    let store = LocalStore::open(data_dir).map_err(err)?;
    let runtime = Arc::new(Runtime::new(Arc::new(store)));
    Ok(CapabilityService::new(runtime))
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
