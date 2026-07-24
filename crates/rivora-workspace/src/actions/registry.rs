//! Canonical Workspace action registry — one list, many surfaces.
#![allow(dead_code)]

use crate::intent::{ContextRequirement, WorkspaceActionId, WorkspaceIntent, WorkspaceRoute};

/// Action category for palette grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceActionCategory {
    Navigation,
    Investigation,
    EngineeringLoop,
    Connectors,
    System,
}

impl WorkspaceActionCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Navigation => "Navigation",
            Self::Investigation => "Investigation",
            Self::EngineeringLoop => "Engineering Loop",
            Self::Connectors => "Connectors",
            Self::System => "System",
        }
    }
}

/// Whether an action can run in the current context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionAvailability {
    Available,
    Disabled { reason: String },
}

impl ActionAvailability {
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }
}

/// Descriptor for a discoverable Workspace action.
#[derive(Debug, Clone)]
pub struct WorkspaceActionDescriptor {
    pub id: WorkspaceActionId,
    pub label: &'static str,
    pub aliases: &'static [&'static str],
    pub description: &'static str,
    pub category: WorkspaceActionCategory,
    pub required_context: &'static [ContextRequirement],
    /// Build the intent for this action given optional active investigation.
    pub intent_builder: fn(ActionContext<'_>) -> Option<WorkspaceIntent>,
}

/// Runtime context used to evaluate availability and build intents.
#[derive(Debug, Clone, Copy)]
pub struct ActionContext<'a> {
    pub active_investigation: Option<rivora::domain::InvestigationId>,
    pub has_selected_proposal: bool,
    pub has_selected_plan: bool,
    pub filter: &'a str,
}

/// Return the full action registry (shared by `/` and Ctrl+P).
pub fn action_registry() -> &'static [WorkspaceActionDescriptor] {
    &ACTIONS
}

/// Filter and rank actions for the palette.
pub fn filter_actions(
    query: &str,
    ctx: ActionContext<'_>,
) -> Vec<(WorkspaceActionDescriptor, ActionAvailability)> {
    let q = query.trim().trim_start_matches('/').to_lowercase();
    let mut out: Vec<(WorkspaceActionDescriptor, ActionAvailability, i32)> = action_registry()
        .iter()
        .filter_map(|desc| {
            let score = match_score(desc, &q);
            if !q.is_empty() && score < 0 {
                return None;
            }
            let availability = availability(desc, ctx);
            // Context-aware boost: actions usable now sort first.
            let mut rank = score;
            if availability.is_available() {
                rank += 10;
            }
            if desc
                .required_context
                .contains(&ContextRequirement::ActiveInvestigation)
                && ctx.active_investigation.is_some()
            {
                rank += 5;
            }
            Some((desc.clone(), availability, rank))
        })
        .collect();
    out.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.label.cmp(b.0.label)));
    out.into_iter().map(|(d, a, _)| (d, a)).collect()
}

fn match_score(desc: &WorkspaceActionDescriptor, q: &str) -> i32 {
    if q.is_empty() {
        return 0;
    }
    let label = desc.label.to_lowercase();
    let id = desc.id.as_str();
    if label == q || id == q {
        return 100;
    }
    if label.starts_with(q) || id.starts_with(q) {
        return 80;
    }
    if label.contains(q) || id.contains(q) {
        return 50;
    }
    for alias in desc.aliases {
        let a = alias.to_lowercase();
        if a == q {
            return 90;
        }
        if a.starts_with(q) || a.contains(q) {
            return 60;
        }
    }
    if desc.description.to_lowercase().contains(q) {
        return 20;
    }
    -1
}

fn availability(desc: &WorkspaceActionDescriptor, ctx: ActionContext<'_>) -> ActionAvailability {
    for req in desc.required_context {
        match req {
            ContextRequirement::ActiveInvestigation if ctx.active_investigation.is_none() => {
                return ActionAvailability::Disabled {
                    reason: "Open an Investigation first".into(),
                };
            }
            ContextRequirement::SelectedProposal if !ctx.has_selected_proposal => {
                return ActionAvailability::Disabled {
                    reason: "Select a Proposal first".into(),
                };
            }
            ContextRequirement::SelectedExecutionPlan if !ctx.has_selected_plan => {
                return ActionAvailability::Disabled {
                    reason: "Select an Execution Plan first".into(),
                };
            }
            _ => {}
        }
    }
    ActionAvailability::Available
}

