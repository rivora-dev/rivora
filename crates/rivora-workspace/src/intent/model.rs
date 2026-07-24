//! Typed Workspace intent model.
//!
//! Natural-language text is interpreted into these variants. Execution always
//! goes through Capabilities — intents never grant Runtime authority by themselves.
#![allow(dead_code)]

use rivora::domain::{InvestigationId, ObjectId};
use serde::{Deserialize, Serialize};

/// Stable identifier for a discoverable Workspace action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceActionId {
    CreateInvestigation,
    OpenInvestigation,
    ListInvestigations,
    SearchInvestigations,
    PriorOutcomes,
    Patterns,
    HistoricalTrends,
    Connectors,
    Observe,
    Evaluate,
    Verify,
    Recommend,
    CreateProposal,
    ReviewProposals,
    CreateExecutionPlan,
    ReviewExecutions,
    Learning,
    Doctor,
    Settings,
    Help,
    Quit,
    AgentHandoff,
    Home,
}

impl WorkspaceActionId {
    /// Stable string key for tests and persistence.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CreateInvestigation => "create_investigation",
            Self::OpenInvestigation => "open_investigation",
            Self::ListInvestigations => "list_investigations",
            Self::SearchInvestigations => "search_investigations",
            Self::PriorOutcomes => "prior_outcomes",
            Self::Patterns => "patterns",
            Self::HistoricalTrends => "historical_trends",
            Self::Connectors => "connectors",
            Self::Observe => "observe",
            Self::Evaluate => "evaluate",
            Self::Verify => "verify",
            Self::Recommend => "recommend",
            Self::CreateProposal => "create_proposal",
            Self::ReviewProposals => "review_proposals",
            Self::CreateExecutionPlan => "create_execution_plan",
            Self::ReviewExecutions => "review_executions",
            Self::Learning => "learning",
            Self::Doctor => "doctor",
            Self::Settings => "settings",
            Self::Help => "help",
            Self::Quit => "quit",
            Self::AgentHandoff => "agent_handoff",
            Self::Home => "home",
        }
    }
}

/// Draft used when creating an Investigation from the Workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvestigationDraft {
    pub title: String,
    pub description: Option<String>,
    pub suggested_sources: Vec<String>,
}

/// Context the Workspace needs before an intent can execute fully.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextRequirement {
    ActiveInvestigation,
    SelectedProposal,
    SelectedExecutionPlan,
    SearchQuery,
    Confirmation,
    ConnectorId,
}

/// Confidence in a natural-language interpretation.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct IntentConfidence(pub f32);

impl IntentConfidence {
    pub fn new(value: f32) -> Self {
        Self(value.clamp(0.0, 1.0))
    }

    pub fn is_high(self) -> bool {
        self.0 >= 0.75
    }

    pub fn is_low(self) -> bool {
        self.0 < 0.45
    }
}

/// Typed Workspace intent. UI language disappears at this boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkspaceIntent {
    /// Free-form prompt still awaiting interpretation (should not reach Runtime).
    SubmitPrompt {
        text: String,
    },
    CreateInvestigation {
        draft: InvestigationDraft,
    },
    OpenInvestigation {
        investigation_id: InvestigationId,
    },
    ListInvestigations,
    SearchInvestigations {
        query: String,
    },
    ShowPriorOutcomes,
    ShowPatterns,
    ShowHistoricalTrends,
    ShowConnectors,
    TestConnector {
        connector_id: String,
    },
    AddObservation {
        investigation_id: InvestigationId,
        summary: String,
    },
    RunEvaluation {
        investigation_id: InvestigationId,
    },
    RunVerification {
        investigation_id: InvestigationId,
    },
    GenerateRecommendation {
        investigation_id: InvestigationId,
    },
    CreateProposal {
        investigation_id: InvestigationId,
    },
    ReviewProposal {
        proposal_id: ObjectId,
    },
    ReviewProposals {
        investigation_id: InvestigationId,
    },
    CreateExecutionPlan {
        investigation_id: InvestigationId,
        proposal_id: ObjectId,
    },
    ReviewExecution {
        plan_id: ObjectId,
    },
    ReviewExecutions {
        investigation_id: InvestigationId,
    },
    ShowLearning {
        investigation_id: Option<InvestigationId>,
    },
    ShowDoctor,
    OpenHelp,
    OpenSettings,
    OpenHome,
    AgentHandoff {
        investigation_id: InvestigationId,
        proposal_id: ObjectId,
    },
    Quit,
    /// Navigate UI without Runtime mutation.
    Navigate {
        route: WorkspaceRoute,
    },
    /// Confirm a pending mutating intent.
    ConfirmPending,
    /// Cancel a pending confirmation or task.
    CancelPending,
}

/// High-level Workspace routes (presentation only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceRoute {
    #[default]
    Home,
    Investigation,
    Search,
    ProposalReview,
    ExecutionReview,
    Connectors,
    Doctor,
    Learning,
    Settings,
    Help,
}

impl WorkspaceIntent {
    /// Whether executing this intent may mutate durable Runtime state.
    pub fn is_mutating(&self) -> bool {
        matches!(
            self,
            Self::CreateInvestigation { .. }
                | Self::AddObservation { .. }
                | Self::RunEvaluation { .. }
                | Self::RunVerification { .. }
                | Self::GenerateRecommendation { .. }
                | Self::CreateProposal { .. }
                | Self::CreateExecutionPlan { .. }
                | Self::AgentHandoff { .. }
                | Self::ConfirmPending
        )
    }

    /// External execution is never implied by natural language alone.
    pub fn requires_execution_authority(&self) -> bool {
        matches!(self, Self::CreateExecutionPlan { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_ids_are_stable() {
        assert_eq!(
            WorkspaceActionId::CreateInvestigation.as_str(),
            "create_investigation"
        );
        assert_eq!(WorkspaceActionId::Quit.as_str(), "quit");
    }

    #[test]
    fn create_execution_plan_requires_authority_path() {
        let intent = WorkspaceIntent::CreateExecutionPlan {
            investigation_id: InvestigationId::new(),
            proposal_id: ObjectId::new(),
        };
        assert!(intent.is_mutating());
        assert!(intent.requires_execution_authority());
    }

    #[test]
    fn navigate_is_not_mutating() {
        assert!(!WorkspaceIntent::Navigate {
            route: WorkspaceRoute::Home
        }
        .is_mutating());
    }
}
