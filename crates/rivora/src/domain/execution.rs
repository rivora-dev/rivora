//! Controlled external execution (RFC-025, RFC-026, RFC-027).
//!
//! Architectural boundary:
//! ```text
//! Proposal Accepted
//!   ≠ Execution Plan exists
//!   ≠ Execution Approved
//!   ≠ Execution Started
//!   ≠ Execution Completed
//!   ≠ Execution Verified
//!   ≠ Outcome Successful
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{Confidence, InvestigationId, ObjectId, Provenance};
use crate::error::{RivoraError, RivoraResult};

macro_rules! string_enum {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident => $value:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($(#[$vmeta])* $variant),+ }
        impl $name {
            /// Stable string form.
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $value),+ }
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

// ---------------------------------------------------------------------------
// Risk, policy, retry
// ---------------------------------------------------------------------------

string_enum!(
    /// Declared risk of an execution capability or action.
    CapabilityRiskLevel {
        /// Observation only.
        ReadOnly => "read_only",
        /// Narrow, easy-to-reverse write.
        LowRiskWrite => "low_risk_write",
        /// Bounded write with clear scope.
        BoundedWrite => "bounded_write",
        /// High blast radius; denied in v0.6.
        HighRiskWrite => "high_risk_write",
        /// Never permitted.
        Prohibited => "prohibited"
    }
);

string_enum!(
    /// Centralized execution policy decision.
    ExecutionPolicyDecisionKind {
        /// Allowed without additional approval (rare; still requires Plan approval in v0.6).
        Allowed => "allowed",
        /// Allowed only after explicit Plan approval.
        AllowedWithApproval => "allowed_with_approval",
        /// Dry-run / plan validation only.
        AllowedDryRunOnly => "allowed_dry_run_only",
        /// Denied.
        Denied => "denied"
    }
);

string_enum!(
    /// Safety of retrying a failed or incomplete attempt.
    RetrySafety {
        /// Safe to retry with the same idempotency key after explicit request.
        Safe => "safe",
        /// Safe only if preconditions re-validate.
        ConditionallySafe => "conditionally_safe",
        /// Requires a new Plan revision and approval.
        Unsafe => "unsafe",
        /// Treated as unsafe.
        Unknown => "unknown"
    }
);

string_enum!(
    /// Result reported by one execution capability invocation.
    CapabilityExecutionStatus {
        /// The requested mutation completed.
        Success => "success",
        /// The mutation definitely did not complete.
        Failed => "failed",
        /// The capability completed only part of the requested mutation.
        Partial => "partial",
        /// The request may have reached the external system but the outcome is unknown.
        Uncertain => "uncertain",
        /// The capability suppressed a duplicate mutation.
        DuplicateSuppressed => "duplicate_suppressed"
    }
);

string_enum!(
    /// Capability-specific independent verification conclusion.
    CapabilityVerificationStatus {
        /// The exact requested postcondition was independently observed.
        Passed => "passed",
        /// Independent observation contradicted the requested postcondition.
        Failed => "failed",
        /// The exact requested postcondition could not be established.
        Inconclusive => "inconclusive"
    }
);

// ---------------------------------------------------------------------------
// Plan lifecycle
// ---------------------------------------------------------------------------

string_enum!(
    /// Execution Plan lifecycle status.
    ExecutionPlanStatus {
        /// Incomplete or editable draft.
        Draft => "draft",
        /// Preconditions and scope validated; ready for human review.
        ReadyForReview => "ready_for_review",
        /// Authorized for the exact revision and scope.
        Approved => "approved",
        /// External invocation began.
        Executing => "executing",
        /// External system returned a completed result for all actions.
        Executed => "executed",
        /// Immediate postconditions checked.
        Verified => "verified",
        /// Workflow complete; ready for Outcome measurement.
        Closed => "closed",
        /// Explicitly rejected.
        Rejected => "rejected",
        /// Approval or plan expired.
        Expired => "expired",
        /// Explicitly cancelled.
        Cancelled => "cancelled",
        /// No valid successful completion.
        Failed => "failed",
        /// Only some actions completed.
        PartiallyExecuted => "partially_executed",
        /// Rollback was recorded (external).
        RolledBack => "rolled_back",
        /// Replaced by a newer Plan.
        Superseded => "superseded"
    }
);

/// Whether a plan status is terminal (no further normal transitions except supersede history).
pub fn execution_plan_status_is_terminal(status: ExecutionPlanStatus) -> bool {
    matches!(
        status,
        ExecutionPlanStatus::Closed
            | ExecutionPlanStatus::Rejected
            | ExecutionPlanStatus::Expired
            | ExecutionPlanStatus::Cancelled
            | ExecutionPlanStatus::RolledBack
            | ExecutionPlanStatus::Superseded
    )
}

/// Validate an Execution Plan status transition.
pub fn valid_execution_plan_transition(from: ExecutionPlanStatus, to: ExecutionPlanStatus) -> bool {
    if from == to {
        return false;
    }
    use ExecutionPlanStatus::*;
    match from {
        Draft => matches!(to, ReadyForReview | Rejected | Cancelled | Superseded),
        ReadyForReview => matches!(to, Approved | Draft | Rejected | Cancelled | Superseded),
        Approved => matches!(
            to,
            Executing | Cancelled | Expired | Superseded | ReadyForReview
        ),
        Executing => matches!(to, Executed | PartiallyExecuted | Failed | Cancelled),
        Executed => matches!(to, Verified | Failed | PartiallyExecuted),
        PartiallyExecuted => matches!(to, Verified | Failed | RolledBack | Closed),
        Failed => matches!(to, Closed | RolledBack | Superseded),
        Verified => matches!(to, Closed | RolledBack),
        Closed | Rejected | Expired | Cancelled | RolledBack | Superseded => false,
    }
}

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

/// One ordered external action in a Plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionAction {
    /// Stable action identifier within the plan.
    pub action_id: String,
    /// Capability-specific action name.
    pub action_name: String,
    /// Structured inputs (never secrets).
    pub inputs: serde_json::Value,
    /// Optional continuation policy after failure of this action.
    pub continue_on_failure: bool,
}

/// Expected external effect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedEffect {
    /// Human-readable description.
    pub description: String,
    /// Resource type (issue, pull_request, workflow_run, …).
    pub resource_type: String,
    /// Expected field checks (key → expected value summary).
    pub expected_fields: Vec<(String, String)>,
}

