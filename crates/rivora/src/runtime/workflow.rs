//! Composite Capabilities and Assisted Workflow execution (RFC-018).

use chrono::Utc;

use crate::domain::{
    AssistedWorkflow, CompositeCapabilityDefinition, InvestigationId, ObjectId, Provenance,
    WorkflowStatus, WorkflowStep, WorkflowStepStatus,
};
use crate::error::{RivoraError, RivoraResult};
use crate::runtime::Runtime;

/// Approved Composite Capability definitions for v0.3 MVP.
pub fn composite_definitions() -> Vec<CompositeCapabilityDefinition> {
    vec![
        CompositeCapabilityDefinition {
            id: "investigate_engineering_problem".into(),
            name: "Investigate Engineering Problem".into(),
            description:
                "Recall, understand, evaluate, verify, and recommend for an Investigation.".into(),
            core_capabilities: vec![
                "recall_memory".into(),
                "derive_knowledge".into(),
                "find_similar_investigations".into(),
                "suggest_recalled_context".into(),
                "evaluate_investigation".into(),
                "verify_all".into(),
                "generate_recommendation".into(),
                "summarize_investigation_state".into(),
            ],
        },
        CompositeCapabilityDefinition {
            id: "assess_deployment_readiness".into(),
            name: "Assess Deployment Readiness".into(),
            description: "Assess whether evidence supports proceed, hold, or inspect.".into(),
            core_capabilities: vec![
                "recall_memory".into(),
                "derive_knowledge".into(),
                "evaluate_investigation".into(),
                "verify_all".into(),
                "assess_deployment_readiness".into(),
                "forecast_risk".into(),
                "generate_recommendation".into(),
                "generate_engineering_report".into(),
            ],
        },
        CompositeCapabilityDefinition {
            id: "explain_failure".into(),
            name: "Explain Failure".into(),
            description: "Generate ranked hypotheses and next verification for a failure.".into(),
            core_capabilities: vec![
                "recall_memory".into(),
                "derive_knowledge".into(),
                "find_similar_investigations".into(),
                "generate_hypotheses".into(),
                "evaluate_investigation".into(),
                "recommend_next_verification".into(),
                "generate_root_cause_guidance".into(),
                "summarize_investigation_state".into(),
            ],
        },
        CompositeCapabilityDefinition {
            id: "propose_engineering_improvement".into(),
            name: "Propose Engineering Improvement".into(),
            description: "Read existing evidence, generate bounded alternatives, compare them, and summarize ranking without accepting or applying a Proposal.".into(),
            core_capabilities: vec![
                "recall_proposal_inputs".into(),
                "generate_improvement_proposals".into(),
                "compare_improvement_proposals".into(),
                "summarize_proposal_ranking".into(),
            ],
        },
    ]
}

fn definition_for(intent: &str) -> RivoraResult<CompositeCapabilityDefinition> {
    composite_definitions()
        .into_iter()
        .find(|d| d.id == intent)
        .ok_or_else(|| {
            RivoraError::validation(format!("unknown composite capability intent: {intent}"))
        })
}

