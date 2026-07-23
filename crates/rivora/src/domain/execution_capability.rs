//! Typed external execution capability contract (RFC-026).
//!
//! Observation connectors remain read-only. Mutation adapters implement
//! [`ExecutionCapability`] and are invoked only by the Runtime.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use super::engineering_loop::{
    default_accepted_input_types, CapabilityLifecycleContributions, ContributionIdentity,
    EngineeringLoopParticipation, EvaluationContributionRequest, ImprovementContributionContext,
    LearningContributionContext, LifecycleParticipation, MemoryContribution, StageContribution,
    VerificationContributionRequest,
};
use super::execution::{
    CapabilityExecutionStatus, CapabilityRiskLevel, CapabilityTarget, CapabilityVerificationStatus,
    DryRunResult, ExecutionCapabilityDescriptor, ExecutionPolicyDecision,
    ExecutionPolicyDecisionKind, RollbackMetadata,
};
use super::{Confidence, InvestigationId, ObjectId};
use crate::error::{RivoraError, RivoraResult};

/// Context for building lifecycle contributions after Capability execution.
#[derive(Debug, Clone)]
pub struct LifecycleContributionContext {
    /// Investigation.
    pub investigation_id: InvestigationId,
    /// Invocation id (typically attempt id).
    pub invocation_id: String,
    /// Actor.
    pub actor: String,
    /// Idempotency key.
    pub idempotency_key: String,
    /// Plan id.
    pub plan_id: Option<ObjectId>,
    /// Attempt id.
    pub attempt_id: Option<ObjectId>,
    /// Receipt ids.
    pub receipt_ids: Vec<ObjectId>,
    /// Proposal id.
    pub proposal_id: Option<ObjectId>,
    /// Observation ids.
    pub observation_ids: Vec<ObjectId>,
    /// Environment.
    pub environment: Option<String>,
    /// Execution verification id when independent verification already ran.
    pub execution_verification_id: Option<ObjectId>,
    /// Measured outcome id when present.
    pub measured_outcome_id: Option<ObjectId>,
    /// Implementation record id when present.
    pub implementation_record_id: Option<ObjectId>,
    /// Action name that was executed.
    pub action_name: Option<String>,
    /// External identifiers from receipts.
    pub external_identifiers: Vec<String>,
    /// Sanitized request/response summary for Memory.
    pub result_summary: String,
    /// Whether API reported success (never treated as verified success).
    pub api_reported_success: bool,
}

/// Request to dry-run or execute a single action through a capability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityInvocation {
    /// Capability id (must match adapter).
    pub capability_id: String,
    /// Action name.
    pub action_name: String,
    /// Action id from the plan.
    pub action_id: String,
    /// Structured inputs (never secrets).
    pub inputs: serde_json::Value,
    /// Target environment.
    pub environment: String,
    /// Idempotency key for this attempt/action.
    pub idempotency_key: String,
    /// Investigation id as string for correlation.
    pub investigation_id: String,
    /// Plan id as string.
    pub plan_id: String,
}

/// Result of a live capability execution (still not verification).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityExecutionResult {
    /// Single typed result status.
    pub status: CapabilityExecutionStatus,
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
    /// Warnings.
    pub warnings: Vec<String>,
    /// Rollback metadata.
    pub rollback: RollbackMetadata,
    /// Verification requirements.
    pub verification_requirements: Vec<String>,
    /// Evidence refs (sanitized).
    pub evidence_refs: Vec<String>,
    /// Error message if failed.
    pub error: Option<String>,
    /// Whether this was a duplicate suppressed by idempotency.
    pub duplicate_suppressed: bool,
}

/// Independent state observation for verification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityStateObservation {
    /// Resource key observed.
    pub resource_key: String,
    /// Observed fields.
    pub fields: HashMap<String, String>,
    /// Summary.
    pub summary: String,
    /// Whether observation succeeded.
    pub observed: bool,
    /// Capability-specific conclusion about the exact requested postcondition.
    pub verification_status: CapabilityVerificationStatus,
    /// Error if observation failed.
    pub error: Option<String>,
}

