//! Capability Engineering Loop orchestration (RFC-028 / v0.7).
//!
//! The Runtime is the sole owner of loop reasoning. Capabilities provide
//! typed contributions; this module validates them, applies existing
//! subsystem APIs, and persists durable lifecycle runs.

use chrono::Utc;

use crate::domain::InvestigationId;
use crate::domain::{
    AssessmentType, CanonicalInputType, CapabilityLifecycleContributions, CapabilityLifecycleRun,
    CapabilityLifecycleRunListing, CapabilityLifecycleTrace, CapabilityRouteMatch,
    CapabilityRoutingDecision, Confidence, ContributionIdentity, EngineeringLoopParticipation,
    EngineeringLoopStage, Evaluation, ExecutionAttemptStatus, LifecycleContributionContext,
    LifecycleParticipation, LifecycleRunStatus, LifecycleStageRecord, LifecycleStageStatus,
    MemoryRecord, ObjectId, Observation, ObservationKind, Provenance, Severity, StageContribution,
    VerificationReceipt, VerificationResult,
};
use crate::error::{RivoraError, RivoraResult};
use crate::runtime::Runtime;

impl Runtime {
    /// Route Observations to compatible registered Capabilities (deterministic).
    ///
    /// Matching uses stable `accepted_input_types` and ObservationKind type ids —
    /// never human-readable names alone.
    pub fn route_observations_to_capabilities(
        &self,
        investigation_id: InvestigationId,
        observation_ids: &[ObjectId],
    ) -> RivoraResult<CapabilityRoutingDecision> {
        let _inv = self.store.load_investigation(&investigation_id)?;
        let mut observations = Vec::new();
        for id in observation_ids {
            let obs = self
                .store
                .list_observations(&investigation_id)?
                .into_iter()
                .find(|o| o.id == *id)
                .ok_or(RivoraError::ObjectNotFound(*id))?;
            observations.push(obs);
        }
        if observations.is_empty() {
            // Allow routing by investigation-wide latest observations when empty? No —
            // explicit empty input is an empty decision.
            return Ok(CapabilityRoutingDecision {
                observation_ids: vec![],
                input_types: vec![],
                matches: vec![],
                unsupported: true,
                ambiguous: false,
                version_incompatibilities: vec![],
                missing_prerequisites: vec![],
                reasons: vec!["no observations provided for routing".into()],
                schema_version: crate::domain::ENGINEERING_LOOP_SCHEMA_VERSION,
            });
        }

        let input_types: Vec<String> = observations
            .iter()
            .map(|o| CanonicalInputType::from_observation_kind(&o.kind).0)
            .collect();
        let mut unique_types = input_types.clone();
        unique_types.sort();
        unique_types.dedup();

        let descriptors = self.list_execution_capabilities();
        let mut matches = Vec::new();
        let mut version_incompatibilities = Vec::new();
        let mut missing_prerequisites = Vec::new();

        for desc in &descriptors {
            let accepted = if desc.accepted_input_types.is_empty() {
                crate::domain::default_accepted_input_types(&desc.capability_id)
            } else {
                desc.accepted_input_types.clone()
            };
            let matched: Vec<String> = unique_types
                .iter()
                .filter(|t| accepted.iter().any(|a| a == *t))
                .cloned()
                .collect();
            if matched.is_empty() {
                continue;
            }
            if desc.version.trim().is_empty() {
                version_incompatibilities.push(format!(
                    "capability `{}` has empty version",
                    desc.capability_id
                ));
                continue;
            }
            // Credential prerequisites are informational for routing, not hard fail.
            if !desc.credential_requirements.is_empty() {
                // Soft note only — routing still matches; execution enforces credentials.
                missing_prerequisites.push(format!(
                    "capability `{}` requires credentials: {}",
                    desc.capability_id,
                    desc.credential_requirements.join(", ")
                ));
            }
            matches.push(CapabilityRouteMatch {
                capability_id: desc.capability_id.clone(),
                version: desc.version.clone(),
                matched_input_types: matched,
                rank: 0, // filled after sort
                reason: format!(
                    "accepted input types overlap with observation kinds {:?}",
                    unique_types
                ),
            });
        }

        // Deterministic ordering by capability_id.
        matches.sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        for (i, m) in matches.iter_mut().enumerate() {
            m.rank = i as u32;
        }

        let unsupported = matches.is_empty();
        let ambiguous = matches.len() > 1;
        let mut reasons = Vec::new();
        if unsupported {
            reasons.push(format!(
                "no registered capability accepts input types {:?}",
                unique_types
            ));
        } else if ambiguous {
            reasons.push(format!(
                "multiple capabilities match ({}); Runtime does not auto-select a single primary",
                matches
                    .iter()
                    .map(|m| m.capability_id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        } else {
            reasons.push(format!(
                "single capability match: {}",
                matches[0].capability_id
            ));
        }

        Ok(CapabilityRoutingDecision {
            observation_ids: observation_ids.to_vec(),
            input_types: unique_types,
            matches,
            unsupported,
            ambiguous,
            version_incompatibilities,
            missing_prerequisites,
            reasons,
            schema_version: crate::domain::ENGINEERING_LOOP_SCHEMA_VERSION,
        })
    }

    /// Process the Engineering Loop for a completed execution attempt.
    ///
    /// Idempotent on `lifecycle:{attempt_id}` — replay returns the latest run
    /// without duplicating Memory / Evaluation / Verification artifacts.
    pub fn run_capability_lifecycle_for_attempt(
        &self,
        investigation_id: InvestigationId,
        attempt_id: ObjectId,
        actor: impl Into<String>,
    ) -> RivoraResult<CapabilityLifecycleRun> {
        let actor = require_nonempty_actor(actor)?;
        let attempt = self
            .store
            .load_execution_attempt(&investigation_id, &attempt_id)?;
        if attempt.dry_run {
            return Err(RivoraError::validation(
                "cannot run Engineering Loop for a dry-run attempt",
            ));
        }
        if matches!(
            attempt.status,
            ExecutionAttemptStatus::Started | ExecutionAttemptStatus::Blocked
        ) {
            return Err(RivoraError::precondition(format!(
                "attempt {} is not ready for lifecycle processing ({})",
                attempt.id,
                attempt.status.as_str()
            )));
        }

        let idempotency_key = format!("lifecycle:{}", attempt.lineage_id());
        if let Some(existing) = self
            .store
            .find_lifecycle_run_by_idempotency(&investigation_id, &idempotency_key)?
        {
            // Return latest head for lineage without re-processing.
            return self.lifecycle_run_head(investigation_id, existing.lineage_id);
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
        let descriptor = cap.descriptor();
        let participation = descriptor.engineering_loop.clone();

        let receipts = self
            .store
            .list_execution_receipts(&investigation_id)?
            .receipts
            .into_iter()
            .filter(|r| r.attempt_id == attempt.lineage_id())
            .collect::<Vec<_>>();
        let receipt_ids: Vec<_> = receipts.iter().map(|r| r.id).collect();
        let external_identifiers: Vec<_> = receipts
            .iter()
            .flat_map(|r| r.external_identifiers.clone())
            .collect();
        let verifications = self
            .store
            .list_execution_verifications(&investigation_id)?
            .verifications
            .into_iter()
            .filter(|v| v.attempt_id == attempt.lineage_id())
            .collect::<Vec<_>>();
        let execution_verification_id = verifications.last().map(|v| v.id);

        let api_reported_success = matches!(
            attempt.status,
            ExecutionAttemptStatus::Completed | ExecutionAttemptStatus::DuplicateSuppressed
        );
        let result_summary = format!(
            "attempt {} status={} completed={} failed={} uncertain={}",
            attempt.id,
            attempt.status.as_str(),
            attempt.completed_actions.len(),
            attempt.failed_actions.len(),
            attempt.uncertain_actions.len()
        );

        // Build a synthetic CapabilityExecutionResult summary for contribution hooks.
        let exec_result = crate::domain::CapabilityExecutionResult {
            status: if api_reported_success {
                crate::domain::CapabilityExecutionStatus::Success
            } else if !attempt.failed_actions.is_empty() {
                crate::domain::CapabilityExecutionStatus::Failed
            } else {
                crate::domain::CapabilityExecutionStatus::Partial
            },
            request_summary: format!("plan {} action set", plan.id),
            response_summary: result_summary.clone(),
            changed_resources: receipts
                .iter()
                .flat_map(|r| r.changed_resources.clone())
                .collect(),
            unchanged_resources: vec![],
            external_identifiers: external_identifiers.clone(),
            warnings: vec![],
            rollback: attempt.rollback.clone(),
            verification_requirements: receipts
                .iter()
                .flat_map(|r| r.verification_requirements.clone())
                .collect(),
            evidence_refs: receipt_ids.iter().map(|id| id.to_string()).collect(),
            error: attempt.errors.first().cloned(),
            duplicate_suppressed: attempt.status == ExecutionAttemptStatus::DuplicateSuppressed,
        };

        let context = LifecycleContributionContext {
            investigation_id,
            invocation_id: attempt.id.to_string(),
            actor: actor.clone(),
            idempotency_key: idempotency_key.clone(),
            plan_id: Some(plan.id),
            attempt_id: Some(attempt.id),
            receipt_ids: receipt_ids.clone(),
            proposal_id: Some(plan.proposal_id),
            observation_ids: vec![],
            environment: Some(plan.target_environment.clone()),
            execution_verification_id,
            measured_outcome_id: None,
            implementation_record_id: None,
            action_name: plan.actions.first().map(|a| a.action_name.clone()),
            external_identifiers,
            result_summary,
            api_reported_success,
        };

        let contributions = cap.lifecycle_contributions(&exec_result, &context)?;
        contributions.validate_against(&participation)?;

        self.process_lifecycle_contributions(
            investigation_id,
            &descriptor.capability_id,
            &attempt.id.to_string(),
            Some(plan.id),
            Some(attempt.id),
            &participation,
            contributions,
            &idempotency_key,
            &actor,
        )
    }

    /// Process arbitrary validated lifecycle contributions (also used by tests).
    #[allow(clippy::too_many_arguments)]
    pub fn process_lifecycle_contributions(
        &self,
        investigation_id: InvestigationId,
        capability_id: &str,
        invocation_id: &str,
        plan_id: Option<ObjectId>,
        attempt_id: Option<ObjectId>,
        participation: &EngineeringLoopParticipation,
        contributions: CapabilityLifecycleContributions,
        idempotency_key: &str,
        actor: &str,
    ) -> RivoraResult<CapabilityLifecycleRun> {
        contributions.validate_against(participation)?;

        if let Some(existing) = self
            .store
            .find_lifecycle_run_by_idempotency(&investigation_id, idempotency_key)?
        {
            return self.lifecycle_run_head(investigation_id, existing.lineage_id);
        }

        let pending = CapabilityLifecycleRun::pending(
            investigation_id,
            capability_id,
            invocation_id,
            participation,
            plan_id,
            attempt_id,
            contributions.identity.observation_ids.clone(),
            idempotency_key,
            Provenance::now(actor, "runtime").with_capability("engineering_loop"),
        );
        self.store.append_lifecycle_run(&pending)?;

        let mut stages = pending.stages.clone();
        let mut any_failed = false;
        let mut any_completed = false;
        let mut any_pending_supported = false;

        // Memory
        self.apply_stage(
            &mut stages,
            EngineeringLoopStage::Memory,
            &contributions.memory,
            |value| {
                self.apply_memory_contribution(
                    investigation_id,
                    value,
                    &contributions.identity,
                    actor,
                )
            },
            &mut any_failed,
            &mut any_completed,
            &mut any_pending_supported,
        )?;

        // Evaluation (do not run if memory failed hard for supported path — still attempt when skipped)
        if !stage_blocked(&stages, EngineeringLoopStage::Memory) {
            self.apply_stage(
                &mut stages,
                EngineeringLoopStage::Evaluation,
                &contributions.evaluation,
                |value| {
                    self.apply_evaluation_contribution(
                        investigation_id,
                        value,
                        &contributions.identity,
                        actor,
                    )
                },
                &mut any_failed,
                &mut any_completed,
                &mut any_pending_supported,
            )?;
        } else {
            mark_blocked(
                &mut stages,
                EngineeringLoopStage::Evaluation,
                "blocked by memory stage failure",
            );
            any_failed = true;
        }

        // Verification — independent; never infers success from API
        if stage_failed(&stages, EngineeringLoopStage::Evaluation)
            && contributions.evaluation.as_supported().is_some()
        {
            mark_blocked(
                &mut stages,
                EngineeringLoopStage::Verification,
                "blocked by evaluation stage failure",
            );
            any_failed = true;
        } else {
            let evaluation_artifact_ids: Vec<ObjectId> = stages
                .iter()
                .find(|s| s.stage == EngineeringLoopStage::Evaluation)
                .map(|s| s.artifact_ids.clone())
                .unwrap_or_default();
            self.apply_stage(
                &mut stages,
                EngineeringLoopStage::Verification,
                &contributions.verification,
                |value| {
                    self.apply_verification_contribution(
                        investigation_id,
                        value,
                        &evaluation_artifact_ids,
                        &contributions.identity,
                        actor,
                    )
                },
                &mut any_failed,
                &mut any_completed,
                &mut any_pending_supported,
            )?;
        }

        // Improvement — never auto-applies
        self.apply_stage(
            &mut stages,
            EngineeringLoopStage::Improvement,
            &contributions.improvement,
            |value| {
                self.apply_improvement_contribution(
                    investigation_id,
                    value,
                    &contributions.identity,
                    actor,
                )
            },
            &mut any_failed,
            &mut any_completed,
            &mut any_pending_supported,
        )?;

        // Learning — never without measured evidence
        self.apply_stage(
            &mut stages,
            EngineeringLoopStage::Learning,
            &contributions.learning,
            |value| {
                self.apply_learning_contribution(
                    investigation_id,
                    value,
                    &contributions.identity,
                    actor,
                )
            },
            &mut any_failed,
            &mut any_completed,
            &mut any_pending_supported,
        )?;

        let overall = if any_failed && any_completed {
            LifecycleRunStatus::Partial
        } else if any_failed {
            LifecycleRunStatus::Failed
        } else if any_pending_supported {
            LifecycleRunStatus::Partial
        } else {
            LifecycleRunStatus::Completed
        };

        let finished = pending.revised(
            stages,
            overall,
            format!(
                "Engineering Loop finished as {} for capability `{}` invocation {}",
                overall.as_str(),
                capability_id,
                invocation_id
            ),
            Provenance::now(actor, "runtime").with_capability("engineering_loop_complete"),
        );
        self.store.append_lifecycle_run(&finished)?;
        Ok(finished)
    }

    /// List lifecycle runs for an Investigation.
    pub fn list_lifecycle_runs(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<CapabilityLifecycleRunListing> {
        self.store.list_lifecycle_runs(&investigation_id)
    }

    /// Get a lifecycle run snapshot.
    pub fn get_lifecycle_run(
        &self,
        investigation_id: InvestigationId,
        run_id: ObjectId,
    ) -> RivoraResult<CapabilityLifecycleRun> {
        self.store.load_lifecycle_run(&investigation_id, &run_id)
    }

    /// Trace lifecycle lineage for an attempt or run.
    pub fn trace_capability_lifecycle(
        &self,
        investigation_id: InvestigationId,
        invocation_or_run_id: &str,
    ) -> RivoraResult<CapabilityLifecycleTrace> {
        let listing = self.store.list_lifecycle_runs(&investigation_id)?;
        let mut candidates: Vec<_> = listing
            .runs
            .into_iter()
            .filter(|r| {
                r.invocation_id == invocation_or_run_id
                    || r.id.to_string() == invocation_or_run_id
                    || r.lineage_id.to_string() == invocation_or_run_id
                    || r.attempt_id
                        .map(|id| id.to_string() == invocation_or_run_id)
                        .unwrap_or(false)
            })
            .collect();
        if candidates.is_empty() {
            // Try loading attempt and finding by attempt id.
            if let Ok(attempt_id) = invocation_or_run_id.parse::<ObjectId>() {
                if let Ok(attempt) = self
                    .store
                    .load_execution_attempt(&investigation_id, &attempt_id)
                {
                    let listing = self.store.list_lifecycle_runs(&investigation_id)?;
                    candidates = listing
                        .runs
                        .into_iter()
                        .filter(|r| {
                            r.attempt_id == Some(attempt.id)
                                || r.attempt_id == Some(attempt.lineage_id())
                                || r.invocation_id == attempt.id.to_string()
                        })
                        .collect();
                    if candidates.is_empty() {
                        return Ok(CapabilityLifecycleTrace {
                            investigation_id,
                            capability_id: attempt.capability_id,
                            invocation_id: attempt.id.to_string(),
                            run_lineage_id: None,
                            run_id: None,
                            status: None,
                            plan_id: Some(attempt.plan_id),
                            attempt_id: Some(attempt.id),
                            observation_ids: vec![],
                            stages: vec![],
                            artifacts: serde_json::Map::new(),
                            explanation: "No Engineering Loop run recorded for this attempt yet"
                                .into(),
                        });
                    }
                }
            }
        }
        if candidates.is_empty() {
            return Err(RivoraError::validation(format!(
                "no lifecycle run found for `{invocation_or_run_id}`"
            )));
        }
        candidates.sort_by(|a, b| {
            a.revision_number
                .cmp(&b.revision_number)
                .then_with(|| a.created_at.cmp(&b.created_at))
        });
        let head = candidates.last().cloned().expect("non-empty");
        let mut artifacts = serde_json::Map::new();
        for stage in &head.stages {
            if !stage.artifact_ids.is_empty() {
                artifacts.insert(
                    stage.stage.as_str().to_string(),
                    serde_json::json!(stage
                        .artifact_ids
                        .iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()),
                );
            }
        }
        Ok(CapabilityLifecycleTrace {
            investigation_id,
            capability_id: head.capability_id.clone(),
            invocation_id: head.invocation_id.clone(),
            run_lineage_id: Some(head.lineage_id),
            run_id: Some(head.id),
            status: Some(head.status),
            plan_id: head.plan_id,
            attempt_id: head.attempt_id,
            observation_ids: head.observation_ids.clone(),
            stages: head.stages.clone(),
            artifacts,
            explanation: head.explanation.clone(),
        })
    }

    fn lifecycle_run_head(
        &self,
        investigation_id: InvestigationId,
        lineage_id: ObjectId,
    ) -> RivoraResult<CapabilityLifecycleRun> {
        let listing = self.store.list_lifecycle_runs(&investigation_id)?;
        let mut revs: Vec<_> = listing
            .runs
            .into_iter()
            .filter(|r| r.lineage_id == lineage_id)
            .collect();
        revs.sort_by_key(|r| r.revision_number);
        revs.pop().ok_or_else(|| {
            RivoraError::precondition(format!("lifecycle lineage {lineage_id} has no revisions"))
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_stage<T, F>(
        &self,
        stages: &mut [LifecycleStageRecord],
        stage: EngineeringLoopStage,
        contribution: &StageContribution<T>,
        apply: F,
        any_failed: &mut bool,
        any_completed: &mut bool,
        any_pending_supported: &mut bool,
    ) -> RivoraResult<()>
    where
        F: FnOnce(&T) -> RivoraResult<Vec<ObjectId>>,
    {
        let record = stages
            .iter_mut()
            .find(|s| s.stage == stage)
            .expect("stage present");
        match contribution {
            StageContribution::NotApplicable { reason } => {
                record.status = LifecycleStageStatus::NotApplicable;
                record.detail = Some(reason.clone());
                record.finished_at = Some(Utc::now());
            }
            StageContribution::Unsupported { reason } => {
                record.status = LifecycleStageStatus::Unsupported;
                record.detail = Some(reason.clone());
                record.finished_at = Some(Utc::now());
            }
            StageContribution::Deferred { reason } => {
                record.status = LifecycleStageStatus::Deferred;
                record.detail = Some(reason.clone());
                record.finished_at = Some(Utc::now());
            }
            StageContribution::Supported { value } => {
                record.status = LifecycleStageStatus::Running;
                match apply(value) {
                    Ok(artifacts) => {
                        record.status = LifecycleStageStatus::Completed;
                        record.artifact_ids = artifacts;
                        record.detail = Some(format!("{} completed", stage.as_str()));
                        record.finished_at = Some(Utc::now());
                        *any_completed = true;
                    }
                    Err(err) => {
                        record.status = LifecycleStageStatus::Failed;
                        record.error = Some(err.to_string());
                        record.detail = Some(format!("{} failed", stage.as_str()));
                        record.finished_at = Some(Utc::now());
                        *any_failed = true;
                    }
                }
            }
        }
        let _ = any_pending_supported;
        Ok(())
    }

    fn apply_memory_contribution(
        &self,
        investigation_id: InvestigationId,
        contribution: &crate::domain::MemoryContribution,
        identity: &ContributionIdentity,
        actor: &str,
    ) -> RivoraResult<Vec<ObjectId>> {
        // Idempotent memory key via observation idempotency.
        let idem = format!("loop-memory:{}", identity.idempotency_key);
        let (observation, memory, replay) = {
            let result = self.ingest_observation(crate::runtime::observation::IngestObservationRequest {
                investigation_id,
                kind: ObservationKind::Event,
                summary: contribution.summary.clone(),
                payload: serde_json::json!({
                    "capability_id": identity.capability_id,
                    "invocation_id": identity.invocation_id,
                    "plan_id": identity.plan_id.map(|id| id.to_string()),
                    "attempt_id": identity.attempt_id.map(|id| id.to_string()),
                    "receipt_ids": identity.receipt_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
                    "schema_version": identity.schema_version,
                }),
                source: "capability_lifecycle".into(),
                observed_at: identity.timestamp,
                idempotency_key: Some(idem),
                actor: actor.to_string(),
            })?;
            (result.observation, result.memory, result.idempotent_replay)
        };
        let _ = (observation, replay);
        // Ensure confidence from contribution is recorded via a correction only if needed —
        // for MVP, the MemoryRecord from observation is the durable fact.
        let _ = memory.confidence;
        let mem_id = memory.id;
        // If contribution.confidence differs, still do not rewrite; append-only.
        let _ = contribution.confidence;
        Ok(vec![mem_id])
    }

    fn apply_evaluation_contribution(
        &self,
        investigation_id: InvestigationId,
        contribution: &crate::domain::EvaluationContributionRequest,
        identity: &ContributionIdentity,
        actor: &str,
    ) -> RivoraResult<Vec<ObjectId>> {
        // Prefer existing knowledge; derive when empty.
        let knowledge = match self.store.list_knowledge(&investigation_id)? {
            k if !k.is_empty() => k,
            _ => self.derive_knowledge(investigation_id, actor)?,
        };
        let knowledge_ids: Vec<_> = knowledge.iter().map(|k| k.id).collect();
        let memory = self.store.list_memory(&investigation_id)?;
        let memory_ids: Vec<_> = memory.iter().map(|m| m.id).collect();

        let severity = match contribution.suggested_severity.as_deref() {
            Some("critical") => Severity::Critical,
            Some("high") => Severity::High,
            Some("medium") => Severity::Medium,
            Some("low") => Severity::Low,
            _ => Severity::Info,
        };

        let provenance = Provenance::now(actor, "runtime")
            .with_capability(format!("lifecycle:{}", identity.capability_id))
            .with_evidence(
                contribution
                    .evidence_ids
                    .iter()
                    .copied()
                    .chain(identity.receipt_ids.iter().copied())
                    .collect(),
            );

        let evaluation = Evaluation::new(
            investigation_id,
            AssessmentType::Readiness,
            format!(
                "{} — {}",
                contribution.subject, contribution.expectation
            ),
            severity,
            Confidence::new(0.7),
            knowledge_ids,
            memory_ids,
            format!(
                "{}\nRationale: {}\nCapability: {}\nInvocation: {}\nAPI success is not verified success.",
                contribution.expectation,
                contribution.rationale,
                identity.capability_id,
                identity.invocation_id
            ),
            provenance,
        );
        self.store.append_evaluation(&evaluation)?;
        Ok(vec![evaluation.id])
    }

    fn apply_verification_contribution(
        &self,
        investigation_id: InvestigationId,
        contribution: &crate::domain::VerificationContributionRequest,
        evaluation_artifact_ids: &[ObjectId],
        identity: &ContributionIdentity,
        actor: &str,
    ) -> RivoraResult<Vec<ObjectId>> {
        let evaluation_id = evaluation_artifact_ids
            .first()
            .copied()
            .unwrap_or_else(ObjectId::new);

        // Prefer independent execution verification when present.
        let (result, reason, confidence) = if let Some(ev_id) =
            contribution.execution_verification_id
        {
            let ev = self
                .store
                .load_execution_verification(&investigation_id, &ev_id)?;
            let result = match ev.status {
                crate::domain::ExecutionVerificationStatus::Passed => VerificationResult::Pass,
                crate::domain::ExecutionVerificationStatus::Failed => VerificationResult::Fail,
                crate::domain::ExecutionVerificationStatus::Inconclusive => {
                    VerificationResult::Inconclusive
                }
            };
            (
                result,
                format!(
                    "Independent execution verification {}: {}. Strategy: {}. Checks: {}. Contradictions: {}",
                    ev.id,
                    ev.status.as_str(),
                    contribution.strategy,
                    ev.checks.join("; "),
                    if ev.contradictions.is_empty() {
                        "none".into()
                    } else {
                        ev.contradictions.join("; ")
                    }
                ),
                Confidence::new(0.85),
            )
        } else if contribution.requires_independent_observation {
            (
                VerificationResult::Inconclusive,
                format!(
                    "Verification inconclusive: independent observation still required. Strategy: {}. Required evidence: {}",
                    contribution.strategy,
                    contribution.required_evidence.join("; ")
                ),
                Confidence::new(0.4),
            )
        } else {
            (
                VerificationResult::Inconclusive,
                format!(
                    "Verification inconclusive without independent evidence. Strategy: {}",
                    contribution.strategy
                ),
                Confidence::new(0.3),
            )
        };

        let provenance = Provenance::now(actor, "runtime")
            .with_capability(format!("lifecycle_verify:{}", identity.capability_id))
            .with_evidence(
                contribution
                    .evidence_ids
                    .iter()
                    .copied()
                    .chain(identity.receipt_ids.iter().copied())
                    .collect(),
            );

        let receipt = VerificationReceipt::new(
            investigation_id,
            evaluation_id,
            format!(
                "Capability `{}` invocation {}",
                identity.capability_id, identity.invocation_id
            ),
            result,
            confidence,
            contribution.evidence_ids.clone(),
            vec![],
            reason,
            provenance,
        );
        self.store.append_verification(&receipt)?;
        Ok(vec![receipt.id])
    }

    fn apply_improvement_contribution(
        &self,
        investigation_id: InvestigationId,
        contribution: &crate::domain::ImprovementContributionContext,
        identity: &ContributionIdentity,
        actor: &str,
    ) -> RivoraResult<Vec<ObjectId>> {
        if !contribution.generate_proposal {
            // Durable context is the stage detail itself; no auto proposal.
            // Record a Memory note only if not already covered — skip to avoid duplication.
            let _ = (investigation_id, identity, actor);
            return Ok(vec![]);
        }
        // Explicit generate_proposal: use existing proposal generation engine.
        let proposals = self.generate_improvement_proposals(investigation_id, actor)?;
        Ok(proposals.into_iter().map(|p| p.id).collect())
    }

    fn apply_learning_contribution(
        &self,
        investigation_id: InvestigationId,
        contribution: &crate::domain::LearningContributionContext,
        identity: &ContributionIdentity,
        actor: &str,
    ) -> RivoraResult<Vec<ObjectId>> {
        if !contribution.measured_evidence_available {
            return Err(RivoraError::precondition(
                "learning requires measured evidence; API or execution success is not Outcome success",
            ));
        }
        let Some(outcome_id) = contribution.measured_outcome_id else {
            return Err(RivoraError::precondition(
                "learning contribution missing measured_outcome_id",
            ));
        };
        // Verify the measured outcome exists — do not create synthetic success.
        let outcome = self
            .store
            .load_measured_learning_outcome(&investigation_id, &outcome_id)?;
        let _ = (identity, actor, contribution.summary.as_str());
        Ok(vec![outcome.id])
    }
}

fn require_nonempty_actor(actor: impl Into<String>) -> RivoraResult<String> {
    let actor = actor.into().trim().to_string();
    if actor.is_empty() {
        return Err(RivoraError::validation("actor is required"));
    }
    Ok(actor)
}

fn stage_failed(stages: &[LifecycleStageRecord], stage: EngineeringLoopStage) -> bool {
    stages
        .iter()
        .find(|s| s.stage == stage)
        .map(|s| s.status == LifecycleStageStatus::Failed)
        .unwrap_or(false)
}

fn stage_blocked(stages: &[LifecycleStageRecord], stage: EngineeringLoopStage) -> bool {
    stages
        .iter()
        .find(|s| s.stage == stage)
        .map(|s| {
            matches!(
                s.status,
                LifecycleStageStatus::Failed | LifecycleStageStatus::Blocked
            )
        })
        .unwrap_or(false)
}

fn mark_blocked(stages: &mut [LifecycleStageRecord], stage: EngineeringLoopStage, detail: &str) {
    if let Some(record) = stages.iter_mut().find(|s| s.stage == stage) {
        if matches!(record.participation, LifecycleParticipation::Supported) {
            record.status = LifecycleStageStatus::Blocked;
            record.detail = Some(detail.into());
            record.finished_at = Some(Utc::now());
        }
    }
}

// Silence unused import warnings for types used only in docs/comments paths.
#[allow(dead_code)]
fn _keep_imports(_: MemoryRecord, _: Observation) {}