fn need_inv(ctx: ActionContext<'_>) -> Option<rivora::domain::InvestigationId> {
    ctx.active_investigation
}

static ACTIONS: [WorkspaceActionDescriptor; 23] = [
    WorkspaceActionDescriptor {
        id: WorkspaceActionId::Home,
        label: "Home",
        aliases: &["home", "start"],
        description: "Conversation-first home screen",
        category: WorkspaceActionCategory::Navigation,
        required_context: &[],
        intent_builder: |_| Some(WorkspaceIntent::OpenHome),
    },
    WorkspaceActionDescriptor {
        id: WorkspaceActionId::CreateInvestigation,
        label: "Create Investigation",
        aliases: &["create", "new", "investigate"],
        description: "Start a new Investigation",
        category: WorkspaceActionCategory::Investigation,
        required_context: &[],
        intent_builder: |_| {
            Some(WorkspaceIntent::CreateInvestigation {
                draft: crate::intent::InvestigationDraft {
                    title: "New Investigation".into(),
                    description: None,
                    suggested_sources: vec!["Local project".into()],
                },
            })
        },
    },
    WorkspaceActionDescriptor {
        id: WorkspaceActionId::OpenInvestigation,
        label: "Open Investigation",
        aliases: &["open"],
        description: "Open from recent or search results",
        category: WorkspaceActionCategory::Investigation,
        required_context: &[],
        intent_builder: |_| Some(WorkspaceIntent::ListInvestigations),
    },
    WorkspaceActionDescriptor {
        id: WorkspaceActionId::ListInvestigations,
        label: "List Investigations",
        aliases: &["list", "ls"],
        description: "Browse recent Investigations",
        category: WorkspaceActionCategory::Investigation,
        required_context: &[],
        intent_builder: |_| Some(WorkspaceIntent::ListInvestigations),
    },
    WorkspaceActionDescriptor {
        id: WorkspaceActionId::SearchInvestigations,
        label: "Search Investigations",
        aliases: &["search", "find", "fuzzy"],
        description: "Fuzzy search by title, repo, status",
        category: WorkspaceActionCategory::Investigation,
        required_context: &[],
        intent_builder: |_| {
            Some(WorkspaceIntent::SearchInvestigations {
                query: String::new(),
            })
        },
    },
    WorkspaceActionDescriptor {
        id: WorkspaceActionId::PriorOutcomes,
        label: "Prior Outcomes",
        aliases: &["outcomes"],
        description: "Recall prior Measured Learning Outcomes",
        category: WorkspaceActionCategory::EngineeringLoop,
        required_context: &[],
        intent_builder: |_| Some(WorkspaceIntent::ShowPriorOutcomes),
    },
    WorkspaceActionDescriptor {
        id: WorkspaceActionId::Patterns,
        label: "Patterns",
        aliases: &["pattern"],
        description: "Detected engineering patterns",
        category: WorkspaceActionCategory::EngineeringLoop,
        required_context: &[],
        intent_builder: |_| Some(WorkspaceIntent::ShowPatterns),
    },
    WorkspaceActionDescriptor {
        id: WorkspaceActionId::HistoricalTrends,
        label: "Historical Trends",
        aliases: &["trends"],
        description: "Historical trend summary",
        category: WorkspaceActionCategory::EngineeringLoop,
        required_context: &[],
        intent_builder: |_| Some(WorkspaceIntent::ShowHistoricalTrends),
    },
    WorkspaceActionDescriptor {
        id: WorkspaceActionId::Connectors,
        label: "Connectors",
        aliases: &["connector", "github", "k8s", "sentry"],
        description: "Connector status and tests",
        category: WorkspaceActionCategory::Connectors,
        required_context: &[],
        intent_builder: |_| Some(WorkspaceIntent::ShowConnectors),
    },
    WorkspaceActionDescriptor {
        id: WorkspaceActionId::Observe,
        label: "Observe",
        aliases: &["observation"],
        description: "Add an observation to the active Investigation",
        category: WorkspaceActionCategory::EngineeringLoop,
        required_context: &[ContextRequirement::ActiveInvestigation],
        intent_builder: |ctx| {
            need_inv(ctx).map(|id| WorkspaceIntent::AddObservation {
                investigation_id: id,
                summary: "Manual observation".into(),
            })
        },
    },
    WorkspaceActionDescriptor {
        id: WorkspaceActionId::Evaluate,
        label: "Evaluate",
        aliases: &["eval", "evaluation"],
        description: "Run evaluation on the active Investigation",
        category: WorkspaceActionCategory::EngineeringLoop,
        required_context: &[ContextRequirement::ActiveInvestigation],
        intent_builder: |ctx| {
            need_inv(ctx).map(|id| WorkspaceIntent::RunEvaluation {
                investigation_id: id,
            })
        },
    },
    WorkspaceActionDescriptor {
        id: WorkspaceActionId::Verify,
        label: "Verify",
        aliases: &["verification"],
        description: "Run verification on conclusions",
        category: WorkspaceActionCategory::EngineeringLoop,
        required_context: &[ContextRequirement::ActiveInvestigation],
        intent_builder: |ctx| {
            need_inv(ctx).map(|id| WorkspaceIntent::RunVerification {
                investigation_id: id,
            })
        },
    },
    WorkspaceActionDescriptor {
        id: WorkspaceActionId::Recommend,
        label: "Recommend",
        aliases: &["recommendation"],
        description: "Generate recommendations",
        category: WorkspaceActionCategory::EngineeringLoop,
        required_context: &[ContextRequirement::ActiveInvestigation],
        intent_builder: |ctx| {
            need_inv(ctx).map(|id| WorkspaceIntent::GenerateRecommendation {
                investigation_id: id,
            })
        },
    },
    WorkspaceActionDescriptor {
        id: WorkspaceActionId::CreateProposal,
        label: "Create Proposal",
        aliases: &["proposal", "propose"],
        description: "Create an Improvement Proposal (not applied)",
        category: WorkspaceActionCategory::EngineeringLoop,
        required_context: &[ContextRequirement::ActiveInvestigation],
        intent_builder: |ctx| {
            need_inv(ctx).map(|id| WorkspaceIntent::CreateProposal {
                investigation_id: id,
            })
        },
    },
    WorkspaceActionDescriptor {
        id: WorkspaceActionId::ReviewProposals,
        label: "Review Proposals",
        aliases: &["proposals"],
        description: "Review Improvement Proposals",
        category: WorkspaceActionCategory::EngineeringLoop,
        required_context: &[ContextRequirement::ActiveInvestigation],
        intent_builder: |ctx| {
            need_inv(ctx).map(|id| WorkspaceIntent::ReviewProposals {
                investigation_id: id,
            })
        },
    },
    WorkspaceActionDescriptor {
        id: WorkspaceActionId::CreateExecutionPlan,
        label: "Create Execution Plan",
        aliases: &["execute", "execution", "plan"],
        description: "Create Execution Plan from an accepted Proposal",
        category: WorkspaceActionCategory::EngineeringLoop,
        required_context: &[
            ContextRequirement::ActiveInvestigation,
            ContextRequirement::SelectedProposal,
        ],
        intent_builder: |_| None, // requires proposal selection in app layer
    },
    WorkspaceActionDescriptor {
        id: WorkspaceActionId::ReviewExecutions,
        label: "Review Executions",
        aliases: &["executions", "plans"],
        description: "Review Execution Plans, Attempts, Receipts",
        category: WorkspaceActionCategory::EngineeringLoop,
        required_context: &[ContextRequirement::ActiveInvestigation],
        intent_builder: |ctx| {
            need_inv(ctx).map(|id| WorkspaceIntent::ReviewExecutions {
                investigation_id: id,
            })
        },
    },
    WorkspaceActionDescriptor {
        id: WorkspaceActionId::Learning,
        label: "Learning",
        aliases: &["learn"],
        description: "Measured Learning Outcomes",
        category: WorkspaceActionCategory::EngineeringLoop,
        required_context: &[],
        intent_builder: |ctx| {
            Some(WorkspaceIntent::ShowLearning {
                investigation_id: ctx.active_investigation,
            })
        },
    },
    WorkspaceActionDescriptor {
        id: WorkspaceActionId::AgentHandoff,
        label: "Agent Handoff",
        aliases: &["agent", "handoff", "coding-agent"],
        description: "Prepare typed coding-agent handoff (no auto-execution)",
        category: WorkspaceActionCategory::EngineeringLoop,
        required_context: &[
            ContextRequirement::ActiveInvestigation,
            ContextRequirement::SelectedProposal,
        ],
        intent_builder: |_| None,
    },
    WorkspaceActionDescriptor {
        id: WorkspaceActionId::Doctor,
        label: "Doctor",
        aliases: &["health", "diagnostics"],
        description: "Runtime health and recovery guidance",
        category: WorkspaceActionCategory::System,
        required_context: &[],
        intent_builder: |_| Some(WorkspaceIntent::ShowDoctor),
    },
    WorkspaceActionDescriptor {
        id: WorkspaceActionId::Settings,
        label: "Settings",
        aliases: &["prefs", "preferences"],
        description: "Workspace preferences",
        category: WorkspaceActionCategory::System,
        required_context: &[],
        intent_builder: |_| Some(WorkspaceIntent::OpenSettings),
    },
    WorkspaceActionDescriptor {
        id: WorkspaceActionId::Help,
        label: "Help",
        aliases: &["?", "shortcuts"],
        description: "Keyboard shortcuts and Workspace help",
        category: WorkspaceActionCategory::System,
        required_context: &[],
        intent_builder: |_| Some(WorkspaceIntent::OpenHelp),
    },
    WorkspaceActionDescriptor {
        id: WorkspaceActionId::Quit,
        label: "Quit",
        aliases: &["exit", "q"],
        description: "Leave the Workspace safely",
        category: WorkspaceActionCategory::System,
        required_context: &[],
        intent_builder: |_| Some(WorkspaceIntent::Quit),
    },
];