/// Plan precondition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPrecondition {
    /// Precondition identifier.
    pub id: String,
    /// Human-readable description.
    pub description: String,
    /// Whether currently satisfied (when known).
    pub satisfied: Option<bool>,
    /// Detail when unsatisfied.
    pub detail: Option<String>,
}

/// Declared risk on a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionRisk {
    /// Risk description.
    pub description: String,
    /// Severity label.
    pub severity: String,
    /// Mitigation.
    pub mitigation: String,
}

/// Rollback metadata (no automatic rollback).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RollbackMetadata {
    /// Whether a rollback path is available.
    pub available: bool,
    /// Capability that could reverse the change, if any.
    pub capability_id: Option<String>,
    /// Suggested rollback inputs.
    pub inputs: Option<serde_json::Value>,
    /// Explicit inverse action name. Never inferred from descriptor ordering.
    #[serde(default)]
    pub inverse_action_name: Option<String>,
    /// Risks of rollback.
    pub risks: Vec<String>,
    /// How to verify rollback.
    pub verification: Option<String>,
    /// Effects that cannot be reversed.
    pub irreversible_effects: Vec<String>,
}

/// Adapter-reported immutable external target identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityTarget {
    /// External provider (`github`, `mock`, ...).
    pub provider: String,
    /// Account or repository owner, when applicable.
    pub owner: Option<String>,
    /// Repository or bounded target name, when applicable.
    pub repository: Option<String>,
    /// Branch or ref bound by the action, when applicable.
    pub branch_or_ref: Option<String>,
}

/// Immutable target authorized by one exact Execution Plan revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetSnapshot {
    /// External provider.
    pub provider: String,
    /// Account or repository owner, when applicable.
    pub owner: Option<String>,
    /// Repository or bounded target name, when applicable.
    pub repository: Option<String>,
    /// Target environment.
    pub environment: String,
    /// Exact execution capability.
    pub capability_id: String,
    /// Exact Plan snapshot.
    pub plan_id: ObjectId,
    /// Exact Plan revision.
    pub plan_revision_number: u32,
    /// Branch or ref, when applicable.
    pub branch_or_ref: Option<String>,
}

impl TargetSnapshot {
    /// Build a Plan-bound snapshot from a capability-reported target.
    pub fn bind(plan: &ExecutionPlan, target: CapabilityTarget) -> Self {
        Self {
            provider: target.provider,
            owner: target.owner,
            repository: target.repository,
            environment: plan.target_environment.clone(),
            capability_id: plan.capability_id.clone(),
            plan_id: plan.id,
            plan_revision_number: plan.revision_number,
            branch_or_ref: target.branch_or_ref,
        }
    }

    /// Rebind immutable Plan identity after a preserved successor snapshot is created.
    pub fn rebound_to(&self, plan_id: ObjectId, plan_revision_number: u32) -> Self {
        let mut next = self.clone();
        next.plan_id = plan_id;
        next.plan_revision_number = plan_revision_number;
        next
    }
}

/// Immediate verification plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ExecutionVerificationPlan {
    /// Named checks to perform after execution.
    pub checks: Vec<String>,
    /// Expected evidence descriptions.
    pub expected_evidence: Vec<String>,
}

/// Authority requirements for approval/execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequiredAuthority {
    /// Whether a named human actor is required (always true in v0.6).
    pub named_actor_required: bool,
    /// Whether a non-empty reason is required.
    pub reason_required: bool,
    /// Whether environment must match.
    pub environment_must_match: bool,
}

impl Default for RequiredAuthority {
    fn default() -> Self {
        Self {
            named_actor_required: true,
            reason_required: true,
            environment_must_match: true,
        }
    }
}

string_enum!(
    /// How idempotency is handled for a plan/capability.
    IdempotencyStrategy {
        /// Client-generated key stored on the attempt.
        ClientKey => "client_key",
        /// Capability performs natural-key deduplication.
        CapabilityNaturalKey => "capability_natural_key",
        /// Both client key and capability natural key.
        Combined => "combined"
    }
);

/// Scope restriction for an Execution Plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ScopeRestrictions {
    /// Allowed repositories (owner/repo).
    pub repositories: Vec<String>,
    /// Allowed resource identifiers.
    pub resource_ids: Vec<String>,
    /// Allowed action names.
    pub action_names: Vec<String>,
    /// Maximum number of mutating actions.
    pub max_actions: Option<u32>,
}

