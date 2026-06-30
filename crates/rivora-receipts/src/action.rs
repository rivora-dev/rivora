//! Suggested actions and approval requirements for a reliability receipt.
//!
//! Actions are **proposals**, never automatic. Any action that could mutate
//! infrastructure requires explicit human approval per Open Rivora's safety
//! philosophy (see [`docs/01-Manifesto.md`](../../docs/01-Manifesto.md)).

use serde::{Deserialize, Serialize};

use rivora_types::NonEmptyString;

/// The kind of action a [`SuggestedAction`] describes.
///
/// Action kinds are deliberately generic — no provider-specific types
/// (AWS, GitHub, Kubernetes, etc.) are hard-coded. Future provider crates
/// can extend this by adding new variants, but the core schema is
/// provider-agnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    /// A read-only query or view (e.g. "show service health").
    Read,
    /// A diagnostic check (e.g. "run health probe").
    Diagnose,
    /// A non-mutating analysis (e.g. "compare deployments").
    Analyze,
    /// A rollback to a previous state.
    Rollback,
    /// A scaling operation (e.g. increase replicas).
    Scale,
    /// A configuration change.
    Configure,
    /// A redeployment or restart.
    Redeploy,
    /// A notification or alert (e.g. post to Slack).
    Notify,
    /// A human task that requires manual action.
    ManualTask,
    /// Some other kind of action not covered above.
    Other,
}

impl ActionKind {
    /// Whether this action kind **mutates** infrastructure.
    ///
    /// Read-only and diagnostic actions return `false`; all write actions
    /// (rollback, scale, configure, redeploy) return `true`.
    #[must_use]
    pub fn is_mutating(&self) -> bool {
        matches!(
            self,
            Self::Rollback | Self::Scale | Self::Configure | Self::Redeploy | Self::Other
        )
    }

    /// Stable lowercase string tag for the kind.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Diagnose => "diagnose",
            Self::Analyze => "analyze",
            Self::Rollback => "rollback",
            Self::Scale => "scale",
            Self::Configure => "configure",
            Self::Redeploy => "redeploy",
            Self::Notify => "notify",
            Self::ManualTask => "manual_task",
            Self::Other => "other",
        }
    }
}

impl std::fmt::Display for ActionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How much human approval is required before an action can be taken.
///
/// The safety invariant: any **mutating** action requires at least
/// `Required` approval. `Blocked` is a stronger signal — the engine will
/// not propose the action even with approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalRequirement {
    /// No approval required (read-only actions only).
    NotRequired,
    /// Approval recommended but not required.
    Recommended,
    /// Approval required before execution.
    Required,
    /// Action is blocked — approval alone is insufficient.
    Blocked,
}

impl ApprovalRequirement {
    /// Returns the snake_case string tag.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotRequired => "not_required",
            Self::Recommended => "recommended",
            Self::Required => "required",
            Self::Blocked => "blocked",
        }
    }
}

impl std::fmt::Display for ApprovalRequirement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A detailed human-approval record attached to an action.
///
/// This is richer than [`ApprovalRequirement`] — it captures who needs to
/// approve, the reason approval is required, and when approval expires.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HumanApproval {
    /// The minimum approval level required.
    pub requirement: ApprovalRequirement,
    /// The role or person required to approve (e.g. `"on-call"`, `"team-lead"`).
    pub approver: NonEmptyString,
    /// A free-text reason explaining why approval is required.
    pub reason: NonEmptyString,
    /// Optional ISO-8601 timestamp after which the approval expires.
    pub expires_at: Option<String>,
}

impl HumanApproval {
    /// Creates a new `HumanApproval`.
    ///
    /// # Errors
    ///
    /// Returns an error if any required string field is empty.
    pub fn new(
        requirement: ApprovalRequirement,
        approver: impl Into<String>,
        reason: impl Into<String>,
    ) -> Result<Self, rivora_errors::RivoraError> {
        Ok(Self {
            requirement,
            approver: NonEmptyString::new(approver.into())?,
            reason: NonEmptyString::new(reason.into())?,
            expires_at: None,
        })
    }

    /// Builder-style setter for `expires_at`.
    #[must_use]
    pub fn with_expires_at(mut self, expires_at: impl Into<String>) -> Self {
        self.expires_at = Some(expires_at.into());
        self
    }
}

/// A proposed action attached to a receipt.
///
/// Actions are proposals. They are never executed automatically. Mutating
/// actions must have [`ApprovalRequirement::Required`] or stronger.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SuggestedAction {
    /// The kind of action.
    pub kind: ActionKind,
    /// A short title for the action (e.g. `"Rollback payment-service to v1.2.3"`).
    pub title: NonEmptyString,
    /// A longer description of what the action does and why.
    pub description: NonEmptyString,
    /// The expected outcome of the action (e.g. `"latency returns to < 100ms"`).
    pub expected_outcome: NonEmptyString,
    /// The risk level associated with this action.
    pub risk_level: crate::risk::RiskLevel,
    /// The approval requirement for this action.
    pub approval: ApprovalRequirement,
    /// Optional human-approval details (richer than `approval`).
    pub human_approval: Option<HumanApproval>,
    /// Whether this action is read-only (does not mutate state).
    pub read_only: bool,
    /// Whether this action would mutate infrastructure.
    pub mutates_infrastructure: bool,
    /// The scope of the action (e.g. `["payment-service", "us-east-1"]`).
    /// May be empty for actions that don't have a specific scope.
    pub scope: Vec<NonEmptyString>,
    /// An optional rollback strategy. **Required** for mutating actions.
    pub rollback_strategy: Option<NonEmptyString>,
}

