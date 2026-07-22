//! Rivora CLI — thin Capability client (RFC-003).
//!
//! No Runtime business logic lives here. All reasoning is invoked via
//! `CapabilityService`.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};
use rivora::domain::{InvestigationId, ObjectId, ObservationKind, OutcomeDisposition};
use rivora::storage::LocalStore;
use rivora::{CapabilityService, Runtime};
use rivora_connectors::github::GitHubConnector;
use rivora_connectors::local::LocalConnector;
use rivora_connectors::NormalizedObservation;

#[derive(Debug, Parser)]
#[command(
    name = "rivora",
    version,
    about = "Rivora — Engineering Understanding Platform CLI"
)]
struct Cli {
    /// Data directory for local Runtime storage.
    #[arg(long, global = true, default_value = ".rivora/data")]
    data_dir: PathBuf,

    /// Emit JSON instead of human-readable text.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Investigation lifecycle commands.
    Investigation {
        #[command(subcommand)]
        action: InvestigationCmd,
    },
    /// Ingest Observations (manual or via connectors).
    Observe {
        /// Investigation id.
        #[arg(long)]
        investigation: String,
        /// Observation summary (manual mode).
        #[arg(long)]
        summary: Option<String>,
        /// Observation kind (manual mode).
        #[arg(long, default_value = "event")]
        kind: String,
        /// JSON payload (manual mode).
        #[arg(long, default_value = "{}")]
        payload: String,
        /// Source name.
        #[arg(long, default_value = "cli")]
        source: String,
        /// Idempotency key.
        #[arg(long)]
        idempotency_key: Option<String>,
        /// Observe local project path with the local connector.
        #[arg(long)]
        local: Option<PathBuf>,
        /// Observe GitHub repository (`owner/repo`).
        #[arg(long)]
        github: Option<String>,
        /// Pull request number for GitHub connector.
        #[arg(long)]
        pr: Option<u64>,
        /// Load GitHub fixture JSON instead of calling the API.
        #[arg(long)]
        github_fixture: Option<PathBuf>,
    },
    /// Recall Investigation Memory.
    Recall {
        #[arg(long)]
        investigation: String,
    },
    /// Generate Investigation timeline.
    Timeline {
        #[arg(long)]
        investigation: String,
    },
    /// Derive Knowledge from Memory.
    Knowledge {
        #[arg(long)]
        investigation: String,
    },
    /// Evaluate Investigation.
    Evaluate {
        #[arg(long)]
        investigation: String,
    },
    /// Verify conclusions.
    Verify {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        evaluation: Option<String>,
    },
    /// Generate Recommendations.
    Recommend {
        #[arg(long)]
        investigation: String,
    },
    /// Record a Learning outcome.
    Learn {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        recommendation: Option<String>,
        #[arg(long, value_enum)]
        disposition: DispositionArg,
        #[arg(long, default_value = "")]
        notes: String,
        #[arg(long)]
        impact: Option<String>,
    },
    /// Run full pipeline: knowledge → evaluate → verify → recommend.
    Pipeline {
        #[arg(long)]
        investigation: String,
    },
}

