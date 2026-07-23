//! Controlled external execution orchestration (RFC-025, RFC-026, RFC-027).
//!
//! Runtime owns plan lifecycle, approval, policy, capability invocation,
//! receipts, verification, and traceability. CLI/Workspace never call
//! external mutation APIs directly.

use chrono::{DateTime, Utc};

use crate::domain::{
    evaluate_execution_policy, verification_confidence, CapabilityExecutionStatus,
    CapabilityInvocation, CapabilityStateQuery, CapabilityTarget, CapabilityVerificationStatus,
    DryRunResult, ExecutionAction, ExecutionApproval, ExecutionAttempt, ExecutionAttemptListing,
    ExecutionAttemptStatus, ExecutionCapability, ExecutionCapabilityDescriptor,
    ExecutionCapabilityRegistry, ExecutionCheckResult, ExecutionPlan, ExecutionPlanListing,
    ExecutionPlanStatus, ExecutionPolicyDecision, ExecutionPolicyDecisionKind,
    ExecutionPrecondition, ExecutionReceipt, ExecutionReceiptListing, ExecutionReceiptResult,
    ExecutionTrace, ExecutionVerification, ExecutionVerificationStatus, ExpectedEffect,
    ImplementationRecord, ImplementationReference, ImplementationSource, InvestigationId, ObjectId,
    ProposalStatus, Provenance, RetrySafety, RollbackMetadata, SanitizationMetadata,
    TargetSnapshot,
};
use crate::error::{RivoraError, RivoraResult};
use crate::runtime::Runtime;

/// Request to create an Execution Plan from an accepted Proposal.
#[derive(Debug, Clone)]
pub struct CreateExecutionPlanRequest {
    /// Exact Proposal snapshot id.
    pub proposal_id: ObjectId,
    /// Capability id.
    pub capability_id: String,
    /// Target system family.
    pub target_system: String,
    /// Target environment.
    pub target_environment: String,
    /// Ordered actions.
    pub actions: Vec<ExecutionAction>,
    /// Shared inputs.
    pub inputs: serde_json::Value,
    /// Expected effects.
    pub expected_effects: Vec<ExpectedEffect>,
    /// Preconditions.
    pub preconditions: Vec<ExecutionPrecondition>,
    /// Whether the plan claims dry-run support.
    pub supports_dry_run: bool,
}

/// Request to revise plan content.
#[derive(Debug, Clone, Default)]
pub struct ReviseExecutionPlanRequest {
    /// Replacement actions (if set).
    pub actions: Option<Vec<ExecutionAction>>,
    /// Replacement inputs.
    pub inputs: Option<serde_json::Value>,
    /// Replacement expected effects.
    pub expected_effects: Option<Vec<ExpectedEffect>>,
    /// Replacement preconditions.
    pub preconditions: Option<Vec<ExecutionPrecondition>>,
    /// Replacement environment.
    pub target_environment: Option<String>,
    /// Replacement capability (invalidates prior approval).
    pub capability_id: Option<String>,
    /// Replacement target system.
    pub target_system: Option<String>,
}

impl Runtime {
    /// Register an execution capability adapter.
    pub fn register_execution_capability(
        &self,
        capability: std::sync::Arc<dyn crate::domain::ExecutionCapability>,
    ) -> RivoraResult<()> {
        self.execution_registry.register(capability)
    }

    /// Access the execution capability registry.
    pub fn execution_registry(&self) -> &ExecutionCapabilityRegistry {
        &self.execution_registry
    }

    /// List registered execution capability descriptors.
    pub fn list_execution_capabilities(&self) -> Vec<ExecutionCapabilityDescriptor> {
        self.execution_registry.list()
    }

    /// Show one execution capability descriptor.
    pub fn show_execution_capability(
        &self,
        capability_id: &str,
    ) -> RivoraResult<ExecutionCapabilityDescriptor> {
        self.execution_registry
            .get(capability_id)
            .map(|c| c.descriptor())
            .ok_or_else(|| {
                RivoraError::validation(format!("unknown execution capability `{capability_id}`"))
            })
    }

    /// Create a draft Execution Plan for an accepted Proposal.
    pub fn create_execution_plan(
        &self,
        investigation_id: InvestigationId,
        request: CreateExecutionPlanRequest,
        actor: impl Into<String>,
    ) -> RivoraResult<ExecutionPlan> {
        let actor = require_actor(actor)?;
        let _inv = self.store.load_investigation(&investigation_id)?;
        let proposal = self
            .store
            .load_proposal(&investigation_id, &request.proposal_id)?;
        if proposal.status != ProposalStatus::Accepted {
            return Err(RivoraError::precondition(format!(
                "execution plans require an accepted proposal; proposal {} is {}",
                proposal.id,
                proposal.status.as_str()
            )));
        }

        let provenance = Provenance::now(actor, "runtime")
            .with_capability("create_execution_plan")
            .with_evidence(vec![proposal.id]);

        let mut plan = ExecutionPlan::draft(
            investigation_id,
            proposal.id,
            proposal.lineage_id,
            proposal.revision_number,
            request.capability_id,
            request.target_system,
            request.target_environment,
            request.actions,
            provenance,
        )?;
        plan.inputs = request.inputs;
        plan.expected_effects = request.expected_effects;
        plan.preconditions = request.preconditions;
        plan.supports_dry_run = request.supports_dry_run;

        // Attach capability defaults when registered.
        if let Some(cap) = self.execution_registry.get(&plan.capability_id) {
            let desc = cap.descriptor();
            let target = self.resolve_plan_target(&plan, cap.as_ref())?;
            plan.target_snapshot = Some(TargetSnapshot::bind(&plan, target.clone()));
            plan.scope_restrictions.repositories = target
                .owner
                .as_ref()
                .zip(target.repository.as_ref())
                .map(|(owner, repository)| vec![format!("{owner}/{repository}")])
                .unwrap_or_default();
            plan.scope_restrictions.action_names = plan
                .actions
                .iter()
                .map(|action| action.action_name.clone())
                .collect();
            plan.scope_restrictions.max_actions = Some(20);
            plan.supports_dry_run = plan.supports_dry_run && desc.supports_dry_run;
            if plan.verification_plan.checks.is_empty() {
                plan.verification_plan.checks = desc
                    .supported_actions
                    .iter()
                    .map(|a| format!("verify action {a}"))
                    .collect();
            }
        }

        let policy = self.evaluate_plan_policy(&plan);
        plan.last_policy_decision = Some(policy);

        self.store.append_execution_plan(&plan)?;
        Ok(plan)
    }