impl SuggestedAction {
    /// Creates a new `SuggestedAction` with the required fields.
    ///
    /// # Errors
    ///
    /// Returns an error if the action violates the safety invariant
    /// (mutating action without required approval) or if any required
    /// string field is empty.
    pub fn new(
        kind: ActionKind,
        title: impl Into<String>,
        description: impl Into<String>,
        expected_outcome: impl Into<String>,
        risk_level: crate::risk::RiskLevel,
    ) -> Result<Self, rivora_errors::RivoraError> {
        let mutates = kind.is_mutating();
        let read_only = !mutates;
        let approval = if mutates {
            ApprovalRequirement::Required
        } else {
            ApprovalRequirement::NotRequired
        };
        Ok(Self {
            kind,
            title: NonEmptyString::new(title.into())?,
            description: NonEmptyString::new(description.into())?,
            expected_outcome: NonEmptyString::new(expected_outcome.into())?,
            risk_level,
            approval,
            human_approval: None,
            read_only,
            mutates_infrastructure: mutates,
            scope: Vec::new(),
            rollback_strategy: None,
        })
    }

    /// Builder-style setter for `scope`.
    #[must_use]
    pub fn with_scope(mut self, scope: Vec<NonEmptyString>) -> Self {
        self.scope = scope;
        self
    }

    /// Builder-style setter for `approval`.
    #[must_use]
    pub fn with_approval(mut self, approval: ApprovalRequirement) -> Self {
        self.approval = approval;
        self
    }

    /// Builder-style setter for `human_approval`.
    #[must_use]
    pub fn with_human_approval(mut self, human_approval: HumanApproval) -> Self {
        self.human_approval = Some(human_approval);
        self
    }

    /// Builder-style setter for `rollback_strategy`. Required for mutating
    /// actions.
    #[must_use]
    pub fn with_rollback_strategy(mut self, strategy: impl Into<String>) -> Self {
        self.rollback_strategy = Some(NonEmptyString::new(strategy.into()).unwrap());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::risk::RiskLevel;

    #[test]
    fn action_kind_is_mutating() {
        assert!(!ActionKind::Read.is_mutating());
        assert!(!ActionKind::Diagnose.is_mutating());
        assert!(!ActionKind::Analyze.is_mutating());
        assert!(!ActionKind::Notify.is_mutating());
        assert!(!ActionKind::ManualTask.is_mutating());
        assert!(ActionKind::Rollback.is_mutating());
        assert!(ActionKind::Scale.is_mutating());
        assert!(ActionKind::Configure.is_mutating());
        assert!(ActionKind::Redeploy.is_mutating());
        assert!(ActionKind::Other.is_mutating());
    }

    #[test]
    fn action_kind_as_str() {
        assert_eq!(ActionKind::Read.as_str(), "read");
        assert_eq!(ActionKind::Rollback.as_str(), "rollback");
    }

    #[test]
    fn action_kind_serializes_as_snake_case() {
        let json = serde_json::to_string(&ActionKind::ManualTask).unwrap();
        assert_eq!(json, "\"manual_task\"");
    }

    #[test]
    fn approval_requirement_as_str() {
        assert_eq!(ApprovalRequirement::NotRequired.as_str(), "not_required");
        assert_eq!(ApprovalRequirement::Required.as_str(), "required");
        assert_eq!(ApprovalRequirement::Blocked.as_str(), "blocked");
    }

    #[test]
    fn mutating_action_gets_required_approval_by_default() {
        let a = SuggestedAction::new(
            ActionKind::Rollback,
            "Rollback",
            "Rollback to v1.2.3",
            "Latency returns to normal",
            RiskLevel::Medium,
        )
        .unwrap();
        assert!(a.mutates_infrastructure);
        assert!(!a.read_only);
        assert_eq!(a.approval, ApprovalRequirement::Required);
    }

    #[test]
    fn read_action_gets_no_approval_by_default() {
        let a = SuggestedAction::new(
            ActionKind::Read,
            "View logs",
            "Show service logs",
            "Logs displayed",
            RiskLevel::Low,
        )
        .unwrap();
        assert!(!a.mutates_infrastructure);
        assert!(a.read_only);
        assert_eq!(a.approval, ApprovalRequirement::NotRequired);
    }

    #[test]
    fn action_rejects_empty_title() {
        let result = SuggestedAction::new(
            ActionKind::Read,
            "",
            "description",
            "outcome",
            RiskLevel::Low,
        );
        assert!(result.is_err());
    }

    #[test]
    fn action_round_trips_through_serde() {
        let a = SuggestedAction::new(
            ActionKind::Rollback,
            "Rollback",
            "Rollback to v1.2.3",
            "Latency returns to normal",
            RiskLevel::Medium,
        )
        .unwrap()
        .with_scope(vec![NonEmptyString::new("payment-service").unwrap()])
        .with_rollback_strategy("redeploy prior image")
        .with_human_approval(
            HumanApproval::new(
                ApprovalRequirement::Required,
                "team-lead",
                "mutating action on production",
            )
            .unwrap(),
        );
        let json = serde_json::to_string(&a).unwrap();
        let back: SuggestedAction = serde_json::from_str(&json).unwrap();
        assert_eq!(back.kind, a.kind);
        assert_eq!(back.approval, a.approval);
        assert_eq!(back.mutates_infrastructure, a.mutates_infrastructure);
        assert_eq!(back.rollback_strategy, a.rollback_strategy);
    }

    #[test]
    fn human_approval_round_trips() {
        let h = HumanApproval::new(
            ApprovalRequirement::Required,
            "on-call",
            "production change",
        )
        .unwrap()
        .with_expires_at("2026-12-31T00:00:00Z");
        let json = serde_json::to_string(&h).unwrap();
        let back: HumanApproval = serde_json::from_str(&json).unwrap();
        assert_eq!(back, h);
    }
}