// Silence unused import in static builders
const _: WorkspaceRoute = WorkspaceRoute::Home;

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(filter: &str) -> ActionContext<'_> {
        ActionContext {
            active_investigation: None,
            has_selected_proposal: false,
            has_selected_plan: false,
            filter,
        }
    }

    #[test]
    fn slash_and_ctrlp_share_one_registry() {
        let a = action_registry();
        assert!(a.len() >= 20);
        assert!(a
            .iter()
            .any(|d| d.id == WorkspaceActionId::CreateInvestigation));
        assert!(a.iter().any(|d| d.id == WorkspaceActionId::Quit));
    }

    #[test]
    fn filter_create() {
        let results = filter_actions("create", ctx("create"));
        assert!(!results.is_empty());
        assert_eq!(results[0].0.id, WorkspaceActionId::CreateInvestigation);
    }

    #[test]
    fn evaluate_disabled_without_investigation() {
        let results = filter_actions("evaluate", ctx("evaluate"));
        let eval = results
            .iter()
            .find(|(d, _)| d.id == WorkspaceActionId::Evaluate)
            .expect("evaluate action");
        assert!(!eval.1.is_available());
    }

    #[test]
    fn evaluate_available_with_investigation() {
        let id = rivora::domain::InvestigationId::new();
        let c = ActionContext {
            active_investigation: Some(id),
            has_selected_proposal: false,
            has_selected_plan: false,
            filter: "evaluate",
        };
        let results = filter_actions("evaluate", c);
        let eval = results
            .iter()
            .find(|(d, _)| d.id == WorkspaceActionId::Evaluate)
            .expect("evaluate");
        assert!(eval.1.is_available());
    }
}