/// Preserved plan lifecycle transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPlanTransition {
    /// Previous status.
    pub from: ExecutionPlanStatus,
    /// New status.
    pub to: ExecutionPlanStatus,
    /// Actor.
    pub actor: String,
    /// Reason.
    pub reason: String,
    /// Timestamp.
    pub at: DateTime<Utc>,
}

/// Recorded policy decision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionPolicyDecision {
    /// Decision kind.
    pub decision: ExecutionPolicyDecisionKind,
    /// Human-readable reasons.
    pub reasons: Vec<String>,
    /// Evaluated risk level.
    pub risk_level: CapabilityRiskLevel,
    /// Whether dry-run is permitted.
    pub dry_run_permitted: bool,
    /// Whether live execution is permitted after approval.
    pub live_execution_permitted: bool,
    /// Evaluated at.
    pub evaluated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Execution Plan
// ---------------------------------------------------------------------------

/// Durable plan converting an accepted Proposal into external actions.
///
/// Never executes merely because it exists.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionPlan {
    /// Snapshot identifier.
    pub id: ObjectId,
    /// Stable lineage across revisions.
    pub lineage_id: ObjectId,
    /// One-based revision number.
    pub revision_number: u32,
    /// Prior immutable snapshot.
    pub parent_plan_id: Option<ObjectId>,
    /// Successor when superseded.
    pub superseding_plan_id: Option<ObjectId>,
    /// Owning Investigation.
    pub investigation_id: InvestigationId,
    /// Exact Proposal snapshot.
    pub proposal_id: ObjectId,
    /// Proposal lineage.
    pub proposal_lineage_id: ObjectId,
    /// Proposal revision at plan creation.
    pub proposal_revision_number: u32,
    /// Lifecycle status.
    pub status: ExecutionPlanStatus,
    /// Target capability identifier.
    pub capability_id: String,
    /// Target system family.
    pub target_system: String,
    /// Target environment.
    pub target_environment: String,
    /// Exact immutable external target for this Plan revision.
    #[serde(default)]
    pub target_snapshot: Option<TargetSnapshot>,
    /// Ordered actions.
    pub actions: Vec<ExecutionAction>,
    /// Shared inputs (never secrets).
    pub inputs: serde_json::Value,
    /// Expected effects.
    pub expected_effects: Vec<ExpectedEffect>,
    /// Preconditions.
    pub preconditions: Vec<ExecutionPrecondition>,
    /// Declared risks.
    pub risks: Vec<ExecutionRisk>,
    /// Rollback metadata.
    pub rollback: RollbackMetadata,
    /// Immediate verification plan.
    pub verification_plan: ExecutionVerificationPlan,
    /// Required authority.
    pub required_authority: RequiredAuthority,
    /// Whether dry-run is supported for this plan/capability.
    pub supports_dry_run: bool,
    /// Idempotency strategy.
    pub idempotency_strategy: IdempotencyStrategy,
    /// Scope restrictions.
    pub scope_restrictions: ScopeRestrictions,
    /// Last policy decision (if evaluated).
    pub last_policy_decision: Option<ExecutionPolicyDecision>,
    /// Lifecycle transitions.
    pub transitions: Vec<ExecutionPlanTransition>,
    /// Provenance.
    pub provenance: Provenance,
    /// Creation timestamp (lineage).
    pub created_at: DateTime<Utc>,
    /// Snapshot update timestamp.
    pub updated_at: DateTime<Utc>,
}

impl ExecutionPlan {
    /// Create a draft Execution Plan linked to an accepted Proposal.
    #[allow(clippy::too_many_arguments)]
    pub fn draft(
        investigation_id: InvestigationId,
        proposal_id: ObjectId,
        proposal_lineage_id: ObjectId,
        proposal_revision_number: u32,
        capability_id: impl Into<String>,
        target_system: impl Into<String>,
        target_environment: impl Into<String>,
        actions: Vec<ExecutionAction>,
        provenance: Provenance,
    ) -> RivoraResult<Self> {
        let capability_id = capability_id.into().trim().to_string();
        let target_system = target_system.into().trim().to_string();
        let target_environment = target_environment.into().trim().to_string();
        if capability_id.is_empty() {
            return Err(RivoraError::validation("capability_id must not be empty"));
        }
        if target_system.is_empty() {
            return Err(RivoraError::validation("target_system must not be empty"));
        }
        if target_environment.is_empty() {
            return Err(RivoraError::validation(
                "target_environment must not be empty",
            ));
        }
        if actions.is_empty() {
            return Err(RivoraError::validation(
                "execution plan requires at least one action",
            ));
        }
        for action in &actions {
            if action.action_id.trim().is_empty() || action.action_name.trim().is_empty() {
                return Err(RivoraError::validation(
                    "action_id and action_name must not be empty",
                ));
            }
        }
        let now = Utc::now();
        let id = ObjectId::new();
        Ok(Self {
            id,
            lineage_id: id,
            revision_number: 1,
            parent_plan_id: None,
            superseding_plan_id: None,
            investigation_id,
            proposal_id,
            proposal_lineage_id,
            proposal_revision_number,
            status: ExecutionPlanStatus::Draft,
            capability_id,
            target_system,
            target_environment,
            target_snapshot: None,
            actions,
            inputs: serde_json::json!({}),
            expected_effects: Vec::new(),
            preconditions: Vec::new(),
            risks: Vec::new(),
            rollback: RollbackMetadata::default(),
            verification_plan: ExecutionVerificationPlan::default(),
            required_authority: RequiredAuthority::default(),
            supports_dry_run: true,
            idempotency_strategy: IdempotencyStrategy::Combined,
            scope_restrictions: ScopeRestrictions::default(),
            last_policy_decision: None,
            transitions: Vec::new(),
            provenance,
            created_at: now,
            updated_at: now,
        })
    }

