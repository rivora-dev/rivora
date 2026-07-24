//! Execute typed Workspace intents via CapabilityService.
//!
//! UI navigation intents produce view-only results. Mutating intents call
//! Capabilities. Nothing here bypasses Execution Plan approval.
#![allow(dead_code)]

use chrono::Utc;
use rivora::domain::{
    Confidence, ObservationKind, ProposalCategory, ProposalPriority, ProposalStatus,
    ProposalTransitionAuthority,
};
use rivora::runtime::proposal::CreateProposalRequest;
use rivora::runtime::search::SearchQuery;
use rivora::{CapabilityService, DEFAULT_LIST_LIMIT};

use super::model::{InvestigationDraft, WorkspaceIntent, WorkspaceRoute};
use crate::error_view::{map_error, WorkspaceErrorView};

/// Result of executing a Workspace intent.
#[derive(Debug, Clone)]
pub enum IntentExecutionResult {
    /// Pure UI navigation.
    Navigate(WorkspaceRoute),
    /// Quit the application.
    Quit,
    /// Investigation created and should become active.
    InvestigationCreated {
        id: rivora::domain::InvestigationId,
        title: String,
        summary: String,
    },
    /// Investigation opened.
    InvestigationOpened {
        id: rivora::domain::InvestigationId,
        title: String,
        status: String,
        summary: String,
    },
    /// List of investigations for UI.
    InvestigationList {
        items: Vec<InvestigationListItem>,
        summary: String,
    },
    /// Search results.
    SearchResults {
        query: String,
        items: Vec<InvestigationListItem>,
        summary: String,
    },
    /// Generic informational content for conversation / panels.
    Info {
        title: String,
        body: String,
        route: Option<WorkspaceRoute>,
    },
    /// Structured lines for doctor / connectors / etc.
    Panel {
        title: String,
        lines: Vec<String>,
        route: WorkspaceRoute,
    },
    /// Capability produced objects — conversation should reference them.
    CapabilityWork {
        title: String,
        body: String,
        investigation_id: Option<rivora::domain::InvestigationId>,
        object_refs: Vec<String>,
        route: Option<WorkspaceRoute>,
    },
    /// Needs confirmation before the app re-dispatches the original typed
    /// intent via the normal authority path. Confirmation is a UI state
    /// transition handled in the app layer (`confirm_pending`), not a
    /// workspace intent variant; `Apply` here means "the app now re-runs
    /// `dispatch_intent(pending)`" which routes through Capabilities as usual.
    NeedsConfirmation {
        preview_title: String,
        preview_body: String,
        pending: WorkspaceIntent,
    },
    /// User-facing error (never raw double-prefixed validation).
    Error(WorkspaceErrorView),
    /// Clarification / low-confidence response.
    Clarification { message: String },
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct InvestigationListItem {
    pub id: rivora::domain::InvestigationId,
    pub title: String,
    pub status: String,
    pub updated_at: String,
    pub score: Option<f64>,
}

/// Execute an intent. Callers must not pass uninterpreted free-form text as authority.
pub fn execute_intent(caps: &CapabilityService, intent: &WorkspaceIntent) -> IntentExecutionResult {
    match intent {
        WorkspaceIntent::SubmitPrompt { text } => IntentExecutionResult::Clarification {
            message: if text.is_empty() {
                "Type a request or press `/` to browse actions.".into()
            } else {
                format!(
                    "I could not map that to a typed action yet.\n\nYou said: {text}\n\n\
                     Try: create an investigation, search, evaluate (with an open investigation), \
                     or press `/` for actions."
                )
            },
        },
        WorkspaceIntent::Quit => IntentExecutionResult::Quit,
        WorkspaceIntent::OpenHome | WorkspaceIntent::Navigate { route: WorkspaceRoute::Home } => {
            IntentExecutionResult::Navigate(WorkspaceRoute::Home)
        }
        WorkspaceIntent::Navigate { route } => IntentExecutionResult::Navigate(*route),
        WorkspaceIntent::OpenHelp => IntentExecutionResult::Panel {
            title: "Workspace Help".into(),
            lines: help_lines(),
            route: WorkspaceRoute::Help,
        },
        WorkspaceIntent::OpenSettings => IntentExecutionResult::Panel {
            title: "Settings".into(),
            lines: vec![
                "Workspace preferences (v0.10)".into(),
                "Theme: default (semantic roles)".into(),
                "Provider: deterministic intent interpreter".into(),
                "Agent: handoff boundary available (no auto-execution)".into(),
                "Secrets are never stored in conversation state.".into(),
            ],
            route: WorkspaceRoute::Settings,
        },
        WorkspaceIntent::ShowDoctor => doctor_panel(caps),
        WorkspaceIntent::ShowConnectors => connectors_panel(),
        WorkspaceIntent::TestConnector { connector_id } => test_connector(connector_id),
        WorkspaceIntent::ShowPriorOutcomes => prior_outcomes(caps),
        WorkspaceIntent::ShowPatterns => patterns(caps),
        WorkspaceIntent::ShowHistoricalTrends => trends(caps),
        WorkspaceIntent::ListInvestigations => list_investigations(caps),
        WorkspaceIntent::SearchInvestigations { query } => search_investigations(caps, query),
        WorkspaceIntent::CreateInvestigation { draft } => create_investigation(caps, draft),
        WorkspaceIntent::OpenInvestigation { investigation_id } => {
            open_investigation(caps, *investigation_id)
        }
        WorkspaceIntent::AddObservation {
            investigation_id,
            summary,
        } => add_observation(caps, *investigation_id, summary),
        WorkspaceIntent::RunEvaluation { investigation_id } => {
            run_evaluation(caps, *investigation_id)
        }
        WorkspaceIntent::RunVerification { investigation_id } => {
            run_verification(caps, *investigation_id)
        }
        WorkspaceIntent::GenerateRecommendation { investigation_id } => {
            run_recommend(caps, *investigation_id)
        }
        WorkspaceIntent::CreateProposal { investigation_id } => {
            create_proposal(caps, *investigation_id)
        }
        WorkspaceIntent::ReviewProposals { investigation_id } => {
            review_proposals(caps, *investigation_id)
        }
        WorkspaceIntent::ReviewProposal { proposal_id } => IntentExecutionResult::Info {
            title: "Proposal".into(),
            body: format!(
                "Proposal {proposal_id}\nOpen Review Proposals from an Investigation to load details."
            ),
            route: Some(WorkspaceRoute::ProposalReview),
        },
        WorkspaceIntent::CreateExecutionPlan {
            investigation_id,
            proposal_id,
        } => IntentExecutionResult::Info {
            title: "Execution Plan requires authority path".into(),
            body: format!(
                "Investigation {investigation_id}\nProposal {proposal_id}\n\n\
                 Creating a plan does not execute anything.\n\
                 Live execution still requires validate → approve (exact revision) → explicit run.\n\
                 Use Review Executions after creating a plan via CLI or proposal session."
            ),
            route: Some(WorkspaceRoute::ExecutionReview),
        },
        WorkspaceIntent::ReviewExecutions { investigation_id } => {
            review_executions(caps, *investigation_id)
        }
        WorkspaceIntent::ReviewExecution { plan_id } => IntentExecutionResult::Info {
            title: "Execution Plan".into(),
            body: format!("Plan {plan_id} — open Review Executions for the active Investigation."),
            route: Some(WorkspaceRoute::ExecutionReview),
        },
        WorkspaceIntent::ShowLearning { investigation_id } => {
            show_learning(caps, *investigation_id)
        }
        WorkspaceIntent::AgentHandoff {
            investigation_id,
            proposal_id,
        } => agent_handoff(caps, *investigation_id, *proposal_id),
    }
}

fn help_lines() -> Vec<String> {
    vec![
        "Rivora Unified Workspace".into(),
        "".into(),
        "Type naturally in Ask Rivora…".into(),
        "Press / for searchable actions".into(),
        "Ctrl+P for the global command palette".into(),
        "Tab / Shift+Tab move focus".into(),
        "Esc close overlay or cancel".into(),
        "? help".into(),
        "Ctrl+C quit (safe terminal restore)".into(),
        "".into(),
        "Conversation is an interface projection.".into(),
        "Runtime authority stays with Capabilities,".into(),
        "Execution Plans, and explicit approval.".into(),
    ]
}

fn doctor_panel(caps: &CapabilityService) -> IntentExecutionResult {
    let mut lines = Vec::new();
    match caps.store_health() {
        Ok(h) => {
            lines.push(format!("Store root: {}", h.root));
            lines.push(format!("Schema version: {}", h.schema_version));
            lines.push(format!("Lock held: {}", h.lock_held));
            lines.push(format!("Investigations: {}", h.investigation_count));
            lines.push(format!("Observations: {}", h.observation_count));
            lines.push(format!("Memory records: {}", h.memory_count));
            lines.push(format!("Migration: {}", h.migration_status));
            lines.push(format!("Disk bytes: {}", h.disk_bytes));
            for note in h.notes.iter().take(6) {
                lines.push(format!("• {note}"));
            }
            if !h.corrupt_records.is_empty() {
                lines.push(format!(
                    "Corrupt records isolated: {}",
                    h.corrupt_records.len()
                ));
            }
        }
        Err(e) => {
            let view = map_error(&e);
            lines.push(view.title);
            lines.push(view.summary);
        }
    }
    let coverage = caps.capability_coverage_report();
    lines.push(format!("Capability coverage: {}", coverage.summary));
    if !coverage.all_first_party_registered {
        lines.push(format!("Gaps: {}", coverage.gaps.join("; ")));
    }
    lines.push("Recovery: rivora doctor health".into());
    IntentExecutionResult::Panel {
        title: "Doctor".into(),
        lines,
        route: WorkspaceRoute::Doctor,
    }
}

fn connectors_panel() -> IntentExecutionResult {
    use rivora_connectors::github_actions::GitHubActionsConnector;
    use rivora_connectors::kubernetes::KubernetesConnector;
    use rivora_connectors::sentry::SentryConnector;

    let mut lines = Vec::new();
    lines.push("local — [local] configured=true read_only=true".into());
    lines.push("  Local project observation (no credentials)".into());
    for status in [
        GitHubActionsConnector::new("owner/repo").status(),
        KubernetesConnector::new("default").status(),
        SentryConnector::new("org", "project").status(),
    ] {
        lines.push(format!(
            "{} [{}] configured={} read_only={}",
            status.id, status.category, status.configured, status.read_only
        ));
        lines.push(format!("  {}", status.details));
    }
    lines.push("Token: never printed".into());
    lines.push("Fixture mode: available where connectors support it".into());
    lines.push("Connectors observe; they do not evaluate or execute.".into());
    IntentExecutionResult::Panel {
        title: "Connectors".into(),
        lines,
        route: WorkspaceRoute::Connectors,
    }
}

fn test_connector(id: &str) -> IntentExecutionResult {
    IntentExecutionResult::Info {
        title: format!("Test connector: {id}"),
        body: format!(
            "Connector `{id}` test requested.\n\
             Status only — no secrets are printed.\n\
             Configure credentials outside Rivora conversation state."
        ),
        route: Some(WorkspaceRoute::Connectors),
    }
}

fn prior_outcomes(caps: &CapabilityService) -> IntentExecutionResult {
    match caps.recall_prior_outcomes(Default::default()) {
        Ok(items) => {
            let mut body = String::new();
            for (i, o) in items.iter().take(DEFAULT_LIST_LIMIT).enumerate() {
                body.push_str(&format!(
                    "{}. [{}] {} — {}\n",
                    i + 1,
                    o.outcome.disposition.as_str(),
                    o.investigation_title,
                    o.recommendation_summary
                        .clone()
                        .unwrap_or_else(|| o.outcome.notes.clone())
                ));
            }
            if body.is_empty() {
                body = "No prior outcomes yet.".into();
            }
            IntentExecutionResult::Info {
                title: "Prior Outcomes".into(),
                body,
                route: Some(WorkspaceRoute::Learning),
            }
        }
        Err(e) => IntentExecutionResult::Error(map_error(&e)),
    }
}

fn patterns(caps: &CapabilityService) -> IntentExecutionResult {
    match caps.detect_patterns("workspace") {
        Ok(items) => {
            let mut body = String::new();
            for (i, p) in items.iter().take(DEFAULT_LIST_LIMIT).enumerate() {
                body.push_str(&format!(
                    "{}. {} (n={})\n",
                    i + 1,
                    p.description,
                    p.occurrence_count
                ));
            }
            if body.is_empty() {
                body = "No patterns detected yet.".into();
            }
            IntentExecutionResult::Info {
                title: "Patterns".into(),
                body,
                route: None,
            }
        }
        Err(e) => IntentExecutionResult::Error(map_error(&e)),
    }
}

fn trends(caps: &CapabilityService) -> IntentExecutionResult {
    match caps.summarize_historical_trend(None) {
        Ok(t) => IntentExecutionResult::Info {
            title: "Historical Trends".into(),
            body: format!(
                "{}\nInvestigations: {}\nMethod: {}",
                t.summary, t.investigation_count, t.derivation_method
            ),
            route: None,
        },
        Err(e) => IntentExecutionResult::Error(map_error(&e)),
    }
}

fn list_investigations(caps: &CapabilityService) -> IntentExecutionResult {
    match load_investigation_items(caps, None) {
        Ok(items) => {
            let n = items.len();
            IntentExecutionResult::InvestigationList {
                items,
                summary: format!("Showing {n} investigation(s). Select one or search."),
            }
        }
        Err(e) => IntentExecutionResult::Error(*e),
    }
}

fn search_investigations(caps: &CapabilityService, query: &str) -> IntentExecutionResult {
    if query.trim().is_empty() {
        return list_investigations(caps);
    }
    let sq = SearchQuery {
        text: Some(query.to_string()),
        limit: Some(DEFAULT_LIST_LIMIT),
        ..SearchQuery::default()
    };
    match caps.search_investigations(sq) {
        Ok(results) => {
            let items: Vec<InvestigationListItem> = results
                .into_iter()
                .filter_map(|r| {
                    caps.open_investigation(r.investigation_id).ok().map(|inv| {
                        InvestigationListItem {
                            id: inv.id,
                            title: inv.title,
                            status: inv.status.as_str().to_string(),
                            updated_at: inv.updated_at.to_rfc3339(),
                            score: Some(r.score),
                        }
                    })
                })
                .collect();
            let n = items.len();
            IntentExecutionResult::SearchResults {
                query: query.to_string(),
                items,
                summary: format!("{n} result(s) for “{query}”."),
            }
        }
        Err(e) => IntentExecutionResult::Error(map_error(&e)),
    }
}

fn load_investigation_items(
    caps: &CapabilityService,
    limit: Option<usize>,
) -> Result<Vec<InvestigationListItem>, Box<WorkspaceErrorView>> {
    let ids = caps
        .list_investigations()
        .map_err(|e| Box::new(map_error(&e)))?;
    let lim = limit.unwrap_or(DEFAULT_LIST_LIMIT);
    let mut items = Vec::new();
    for id in ids.into_iter().take(lim) {
        match caps.open_investigation(id) {
            Ok(inv) => items.push(InvestigationListItem {
                id: inv.id,
                title: inv.title,
                status: inv.status.as_str().to_string(),
                updated_at: inv.updated_at.to_rfc3339(),
                score: None,
            }),
            Err(_) => {
                // Corrupt isolation — skip one bad record.
                continue;
            }
        }
    }
    items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(items)
}

fn create_investigation(
    caps: &CapabilityService,
    draft: &InvestigationDraft,
) -> IntentExecutionResult {
    match caps.create_investigation(draft.title.clone(), draft.description.clone(), "workspace") {
        Ok(inv) => {
            let sources = if draft.suggested_sources.is_empty() {
                String::new()
            } else {
                format!(
                    "\nSuggested sources: {}",
                    draft.suggested_sources.join(", ")
                )
            };
            IntentExecutionResult::InvestigationCreated {
                id: inv.id,
                title: inv.title.clone(),
                summary: format!(
                    "Created Investigation {} [{}].{sources}\nConversation projects this object; history lives on the Investigation.",
                    inv.id,
                    inv.status.as_str()
                ),
            }
        }
        Err(e) => IntentExecutionResult::Error(map_error(&e)),
    }
}

fn open_investigation(
    caps: &CapabilityService,
    id: rivora::domain::InvestigationId,
) -> IntentExecutionResult {
    match caps.open_investigation(id) {
        Ok(inv) => {
            let timeline = caps.generate_timeline(id).map(|t| t.len()).unwrap_or(0);
            IntentExecutionResult::InvestigationOpened {
                id: inv.id,
                title: inv.title.clone(),
                status: inv.status.as_str().to_string(),
                summary: format!(
                    "Opened “{}” [{}]\nTimeline entries: {timeline}\nId: {}",
                    inv.title,
                    inv.status.as_str(),
                    inv.id
                ),
            }
        }
        Err(e) => IntentExecutionResult::Error(map_error(&e)),
    }
}

fn add_observation(
    caps: &CapabilityService,
    id: rivora::domain::InvestigationId,
    summary: &str,
) -> IntentExecutionResult {
    match caps.ingest_observation(
        id,
        ObservationKind::UserInput,
        summary,
        serde_json::json!({"source": "workspace"}),
        "workspace",
        Utc::now(),
        None,
        "workspace",
    ) {
        Ok((obs, _mem, replay)) => IntentExecutionResult::CapabilityWork {
            title: "Observation recorded".into(),
            body: format!(
                "{}\nId: {}\nIdempotent replay: {replay}",
                obs.summary, obs.id
            ),
            investigation_id: Some(id),
            object_refs: vec![obs.id.to_string()],
            route: Some(WorkspaceRoute::Investigation),
        },
        Err(e) => IntentExecutionResult::Error(map_error(&e)),
    }
}

fn run_evaluation(
    caps: &CapabilityService,
    id: rivora::domain::InvestigationId,
) -> IntentExecutionResult {
    match caps.evaluate_investigation(id, "workspace") {
        Ok(evals) => {
            let mut body = String::new();
            let mut refs = Vec::new();
            for e in &evals {
                body.push_str(&format!("• {} ({})\n", e.summary, e.id));
                refs.push(e.id.to_string());
            }
            if body.is_empty() {
                body = "No evaluations produced.".into();
            }
            IntentExecutionResult::CapabilityWork {
                title: "Evaluation completed".into(),
                body,
                investigation_id: Some(id),
                object_refs: refs,
                route: Some(WorkspaceRoute::Investigation),
            }
        }
        Err(e) => IntentExecutionResult::Error(map_error(&e)),
    }
}

fn run_verification(
    caps: &CapabilityService,
    id: rivora::domain::InvestigationId,
) -> IntentExecutionResult {
    match caps.verify_all(id, "workspace") {
        Ok(items) => {
            let mut body = String::new();
            let mut refs = Vec::new();
            for v in &items {
                body.push_str(&format!(
                    "• {} — {} ({})\n",
                    v.result.as_str(),
                    v.subject,
                    v.reason
                ));
                refs.push(v.id.to_string());
            }
            if body.is_empty() {
                body = "No verifications produced.".into();
            }
            IntentExecutionResult::CapabilityWork {
                title: "Verification completed".into(),
                body,
                investigation_id: Some(id),
                object_refs: refs,
                route: Some(WorkspaceRoute::Investigation),
            }
        }
        Err(e) => IntentExecutionResult::Error(map_error(&e)),
    }
}

fn run_recommend(
    caps: &CapabilityService,
    id: rivora::domain::InvestigationId,
) -> IntentExecutionResult {
    match caps.generate_recommendation(id, "workspace") {
        Ok(items) => {
            let mut body = String::new();
            let mut refs = Vec::new();
            for r in &items {
                body.push_str(&format!("• {} [{}]\n", r.summary, r.status.as_str()));
                refs.push(r.id.to_string());
            }
            if body.is_empty() {
                body = "No recommendations produced.".into();
            }
            IntentExecutionResult::CapabilityWork {
                title: "Recommendations".into(),
                body,
                investigation_id: Some(id),
                object_refs: refs,
                route: Some(WorkspaceRoute::Investigation),
            }
        }
        Err(e) => IntentExecutionResult::Error(map_error(&e)),
    }
}

fn create_proposal(
    caps: &CapabilityService,
    id: rivora::domain::InvestigationId,
) -> IntentExecutionResult {
    let inv = match caps.open_investigation(id) {
        Ok(i) => i,
        Err(e) => return IntentExecutionResult::Error(map_error(&e)),
    };
    let req = CreateProposalRequest {
        title: format!("Improve: {}", inv.title),
        summary: format!("Proposed improvement derived from “{}”", inv.title),
        rationale: "Generated from Workspace Create Proposal action. Proposal only — not applied."
            .into(),
        category: ProposalCategory::Reliability,
        priority: ProposalPriority::Medium,
        confidence: Confidence::new(0.6),
        supporting_evidence_ids: Vec::new(),
        contradicting_evidence_ids: Vec::new(),
        source_recommendation_ids: Vec::new(),
        affected_components: Vec::new(),
        affected_resources: Vec::new(),
    };
    match caps.create_improvement_proposal(id, req, "workspace") {
        Ok(p) => {
            // Advance to Proposed for review surface when still Draft.
            let p = if p.status == ProposalStatus::Draft {
                caps.update_improvement_proposal_status(
                    id,
                    p.id,
                    ProposalStatus::Proposed,
                    "workspace",
                    "workspace submit for review",
                    ProposalTransitionAuthority::ExternalCaller,
                )
                .unwrap_or(p)
            } else {
                p
            };
            IntentExecutionResult::CapabilityWork {
                title: "Proposal created".into(),
                body: format!(
                    "{} [{}]\n{}\nProposal only — not applied, not implemented, not verified.\nId: {}",
                    p.title,
                    p.status.as_str(),
                    p.summary,
                    p.id
                ),
                investigation_id: Some(id),
                object_refs: vec![p.id.to_string()],
                route: Some(WorkspaceRoute::ProposalReview),
            }
        }
        Err(e) => IntentExecutionResult::Error(map_error(&e)),
    }
}

fn review_proposals(
    caps: &CapabilityService,
    id: rivora::domain::InvestigationId,
) -> IntentExecutionResult {
    match caps.list_improvement_proposals(id) {
        Ok(listing) => {
            let mut body =
                String::from("Proposals are candidates only. Acceptance ≠ execution approval.\n\n");
            let mut refs = Vec::new();
            for p in listing.proposals.iter().take(DEFAULT_LIST_LIMIT) {
                body.push_str(&format!(
                    "• {} [{} / {}] {}\n",
                    p.id,
                    p.status.as_str(),
                    p.priority.as_str(),
                    p.title
                ));
                refs.push(p.id.to_string());
            }
            if refs.is_empty() {
                body.push_str("No proposals yet. Use Create Proposal.\n");
            }
            IntentExecutionResult::CapabilityWork {
                title: "Proposal review".into(),
                body,
                investigation_id: Some(id),
                object_refs: refs,
                route: Some(WorkspaceRoute::ProposalReview),
            }
        }
        Err(e) => IntentExecutionResult::Error(map_error(&e)),
    }
}

fn review_executions(
    caps: &CapabilityService,
    id: rivora::domain::InvestigationId,
) -> IntentExecutionResult {
    match caps.list_execution_plans(id) {
        Ok(listing) => {
            let mut body = String::from(
                "Execution Plans require exact-revision approval before live runs.\n\n",
            );
            let mut refs = Vec::new();
            for p in listing.plans.iter().take(DEFAULT_LIST_LIMIT) {
                body.push_str(&format!(
                    "• plan {} rev {} [{}] capability={}\n",
                    p.id,
                    p.revision_number,
                    p.status.as_str(),
                    p.capability_id
                ));
                refs.push(p.id.to_string());
            }
            if refs.is_empty() {
                body.push_str("No execution plans yet.\n");
            }
            IntentExecutionResult::CapabilityWork {
                title: "Execution review".into(),
                body,
                investigation_id: Some(id),
                object_refs: refs,
                route: Some(WorkspaceRoute::ExecutionReview),
            }
        }
        Err(e) => IntentExecutionResult::Error(map_error(&e)),
    }
}

fn show_learning(
    caps: &CapabilityService,
    investigation_id: Option<rivora::domain::InvestigationId>,
) -> IntentExecutionResult {
    if let Some(id) = investigation_id {
        match caps.list_measured_learning_outcomes(id) {
            Ok(listing) => {
                let mut body = String::new();
                for o in listing.outcomes.iter().take(DEFAULT_LIST_LIMIT) {
                    body.push_str(&format!("• {} [{}]\n", o.id, o.status.as_str()));
                }
                if body.is_empty() {
                    body = "No measured learning outcomes for this Investigation.".into();
                }
                IntentExecutionResult::CapabilityWork {
                    title: "Learning".into(),
                    body,
                    investigation_id: Some(id),
                    object_refs: vec![],
                    route: Some(WorkspaceRoute::Learning),
                }
            }
            Err(e) => IntentExecutionResult::Error(map_error(&e)),
        }
    } else {
        match caps.list_learning_patterns() {
            Ok(patterns) => {
                let mut body = String::new();
                for p in patterns.iter().take(DEFAULT_LIST_LIMIT) {
                    body.push_str(&format!("• {} ({})\n", p.title, p.signature));
                }
                if body.is_empty() {
                    body = "No learning patterns yet.".into();
                }
                IntentExecutionResult::Info {
                    title: "Learning patterns".into(),
                    body,
                    route: Some(WorkspaceRoute::Learning),
                }
            }
            Err(e) => IntentExecutionResult::Error(map_error(&e)),
        }
    }
}

fn agent_handoff(
    caps: &CapabilityService,
    investigation_id: rivora::domain::InvestigationId,
    proposal_id: rivora::domain::ObjectId,
) -> IntentExecutionResult {
    match crate::agent_handoff::prepare_handoff(caps, investigation_id, proposal_id) {
        Ok(h) => IntentExecutionResult::Info {
            title: "Coding-agent handoff (bounded)".into(),
            body: format!(
                "{}\n\nThis does not grant execution authority.\nSecrets excluded.\nConfirmation required before sending externally.",
                h.preview
            ),
            route: Some(WorkspaceRoute::ProposalReview),
        },
        Err(e) => IntentExecutionResult::Error(*e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rivora::storage::LocalStore;
    use rivora::{MockExecutionCapability, Runtime};
    use rivora_connectors::register_first_party_github_execution_capabilities;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn caps() -> CapabilityService {
        let dir = tempdir().unwrap();
        let store = LocalStore::open(dir.path()).unwrap();
        let runtime = Arc::new(Runtime::new(Arc::new(store)));
        runtime
            .register_execution_capability(Arc::new(MockExecutionCapability::new()))
            .unwrap();
        register_first_party_github_execution_capabilities(runtime.execution_registry()).unwrap();
        // Keep tempdir alive for duration of test by leaking — only for unit tests.
        std::mem::forget(dir);
        CapabilityService::new(runtime)
    }

    #[test]
    fn create_and_open_investigation() {
        let caps = caps();
        let draft = InvestigationDraft {
            title: "Latency spike".into(),
            description: Some("after deploy".into()),
            suggested_sources: vec!["GitHub Actions".into()],
        };
        let r = execute_intent(&caps, &WorkspaceIntent::CreateInvestigation { draft });
        let id = match r {
            IntentExecutionResult::InvestigationCreated { id, .. } => id,
            other => panic!("unexpected {other:?}"),
        };
        let r2 = execute_intent(
            &caps,
            &WorkspaceIntent::OpenInvestigation {
                investigation_id: id,
            },
        );
        assert!(matches!(
            r2,
            IntentExecutionResult::InvestigationOpened { .. }
        ));
    }

    #[test]
    fn quit_is_ui_only() {
        let caps = caps();
        assert!(matches!(
            execute_intent(&caps, &WorkspaceIntent::Quit),
            IntentExecutionResult::Quit
        ));
    }
}
