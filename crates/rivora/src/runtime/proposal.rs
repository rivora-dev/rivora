//! Improvement Proposal lifecycle orchestration (RFC-020).

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::domain::{
    Confidence, EvidenceReference, EvidenceScope, ImprovementProposal, InvestigationId, ObjectId,
    ProposalAlternative, ProposalCategory, ProposalComparison, ProposalComparisonFactor,
    ProposalEffort, ProposalFeedback, ProposalFeedbackCategory, ProposalGenerationMethod,
    ProposalListing, ProposalPriority, ProposalRisk, ProposalStatus, ProposalTransitionAuthority,
    ProposalVerificationPlan, Provenance, RankedProposal, RecalledContextState, Severity,
    VerificationResult,
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

impl Runtime {
    /// Create a human-requested concrete Proposal in Proposed state.
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
        proposal.status = ProposalStatus::Proposed;
        proposal.derivation_method = "explicit_caller_proposal_v1".into();
        self.store.append_proposal(&proposal)?;
        Ok(proposal)
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
        let proposal = self.get_improvement_proposal(investigation_id, proposal_id)?;
        let next = proposal.transitioned(status, actor, reason, Utc::now(), authority)?;
        self.store.append_proposal(&next)?;
        Ok(next)
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
        let proposal = self.get_improvement_proposal(investigation_id, proposal_id)?;
        let at = Utc::now();
        let mut next = proposal.revised(actor.trim(), "feedback attached", at)?;
        next.feedback.push(ProposalFeedback {
            category,
            comment: comment.trim().into(),
            actor: actor.trim().into(),
            at,
        });
        self.store.append_proposal(&next)?;
        Ok(next)
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
        let proposal = self.get_improvement_proposal(investigation_id, proposal_id)?;
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
        self.store.append_proposal(&next)?;
        Ok(next)
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
        let _replacement = self.get_improvement_proposal(investigation_id, replacement_id)?;
        let proposal = self.get_improvement_proposal(investigation_id, proposal_id)?;
        let mut next = proposal.transitioned(
            ProposalStatus::Superseded,
            actor,
            reason,
            Utc::now(),
            ProposalTransitionAuthority::ExternalCaller,
        )?;
        next.superseding_proposal_id = Some(replacement_id);
        self.store.append_proposal(&next)?;
        Ok(next)
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
        let workflows = self.store.list_workflows(&investigation_id)?;
        let contexts: Vec<_> = self
            .store
            .list_recalled_context(&investigation_id)?
            .into_iter()
            .filter(|context| context.state == RecalledContextState::Attached)
            .collect();

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
        dedupe_ids(&mut current_ids);

        let mut supporting_ids = current_ids.clone();
        supporting_ids.extend(
            hypotheses
                .iter()
                .flat_map(|hypothesis| hypothesis.supporting_evidence.iter().copied()),
        );
        supporting_ids.extend(
            verifications
                .iter()
                .flat_map(|receipt| receipt.evidence_ids.iter().copied()),
        );
        dedupe_ids(&mut supporting_ids);

        let mut contradicting_ids: Vec<_> = hypotheses
            .iter()
            .flat_map(|hypothesis| hypothesis.contradicting_evidence.iter().copied())
            .chain(
                verifications
                    .iter()
                    .flat_map(|receipt| receipt.conflicting_ids.iter().copied()),
            )
            .collect();
        dedupe_ids(&mut contradicting_ids);

        let historical_ids: Vec<_> = contexts
            .iter()
            .flat_map(|context| context.source_object_ids.iter().copied())
            .collect();
        let related_investigation_ids = contexts
            .iter()
            .map(|context| context.source_investigation_id)
            .collect::<Vec<_>>();

        let mut current_input_ids = current_ids.clone();
        current_input_ids.extend(supporting_ids.iter().copied());
        current_input_ids.extend(contradicting_ids.iter().copied());
        dedupe_ids(&mut current_input_ids);
        let mut generation_inputs = scoped_refs(&current_input_ids, EvidenceScope::Current);
        generation_inputs.extend(scoped_refs(&historical_ids, EvidenceScope::Historical));
        dedupe_refs(&mut generation_inputs);
        let supporting_evidence = scoped_refs(&supporting_ids, EvidenceScope::Current);
        let contradicting_evidence = scoped_refs(&contradicting_ids, EvidenceScope::Current);

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
        let (priority, priority_explanation) = infer_priority(&evaluations, &observations);
        let subject = recommendations
            .last()
            .map(|recommendation| recommendation.summary.clone())
            .unwrap_or_else(|| investigation.title.clone());
        let confidence = if verifications
            .iter()
            .any(|receipt| receipt.result == VerificationResult::Pass)
        {
            Confidence::new(0.8)
        } else if verifications.is_empty() {
            Confidence::new(0.55)
        } else {
            Confidence::new(0.65)
        };
        let components = infer_components(&observations);
        let assumptions: Vec<_> = hypotheses
            .iter()
            .filter(|hypothesis| hypothesis.verification_summary.contains("unverified"))
            .map(|hypothesis| format!("Unverified hypothesis: {}", hypothesis.statement))
            .collect();
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
            supporting_evidence.clone(),
            contradicting_evidence.clone(),
            generation_inputs.clone(),
            &hypotheses,
            &evaluations,
            &verifications,
            &recommendations,
            related_investigation_ids.clone(),
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
            supporting_evidence,
            contradicting_evidence,
            generation_inputs,
            &hypotheses,
            &evaluations,
            &verifications,
            &recommendations,
            related_investigation_ids,
            &assumptions,
            alternatives,
            ProposalEffort::Medium,
            "shared_validation_proposal_v1",
            false,
        );

        self.store.append_proposal(&targeted)?;
        self.store.append_proposal(&shared)?;
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
            let factors = comparison_factors(&proposal);
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
    let priority = match severity {
        Some(Severity::Critical) => ProposalPriority::Critical,
        Some(Severity::High) => ProposalPriority::High,
        Some(Severity::Medium) if failure_count > 0 => ProposalPriority::High,
        Some(Severity::Medium) => ProposalPriority::Medium,
        Some(Severity::Low | Severity::Info) if failure_count > 1 => ProposalPriority::Medium,
        Some(Severity::Low | Severity::Info) => ProposalPriority::Low,
        None if failure_count > 0 => ProposalPriority::Medium,
        None => ProposalPriority::Exploratory,
    };
    (
        priority,
        format!(
            "Priority {} reflects the highest current Evaluation severity ({}) and {} explicit failure signal(s); confidence alone did not set priority.",
            priority.as_str(),
            severity.map(|value| value.as_str()).unwrap_or("unknown"),
            failure_count
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
    supporting_evidence: Vec<EvidenceReference>,
    contradicting_evidence: Vec<EvidenceReference>,
    generation_inputs: Vec<EvidenceReference>,
    hypotheses: &[crate::domain::Hypothesis],
    evaluations: &[crate::domain::Evaluation],
    verifications: &[crate::domain::VerificationReceipt],
    recommendations: &[crate::domain::Recommendation],
    related_investigation_ids: Vec<InvestigationId>,
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
    proposal.supporting_evidence = supporting_evidence;
    proposal.contradicting_evidence = contradicting_evidence;
    proposal.generation_inputs = generation_inputs;
    proposal.hypothesis_ids = hypotheses.iter().map(|item| item.id).collect();
    proposal.evaluation_ids = evaluations.iter().map(|item| item.id).collect();
    proposal.verification_ids = verifications.iter().map(|item| item.id).collect();
    proposal.source_recommendation_ids = recommendations.iter().map(|item| item.id).collect();
    proposal.related_investigation_ids = related_investigation_ids;
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

fn comparison_factors(proposal: &ImprovementProposal) -> Vec<ProposalComparisonFactor> {
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
    let architecture = if proposal
        .constraints
        .iter()
        .any(|constraint| constraint.contains("Runtime and Capability"))
    {
        0.9
    } else {
        0.5
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
    let historical = if proposal
        .generation_inputs
        .iter()
        .any(|input| input.scope == EvidenceScope::Historical)
    {
        0.7
    } else {
        0.5
    };
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
            "Preserves the declared Runtime and Capability boundary.".into(),
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
            "Historical inputs are labeled and never treated as current proof.".into(),
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

fn sanitize_text(text: &str) -> String {
    text.split_whitespace()
        .map(|token| {
            let lower = token.to_lowercase();
            if lower.starts_with("ghp_")
                || lower.starts_with("sk_")
                || lower.contains("token=")
                || lower.contains("password=")
                || lower.contains("secret=")
            {
                "[REDACTED]"
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