    /// Create an immutable successor revision with optional content updates.
    pub fn revised(
        &self,
        actor: impl Into<String>,
        reason: impl Into<String>,
        at: DateTime<Utc>,
    ) -> RivoraResult<Self> {
        let actor = actor.into().trim().to_string();
        let reason = reason.into().trim().to_string();
        if actor.is_empty() || reason.is_empty() {
            return Err(RivoraError::validation(
                "revise_execution_plan requires non-empty actor and reason",
            ));
        }
        if execution_plan_status_is_terminal(self.status)
            || matches!(
                self.status,
                ExecutionPlanStatus::Executing
                    | ExecutionPlanStatus::Executed
                    | ExecutionPlanStatus::Verified
                    | ExecutionPlanStatus::PartiallyExecuted
            )
        {
            return Err(RivoraError::validation(format!(
                "cannot revise execution plan in status {}",
                self.status.as_str()
            )));
        }
        let mut next = self.clone();
        next.id = ObjectId::new();
        next.parent_plan_id = Some(self.id);
        next.revision_number = self.revision_number.saturating_add(1);
        next.status = ExecutionPlanStatus::Draft;
        next.target_snapshot = self
            .target_snapshot
            .as_ref()
            .map(|target| target.rebound_to(next.id, next.revision_number));
        next.updated_at = at;
        next.last_policy_decision = None;
        next.provenance = Provenance::now(actor, "runtime")
            .with_capability("revise_execution_plan")
            .with_evidence(vec![self.id]);
        next.transitions.push(ExecutionPlanTransition {
            from: self.status,
            to: ExecutionPlanStatus::Draft,
            actor: next.provenance.actor.clone(),
            reason,
            at,
        });
        Ok(next)
    }

    /// Transition lifecycle status with immutable successor snapshot.
    pub fn transitioned(
        &self,
        to: ExecutionPlanStatus,
        actor: impl Into<String>,
        reason: impl Into<String>,
        at: DateTime<Utc>,
    ) -> RivoraResult<Self> {
        let actor = actor.into().trim().to_string();
        let reason = reason.into().trim().to_string();
        if actor.is_empty() || reason.is_empty() {
            return Err(RivoraError::validation(
                "execution plan transition requires non-empty actor and reason",
            ));
        }
        if !valid_execution_plan_transition(self.status, to) {
            return Err(RivoraError::validation(format!(
                "invalid execution plan transition {} → {}",
                self.status.as_str(),
                to.as_str()
            )));
        }
        let mut next = self.clone();
        next.id = ObjectId::new();
        next.parent_plan_id = Some(self.id);
        next.revision_number = self.revision_number.saturating_add(1);
        next.status = to;
        next.target_snapshot = self
            .target_snapshot
            .as_ref()
            .map(|target| target.rebound_to(next.id, next.revision_number));
        next.updated_at = at;
        next.provenance = Provenance::now(&actor, "runtime")
            .with_capability("transition_execution_plan")
            .with_evidence(vec![self.id]);
        next.transitions.push(ExecutionPlanTransition {
            from: self.status,
            to,
            actor,
            reason,
            at,
        });
        Ok(next)
    }
}

// ---------------------------------------------------------------------------
// Approval
// ---------------------------------------------------------------------------

/// Explicit approval bound to an exact Execution Plan revision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionApproval {
    /// Approval identifier.
    pub id: ObjectId,
    /// Exact Plan snapshot approved.
    pub plan_id: ObjectId,
    /// Plan lineage.
    pub plan_lineage_id: ObjectId,
    /// Exact Plan revision number.
    pub plan_revision_number: u32,
    /// Owning Investigation.
    pub investigation_id: InvestigationId,
    /// Named approver.
    pub actor: String,
    /// Non-empty reason.
    pub reason: String,
    /// Approved action identifiers (empty means all plan actions).
    pub approved_actions: Vec<String>,
    /// Explicitly denied actions.
    pub denied_actions: Vec<String>,
    /// Approved environment.
    pub environment: String,
    /// Approved capability.
    pub capability_id: String,
    /// Exact external target authorized with the Plan revision.
    #[serde(default)]
    pub target_snapshot: Option<TargetSnapshot>,
    /// Policy decision at approval time.
    pub policy_decision: ExecutionPolicyDecision,
    /// Optional expiration.
    pub expires_at: Option<DateTime<Utc>>,
    /// Whether this approval is one-time.
    pub one_time: bool,
    /// Whether already consumed by an execution attempt.
    pub consumed: bool,
    /// Whether invalidated (revision change, expiry, etc.).
    pub invalidated: bool,
    /// Invalidation reason when set.
    pub invalidation_reason: Option<String>,
    /// Provenance.
    pub provenance: Provenance,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