/// Request for independent state observation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityStateQuery {
    /// Capability id.
    pub capability_id: String,
    /// Action name that was executed.
    pub action_name: String,
    /// Original inputs.
    pub inputs: serde_json::Value,
    /// External identifiers from receipt.
    pub external_identifiers: Vec<String>,
    /// Environment.
    pub environment: String,
}

/// Typed external mutation adapter.
///
/// Implementations must never own execution policy, approval, or lifecycle decisions.
/// They may provide typed Engineering Loop contributions; the Runtime owns reasoning.
pub trait ExecutionCapability: Send + Sync {
    /// Capability descriptor.
    fn descriptor(&self) -> ExecutionCapabilityDescriptor;

    /// Resolve the exact immutable external target for these inputs.
    fn target(
        &self,
        _environment: &str,
        inputs: &serde_json::Value,
    ) -> RivoraResult<CapabilityTarget> {
        let descriptor = self.descriptor();
        Ok(CapabilityTarget {
            provider: descriptor
                .capability_id
                .split('.')
                .next()
                .unwrap_or("unknown")
                .to_string(),
            owner: None,
            repository: descriptor.target_restrictions.first().cloned(),
            branch_or_ref: inputs
                .get("ref")
                .or_else(|| inputs.get("head"))
                .and_then(|value| value.as_str())
                .map(str::to_string),
        })
    }

    /// Check credentials and adapter-specific preconditions without mutation.
    fn validate_preconditions(&self, _request: &CapabilityInvocation) -> RivoraResult<()> {
        Ok(())
    }

    /// Dry-run or plan validation. Must never mutate.
    fn dry_run(&self, request: &CapabilityInvocation) -> RivoraResult<DryRunResult>;

    /// Execute a single action. Runtime calls this only after approval and policy checks.
    fn execute(&self, request: &CapabilityInvocation) -> RivoraResult<CapabilityExecutionResult>;

    /// Independently observe external state for verification.
    fn observe_state(
        &self,
        query: &CapabilityStateQuery,
    ) -> RivoraResult<CapabilityStateObservation>;

    /// Typed Engineering Loop contributions for a completed (or partial) execution.
    ///
    /// Default: build contributions from descriptor participation without inventing
    /// Supported payloads for stages the Capability did not customize.
    fn lifecycle_contributions(
        &self,
        _result: &CapabilityExecutionResult,
        context: &LifecycleContributionContext,
    ) -> RivoraResult<CapabilityLifecycleContributions> {
        let descriptor = self.descriptor();
        let identity = ContributionIdentity {
            capability_id: descriptor.capability_id.clone(),
            invocation_id: context.invocation_id.clone(),
            investigation_id: context.investigation_id,
            observation_ids: context.observation_ids.clone(),
            engineering_object_ids: context
                .receipt_ids
                .iter()
                .copied()
                .chain(context.attempt_id)
                .chain(context.plan_id)
                .collect(),
            plan_id: context.plan_id,
            attempt_id: context.attempt_id,
            receipt_ids: context.receipt_ids.clone(),
            proposal_id: context.proposal_id,
            correlation_id: context.attempt_id.map(|id| id.to_string()),
            causation_id: context.plan_id.map(|id| id.to_string()),
            actor: context.actor.clone(),
            environment: context.environment.clone(),
            idempotency_key: context.idempotency_key.clone(),
            timestamp: chrono::Utc::now(),
            schema_version: super::engineering_loop::ENGINEERING_LOOP_SCHEMA_VERSION,
            evidence_refs: context.receipt_ids.clone(),
            metadata: serde_json::Map::new(),
        };
        Ok(default_lifecycle_contributions(
            identity,
            &descriptor.engineering_loop,
            context,
        ))
    }
}

