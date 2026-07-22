//! Improvement Proposal lifecycle orchestration (RFC-020).

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::domain::{
    Confidence, EvidenceReference, EvidenceScope, HypothesisStatus, ImprovementProposal,
    InvestigationId, ObjectId, OutcomeDisposition, ProposalAlternative, ProposalArtifact,
    ProposalArtifactListing, ProposalCategory, ProposalComparison, ProposalComparisonFactor,
    ProposalEffort, ProposalFeedback, ProposalFeedbackCategory, ProposalGenerationMethod,
    ProposalListing, ProposalPriority, ProposalRisk, ProposalStatus, ProposalStorageDiagnostic,
    ProposalTrace, ProposalTransitionAuthority, ProposalVerificationPlan, Provenance,
    RankedProposal, RecalledContextState, Severity, VerificationResult,
};
use crate::error::{RivoraError, RivoraResult};
use crate::runtime::Runtime;

/// Explicit caller request to create a concrete Proposal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateProposalRequest {
    /// Proposal title.
    pub title: String,
    /// Concise summary.
    pub summary: String,
    /// Evidence-backed rationale.
    pub rationale: String,
    /// Category.
    pub category: ProposalCategory,
    /// Priority.
    pub priority: ProposalPriority,
    /// Confidence.
    pub confidence: Confidence,
    /// Explicit current supporting evidence references.
    #[serde(default)]
    pub supporting_evidence_ids: Vec<ObjectId>,
    /// Explicit current contradicting evidence references.
    #[serde(default)]
    pub contradicting_evidence_ids: Vec<ObjectId>,
    /// Explicit source Recommendations.
    #[serde(default)]
    pub source_recommendation_ids: Vec<ObjectId>,
    /// Explicit affected components.
    #[serde(default)]
    pub affected_components: Vec<String>,
    /// Explicit likely affected resources.
    #[serde(default)]
    pub affected_resources: Vec<String>,
}

/// Explicit content changes for a preserved Proposal revision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RefineProposalRequest {
    /// Replacement title when present.
    pub title: Option<String>,
    /// Replacement summary when present.
    pub summary: Option<String>,
    /// Replacement rationale when present.
    pub rationale: Option<String>,
    /// Replacement affected components when present.
    pub affected_components: Option<Vec<String>>,
    /// Replacement proposed test strategy when present.
    pub test_strategy: Option<Vec<String>>,
}

/// Investigation-level Proposal portfolio filters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProposalPortfolioFilter {
    /// Filter by lifecycle status.
    pub status: Option<ProposalStatus>,
    /// Filter by priority.
    pub priority: Option<ProposalPriority>,
    /// Filter by category.
    pub category: Option<ProposalCategory>,
    /// Filter by source Recommendation.
    pub source_recommendation_id: Option<ObjectId>,
    /// Filter by affected component.
    pub affected_component: Option<String>,
    /// Only unresolved Critical or High Proposals.
    pub unresolved_high_priority: bool,
}

impl Runtime {
    /// Create a human-requested concrete Proposal.
    pub fn create_improvement_proposal(
        &self,
        investigation_id: InvestigationId,
        request: CreateProposalRequest,
        actor: impl Into<String>,
    ) -> RivoraResult<ImprovementProposal> {
        let actor = actor.into();
        if actor.trim().is_empty() {
            return Err(RivoraError::validation(
                "proposal creator actor is required",
            ));
        }
        self.store.load_investigation(&investigation_id)?;
        let mut proposal = ImprovementProposal::generated(
            investigation_id,
            request.title,
            request.summary,
            request.rationale,
            request.category,
            request.priority,
            request.confidence,
            ProposalGenerationMethod::Human,
            Provenance::now(actor.trim(), "runtime").with_capability("create_improvement_proposal"),
        )?;
        proposal.status = if request.supporting_evidence_ids.is_empty()
            && request.contradicting_evidence_ids.is_empty()
            && request.source_recommendation_ids.is_empty()
        {
            ProposalStatus::Draft
        } else {
            ProposalStatus::Proposed
        };
        proposal.derivation_method = "explicit_caller_proposal_v1".into();
        let available = self.current_proposal_input_ids(&proposal.investigation_id)?;
        for evidence_id in request
            .supporting_evidence_ids
            .iter()
            .chain(&request.contradicting_evidence_ids)
        {
            if !available.contains(evidence_id) {
                return Err(RivoraError::validation(format!(
                    "proposal evidence {} does not belong to the Investigation",
                    evidence_id
                )));
            }
        }
        let recommendation_ids: std::collections::HashSet<_> = self
            .store
            .list_recommendations(&proposal.investigation_id)?
            .iter()
            .map(|recommendation| recommendation.id)
            .collect();
        if request
            .source_recommendation_ids
            .iter()
            .any(|id| !recommendation_ids.contains(id))
        {
            return Err(RivoraError::validation(
                "source Recommendation does not belong to the Investigation",
            ));
        }
        proposal.supporting_evidence =
            scoped_refs(&request.supporting_evidence_ids, EvidenceScope::Current);
        proposal.contradicting_evidence =
            scoped_refs(&request.contradicting_evidence_ids, EvidenceScope::Current);
        proposal.generation_inputs = proposal
            .supporting_evidence
            .iter()
            .chain(&proposal.contradicting_evidence)
            .cloned()
            .collect();
        proposal.generation_inputs.extend(scoped_refs(
            &request.source_recommendation_ids,
            EvidenceScope::Current,
        ));
        dedupe_refs(&mut proposal.generation_inputs);
        proposal.source_recommendation_ids = request.source_recommendation_ids;
        proposal.provenance.supporting_evidence = proposal
            .generation_inputs
            .iter()
            .map(|reference| reference.object_id)
            .collect();
        proposal.affected_components = clean_strings(request.affected_components);
        proposal.affected_resources = clean_strings(request.affected_resources);
        self.persist_sanitized_proposal(proposal)
    }

    /// Load one Proposal snapshot.
    pub fn get_improvement_proposal(
        &self,
        investigation_id: InvestigationId,
        proposal_id: ObjectId,
    ) -> RivoraResult<ImprovementProposal> {
        self.store.load_investigation(&investigation_id)?;
        self.store.load_proposal(&investigation_id, &proposal_id)
    }