fn plan_steps(def: &CompositeCapabilityDefinition) -> Vec<WorkflowStep> {
    def.core_capabilities
        .iter()
        .enumerate()
        .map(|(i, cap)| {
            let confirmation = matches!(cap.as_str(), "record_outcome" | "complete_investigation");
            let description = match cap.as_str() {
                "recall_memory" => "Recall current Memory",
                "derive_knowledge" => "Derive or refresh Knowledge",
                "find_similar_investigations" => "Find related / similar Investigations",
                "suggest_recalled_context" => "Suggest historical Recalled Context",
                "evaluate_investigation" => "Evaluate current understanding",
                "verify_all" => "Verify available claims",
                "generate_recommendation" => "Generate prioritized Recommendations",
                "summarize_investigation_state" => "Summarize current understanding",
                "assess_deployment_readiness" => "Assess deployment readiness",
                "forecast_risk" => "Forecast evidence-backed risks",
                "generate_engineering_report" => "Generate engineering report",
                "generate_hypotheses" => "Generate ranked hypotheses",
                "recommend_next_verification" => "Recommend next verification",
                "generate_root_cause_guidance" => "Produce root-cause guidance",
                "recall_proposal_inputs" => "Read existing durable Proposal inputs",
                "generate_improvement_proposals" => {
                    "Generate bounded Improvement Proposal alternatives"
                }
                "compare_improvement_proposals" => {
                    "Compare Proposal alternatives with inspectable factors"
                }
                "summarize_proposal_ranking" => {
                    "Summarize Proposal ranking without selecting or accepting"
                }
                other => other,
            };
            WorkflowStep::planned(
                i as u32,
                cap.clone(),
                cap.clone(),
                description,
                confirmation,
            )
        })
        .collect()
}

impl Runtime {
    /// List approved Composite Capability definitions.
    pub fn list_composite_capabilities(&self) -> Vec<CompositeCapabilityDefinition> {
        composite_definitions()
    }

    /// Plan an Assisted Workflow without executing steps.
    pub fn plan_workflow(
        &self,
        investigation_id: InvestigationId,
        intent: impl Into<String>,
        actor: impl Into<String>,
    ) -> RivoraResult<AssistedWorkflow> {
        let actor = actor.into();
        let intent = intent.into();
        let _ = self.store.load_investigation(&investigation_id)?;
        let def = definition_for(&intent)?;
        let steps = plan_steps(&def);
        let provenance = Provenance::now(actor, "runtime").with_capability("plan_workflow");
        let workflow =
            AssistedWorkflow::planned(investigation_id, def.id, def.description, steps, provenance);
        self.store.save_workflow(&workflow)?;
        Ok(workflow)
    }

    /// Open a workflow by id.
    pub fn open_workflow(
        &self,
        investigation_id: InvestigationId,
        workflow_id: ObjectId,
    ) -> RivoraResult<AssistedWorkflow> {
        self.store.load_workflow(&investigation_id, &workflow_id)
    }

