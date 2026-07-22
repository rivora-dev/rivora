//! Rivora CLI — thin Capability client (RFC-003).
//!
//! No Runtime business logic lives here. All reasoning is invoked via
//! `CapabilityService`.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand, ValueEnum};
use rivora::domain::{
    InvestigationId, InvestigationStatus, ObjectId, ObservationKind, OutcomeDisposition,
    RelationshipKind, VerificationResult,
};
use rivora::runtime::search::{OutcomeFilter, SearchQuery, SearchResult};
use rivora::storage::LocalStore;
use rivora::{CapabilityService, Runtime};
use rivora_connectors::github::GitHubConnector;
use rivora_connectors::github_actions::{ConnectorStatusReport, GitHubActionsConnector};
use rivora_connectors::kubernetes::KubernetesConnector;
use rivora_connectors::local::LocalConnector;
use rivora_connectors::sentry::SentryConnector;
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
    /// Recall Investigation Memory, related evidence, or prior outcomes.
    Recall {
        /// Investigation id (Memory recall; combine with --evidence for
        /// related-evidence recall).
        #[arg(long)]
        investigation: Option<String>,
        /// Recall related evidence for the Investigation.
        #[arg(long)]
        evidence: bool,
        /// Prior-outcome recall: repository filter.
        #[arg(long)]
        repository: Option<String>,
        /// Prior-outcome recall: only this disposition.
        #[arg(long, value_enum)]
        outcome: Option<DispositionArg>,
        /// Prior-outcome recall: only Investigations related to this one.
        #[arg(long)]
        similar_to: Option<String>,
    },
    /// Search Investigations (text and/or structured filters).
    Search {
        /// Free-text query.
        query: Option<String>,
        /// Repository filter.
        #[arg(long)]
        repository: Option<String>,
        /// Status filter (e.g. collecting, completed).
        #[arg(long)]
        status: Option<String>,
        /// Connector source filter.
        #[arg(long)]
        source: Option<String>,
        /// Verification result filter: pass, fail, inconclusive.
        #[arg(long)]
        verification: Option<String>,
        /// Learning outcome filter.
        #[arg(long, value_enum)]
        outcome: Option<DispositionArg>,
        /// Changed-file path filter.
        #[arg(long)]
        file: Option<String>,
        /// Relationship kind filter (snake_case, e.g. shared_repository).
        #[arg(long)]
        relationship: Option<String>,
        /// Only Investigations created after this RFC3339 timestamp.
        #[arg(long)]
        after: Option<String>,
        /// Only Investigations created before this RFC3339 timestamp.
        #[arg(long)]
        before: Option<String>,
        /// Maximum number of results.
        #[arg(long)]
        limit: Option<usize>,
        /// Explain a specific result instead of listing all matches.
        #[arg(long)]
        explain: Option<String>,
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
    /// Detect Investigation patterns across durable records (RFC-017).
    Patterns,
    /// Summarize historical trends (RFC-017).
    Trends {
        /// Optional repository filter.
        #[arg(long)]
        repository: Option<String>,
    },
    /// Engineering assistance and Composite Capabilities (RFC-018 / RFC-019).
    Assist {
        #[command(subcommand)]
        action: AssistCmd,
    },
    /// Read-only connector operations (RFC-012).
    Connector {
        #[command(subcommand)]
        action: ConnectorCmd,
    },
    /// Generate an engineering report for an Investigation.
    Report {
        #[arg(long)]
        investigation: String,
    },
}

#[derive(Debug, Subcommand)]
enum AssistCmd {
    /// List Composite Capability intents.
    Intents,
    /// Plan a Composite Capability workflow.
    Plan {
        /// Composite intent slug.
        intent: String,
        #[arg(long)]
        investigation: String,
    },
    /// Investigate an engineering problem (composite).
    Investigate { investigation: String },
    /// Assess deployment readiness (composite).
    Readiness { investigation: String },
    /// Explain a failure (composite).
    ExplainFailure { investigation: String },
    /// Generate ranked hypotheses.
    Hypotheses {
        #[arg(long)]
        investigation: String,
    },
    /// Recommend next verification steps.
    NextVerification {
        #[arg(long)]
        investigation: String,
    },
    /// Forecast risks.
    Risks {
        #[arg(long)]
        investigation: String,
    },
    /// Root-cause guidance.
    RootCause {
        #[arg(long)]
        investigation: String,
    },
    /// Prioritize recommendations.
    Prioritize {
        #[arg(long)]
        investigation: String,
    },
    /// Summarize investigation state.
    Summarize {
        #[arg(long)]
        investigation: String,
    },
    /// Workflow inspection and control.
    Workflow {
        #[command(subcommand)]
        action: WorkflowCmd,
    },
}