/// Build standard lifecycle contributions for an execution capability.
pub fn default_lifecycle_contributions(
    identity: ContributionIdentity,
    participation: &EngineeringLoopParticipation,
    context: &LifecycleContributionContext,
) -> CapabilityLifecycleContributions {
    let evidence = context.receipt_ids.clone();
    let memory = match participation.memory {
        LifecycleParticipation::Supported => StageContribution::Supported {
            value: MemoryContribution {
                summary: format!(
                    "Capability `{}` invocation {}: {}",
                    identity.capability_id, identity.invocation_id, context.result_summary
                ),
                observation_id: context.observation_ids.first().copied(),
                confidence: if context.api_reported_success {
                    0.7
                } else {
                    0.5
                },
                evidence_ids: evidence.clone(),
            },
        },
        other => participation_to_stage(other, "memory"),
    };
    let evaluation = match participation.evaluation {
        LifecycleParticipation::Supported => StageContribution::Supported {
            value: EvaluationContributionRequest {
                subject: format!(
                    "Capability `{}` execution outcome",
                    identity.capability_id
                ),
                expectation: "External action completed as planned; independent verification still required".into(),
                rationale: "Evaluate the engineering significance of the bounded external action using evidence, not API success alone".into(),
                evidence_ids: evidence.clone(),
                suggested_severity: Some(if context.api_reported_success {
                    "info".into()
                } else {
                    "medium".into()
                }),
            },
        },
        other => participation_to_stage(other, "evaluation"),
    };
    let verification = match participation.verification {
        LifecycleParticipation::Supported => StageContribution::Supported {
            value: VerificationContributionRequest {
                strategy: "Independent state observation; do not trust capability execution result as proof".into(),
                required_evidence: vec![
                    "Independent external state observation".into(),
                    "Correlation of external identifiers".into(),
                ],
                execution_verification_id: context.execution_verification_id,
                requires_independent_observation: context.execution_verification_id.is_none(),
                evidence_ids: evidence.clone(),
            },
        },
        other => participation_to_stage(other, "verification"),
    };
    let improvement = match participation.improvement {
        LifecycleParticipation::Supported => StageContribution::Supported {
            value: ImprovementContributionContext {
                summary: format!(
                    "Improvement context after `{}` invocation",
                    identity.capability_id
                ),
                focus_areas: vec!["follow-up hardening".into()],
                generate_proposal: false,
                evidence_ids: evidence.clone(),
            },
        },
        LifecycleParticipation::Deferred => StageContribution::Deferred {
            reason: "Improvement deferred until verification and optional measured outcome context"
                .into(),
        },
        other => participation_to_stage(other, "improvement"),
    };
    let learning = match participation.learning {
        LifecycleParticipation::Supported if context.measured_evidence_available() => {
            StageContribution::Supported {
                value: LearningContributionContext {
                    summary: format!(
                        "Learning context for `{}` after measured evidence",
                        identity.capability_id
                    ),
                    measured_outcome_id: context.measured_outcome_id,
                    implementation_record_id: context.implementation_record_id,
                    measured_evidence_available: true,
                    evidence_ids: evidence,
                },
            }
        }
        LifecycleParticipation::Supported => StageContribution::Deferred {
            reason: "Learning declared supported but measured evidence is not yet available".into(),
        },
        LifecycleParticipation::Deferred => StageContribution::Deferred {
            reason:
                "Learning deferred — awaiting measured outcome; API success is not Outcome success"
                    .into(),
        },
        other => participation_to_stage(other, "learning"),
    };
    CapabilityLifecycleContributions {
        identity,
        memory,
        evaluation,
        verification,
        improvement,
        learning,
    }
}

fn participation_to_stage<T>(p: LifecycleParticipation, stage: &str) -> StageContribution<T> {
    match p {
        LifecycleParticipation::Supported => StageContribution::Deferred {
            reason: format!("{stage} declared supported but no contribution produced"),
        },
        LifecycleParticipation::NotApplicable => StageContribution::NotApplicable {
            reason: format!("{stage} does not apply to this capability"),
        },
        LifecycleParticipation::Unsupported => StageContribution::Unsupported {
            reason: format!("{stage} is not implemented for this capability"),
        },
        LifecycleParticipation::Deferred => StageContribution::Deferred {
            reason: format!("{stage} is intentionally deferred"),
        },
    }
}