impl ExecutionApproval {
    /// Create an approval for an exact plan revision.
    #[allow(clippy::too_many_arguments)]
    pub fn grant(
        plan: &ExecutionPlan,
        actor: impl Into<String>,
        reason: impl Into<String>,
        approved_actions: Vec<String>,
        denied_actions: Vec<String>,
        policy_decision: ExecutionPolicyDecision,
        expires_at: Option<DateTime<Utc>>,
        one_time: bool,
        provenance: Provenance,
    ) -> RivoraResult<Self> {
        let actor = actor.into().trim().to_string();
        let reason = reason.into().trim().to_string();
        if actor.is_empty() || reason.is_empty() {
            return Err(RivoraError::validation(
                "execution approval requires non-empty actor and reason",
            ));
        }
        if plan.status != ExecutionPlanStatus::ReadyForReview
            && plan.status != ExecutionPlanStatus::Approved
        {
            return Err(RivoraError::validation(format!(
                "can only approve plans in ready_for_review or approved status, got {}",
                plan.status.as_str()
            )));
        }
        if !matches!(
            policy_decision.decision,
            ExecutionPolicyDecisionKind::Allowed | ExecutionPolicyDecisionKind::AllowedWithApproval
        ) {
            return Err(RivoraError::validation(format!(
                "cannot approve plan when policy decision is {}",
                policy_decision.decision.as_str()
            )));
        }
        if !policy_decision.live_execution_permitted {
            return Err(RivoraError::validation(
                "cannot approve live execution when policy does not permit live execution",
            ));
        }
        let action_ids: std::collections::HashSet<&str> = plan
            .actions
            .iter()
            .map(|action| action.action_id.as_str())
            .collect();
        if approved_actions
            .iter()
            .chain(denied_actions.iter())
            .any(|action| !action_ids.contains(action.as_str()))
        {
            return Err(RivoraError::validation(
                "approval scope contains an action not present in the Plan",
            ));
        }
        if approved_actions
            .iter()
            .any(|action| denied_actions.contains(action))
        {
            return Err(RivoraError::validation(
                "an action cannot be both approved and denied",
            ));
        }
        let target_snapshot = plan.target_snapshot.clone().ok_or_else(|| {
            RivoraError::validation("cannot approve a Plan without an immutable target snapshot")
        })?;
        Ok(Self {
            id: ObjectId::new(),
            plan_id: plan.id,
            plan_lineage_id: plan.lineage_id,
            plan_revision_number: plan.revision_number,
            investigation_id: plan.investigation_id,
            actor,
            reason,
            approved_actions,
            denied_actions,
            environment: plan.target_environment.clone(),
            capability_id: plan.capability_id.clone(),
            target_snapshot: Some(target_snapshot),
            policy_decision,
            expires_at,
            one_time,
            consumed: false,
            invalidated: false,
            invalidation_reason: None,
            provenance,
            created_at: Utc::now(),
        })
    }

    /// Whether this approval is currently usable for the given plan snapshot.
    pub fn is_valid_for(&self, plan: &ExecutionPlan, now: DateTime<Utc>) -> RivoraResult<()> {
        if self.invalidated {
            return Err(RivoraError::precondition(format!(
                "approval {} is invalidated: {}",
                self.id,
                self.invalidation_reason
                    .as_deref()
                    .unwrap_or("no reason recorded")
            )));
        }
        if self.consumed && self.one_time {
            return Err(RivoraError::precondition(format!(
                "one-time approval {} already consumed",
                self.id
            )));
        }
        if let Some(exp) = self.expires_at {
            if now > exp {
                return Err(RivoraError::precondition(format!(
                    "approval {} expired at {exp}",
                    self.id
                )));
            }
        }
        if self.plan_id != plan.id {
            return Err(RivoraError::precondition(format!(
                "approval {} binds plan snapshot {}, not {}",
                self.id, self.plan_id, plan.id
            )));
        }
        if self.plan_lineage_id != plan.lineage_id {
            return Err(RivoraError::precondition(
                "approval plan lineage does not match",
            ));
        }
        if self.plan_revision_number != plan.revision_number {
            return Err(RivoraError::precondition(format!(
                "stale approval: binds revision {}, plan is revision {}",
                self.plan_revision_number, plan.revision_number
            )));
        }
        if self.environment != plan.target_environment {
            return Err(RivoraError::precondition(
                "approval environment does not match plan environment",
            ));
        }
        if self.capability_id != plan.capability_id {
            return Err(RivoraError::precondition(
                "approval capability does not match plan capability",
            ));
        }
        match (&self.target_snapshot, &plan.target_snapshot) {
            (Some(approved), Some(planned)) if approved == planned => {}
            (None, _) | (_, None) => {
                return Err(RivoraError::precondition(
                    "approval and Plan must both contain an immutable target snapshot",
                ));
            }
            _ => {
                return Err(RivoraError::precondition(
                    "approval target snapshot does not match Plan target",
                ));
            }
        }
        Ok(())
    }

    /// Mark approval consumed (one-time semantics).
    pub fn mark_consumed(&self) -> Self {
        let mut next = self.clone();
        next.consumed = true;
        next
    }

    /// Invalidate this approval.
    pub fn invalidate(&self, reason: impl Into<String>) -> Self {
        let mut next = self.clone();
        next.invalidated = true;
        next.invalidation_reason = Some(reason.into());
        next
    }
}

// ---------------------------------------------------------------------------
// Attempt / Receipt / Verification
// ---------------------------------------------------------------------------

string_enum!(
    /// Status of one execution attempt.
    ExecutionAttemptStatus {
        /// Invocation started.
        Started => "started",
        /// All requested actions completed.
        Completed => "completed",
        /// Some actions completed, some failed.
        PartiallyCompleted => "partially_completed",
        /// No successful completion.
        Failed => "failed",
        /// Blocked by precondition or policy before mutation.
        Blocked => "blocked",
        /// Suppressed as idempotent duplicate.
        DuplicateSuppressed => "duplicate_suppressed"
    }
);