    /// List latest Proposal snapshots for an Investigation.
    pub fn list_improvement_proposals(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<ProposalListing> {
        self.store.load_investigation(&investigation_id)?;
        let listing = self.store.list_proposals(&investigation_id)?;
        let mut latest = std::collections::BTreeMap::new();
        for proposal in listing.proposals {
            latest
                .entry(proposal.lineage_id.to_string())
                .and_modify(|current: &mut ImprovementProposal| {
                    if proposal.revision_number > current.revision_number {
                        *current = proposal.clone();
                    }
                })
                .or_insert(proposal);
        }
        let mut proposals: Vec<_> = latest.into_values().collect();
        proposals.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then_with(|| a.id.to_string().cmp(&b.id.to_string()))
        });
        Ok(ProposalListing {
            proposals,
            diagnostics: listing.diagnostics,
        })
    }

    /// Explain a Proposal and its strict no-application boundary.
    pub fn explain_improvement_proposal(
        &self,
        investigation_id: InvestigationId,
        proposal_id: ObjectId,
    ) -> RivoraResult<String> {
        let proposal = self.get_improvement_proposal(investigation_id, proposal_id)?;
        Ok(format!(
            "Proposal {} revision {} [{} / {}]\n{}\nRationale: {}\nSupporting evidence: {}; contradicting evidence: {}.\nProposal only — not applied, not implemented, not verified.",
            proposal.id,
            proposal.revision_number,
            proposal.status.as_str(),
            proposal.priority.as_str(),
            proposal.summary,
            proposal.rationale,
            proposal.supporting_evidence.len(),
            proposal.contradicting_evidence.len(),
        ))
    }

    /// Create an immutable status-transition revision.
    #[allow(clippy::too_many_arguments)]
    pub fn update_improvement_proposal_status(
        &self,
        investigation_id: InvestigationId,
        proposal_id: ObjectId,
        status: ProposalStatus,
        actor: impl Into<String>,
        reason: impl Into<String>,
        authority: ProposalTransitionAuthority,
    ) -> RivoraResult<ImprovementProposal> {
        let proposal = self.get_latest_improvement_proposal(investigation_id, proposal_id)?;
        let next = proposal.transitioned(status, actor, reason, Utc::now(), authority)?;
        self.persist_sanitized_proposal(next)
    }

    /// Add explicit feedback as a preserved immutable revision.
    pub fn add_improvement_proposal_feedback(
        &self,
        investigation_id: InvestigationId,
        proposal_id: ObjectId,
        category: ProposalFeedbackCategory,
        comment: impl Into<String>,
        actor: impl Into<String>,
    ) -> RivoraResult<ImprovementProposal> {
        let comment = comment.into();
        let actor = actor.into();
        if comment.trim().is_empty() || actor.trim().is_empty() {
            return Err(RivoraError::validation(
                "proposal feedback actor and comment are required",
            ));
        }
        let proposal = self.get_latest_improvement_proposal(investigation_id, proposal_id)?;
        ensure_content_revision_allowed(&proposal)?;
        let at = Utc::now();
        let mut next = proposal.revised(actor.trim(), "feedback attached", at)?;
        next.feedback.push(ProposalFeedback {
            category,
            comment: comment.trim().into(),
            actor: actor.trim().into(),
            at,
        });
        self.persist_sanitized_proposal(next)
    }

    /// Refine content into a new immutable revision.
    #[allow(clippy::too_many_arguments)]
    pub fn refine_improvement_proposal(
        &self,
        investigation_id: InvestigationId,
        proposal_id: ObjectId,
        request: RefineProposalRequest,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<ImprovementProposal> {
        let proposal = self.get_latest_improvement_proposal(investigation_id, proposal_id)?;
        ensure_content_revision_allowed(&proposal)?;
        let mut next = proposal.revised(actor, reason, Utc::now())?;
        if let Some(title) = request.title {
            if title.trim().is_empty() {
                return Err(RivoraError::validation("proposal title cannot be empty"));
            }
            next.title = title.trim().into();
        }
        if let Some(summary) = request.summary {
            if summary.trim().is_empty() {
                return Err(RivoraError::validation("proposal summary cannot be empty"));
            }
            next.summary = summary.trim().into();
        }
        if let Some(rationale) = request.rationale {
            if rationale.trim().is_empty() {
                return Err(RivoraError::validation(
                    "proposal rationale cannot be empty",
                ));
            }
            next.rationale = rationale.trim().into();
        }
        if let Some(components) = request.affected_components {
            next.affected_components = clean_strings(components);
        }
        if let Some(strategy) = request.test_strategy {
            next.test_strategy = clean_strings(strategy);
        }
        self.persist_sanitized_proposal(next)
    }

    /// Supersede a Proposal with an explicitly selected replacement.
    pub fn supersede_improvement_proposal(
        &self,
        investigation_id: InvestigationId,
        proposal_id: ObjectId,
        replacement_id: ObjectId,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<ImprovementProposal> {
        if proposal_id == replacement_id {
            return Err(RivoraError::validation(
                "a proposal cannot supersede itself",
            ));
        }
        let _replacement =
            self.get_latest_improvement_proposal(investigation_id, replacement_id)?;
        let proposal = self.get_latest_improvement_proposal(investigation_id, proposal_id)?;
        let mut next = proposal.transitioned(
            ProposalStatus::Superseded,
            actor,
            reason,
            Utc::now(),
            ProposalTransitionAuthority::ExternalCaller,
        )?;
        next.superseding_proposal_id = Some(replacement_id);
        self.persist_sanitized_proposal(next)
    }

    /// List every immutable revision for a Proposal lineage.
    pub fn list_improvement_proposal_revisions(
        &self,
        investigation_id: InvestigationId,
        lineage_id: ObjectId,
    ) -> RivoraResult<ProposalListing> {
        self.store.load_investigation(&investigation_id)?;
        self.store
            .list_proposal_revisions(&investigation_id, &lineage_id)
    }

    /// Generate at least two bounded deterministic Improvement Proposal alternatives.
    pub fn generate_improvement_proposals(
        &self,
        investigation_id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<Vec<ImprovementProposal>> {
        let actor = actor.into();
        if actor.trim().is_empty() {
            return Err(RivoraError::validation(
                "proposal generator actor is required",
            ));
        }
        let investigation = self.store.load_investigation(&investigation_id)?;
        let observations = self.store.list_observations(&investigation_id)?;
        let memory = self.store.list_memory(&investigation_id)?;
        let knowledge = self.store.list_knowledge(&investigation_id)?;
        let evaluations = self.store.list_evaluations(&investigation_id)?;
        let verifications = self.store.list_verifications(&investigation_id)?;
        let hypotheses = self.store.list_hypotheses(&investigation_id)?;
        let recommendations = self.store.list_recommendations(&investigation_id)?;
        let learning = self.store.list_learning(&investigation_id)?;
        let workflows: Vec<_> = self
            .store
            .list_workflows(&investigation_id)?
            .into_iter()
            .filter(|workflow| workflow.intent != "propose_engineering_improvement")
            .collect();
        let verification_suggestions = self
            .store
            .list_verification_suggestions(&investigation_id)?;
        let readiness = self.store.list_deployment_readiness(&investigation_id)?;
        let risk_forecasts = self.store.list_risk_forecasts(&investigation_id)?;
        let root_cause_guidance = self.store.list_root_cause_guidance(&investigation_id)?;
        let engineering_reports = self.store.list_engineering_reports(&investigation_id)?;
        let contexts: Vec<_> = self
            .store
            .list_recalled_context(&investigation_id)?
            .into_iter()
            .filter(|context| context.state == RecalledContextState::Attached)
            .collect();

        if observations.is_empty()
            && memory.is_empty()
            && knowledge.is_empty()
            && evaluations.is_empty()
            && verifications.is_empty()
            && hypotheses.is_empty()
            && recommendations.is_empty()
            && learning.is_empty()
            && workflows.is_empty()
            && verification_suggestions.is_empty()
            && readiness.is_empty()
            && risk_forecasts.is_empty()
            && root_cause_guidance.is_empty()
            && engineering_reports.is_empty()
            && contexts.is_empty()
        {
            return Err(RivoraError::Precondition(
                "an evidence-backed improvement opportunity is required before proposal generation"
                    .into(),
            ));
        }

        let mut current_ids = Vec::new();
        current_ids.extend(observations.iter().map(|item| item.id));
        current_ids.extend(memory.iter().map(|item| item.id));
        current_ids.extend(knowledge.iter().map(|item| item.id));
        current_ids.extend(evaluations.iter().map(|item| item.id));
        current_ids.extend(verifications.iter().map(|item| item.id));
        current_ids.extend(hypotheses.iter().map(|item| item.id));
        current_ids.extend(recommendations.iter().map(|item| item.id));
        current_ids.extend(learning.iter().map(|item| item.id));
        current_ids.extend(workflows.iter().map(|item| item.id));
        current_ids.extend(contexts.iter().map(|item| item.id));
        current_ids.extend(verification_suggestions.iter().map(|item| item.id));
        current_ids.extend(readiness.iter().map(|item| item.id));
        current_ids.extend(risk_forecasts.iter().map(|item| item.id));
        current_ids.extend(root_cause_guidance.iter().map(|item| item.id));
        current_ids.extend(engineering_reports.iter().map(|item| item.id));
        dedupe_ids(&mut current_ids);
        let durable_current_ids: std::collections::HashSet<_> =
            current_ids.iter().copied().collect();

        let mut supporting_ids = Vec::new();
        supporting_ids.extend(observations.iter().map(|item| item.id));
        supporting_ids.extend(memory.iter().map(|item| item.id));
        supporting_ids.extend(knowledge.iter().map(|item| item.id));
        supporting_ids.extend(evaluations.iter().map(|item| item.id));
        supporting_ids.extend(
            hypotheses
                .iter()
                .flat_map(|hypothesis| hypothesis.supporting_evidence.iter().copied()),
        );
        supporting_ids.extend(
            hypotheses
                .iter()
                .filter(|hypothesis| {
                    matches!(
                        hypothesis.status,
                        HypothesisStatus::Supported | HypothesisStatus::Verified
                    )
                })
                .map(|hypothesis| hypothesis.id),
        );
        supporting_ids.extend(
            verifications
                .iter()
                .filter(|receipt| receipt.result == VerificationResult::Pass)
                .map(|receipt| receipt.id),
        );
        supporting_ids.extend(
            verifications
                .iter()
                .filter(|receipt| receipt.result == VerificationResult::Pass)
                .flat_map(|receipt| receipt.evidence_ids.iter().copied()),
        );
        supporting_ids.extend(
            learning
                .iter()
                .filter(|outcome| outcome.disposition == OutcomeDisposition::Successful)
                .map(|outcome| outcome.id),
        );
        supporting_ids.extend(
            verification_suggestions
                .iter()
                .flat_map(|suggestion| suggestion.supporting_evidence.iter().copied()),
        );
        supporting_ids.extend(
            readiness
                .iter()
                .flat_map(|assessment| assessment.supporting_evidence.iter().copied()),
        );
        supporting_ids.extend(risk_forecasts.iter().flat_map(|forecast| {
            forecast
                .items
                .iter()
                .flat_map(|item| item.supporting_evidence.iter().copied())
        }));
        supporting_ids.extend(
            root_cause_guidance
                .iter()
                .flat_map(|guidance| guidance.supporting_evidence.iter().copied()),
        );
        dedupe_ids(&mut supporting_ids);
        supporting_ids.retain(|id| durable_current_ids.contains(id));

        let mut contradicting_ids: Vec<_> = hypotheses
            .iter()
            .flat_map(|hypothesis| hypothesis.contradicting_evidence.iter().copied())
            .chain(
                verifications
                    .iter()
                    .flat_map(|receipt| receipt.conflicting_ids.iter().copied()),
            )
            .collect();
        contradicting_ids.extend(
            hypotheses
                .iter()
                .filter(|hypothesis| {
                    matches!(
                        hypothesis.status,
                        HypothesisStatus::Contradicted | HypothesisStatus::Rejected
                    )
                })
                .map(|hypothesis| hypothesis.id),
        );
        contradicting_ids.extend(
            verifications
                .iter()
                .filter(|receipt| receipt.result == VerificationResult::Fail)
                .map(|receipt| receipt.id),
        );
        contradicting_ids.extend(
            learning
                .iter()
                .filter(|outcome| {
                    matches!(
                        outcome.disposition,
                        OutcomeDisposition::Rejected | OutcomeDisposition::Unsuccessful
                    )
                })
                .map(|outcome| outcome.id),
        );
        contradicting_ids.extend(
            readiness
                .iter()
                .flat_map(|assessment| assessment.contradicting_evidence.iter().copied()),
        );
        contradicting_ids.extend(
            root_cause_guidance
                .iter()
                .flat_map(|guidance| guidance.contradicting_evidence.iter().copied()),
        );
        dedupe_ids(&mut contradicting_ids);
        contradicting_ids.retain(|id| durable_current_ids.contains(id));

        let historical_ids: Vec<_> = contexts
            .iter()
            .flat_map(|context| context.source_object_ids.iter().copied())
            .collect();
        let related_investigation_ids = contexts
            .iter()
            .map(|context| context.source_investigation_id)
            .collect::<Vec<_>>();
        let mut learning_outcome_ids: Vec<_> = learning.iter().map(|item| item.id).collect();
        let mut historical_supporting_ids = Vec::new();
        let mut historical_contradicting_ids = Vec::new();
        for context in &contexts {
            let selected = |id: ObjectId| context.source_object_ids.contains(&id);
            historical_supporting_ids.extend(
                self.store
                    .list_observations(&context.source_investigation_id)?
                    .into_iter()
                    .map(|item| item.id)
                    .filter(|id| selected(*id)),
            );
            historical_supporting_ids.extend(
                self.store
                    .list_memory(&context.source_investigation_id)?
                    .into_iter()
                    .map(|item| item.id)
                    .filter(|id| selected(*id)),
            );
            historical_supporting_ids.extend(
                self.store
                    .list_knowledge(&context.source_investigation_id)?
                    .into_iter()
                    .map(|item| item.id)
                    .filter(|id| selected(*id)),
            );
            historical_supporting_ids.extend(
                self.store
                    .list_evaluations(&context.source_investigation_id)?
                    .into_iter()
                    .map(|item| item.id)
                    .filter(|id| selected(*id)),
            );
            for hypothesis in self
                .store
                .list_hypotheses(&context.source_investigation_id)?
                .into_iter()
                .filter(|item| selected(item.id))
            {
                match hypothesis.status {
                    HypothesisStatus::Supported | HypothesisStatus::Verified => {
                        historical_supporting_ids.push(hypothesis.id);
                    }
                    HypothesisStatus::Contradicted | HypothesisStatus::Rejected => {
                        historical_contradicting_ids.push(hypothesis.id);
                    }
                    HypothesisStatus::Proposed | HypothesisStatus::Inconclusive => {}
                }
            }
            for receipt in self
                .store
                .list_verifications(&context.source_investigation_id)?
                .into_iter()
                .filter(|item| selected(item.id))
            {
                match receipt.result {
                    VerificationResult::Pass => historical_supporting_ids.push(receipt.id),
                    VerificationResult::Fail => historical_contradicting_ids.push(receipt.id),
                    VerificationResult::Inconclusive => {}
                }
            }
            for outcome in self
                .store
                .list_learning(&context.source_investigation_id)?
                .into_iter()
                .filter(|item| selected(item.id))
            {
                learning_outcome_ids.push(outcome.id);
                match outcome.disposition {
                    OutcomeDisposition::Successful => historical_supporting_ids.push(outcome.id),
                    OutcomeDisposition::Rejected | OutcomeDisposition::Unsuccessful => {
                        historical_contradicting_ids.push(outcome.id);
                    }
                    OutcomeDisposition::Accepted | OutcomeDisposition::Ignored => {}
                }
            }
        }
        dedupe_ids(&mut learning_outcome_ids);
        dedupe_ids(&mut historical_supporting_ids);
        dedupe_ids(&mut historical_contradicting_ids);

        let mut current_input_ids = current_ids.clone();
        current_input_ids.extend(supporting_ids.iter().copied());
        current_input_ids.extend(contradicting_ids.iter().copied());
        dedupe_ids(&mut current_input_ids);
        let mut generation_inputs = scoped_refs(&current_input_ids, EvidenceScope::Current);
        generation_inputs.extend(scoped_refs(&historical_ids, EvidenceScope::Historical));
        dedupe_refs(&mut generation_inputs);
        let mut supporting_evidence = scoped_refs(&supporting_ids, EvidenceScope::Current);
        supporting_evidence.extend(scoped_refs(
            &historical_supporting_ids,
            EvidenceScope::Historical,
        ));
        dedupe_refs(&mut supporting_evidence);
        let mut contradicting_evidence = scoped_refs(&contradicting_ids, EvidenceScope::Current);
        contradicting_evidence.extend(scoped_refs(
            &historical_contradicting_ids,
            EvidenceScope::Historical,
        ));
        dedupe_refs(&mut contradicting_evidence);

        let combined = format!(
            "{} {} {}",
            investigation.title,
            observations
                .iter()
                .map(|observation| observation.summary.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            recommendations
                .iter()
                .map(|recommendation| recommendation.summary.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        );
        let category = infer_category(&combined);
        let (priority, priority_explanation) = infer_priority(
            &evaluations,
            &observations,
            &verifications,
            &readiness,
            &risk_forecasts,
        );
        let subject = recommendations
            .last()
            .map(|recommendation| recommendation.summary.clone())
            .unwrap_or_else(|| investigation.title.clone());
        let confidence = if verifications
            .iter()
            .any(|receipt| receipt.result == VerificationResult::Pass)
        {
            Confidence::new(0.8)
        } else if verifications
            .iter()
            .any(|receipt| receipt.result == VerificationResult::Fail)
        {
            Confidence::new(0.4)
        } else if verifications
            .iter()
            .any(|receipt| receipt.result == VerificationResult::Inconclusive)
        {
            Confidence::new(0.45)
        } else {
            Confidence::new(0.5)
        };
        let components = infer_components(&observations);
        let resources = infer_resources(&observations);
        let mut assumptions: Vec<_> = hypotheses
            .iter()
            .filter(|hypothesis| hypothesis.status != HypothesisStatus::Verified)
            .map(|hypothesis| {
                format!(
                    "Unverified hypothesis [{}]: {}",
                    hypothesis.status.as_str(),
                    hypothesis.statement
                )
            })
            .collect();
        assumptions.extend(contexts.iter().map(|context| {
            format!(
                "Attached Recalled Context {} from historical Investigation {} authorized historical influence at {}.",
                context.id,
                context.source_investigation_id,
                context.recalled_at.to_rfc3339()
            )
        }));
        let group_id = ObjectId::new();
        let alternatives = alternative_summaries();

        let mut targeted = ImprovementProposal::generated(
            investigation_id,
            format!("Add targeted validation for {}", investigation.title),
            format!(
                "Add a bounded validation path addressing: {}",
                sanitize_text(&subject)
            ),
            format!(
                "Current evidence contains {} supporting and {} contradicting reference(s). The smallest useful change should validate the observed boundary without broad migration.",
                supporting_evidence.len(),
                contradicting_evidence.len()
            ),
            category,
            priority,
            confidence,
            ProposalGenerationMethod::Deterministic,
            Provenance::now(actor.trim(), "runtime")
                .with_capability("generate_improvement_proposals")
                .with_evidence(current_ids.clone()),
        )?;
        populate_generated(
            &mut targeted,
            group_id,
            priority_explanation.clone(),
            components.clone(),
            resources.clone(),
            supporting_evidence.clone(),
            contradicting_evidence.clone(),
            generation_inputs.clone(),
            &hypotheses,
            &evaluations,
            &verifications,
            &recommendations,
            related_investigation_ids.clone(),
            learning_outcome_ids.clone(),
            &assumptions,
            alternatives.clone(),
            ProposalEffort::Small,
            "targeted_validation_proposal_v1",
            true,
        );

        let mut shared = ImprovementProposal::generated(
            investigation_id,
            format!("Introduce a shared validation boundary for {}", investigation.title),
            format!(
                "Centralize validation for the affected component while preserving existing interfaces: {}",
                sanitize_text(&subject)
            ),
            "A shared boundary may reduce recurrence but has broader scope and verification cost. Evidence and contradictions remain identical to the targeted alternative.",
            category,
            priority,
            Confidence::new((confidence.value() - 0.08).max(0.0)),
            ProposalGenerationMethod::Deterministic,
            Provenance::now(actor.trim(), "runtime")
                .with_capability("generate_improvement_proposals")
                .with_evidence(current_ids),
        )?;
        populate_generated(
            &mut shared,
            group_id,
            priority_explanation,
            components,
            resources,
            supporting_evidence,
            contradicting_evidence,
            generation_inputs,
            &hypotheses,
            &evaluations,
            &verifications,
            &recommendations,
            related_investigation_ids,
            learning_outcome_ids,
            &assumptions,
            alternatives,
            ProposalEffort::Medium,
            "shared_validation_proposal_v1",
            false,
        );

        let targeted = self.persist_sanitized_proposal(targeted)?;
        let shared = self.persist_sanitized_proposal(shared)?;
        Ok(vec![targeted, shared])
    }

    /// Generate alternative Proposals for the current improvement opportunity.
    pub fn generate_proposal_alternatives(
        &self,
        investigation_id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<Vec<ImprovementProposal>> {
        self.generate_improvement_proposals(investigation_id, actor)
    }

    /// Compare Proposal snapshots with inspectable deterministic factors.
    pub fn compare_improvement_proposals(
        &self,
        investigation_id: InvestigationId,
        proposal_ids: Vec<ObjectId>,
    ) -> RivoraResult<ProposalComparison> {
        self.store.load_investigation(&investigation_id)?;
        if proposal_ids.len() < 2 {
            return Err(RivoraError::validation(
                "at least two proposals are required for comparison",
            ));
        }
        let mut unique = proposal_ids;
        unique.sort_by_key(|id| id.to_string());
        unique.dedup();
        if unique.len() < 2 {
            return Err(RivoraError::validation(
                "at least two distinct proposals are required for comparison",
            ));
        }

        let mut ranked = Vec::new();
        for proposal_id in unique {
            let proposal = self.get_improvement_proposal(investigation_id, proposal_id)?;
            let historical_outcomes = self.historical_outcome_factor(&proposal)?;
            let factors = comparison_factors(&proposal, historical_outcomes);
            let score = factors.iter().map(|factor| factor.contribution).sum();
            let detail = factors
                .iter()
                .map(|factor| format!("{}={:.3}", factor.name, factor.contribution))
                .collect::<Vec<_>>()
                .join("; ");
            ranked.push(RankedProposal {
                proposal_id,
                rank: 0,
                score,
                factors,
                explanation: format!(
                    "Score {:.3} from inspectable factors: {}. This ranking is guidance, not an implementation decision.",
                    score, detail
                ),
            });
        }
        ranked.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.proposal_id.to_string().cmp(&b.proposal_id.to_string()))
        });
        for (index, item) in ranked.iter_mut().enumerate() {
            item.rank = (index + 1) as u32;
        }
        Ok(ProposalComparison {
            investigation_id,
            ranked,
            compared_at: Utc::now(),
            method: "proposal_comparison_v1".into(),
            explanation: "Alternatives are ranked by visible evidence, contradiction, impact, effort, architecture, reversibility, verification, and historical factors. The highest-ranked Proposal is preferred only for review and is not guaranteed correct, applied, implemented, or verified.".into(),
        })
    }

    /// Prioritize latest Proposals for an Investigation.
    pub fn prioritize_improvement_proposals(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<ProposalComparison> {
        let listing = self.list_improvement_proposals(investigation_id)?;
        self.compare_improvement_proposals(
            investigation_id,
            listing.proposals.iter().map(|p| p.id).collect(),
        )
    }

    /// Latest independently versioned alternative group for Composite execution.
    pub(crate) fn latest_proposal_alternative_group(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<Vec<ImprovementProposal>> {
        let listing = self.list_improvement_proposals(investigation_id)?;
        let latest = listing
            .proposals
            .iter()
            .filter(|proposal| proposal.alternative_group_id.is_some())
            .max_by(|left, right| {
                left.created_at
                    .cmp(&right.created_at)
                    .then_with(|| left.id.to_string().cmp(&right.id.to_string()))
            })
            .ok_or_else(|| {
                RivoraError::Precondition(
                    "no generated Proposal alternative group is available".into(),
                )
            })?;
        let group_id = latest.alternative_group_id;
        let mut proposals: Vec<_> = listing
            .proposals
            .into_iter()
            .filter(|proposal| proposal.alternative_group_id == group_id)
            .collect();
        proposals.sort_by_key(|proposal| proposal.id.to_string());
        if proposals.len() < 2 {
            return Err(RivoraError::Precondition(
                "latest Proposal opportunity does not contain two alternatives".into(),
            ));
        }
        Ok(proposals)
    }

    /// Return the concrete proposed Verification Plan without executing it.
    pub fn generate_proposal_verification_plan(
        &self,
        investigation_id: InvestigationId,
        proposal_id: ObjectId,
    ) -> RivoraResult<ProposalVerificationPlan> {
        Ok(self
            .get_improvement_proposal(investigation_id, proposal_id)?
            .verification_plan)
    }

    /// Return the bounded expected implementation outline without applying it.
    pub fn generate_proposal_implementation_outline(
        &self,
        investigation_id: InvestigationId,
        proposal_id: ObjectId,
    ) -> RivoraResult<Vec<String>> {
        Ok(self
            .get_improvement_proposal(investigation_id, proposal_id)?
            .implementation_outline)
    }

    /// Explain Proposal generation provenance and temporal evidence scope.
    pub fn explain_improvement_proposal_provenance(
        &self,
        investigation_id: InvestigationId,
        proposal_id: ObjectId,
    ) -> RivoraResult<String> {
        let proposal = self.get_improvement_proposal(investigation_id, proposal_id)?;
        let current = proposal
            .generation_inputs
            .iter()
            .filter(|e| e.scope == crate::domain::EvidenceScope::Current)
            .count();
        let historical = proposal.generation_inputs.len().saturating_sub(current);
        Ok(format!(
            "Proposal {} used {} current and {} labeled historical input(s) via {}. Unverified hypotheses remain assumptions. Proposal only — not applied, not implemented, not verified.",
            proposal.id, current, historical, proposal.derivation_method
        ))
    }

    /// Generate and durably store a sanitized Proposal artifact.
    pub fn generate_proposal_artifact(
        &self,
        investigation_id: InvestigationId,
        proposal_id: ObjectId,
        actor: impl Into<String>,
    ) -> RivoraResult<ProposalArtifact> {
        let actor = actor.into();
        if actor.trim().is_empty() {
            return Err(RivoraError::validation(
                "proposal artifact actor is required",
            ));
        }
        let proposal = self.get_improvement_proposal(investigation_id, proposal_id)?;
        let sanitized = sanitized_proposal(&proposal)?;
        let revision_listing = self
            .store
            .list_proposal_revisions(&investigation_id, &proposal.lineage_id)?;
        let revision_diagnostics = revision_listing
            .diagnostics
            .into_iter()
            .map(|diagnostic| ProposalStorageDiagnostic {
                path: sanitize_text(&diagnostic.path),
                error: sanitize_text(&diagnostic.error),
            })
            .collect::<Vec<_>>();
        let revisions = revision_listing
            .proposals
            .iter()
            .map(sanitized_proposal)
            .collect::<RivoraResult<Vec<_>>>()?;
        let artifact = ProposalArtifact {
            id: ObjectId::new(),
            investigation_id,
            proposal_id,
            markdown: render_proposal_markdown(&sanitized, &revisions, &revision_diagnostics),
            proposal: sanitized,
            revisions,
            revision_diagnostics,
            boundary: PROPOSAL_BOUNDARY.into(),
            generated_at: Utc::now(),
            provenance: Provenance::now(sanitize_text(actor.trim()), "runtime")
                .with_capability("generate_proposal_artifact")
                .with_evidence(vec![proposal_id]),
        };
        self.store.append_proposal_artifact(&artifact)?;
        Ok(artifact)
    }

    /// List durable Proposal artifacts after restart.
    pub fn list_proposal_artifacts(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<ProposalArtifactListing> {
        self.store.load_investigation(&investigation_id)?;
        self.store.list_proposal_artifacts(&investigation_id)
    }

    /// Generate bounded coding-agent handoff text without invoking an agent.
    pub fn generate_coding_agent_handoff(
        &self,
        investigation_id: InvestigationId,
        proposal_id: ObjectId,
    ) -> RivoraResult<String> {
        let proposal =
            sanitized_proposal(&self.get_improvement_proposal(investigation_id, proposal_id)?)?;
        Ok(render_coding_agent_handoff(&proposal))
    }

    /// Filter the latest Investigation-level Proposal portfolio.
    pub fn proposal_portfolio(
        &self,
        investigation_id: InvestigationId,
        filter: ProposalPortfolioFilter,
    ) -> RivoraResult<Vec<ImprovementProposal>> {
        let listing = self.list_improvement_proposals(investigation_id)?;
        let component = filter
            .affected_component
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_lowercase);
        Ok(listing
            .proposals
            .into_iter()
            .filter(|proposal| match filter.status {
                Some(value) => proposal.status == value,
                None => true,
            })
            .filter(|proposal| match filter.priority {
                Some(value) => proposal.priority == value,
                None => true,
            })
            .filter(|proposal| match filter.category {
                Some(value) => proposal.category == value,
                None => true,
            })
            .filter(|proposal| match filter.source_recommendation_id {
                Some(id) => proposal.source_recommendation_ids.contains(&id),
                None => true,
            })
            .filter(|proposal| match component.as_ref() {
                Some(value) => proposal
                    .affected_components
                    .iter()
                    .any(|candidate| candidate.to_lowercase() == *value),
                None => true,
            })
            .filter(|proposal| {
                !filter.unresolved_high_priority || is_unresolved_high_priority(proposal)
            })
            .collect())
    }

    /// Trace current Engineering Objects through a Proposal.
    pub fn trace_improvement_proposal(
        &self,
        investigation_id: InvestigationId,
        proposal_id: ObjectId,
    ) -> RivoraResult<ProposalTrace> {
        let proposal = self.get_improvement_proposal(investigation_id, proposal_id)?;
        let inputs: std::collections::HashSet<_> = proposal
            .generation_inputs
            .iter()
            .map(|reference| reference.object_id)
            .chain(proposal.supporting_evidence.iter().map(|r| r.object_id))
            .chain(proposal.contradicting_evidence.iter().map(|r| r.object_id))
            .collect();
        let mut observation_ids = retained_ids(
            self.store.list_observations(&investigation_id)?,
            &inputs,
            |item| item.id,
        );
        let mut memory_ids = retained_ids(
            self.store.list_memory(&investigation_id)?,
            &inputs,
            |item| item.id,
        );
        let mut knowledge_ids = retained_ids(
            self.store.list_knowledge(&investigation_id)?,
            &inputs,
            |item| item.id,
        );
        let mut evaluation_ids = retained_ids(
            self.store.list_evaluations(&investigation_id)?,
            &inputs,
            |item| item.id,
        );
        let mut verification_ids = retained_ids(
            self.store.list_verifications(&investigation_id)?,
            &inputs,
            |item| item.id,
        );
        let mut recommendation_ids = proposal.source_recommendation_ids.clone();
        dedupe_ids(&mut observation_ids);
        dedupe_ids(&mut memory_ids);
        dedupe_ids(&mut knowledge_ids);
        dedupe_ids(&mut evaluation_ids);
        dedupe_ids(&mut verification_ids);
        dedupe_ids(&mut recommendation_ids);
        Ok(ProposalTrace {
            investigation_id,
            observation_ids,
            memory_ids,
            knowledge_ids,
            evaluation_ids,
            verification_ids,
            recommendation_ids,
            proposal_id,
            external_implementation_reference: proposal
                .external_implementation_reference
                .as_deref()
                .map(sanitize_text),
            explanation: "Observation -> Memory -> Knowledge -> Evaluation -> Verification -> Recommendation -> Improvement Proposal. accepted does not mean implemented. Any manually supplied external implementation reference is inert metadata, not proof of implementation and not a verified outcome.".into(),
        })
    }

    /// Record an inert manually supplied external implementation reference as a new revision.
    pub fn record_external_implementation_reference(
        &self,
        investigation_id: InvestigationId,
        proposal_id: ObjectId,
        reference: impl Into<String>,
        actor: impl Into<String>,
    ) -> RivoraResult<ImprovementProposal> {
        let reference = reference.into();
        if reference.trim().is_empty() {
            return Err(RivoraError::validation(
                "external implementation reference is required",
            ));
        }
        let actor = actor.into();
        let proposal = self.get_latest_improvement_proposal(investigation_id, proposal_id)?;
        let mut next = proposal.revised(
            actor,
            "manually recorded external implementation reference",
            Utc::now(),
        )?;
        next.external_implementation_reference = Some(sanitize_text(reference.trim()));
        next.provenance.capability = Some("record_external_implementation_reference".into());
        self.persist_sanitized_proposal(next)
    }

    fn persist_sanitized_proposal(
        &self,
        proposal: ImprovementProposal,
    ) -> RivoraResult<ImprovementProposal> {
        let proposal = sanitized_proposal(&proposal)?;
        self.store.append_proposal(&proposal)?;
        Ok(proposal)
    }

    fn get_latest_improvement_proposal(
        &self,
        investigation_id: InvestigationId,
        proposal_id: ObjectId,
    ) -> RivoraResult<ImprovementProposal> {
        let proposal = self.get_improvement_proposal(investigation_id, proposal_id)?;
        let revisions = self
            .store
            .list_proposal_revisions(&investigation_id, &proposal.lineage_id)?
            .proposals;
        let maximum_revision = revisions
            .iter()
            .map(|revision| revision.revision_number)
            .max()
            .unwrap_or(proposal.revision_number);
        let heads: Vec<_> = revisions
            .iter()
            .filter(|revision| revision.revision_number == maximum_revision)
            .collect();
        if heads.len() != 1 || heads[0].id != proposal.id {
            return Err(RivoraError::Precondition(format!(
                "proposal operation requires the unique latest revision in lineage {}",
                proposal.lineage_id
            )));
        }
        Ok(proposal)
    }

    fn historical_outcome_factor(
        &self,
        proposal: &ImprovementProposal,
    ) -> RivoraResult<(f64, String)> {
        let selected: std::collections::HashSet<_> =
            proposal.learning_outcome_ids.iter().copied().collect();
        let mut investigations = proposal.related_investigation_ids.clone();
        investigations.sort_by_key(|id| id.to_string());
        investigations.dedup();
        let mut successful = 0usize;
        let mut unsuccessful = 0usize;
        let mut neutral = 0usize;
        for investigation_id in investigations {
            for outcome in self.store.list_learning(&investigation_id)? {
                if !selected.contains(&outcome.id) {
                    continue;
                }
                match outcome.disposition {
                    OutcomeDisposition::Successful => successful += 1,
                    OutcomeDisposition::Rejected | OutcomeDisposition::Unsuccessful => {
                        unsuccessful += 1;
                    }
                    OutcomeDisposition::Accepted | OutcomeDisposition::Ignored => neutral += 1,
                }
            }
        }
        let mut current_successful = 0usize;
        let mut current_unsuccessful = 0usize;
        let mut current_neutral = 0usize;
        for outcome in self.store.list_learning(&proposal.investigation_id)? {
            if !selected.contains(&outcome.id) {
                continue;
            }
            match outcome.disposition {
                OutcomeDisposition::Successful => current_successful += 1,
                OutcomeDisposition::Rejected | OutcomeDisposition::Unsuccessful => {
                    current_unsuccessful += 1;
                }
                OutcomeDisposition::Accepted | OutcomeDisposition::Ignored => current_neutral += 1,
            }
        }
        let raw = if successful > unsuccessful {
            0.8
        } else if unsuccessful > successful {
            0.3
        } else {
            0.5
        };
        Ok((
            raw,
            format!(
                "Labeled historical outcomes: {successful} successful, {unsuccessful} unsuccessful/rejected, {neutral} accepted/ignored. Current labeled outcomes: {current_successful} successful, {current_unsuccessful} unsuccessful/rejected, {current_neutral} accepted/ignored."
            ),
        ))
    }

    fn current_proposal_input_ids(
        &self,
        investigation_id: &InvestigationId,
    ) -> RivoraResult<std::collections::HashSet<ObjectId>> {
        let mut ids = Vec::new();
        ids.extend(
            self.store
                .list_observations(investigation_id)?
                .iter()
                .map(|item| item.id),
        );
        ids.extend(
            self.store
                .list_memory(investigation_id)?
                .iter()
                .map(|item| item.id),
        );
        ids.extend(
            self.store
                .list_knowledge(investigation_id)?
                .iter()
                .map(|item| item.id),
        );
        ids.extend(
            self.store
                .list_evaluations(investigation_id)?
                .iter()
                .map(|item| item.id),
        );
        ids.extend(
            self.store
                .list_verifications(investigation_id)?
                .iter()
                .map(|item| item.id),
        );
        ids.extend(
            self.store
                .list_hypotheses(investigation_id)?
                .iter()
                .map(|item| item.id),
        );
        ids.extend(
            self.store
                .list_recommendations(investigation_id)?
                .iter()
                .map(|item| item.id),
        );
        ids.extend(
            self.store
                .list_learning(investigation_id)?
                .iter()
                .map(|item| item.id),
        );
        ids.extend(
            self.store
                .list_recalled_context(investigation_id)?
                .iter()
                .filter(|item| item.state == RecalledContextState::Attached)
                .map(|item| item.id),
        );
        ids.extend(
            self.store
                .list_workflows(investigation_id)?
                .iter()
                .filter(|item| item.intent != "propose_engineering_improvement")
                .map(|item| item.id),
        );
        ids.extend(
            self.store
                .list_verification_suggestions(investigation_id)?
                .iter()
                .map(|item| item.id),
        );
        ids.extend(
            self.store
                .list_deployment_readiness(investigation_id)?
                .iter()
                .map(|item| item.id),
        );
        ids.extend(
            self.store
                .list_risk_forecasts(investigation_id)?
                .iter()
                .map(|item| item.id),
        );
        ids.extend(
            self.store
                .list_root_cause_guidance(investigation_id)?
                .iter()
                .map(|item| item.id),
        );
        ids.extend(
            self.store
                .list_engineering_reports(investigation_id)?
                .iter()
                .map(|item| item.id),
        );
        Ok(ids.into_iter().collect())
    }
}

const PROPOSAL_BOUNDARY: &str = "Proposal only — not applied, not implemented, not verified.";

fn ensure_content_revision_allowed(proposal: &ImprovementProposal) -> RivoraResult<()> {
    if matches!(
        proposal.status,
        ProposalStatus::Accepted
            | ProposalStatus::Rejected
            | ProposalStatus::Superseded
            | ProposalStatus::Withdrawn
    ) {
        return Err(RivoraError::Precondition(format!(
            "terminal {} proposal content cannot be revised; create a new proposal",
            proposal.status.as_str()
        )));
    }
    Ok(())
}

fn clean_strings(values: Vec<String>) -> Vec<String> {
    let mut values: Vec<_> = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect();
    values.sort();
    values.dedup();
    values
}

fn dedupe_ids(ids: &mut Vec<ObjectId>) {
    ids.sort_by_key(|id| id.to_string());
    ids.dedup();
}

fn scoped_refs(ids: &[ObjectId], scope: EvidenceScope) -> Vec<EvidenceReference> {
    ids.iter()
        .copied()
        .map(|object_id| EvidenceReference { object_id, scope })
        .collect()
}

fn dedupe_refs(references: &mut Vec<EvidenceReference>) {
    references.sort_by(|a, b| {
        a.scope
            .as_str()
            .cmp(b.scope.as_str())
            .then_with(|| a.object_id.to_string().cmp(&b.object_id.to_string()))
    });
    references.dedup();
}

fn infer_category(text: &str) -> ProposalCategory {
    let text = text.to_lowercase();
    if text.contains("security") || text.contains("credential") || text.contains("secret") {
        ProposalCategory::Security
    } else if text.contains("test") || text.contains("ci") {
        ProposalCategory::Testing
    } else if text.contains("config") || text.contains("schema") {
        ProposalCategory::Configuration
    } else if text.contains("timeout") || text.contains("fail") || text.contains("error") {
        ProposalCategory::Reliability
    } else if text.contains("performance") || text.contains("latency") {
        ProposalCategory::Performance
    } else if text.contains("documentation") || text.contains("docs") {
        ProposalCategory::Documentation
    } else {
        ProposalCategory::Code
    }
}

fn infer_priority(
    evaluations: &[crate::domain::Evaluation],
    observations: &[crate::domain::Observation],
    verifications: &[crate::domain::VerificationReceipt],
    readiness: &[crate::domain::DeploymentReadiness],
    risk_forecasts: &[crate::domain::RiskForecast],
) -> (ProposalPriority, String) {
    let severity = evaluations
        .iter()
        .map(|evaluation| evaluation.severity)
        .max_by_key(|severity| match severity {
            Severity::Info => 0,
            Severity::Low => 1,
            Severity::Medium => 2,
            Severity::High => 3,
            Severity::Critical => 4,
        });
    let recurring_failure = observations.iter().filter(|observation| {
        let text = observation.summary.to_lowercase();
        text.contains("fail") || text.contains("error") || text.contains("timeout")
    });
    let failure_count = recurring_failure.count();
    let verified_count = verifications
        .iter()
        .filter(|receipt| receipt.result == VerificationResult::Pass)
        .count();
    let blocker_count = readiness
        .iter()
        .map(|assessment| assessment.blockers.len())
        .sum::<usize>();
    let material_risk_count = risk_forecasts
        .iter()
        .flat_map(|forecast| &forecast.items)
        .filter(|risk| matches!(risk.severity, Severity::High | Severity::Critical))
        .count();
    let severity_points = match severity {
        Some(Severity::Critical) => 4,
        Some(Severity::High) => 3,
        Some(Severity::Medium) => 2,
        Some(Severity::Low) => 1,
        Some(Severity::Info) | None => 0,
    };
    let urgency_points = usize::from(matches!(
        severity,
        Some(Severity::High | Severity::Critical)
    ));
    let score = severity_points
        + failure_count.min(2)
        + usize::from(verified_count > 0)
        + blocker_count.min(2)
        + material_risk_count.min(2)
        + urgency_points
        + 1; // the minimal alternative is explicitly bounded and reversible
    let priority = match score {
        9.. => ProposalPriority::Critical,
        6..=8 => ProposalPriority::High,
        3..=5 => ProposalPriority::Medium,
        1..=2 => ProposalPriority::Low,
        _ => ProposalPriority::Exploratory,
    };
    (
        priority,
        format!(
            "Priority {} reflects current impact/severity ({}), {} recurrence signal(s), {} verified evidence receipt(s), {} blocked-work signal(s), {} material risk-reduction opportunity(ies), urgency {}, and the baseline's low-cost reversible scope; confidence alone did not set priority.",
            priority.as_str(),
            severity.map(|value| value.as_str()).unwrap_or("unknown"),
            failure_count,
            verified_count,
            blocker_count,
            material_risk_count,
            if urgency_points > 0 { "present" } else { "not established" },
        ),
    )
}

fn infer_components(observations: &[crate::domain::Observation]) -> Vec<String> {
    let mut components: Vec<String> = observations
        .iter()
        .filter_map(|observation| {
            observation
                .payload
                .get("component")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .collect();
    if components.is_empty() {
        components.push("runtime".into());
    }
    clean_strings(components)
}

fn infer_resources(observations: &[crate::domain::Observation]) -> Vec<String> {
    let mut resources = Vec::new();
    for observation in observations {
        for key in ["path", "file", "resource", "repository"] {
            if let Some(value) = observation
                .payload
                .get(key)
                .and_then(serde_json::Value::as_str)
            {
                resources.push(sanitize_text(value));
            }
        }
        if let Some(values) = observation
            .payload
            .get("files")
            .and_then(serde_json::Value::as_array)
        {
            resources.extend(
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(sanitize_text),
            );
        }
    }
    clean_strings(resources)
}

fn alternative_summaries() -> Vec<ProposalAlternative> {
    vec![
        ProposalAlternative {
            title: "Minimal targeted validation".into(),
            expected_benefit: "Addresses the observed failure with the smallest useful scope."
                .into(),
            effort: ProposalEffort::Small,
            implementation_risk: "low".into(),
            verification_complexity: "focused fixtures and regression checks".into(),
            reversibility: "localized validation can be reverted independently".into(),
            architectural_fit: "extends the existing Runtime boundary".into(),
            evidence_strength: "uses current verified and contradicting evidence".into(),
            drawbacks: vec!["May not prevent equivalent failures in sibling components.".into()],
        },
        ProposalAlternative {
            title: "Shared validation boundary".into(),
            expected_benefit: "Reduces recurrence across related components.".into(),
            effort: ProposalEffort::Medium,
            implementation_risk: "medium".into(),
            verification_complexity: "cross-component compatibility and regression checks".into(),
            reversibility: "requires coordinated rollback across consumers".into(),
            architectural_fit: "fits only if existing module boundaries support shared ownership"
                .into(),
            evidence_strength: "same evidence with a broader inferred scope".into(),
            drawbacks: vec![
                "Broader change surface.".into(),
                "Higher backward-compatibility risk.".into(),
            ],
        },
    ]
}

#[allow(clippy::too_many_arguments)]
fn populate_generated(
    proposal: &mut ImprovementProposal,
    group_id: ObjectId,
    priority_explanation: String,
    components: Vec<String>,
    resources: Vec<String>,
    supporting_evidence: Vec<EvidenceReference>,
    contradicting_evidence: Vec<EvidenceReference>,
    generation_inputs: Vec<EvidenceReference>,
    hypotheses: &[crate::domain::Hypothesis],
    evaluations: &[crate::domain::Evaluation],
    verifications: &[crate::domain::VerificationReceipt],
    recommendations: &[crate::domain::Recommendation],
    related_investigation_ids: Vec<InvestigationId>,
    learning_outcome_ids: Vec<ObjectId>,
    assumptions: &[String],
    alternatives: Vec<ProposalAlternative>,
    effort: ProposalEffort,
    derivation_method: &str,
    targeted: bool,
) {
    proposal.alternative_group_id = Some(group_id);
    proposal.priority_explanation = priority_explanation;
    proposal.expected_impact = if targeted {
        "Prevent the observed invalid input from crossing the affected Runtime boundary.".into()
    } else {
        "Reduce recurrence across consumers of a shared validation boundary.".into()
    };
    proposal.affected_components = components;
    proposal.affected_resources = resources;
    proposal.supporting_evidence = supporting_evidence;
    proposal.contradicting_evidence = contradicting_evidence;
    proposal.generation_inputs = generation_inputs;
    proposal.hypothesis_ids = hypotheses.iter().map(|item| item.id).collect();
    proposal.evaluation_ids = evaluations.iter().map(|item| item.id).collect();
    proposal.verification_ids = verifications.iter().map(|item| item.id).collect();
    proposal.source_recommendation_ids = recommendations.iter().map(|item| item.id).collect();
    proposal.related_investigation_ids = related_investigation_ids;
    proposal.learning_outcome_ids = learning_outcome_ids;
    proposal.assumptions = assumptions.to_vec();
    proposal.constraints = vec![
        "Preserve existing Runtime and Capability boundaries.".into(),
        "Do not mutate source Engineering Objects or external systems.".into(),
        "Treat suggested modules and resources as expected scope only.".into(),
    ];
    proposal.risks = vec![ProposalRisk {
        description: if targeted {
            "A narrow fix may leave equivalent sibling paths uncovered.".into()
        } else {
            "A shared abstraction may expand compatibility and migration risk.".into()
        },
        severity: if targeted {
            Severity::Low
        } else {
            Severity::Medium
        },
        mitigation: "Add focused boundary, compatibility, and regression verification.".into(),
    }];
    proposal.alternatives = alternatives;
    proposal.implementation_outline = if targeted {
        vec![
            "Inspect the current affected boundary and confirm the evidence references.".into(),
            "Add deterministic validation at the narrowest existing Runtime-owned boundary.".into(),
            "Expose failures through the existing structured error type.".into(),
            "Add malformed, boundary, and regression fixtures without changing connectors.".into(),
        ]
    } else {
        vec![
            "Confirm shared ownership and compatibility requirements across affected consumers."
                .into(),
            "Introduce one Runtime-owned validation abstraction behind existing interfaces.".into(),
            "Migrate consumers incrementally without destructive data migration.".into(),
            "Add cross-consumer compatibility, regression, and rollback coverage.".into(),
        ]
    };
    proposal.test_strategy = vec![
        "Add a focused failing fixture for the verified condition.".into(),
        "Add boundary and malformed-input cases.".into(),
        "Run architecture and prior-release regression suites.".into(),
    ];
    proposal.verification_plan = ProposalVerificationPlan {
        claims: vec!["The proposed validation prevents the observed failure condition.".into()],
        preconditions: vec![
            "Inspect current code and confirm the proposed boundary still exists.".into(),
        ],
        tests: proposal.test_strategy.clone(),
        checks: vec![
            "Run formatting, denied-warning lint, full tests, and release build.".into(),
            "Check backward compatibility and architecture boundaries.".into(),
        ],
        manual_workflows: vec![
            "Repeat the isolated failing workflow with valid and invalid fixtures.".into(),
        ],
        expected_evidence: vec![
            "A failing test before implementation and passing evidence afterward.".into(),
        ],
        success_criteria: vec![
            "Invalid input is rejected deterministically without external mutation.".into(),
        ],
        failure_criteria: vec![
            "Invalid input still crosses the boundary or valid input regresses.".into(),
        ],
        inconclusive_conditions: vec![
            "The original failure cannot be reproduced from current evidence.".into(),
        ],
        recovery_checks: vec!["Confirm the change can be reverted without data loss.".into()],
    };
    proposal.success_criteria = proposal.verification_plan.success_criteria.clone();
    proposal.reversibility = if targeted {
        "Localized and independently reversible; verify old behavior restoration in isolation."
            .into()
    } else {
        "Requires coordinated consumer rollback; preserve the prior boundary until verification completes.".into()
    };
    proposal.estimated_effort = effort;
    proposal.derivation_method = derivation_method.into();
}

fn comparison_factors(
    proposal: &ImprovementProposal,
    historical_outcomes: (f64, String),
) -> Vec<ProposalComparisonFactor> {
    let evidence = (proposal.supporting_evidence.len() as f64 / 8.0).clamp(0.1, 1.0);
    let contradiction = 1.0 / (1.0 + proposal.contradicting_evidence.len() as f64);
    let impact = match proposal.priority {
        ProposalPriority::Critical => 1.0,
        ProposalPriority::High => 0.85,
        ProposalPriority::Medium => 0.65,
        ProposalPriority::Low => 0.4,
        ProposalPriority::Exploratory => 0.25,
    };
    let effort = match proposal.estimated_effort {
        ProposalEffort::Small => 1.0,
        ProposalEffort::Medium => 0.65,
        ProposalEffort::Large => 0.35,
    };
    let architecture = match proposal.generation_method {
        ProposalGenerationMethod::Deterministic => 0.9,
        ProposalGenerationMethod::ModelAssisted => 0.7,
        ProposalGenerationMethod::Human => 0.5,
    };
    let reversibility = match proposal.estimated_effort {
        ProposalEffort::Small => 0.9,
        ProposalEffort::Medium => 0.65,
        ProposalEffort::Large => 0.35,
    };
    let verification = if proposal.verification_plan.tests.is_empty() {
        0.2
    } else {
        0.9
    };
    let (historical, historical_explanation) = historical_outcomes;
    [
        (
            "evidence_strength",
            0.20,
            evidence,
            format!(
                "{} supporting reference(s).",
                proposal.supporting_evidence.len()
            ),
        ),
        (
            "contradiction_level",
            0.12,
            contradiction,
            format!(
                "{} contradiction(s) remain visible.",
                proposal.contradicting_evidence.len()
            ),
        ),
        (
            "expected_impact",
            0.15,
            impact,
            proposal.priority_explanation.clone(),
        ),
        (
            "implementation_effort",
            0.15,
            effort,
            format!("Effort category is {}.", proposal.estimated_effort.as_str()),
        ),
        (
            "architectural_fit",
            0.10,
            architecture,
            format!(
                "Architectural fit is {:.1} for {} generation and remains reviewable.",
                architecture,
                proposal.generation_method.as_str()
            ),
        ),
        (
            "reversibility",
            0.10,
            reversibility,
            proposal.reversibility.clone(),
        ),
        (
            "verification_feasibility",
            0.10,
            verification,
            format!(
                "{} proposed verification test(s).",
                proposal.verification_plan.tests.len()
            ),
        ),
        (
            "historical_context",
            0.08,
            historical,
            historical_explanation,
        ),
    ]
    .into_iter()
    .map(
        |(name, weight, raw, explanation)| ProposalComparisonFactor {
            name: name.into(),
            weight,
            contribution: weight * raw,
            explanation,
        },
    )
    .collect()
}

fn sanitized_proposal(proposal: &ImprovementProposal) -> RivoraResult<ImprovementProposal> {
    let mut value = serde_json::to_value(proposal)
        .map_err(|error| RivoraError::serialization(error.to_string()))?;
    sanitize_json_strings(&mut value);
    serde_json::from_value(value).map_err(|error| RivoraError::serialization(error.to_string()))
}

fn sanitize_json_strings(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(text) => *text = sanitize_text(text),
        serde_json::Value::Array(values) => {
            for value in values {
                sanitize_json_strings(value);
            }
        }
        serde_json::Value::Object(values) => {
            for value in values.values_mut() {
                sanitize_json_strings(value);
            }
        }
        _ => {}
    }
}

fn render_proposal_markdown(
    proposal: &ImprovementProposal,
    revisions: &[ImprovementProposal],
    revision_diagnostics: &[ProposalStorageDiagnostic],
) -> String {
    use std::fmt::Write as _;

    let mut output = String::new();
    writeln!(output, "# Improvement Proposal: {}\n", proposal.title).ok();
    writeln!(output, "> {PROPOSAL_BOUNDARY}").ok();
    writeln!(
        output,
        "> Accepted means approved for possible external implementation; it does not mean implemented or verified.\n"
    )
    .ok();
    writeln!(output, "## Status and priority\n").ok();
    writeln!(output, "- Status: {}", proposal.status.as_str()).ok();
    writeln!(output, "- Priority: {}", proposal.priority.as_str()).ok();
    writeln!(output, "- Category: {}", proposal.category.as_str()).ok();
    writeln!(output, "- Confidence: {:.2}", proposal.confidence.value()).ok();
    writeln!(
        output,
        "- Estimated effort: {}",
        proposal.estimated_effort.as_str()
    )
    .ok();
    writeln!(output, "- Revision: {}\n", proposal.revision_number).ok();

    markdown_text_section(&mut output, "Summary", &proposal.summary);
    markdown_text_section(&mut output, "Problem statement", &proposal.rationale);
    markdown_text_section(&mut output, "Proposed change", &proposal.expected_impact);
    markdown_list_section(
        &mut output,
        "Affected components",
        &proposal.affected_components,
    );
    markdown_list_section(
        &mut output,
        "Likely files or resources (non-authoritative)",
        &proposal.affected_resources,
    );
    markdown_evidence_section(
        &mut output,
        "Supporting evidence",
        &proposal.supporting_evidence,
    );
    markdown_evidence_section(
        &mut output,
        "Contradicting evidence",
        &proposal.contradicting_evidence,
    );

    let history = proposal
        .related_investigation_ids
        .iter()
        .map(|id| format!("Historical Investigation `{id}`"))
        .chain(proposal.learning_outcome_ids.iter().map(|id| {
            let scope = proposal
                .generation_inputs
                .iter()
                .find(|reference| reference.object_id == *id)
                .map(|reference| reference.scope)
                .unwrap_or(EvidenceScope::Current);
            format!("{} Learning Outcome `{id}`", scope.as_str())
        }))
        .collect::<Vec<_>>();
    markdown_list_section(&mut output, "Historical context", &history);
    markdown_list_section(&mut output, "Assumptions", &proposal.assumptions);
    markdown_list_section(&mut output, "Constraints", &proposal.constraints);

    writeln!(output, "## Alternatives considered\n").ok();
    if proposal.alternatives.is_empty() {
        writeln!(output, "- None recorded.\n").ok();
    } else {
        for alternative in &proposal.alternatives {
            writeln!(output, "### {}\n", alternative.title).ok();
            writeln!(
                output,
                "- Expected benefit: {}",
                alternative.expected_benefit
            )
            .ok();
            writeln!(output, "- Effort: {}", alternative.effort.as_str()).ok();
            writeln!(
                output,
                "- Implementation risk: {}",
                alternative.implementation_risk
            )
            .ok();
            writeln!(
                output,
                "- Verification complexity: {}",
                alternative.verification_complexity
            )
            .ok();
            writeln!(output, "- Reversibility: {}", alternative.reversibility).ok();
            writeln!(
                output,
                "- Architectural fit: {}",
                alternative.architectural_fit
            )
            .ok();
            writeln!(
                output,
                "- Evidence strength: {}",
                alternative.evidence_strength
            )
            .ok();
            for drawback in &alternative.drawbacks {
                writeln!(output, "- Known drawback: {drawback}").ok();
            }
            writeln!(output).ok();
        }
    }

    markdown_list_section(
        &mut output,
        "Implementation outline",
        &proposal.implementation_outline,
    );
    markdown_list_section(&mut output, "Test strategy", &proposal.test_strategy);
    render_verification_plan(&mut output, &proposal.verification_plan);

    writeln!(output, "## Risks\n").ok();
    if proposal.risks.is_empty() {
        writeln!(output, "- None recorded.\n").ok();
    } else {
        for risk in &proposal.risks {
            writeln!(
                output,
                "- [{}] {} Mitigation: {}",
                risk.severity.as_str(),
                risk.description,
                risk.mitigation
            )
            .ok();
        }
        writeln!(output).ok();
    }
    markdown_list_section(&mut output, "Success criteria", &proposal.success_criteria);
    markdown_text_section(&mut output, "Expected impact", &proposal.expected_impact);
    markdown_text_section(&mut output, "Reversibility", &proposal.reversibility);
    markdown_list_section(
        &mut output,
        "Unresolved questions",
        &proposal.unresolved_questions,
    );

    writeln!(output, "## Provenance\n").ok();
    writeln!(output, "- Proposal ID: `{}`", proposal.id).ok();
    writeln!(
        output,
        "- Investigation ID: `{}`",
        proposal.investigation_id
    )
    .ok();
    writeln!(
        output,
        "- Generation method: {}",
        proposal.generation_method.as_str()
    )
    .ok();
    writeln!(
        output,
        "- Derivation method: {}",
        proposal.derivation_method
    )
    .ok();
    writeln!(output, "- Actor: {}", proposal.provenance.actor).ok();
    writeln!(output, "- Source: {}\n", proposal.provenance.source).ok();

    writeln!(output, "## Revision history\n").ok();
    if !revision_diagnostics.is_empty() {
        writeln!(
            output,
            "> Warning: {} corrupt revision record(s) were isolated; revision history may be incomplete.\n",
            revision_diagnostics.len()
        )
        .ok();
    }
    for (index, revision) in revisions.iter().enumerate() {
        let previous = index.checked_sub(1).and_then(|prior| revisions.get(prior));
        let previous_questions = previous.map_or(0, |item| item.unresolved_questions.len());
        let previous_transitions = previous.map_or(0, |item| item.transitions.len());
        let previous_feedback = previous.map_or(0, |item| item.feedback.len());
        writeln!(
            output,
            "### Revision {} — `{}`\n",
            revision.revision_number, revision.id
        )
        .ok();
        writeln!(output, "- Status: {}", revision.status.as_str()).ok();
        writeln!(output, "- Actor: {}", revision.provenance.actor).ok();
        writeln!(output, "- Updated at: {}", revision.updated_at.to_rfc3339()).ok();
        if let Some(parent) = revision.parent_proposal_id {
            writeln!(output, "- Parent snapshot: `{parent}`").ok();
        }
        for reason in revision
            .unresolved_questions
            .iter()
            .skip(previous_questions)
            .filter(|question| question.starts_with("Revision reason:"))
        {
            writeln!(output, "- {reason}").ok();
        }
        for transition in revision.transitions.iter().skip(previous_transitions) {
            writeln!(
                output,
                "- Transition: {} -> {} by {} at {} — {}",
                transition.from.as_str(),
                transition.to.as_str(),
                transition.actor,
                transition.at.to_rfc3339(),
                transition.reason
            )
            .ok();
        }
        for feedback in revision.feedback.iter().skip(previous_feedback) {
            writeln!(
                output,
                "- Feedback: {} by {} at {} — {}",
                feedback.category.as_str(),
                feedback.actor,
                feedback.at.to_rfc3339(),
                feedback.comment
            )
            .ok();
        }
        writeln!(output).ok();
    }
    output
}

fn markdown_text_section(output: &mut String, heading: &str, value: &str) {
    use std::fmt::Write as _;
    writeln!(output, "## {heading}\n").ok();
    if value.trim().is_empty() {
        writeln!(output, "Not specified.\n").ok();
    } else {
        writeln!(output, "{}\n", value.trim()).ok();
    }
}

fn markdown_list_section(output: &mut String, heading: &str, values: &[String]) {
    use std::fmt::Write as _;
    writeln!(output, "## {heading}\n").ok();
    if values.is_empty() {
        writeln!(output, "- None recorded.\n").ok();
    } else {
        for value in values {
            writeln!(output, "- {value}").ok();
        }
        writeln!(output).ok();
    }
}

fn markdown_evidence_section(output: &mut String, heading: &str, references: &[EvidenceReference]) {
    use std::fmt::Write as _;
    writeln!(output, "## {heading}\n").ok();
    if references.is_empty() {
        writeln!(output, "- None recorded.\n").ok();
    } else {
        for reference in references {
            writeln!(
                output,
                "- [{}] `{}`",
                reference.scope.as_str(),
                reference.object_id
            )
            .ok();
        }
        writeln!(output).ok();
    }
}

fn render_verification_plan(output: &mut String, plan: &ProposalVerificationPlan) {
    use std::fmt::Write as _;
    writeln!(output, "## Verification Plan\n").ok();
    for (label, values) in [
        ("Claims", &plan.claims),
        ("Preconditions", &plan.preconditions),
        ("Tests and fixtures", &plan.tests),
        ("Static and compatibility checks", &plan.checks),
        ("Manual workflows", &plan.manual_workflows),
        ("Expected evidence", &plan.expected_evidence),
        ("Success criteria", &plan.success_criteria),
        ("Failure criteria", &plan.failure_criteria),
        ("Inconclusive conditions", &plan.inconclusive_conditions),
        ("Recovery checks", &plan.recovery_checks),
    ] {
        writeln!(output, "### {label}\n").ok();
        if values.is_empty() {
            writeln!(output, "- None recorded.\n").ok();
        } else {
            for value in values {
                writeln!(output, "- {value}").ok();
            }
            writeln!(output).ok();
        }
    }
}

fn render_coding_agent_handoff(proposal: &ImprovementProposal) -> String {
    use std::fmt::Write as _;

    let mut output = String::new();
    writeln!(output, "# Coding-Agent Implementation Handoff\n").ok();
    writeln!(output, "> {PROPOSAL_BOUNDARY}\n").ok();
    writeln!(output, "This is an implementation proposal. Review repository state and current code before acting. Do not treat suggested files or implementation details as authoritative without inspecting the repository. Do not exceed the approved Proposal scope.\n").ok();
    writeln!(output, "## {}\n", proposal.title).ok();
    writeln!(output, "## Repository context\n").ok();
    writeln!(output, "- Investigation: `{}`", proposal.investigation_id).ok();
    writeln!(
        output,
        "- Proposal: `{}` revision {}",
        proposal.id, proposal.revision_number
    )
    .ok();
    writeln!(
        output,
        "- Affected subsystem: {}",
        proposal.affected_components.join(", ")
    )
    .ok();
    writeln!(output, "- Relevant RFCs: RFC-020 and RFC-021").ok();
    writeln!(
        output,
        "- Architectural invariants: all implementation decisions remain outside Rivora v0.4.\n"
    )
    .ok();
    markdown_text_section(
        &mut output,
        "Bounded implementation objective",
        &proposal.summary,
    );
    markdown_list_section(&mut output, "Out of scope", &[
        "Automatic application, repository editing, branch creation, commits, pull requests, deployment, infrastructure or external-system mutation.".into(),
        "Treating acceptance as implementation or a verified outcome.".into(),
        "Work beyond the approved Proposal lineage and revision.".into(),
    ]);
    markdown_list_section(
        &mut output,
        "Likely modules and files",
        &proposal.affected_resources,
    );
    markdown_list_section(
        &mut output,
        "Expected implementation outline",
        &proposal.implementation_outline,
    );
    markdown_list_section(
        &mut output,
        "Tests to write or update",
        &proposal.test_strategy,
    );
    render_verification_plan(&mut output, &proposal.verification_plan);
    markdown_list_section(
        &mut output,
        "Compatibility requirements",
        &proposal.constraints,
    );
    markdown_list_section(&mut output, "Safety boundaries", &[
        "This handoff generates text only and does not invoke a coding agent.".into(),
        "Suggested commands, snippets, and file paths are proposals and must not be executed automatically.".into(),
    ]);
    markdown_list_section(
        &mut output,
        "Acceptance criteria",
        &proposal.success_criteria,
    );
    output
}

fn is_unresolved_high_priority(proposal: &ImprovementProposal) -> bool {
    matches!(
        proposal.priority,
        ProposalPriority::Critical | ProposalPriority::High
    ) && matches!(
        proposal.status,
        ProposalStatus::Draft
            | ProposalStatus::Proposed
            | ProposalStatus::UnderReview
            | ProposalStatus::Deferred
    )
}

fn retained_ids<T>(
    values: Vec<T>,
    inputs: &std::collections::HashSet<ObjectId>,
    id: impl Fn(&T) -> ObjectId,
) -> Vec<ObjectId> {
    values
        .iter()
        .map(id)
        .filter(|candidate| inputs.contains(candidate))
        .collect()
}

fn sanitize_text(text: &str) -> String {
    let text_lower = text.to_lowercase();
    if contains_structured_secret(text, &text_lower) {
        return "[REDACTED]".into();
    }
    text.split_whitespace()
        .map(|token| {
            if token_looks_sensitive(token) {
                "[REDACTED]"
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn contains_structured_secret(text: &str, lower: &str) -> bool {
    if lower.contains("-----begin private key-----")
        || lower.contains("-----begin rsa private key-----")
        || lower.contains("-----begin openssh private key-----")
        || lower.contains("authorization:")
        || lower.contains("bearer ")
    {
        return true;
    }
    for key in [
        "token",
        "password",
        "secret",
        "api_key",
        "apikey",
        "credential",
        "authorization",
    ] {
        if lower.contains(&format!("\"{key}\":"))
            || lower.contains(&format!("'{key}':"))
            || lower.contains(&format!("{key}:"))
        {
            return true;
        }
    }
    let mut remainder = text;
    while let Some(scheme) = remainder.find("://") {
        let authority = &remainder[scheme + 3..];
        let authority = authority
            .split(['/', '?', '#', ' ', '\t', '\r', '\n'])
            .next()
            .unwrap_or_default();
        if authority
            .split_once('@')
            .is_some_and(|(userinfo, _)| userinfo.contains(':'))
        {
            return true;
        }
        remainder = &remainder[scheme + 3..];
    }
    false
}

fn token_looks_sensitive(token: &str) -> bool {
    let trimmed = token.trim_matches(|character: char| {
        matches!(
            character,
            '"' | '\'' | '`' | ',' | ';' | '(' | ')' | '[' | ']' | '{' | '}'
        )
    });
    let lower = trimmed.to_lowercase();
    if lower.starts_with("ghp_")
        || lower.starts_with("github_pat_")
        || lower.starts_with("sk_")
        || lower.starts_with("xoxb-")
        || lower.starts_with("xoxp-")
        || [
            "token=",
            "password=",
            "secret=",
            "api_key=",
            "apikey=",
            "credential=",
            "authorization=",
        ]
        .iter()
        .any(|marker| lower.contains(marker))
    {
        return true;
    }
    if trimmed.starts_with("AKIA")
        && trimmed.len() == 20
        && trimmed
            .chars()
            .all(|character| character.is_ascii_uppercase() || character.is_ascii_digit())
    {
        return true;
    }
    let jwt_parts: Vec<_> = trimmed.split('.').collect();
    if jwt_parts.len() == 3
        && jwt_parts[0].starts_with("eyJ")
        && jwt_parts.iter().all(|part| !part.is_empty())
    {
        return true;
    }
    trimmed.split_once('=').is_some_and(|(key, value)| {
        !value.is_empty()
            && key.len() >= 2
            && key
                .chars()
                .all(|character| character.is_ascii_uppercase() || character == '_')
    })
}
