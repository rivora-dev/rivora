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
            1 => {
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
            2 => {
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
            3 => {
                let memory = caps.recall_memory(inv.id).map_err(err)?;
                if memory.is_empty() {
                    println!("No memory yet.");
                } else {
                    for m in memory {
                        println!("  • {}  {}", m.recorded_at.to_rfc3339(), m.summary);
                    }
                }
            }
            4 => {
                let timeline = caps.generate_timeline(inv.id).map_err(err)?;
                for e in timeline {
                    println!("  {}  [{}]  {}", e.at.to_rfc3339(), e.source, e.summary);
                }
            }
            5 => {
                let knowledge = caps.derive_knowledge(inv.id, "workspace").map_err(err)?;
                println!(
                    "{} Derived {} knowledge object(s)",
                    style("✓").green(),
                    knowledge.len()
                );
            }
            6 => {
                for k in caps.list_knowledge(inv.id).map_err(err)? {
                    println!("  • {:?}  {}", k.kind, k.summary);
                }
            }
            7 => {
                let evaluations = caps
                    .evaluate_investigation(inv.id, "workspace")
                    .map_err(err)?;
                println!(
                    "{} Produced {} evaluation(s)",
                    style("✓").green(),
                    evaluations.len()
                );
            }
            8 => {
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
            9 => {
                let receipts = caps.verify_all(inv.id, "workspace").map_err(err)?;
                println!(
                    "{} Produced {} verification receipt(s)",
                    style("✓").green(),
                    receipts.len()
                );
            }
            10 => {
                for r in caps.list_verifications(inv.id).map_err(err)? {
                    println!("  • [{}] {} — {}", r.result.as_str(), r.subject, r.reason);
                }
            }
            11 => {
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
            12 => {
                for r in caps.list_recommendations(inv.id).map_err(err)? {
                    println!(
                        "  • [{}] {} (confidence {:.0}%)",
                        r.status.as_str(),
                        r.summary,
                        r.confidence.value() * 100.0
                    );
                }
            }
            13 => {
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
            14 => {
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
            15 => {
                inv = caps
                    .reopen_investigation(inv.id, Some("reopened in workspace".into()))
                    .map_err(err)?;
                println!("{} Reopened ({})", style("✓").green(), inv.status);
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