impl LifecycleContributionContext {
    fn measured_evidence_available(&self) -> bool {
        self.measured_outcome_id.is_some()
    }
}

/// Registry of execution capabilities available to the Runtime.
#[derive(Clone, Default)]
pub struct ExecutionCapabilityRegistry {
    inner: Arc<Mutex<HashMap<String, Arc<dyn ExecutionCapability>>>>,
}

impl ExecutionCapabilityRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a capability adapter. Duplicate ids are rejected.
    pub fn register(&self, capability: Arc<dyn ExecutionCapability>) -> RivoraResult<()> {
        let desc = capability.descriptor();
        let mut guard = self.inner.lock().expect("execution registry lock");
        if guard.contains_key(&desc.capability_id) {
            return Err(RivoraError::validation(format!(
                "execution capability `{}` is already registered",
                desc.capability_id
            )));
        }
        guard.insert(desc.capability_id, capability);
        Ok(())
    }

    /// Get a capability by id.
    pub fn get(&self, capability_id: &str) -> Option<Arc<dyn ExecutionCapability>> {
        let guard = self.inner.lock().expect("execution registry lock");
        guard.get(capability_id).cloned()
    }

    /// List descriptors sorted by capability id.
    pub fn list(&self) -> Vec<ExecutionCapabilityDescriptor> {
        let guard = self.inner.lock().expect("execution registry lock");
        let mut out: Vec<_> = guard.values().map(|c| c.descriptor()).collect();
        out.sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        out
    }

    /// Whether the registry contains the capability.
    pub fn contains(&self, capability_id: &str) -> bool {
        let guard = self.inner.lock().expect("execution registry lock");
        guard.contains_key(capability_id)
    }
}

impl std::fmt::Debug for ExecutionCapabilityRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ids: Vec<String> = self.list().into_iter().map(|d| d.capability_id).collect();
        f.debug_struct("ExecutionCapabilityRegistry")
            .field("capabilities", &ids)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Mock capability for tests and local validation
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct MockState {
    /// resource_key → fields
    resources: HashMap<String, HashMap<String, String>>,
    /// idempotency_key → previous result
    idempotency: HashMap<String, CapabilityExecutionResult>,
    /// When true, next execute fails after recording nothing (or partial).
    fail_next: bool,
    /// When true, execute reports success but observe_state returns mismatch.
    lie_success: bool,
    /// When true, second action fails for partial failure tests.
    fail_action_names: Vec<String>,
}

/// In-process mock execution capability (`mock.record`).
///
/// Used by tests. Never talks to real external systems.
#[derive(Clone, Default)]
pub struct MockExecutionCapability {
    state: Arc<Mutex<MockState>>,
}

impl MockExecutionCapability {
    /// Create a new mock capability.
    pub fn new() -> Self {
        Self::default()
    }

    /// Configure the next execute call to fail.
    pub fn set_fail_next(&self, fail: bool) {
        self.state.lock().expect("mock lock").fail_next = fail;
    }

    /// Configure success response that fails independent verification.
    pub fn set_lie_success(&self, lie: bool) {
        self.state.lock().expect("mock lock").lie_success = lie;
    }

    /// Configure specific action names to fail.
    pub fn set_fail_action_names(&self, names: Vec<String>) {
        self.state.lock().expect("mock lock").fail_action_names = names;
    }

    /// Seed resource state for precondition/verification tests.
    pub fn seed_resource(&self, key: impl Into<String>, fields: HashMap<String, String>) {
        self.state
            .lock()
            .expect("mock lock")
            .resources
            .insert(key.into(), fields);
    }

