//! Improvement Proposal lifecycle orchestration (RFC-020).

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::domain::{
    Confidence, ImprovementProposal, InvestigationId, ObjectId, ProposalCategory, ProposalFeedback,
    ProposalFeedbackCategory, ProposalGenerationMethod, ProposalListing, ProposalPriority,
    ProposalStatus, ProposalTransitionAuthority, Provenance,
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