#[derive(Debug, Subcommand)]
enum WorkflowCmd {
    /// Show a workflow.
    Show {
        #[arg(long)]
        investigation: String,
        workflow: String,
    },
    /// List workflows for an Investigation.
    List {
        #[arg(long)]
        investigation: String,
    },
    /// Resume a workflow.
    Resume {
        #[arg(long)]
        investigation: String,
        workflow: String,
    },
    /// Cancel a workflow.
    Cancel {
        #[arg(long)]
        investigation: String,
        workflow: String,
        #[arg(long)]
        reason: Option<String>,
    },
    /// Explain a workflow.
    Explain {
        #[arg(long)]
        investigation: String,
        workflow: String,
    },
}

#[derive(Debug, Subcommand)]
enum ConnectorCmd {
    /// List available connectors.
    List,
    /// Show connector status (no secrets).
    Status {
        /// Connector id: github_actions | kubernetes | sentry | github | local
        connector: String,
    },
    /// Test connector configuration.
    Test {
        connector: String,
        #[arg(long)]
        repository: Option<String>,
        #[arg(long)]
        namespace: Option<String>,
        #[arg(long)]
        organization: Option<String>,
        #[arg(long)]
        project: Option<String>,
    },
    /// Collect / preview normalized observations (fixture or live).
    Collect {
        connector: String,
        /// Fixture JSON path (offline).
        #[arg(long)]
        fixture: Option<PathBuf>,
        #[arg(long)]
        repository: Option<String>,
        #[arg(long)]
        namespace: Option<String>,
        #[arg(long)]
        organization: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        path: Option<PathBuf>,
        /// When set, ingest into this Investigation.
        #[arg(long)]
        investigation: Option<String>,
        /// Preview only (default true unless --investigation is set with --ingest).
        #[arg(long)]
        ingest: bool,
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
    /// List Investigations related to this one (RFC-015).
    Related { id: String },
    /// Create an explicit link between two Investigations.
    Link {
        source: String,
        target: String,
        #[arg(long)]
        reason: Option<String>,
    },
    /// Remove an explicit link (derived relationships refresh instead).
    Unlink { relationship_id: String },
    /// Explain why two Investigations are related.
    Relationship { relationship_id: String },
    /// Re-derive relationships for an Investigation.
    RefreshRelationships { id: String },
    /// Confirm a relationship as relevant.
    ConfirmRelationship { relationship_id: String },
    /// Dismiss a relationship as irrelevant.
    DismissRelationship { relationship_id: String },
    /// Find Investigations similar to this one.
    Similar {
        id: String,
        #[arg(long)]
        limit: Option<usize>,
    },
    /// List Recalled Context for an Investigation (RFC-017).
    Context { id: String },
    /// Suggest Recalled Context from related / similar Investigations.
    ContextSuggest { id: String },
    /// Attach historical context from a source Investigation (or confirm a suggestion).
    ContextAttach {
        /// Current Investigation id.
        id: String,
        /// Source Investigation id (manual attach).
        #[arg(long)]
        source: Option<String>,
        /// Existing Recalled Context id (confirm suggested).
        #[arg(long)]
        context: Option<String>,
        /// Reason for attachment.
        #[arg(long)]
        reason: Option<String>,
    },
    /// Dismiss a Recalled Context record.
    ContextDismiss { id: String, context: String },
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
            InvestigationCmd::Related { id } => {
                let related = caps
                    .list_related_investigations(parse_inv(&id)?)
                    .map_err(err)?;
                print_value(cli.json, &related, || {
                    if related.is_empty() {
                        "No related Investigations.".into()
                    } else {
                        related
                            .iter()
                            .map(|r| {
                                format!(
                                    "{}  [{}]  {}  [{}]  {}  (confidence {:.0}%, {})",
                                    r.relationship.id,
                                    r.relationship.kind.as_str(),
                                    r.related.id,
                                    r.related.status,
                                    r.related.title,
                                    r.relationship.confidence.value() * 100.0,
                                    r.relationship.confirmation.state.as_str()
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    }
                });
            }
            InvestigationCmd::Link {
                source,
                target,
                reason,
            } => {
                let relationship = caps
                    .link_investigations(parse_inv(&source)?, parse_inv(&target)?, reason, "cli")
                    .map_err(err)?;
                print_value(cli.json, &relationship, || {
                    format!(
                        "Linked {} ↔ {} ({})",
                        relationship.source_investigation_id,
                        relationship.target_investigation_id,
                        relationship.id
                    )
                });
            }
            InvestigationCmd::Unlink { relationship_id } => {
                let id = parse_obj(&relationship_id)?;
                caps.unlink_investigation(id, "cli").map_err(err)?;
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({ "unlinked": id }))
                            .map_err(|e| e.to_string())?
                    );
                } else {
                    println!("Unlinked {id}");
                }
            }
            InvestigationCmd::Relationship { relationship_id } => {
                let explanation = caps
                    .explain_relationship(parse_obj(&relationship_id)?)
                    .map_err(err)?;
                print_value(cli.json, &explanation, || explanation.explanation.clone());
            }
            InvestigationCmd::RefreshRelationships { id } => {
                let relationships = caps
                    .refresh_relationships(parse_inv(&id)?, "cli")
                    .map_err(err)?;
                print_value(cli.json, &relationships, || {
                    let mut out = format!("{} relationship(s):", relationships.len());
                    for r in &relationships {
                        out.push_str(&format!(
                            "\n  [{}]  {} ↔ {}  (confidence {:.0}%, {})",
                            r.kind.as_str(),
                            r.source_investigation_id,
                            r.target_investigation_id,
                            r.confidence.value() * 100.0,
                            r.confirmation.state.as_str()
                        ));
                    }
                    out
                });
            }
            InvestigationCmd::ConfirmRelationship { relationship_id } => {
                let relationship = caps
                    .confirm_relationship(parse_obj(&relationship_id)?, "cli")
                    .map_err(err)?;
                print_value(cli.json, &relationship, || {
                    format!("Confirmed relationship {}", relationship.id)
                });
            }
            InvestigationCmd::DismissRelationship { relationship_id } => {
                let relationship = caps
                    .dismiss_relationship(parse_obj(&relationship_id)?, "cli")
                    .map_err(err)?;
                print_value(cli.json, &relationship, || {
                    format!("Dismissed relationship {}", relationship.id)
                });
            }
            InvestigationCmd::Similar { id, limit } => {
                let results = caps
                    .find_similar_investigations(parse_inv(&id)?, limit)
                    .map_err(err)?;
                print_value(cli.json, &results, || print_search_results(&results));
            }
            InvestigationCmd::Context { id } => {
                let contexts = caps.list_recalled_context(parse_inv(&id)?).map_err(err)?;
                print_value(cli.json, &contexts, || print_recalled_contexts(&contexts));
            }
            InvestigationCmd::ContextSuggest { id } => {
                let contexts = caps
                    .suggest_recalled_context(parse_inv(&id)?, "cli")
                    .map_err(err)?;
                print_value(cli.json, &contexts, || print_recalled_contexts(&contexts));
            }
            InvestigationCmd::ContextAttach {
                id,
                source,
                context,
                reason,
            } => {
                let inv = parse_inv(&id)?;
                let attached = match (source, context) {
                    (Some(source), None) => caps
                        .attach_recalled_context_from_source(
                            inv,
                            parse_inv(&source)?,
                            reason,
                            "cli",
                        )
                        .map_err(err)?,
                    (None, Some(context_id)) => caps
                        .attach_recalled_context(inv, parse_obj(&context_id)?, "cli")
                        .map_err(err)?,
                    _ => {
                        return Err(
                            "context-attach requires exactly one of --source or --context".into(),
                        );
                    }
                };
                print_value(cli.json, &attached, || {
                    format!(
                        "Attached context {} from {} ({})",
                        attached.id,
                        attached.source_investigation_id,
                        attached.state.as_str()
                    )
                });
            }
            InvestigationCmd::ContextDismiss { id, context } => {
                let dismissed = caps
                    .dismiss_recalled_context(parse_inv(&id)?, parse_obj(&context)?, "cli")
                    .map_err(err)?;
                print_value(cli.json, &dismissed, || {
                    format!(
                        "Dismissed context {} ({})",
                        dismissed.id,
                        dismissed.state.as_str()
                    )
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
        Commands::Recall {
            investigation,
            evidence,
            repository,
            outcome,
            similar_to,
        } => {
            let has_outcome_filters =
                repository.is_some() || outcome.is_some() || similar_to.is_some();
            match (investigation, evidence, has_outcome_filters) {
                (Some(id), true, _) => {
                    let recalled = caps.recall_related_evidence(parse_inv(&id)?).map_err(err)?;
                    print_value(cli.json, &recalled, || {
                        if recalled.is_empty() {
                            "No related evidence.".into()
                        } else {
                            recalled
                                .iter()
                                .map(|r| {
                                    format!(
                                        "[{}] from {}\n  {}",
                                        r.relationship_kind.as_str(),
                                        r.investigation_id,
                                        r.explanation.lines().next().unwrap_or("")
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("\n")
                        }
                    });
                }
                (Some(id), false, false) => {
                    let memory = caps.recall_memory(parse_inv(&id)?).map_err(err)?;
                    print_value(cli.json, &memory, || {
                        if memory.is_empty() {
                            "No memory records.".into()
                        } else {
                            memory
                                .iter()
                                .map(|m| {
                                    format!(
                                        "{}  {}  {}",
                                        m.recorded_at.to_rfc3339(),
                                        m.id,
                                        m.summary
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("\n")
                        }
                    });
                }
                (None, false, true) => {
                    let outcomes = caps
                        .recall_prior_outcomes(OutcomeFilter {
                            repository,
                            similar_to: similar_to.map(|s| parse_inv(&s)).transpose()?,
                            disposition: outcome.map(Into::into),
                        })
                        .map_err(err)?;
                    print_value(cli.json, &outcomes, || {
                        if outcomes.is_empty() {
                            "No prior outcomes.".into()
                        } else {
                            outcomes
                                .iter()
                                .map(|o| {
                                    format!(
                                        "{}  [{}]  {} — {}{}",
                                        o.investigation_id,
                                        o.outcome.disposition.as_str(),
                                        o.investigation_title,
                                        o.outcome.notes,
                                        o.recommendation_summary
                                            .as_ref()
                                            .map(|s| format!(" (re: {s})"))
                                            .unwrap_or_default()
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("\n")
                        }
                    });
                }
                _ => {
                    return Err("provide --investigation, --investigation with --evidence, \
                         or outcome filters (--repository/--outcome/--similar-to)"
                        .into())
                }
            }
        }
        Commands::Search {
            query,
            repository,
            status,
            source,
            verification,
            outcome,
            file,
            relationship,
            after,
            before,
            limit,
            explain,
        } => {
            let search_query = SearchQuery {
                text: query,
                investigation_id: None,
                repository,
                status: status.map(|s| parse_status(&s)).transpose()?,
                connector_source: source,
                verification_result: verification.map(|v| parse_verification(&v)).transpose()?,
                outcome: outcome.map(Into::into),
                relationship_kind: relationship
                    .map(|r| parse_relationship_kind(&r))
                    .transpose()?,
                file,
                created_after: after.map(|d| parse_datetime(&d)).transpose()?,
                created_before: before.map(|d| parse_datetime(&d)).transpose()?,
                limit,
            };
            if let Some(id) = explain {
                let result = caps
                    .explain_search_result(parse_inv(&id)?, search_query)
                    .map_err(err)?;
                print_value(cli.json, &result, || {
                    format!(
                        "{}  [{}]  {}\n  score: {:.2}\n  {}",
                        result.investigation_id,
                        result.status,
                        result.title,
                        result.score,
                        result.explanation
                    )
                });
            } else {
                let results = caps.search_investigations(search_query).map_err(err)?;
                print_value(cli.json, &results, || print_search_results(&results));
            }
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
        Commands::Patterns => {
            let patterns = caps.detect_patterns("cli").map_err(err)?;
            print_value(cli.json, &patterns, || {
                if patterns.is_empty() {
                    "No patterns detected.".into()
                } else {
                    patterns
                        .iter()
                        .map(|p| {
                            format!(
                                "[{}]  {}  ({} investigations, confidence {:.0}%)\n  {}",
                                p.kind.as_str(),
                                p.signature,
                                p.occurrence_count,
                                p.confidence.value() * 100.0,
                                p.description
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            });
        }
        Commands::Trends { repository } => {
            let trend = caps.summarize_historical_trend(repository).map_err(err)?;
            print_value(cli.json, &trend, || {
                let mut out = trend.summary.clone();
                if !trend.top_failure_signatures.is_empty() {
                    out.push_str("\nTop failure signatures:");
                    for item in &trend.top_failure_signatures {
                        out.push_str(&format!("\n  {} ({})", item.label, item.count));
                    }
                }
                out
            });
        }
        Commands::Assist { action } => match action {
            AssistCmd::Intents => {
                let defs = caps.list_composite_capabilities();
                print_value(cli.json, &defs, || {
                    defs.iter()
                        .map(|d| {
                            format!(
                                "{} — {}\n  cores: {}",
                                d.id,
                                d.description,
                                d.core_capabilities.join(" → ")
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                });
            }
            AssistCmd::Plan {
                intent,
                investigation,
            } => {
                let wf = caps
                    .plan_workflow(parse_inv(&investigation)?, intent, "cli")
                    .map_err(err)?;
                print_value(cli.json, &wf, || {
                    format!(
                        "Planned workflow {} intent={} steps={}",
                        wf.id,
                        wf.intent,
                        wf.steps.len()
                    )
                });
            }
            AssistCmd::Investigate { investigation } => {
                let wf = caps
                    .run_composite(
                        parse_inv(&investigation)?,
                        "investigate_engineering_problem",
                        "cli",
                    )
                    .map_err(err)?;
                print_value(cli.json, &wf, || {
                    format!(
                        "Workflow {} status={}\n{}",
                        wf.id,
                        wf.status.as_str(),
                        wf.summary.as_deref().unwrap_or("")
                    )
                });
            }
            AssistCmd::Readiness { investigation } => {
                let id = parse_inv(&investigation)?;
                let wf = caps
                    .run_composite(id, "assess_deployment_readiness", "cli")
                    .map_err(err)?;
                print_value(cli.json, &wf, || {
                    format!(
                        "Readiness workflow {} status={}\n{}",
                        wf.id,
                        wf.status.as_str(),
                        wf.summary.as_deref().unwrap_or("")
                    )
                });
            }
            AssistCmd::ExplainFailure { investigation } => {
                let wf = caps
                    .run_composite(parse_inv(&investigation)?, "explain_failure", "cli")
                    .map_err(err)?;
                print_value(cli.json, &wf, || {
                    format!(
                        "Explain-failure workflow {} status={}\n{}",
                        wf.id,
                        wf.status.as_str(),
                        wf.summary.as_deref().unwrap_or("")
                    )
                });
            }
            AssistCmd::Hypotheses { investigation } => {
                let hyps = caps
                    .generate_hypotheses(parse_inv(&investigation)?, "cli")
                    .map_err(err)?;
                print_value(cli.json, &hyps, || {
                    hyps.iter()
                        .map(|h| {
                            format!(
                                "{}. [{} conf={:.0}%] {}",
                                h.rank,
                                h.status.as_str(),
                                h.confidence.value() * 100.0,
                                h.statement
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                });
            }
            AssistCmd::NextVerification { investigation } => {
                let suggestions = caps
                    .recommend_next_verification(parse_inv(&investigation)?, "cli")
                    .map_err(err)?;
                print_value(cli.json, &suggestions, || {
                    suggestions
                        .iter()
                        .map(|s| {
                            format!(
                                "{}. {} — {} ({})",
                                s.rank,
                                s.claim,
                                s.method,
                                s.feasibility.as_str()
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                });
            }
            AssistCmd::Risks { investigation } => {
                let forecast = caps
                    .forecast_risk(parse_inv(&investigation)?, "cli")
                    .map_err(err)?;
                print_value(cli.json, &forecast, || {
                    let mut out = forecast.summary.clone();
                    for item in &forecast.items {
                        out.push_str(&format!(
                            "\n- {} [{}]: {}",
                            item.category.as_str(),
                            item.severity.as_str(),
                            item.mitigation
                        ));
                    }
                    out
                });
            }
            AssistCmd::RootCause { investigation } => {
                let guidance = caps
                    .generate_root_cause_guidance(parse_inv(&investigation)?, "cli")
                    .map_err(err)?;
                print_value(cli.json, &guidance, || guidance.guidance.clone());
            }
            AssistCmd::Prioritize { investigation } => {
                let ranked = caps
                    .prioritize_recommendations(parse_inv(&investigation)?, "cli")
                    .map_err(err)?;
                print_value(cli.json, &ranked, || {
                    ranked
                        .iter()
                        .map(|r| {
                            format!(
                                "{}. score={:.3} {} — {}",
                                r.rank, r.score, r.summary, r.explanation
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                });
            }
            AssistCmd::Summarize { investigation } => {
                let summary = caps
                    .summarize_investigation_state(parse_inv(&investigation)?, "cli")
                    .map_err(err)?;
                print_value(cli.json, &summary, || summary.summary.clone());
            }
            AssistCmd::Workflow { action } => match action {
                WorkflowCmd::Show {
                    investigation,
                    workflow,
                } => {
                    let wf = caps
                        .open_workflow(parse_inv(&investigation)?, parse_obj(&workflow)?)
                        .map_err(err)?;
                    print_value(cli.json, &wf, || {
                        format!(
                            "Workflow {} intent={} status={}\n{}",
                            wf.id,
                            wf.intent,
                            wf.status.as_str(),
                            wf.summary.as_deref().unwrap_or("")
                        )
                    });
                }
                WorkflowCmd::List { investigation } => {
                    let list = caps
                        .list_workflows(parse_inv(&investigation)?)
                        .map_err(err)?;
                    print_value(cli.json, &list, || {
                        list.iter()
                            .map(|w| {
                                format!(
                                    "{}  {}  [{}]  steps={}",
                                    w.id,
                                    w.intent,
                                    w.status.as_str(),
                                    w.steps.len()
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    });
                }
                WorkflowCmd::Resume {
                    investigation,
                    workflow,
                } => {
                    let wf = caps
                        .resume_workflow(parse_inv(&investigation)?, parse_obj(&workflow)?, "cli")
                        .map_err(err)?;
                    print_value(cli.json, &wf, || {
                        format!("Resumed {} status={}", wf.id, wf.status.as_str())
                    });
                }
                WorkflowCmd::Cancel {
                    investigation,
                    workflow,
                    reason,
                } => {
                    let wf = caps
                        .cancel_workflow(
                            parse_inv(&investigation)?,
                            parse_obj(&workflow)?,
                            reason,
                            "cli",
                        )
                        .map_err(err)?;
                    print_value(cli.json, &wf, || {
                        format!("Cancelled {} status={}", wf.id, wf.status.as_str())
                    });
                }
                WorkflowCmd::Explain {
                    investigation,
                    workflow,
                } => {
                    let text = caps
                        .explain_workflow(parse_inv(&investigation)?, parse_obj(&workflow)?)
                        .map_err(err)?;
                    if cli.json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(
                                &serde_json::json!({ "explanation": text })
                            )
                            .map_err(|e| e.to_string())?
                        );
                    } else {
                        println!("{text}");
                    }
                }
            },
        },
        Commands::Connector { action } => match action {
            ConnectorCmd::List => {
                let list = vec![
                    ConnectorStatusReport {
                        id: "local".into(),
                        category: "local".into(),
                        configured: true,
                        read_only: true,
                        details: "local project observer".into(),
                    },
                    ConnectorStatusReport {
                        id: "github".into(),
                        category: "code".into(),
                        configured: std::env::var("GITHUB_TOKEN").is_ok(),
                        read_only: true,
                        details: "GitHub repository/PR observer".into(),
                    },
                    GitHubActionsConnector::new("owner/repo").status(),
                    KubernetesConnector::new("default").status(),
                    SentryConnector::new("org", "project").status(),
                ];
                print_value(cli.json, &list, || {
                    list.iter()
                        .map(|c| {
                            format!(
                                "{}  [{}]  configured={}  read_only={}  {}",
                                c.id, c.category, c.configured, c.read_only, c.details
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                });
            }
            ConnectorCmd::Status { connector } => {
                let report = connector_status(&connector)?;
                print_value(cli.json, &report, || {
                    format!(
                        "{} [{}] configured={} read_only={}\n{}",
                        report.id,
                        report.category,
                        report.configured,
                        report.read_only,
                        report.details
                    )
                });
            }
            ConnectorCmd::Test {
                connector,
                repository,
                namespace,
                organization,
                project,
            } => {
                let msg = connector_test(
                    &connector,
                    repository.as_deref(),
                    namespace.as_deref(),
                    organization.as_deref(),
                    project.as_deref(),
                )?;
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({ "result": msg }))
                            .map_err(|e| e.to_string())?
                    );
                } else {
                    println!("{msg}");
                }
            }
            ConnectorCmd::Collect {
                connector,
                fixture,
                repository,
                namespace,
                organization,
                project,
                path,
                investigation,
                ingest,
            } => {
                let observations = connector_collect(
                    &connector,
                    fixture.as_ref(),
                    repository.as_deref(),
                    namespace.as_deref(),
                    organization.as_deref(),
                    project.as_deref(),
                    path.as_ref(),
                )?;
                if let Some(inv) = investigation {
                    if ingest {
                        let inv_id = parse_inv(&inv)?;
                        let mut receipts = Vec::new();
                        for obs in &observations {
                            let (observation, memory, replay) = caps
                                .ingest_observation(
                                    inv_id,
                                    obs.kind.clone(),
                                    obs.summary.clone(),
                                    obs.payload.clone(),
                                    obs.source.clone(),
                                    obs.observed_at,
                                    obs.idempotency_key.clone(),
                                    "cli",
                                )
                                .map_err(err)?;
                            receipts.push(serde_json::json!({
                                "observation_id": observation.id,
                                "memory_id": memory.id,
                                "summary": observation.summary,
                                "idempotent_replay": replay,
                            }));
                        }
                        print_value(cli.json, &receipts, || {
                            format!("Ingested {} observation(s).", receipts.len())
                        });
                    } else {
                        print_value(
                            cli.json,
                            &observations
                                .iter()
                                .map(|o| {
                                    serde_json::json!({
                                        "kind": o.kind.as_str(),
                                        "summary": o.summary,
                                        "source": o.source,
                                        "idempotency_key": o.idempotency_key,
                                    })
                                })
                                .collect::<Vec<_>>(),
                            || {
                                format!(
                                    "Preview {} observation(s). Pass --ingest to write Memory.",
                                    observations.len()
                                )
                            },
                        );
                    }
                } else {
                    print_value(
                        cli.json,
                        &observations
                            .iter()
                            .map(|o| {
                                serde_json::json!({
                                    "kind": o.kind.as_str(),
                                    "summary": o.summary,
                                    "source": o.source,
                                    "idempotency_key": o.idempotency_key,
                                })
                            })
                            .collect::<Vec<_>>(),
                        || {
                            observations
                                .iter()
                                .map(|o| format!("[{}] {}", o.kind.as_str(), o.summary))
                                .collect::<Vec<_>>()
                                .join("\n")
                        },
                    );
                }
            }
        },
        Commands::Report { investigation } => {
            let report = caps
                .generate_engineering_report(parse_inv(&investigation)?, "cli")
                .map_err(err)?;
            if cli.json {
                print_value(true, &report, String::new);
            } else {
                println!("{}", report.markdown);
            }
        }
    }

    Ok(())
}

fn connector_status(name: &str) -> Result<ConnectorStatusReport, String> {
    match name {
        "github_actions" | "actions" | "ci" => {
            Ok(GitHubActionsConnector::new("owner/repo").status())
        }
        "kubernetes" | "k8s" => Ok(KubernetesConnector::new("default").status()),
        "sentry" => Ok(SentryConnector::new("org", "project").status()),
        "github" => Ok(ConnectorStatusReport {
            id: "github".into(),
            category: "code".into(),
            configured: std::env::var("GITHUB_TOKEN")
                .ok()
                .filter(|s| !s.is_empty())
                .is_some(),
            read_only: true,
            details: "GitHub connector (repository/PR)".into(),
        }),
        "local" => Ok(ConnectorStatusReport {
            id: "local".into(),
            category: "local".into(),
            configured: true,
            read_only: true,
            details: "Local project connector".into(),
        }),
        other => Err(format!("unknown connector: {other}")),
    }
}

fn connector_test(
    name: &str,
    repository: Option<&str>,
    namespace: Option<&str>,
    organization: Option<&str>,
    project: Option<&str>,
) -> Result<String, String> {
    match name {
        "github_actions" | "actions" | "ci" => {
            GitHubActionsConnector::new(repository.unwrap_or("owner/repo"))
                .test_configuration()
                .map_err(err)
        }
        "kubernetes" | "k8s" => KubernetesConnector::new(namespace.unwrap_or("default"))
            .test_configuration()
            .map_err(err),
        "sentry" => {
            SentryConnector::new(organization.unwrap_or("org"), project.unwrap_or("project"))
                .test_configuration()
                .map_err(err)
        }
        "github" | "local" => Ok(format!("{name}: available (read-only)")),
        other => Err(format!("unknown connector: {other}")),
    }
}

fn connector_collect(
    name: &str,
    fixture: Option<&PathBuf>,
    repository: Option<&str>,
    namespace: Option<&str>,
    organization: Option<&str>,
    project: Option<&str>,
    path: Option<&PathBuf>,
) -> Result<Vec<NormalizedObservation>, String> {
    match name {
        "github_actions" | "actions" | "ci" => {
            if let Some(path) = fixture {
                let raw = std::fs::read_to_string(path).map_err(err)?;
                let value: serde_json::Value = serde_json::from_str(&raw).map_err(err)?;
                GitHubActionsConnector::observe_from_fixture(&value).map_err(err)
            } else {
                GitHubActionsConnector::new(repository.unwrap_or("owner/repo"))
                    .observe()
                    .map_err(err)
            }
        }
        "kubernetes" | "k8s" => {
            if let Some(path) = fixture {
                let raw = std::fs::read_to_string(path).map_err(err)?;
                let value: serde_json::Value = serde_json::from_str(&raw).map_err(err)?;
                KubernetesConnector::observe_from_fixture(&value).map_err(err)
            } else {
                KubernetesConnector::new(namespace.unwrap_or("default"))
                    .observe()
                    .map_err(err)
            }
        }
        "sentry" => {
            if let Some(path) = fixture {
                let raw = std::fs::read_to_string(path).map_err(err)?;
                let value: serde_json::Value = serde_json::from_str(&raw).map_err(err)?;
                SentryConnector::observe_from_fixture(&value).map_err(err)
            } else {
                SentryConnector::new(organization.unwrap_or("org"), project.unwrap_or("project"))
                    .observe()
                    .map_err(err)
            }
        }
        "local" => {
            let root = path.cloned().unwrap_or_else(|| PathBuf::from("."));
            LocalConnector::new(root).observe().map_err(err)
        }
        "github" => {
            if let Some(path) = fixture {
                let raw = std::fs::read_to_string(path).map_err(err)?;
                let value: serde_json::Value = serde_json::from_str(&raw).map_err(err)?;
                GitHubConnector::observe_from_fixture(&value).map_err(err)
            } else {
                GitHubConnector::new(repository.ok_or("repository required for github")?)
                    .observe()
                    .map_err(err)
            }
        }
        other => Err(format!("unknown connector: {other}")),
    }
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
        "workflow_run" | "workflow-run" | "ci" => ObservationKind::WorkflowRun,
        "infrastructure" | "infra" | "k8s" => ObservationKind::Infrastructure,
        "observability" | "alert" | "sentry" => ObservationKind::Observability,
        other => ObservationKind::Other(other.into()),
    }
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn parse_status(s: &str) -> Result<InvestigationStatus, String> {
    match s.to_ascii_lowercase().as_str() {
        "created" => Ok(InvestigationStatus::Created),
        "collecting" => Ok(InvestigationStatus::Collecting),
        "understanding" => Ok(InvestigationStatus::Understanding),
        "evaluating" => Ok(InvestigationStatus::Evaluating),
        "verifying" => Ok(InvestigationStatus::Verifying),
        "recommending" => Ok(InvestigationStatus::Recommending),
        "learning" => Ok(InvestigationStatus::Learning),
        "completed" => Ok(InvestigationStatus::Completed),
        other => Err(format!("unknown status: {other}")),
    }
}

fn parse_verification(s: &str) -> Result<VerificationResult, String> {
    match s.to_ascii_lowercase().as_str() {
        "pass" => Ok(VerificationResult::Pass),
        "fail" => Ok(VerificationResult::Fail),
        "inconclusive" => Ok(VerificationResult::Inconclusive),
        other => Err(format!("unknown verification result: {other}")),
    }
}

fn parse_relationship_kind(s: &str) -> Result<RelationshipKind, String> {
    match s.to_ascii_lowercase().as_str() {
        "shared_repository" => Ok(RelationshipKind::SharedRepository),
        "shared_commit" => Ok(RelationshipKind::SharedCommit),
        "shared_pull_request" => Ok(RelationshipKind::SharedPullRequest),
        "shared_file_path" => Ok(RelationshipKind::SharedFilePath),
        "shared_connector_source" => Ok(RelationshipKind::SharedConnectorSource),
        "similar_observations" => Ok(RelationshipKind::SimilarObservations),
        "shared_evaluation_category" => Ok(RelationshipKind::SharedEvaluationCategory),
        "related_verification_outcome" => Ok(RelationshipKind::RelatedVerificationOutcome),
        "repeated_failure_signature" => Ok(RelationshipKind::RepeatedFailureSignature),
        "related_recommendation" => Ok(RelationshipKind::RelatedRecommendation),
        "related_learning_outcome" => Ok(RelationshipKind::RelatedLearningOutcome),
        "explicit_link" => Ok(RelationshipKind::ExplicitLink),
        other => Err(format!("unknown relationship kind: {other}")),
    }
}

fn parse_datetime(s: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| format!("invalid RFC3339 timestamp `{s}`: {e}"))
}

fn print_recalled_contexts(contexts: &[rivora::domain::RecalledContext]) -> String {
    if contexts.is_empty() {
        return "No recalled context.".into();
    }
    contexts
        .iter()
        .map(|c| {
            format!(
                "{}  [{}]  from {}  ({})\n  reason: {}\n  {}\n  {}",
                c.id,
                c.state.as_str(),
                c.source_investigation_id,
                c.origin.as_str(),
                c.reason,
                c.evidence_summary,
                c.explanation
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn print_search_results(results: &[SearchResult]) -> String {
    if results.is_empty() {
        return "No matching Investigations.".into();
    }
    results
        .iter()
        .map(|r| {
            format!(
                "{}  [{}]  {}  (score {:.2})\n    {}",
                r.investigation_id, r.status, r.title, r.score, r.explanation
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
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