    /// Read resource state (tests).
    pub fn get_resource(&self, key: &str) -> Option<HashMap<String, String>> {
        self.state
            .lock()
            .expect("mock lock")
            .resources
            .get(key)
            .cloned()
    }

    fn resource_key(inputs: &serde_json::Value) -> String {
        inputs
            .get("resource_key")
            .and_then(|v| v.as_str())
            .unwrap_or("mock/default")
            .to_string()
    }
}

impl ExecutionCapability for MockExecutionCapability {
    fn descriptor(&self) -> ExecutionCapabilityDescriptor {
        ExecutionCapabilityDescriptor {
            capability_id: "mock.record".into(),
            version: "1".into(),
            risk_level: CapabilityRiskLevel::LowRiskWrite,
            supported_actions: vec!["record_mutation".into(), "fail_mutation".into()],
            required_inputs: vec!["resource_key".into(), "field".into(), "value".into()],
            supports_dry_run: true,
            idempotency_behavior: "client key deduplicates identical mutations".into(),
            reversibility: "overwrite field with previous value when known".into(),
            verification_method: "read resource fields and compare".into(),
            credential_requirements: vec![],
            target_restrictions: vec!["mock".into(), "sandbox".into()],
            failure_semantics: "failed actions leave prior state unchanged".into(),
            description: "In-process mock mutation for tests".into(),
            engineering_loop: EngineeringLoopParticipation::execution_capability_default(),
            accepted_input_types: default_accepted_input_types("mock.record"),
            provider_independent: true,
        }
    }

    fn target(
        &self,
        environment: &str,
        _inputs: &serde_json::Value,
    ) -> RivoraResult<CapabilityTarget> {
        Ok(CapabilityTarget {
            provider: "mock".into(),
            owner: None,
            repository: Some("local".into()),
            branch_or_ref: Some(environment.to_string()),
        })
    }

    fn validate_preconditions(&self, request: &CapabilityInvocation) -> RivoraResult<()> {
        if request.capability_id != "mock.record" {
            return Err(RivoraError::validation("capability mismatch"));
        }
        if !matches!(
            request.action_name.as_str(),
            "record_mutation" | "fail_mutation"
        ) {
            return Err(RivoraError::validation(format!(
                "unsupported action {}",
                request.action_name
            )));
        }
        for key in ["resource_key", "field", "value"] {
            if request
                .inputs
                .get(key)
                .and_then(|value| value.as_str())
                .map_or(true, str::is_empty)
            {
                return Err(RivoraError::validation(format!(
                    "required input `{key}` is missing"
                )));
            }
        }
        Ok(())
    }

    fn dry_run(&self, request: &CapabilityInvocation) -> RivoraResult<DryRunResult> {
        if request.capability_id != "mock.record" {
            return Err(RivoraError::validation("capability mismatch"));
        }
        let key = Self::resource_key(&request.inputs);
        let field = request
            .inputs
            .get("field")
            .and_then(|v| v.as_str())
            .unwrap_or("value");
        let value = request
            .inputs
            .get("value")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let current = self.get_resource(&key);
        let current_state = current.as_ref().map(|m| format!("{m:?}"));
        Ok(DryRunResult {
            actions: vec![request.action_name.clone()],
            target: key.clone(),
            expected_mutations: vec![format!("set {field}={value} on {key}")],
            required_permissions: vec!["mock:write".into()],
            current_state,
            predicted_state: Some(format!("{field}={value}")),
            risks: vec!["test-only mutation".into()],
            policy_decision: ExecutionPolicyDecision {
                decision: ExecutionPolicyDecisionKind::AllowedWithApproval,
                reasons: vec!["mock low-risk write".into()],
                risk_level: CapabilityRiskLevel::LowRiskWrite,
                dry_run_permitted: true,
                live_execution_permitted: true,
                evaluated_at: chrono::Utc::now(),
            },
            missing_preconditions: Vec::new(),
            verification_steps: vec![format!("observe {key} field {field} equals {value}")],
            rollback_options: vec!["set previous field value".into()],
            simulated: true,
        })
    }