#[derive(Debug, Subcommand)]
enum InvestigationCmd {
    /// Create a new Investigation.
    Create {
        title: String,
        #[arg(long)]
        description: Option<String>,
    },
    /// Show an Investigation.
    Show { id: String },
    /// List Investigations.
    List,
    /// Complete an Investigation (must be in Learning).
    Complete {
        id: String,
        #[arg(long)]
        reason: Option<String>,
    },
    /// Reopen a completed Investigation.
    Reopen {
        id: String,
        #[arg(long)]
        reason: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DispositionArg {
    Accepted,
    Rejected,
    Ignored,
    Successful,
    Unsuccessful,
}

impl From<DispositionArg> for OutcomeDisposition {
    fn from(value: DispositionArg) -> Self {
        match value {
            DispositionArg::Accepted => Self::Accepted,
            DispositionArg::Rejected => Self::Rejected,
            DispositionArg::Ignored => Self::Ignored,
            DispositionArg::Successful => Self::Successful,
            DispositionArg::Unsuccessful => Self::Unsuccessful,
        }
    }
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
    let cli = Cli::parse();
    let caps = open_capabilities(&cli.data_dir)?;

    match cli.command {
        Commands::Investigation { action } => match action {
            InvestigationCmd::Create { title, description } => {
                let inv = caps
                    .create_investigation(title, description, "cli")
                    .map_err(err)?;
                print_value(cli.json, &inv, || {
                    format!(
                        "Created investigation {}\n  title: {}\n  status: {}",
                        inv.id, inv.title, inv.status
                    )
                });
            }
            InvestigationCmd::Show { id } => {
                let id = parse_inv(&id)?;
                let inv = caps.open_investigation(id).map_err(err)?;
                let memory = caps.recall_memory(id).map_err(err)?;
                let knowledge = caps.list_knowledge(id).map_err(err)?;
                let evaluations = caps.list_evaluations(id).map_err(err)?;
                let verifications = caps.list_verifications(id).map_err(err)?;
                let recommendations = caps.list_recommendations(id).map_err(err)?;
                let learning = caps.list_learning(id).map_err(err)?;
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "investigation": inv,
                            "memory_count": memory.len(),
                            "knowledge_count": knowledge.len(),
                            "evaluation_count": evaluations.len(),
                            "verification_count": verifications.len(),
                            "recommendation_count": recommendations.len(),
                            "learning_count": learning.len(),
                        }))
                        .map_err(|e| e.to_string())?
                    );
                } else {
                    println!("Investigation {}", inv.id);
                    println!("  title:  {}", inv.title);
                    println!("  status: {}", inv.status);
                    if let Some(d) = &inv.description {
                        println!("  description: {d}");
                    }
                    println!("  memory: {}", memory.len());
                    println!("  knowledge: {}", knowledge.len());
                    println!("  evaluations: {}", evaluations.len());
                    println!("  verifications: {}", verifications.len());
                    println!("  recommendations: {}", recommendations.len());
                    println!("  learning: {}", learning.len());
                    if !inv.transitions.is_empty() {
                        println!("  transitions:");
                        for t in &inv.transitions {
                            println!("    {} → {} ({})", t.from, t.to, t.at.to_rfc3339());
                        }
                    }
                }
            }
            InvestigationCmd::List => {
                let ids = caps.list_investigations().map_err(err)?;
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&ids).map_err(|e| e.to_string())?
                    );
                } else if ids.is_empty() {
                    println!("No investigations found.");
                } else {
                    for id in ids {
                        let inv = caps.open_investigation(id).map_err(err)?;
                        println!("{}  [{}]  {}", inv.id, inv.status, inv.title);
                    }
                }
            }
            InvestigationCmd::Complete { id, reason } => {
                let inv = caps
                    .complete_investigation(parse_inv(&id)?, reason)
                    .map_err(err)?;
                print_value(cli.json, &inv, || {
                    format!("Completed investigation {} ({})", inv.id, inv.status)
                });
            }
            InvestigationCmd::Reopen { id, reason } => {
                let inv = caps
                    .reopen_investigation(parse_inv(&id)?, reason)
                    .map_err(err)?;
                print_value(cli.json, &inv, || {
                    format!("Reopened investigation {} ({})", inv.id, inv.status)
                });
            }
        },
        Commands::Observe {
            investigation,
            summary,
            kind,
            payload,
            source,
            idempotency_key,
            local,
            github,
            pr,
            github_fixture,
        } => {
            let inv_id = parse_inv(&investigation)?;
            let mut observations: Vec<NormalizedObservation> = Vec::new();

            if let Some(path) = local {
                let connector = LocalConnector::new(path);
                observations.extend(connector.observe().map_err(|e| e.to_string())?);
            }
            if let Some(fixture_path) = github_fixture {
                let raw = std::fs::read_to_string(fixture_path).map_err(|e| e.to_string())?;
                let fixture: serde_json::Value =
                    serde_json::from_str(&raw).map_err(|e| e.to_string())?;
                observations.extend(
                    GitHubConnector::observe_from_fixture(&fixture).map_err(|e| e.to_string())?,
                );
            } else if let Some(repo) = github {
                let mut connector = GitHubConnector::new(repo);
                if let Some(n) = pr {
                    connector = connector.with_pull_request(n);
                }
                observations.extend(connector.observe().map_err(|e| e.to_string())?);
            }

            if let Some(summary) = summary {
                let payload_value: serde_json::Value =
                    serde_json::from_str(&payload).map_err(|e| format!("payload json: {e}"))?;
                observations.push(NormalizedObservation::new(
                    parse_kind(&kind),
                    summary,
                    payload_value,
                    source,
                    Utc::now(),
                    idempotency_key,
                    "cli",
                ));
            }

            if observations.is_empty() {
                return Err(
                    "provide --summary, --local <path>, --github <owner/repo>, or --github-fixture"
                        .into(),
                );
            }

            let mut ingested = Vec::new();
            for obs in observations {
                let (observation, memory, replay) = caps
                    .ingest_observation(
                        inv_id,
                        obs.kind,
                        obs.summary,
                        obs.payload,
                        obs.source,
                        obs.observed_at,
                        obs.idempotency_key,
                        "cli",
                    )
                    .map_err(err)?;
                ingested.push(serde_json::json!({
                    "observation_id": observation.id,
                    "memory_id": memory.id,
                    "summary": observation.summary,
                    "idempotent_replay": replay,
                }));
            }

            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&ingested).map_err(|e| e.to_string())?
                );
            } else {
                println!("Ingested {} observation(s):", ingested.len());
                for item in ingested {
                    println!(
                        "  {}  {}{}",
                        item["observation_id"],
                        item["summary"].as_str().unwrap_or(""),
                        if item["idempotent_replay"].as_bool() == Some(true) {
                            " (replay)"
                        } else {
                            ""
                        }
                    );
                }
            }
        }
        Commands::Recall { investigation } => {
            let memory = caps
                .recall_memory(parse_inv(&investigation)?)
                .map_err(err)?;
            print_value(cli.json, &memory, || {
                if memory.is_empty() {
                    "No memory records.".into()
                } else {
                    memory
                        .iter()
                        .map(|m| format!("{}  {}  {}", m.recorded_at.to_rfc3339(), m.id, m.summary))
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            });
        }
        Commands::Timeline { investigation } => {
            let timeline = caps
                .generate_timeline(parse_inv(&investigation)?)
                .map_err(err)?;
            print_value(cli.json, &timeline, || {
                timeline
                    .iter()
                    .map(|e| format!("{}  [{}]  {}", e.at.to_rfc3339(), e.source, e.summary))
                    .collect::<Vec<_>>()
                    .join("\n")
            });
        }
        Commands::Knowledge { investigation } => {
            let knowledge = caps
                .derive_knowledge(parse_inv(&investigation)?, "cli")
                .map_err(err)?;
            print_value(cli.json, &knowledge, || {
                knowledge
                    .iter()
                    .map(|k| {
                        format!(
                            "{}  [{:?}]  {} (confidence {:.0}%)",
                            k.id,
                            k.kind,
                            k.summary,
                            k.confidence.value() * 100.0
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            });
        }
        Commands::Evaluate { investigation } => {
            let evaluations = caps
                .evaluate_investigation(parse_inv(&investigation)?, "cli")
                .map_err(err)?;
            print_value(cli.json, &evaluations, || {
                evaluations
                    .iter()
                    .map(|e| {
                        format!(
                            "{}  [{:?}/{}]  {}\n    {}",
                            e.id,
                            e.assessment_type,
                            e.severity.as_str(),
                            e.summary,
                            e.explanation
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            });
        }
        Commands::Verify {
            investigation,
            evaluation,
        } => {
            let inv = parse_inv(&investigation)?;
            if let Some(eval) = evaluation {
                let receipt = caps
                    .verify_conclusion(inv, Some(parse_obj(&eval)?), "cli")
                    .map_err(err)?;
                print_value(cli.json, &receipt, || {
                    format!(
                        "{}  {}  {}\n  {}",
                        receipt.id,
                        receipt.result.as_str(),
                        receipt.subject,
                        receipt.reason
                    )
                });
            } else {
                let receipts = caps.verify_all(inv, "cli").map_err(err)?;
                print_value(cli.json, &receipts, || {
                    receipts
                        .iter()
                        .map(|r| {
                            format!(
                                "{}  {}  {}\n  {}",
                                r.id,
                                r.result.as_str(),
                                r.subject,
                                r.reason
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                });
            }
        }
        Commands::Recommend { investigation } => {
            let recs = caps
                .generate_recommendation(parse_inv(&investigation)?, "cli")
                .map_err(err)?;
            print_value(cli.json, &recs, || {
                recs.iter()
                    .map(|r| {
                        format!(
                            "{}  [{}]  {}\n  rationale: {}",
                            r.id,
                            r.status.as_str(),
                            r.summary,
                            r.rationale
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            });
        }
        Commands::Learn {
            investigation,
            recommendation,
            disposition,
            notes,
            impact,
        } => {
            let outcome = caps
                .record_outcome(
                    parse_inv(&investigation)?,
                    recommendation.map(|s| parse_obj(&s)).transpose()?,
                    disposition.into(),
                    notes,
                    impact,
                    "cli",
                )
                .map_err(err)?;
            print_value(cli.json, &outcome, || {
                format!(
                    "Recorded learning {} ({})",
                    outcome.id,
                    outcome.disposition.as_str()
                )
            });
        }
        Commands::Pipeline { investigation } => {
            let result = caps
                .run_full_pipeline(parse_inv(&investigation)?, "cli")
                .map_err(err)?;
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "knowledge": result.knowledge,
                        "evaluations": result.evaluations,
                        "verifications": result.verifications,
                        "recommendations": result.recommendations,
                    }))
                    .map_err(|e| e.to_string())?
                );
            } else {
                println!("Knowledge: {}", result.knowledge.len());
                println!("Evaluations: {}", result.evaluations.len());
                println!("Verifications: {}", result.verifications.len());
                println!("Recommendations: {}", result.recommendations.len());
                if let Some(rec) = result.recommendations.first() {
                    println!("\nTop recommendation:\n  {}", rec.summary);
                    println!("  {}", rec.rationale);
                }
            }
        }
    }

    Ok(())
}

fn open_capabilities(data_dir: &PathBuf) -> Result<CapabilityService, String> {
    let store = LocalStore::open(data_dir).map_err(err)?;
    let runtime = Arc::new(Runtime::new(Arc::new(store)));
    Ok(CapabilityService::new(runtime))
}

fn parse_inv(s: &str) -> Result<InvestigationId, String> {
    s.parse().map_err(err)
}

fn parse_obj(s: &str) -> Result<ObjectId, String> {
    s.parse().map_err(err)
}

fn parse_kind(s: &str) -> ObservationKind {
    match s.to_ascii_lowercase().as_str() {
        "event" => ObservationKind::Event,
        "repository" => ObservationKind::Repository,
        "commit" => ObservationKind::Commit,
        "git_status" | "git-status" => ObservationKind::GitStatus,
        "changed_files" | "changed-files" => ObservationKind::ChangedFiles,
        "pull_request" | "pr" => ObservationKind::PullRequest,
        "check" | "check_result" => ObservationKind::CheckResult,
        "test" | "test_output" => ObservationKind::TestOutput,
        "issue" => ObservationKind::Issue,
        "user" | "user_input" => ObservationKind::UserInput,
        "local_event" => ObservationKind::LocalEvent,
        other => ObservationKind::Other(other.into()),
    }
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn print_value<T: serde::Serialize>(json: bool, value: &T, human: impl FnOnce() -> String) {
    if json {
        match serde_json::to_string_pretty(value) {
            Ok(s) => println!("{s}"),
            Err(e) => eprintln!("error encoding json: {e}"),
        }
    } else {
        println!("{}", human());
    }
}