string_enum!(
    /// Receipt-level result status from the external system report.
    ExecutionReceiptResult {
        /// External system reported success.
        Success => "success",
        /// External system reported failure.
        Failed => "failed",
        /// Partial external result.
        Partial => "partial",
        /// Uncertain external result.
        Uncertain => "uncertain"
    }
);

string_enum!(
    /// Immediate execution verification status.
    ExecutionVerificationStatus {
        /// All checks passed.
        Passed => "passed",
        /// One or more checks failed.
        Failed => "failed",
        /// Insufficient evidence or ambiguous state.
        Inconclusive => "inconclusive"
    }
);

/// One attempt to invoke an approved plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionAttempt {
    /// Attempt identifier.
    pub id: ObjectId,
    /// Stable attempt lineage across Started and terminal snapshots.
    #[serde(default)]
    pub attempt_lineage_id: Option<ObjectId>,
    /// Prior immutable Attempt snapshot.
    #[serde(default)]
    pub parent_attempt_id: Option<ObjectId>,
    /// One-based Attempt snapshot revision.
    #[serde(default = "default_revision")]
    pub revision_number: u32,
    /// Owning Investigation.
    pub investigation_id: InvestigationId,
    /// Exact Plan snapshot.
    pub plan_id: ObjectId,
    /// Plan lineage.
    pub plan_lineage_id: ObjectId,
    /// Plan revision.
    pub plan_revision_number: u32,
    /// Approval used.
    pub approval_id: ObjectId,
    /// Actor who started the attempt.
    pub actor: String,
    /// Capability.
    pub capability_id: String,
    /// Target system.
    pub target_system: String,
    /// Environment.
    pub environment: String,
    /// Exact target snapshot used for this Attempt.
    #[serde(default)]
    pub target_snapshot: Option<TargetSnapshot>,
    /// Attempt status.
    pub status: ExecutionAttemptStatus,
    /// Requested action ids.
    pub requested_actions: Vec<String>,
    /// Completed action ids.
    pub completed_actions: Vec<String>,
    /// Failed action ids.
    pub failed_actions: Vec<String>,
    /// Skipped action ids.
    pub skipped_actions: Vec<String>,
    /// Uncertain action ids.
    pub uncertain_actions: Vec<String>,
    /// Idempotency key.
    pub idempotency_key: String,
    /// Retry safety classification.
    pub retry_safety: RetrySafety,
    /// Structured errors.
    pub errors: Vec<String>,
    /// External references.
    pub external_references: Vec<String>,
    /// Linked receipt ids.
    pub receipt_ids: Vec<ObjectId>,
    /// Linked verification id if any.
    pub verification_id: Option<ObjectId>,
    /// Original Attempt when this request was durably duplicate-suppressed.
    #[serde(default)]
    pub duplicate_of_attempt_id: Option<ObjectId>,
    /// Whether this was a dry-run (must never mutate).
    pub dry_run: bool,
    /// Recommended next action.
    pub recommended_next_action: Option<String>,
    /// Rollback availability after attempt.
    pub rollback: RollbackMetadata,
    /// Started at.
    pub started_at: DateTime<Utc>,
    /// Finished at.
    pub finished_at: Option<DateTime<Utc>>,
    /// Provenance.
    pub provenance: Provenance,
}

impl ExecutionAttempt {
    /// Start a new attempt.
    pub fn start(
        plan: &ExecutionPlan,
        approval: &ExecutionApproval,
        actor: impl Into<String>,
        idempotency_key: impl Into<String>,
        dry_run: bool,
        provenance: Provenance,
    ) -> RivoraResult<Self> {
        let actor = actor.into().trim().to_string();
        let idempotency_key = idempotency_key.into().trim().to_string();
        if actor.is_empty() {
            return Err(RivoraError::validation(
                "execution attempt requires non-empty actor",
            ));
        }
        if idempotency_key.is_empty() {
            return Err(RivoraError::validation(
                "execution attempt requires non-empty idempotency_key",
            ));
        }
        let requested: Vec<String> = plan.actions.iter().map(|a| a.action_id.clone()).collect();
        let id = ObjectId::new();
        Ok(Self {
            id,
            attempt_lineage_id: Some(id),
            parent_attempt_id: None,
            revision_number: 1,
            investigation_id: plan.investigation_id,
            plan_id: plan.id,
            plan_lineage_id: plan.lineage_id,
            plan_revision_number: plan.revision_number,
            approval_id: approval.id,
            actor,
            capability_id: plan.capability_id.clone(),
            target_system: plan.target_system.clone(),
            environment: plan.target_environment.clone(),
            target_snapshot: plan.target_snapshot.clone(),
            status: ExecutionAttemptStatus::Started,
            requested_actions: requested,
            completed_actions: Vec::new(),
            failed_actions: Vec::new(),
            skipped_actions: Vec::new(),
            uncertain_actions: Vec::new(),
            idempotency_key,
            retry_safety: RetrySafety::Unknown,
            errors: Vec::new(),
            external_references: Vec::new(),
            receipt_ids: Vec::new(),
            verification_id: None,
            duplicate_of_attempt_id: None,
            dry_run,
            recommended_next_action: None,
            rollback: plan.rollback.clone(),
            started_at: Utc::now(),
            finished_at: None,
            provenance,
        })
    }

    /// Stable lineage identifier, including backward-compatible v0.6 snapshots.
    pub fn lineage_id(&self) -> ObjectId {
        self.attempt_lineage_id.unwrap_or(self.id)
    }