    fn execute(&self, request: &CapabilityInvocation) -> RivoraResult<CapabilityExecutionResult> {
        if request.capability_id != "mock.record" {
            return Err(RivoraError::validation("capability mismatch"));
        }
        let mut state = self.state.lock().expect("mock lock");
        if let Some(prev) = state.idempotency.get(&request.idempotency_key) {
            let mut dup = prev.clone();
            dup.duplicate_suppressed = true;
            dup.warnings.push("idempotent duplicate suppressed".into());
            return Ok(dup);
        }
        let key = Self::resource_key(&request.inputs);
        let field = request
            .inputs
            .get("field")
            .and_then(|v| v.as_str())
            .unwrap_or("value")
            .to_string();
        let value = request
            .inputs
            .get("value")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if state.fail_next
            || state
                .fail_action_names
                .iter()
                .any(|n| n == &request.action_name)
            || request.action_name == "fail_mutation"
        {
            state.fail_next = false;
            let result = CapabilityExecutionResult {
                status: CapabilityExecutionStatus::Failed,
                request_summary: format!("record_mutation {key}.{field}"),
                response_summary: "mock failure".into(),
                changed_resources: Vec::new(),
                unchanged_resources: vec![key],
                external_identifiers: Vec::new(),
                warnings: Vec::new(),
                rollback: RollbackMetadata::default(),
                verification_requirements: Vec::new(),
                evidence_refs: Vec::new(),
                error: Some("mock configured failure".into()),
                duplicate_suppressed: false,
            };
            // Do not store failed attempts as idempotent success.
            return Ok(result);
        }

        let previous = state
            .resources
            .get(&key)
            .and_then(|m| m.get(&field).cloned());
        state
            .resources
            .entry(key.clone())
            .or_default()
            .insert(field.clone(), value.clone());

        let external_id = format!("mock:{key}:{field}");
        let result = CapabilityExecutionResult {
            status: CapabilityExecutionStatus::Success,
            request_summary: format!("set {key}.{field}={value}"),
            response_summary: if state.lie_success {
                "reported success".into()
            } else {
                format!("updated {key}.{field}")
            },
            changed_resources: vec![format!("{key}.{field}")],
            unchanged_resources: Vec::new(),
            external_identifiers: vec![external_id],
            warnings: Vec::new(),
            rollback: RollbackMetadata {
                available: previous.is_some(),
                capability_id: Some("mock.record".into()),
                inputs: previous.as_ref().map(|p| {
                    serde_json::json!({
                        "resource_key": key,
                        "field": field,
                        "value": p,
                    })
                }),
                inverse_action_name: previous.as_ref().map(|_| "record_mutation".into()),
                risks: vec![],
                verification: Some(format!("observe {key}.{field}")),
                irreversible_effects: if previous.is_none() {
                    vec!["first write has no prior value".into()]
                } else {
                    vec![]
                },
            },
            verification_requirements: vec![format!("{field}=={value}")],
            evidence_refs: vec![format!("mock-state:{key}")],
            error: None,
            duplicate_suppressed: false,
        };
        if !state.lie_success {
            state
                .idempotency
                .insert(request.idempotency_key.clone(), result.clone());
        }
        Ok(result)
    }