    /// Revise a plan (creates immutable successor; invalidates prior approvals).
    pub fn revise_execution_plan(
        &self,
        investigation_id: InvestigationId,
        plan_id: ObjectId,
        request: ReviseExecutionPlanRequest,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<ExecutionPlan> {
        let actor = require_actor(actor)?;
        let reason = require_reason(reason)?;
        self.ensure_plan_head(investigation_id, plan_id)?;
        let current = self
            .store
            .load_execution_plan(&investigation_id, &plan_id)?;
        let mut next = current.revised(&actor, &reason, Utc::now())?;
        if let Some(actions) = request.actions {
            if actions.is_empty() {
                return Err(RivoraError::validation(
                    "revised plan must keep at least one action",
                ));
            }
            next.actions = actions;
        }
        if let Some(inputs) = request.inputs {
            next.inputs = inputs;
        }
        if let Some(effects) = request.expected_effects {
            next.expected_effects = effects;
        }
        if let Some(pre) = request.preconditions {
            next.preconditions = pre;
        }
        if let Some(env) = request.target_environment {
            let env = env.trim().to_string();
            if env.is_empty() {
                return Err(RivoraError::validation(
                    "target_environment must not be empty",
                ));
            }
            next.target_environment = env;
        }
        if let Some(cap) = request.capability_id {
            let cap = cap.trim().to_string();
            if cap.is_empty() {
                return Err(RivoraError::validation("capability_id must not be empty"));
            }
            next.capability_id = cap;
        }
        if let Some(sys) = request.target_system {
            let sys = sys.trim().to_string();
            if sys.is_empty() {
                return Err(RivoraError::validation("target_system must not be empty"));
            }
            next.target_system = sys;
        }
        if let Some(capability) = self.execution_registry.get(&next.capability_id) {
            let target = self.resolve_plan_target(&next, capability.as_ref())?;
            next.target_snapshot = Some(TargetSnapshot::bind(&next, target.clone()));
            next.scope_restrictions.repositories = target
                .owner
                .as_ref()
                .zip(target.repository.as_ref())
                .map(|(owner, repository)| vec![format!("{owner}/{repository}")])
                .unwrap_or_default();
            next.scope_restrictions.action_names = next
                .actions
                .iter()
                .map(|action| action.action_name.clone())
                .collect();
            next.scope_restrictions.max_actions = Some(20);
        } else {
            next.target_snapshot = None;
        }
        next.last_policy_decision = Some(self.evaluate_plan_policy(&next));
        if current.status == ExecutionPlanStatus::Approved {
            let mut superseded = current.transitioned(
                ExecutionPlanStatus::Superseded,
                &actor,
                format!(
                    "superseded by edited plan revision {}",
                    next.revision_number.saturating_add(1)
                ),
                Utc::now(),
            )?;
            next.parent_plan_id = Some(superseded.id);
            next.revision_number = superseded.revision_number.saturating_add(1);
            next.target_snapshot = next
                .target_snapshot
                .as_ref()
                .map(|target| target.rebound_to(next.id, next.revision_number));
            superseded.superseding_plan_id = Some(next.id);
            self.store.append_execution_plan(&superseded)?;
        }
        self.store.append_execution_plan(&next)?;

        // Invalidate approvals that pointed at the prior snapshot.
        for approval in self
            .store
            .list_execution_approvals(&investigation_id)?
            .approvals
        {
            if approval.plan_id == plan_id && !approval.invalidated {
                let invalidated = approval.invalidate(format!(
                    "plan revised to snapshot {} (revision {})",
                    next.id, next.revision_number
                ));
                self.store.save_execution_approval(&invalidated)?;
            }
        }
        Ok(next)
    }

    /// Validate preconditions and mark plan ReadyForReview when policy allows review.
    pub fn validate_execution_plan(
        &self,
        investigation_id: InvestigationId,
        plan_id: ObjectId,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<ExecutionPlan> {
        let actor = require_actor(actor)?;
        let reason = require_reason(reason)?;
        self.ensure_plan_head(investigation_id, plan_id)?;
        let plan = self
            .store
            .load_execution_plan(&investigation_id, &plan_id)?;
        if plan.status != ExecutionPlanStatus::Draft
            && plan.status != ExecutionPlanStatus::ReadyForReview
        {
            return Err(RivoraError::validation(format!(
                "validate_execution_plan requires draft or ready_for_review, got {}",
                plan.status.as_str()
            )));
        }

        let capability = self
            .execution_registry
            .get(&plan.capability_id)
            .ok_or_else(|| {
                RivoraError::precondition(format!(
                    "capability `{}` is not registered",
                    plan.capability_id
                ))
            })?;
        let target = self.validate_plan_contract(&plan, capability.as_ref())?;
        let policy = self.evaluate_plan_policy(&plan);
        if policy.decision == ExecutionPolicyDecisionKind::Denied {
            return Err(RivoraError::precondition(format!(
                "policy denied plan: {}",
                policy.reasons.join("; ")
            )));
        }

        let at = Utc::now();
        let mut next = if plan.status == ExecutionPlanStatus::Draft {
            plan.transitioned(ExecutionPlanStatus::ReadyForReview, &actor, &reason, at)?
        } else {
            // Already ReadyForReview: content-preserving successor (same status).
            let mut successor = plan.clone();
            successor.id = ObjectId::new();
            successor.parent_plan_id = Some(plan.id);
            successor.revision_number = plan.revision_number.saturating_add(1);
            successor.updated_at = at;
            successor.provenance = Provenance::now(&actor, "runtime")
                .with_capability("validate_execution_plan")
                .with_evidence(vec![plan.id]);
            successor
                .transitions
                .push(crate::domain::ExecutionPlanTransition {
                    from: plan.status,
                    to: ExecutionPlanStatus::ReadyForReview,
                    actor: actor.clone(),
                    reason: format!("revalidated: {reason}"),
                    at,
                });
            successor
        };
        next.target_snapshot = Some(TargetSnapshot::bind(&next, target.clone()));
        next.scope_restrictions.repositories = target
            .owner
            .as_ref()
            .zip(target.repository.as_ref())
            .map(|(owner, repository)| vec![format!("{owner}/{repository}")])
            .unwrap_or_default();
        next.scope_restrictions.action_names = next
            .actions
            .iter()
            .map(|action| action.action_name.clone())
            .collect();
        next.scope_restrictions.max_actions = Some(20);
        next.last_policy_decision = Some(policy);
        self.store.append_execution_plan(&next)?;
        Ok(next)
    }

    /// Preview / dry-run a plan without mutating external systems.
    pub fn preview_execution_plan(
        &self,
        investigation_id: InvestigationId,
        plan_id: ObjectId,
    ) -> RivoraResult<DryRunResult> {
        let plan = self
            .store
            .load_execution_plan(&investigation_id, &plan_id)?;
        let policy = self.evaluate_plan_policy(&plan);
        if !policy.dry_run_permitted && policy.decision == ExecutionPolicyDecisionKind::Denied {
            return Err(RivoraError::precondition(format!(
                "policy denied preview: {}",
                policy.reasons.join("; ")
            )));
        }

        let cap = self
            .execution_registry
            .get(&plan.capability_id)
            .ok_or_else(|| {
                RivoraError::precondition(format!(
                    "capability `{}` is not registered",
                    plan.capability_id
                ))
            })?;

        if !plan.supports_dry_run || !cap.descriptor().supports_dry_run {
            // Plan validation only — no simulated mutation certainty.
            return Ok(DryRunResult {
                actions: plan.actions.iter().map(|a| a.action_name.clone()).collect(),
                target: format!("{}:{}", plan.target_system, plan.target_environment),
                expected_mutations: plan
                    .expected_effects
                    .iter()
                    .map(|e| e.description.clone())
                    .collect(),
                required_permissions: cap.descriptor().credential_requirements,
                current_state: None,
                predicted_state: None,
                risks: plan.risks.iter().map(|r| r.description.clone()).collect(),
                policy_decision: policy,
                missing_preconditions: plan
                    .preconditions
                    .iter()
                    .filter(|p| p.satisfied == Some(false))
                    .map(|p| p.description.clone())
                    .collect(),
                verification_steps: plan.verification_plan.checks.clone(),
                rollback_options: if plan.rollback.available {
                    vec!["rollback metadata present".into()]
                } else {
                    vec!["no automatic rollback".into()]
                },
                simulated: false,
            });
        }

        // Dry-run first action thoroughly; aggregate summaries for multi-action plans.
        let mut aggregate: Option<DryRunResult> = None;
        for action in &plan.actions {
            let invocation = CapabilityInvocation {
                capability_id: plan.capability_id.clone(),
                action_name: action.action_name.clone(),
                action_id: action.action_id.clone(),
                inputs: merge_inputs(&plan.inputs, &action.inputs),
                environment: plan.target_environment.clone(),
                idempotency_key: format!("dry-run:{}:{}", plan.id, action.action_id),
                investigation_id: investigation_id.to_string(),
                plan_id: plan.id.to_string(),
            };
            let result = cap.dry_run(&invocation)?;
            aggregate = Some(match aggregate {
                None => result,
                Some(mut acc) => {
                    acc.actions.extend(result.actions);
                    acc.expected_mutations.extend(result.expected_mutations);
                    acc.risks.extend(result.risks);
                    acc.missing_preconditions
                        .extend(result.missing_preconditions);
                    acc.verification_steps.extend(result.verification_steps);
                    acc
                }
            });
        }
        let mut result = aggregate
            .ok_or_else(|| RivoraError::validation("preview requires at least one action"))?;
        result.policy_decision = policy;
        Ok(result)
    }

    /// Approve an exact plan revision for live execution.
    #[allow(clippy::too_many_arguments)]
    pub fn approve_execution_plan(
        &self,
        investigation_id: InvestigationId,
        plan_id: ObjectId,
        actor: impl Into<String>,
        reason: impl Into<String>,
        approved_actions: Vec<String>,
        denied_actions: Vec<String>,
        expires_at: Option<DateTime<Utc>>,
        one_time: bool,
    ) -> RivoraResult<(ExecutionPlan, ExecutionApproval)> {
        let actor = require_actor(actor)?;
        let reason = require_reason(reason)?;
        self.ensure_plan_head(investigation_id, plan_id)?;
        let plan = self
            .store
            .load_execution_plan(&investigation_id, &plan_id)?;
        if plan.status != ExecutionPlanStatus::ReadyForReview
            && plan.status != ExecutionPlanStatus::Approved
        {
            return Err(RivoraError::precondition(format!(
                "approve requires ready_for_review (or re-approve approved); got {}",
                plan.status.as_str()
            )));
        }

        let policy = self.evaluate_plan_policy(&plan);
        if !policy.live_execution_permitted
            || matches!(
                policy.decision,
                ExecutionPolicyDecisionKind::Denied
                    | ExecutionPolicyDecisionKind::AllowedDryRunOnly
            )
        {
            return Err(RivoraError::precondition(format!(
                "policy does not permit live execution approval: {}",
                policy.reasons.join("; ")
            )));
        }

        let provenance = Provenance::now(&actor, "runtime")
            .with_capability("approve_execution_plan")
            .with_evidence(vec![plan.id]);
        let approval = ExecutionApproval::grant(
            &plan,
            &actor,
            &reason,
            approved_actions,
            denied_actions,
            policy.clone(),
            expires_at,
            one_time,
            provenance,
        )?;
        self.store.save_execution_approval(&approval)?;

        let approved_plan = if plan.status == ExecutionPlanStatus::ReadyForReview {
            let mut next =
                plan.transitioned(ExecutionPlanStatus::Approved, &actor, &reason, Utc::now())?;
            next.last_policy_decision = Some(policy.clone());
            self.store.append_execution_plan(&next)?;
            // Approval was bound to pre-transition snapshot; re-bind to approved snapshot.
            // Exact-revision binding: re-issue approval for the Approved snapshot.
            let rebound = ExecutionApproval::grant(
                &next,
                &actor,
                &reason,
                approval.approved_actions.clone(),
                approval.denied_actions.clone(),
                policy,
                expires_at,
                one_time,
                Provenance::now(&actor, "runtime")
                    .with_capability("approve_execution_plan")
                    .with_evidence(vec![next.id]),
            )?;
            // Invalidate the intermediate approval that bound ReadyForReview snapshot.
            let invalidated = approval
                .invalidate("superseded by approval rebound to Approved snapshot".to_string());
            self.store.save_execution_approval(&invalidated)?;
            self.store.save_execution_approval(&rebound)?;
            (next, rebound)
        } else {
            (plan, approval)
        };
        Ok(approved_plan)
    }

    /// Reject a plan.
    pub fn reject_execution_plan(
        &self,
        investigation_id: InvestigationId,
        plan_id: ObjectId,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<ExecutionPlan> {
        self.transition_plan(
            investigation_id,
            plan_id,
            ExecutionPlanStatus::Rejected,
            actor,
            reason,
        )
    }

    /// Cancel a plan.
    pub fn cancel_execution_plan(
        &self,
        investigation_id: InvestigationId,
        plan_id: ObjectId,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<ExecutionPlan> {
        self.transition_plan(
            investigation_id,
            plan_id,
            ExecutionPlanStatus::Cancelled,
            actor,
            reason,
        )
    }

    /// Execute an approved plan (or dry-run when `dry_run` is true).
    pub fn execute_plan(
        &self,
        investigation_id: InvestigationId,
        plan_id: ObjectId,
        approval_id: ObjectId,
        actor: impl Into<String>,
        idempotency_key: impl Into<String>,
        dry_run: bool,
    ) -> RivoraResult<ExecutionAttempt> {
        let actor = require_actor(actor)?;
        let idempotency_key = {
            let k = idempotency_key.into().trim().to_string();
            if k.is_empty() || k.len() > 256 || k.chars().any(char::is_control) {
                return Err(RivoraError::validation(
                    "idempotency_key must be a non-empty bounded printable value",
                ));
            }
            k
        };
        let effective_key = format!(
            "mode={};key={idempotency_key}",
            if dry_run { "dry_run" } else { "live" }
        );
        let plan = self
            .store
            .load_execution_plan(&investigation_id, &plan_id)?;
        let approval = self
            .store
            .load_execution_approval(&investigation_id, &approval_id)?;

        // A duplicate request is a new durable trace record, but it never invokes
        // the adapter. Keys bind the exact Plan snapshot, capability, target, and mode.
        if let Some(existing) =
            self.find_attempt_by_idempotency(investigation_id, &effective_key)?
        {
            if existing.plan_id != plan.id
                || existing.capability_id != plan.capability_id
                || existing.target_snapshot != plan.target_snapshot
                || existing.dry_run != dry_run
            {
                return Err(RivoraError::precondition(format!(
                    "idempotency key is already reserved by attempt {} for a different exact execution",
                    existing.id
                )));
            }
            let existing = if existing.status == ExecutionAttemptStatus::Started {
                let mut recovered = existing.revised(
                    Provenance::now(&actor, "runtime")
                        .with_capability("recover_interrupted_execution")
                        .with_evidence(vec![existing.id]),
                );
                recovered.status = ExecutionAttemptStatus::PartiallyCompleted;
                recovered.uncertain_actions = recovered.requested_actions.clone();
                recovered.errors.push(
                    "process interruption left the external mutation outcome uncertain".into(),
                );
                recovered.finished_at = Some(Utc::now());
                recovered.retry_safety = RetrySafety::Unsafe;
                recovered.recommended_next_action = Some(
                    "independently observe external state before authoring any new plan".into(),
                );
                self.store.append_execution_attempt(&recovered)?;
                recovered
            } else {
                existing
            };
            let mut duplicate = ExecutionAttempt::start(
                &plan,
                &approval,
                &actor,
                &effective_key,
                dry_run,
                Provenance::now(&actor, "runtime")
                    .with_capability("suppress_duplicate_execution")
                    .with_evidence(vec![existing.id]),
            )?;
            duplicate.status = ExecutionAttemptStatus::DuplicateSuppressed;
            duplicate.duplicate_of_attempt_id = Some(existing.id);
            duplicate.finished_at = Some(Utc::now());
            duplicate.retry_safety = existing.retry_safety;
            duplicate.recommended_next_action =
                Some("inspect the original durable Attempt; no mutation was repeated".into());
            self.store.append_execution_attempt(&duplicate)?;
            return Ok(duplicate);
        }

        self.ensure_plan_head(investigation_id, plan_id)?;

        if dry_run {
            let policy = self.evaluate_plan_policy(&plan);
            if !policy.dry_run_permitted {
                return Err(RivoraError::precondition(format!(
                    "dry-run not permitted: {}",
                    policy.reasons.join("; ")
                )));
            }
            let preview = self.preview_execution_plan(investigation_id, plan_id)?;
            let started = ExecutionAttempt::start(
                &plan,
                &approval,
                &actor,
                &effective_key,
                true,
                Provenance::now(&actor, "runtime").with_capability("execute_plan"),
            )?;
            self.store.append_execution_attempt(&started)?;
            let mut attempt = started.revised(
                Provenance::now(&actor, "runtime")
                    .with_capability("complete_dry_run")
                    .with_evidence(vec![started.id]),
            );
            attempt.status = ExecutionAttemptStatus::Completed;
            attempt.completed_actions = attempt.requested_actions.clone();
            attempt.finished_at = Some(Utc::now());
            attempt.retry_safety = RetrySafety::Safe;
            attempt.recommended_next_action =
                Some("review dry-run result then approve live execution".into());
            attempt.errors = preview.missing_preconditions;
            self.store.append_execution_attempt(&attempt)?;
            return Ok(attempt);
        }

        if plan.status != ExecutionPlanStatus::Approved {
            return Err(RivoraError::precondition(format!(
                "live execution requires approved plan; got {}",
                plan.status.as_str()
            )));
        }
        approval.is_valid_for(&plan, Utc::now())?;

        let policy = self.evaluate_plan_policy(&plan);
        if !policy.live_execution_permitted
            || policy.decision == ExecutionPolicyDecisionKind::Denied
            || policy.decision == ExecutionPolicyDecisionKind::AllowedDryRunOnly
        {
            return Err(RivoraError::precondition(format!(
                "policy denied live execution: {}",
                policy.reasons.join("; ")
            )));
        }

        let cap = self
            .execution_registry
            .get(&plan.capability_id)
            .ok_or_else(|| {
                RivoraError::precondition(format!(
                    "capability `{}` is not registered",
                    plan.capability_id
                ))
            })?;
        self.validate_plan_contract(&plan, cap.as_ref())?;

        // Reserve the exact execution durably before checking dynamic preconditions.
        let started = ExecutionAttempt::start(
            &plan,
            &approval,
            &actor,
            &effective_key,
            false,
            Provenance::now(&actor, "runtime").with_capability("execute_plan"),
        )?;
        self.store.append_execution_attempt(&started)?;

        let mut blocked_errors: Vec<String> = plan
            .preconditions
            .iter()
            .filter(|precondition| precondition.satisfied != Some(true))
            .map(|precondition| {
                format!(
                    "precondition {}: {}",
                    precondition.id,
                    precondition
                        .detail
                        .clone()
                        .unwrap_or_else(|| precondition.description.clone())
                )
            })
            .collect();
        for action in &plan.actions {
            if !approval.approved_actions.is_empty()
                && !approval.approved_actions.contains(&action.action_id)
            {
                continue;
            }
            if approval.denied_actions.contains(&action.action_id) {
                continue;
            }
            let invocation = CapabilityInvocation {
                capability_id: plan.capability_id.clone(),
                action_name: action.action_name.clone(),
                action_id: action.action_id.clone(),
                inputs: merge_inputs(&plan.inputs, &action.inputs),
                environment: plan.target_environment.clone(),
                idempotency_key: format!("{};action={}", effective_key, action.action_id),
                investigation_id: investigation_id.to_string(),
                plan_id: plan.id.to_string(),
            };
            if let Err(error) = cap.validate_preconditions(&invocation) {
                blocked_errors.push(error.to_string());
            }
        }
        if !blocked_errors.is_empty() {
            let mut attempt = started.revised(
                Provenance::now(&actor, "runtime")
                    .with_capability("block_execution_attempt")
                    .with_evidence(vec![started.id]),
            );
            attempt.status = ExecutionAttemptStatus::Blocked;
            attempt.errors = blocked_errors;
            attempt.finished_at = Some(Utc::now());
            attempt.retry_safety = RetrySafety::ConditionallySafe;
            attempt.recommended_next_action =
                Some("resolve preconditions and re-validate plan".into());
            self.store.append_execution_attempt(&attempt)?;
            return Ok(attempt);
        }

        // Consume one-time authority and persist Executing before external mutation.
        if approval.one_time {
            self.store
                .save_execution_approval(&approval.mark_consumed())?;
        }
        let mut executing = plan.transitioned(
            ExecutionPlanStatus::Executing,
            &actor,
            "begin external invocation",
            Utc::now(),
        )?;
        executing.last_policy_decision = Some(policy);
        self.store.append_execution_plan(&executing)?;

        let mut attempt = started.revised(
            Provenance::now(&actor, "runtime")
                .with_capability("complete_execution_attempt")
                .with_evidence(vec![started.id]),
        );
        let mut any_success = false;
        let mut any_failure = false;
        let mut any_uncertain = false;
        let mut stop = false;

        for action in &plan.actions {
            if stop {
                attempt.skipped_actions.push(action.action_id.clone());
                continue;
            }
            if !approval.approved_actions.is_empty()
                && !approval.approved_actions.contains(&action.action_id)
            {
                attempt.skipped_actions.push(action.action_id.clone());
                continue;
            }
            if approval.denied_actions.contains(&action.action_id) {
                attempt.skipped_actions.push(action.action_id.clone());
                continue;
            }

            let invocation = CapabilityInvocation {
                capability_id: plan.capability_id.clone(),
                action_name: action.action_name.clone(),
                action_id: action.action_id.clone(),
                inputs: merge_inputs(&plan.inputs, &action.inputs),
                environment: plan.target_environment.clone(),
                idempotency_key: format!("{};action={}", effective_key, action.action_id),
                investigation_id: investigation_id.to_string(),
                plan_id: plan.id.to_string(),
            };

            match cap.execute(&invocation) {
                Ok(result) => {
                    let receipt_status = match result.status {
                        CapabilityExecutionStatus::Success
                        | CapabilityExecutionStatus::DuplicateSuppressed => {
                            ExecutionReceiptResult::Success
                        }
                        CapabilityExecutionStatus::Partial => ExecutionReceiptResult::Partial,
                        CapabilityExecutionStatus::Uncertain => ExecutionReceiptResult::Uncertain,
                        CapabilityExecutionStatus::Failed => ExecutionReceiptResult::Failed,
                    };
                    let receipt = ExecutionReceipt {
                        id: ObjectId::new(),
                        attempt_id: attempt.lineage_id(),
                        investigation_id,
                        capability_id: plan.capability_id.clone(),
                        target_system: plan.target_system.clone(),
                        action_name: action.action_name.clone(),
                        action_id: action.action_id.clone(),
                        request_summary: result.request_summary,
                        response_summary: result.response_summary,
                        changed_resources: result.changed_resources,
                        unchanged_resources: result.unchanged_resources,
                        external_identifiers: result.external_identifiers.clone(),
                        result_status: receipt_status,
                        warnings: result.warnings,
                        rollback_metadata: result.rollback.clone(),
                        verification_requirements: result.verification_requirements,
                        raw_evidence_refs: result.evidence_refs,
                        sanitization: SanitizationMetadata {
                            redacted_keys: vec![
                                "token".into(),
                                "authorization".into(),
                                "password".into(),
                                "secret".into(),
                            ],
                            raw_body_discarded: true,
                        },
                        provenance: Provenance::now(&actor, "runtime")
                            .with_capability("record_execution_receipt"),
                        created_at: Utc::now(),
                    };
                    attempt.receipt_ids.push(receipt.id);
                    attempt
                        .external_references
                        .extend(result.external_identifiers);
                    match result.status {
                        CapabilityExecutionStatus::Success
                        | CapabilityExecutionStatus::DuplicateSuppressed => {
                            any_success = true;
                            attempt.completed_actions.push(action.action_id.clone());
                            if result.rollback.available {
                                attempt.rollback.available = true;
                            }
                        }
                        CapabilityExecutionStatus::Failed => {
                            any_failure = true;
                            attempt.failed_actions.push(action.action_id.clone());
                        }
                        CapabilityExecutionStatus::Partial
                        | CapabilityExecutionStatus::Uncertain => {
                            any_uncertain = true;
                            attempt.uncertain_actions.push(action.action_id.clone());
                        }
                    }
                    if let Some(err) = result.error {
                        attempt.errors.push(err);
                    }
                    if !matches!(
                        result.status,
                        CapabilityExecutionStatus::Success
                            | CapabilityExecutionStatus::DuplicateSuppressed
                    ) && !action.continue_on_failure
                    {
                        stop = true;
                    }
                    self.store.append_execution_receipt(&receipt)?;
                }
                Err(err) => {
                    // An unexpected adapter error after invocation may have occurred
                    // after submission. Conservatively retain it as uncertain.
                    any_uncertain = true;
                    attempt.uncertain_actions.push(action.action_id.clone());
                    attempt.errors.push(err.to_string());
                    let receipt = ExecutionReceipt {
                        id: ObjectId::new(),
                        attempt_id: attempt.lineage_id(),
                        investigation_id,
                        capability_id: plan.capability_id.clone(),
                        target_system: plan.target_system.clone(),
                        action_name: action.action_name.clone(),
                        action_id: action.action_id.clone(),
                        request_summary: format!("invoke {}", action.action_name),
                        response_summary: "adapter outcome uncertain".into(),
                        changed_resources: vec![],
                        unchanged_resources: vec![],
                        external_identifiers: vec![],
                        result_status: ExecutionReceiptResult::Uncertain,
                        warnings: vec!["do not retry until independently observed".into()],
                        rollback_metadata: RollbackMetadata::default(),
                        verification_requirements: vec![
                            "determine whether the external mutation occurred".into(),
                        ],
                        raw_evidence_refs: vec![],
                        sanitization: SanitizationMetadata {
                            redacted_keys: vec![
                                "token".into(),
                                "authorization".into(),
                                "password".into(),
                                "secret".into(),
                            ],
                            raw_body_discarded: true,
                        },
                        provenance: Provenance::now(&actor, "runtime")
                            .with_capability("record_uncertain_execution_receipt"),
                        created_at: Utc::now(),
                    };
                    attempt.receipt_ids.push(receipt.id);
                    self.store.append_execution_receipt(&receipt)?;
                    if !action.continue_on_failure {
                        stop = true;
                    }
                }
            }
        }

        attempt.finished_at = Some(Utc::now());
        attempt.status = match (any_success, any_failure, any_uncertain) {
            (true, false, false) if attempt.skipped_actions.is_empty() => {
                ExecutionAttemptStatus::Completed
            }
            (true, _, _) | (_, _, true) => ExecutionAttemptStatus::PartiallyCompleted,
            (false, true, false) => ExecutionAttemptStatus::Failed,
            (false, false, false) => ExecutionAttemptStatus::Blocked,
        };
        attempt.retry_safety = if any_uncertain {
            RetrySafety::Unsafe
        } else {
            match attempt.status {
                ExecutionAttemptStatus::Completed | ExecutionAttemptStatus::DuplicateSuppressed => {
                    RetrySafety::Safe
                }
                ExecutionAttemptStatus::PartiallyCompleted => RetrySafety::Unsafe,
                ExecutionAttemptStatus::Failed | ExecutionAttemptStatus::Blocked => {
                    RetrySafety::Unknown
                }
                ExecutionAttemptStatus::Started => RetrySafety::Unknown,
            }
        };
        attempt.recommended_next_action = Some(match attempt.status {
            ExecutionAttemptStatus::Completed => "verify_execution_attempt".into(),
            ExecutionAttemptStatus::PartiallyCompleted => {
                "inspect receipts; create new plan revision for remaining actions".into()
            }
            ExecutionAttemptStatus::Failed => {
                "inspect errors; do not auto-retry when retry_safety is unknown/unsafe".into()
            }
            ExecutionAttemptStatus::Blocked => "resolve blockers and re-approve if needed".into(),
            ExecutionAttemptStatus::DuplicateSuppressed => {
                "use existing attempt and verify if needed".into()
            }
            ExecutionAttemptStatus::Started => "wait for completion".into(),
        });

        let plan_status = match attempt.status {
            ExecutionAttemptStatus::Completed | ExecutionAttemptStatus::DuplicateSuppressed => {
                ExecutionPlanStatus::Executed
            }
            ExecutionAttemptStatus::PartiallyCompleted => ExecutionPlanStatus::PartiallyExecuted,
            _ => ExecutionPlanStatus::Failed,
        };
        let finished_plan = executing.transitioned(
            plan_status,
            &actor,
            format!(
                "attempt {} finished as {}",
                attempt.id,
                attempt.status.as_str()
            ),
            Utc::now(),
        )?;
        self.store.append_execution_plan(&finished_plan)?;
        self.store.append_execution_attempt(&attempt)?;
        Ok(attempt)
    }

    /// Verify an attempt independently of API success responses.
    pub fn verify_execution_attempt(
        &self,
        investigation_id: InvestigationId,
        attempt_id: ObjectId,
        actor: impl Into<String>,
    ) -> RivoraResult<ExecutionVerification> {
        let actor = require_actor(actor)?;
        let attempt = self
            .store
            .load_execution_attempt(&investigation_id, &attempt_id)?;
        if attempt.dry_run {
            return Err(RivoraError::validation(
                "cannot verify a dry-run attempt as live execution",
            ));
        }
        if matches!(
            attempt.status,
            ExecutionAttemptStatus::Started
                | ExecutionAttemptStatus::Blocked
                | ExecutionAttemptStatus::DuplicateSuppressed
        ) {
            return Err(RivoraError::validation(format!(
                "cannot verify an attempt in status {}",
                attempt.status.as_str()
            )));
        }
        let plan = self
            .store
            .load_execution_plan(&investigation_id, &attempt.plan_id)?;
        let cap = self
            .execution_registry
            .get(&attempt.capability_id)
            .ok_or_else(|| {
                RivoraError::precondition(format!(
                    "capability `{}` is not registered",
                    attempt.capability_id
                ))
            })?;

        let receipts = self.store.list_execution_receipts(&investigation_id)?;
        let attempt_receipts: Vec<_> = receipts
            .receipts
            .into_iter()
            .filter(|r| r.attempt_id == attempt.lineage_id())
            .collect();

        let mut checks = plan.verification_plan.checks.clone();
        if checks.is_empty() {
            checks = attempt_receipts
                .iter()
                .flat_map(|r| r.verification_requirements.clone())
                .collect();
        }
        if checks.is_empty() {
            checks.push("external identifiers present".into());
        }

        let mut results = Vec::new();
        let mut contradictions = Vec::new();
        let mut evidence = Vec::new();

        for receipt in &attempt_receipts {
            let action = plan
                .actions
                .iter()
                .find(|a| a.action_id == receipt.action_id);
            let inputs = action
                .map(|a| merge_inputs(&plan.inputs, &a.inputs))
                .unwrap_or_else(|| plan.inputs.clone());
            let query = CapabilityStateQuery {
                capability_id: attempt.capability_id.clone(),
                action_name: receipt.action_name.clone(),
                inputs: inputs.clone(),
                external_identifiers: receipt.external_identifiers.clone(),
                environment: attempt.environment.clone(),
            };
            let observation = cap.observe_state(&query)?;
            evidence.push(observation.summary.clone());

            let capability_passed = observation.verification_status
                == CapabilityVerificationStatus::Passed
                && observation.observed
                && observation.error.is_none();
            match observation.verification_status {
                CapabilityVerificationStatus::Passed if capability_passed => {}
                CapabilityVerificationStatus::Passed => contradictions.push(format!(
                    "receipt {} returned an internally inconsistent passing observation",
                    receipt.id
                )),
                CapabilityVerificationStatus::Failed => contradictions.push(format!(
                    "receipt {} postcondition contradicted: {}",
                    receipt.id, observation.summary
                )),
                CapabilityVerificationStatus::Inconclusive => {}
            }

            // Expected fields from plan effects.
            for effect in &plan.expected_effects {
                for (field, expected) in &effect.expected_fields {
                    let actual = observation.fields.get(field).cloned().unwrap_or_default();
                    let passed = actual == *expected;
                    if !passed {
                        contradictions
                            .push(format!("expected {field}={expected}, observed {actual:?}"));
                    }
                    results.push(ExecutionCheckResult {
                        check: format!("{}:{field}", effect.resource_type),
                        passed,
                        detail: format!("expected={expected} actual={actual}"),
                        evidence: vec![observation.summary.clone()],
                    });
                }
            }

            results.push(ExecutionCheckResult {
                check: format!("receipt:{}", receipt.action_id),
                passed: capability_passed,
                detail: format!(
                    "{}; independent observation: {}",
                    receipt.response_summary, observation.summary
                ),
                evidence: receipt
                    .raw_evidence_refs
                    .iter()
                    .cloned()
                    .chain(std::iter::once(observation.resource_key))
                    .collect(),
            });
        }

        if results.is_empty() {
            results.push(ExecutionCheckResult {
                check: "no receipts".into(),
                passed: false,
                detail: "attempt has no receipts to verify".into(),
                evidence: vec![],
            });
        }

        let passed = results.iter().filter(|r| r.passed).count();
        let total = results.len();
        let has_inconclusive = attempt_receipts.iter().enumerate().any(|(index, _)| {
            results
                .iter()
                .filter(|result| result.check.starts_with("receipt:"))
                .nth(index)
                .is_some_and(|result| !result.passed)
        }) && contradictions.is_empty();
        let status = if !attempt.failed_actions.is_empty() || !contradictions.is_empty() {
            ExecutionVerificationStatus::Failed
        } else if has_inconclusive || !attempt.uncertain_actions.is_empty() && passed != total {
            ExecutionVerificationStatus::Inconclusive
        } else if passed == total {
            ExecutionVerificationStatus::Passed
        } else if passed == 0 {
            ExecutionVerificationStatus::Failed
        } else {
            ExecutionVerificationStatus::Inconclusive
        };

        let prior_verifications = self
            .store
            .list_execution_verifications(&investigation_id)?
            .verifications;
        let prior = prior_verifications
            .iter()
            .filter(|verification| verification.attempt_id == attempt.lineage_id())
            .max_by_key(|verification| verification.revision);
        let verification = ExecutionVerification {
            id: ObjectId::new(),
            parent_verification_id: prior.map(|verification| verification.id),
            attempt_id: attempt.lineage_id(),
            receipt_ids: attempt_receipts.iter().map(|r| r.id).collect(),
            investigation_id,
            checks,
            results,
            status,
            confidence: verification_confidence(passed, total),
            contradictions,
            unresolved_risks: plan.risks.iter().map(|r| r.description.clone()).collect(),
            actor: actor.clone(),
            evidence,
            provenance: Provenance::now(&actor, "runtime")
                .with_capability("verify_execution_attempt"),
            created_at: Utc::now(),
            revision: prior
                .map(|verification| verification.revision.saturating_add(1))
                .unwrap_or(1),
        };
        self.store.append_execution_verification(&verification)?;

        // Advance plan lifecycle when verification passes.
        if let Ok(head) = self.latest_plan_in_lineage(investigation_id, plan.lineage_id) {
            if verification.status == ExecutionVerificationStatus::Passed
                && matches!(
                    head.status,
                    ExecutionPlanStatus::Executed | ExecutionPlanStatus::PartiallyExecuted
                )
            {
                let verified = head.transitioned(
                    ExecutionPlanStatus::Verified,
                    &actor,
                    format!("verification {} passed", verification.id),
                    Utc::now(),
                )?;
                self.store.append_execution_plan(&verified)?;
            }
        }

        Ok(verification)
    }

    /// Close a verified plan (ready for Outcome measurement).
    pub fn close_execution_plan(
        &self,
        investigation_id: InvestigationId,
        plan_id: ObjectId,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<ExecutionPlan> {
        self.transition_plan(
            investigation_id,
            plan_id,
            ExecutionPlanStatus::Closed,
            actor,
            reason,
        )
    }

    /// Link a successful execution attempt to a new Implementation Record.
    pub fn link_execution_to_implementation(
        &self,
        investigation_id: InvestigationId,
        attempt_id: ObjectId,
        actor: impl Into<String>,
        summary: impl Into<String>,
    ) -> RivoraResult<ImplementationRecord> {
        let actor = require_actor(actor)?;
        let summary = {
            let s = summary.into().trim().to_string();
            if s.is_empty() {
                return Err(RivoraError::validation("summary must not be empty"));
            }
            s
        };
        let attempt = self
            .store
            .load_execution_attempt(&investigation_id, &attempt_id)?;
        if attempt.dry_run {
            return Err(RivoraError::validation(
                "cannot link dry-run attempt to implementation",
            ));
        }
        if !matches!(
            attempt.status,
            ExecutionAttemptStatus::Completed | ExecutionAttemptStatus::PartiallyCompleted
        ) {
            return Err(RivoraError::precondition(
                "only completed or partially completed attempts can link to implementation",
            ));
        }
        let plan = self
            .store
            .load_execution_plan(&investigation_id, &attempt.plan_id)?;
        let attempt_lineage_id = attempt.lineage_id();
        let implementations = self.store.list_implementation_records(&investigation_id)?;
        if let Some(existing) = implementations.records.into_iter().find(|record| {
            record.references.iter().any(|reference| {
                matches!(
                    reference,
                    ImplementationReference::ExecutionAttempt { attempt_id }
                        if *attempt_id == attempt_lineage_id
                )
            })
        }) {
            return Err(RivoraError::precondition(format!(
                "execution attempt {} is already linked to implementation record {}",
                attempt_lineage_id, existing.id
            )));
        }
        let mut refs: Vec<ImplementationReference> = attempt
            .external_references
            .iter()
            .map(|r| {
                if r.contains("pull") || r.contains("/pulls/") {
                    ImplementationReference::PullRequest {
                        reference: r.clone(),
                    }
                } else if r.chars().all(|c| c.is_ascii_hexdigit()) && r.len() >= 7 {
                    ImplementationReference::CommitSha { sha: r.clone() }
                } else {
                    ImplementationReference::ExternalUri { uri: r.clone() }
                }
            })
            .collect();
        refs.push(ImplementationReference::ExecutionAttempt {
            attempt_id: attempt_lineage_id,
        });
        refs.push(ImplementationReference::HumanNote {
            note: format!(
                "linked from execution attempt {} plan {} capability {}",
                attempt.id, plan.id, plan.capability_id
            ),
        });

        self.record_external_implementation(
            investigation_id,
            plan.proposal_id,
            crate::runtime::outcome::RecordImplementationRequest {
                source: ImplementationSource::ExternalAgent,
                summary,
                references: refs,
                implemented_at: attempt.finished_at,
                observed_files: Vec::new(),
                observed_components: vec![plan.capability_id.clone()],
                declared_scope: format!(
                    "execution plan {} revision {}",
                    plan.lineage_id, plan.revision_number
                ),
            },
            actor,
        )
    }

    /// Create a rollback plan draft from an attempt's explicit receipt inverses.
    ///
    /// Rule: **no explicit, validated inverse → no executable rollback Plan**.
    /// Runtime never guesses an inverse from `supported_actions` order, naming,
    /// or capability registration order. Inverses originate only from immutable
    /// Execution Receipt metadata emitted by capabilities after a real mutation.
    ///
    /// The resulting Plan is always Draft. It inherits neither approval nor
    /// execution from the original Attempt.
    pub fn create_rollback_plan(
        &self,
        investigation_id: InvestigationId,
        attempt_id: ObjectId,
        actor: impl Into<String>,
    ) -> RivoraResult<ExecutionPlan> {
        let actor = require_actor(actor)?;
        let attempt = self
            .store
            .load_execution_attempt(&investigation_id, &attempt_id)?;
        let source_plan = self
            .store
            .load_execution_plan(&investigation_id, &attempt.plan_id)?;
        let receipts = self.store.list_execution_receipts(&investigation_id)?;
        let mut rollback_receipts: Vec<_> = receipts
            .receipts
            .into_iter()
            .filter(|receipt| {
                receipt.attempt_id == attempt.lineage_id()
                    && attempt.completed_actions.contains(&receipt.action_id)
                    && receipt.result_status == ExecutionReceiptResult::Success
            })
            .collect();
        // Reverse completion order so later mutations are undone first.
        rollback_receipts.reverse();
        if rollback_receipts.is_empty() {
            return Err(RivoraError::precondition(
                "rollback is not available for this attempt: no successful completed receipts",
            ));
        }

        let mut capability_id: Option<String> = None;
        let mut actions = Vec::with_capacity(rollback_receipts.len());
        let mut risks = Vec::new();
        let mut evidence = vec![source_plan.id, attempt.id];
        for receipt in &rollback_receipts {
            evidence.push(receipt.id);
            let rollback = &receipt.rollback_metadata;
            if !rollback.available {
                return Err(RivoraError::precondition(format!(
                    "rollback unavailable for action `{}`: capability did not declare a reversible mutation",
                    receipt.action_id
                )));
            }
            let receipt_capability = rollback
                .capability_id
                .as_ref()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    RivoraError::precondition(format!(
                        "rollback inverse incomplete for `{}`: missing capability_id",
                        receipt.action_id
                    ))
                })?;
            if capability_id
                .as_ref()
                .is_some_and(|existing| existing != &receipt_capability)
            {
                return Err(RivoraError::precondition(
                    "rollback actions require more than one capability; create separate plans",
                ));
            }
            capability_id = Some(receipt_capability.clone());

            // Explicit inverse only — never infer from descriptor ordering.
            let action_name = rollback
                .inverse_action_name
                .as_ref()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    RivoraError::precondition(format!(
                        "rollback inverse incomplete for `{}`: explicit inverse_action_name is required",
                        receipt.action_id
                    ))
                })?;
            let inputs = rollback.inputs.clone().ok_or_else(|| {
                RivoraError::precondition(format!(
                    "rollback inverse incomplete for `{}`: inverse inputs are required",
                    receipt.action_id
                ))
            })?;

            // Capability must be registered and declare the inverse action.
            let inverse_cap = self
                .execution_registry
                .get(&receipt_capability)
                .ok_or_else(|| {
                    RivoraError::precondition(format!(
                        "rollback capability unavailable: `{receipt_capability}` is not registered"
                    ))
                })?;
            let descriptor = inverse_cap.descriptor();
            if !descriptor
                .supported_actions
                .iter()
                .any(|name| name == &action_name)
            {
                return Err(RivoraError::precondition(format!(
                    "rollback action unsupported: `{action_name}` is not declared by capability `{receipt_capability}`"
                )));
            }

            actions.push(ExecutionAction {
                action_id: format!("rollback-{}", receipt.action_id),
                action_name,
                inputs,
                continue_on_failure: false,
            });
            risks.extend(
                rollback
                    .risks
                    .iter()
                    .map(|risk| crate::domain::ExecutionRisk {
                        description: risk.clone(),
                        severity: "medium".into(),
                        mitigation: "explicit approval required; no automatic rollback".into(),
                    }),
            );
        }
        let capability_id = capability_id.ok_or_else(|| {
            RivoraError::precondition("rollback capability metadata is unavailable")
        })?;
        let request = CreateExecutionPlanRequest {
            proposal_id: source_plan.proposal_id,
            capability_id,
            target_system: source_plan.target_system.clone(),
            target_environment: source_plan.target_environment.clone(),
            actions,
            inputs: serde_json::json!({
                "rollback_of_attempt": attempt.id.to_string(),
                "rollback_of_plan": source_plan.id.to_string(),
            }),
            expected_effects: vec![ExpectedEffect {
                description: "rollback prior mutation using capability-provided inverse".into(),
                resource_type: "rollback".into(),
                expected_fields: vec![],
            }],
            preconditions: vec![],
            supports_dry_run: true,
        };
        // Draft only — no approval, no attempt, no adapter invocation.
        let mut draft = self.create_execution_plan(investigation_id, request, &actor)?;
        draft = self.revise_execution_plan(
            investigation_id,
            draft.id,
            ReviseExecutionPlanRequest {
                expected_effects: Some(draft.expected_effects.clone()),
                preconditions: Some(draft.preconditions.clone()),
                ..Default::default()
            },
            &actor,
            "persist rollback draft for explicit re-approval",
        )?;
        let mut with_risks = draft.clone();
        with_risks.id = ObjectId::new();
        with_risks.parent_plan_id = Some(draft.id);
        with_risks.revision_number = draft.revision_number.saturating_add(1);
        with_risks.target_snapshot = with_risks
            .target_snapshot
            .as_ref()
            .map(|target| target.rebound_to(with_risks.id, with_risks.revision_number));
        with_risks.risks = risks;
        with_risks.updated_at = Utc::now();
        with_risks.provenance = Provenance::now(&actor, "runtime")
            .with_capability("create_rollback_plan")
            .with_evidence(evidence);
        with_risks
            .transitions
            .push(crate::domain::ExecutionPlanTransition {
                from: draft.status,
                to: ExecutionPlanStatus::Draft,
                actor: actor.clone(),
                reason: "attach rollback risks from explicit receipt inverses".into(),
                at: Utc::now(),
            });
        debug_assert_eq!(with_risks.status, ExecutionPlanStatus::Draft);
        self.store.append_execution_plan(&with_risks)?;
        Ok(with_risks)
    }

    /// Explain policy for a plan.
    pub fn explain_execution_policy(
        &self,
        investigation_id: InvestigationId,
        plan_id: ObjectId,
    ) -> RivoraResult<ExecutionPolicyDecision> {
        let plan = self
            .store
            .load_execution_plan(&investigation_id, &plan_id)?;
        Ok(self.evaluate_plan_policy(&plan))
    }

    /// Trace execution lineage.
    pub fn trace_execution(
        &self,
        investigation_id: InvestigationId,
        plan_id: ObjectId,
    ) -> RivoraResult<ExecutionTrace> {
        let requested_plan = self
            .store
            .load_execution_plan(&investigation_id, &plan_id)?;
        let plan = self.latest_plan_in_lineage(investigation_id, requested_plan.lineage_id)?;
        let approvals = self
            .store
            .list_execution_approvals(&investigation_id)?
            .approvals;
        let approval_ids = approvals
            .into_iter()
            .filter(|a| a.plan_lineage_id == plan.lineage_id)
            .map(|a| a.id)
            .collect();
        let attempts = self.store.list_execution_attempts(&investigation_id)?;
        let attempt_ids: Vec<_> = attempts
            .attempts
            .iter()
            .filter(|a| a.plan_lineage_id == plan.lineage_id)
            .map(|a| a.id)
            .collect();
        let receipts = self.store.list_execution_receipts(&investigation_id)?;
        let receipt_ids: Vec<_> = receipts
            .receipts
            .iter()
            .filter(|r| attempt_ids.contains(&r.attempt_id))
            .map(|r| r.id)
            .collect();
        let verifications = self
            .store
            .list_execution_verifications(&investigation_id)?
            .verifications;
        let verification_ids: Vec<_> = verifications
            .iter()
            .filter(|v| attempt_ids.contains(&v.attempt_id))
            .map(|v| v.id)
            .collect();
        let implementations: Vec<_> = self
            .store
            .list_implementation_records(&investigation_id)?
            .records
            .into_iter()
            .filter(|record| {
                record.references.iter().any(|reference| {
                    matches!(
                        reference,
                        ImplementationReference::ExecutionAttempt { attempt_id }
                            if attempt_ids.contains(attempt_id)
                    )
                })
            })
            .collect();
        let measured_outcome = self
            .store
            .list_measured_learning_outcomes(&investigation_id)?
            .outcomes
            .into_iter()
            .filter(|outcome| {
                implementations
                    .iter()
                    .any(|record| record.id == outcome.implementation_record_id)
            })
            .max_by_key(|outcome| (outcome.revision_number, outcome.updated_at));
        let implementation_record_id = if let Some(outcome) = measured_outcome.as_ref() {
            implementations
                .iter()
                .find(|record| record.id == outcome.implementation_record_id)
                .map(|record| record.id)
        } else {
            implementations
                .iter()
                .max_by_key(|record| (record.revision_number, record.updated_at))
                .map(|record| record.id)
        };
        let measured_outcome_id = measured_outcome.map(|outcome| outcome.id);

        Ok(ExecutionTrace {
            investigation_id,
            plan_lineage_id: plan.lineage_id,
            plan_id: plan.id,
            plan_revision_number: plan.revision_number,
            plan_status: plan.status,
            proposal_id: plan.proposal_id,
            proposal_revision_number: plan.proposal_revision_number,
            approval_ids,
            attempt_ids,
            receipt_ids,
            verification_ids,
            implementation_record_id,
            measured_outcome_id,
            explanation: "Proposal Accepted ≠ Execution Approved ≠ Execution Started ≠ Execution Completed ≠ Execution Verified ≠ Outcome Successful. Each step requires explicit authority.".into(),
        })
    }

    /// Export plan as JSON.
    pub fn export_execution_plan(
        &self,
        investigation_id: InvestigationId,
        plan_id: ObjectId,
    ) -> RivoraResult<String> {
        let plan = self
            .store
            .load_execution_plan(&investigation_id, &plan_id)?;
        serde_json::to_string_pretty(&plan).map_err(|e| RivoraError::serialization(e.to_string()))
    }

    /// Export receipt as JSON.
    pub fn export_execution_receipt(
        &self,
        investigation_id: InvestigationId,
        receipt_id: ObjectId,
    ) -> RivoraResult<String> {
        let receipt = self
            .store
            .load_execution_receipt(&investigation_id, &receipt_id)?;
        serde_json::to_string_pretty(&receipt)
            .map_err(|e| RivoraError::serialization(e.to_string()))
    }

    /// List plans for an investigation.
    pub fn list_execution_plans(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<ExecutionPlanListing> {
        self.store.list_execution_plans(&investigation_id)
    }

    /// Get one plan.
    pub fn get_execution_plan(
        &self,
        investigation_id: InvestigationId,
        plan_id: ObjectId,
    ) -> RivoraResult<ExecutionPlan> {
        self.store.load_execution_plan(&investigation_id, &plan_id)
    }

    /// List plan revisions.
    pub fn list_execution_plan_revisions(
        &self,
        investigation_id: InvestigationId,
        lineage_id: ObjectId,
    ) -> RivoraResult<ExecutionPlanListing> {
        self.store
            .list_execution_plan_revisions(&investigation_id, &lineage_id)
    }

    /// List attempts.
    pub fn list_execution_attempts(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<ExecutionAttemptListing> {
        self.store.list_execution_attempts(&investigation_id)
    }

    /// Get attempt.
    pub fn get_execution_attempt(
        &self,
        investigation_id: InvestigationId,
        attempt_id: ObjectId,
    ) -> RivoraResult<ExecutionAttempt> {
        self.store
            .load_execution_attempt(&investigation_id, &attempt_id)
    }

    /// List receipts.
    pub fn list_execution_receipts(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<ExecutionReceiptListing> {
        self.store.list_execution_receipts(&investigation_id)
    }

    /// Refuse unsafe retries without new plan/approval.
    pub fn classify_retry_safety(
        &self,
        investigation_id: InvestigationId,
        attempt_id: ObjectId,
    ) -> RivoraResult<RetrySafety> {
        let attempt = self
            .store
            .load_execution_attempt(&investigation_id, &attempt_id)?;
        Ok(attempt.retry_safety)
    }

    // -- helpers --

    fn evaluate_plan_policy(&self, plan: &ExecutionPlan) -> ExecutionPolicyDecision {
        let desc = self
            .execution_registry
            .get(&plan.capability_id)
            .map(|c| c.descriptor());
        evaluate_execution_policy(
            desc.as_ref(),
            &plan.capability_id,
            &plan.target_environment,
            plan.actions.len(),
            plan.supports_dry_run,
        )
    }

    fn resolve_plan_target(
        &self,
        plan: &ExecutionPlan,
        capability: &dyn ExecutionCapability,
    ) -> RivoraResult<CapabilityTarget> {
        let mut resolved: Option<CapabilityTarget> = None;
        for action in &plan.actions {
            let inputs = merge_inputs(&plan.inputs, &action.inputs);
            let target = capability.target(&plan.target_environment, &inputs)?;
            if target.provider.trim().is_empty() {
                return Err(RivoraError::validation(
                    "capability target provider must not be empty",
                ));
            }
            if target.provider != plan.target_system {
                return Err(RivoraError::validation(format!(
                    "plan target_system `{}` does not match capability provider `{}`",
                    plan.target_system, target.provider
                )));
            }
            if let Some(existing) = &resolved {
                if existing != &target {
                    return Err(RivoraError::validation(
                        "all actions in one Plan must resolve to the same immutable target",
                    ));
                }
            } else {
                resolved = Some(target);
            }
        }
        resolved.ok_or_else(|| RivoraError::validation("execution plan requires an action"))
    }

    fn validate_plan_contract(
        &self,
        plan: &ExecutionPlan,
        capability: &dyn ExecutionCapability,
    ) -> RivoraResult<CapabilityTarget> {
        let descriptor = capability.descriptor();
        if descriptor.capability_id != plan.capability_id {
            return Err(RivoraError::validation(
                "registered capability descriptor does not match Plan capability",
            ));
        }
        let mut action_ids = std::collections::HashSet::new();
        let mut action_signatures = std::collections::HashSet::new();
        for action in &plan.actions {
            if !action_ids.insert(action.action_id.as_str()) {
                return Err(RivoraError::validation(format!(
                    "duplicate action id `{}`",
                    action.action_id
                )));
            }
            if !descriptor.supported_actions.contains(&action.action_name) {
                return Err(RivoraError::validation(format!(
                    "unsupported action `{}` for capability `{}`",
                    action.action_name, plan.capability_id
                )));
            }
            let inputs = merge_inputs(&plan.inputs, &action.inputs);
            let signature = serde_json::to_string(&(action.action_name.as_str(), &inputs))
                .map_err(|error| RivoraError::serialization(error.to_string()))?;
            if !action_signatures.insert(signature) {
                return Err(RivoraError::validation(format!(
                    "duplicate action `{}` with identical inputs",
                    action.action_name
                )));
            }
            let object = inputs.as_object().ok_or_else(|| {
                RivoraError::validation(format!(
                    "inputs for action `{}` must be an object",
                    action.action_id
                ))
            })?;
            for required in &descriptor.required_inputs {
                let present = object.get(required).is_some_and(|value| match value {
                    serde_json::Value::Null => false,
                    serde_json::Value::String(value) => !value.trim().is_empty(),
                    _ => true,
                });
                if !present {
                    return Err(RivoraError::validation(format!(
                        "action `{}` is missing required input `{required}`",
                        action.action_id
                    )));
                }
            }
        }
        let target = self.resolve_plan_target(plan, capability)?;
        let expected = TargetSnapshot::bind(plan, target.clone());
        if let Some(bound) = &plan.target_snapshot {
            if bound != &expected {
                return Err(RivoraError::precondition(
                    "runtime target does not match the Plan target snapshot",
                ));
            }
        }
        Ok(target)
    }

    fn transition_plan(
        &self,
        investigation_id: InvestigationId,
        plan_id: ObjectId,
        to: ExecutionPlanStatus,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<ExecutionPlan> {
        let actor = require_actor(actor)?;
        let reason = require_reason(reason)?;
        self.ensure_plan_head(investigation_id, plan_id)?;
        let plan = self
            .store
            .load_execution_plan(&investigation_id, &plan_id)?;
        let next = plan.transitioned(to, actor, reason, Utc::now())?;
        self.store.append_execution_plan(&next)?;
        Ok(next)
    }

    fn ensure_plan_head(
        &self,
        investigation_id: InvestigationId,
        plan_id: ObjectId,
    ) -> RivoraResult<()> {
        let plan = self
            .store
            .load_execution_plan(&investigation_id, &plan_id)?;
        let listing = self
            .store
            .list_execution_plan_revisions(&investigation_id, &plan.lineage_id)?;
        let head = listing
            .plans
            .last()
            .ok_or_else(|| RivoraError::validation("execution plan lineage has no revisions"))?;
        if head.id != plan_id {
            return Err(RivoraError::validation(format!(
                "plan {} is not the head of lineage {} (head is {})",
                plan_id, plan.lineage_id, head.id
            )));
        }
        Ok(())
    }

    fn latest_plan_in_lineage(
        &self,
        investigation_id: InvestigationId,
        lineage_id: ObjectId,
    ) -> RivoraResult<ExecutionPlan> {
        let listing = self
            .store
            .list_execution_plan_revisions(&investigation_id, &lineage_id)?;
        listing
            .plans
            .into_iter()
            .next_back()
            .ok_or_else(|| RivoraError::validation("empty plan lineage"))
    }

    fn find_attempt_by_idempotency(
        &self,
        investigation_id: InvestigationId,
        key: &str,
    ) -> RivoraResult<Option<ExecutionAttempt>> {
        let listing = self.store.list_execution_attempts(&investigation_id)?;
        Ok(listing
            .attempts
            .into_iter()
            .filter(|a| {
                a.idempotency_key == key && a.status != ExecutionAttemptStatus::DuplicateSuppressed
            })
            .max_by_key(|a| a.revision_number))
    }
}

fn require_actor(actor: impl Into<String>) -> RivoraResult<String> {
    let actor = actor.into().trim().to_string();
    if actor.is_empty() {
        return Err(RivoraError::validation("actor must not be empty"));
    }
    Ok(actor)
}

fn require_reason(reason: impl Into<String>) -> RivoraResult<String> {
    let reason = reason.into().trim().to_string();
    if reason.is_empty() {
        return Err(RivoraError::validation("reason must not be empty"));
    }
    Ok(reason)
}

fn merge_inputs(
    plan_inputs: &serde_json::Value,
    action_inputs: &serde_json::Value,
) -> serde_json::Value {
    match (plan_inputs, action_inputs) {
        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
            let mut merged = a.clone();
            for (k, v) in b {
                merged.insert(k.clone(), v.clone());
            }
            serde_json::Value::Object(merged)
        }
        (_, action) if !action.is_null() => action.clone(),
        (plan, _) => plan.clone(),
    }
}

// Silence unused import if RollbackMetadata only used via receipt.
#[allow(dead_code)]
fn _rollback_type_use(_: &RollbackMetadata) {}
