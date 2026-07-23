//! Controlled external execution orchestration (RFC-025, RFC-026, RFC-027).
//!
//! Runtime owns plan lifecycle, approval, policy, capability invocation,
//! receipts, verification, and traceability. CLI/Workspace never call
//! external mutation APIs directly.

use chrono::{DateTime, Utc};

use crate::domain::{
    evaluate_execution_policy, verification_confidence, CapabilityInvocation, CapabilityStateQuery,
    DryRunResult, ExecutionAction, ExecutionApproval, ExecutionAttempt, ExecutionAttemptListing,
    ExecutionAttemptStatus, ExecutionCapabilityDescriptor, ExecutionCapabilityRegistry,
    ExecutionCheckResult, ExecutionPlan, ExecutionPlanListing, ExecutionPlanStatus,
    ExecutionPolicyDecision, ExecutionPolicyDecisionKind, ExecutionPrecondition, ExecutionReceipt,
    ExecutionReceiptListing, ExecutionReceiptResult, ExecutionTrace, ExecutionVerification,
    ExecutionVerificationStatus, ExpectedEffect, ImplementationRecord, ImplementationReference,
    ImplementationSource, InvestigationId, ObjectId, ProposalStatus, Provenance, RetrySafety,
    RollbackMetadata, SanitizationMetadata,
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
    ) {
        self.execution_registry.register(capability);
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
        next.last_policy_decision = Some(self.evaluate_plan_policy(&next));
        self.store.append_execution_plan(&next)?;

        // Invalidate approvals that pointed at the prior snapshot.
        for approval in self.store.list_execution_approvals(&investigation_id)? {
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

        let policy = self.evaluate_plan_policy(&plan);
        if policy.decision == ExecutionPolicyDecisionKind::Denied {
            return Err(RivoraError::precondition(format!(
                "policy denied plan: {}",
                policy.reasons.join("; ")
            )));
        }

        // Capability must be registered for validation to ReadyForReview for live paths.
        if self.execution_registry.get(&plan.capability_id).is_none() {
            return Err(RivoraError::precondition(format!(
                "capability `{}` is not registered",
                plan.capability_id
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
            if k.is_empty() {
                return Err(RivoraError::validation("idempotency_key must not be empty"));
            }
            k
        };

        // Namespace dry-run keys so dry-run never suppresses live execution.
        let effective_key = if dry_run {
            format!("dry-run:{idempotency_key}")
        } else {
            idempotency_key.clone()
        };

        // Idempotent replay must run before head/approval checks so retries of a
        // completed attempt do not require the plan to remain Approved.
        if let Some(existing) =
            self.find_attempt_by_idempotency(investigation_id, &effective_key)?
        {
            if existing.dry_run != dry_run {
                return Err(RivoraError::precondition(format!(
                    "idempotency key collides with attempt {} under a different dry_run mode",
                    existing.id
                )));
            }
            if let Ok(p) = self.store.load_execution_plan(&investigation_id, &plan_id) {
                if existing.plan_id == plan_id || existing.plan_lineage_id == p.lineage_id {
                    return Ok(existing);
                }
            } else if existing.plan_id == plan_id {
                return Ok(existing);
            }
            return Err(RivoraError::precondition(format!(
                "idempotency key already used for attempt {} on a different plan",
                existing.id
            )));
        }

        self.ensure_plan_head(investigation_id, plan_id)?;
        let plan = self
            .store
            .load_execution_plan(&investigation_id, &plan_id)?;
        let approval = self
            .store
            .load_execution_approval(&investigation_id, &approval_id)?;

        if dry_run {
            // Dry-run attempt does not require approval consumption; still validates policy.
            let policy = self.evaluate_plan_policy(&plan);
            if !policy.dry_run_permitted {
                return Err(RivoraError::precondition(format!(
                    "dry-run not permitted: {}",
                    policy.reasons.join("; ")
                )));
            }
            let preview = self.preview_execution_plan(investigation_id, plan_id)?;
            let mut attempt = ExecutionAttempt::start(
                &plan,
                &approval,
                &actor,
                &effective_key,
                true,
                Provenance::now(&actor, "runtime").with_capability("execute_plan"),
            )?;
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

        // Live execution requires Approved plan + valid approval.
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

        // Evaluate plan-level preconditions marked unsatisfied.
        let failed_pre: Vec<_> = plan
            .preconditions
            .iter()
            .filter(|p| p.satisfied == Some(false))
            .cloned()
            .collect();
        if !failed_pre.is_empty() {
            let mut attempt = ExecutionAttempt::start(
                &plan,
                &approval,
                &actor,
                &effective_key,
                false,
                Provenance::now(&actor, "runtime").with_capability("execute_plan"),
            )?;
            attempt.status = ExecutionAttemptStatus::Blocked;
            attempt.errors = failed_pre
                .iter()
                .map(|p| {
                    format!(
                        "precondition {}: {}",
                        p.id,
                        p.detail.clone().unwrap_or_else(|| p.description.clone())
                    )
                })
                .collect();
            attempt.finished_at = Some(Utc::now());
            attempt.retry_safety = RetrySafety::ConditionallySafe;
            attempt.recommended_next_action =
                Some("resolve preconditions and re-validate plan".into());
            self.store.append_execution_attempt(&attempt)?;
            return Ok(attempt);
        }

        // Transition plan to Executing.
        let mut executing = plan.transitioned(
            ExecutionPlanStatus::Executing,
            &actor,
            "begin external invocation",
            Utc::now(),
        )?;
        executing.last_policy_decision = Some(policy);
        self.store.append_execution_plan(&executing)?;

        // Note: approval bound to Approved snapshot; after transition approval is stale for
        // the new Executing snapshot by design. Validity for execute is checked against the
        // Approved plan before transition. We keep approval.plan_id == plan.id (Approved).

        let mut attempt = ExecutionAttempt::start(
            &plan,
            &approval,
            &actor,
            &effective_key,
            false,
            Provenance::now(&actor, "runtime").with_capability("execute_plan"),
        )?;

        let mut any_success = false;
        let mut any_failure = false;
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
                idempotency_key: format!("{}:{}", effective_key, action.action_id),
                investigation_id: investigation_id.to_string(),
                plan_id: plan.id.to_string(),
            };

            match cap.execute(&invocation) {
                Ok(result) => {
                    if result.duplicate_suppressed {
                        attempt.status = ExecutionAttemptStatus::DuplicateSuppressed;
                    }
                    let receipt_status = match result.result_status.as_str() {
                        "success" => ExecutionReceiptResult::Success,
                        "partial" => ExecutionReceiptResult::Partial,
                        "uncertain" => ExecutionReceiptResult::Uncertain,
                        _ => ExecutionReceiptResult::Failed,
                    };
                    let receipt = ExecutionReceipt {
                        id: ObjectId::new(),
                        attempt_id: attempt.id,
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
                    if result.success {
                        any_success = true;
                        attempt.completed_actions.push(action.action_id.clone());
                        if result.rollback.available {
                            attempt.rollback = result.rollback;
                        }
                    } else {
                        any_failure = true;
                        attempt.failed_actions.push(action.action_id.clone());
                        if let Some(err) = result.error {
                            attempt.errors.push(err);
                        }
                        if !action.continue_on_failure {
                            stop = true;
                        }
                    }
                    self.store.append_execution_receipt(&receipt)?;
                }
                Err(err) => {
                    any_failure = true;
                    attempt.failed_actions.push(action.action_id.clone());
                    attempt.errors.push(err.to_string());
                    if !action.continue_on_failure {
                        stop = true;
                    }
                }
            }
        }

        attempt.finished_at = Some(Utc::now());
        attempt.status = match (any_success, any_failure) {
            (true, false) => ExecutionAttemptStatus::Completed,
            (true, true) => ExecutionAttemptStatus::PartiallyCompleted,
            (false, true) => ExecutionAttemptStatus::Failed,
            (false, false) => ExecutionAttemptStatus::Blocked,
        };
        attempt.retry_safety = match attempt.status {
            ExecutionAttemptStatus::Completed | ExecutionAttemptStatus::DuplicateSuppressed => {
                RetrySafety::Safe
            }
            ExecutionAttemptStatus::PartiallyCompleted => RetrySafety::Unsafe,
            ExecutionAttemptStatus::Failed | ExecutionAttemptStatus::Blocked => {
                RetrySafety::Unknown
            }
            ExecutionAttemptStatus::Started => RetrySafety::Unknown,
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

        // Consume one-time approval after live attempt starts completing.
        if approval.one_time {
            let consumed = approval.mark_consumed();
            self.store.save_execution_approval(&consumed)?;
        }

        // Update plan status from Executing.
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
            .filter(|r| r.attempt_id == attempt.id)
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

            // API success but empty observation is a contradiction.
            if receipt.result_status == ExecutionReceiptResult::Success && !observation.observed {
                contradictions.push(format!(
                    "receipt {} reported success but independent observation failed",
                    receipt.id
                ));
            }

            // Expected fields from plan effects.
            for effect in &plan.expected_effects {
                for (field, expected) in &effect.expected_fields {
                    let actual = observation.fields.get(field).cloned().unwrap_or_default();
                    let passed = actual == *expected;
                    if !passed && receipt.result_status == ExecutionReceiptResult::Success {
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

            // Default check: success receipts must produce at least one external id or changed resource.
            let basic_ok = receipt.result_status != ExecutionReceiptResult::Success
                || !receipt.external_identifiers.is_empty()
                || !receipt.changed_resources.is_empty()
                || observation.observed;
            results.push(ExecutionCheckResult {
                check: format!("receipt:{}", receipt.action_id),
                passed: basic_ok && contradictions.is_empty(),
                detail: receipt.response_summary.clone(),
                evidence: receipt.raw_evidence_refs.clone(),
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
        let status = if !contradictions.is_empty() {
            ExecutionVerificationStatus::Failed
        } else if passed == total {
            ExecutionVerificationStatus::Passed
        } else if passed == 0 {
            ExecutionVerificationStatus::Failed
        } else {
            ExecutionVerificationStatus::Inconclusive
        };

        let verification = ExecutionVerification {
            id: ObjectId::new(),
            attempt_id: attempt.id,
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
            revision: 1,
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

    /// Create a rollback plan draft from an attempt's rollback metadata.
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
        if !attempt.rollback.available {
            return Err(RivoraError::precondition(
                "rollback is not available for this attempt",
            ));
        }
        let plan = self
            .store
            .load_execution_plan(&investigation_id, &attempt.plan_id)?;
        let capability_id = attempt
            .rollback
            .capability_id
            .clone()
            .unwrap_or_else(|| plan.capability_id.clone());
        let mut inputs = attempt
            .rollback
            .inputs
            .clone()
            .unwrap_or_else(|| serde_json::json!({}));
        // Prefer an explicit inverse action from rollback metadata; otherwise pick a
        // capability-supported action (never hardcode mock-only names for GitHub).
        let action_name = inputs
            .get("inverse")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                self.execution_registry
                    .get(&capability_id)
                    .and_then(|c| c.descriptor().supported_actions.first().cloned())
            })
            .ok_or_else(|| {
                RivoraError::precondition(format!(
                    "cannot derive rollback action for capability `{capability_id}`"
                ))
            })?;
        // Drop the inverse helper field from action inputs if present.
        if let Some(obj) = inputs.as_object_mut() {
            obj.remove("inverse");
        }
        let risks: Vec<crate::domain::ExecutionRisk> = attempt
            .rollback
            .risks
            .iter()
            .map(|r| crate::domain::ExecutionRisk {
                description: r.clone(),
                severity: "medium".into(),
                mitigation: "explicit approval required; no automatic rollback".into(),
            })
            .collect();
        let request = CreateExecutionPlanRequest {
            proposal_id: plan.proposal_id,
            capability_id,
            target_system: plan.target_system.clone(),
            target_environment: plan.target_environment.clone(),
            actions: vec![ExecutionAction {
                action_id: "rollback".into(),
                action_name,
                inputs: inputs.clone(),
                continue_on_failure: false,
            }],
            inputs,
            expected_effects: vec![ExpectedEffect {
                description: "rollback prior mutation".into(),
                resource_type: "rollback".into(),
                expected_fields: vec![],
            }],
            preconditions: vec![],
            supports_dry_run: true,
        };
        let mut plan = self.create_execution_plan(investigation_id, request, &actor)?;
        // Persist risks by revising content on the new draft head.
        plan = self.revise_execution_plan(
            investigation_id,
            plan.id,
            ReviseExecutionPlanRequest {
                expected_effects: Some(plan.expected_effects.clone()),
                preconditions: Some(plan.preconditions.clone()),
                ..Default::default()
            },
            &actor,
            "persist rollback draft for explicit re-approval",
        )?;
        // Attach risks on the in-memory successor by a second domain-level rewrite
        // through revise is insufficient; store a final draft with risks via transition-free append.
        // Load head and re-save risks by creating one more revised snapshot with risks set.
        let mut with_risks = plan.clone();
        with_risks.id = ObjectId::new();
        with_risks.parent_plan_id = Some(plan.id);
        with_risks.revision_number = plan.revision_number.saturating_add(1);
        with_risks.risks = risks;
        with_risks.updated_at = Utc::now();
        with_risks.provenance = Provenance::now(&actor, "runtime")
            .with_capability("create_rollback_plan")
            .with_evidence(vec![plan.id, attempt.id]);
        with_risks
            .transitions
            .push(crate::domain::ExecutionPlanTransition {
                from: plan.status,
                to: ExecutionPlanStatus::Draft,
                actor: actor.clone(),
                reason: "attach rollback risks".into(),
                at: Utc::now(),
            });
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
        let plan = self
            .store
            .load_execution_plan(&investigation_id, &plan_id)?;
        let approvals = self.store.list_execution_approvals(&investigation_id)?;
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
        let verifications = self.store.list_execution_verifications(&investigation_id)?;
        let verification_ids: Vec<_> = verifications
            .iter()
            .filter(|v| attempt_ids.contains(&v.attempt_id))
            .map(|v| v.id)
            .collect();

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
            implementation_record_id: None,
            measured_outcome_id: None,
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
            .find(|a| a.idempotency_key == key))
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