    /// Create a preserved successor snapshot for a terminal Attempt state.
    pub fn revised(&self, provenance: Provenance) -> Self {
        let mut next = self.clone();
        next.id = ObjectId::new();
        next.attempt_lineage_id = Some(self.lineage_id());
        next.parent_attempt_id = Some(self.id);
        next.revision_number = self.revision_number.saturating_add(1);
        next.provenance = provenance;
        next
    }
}

fn default_revision() -> u32 {
    1
}

/// Sanitization metadata for receipts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SanitizationMetadata {
    /// Keys redacted from request/response.
    pub redacted_keys: Vec<String>,
    /// Whether raw body was discarded.
    pub raw_body_discarded: bool,
}

/// Durable receipt of external system report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionReceipt {
    /// Receipt identifier.
    pub id: ObjectId,
    /// Parent attempt.
    pub attempt_id: ObjectId,
    /// Owning Investigation.
    pub investigation_id: InvestigationId,
    /// Capability.
    pub capability_id: String,
    /// Target system.
    pub target_system: String,
    /// Action name.
    pub action_name: String,
    /// Action id within plan.
    pub action_id: String,
    /// Sanitized request summary.
    pub request_summary: String,
    /// Sanitized response summary.
    pub response_summary: String,
    /// Changed resources.
    pub changed_resources: Vec<String>,
    /// Unchanged resources.
    pub unchanged_resources: Vec<String>,
    /// External identifiers.
    pub external_identifiers: Vec<String>,
    /// Result status as reported.
    pub result_status: ExecutionReceiptResult,
    /// Warnings.
    pub warnings: Vec<String>,
    /// Rollback metadata.
    pub rollback_metadata: RollbackMetadata,
    /// Verification requirements remaining.
    pub verification_requirements: Vec<String>,
    /// Sanitized evidence refs.
    pub raw_evidence_refs: Vec<String>,
    /// Sanitization metadata.
    pub sanitization: SanitizationMetadata,
    /// Provenance.
    pub provenance: Provenance,
    /// Created at.
    pub created_at: DateTime<Utc>,
}

/// One verification check result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionCheckResult {
    /// Check name.
    pub check: String,
    /// Whether the check passed.
    pub passed: bool,
    /// Detail.
    pub detail: String,
    /// Evidence references.
    pub evidence: Vec<String>,
}

/// Immediate post-execution verification record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionVerification {
    /// Verification identifier.
    pub id: ObjectId,
    /// Prior verification revision for the same Attempt.
    #[serde(default)]
    pub parent_verification_id: Option<ObjectId>,
    /// Attempt verified.
    pub attempt_id: ObjectId,
    /// Receipts considered.
    pub receipt_ids: Vec<ObjectId>,
    /// Owning Investigation.
    pub investigation_id: InvestigationId,
    /// Checks performed.
    pub checks: Vec<String>,
    /// Per-check results.
    pub results: Vec<ExecutionCheckResult>,
    /// Overall status.
    pub status: ExecutionVerificationStatus,
    /// Confidence.
    pub confidence: Confidence,
    /// Contradictions.
    pub contradictions: Vec<String>,
    /// Unresolved risks.
    pub unresolved_risks: Vec<String>,
    /// Actor.
    pub actor: String,
    /// Evidence refs.
    pub evidence: Vec<String>,
    /// Provenance.
    pub provenance: Provenance,
    /// Created at.
    pub created_at: DateTime<Utc>,
    /// Revision (1-based for re-verification).
    pub revision: u32,
}

/// Dry-run / plan validation outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DryRunResult {
    /// Normalized actions.
    pub actions: Vec<String>,
    /// Target summary.
    pub target: String,
    /// Expected mutations.
    pub expected_mutations: Vec<String>,
    /// Required permissions.
    pub required_permissions: Vec<String>,
    /// Current state summary when available.
    pub current_state: Option<String>,
    /// Predicted resulting state.
    pub predicted_state: Option<String>,
    /// Risks.
    pub risks: Vec<String>,
    /// Policy decision.
    pub policy_decision: ExecutionPolicyDecision,
    /// Missing preconditions.
    pub missing_preconditions: Vec<String>,
    /// Verification steps.
    pub verification_steps: Vec<String>,
    /// Rollback options.
    pub rollback_options: Vec<String>,
    /// Whether this was true dry-run vs plan validation only.
    pub simulated: bool,
}

/// Capability descriptor for listing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionCapabilityDescriptor {
    /// Unique capability id.
    pub capability_id: String,
    /// Contract version.
    pub version: String,
    /// Risk level.
    pub risk_level: CapabilityRiskLevel,
    /// Supported action names.
    pub supported_actions: Vec<String>,
    /// Required input keys.
    pub required_inputs: Vec<String>,
    /// Dry-run support.
    pub supports_dry_run: bool,
    /// Idempotency behavior summary.
    pub idempotency_behavior: String,
    /// Reversibility summary.
    pub reversibility: String,
    /// Verification method summary.
    pub verification_method: String,
    /// Credential requirements (names only).
    pub credential_requirements: Vec<String>,
    /// Target restrictions.
    pub target_restrictions: Vec<String>,
    /// Failure semantics summary.
    pub failure_semantics: String,
    /// Human description.
    pub description: String,
}