    /// List workflows for an Investigation.
    pub fn list_workflows(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<Vec<AssistedWorkflow>> {
        let _ = self.store.load_investigation(&investigation_id)?;
        self.store.list_workflows(&investigation_id)
    }

    /// Grant confirmation for a confirmation-required step.
    pub fn confirm_workflow_step(
        &self,
        investigation_id: InvestigationId,
        workflow_id: ObjectId,
        step_index: u32,
        actor: impl Into<String>,
    ) -> RivoraResult<AssistedWorkflow> {
        let actor = actor.into();
        let mut workflow = self.store.load_workflow(&investigation_id, &workflow_id)?;
        let step = workflow
            .steps
            .iter_mut()
            .find(|s| s.index == step_index)
            .ok_or_else(|| {
                RivoraError::validation(format!("workflow step {step_index} not found"))
            })?;
        if !step.confirmation_required {
            return Err(RivoraError::validation(
                "step does not require confirmation",
            ));
        }
        step.confirmation_granted = true;
        workflow.metadata.insert(
            "last_confirmation_actor".into(),
            serde_json::Value::String(actor),
        );
        self.store.save_workflow(&workflow)?;
        Ok(workflow)
    }

    /// Execute a planned or resumable workflow to completion, partial stop, or failure.
    pub fn execute_workflow(
        &self,
        investigation_id: InvestigationId,
        workflow_id: ObjectId,
        actor: impl Into<String>,
    ) -> RivoraResult<AssistedWorkflow> {
        let actor = actor.into();
        let mut workflow = self.store.load_workflow(&investigation_id, &workflow_id)?;
        if matches!(
            workflow.status,
            WorkflowStatus::Completed | WorkflowStatus::Cancelled
        ) {
            return Err(RivoraError::Precondition(format!(
                "cannot execute workflow in status {}",
                workflow.status.as_str()
            )));
        }

        if workflow.started_at.is_none() {
            workflow.started_at = Some(Utc::now());
        }
        workflow.status = WorkflowStatus::Running;
        self.store.save_workflow(&workflow)?;

        let step_count = workflow.steps.len();
        for i in 0..step_count {
            let status = workflow.steps[i].status;
            if matches!(
                status,
                WorkflowStepStatus::Completed
                    | WorkflowStepStatus::Skipped
                    | WorkflowStepStatus::Cancelled
            ) {
                continue;
            }

            if workflow.steps[i].confirmation_required && !workflow.steps[i].confirmation_granted {
                workflow.status = WorkflowStatus::PartiallyCompleted;
                workflow.steps[i].notes =
                    "Awaiting explicit confirmation before executing this step.".into();
                workflow.summary = Some(self.build_workflow_summary(&workflow));
                workflow.completed_at = Some(Utc::now());
                self.store.save_workflow(&workflow)?;
                return Ok(workflow);
            }

            workflow.steps[i].status = WorkflowStepStatus::Running;
            workflow.steps[i].started_at = Some(Utc::now());
            self.store.save_workflow(&workflow)?;

            let capability = workflow.steps[i].capability.clone();
            match self.run_core_capability(investigation_id, &capability, &actor) {
                Ok((outputs, evidence, notes)) => {
                    workflow.steps[i].status = WorkflowStepStatus::Completed;
                    workflow.steps[i].output_refs = outputs;
                    workflow.steps[i].evidence_refs = evidence;
                    workflow.steps[i].notes = notes;
                    workflow.steps[i].completed_at = Some(Utc::now());
                    workflow.steps[i].failure = None;
                }
                Err(e) => {
                    workflow.steps[i].status = WorkflowStepStatus::Failed;
                    workflow.steps[i].failure = Some(e.to_string());
                    workflow.steps[i].completed_at = Some(Utc::now());
                    // Soft-fail: continue for non-critical search/context steps.
                    if matches!(
                        capability.as_str(),
                        "find_similar_investigations" | "suggest_recalled_context"
                    ) {
                        workflow.steps[i].status = WorkflowStepStatus::Skipped;
                        workflow.steps[i].skip_reason = Some(e.to_string());
                        workflow.steps[i].failure = None;
                    } else {
                        // Mark remaining planned steps as cancelled after hard failure.
                        for j in (i + 1)..step_count {
                            if workflow.steps[j].status == WorkflowStepStatus::Planned {
                                workflow.steps[j].status = WorkflowStepStatus::Cancelled;
                                workflow.steps[j].skip_reason = Some("prior step failed".into());
                            }
                        }
                        workflow.status = if workflow
                            .steps
                            .iter()
                            .any(|s| s.status == WorkflowStepStatus::Completed)
                        {
                            WorkflowStatus::PartiallyCompleted
                        } else {
                            WorkflowStatus::Failed
                        };
                        workflow.summary = Some(self.build_workflow_summary(&workflow));
                        workflow.completed_at = Some(Utc::now());
                        self.store.save_workflow(&workflow)?;
                        return Ok(workflow);
                    }
                }
            }
            self.store.save_workflow(&workflow)?;
        }

        let any_failed = workflow
            .steps
            .iter()
            .any(|s| s.status == WorkflowStepStatus::Failed);
        let all_done = workflow.steps.iter().all(|s| {
            matches!(
                s.status,
                WorkflowStepStatus::Completed
                    | WorkflowStepStatus::Skipped
                    | WorkflowStepStatus::Cancelled
            )
        });
        workflow.status = if any_failed {
            WorkflowStatus::PartiallyCompleted
        } else if all_done {
            WorkflowStatus::Completed
        } else {
            WorkflowStatus::PartiallyCompleted
        };
        workflow.summary = Some(self.build_workflow_summary(&workflow));
        workflow.completed_at = Some(Utc::now());
        self.store.save_workflow(&workflow)?;
        Ok(workflow)
    }

    /// Plan and immediately execute a Composite Capability.
    pub fn run_composite(
        &self,
        investigation_id: InvestigationId,
        intent: impl Into<String>,
        actor: impl Into<String>,
    ) -> RivoraResult<AssistedWorkflow> {
        let actor = actor.into();
        let workflow = self.plan_workflow(investigation_id, intent, actor.clone())?;
        self.execute_workflow(investigation_id, workflow.id, actor)
    }

    /// Cancel a workflow safely, preserving completed steps.
    pub fn cancel_workflow(
        &self,
        investigation_id: InvestigationId,
        workflow_id: ObjectId,
        reason: Option<String>,
        actor: impl Into<String>,
    ) -> RivoraResult<AssistedWorkflow> {
        let actor = actor.into();
        let mut workflow = self.store.load_workflow(&investigation_id, &workflow_id)?;
        if matches!(
            workflow.status,
            WorkflowStatus::Completed | WorkflowStatus::Cancelled
        ) {
            return Err(RivoraError::Precondition(format!(
                "cannot cancel workflow in status {}",
                workflow.status.as_str()
            )));
        }
        for step in &mut workflow.steps {
            if matches!(
                step.status,
                WorkflowStepStatus::Planned | WorkflowStepStatus::Running
            ) {
                step.status = WorkflowStepStatus::Cancelled;
                step.skip_reason = Some(
                    reason
                        .clone()
                        .unwrap_or_else(|| "cancelled by operator".into()),
                );
            }
        }
        workflow.status = WorkflowStatus::Cancelled;
        workflow.cancellation_reason = reason.or_else(|| Some(format!("cancelled by {actor}")));
        workflow.summary = Some(self.build_workflow_summary(&workflow));
        workflow.completed_at = Some(Utc::now());
        self.store.save_workflow(&workflow)?;
        Ok(workflow)
    }

    /// Resume a partially completed or failed workflow after safe steps.
    pub fn resume_workflow(
        &self,
        investigation_id: InvestigationId,
        workflow_id: ObjectId,
        actor: impl Into<String>,
    ) -> RivoraResult<AssistedWorkflow> {
        let actor = actor.into();
        let mut workflow = self.store.load_workflow(&investigation_id, &workflow_id)?;
        if !matches!(
            workflow.status,
            WorkflowStatus::PartiallyCompleted | WorkflowStatus::Failed | WorkflowStatus::Running
        ) {
            return Err(RivoraError::Precondition(format!(
                "cannot resume workflow in status {}",
                workflow.status.as_str()
            )));
        }
        // Re-open cancelled-after-failure steps for resume, and retry failed steps.
        for step in &mut workflow.steps {
            if step.status == WorkflowStepStatus::Failed {
                step.status = WorkflowStepStatus::Planned;
                step.failure = None;
                step.started_at = None;
                step.completed_at = None;
            } else if step.status == WorkflowStepStatus::Cancelled
                && step
                    .skip_reason
                    .as_deref()
                    .is_some_and(|r| r.contains("prior step failed"))
            {
                step.status = WorkflowStepStatus::Planned;
                step.skip_reason = None;
            }
        }
        workflow.status = WorkflowStatus::Running;
        workflow.completed_at = None;
        workflow.summary = None;
        self.store.save_workflow(&workflow)?;
        self.execute_workflow(investigation_id, workflow_id, actor)
    }

    /// Retry a single failed safe step, then continue.
    pub fn retry_workflow_step(
        &self,
        investigation_id: InvestigationId,
        workflow_id: ObjectId,
        step_index: u32,
        actor: impl Into<String>,
    ) -> RivoraResult<AssistedWorkflow> {
        let actor = actor.into();
        let mut workflow = self.store.load_workflow(&investigation_id, &workflow_id)?;
        let step = workflow
            .steps
            .iter_mut()
            .find(|s| s.index == step_index)
            .ok_or_else(|| {
                RivoraError::validation(format!("workflow step {step_index} not found"))
            })?;
        if step.status != WorkflowStepStatus::Failed {
            return Err(RivoraError::validation("only failed steps can be retried"));
        }
        step.status = WorkflowStepStatus::Planned;
        step.failure = None;
        step.started_at = None;
        step.completed_at = None;
        // Un-cancel subsequent steps that were cancelled due to this failure.
        for s in &mut workflow.steps {
            if s.index > step_index
                && s.status == WorkflowStepStatus::Cancelled
                && s.skip_reason
                    .as_deref()
                    .is_some_and(|r| r.contains("prior step failed"))
            {
                s.status = WorkflowStepStatus::Planned;
                s.skip_reason = None;
            }
        }
        workflow.status = WorkflowStatus::Running;
        workflow.completed_at = None;
        self.store.save_workflow(&workflow)?;
        self.execute_workflow(investigation_id, workflow_id, actor)
    }

    /// Explain why a workflow or step is in its current state.
    pub fn explain_workflow(
        &self,
        investigation_id: InvestigationId,
        workflow_id: ObjectId,
    ) -> RivoraResult<String> {
        let workflow = self.store.load_workflow(&investigation_id, &workflow_id)?;
        let mut lines = vec![
            format!(
                "Workflow {} intent={} status={}",
                workflow.id,
                workflow.intent,
                workflow.status.as_str()
            ),
            format!("Description: {}", workflow.intent_description),
        ];
        for step in &workflow.steps {
            lines.push(format!(
                "  step {} [{}] capability={} status={} notes={}",
                step.index,
                step.step_id,
                step.capability,
                step.status.as_str(),
                if step.notes.is_empty() {
                    "-"
                } else {
                    &step.notes
                }
            ));
            if let Some(fail) = &step.failure {
                lines.push(format!("    failure: {fail}"));
            }
            if let Some(skip) = &step.skip_reason {
                lines.push(format!("    skipped: {skip}"));
            }
            if step.confirmation_required {
                lines.push(format!(
                    "    confirmation_required=true granted={}",
                    step.confirmation_granted
                ));
            }
        }
        if let Some(summary) = &workflow.summary {
            lines.push(format!("Summary: {summary}"));
        }
        Ok(lines.join("\n"))
    }

    /// Summarize a workflow without re-running it.
    pub fn summarize_workflow(
        &self,
        investigation_id: InvestigationId,
        workflow_id: ObjectId,
    ) -> RivoraResult<String> {
        let workflow = self.store.load_workflow(&investigation_id, &workflow_id)?;
        Ok(self.build_workflow_summary(&workflow))
    }

    fn build_workflow_summary(&self, workflow: &AssistedWorkflow) -> String {
        let completed = workflow
            .steps
            .iter()
            .filter(|s| s.status == WorkflowStepStatus::Completed)
            .count();
        let failed = workflow
            .steps
            .iter()
            .filter(|s| s.status == WorkflowStepStatus::Failed)
            .count();
        let skipped = workflow
            .steps
            .iter()
            .filter(|s| s.status == WorkflowStepStatus::Skipped)
            .count();
        let cancelled = workflow
            .steps
            .iter()
            .filter(|s| s.status == WorkflowStepStatus::Cancelled)
            .count();
        format!(
            "Assisted workflow `{}` ({}) is {}. Steps: {} completed, {} failed, {} skipped, {} cancelled of {} total. {}",
            workflow.intent,
            workflow.id,
            workflow.status.as_str(),
            completed,
            failed,
            skipped,
            cancelled,
            workflow.steps.len(),
            workflow
                .cancellation_reason
                .as_deref()
                .unwrap_or("No external systems were mutated.")
        )
    }

    /// Execute one Core Capability by stable name for workflow composition.
    fn run_core_capability(
        &self,
        investigation_id: InvestigationId,
        capability: &str,
        actor: &str,
    ) -> RivoraResult<(Vec<ObjectId>, Vec<ObjectId>, String)> {
        match capability {
            "recall_memory" => {
                let mem = self.recall_memory(investigation_id)?;
                let ids: Vec<_> = mem.iter().map(|m| m.id).collect();
                Ok((
                    ids.clone(),
                    ids,
                    format!("Recalled {} Memory record(s).", mem.len()),
                ))
            }
            "derive_knowledge" => {
                let knowledge = self.derive_knowledge(investigation_id, actor)?;
                let ids: Vec<_> = knowledge.iter().map(|k| k.id).collect();
                Ok((
                    ids.clone(),
                    ids,
                    format!("Derived {} Knowledge object(s).", knowledge.len()),
                ))
            }
            "find_similar_investigations" => {
                let similar = self.find_similar_investigations(investigation_id, Some(5))?;
                Ok((
                    Vec::new(),
                    Vec::new(),
                    format!("Found {} similar Investigation(s).", similar.len()),
                ))
            }
            "suggest_recalled_context" => {
                let ctx = self.suggest_recalled_context(investigation_id, actor)?;
                let ids: Vec<_> = ctx.iter().map(|c| c.id).collect();
                Ok((
                    ids.clone(),
                    ids,
                    format!("Suggested {} Recalled Context record(s).", ctx.len()),
                ))
            }
            "evaluate_investigation" => {
                let evals = self.evaluate_investigation(investigation_id, actor)?;
                let ids: Vec<_> = evals.iter().map(|e| e.id).collect();
                Ok((
                    ids.clone(),
                    ids,
                    format!("Produced {} Evaluation(s).", evals.len()),
                ))
            }
            "verify_all" => {
                let receipts = self.verify_all(investigation_id, actor)?;
                let ids: Vec<_> = receipts.iter().map(|r| r.id).collect();
                Ok((
                    ids.clone(),
                    ids,
                    format!("Produced {} Verification Receipt(s).", receipts.len()),
                ))
            }
            "generate_recommendation" => {
                let recs = self.generate_recommendation(investigation_id, actor)?;
                let ids: Vec<_> = recs.iter().map(|r| r.id).collect();
                Ok((
                    ids.clone(),
                    ids,
                    format!("Generated {} Recommendation(s).", recs.len()),
                ))
            }
            "summarize_investigation_state" => {
                let summary = self.summarize_investigation_state(investigation_id, actor)?;
                Ok((Vec::new(), Vec::new(), summary.summary))
            }
            "assess_deployment_readiness" => {
                let readiness = self.assess_deployment_readiness(investigation_id, actor)?;
                Ok((
                    vec![readiness.id],
                    readiness.supporting_evidence.clone(),
                    format!(
                        "Readiness status={} confidence={:.0}%",
                        readiness.status.as_str(),
                        readiness.confidence.value() * 100.0
                    ),
                ))
            }
            "forecast_risk" => {
                let forecast = self.forecast_risk(investigation_id, actor)?;
                Ok((
                    vec![forecast.id],
                    forecast
                        .items
                        .iter()
                        .flat_map(|i| i.supporting_evidence.iter().copied())
                        .collect(),
                    forecast.summary,
                ))
            }
            "generate_engineering_report" => {
                let report = self.generate_engineering_report(investigation_id, actor)?;
                Ok((
                    vec![report.id],
                    Vec::new(),
                    format!("Generated report `{}`.", report.title),
                ))
            }
            "generate_hypotheses" => {
                let hyps = self.generate_hypotheses(investigation_id, actor)?;
                let ids: Vec<_> = hyps.iter().map(|h| h.id).collect();
                Ok((
                    ids.clone(),
                    hyps.iter()
                        .flat_map(|h| h.supporting_evidence.iter().copied())
                        .collect(),
                    format!("Generated {} Hypothesis(es).", hyps.len()),
                ))
            }
            "recommend_next_verification" => {
                let suggestions = self.recommend_next_verification(investigation_id, actor)?;
                let ids: Vec<_> = suggestions.iter().map(|s| s.id).collect();
                Ok((
                    ids,
                    suggestions
                        .iter()
                        .flat_map(|s| s.supporting_evidence.iter().copied())
                        .collect(),
                    format!("Suggested {} next verification step(s).", suggestions.len()),
                ))
            }
            "generate_root_cause_guidance" => {
                let guidance = self.generate_root_cause_guidance(investigation_id, actor)?;
                Ok((
                    vec![guidance.id],
                    guidance.supporting_evidence.clone(),
                    guidance.guidance.clone(),
                ))
            }
            "recall_proposal_inputs" => {
                let mut ids = Vec::new();
                ids.extend(
                    self.store
                        .list_observations(&investigation_id)?
                        .iter()
                        .map(|item| item.id),
                );
                ids.extend(
                    self.store
                        .list_memory(&investigation_id)?
                        .iter()
                        .map(|item| item.id),
                );
                ids.extend(
                    self.store
                        .list_knowledge(&investigation_id)?
                        .iter()
                        .map(|item| item.id),
                );
                ids.extend(
                    self.store
                        .list_evaluations(&investigation_id)?
                        .iter()
                        .map(|item| item.id),
                );
                ids.extend(
                    self.store
                        .list_verifications(&investigation_id)?
                        .iter()
                        .map(|item| item.id),
                );
                ids.sort_by_key(|id| id.to_string());
                ids.dedup();
                Ok((
                    Vec::new(),
                    ids.clone(),
                    format!(
                        "Read {} existing durable input(s); no source objects were created or modified.",
                        ids.len()
                    ),
                ))
            }
            "generate_improvement_proposals" => {
                let proposals = self.generate_improvement_proposals(investigation_id, actor)?;
                let ids: Vec<_> = proposals.iter().map(|proposal| proposal.id).collect();
                let evidence = proposals
                    .iter()
                    .flat_map(|proposal| {
                        proposal
                            .generation_inputs
                            .iter()
                            .map(|reference| reference.object_id)
                    })
                    .collect();
                Ok((
                    ids,
                    evidence,
                    format!(
                        "Generated {} bounded Draft Proposal alternative(s); none accepted or applied.",
                        proposals.len()
                    ),
                ))
            }
            "compare_improvement_proposals" => {
                let proposals = self.list_improvement_proposals(investigation_id)?;
                let ids: Vec<_> = proposals
                    .proposals
                    .iter()
                    .map(|proposal| proposal.id)
                    .collect();
                let comparison =
                    self.compare_improvement_proposals(investigation_id, ids.clone())?;
                Ok((
                    Vec::new(),
                    ids,
                    format!(
                        "Compared {} Proposal alternative(s) using {}. Ranking is inspectable guidance only.",
                        comparison.ranked.len(), comparison.method
                    ),
                ))
            }
            "summarize_proposal_ranking" => {
                let comparison = self.prioritize_improvement_proposals(investigation_id)?;
                let top = comparison
                    .ranked
                    .first()
                    .map(|ranked| ranked.proposal_id.to_string())
                    .unwrap_or_else(|| "none".into());
                Ok((
                    Vec::new(),
                    comparison
                        .ranked
                        .iter()
                        .map(|ranked| ranked.proposal_id)
                        .collect(),
                    format!(
                        "Top review candidate: {top}. No Proposal was selected, accepted, applied, implemented, or verified."
                    ),
                ))
            }
            other => Err(RivoraError::validation(format!(
                "unsupported core capability in composite: {other}"
            ))),
        }
    }
}