    fn observe_state(
        &self,
        query: &CapabilityStateQuery,
    ) -> RivoraResult<CapabilityStateObservation> {
        let key = Self::resource_key(&query.inputs);
        let state = self.state.lock().expect("mock lock");
        if state.lie_success {
            // Independent observation disagrees with success response.
            return Ok(CapabilityStateObservation {
                resource_key: key.clone(),
                fields: HashMap::new(),
                summary: "observed empty (mismatch)".into(),
                observed: true,
                verification_status: CapabilityVerificationStatus::Failed,
                error: None,
            });
        }
        match state.resources.get(&key) {
            Some(fields) => Ok(CapabilityStateObservation {
                resource_key: key,
                fields: fields.clone(),
                summary: format!("observed {fields:?}"),
                observed: true,
                verification_status: {
                    let expected_field = query
                        .inputs
                        .get("field")
                        .and_then(|value| value.as_str())
                        .unwrap_or("value");
                    let expected_value = query
                        .inputs
                        .get("value")
                        .and_then(|value| value.as_str())
                        .unwrap_or_default();
                    if fields
                        .get(expected_field)
                        .is_some_and(|value| value == expected_value)
                    {
                        CapabilityVerificationStatus::Passed
                    } else {
                        CapabilityVerificationStatus::Failed
                    }
                },
                error: None,
            }),
            None => Ok(CapabilityStateObservation {
                resource_key: key,
                fields: HashMap::new(),
                summary: "resource not found".into(),
                observed: false,
                verification_status: CapabilityVerificationStatus::Failed,
                error: Some("resource not found".into()),
            }),
        }
    }
}

/// Evaluate centralized execution policy (RFC-025 / RFC-026).
pub fn evaluate_execution_policy(
    descriptor: Option<&ExecutionCapabilityDescriptor>,
    capability_id: &str,
    environment: &str,
    action_count: usize,
    supports_dry_run: bool,
) -> ExecutionPolicyDecision {
    let now = chrono::Utc::now();
    let Some(desc) = descriptor else {
        return ExecutionPolicyDecision {
            decision: ExecutionPolicyDecisionKind::Denied,
            reasons: vec![format!("capability `{capability_id}` is not registered")],
            risk_level: CapabilityRiskLevel::Prohibited,
            dry_run_permitted: false,
            live_execution_permitted: false,
            evaluated_at: now,
        };
    };

    match desc.risk_level {
        CapabilityRiskLevel::Prohibited | CapabilityRiskLevel::HighRiskWrite => {
            return ExecutionPolicyDecision {
                decision: ExecutionPolicyDecisionKind::Denied,
                reasons: vec![format!(
                    "risk level {} is not permitted in v0.6",
                    desc.risk_level.as_str()
                )],
                risk_level: desc.risk_level,
                dry_run_permitted: false,
                live_execution_permitted: false,
                evaluated_at: now,
            };
        }
        _ => {}
    }

    if action_count == 0 {
        return ExecutionPolicyDecision {
            decision: ExecutionPolicyDecisionKind::Denied,
            reasons: vec!["plan has no actions".into()],
            risk_level: desc.risk_level,
            dry_run_permitted: false,
            live_execution_permitted: false,
            evaluated_at: now,
        };
    }

    if action_count > 20 {
        return ExecutionPolicyDecision {
            decision: ExecutionPolicyDecisionKind::Denied,
            reasons: vec!["action count exceeds v0.6 blast-radius limit (20)".into()],
            risk_level: desc.risk_level,
            dry_run_permitted: supports_dry_run && desc.supports_dry_run,
            live_execution_permitted: false,
            evaluated_at: now,
        };
    }

    // Production environment: still allowed with approval for low/bounded only.
    let mut reasons = vec![
        format!("capability {}", desc.capability_id),
        format!("risk {}", desc.risk_level.as_str()),
        format!("environment {environment}"),
    ];

    if environment.eq_ignore_ascii_case("production")
        && matches!(desc.risk_level, CapabilityRiskLevel::BoundedWrite)
    {
        reasons.push("production bounded write requires explicit approval".into());
    }

    let dry = supports_dry_run && desc.supports_dry_run;
    ExecutionPolicyDecision {
        decision: ExecutionPolicyDecisionKind::AllowedWithApproval,
        reasons,
        risk_level: desc.risk_level,
        dry_run_permitted: dry,
        live_execution_permitted: true,
        evaluated_at: now,
    }
}

/// Confidence helper for verification aggregation.
pub fn verification_confidence(passed: usize, total: usize) -> Confidence {
    if total == 0 {
        return Confidence::none();
    }
    Confidence::new(passed as f64 / total as f64)
}