/// End-to-end execution trace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionTrace {
    /// Investigation.
    pub investigation_id: InvestigationId,
    /// Plan lineage.
    pub plan_lineage_id: ObjectId,
    /// Current plan snapshot.
    pub plan_id: ObjectId,
    /// Plan revision.
    pub plan_revision_number: u32,
    /// Plan status.
    pub plan_status: ExecutionPlanStatus,
    /// Proposal snapshot.
    pub proposal_id: ObjectId,
    /// Proposal revision.
    pub proposal_revision_number: u32,
    /// Approvals for this plan lineage.
    pub approval_ids: Vec<ObjectId>,
    /// Attempts.
    pub attempt_ids: Vec<ObjectId>,
    /// Receipts.
    pub receipt_ids: Vec<ObjectId>,
    /// Verifications.
    pub verification_ids: Vec<ObjectId>,
    /// Linked implementation record if any.
    pub implementation_record_id: Option<ObjectId>,
    /// Linked measured outcome if any.
    pub measured_outcome_id: Option<ObjectId>,
    /// Human-readable boundary explanation.
    pub explanation: String,
}

/// Listing with corruption isolation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ExecutionPlanListing {
    /// Valid plans.
    pub plans: Vec<ExecutionPlan>,
    /// Diagnostics for corrupt files.
    pub diagnostics: Vec<ExecutionStorageDiagnostic>,
}

/// Attempt listing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ExecutionAttemptListing {
    /// Valid attempts.
    pub attempts: Vec<ExecutionAttempt>,
    /// Diagnostics.
    pub diagnostics: Vec<ExecutionStorageDiagnostic>,
}

/// Receipt listing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ExecutionReceiptListing {
    /// Valid receipts.
    pub receipts: Vec<ExecutionReceipt>,
    /// Diagnostics.
    pub diagnostics: Vec<ExecutionStorageDiagnostic>,
}

/// Approval listing with corruption isolation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ExecutionApprovalListing {
    /// Valid approvals.
    pub approvals: Vec<ExecutionApproval>,
    /// Corrupt or mis-owned records.
    pub diagnostics: Vec<ExecutionStorageDiagnostic>,
}

/// Verification listing with corruption isolation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ExecutionVerificationListing {
    /// Valid verification revisions.
    pub verifications: Vec<ExecutionVerification>,
    /// Corrupt or mis-owned records.
    pub diagnostics: Vec<ExecutionStorageDiagnostic>,
}

/// Storage diagnostic for corrupt or mis-owned execution objects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionStorageDiagnostic {
    /// File path.
    pub path: String,
    /// Error message.
    pub error: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_action() -> ExecutionAction {
        ExecutionAction {
            action_id: "a1".into(),
            action_name: "create_comment".into(),
            inputs: serde_json::json!({"body": "hello"}),
            continue_on_failure: false,
        }
    }

    #[test]
    fn draft_plan_requires_actions() {
        let err = ExecutionPlan::draft(
            InvestigationId::new(),
            ObjectId::new(),
            ObjectId::new(),
            1,
            "github.issue.comment",
            "github",
            "sandbox",
            vec![],
            Provenance::now("t", "t"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("at least one action"));
    }

    #[test]
    fn invalid_transition_rejected() {
        let plan = ExecutionPlan::draft(
            InvestigationId::new(),
            ObjectId::new(),
            ObjectId::new(),
            1,
            "mock.record",
            "mock",
            "sandbox",
            vec![sample_action()],
            Provenance::now("t", "t"),
        )
        .unwrap();
        let err = plan
            .transitioned(ExecutionPlanStatus::Executed, "actor", "nope", Utc::now())
            .unwrap_err();
        assert!(err
            .to_string()
            .contains("invalid execution plan transition"));
    }

    #[test]
    fn approval_binds_exact_revision() {
        let mut plan = ExecutionPlan::draft(
            InvestigationId::new(),
            ObjectId::new(),
            ObjectId::new(),
            1,
            "mock.record",
            "mock",
            "sandbox",
            vec![sample_action()],
            Provenance::now("t", "t"),
        )
        .unwrap();
        plan.target_snapshot = Some(TargetSnapshot::bind(
            &plan,
            CapabilityTarget {
                provider: "mock".into(),
                owner: None,
                repository: Some("local".into()),
                branch_or_ref: Some("sandbox".into()),
            },
        ));
        let ready = plan
            .transitioned(
                ExecutionPlanStatus::ReadyForReview,
                "actor",
                "validated",
                Utc::now(),
            )
            .unwrap();
        let decision = ExecutionPolicyDecision {
            decision: ExecutionPolicyDecisionKind::AllowedWithApproval,
            reasons: vec!["ok".into()],
            risk_level: CapabilityRiskLevel::LowRiskWrite,
            dry_run_permitted: true,
            live_execution_permitted: true,
            evaluated_at: Utc::now(),
        };
        let approval = ExecutionApproval::grant(
            &ready,
            "reviewer",
            "ship it",
            vec![],
            vec![],
            decision,
            None,
            true,
            Provenance::now("reviewer", "cli"),
        )
        .unwrap();
        let revised = ready.revised("actor", "change body", Utc::now()).unwrap();
        let err = approval.is_valid_for(&revised, Utc::now()).unwrap_err();
        assert!(err.to_string().contains("stale approval") || err.to_string().contains("snapshot"));
    }

    #[test]
    fn serialization_round_trip() {
        let plan = ExecutionPlan::draft(
            InvestigationId::new(),
            ObjectId::new(),
            ObjectId::new(),
            1,
            "mock.record",
            "mock",
            "sandbox",
            vec![sample_action()],
            Provenance::now("t", "t"),
        )
        .unwrap();
        let json = serde_json::to_string(&plan).unwrap();
        let back: ExecutionPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(plan, back);
    }
}
