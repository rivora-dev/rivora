//! Implementation Records, Measured Learning Outcomes, and Learning Patterns
//! (RFC-022, RFC-023, RFC-024).
//!
//! This module records external implementation, evaluates measured outcomes
//! deterministically, and derives historical learning patterns. It never
//! applies changes, mutates external systems, or invokes coding agents.

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::domain::{
    CausalLanguage, Confidence, ConfidenceBreakdown, ConfidenceComponent, ConfidencePenalty,
    ExpectedResultAssessment, ExpectedResultKind, ExpectedResultSpec, HistoricalInfluenceExplanation,
    HistoricalPatternInfluence, ImplementationListing, ImplementationRecord,
    ImplementationReference, ImplementationSource, ImplementationStatus, ImprovementProposal,
    InvestigationId, LearningPattern, MaterialitySeverity, MeasuredLearningOutcome,
    MeasuredOutcomeListing, MeasuredOutcomeStatus, ObjectId, OutcomeClassification,
    OutcomeEvaluationReport, OutcomeEvidenceLink, OutcomeEvidenceRelation, PatternStatus,
    Provenance, ResultAssessmentKind,
};
use crate::error::{RivoraError, RivoraResult};
use crate::runtime::Runtime;

/// Request to record an external implementation for a Proposal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordImplementationRequest {
    /// Typed implementation source.
    pub source: ImplementationSource,
    /// Human-readable summary.
    pub summary: String,
    /// Typed implementation references.
    #[serde(default)]
    pub references: Vec<ImplementationReference>,
    /// Optional implementation timestamp.
    pub implemented_at: Option<chrono::DateTime<Utc>>,
    /// Declared observed files.
    #[serde(default)]
    pub observed_files: Vec<String>,
    /// Declared observed components.
    #[serde(default)]
    pub observed_components: Vec<String>,
    /// Declared scope description.
    #[serde(default)]
    pub declared_scope: String,
}

/// Content revision request for an Implementation Record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ReviseImplementationRequest {
    /// Replacement summary when present.
    pub summary: Option<String>,
    /// Replacement references when present.
    pub references: Option<Vec<ImplementationReference>>,
    /// Replacement observed files when present.
    pub observed_files: Option<Vec<String>>,
    /// Replacement observed components when present.
    pub observed_components: Option<Vec<String>>,
    /// Replacement declared scope when present.
    pub declared_scope: Option<String>,
    /// Replacement implemented_at when present.
    pub implemented_at: Option<Option<chrono::DateTime<Utc>>>,
}

/// Request to collect evidence on a Measured Learning Outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectOutcomeEvidenceRequest {
    /// Linked Engineering Object identifier.
    pub object_id: ObjectId,
    /// Relationship to the Outcome.
    pub relation: OutcomeEvidenceRelation,
    /// Optional related expected result.
    pub expected_result_id: Option<ObjectId>,
    /// Optional reason (required for dismissal).
    pub reason: Option<String>,
}

/// Content revision request for a Measured Learning Outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ReviseMeasuredOutcomeRequest {
    /// Additional observed results.
    pub observed_results: Option<Vec<crate::domain::ObservedResultSummary>>,
    /// Additional unresolved questions.
    pub unresolved_questions: Option<Vec<String>>,
    /// Additional recommended follow-up.
    pub recommended_follow_up: Option<Vec<String>>,
    /// Override causal language when present.
    pub causal_language: Option<CausalLanguage>,
}

/// Trace from Proposal through Implementation to Measured Learning Outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeasuredOutcomeTrace {
    /// Owning Investigation.
    pub investigation_id: InvestigationId,
    /// Proposal snapshot.
    pub proposal_id: ObjectId,
    /// Implementation Record snapshot.
    pub implementation_record_id: ObjectId,
    /// Measured Learning Outcome snapshot.
    pub outcome_id: ObjectId,
    /// Classification.
    pub classification: OutcomeClassification,
    /// Lifecycle status.
    pub status: MeasuredOutcomeStatus,
    /// Boundary explanation.
    pub explanation: String,
}

impl Runtime {
    // -----------------------------------------------------------------------
    // Implementation Records
    // -----------------------------------------------------------------------

    /// Record that external work associated with a Proposal was performed.
    ///
    /// Does not prove success. Multiple implementations per Proposal are allowed.
    pub fn record_external_implementation(
        &self,
        investigation_id: InvestigationId,
        proposal_id: ObjectId,
        request: RecordImplementationRequest,
        actor: impl Into<String>,
    ) -> RivoraResult<ImplementationRecord> {
        let actor = actor.into();
        if actor.trim().is_empty() {
            return Err(RivoraError::validation(
                "implementation reporter actor is required",
            ));
        }
        self.store.load_investigation(&investigation_id)?;
        let proposal = self.load_proposal_snapshot(investigation_id, proposal_id)?;
        let mut record = ImplementationRecord::reported(
            investigation_id,
            proposal.id,
            proposal.lineage_id,
            proposal.revision_number,
            actor.trim(),
            request.source,
            request.summary,
            Provenance::now(actor.trim(), "runtime")
                .with_capability("record_external_implementation")
                .with_evidence(vec![proposal.id]),
        )?;
        record.references = request.references;
        record.implemented_at = request.implemented_at;
        record.observed_files = request.observed_files;
        record.observed_components = request.observed_components;
        record.declared_scope = request.declared_scope;
        self.store.append_implementation_record(&record)?;
        Ok(record)
    }

    /// Create an immutable content revision of an Implementation Record.
    pub fn revise_implementation_record(
        &self,
        investigation_id: InvestigationId,
        record_id: ObjectId,
        request: ReviseImplementationRequest,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<ImplementationRecord> {
        let actor = actor.into();
        let reason = reason.into();
        let current = self.get_latest_implementation(investigation_id, record_id)?;
        let mut next = current.revised(&actor, &reason, Utc::now())?;
        if let Some(summary) = request.summary {
            if summary.trim().is_empty() {
                return Err(RivoraError::validation(
                    "implementation summary cannot be empty",
                ));
            }
            next.summary = summary.trim().into();
        }
        if let Some(references) = request.references {
            next.references = references;
        }
        if let Some(files) = request.observed_files {
            next.observed_files = files;
        }
        if let Some(components) = request.observed_components {
            next.observed_components = components;
        }
        if let Some(scope) = request.declared_scope {
            next.declared_scope = scope;
        }
        if let Some(implemented_at) = request.implemented_at {
            next.implemented_at = implemented_at;
        }
        next.provenance.capability = Some("revise_implementation_record".into());
        self.store.append_implementation_record(&next)?;
        Ok(next)
    }

    /// Link evidence object identifiers to an Implementation Record.
    pub fn link_implementation_evidence(
        &self,
        investigation_id: InvestigationId,
        record_id: ObjectId,
        evidence_ids: Vec<ObjectId>,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<ImplementationRecord> {
        if evidence_ids.is_empty() {
            return Err(RivoraError::validation(
                "at least one evidence id is required",
            ));
        }
        let actor = actor.into();
        let reason = reason.into();
        let current = self.get_latest_implementation(investigation_id, record_id)?;
        let mut next = if current.status == ImplementationStatus::Reported {
            current.transitioned(
                ImplementationStatus::EvidenceLinked,
                &actor,
                &reason,
                Utc::now(),
            )?
        } else {
            current.revised(&actor, &reason, Utc::now())?
        };
        for id in evidence_ids {
            if !next.evidence_ids.contains(&id) {
                next.evidence_ids.push(id);
            }
        }
        next.provenance.capability = Some("link_implementation_evidence".into());
        self.store.append_implementation_record(&next)?;
        Ok(next)
    }

    /// Mark an Implementation Record ready for Measured Learning Outcome evaluation.
    pub fn mark_implementation_ready(
        &self,
        investigation_id: InvestigationId,
        record_id: ObjectId,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<ImplementationRecord> {
        let actor = actor.into();
        let reason = reason.into();
        let current = self.get_latest_implementation(investigation_id, record_id)?;
        let next = match current.status {
            ImplementationStatus::Reported | ImplementationStatus::EvidenceLinked => current
                .transitioned(
                    ImplementationStatus::ReadyForEvaluation,
                    &actor,
                    &reason,
                    Utc::now(),
                )?,
            ImplementationStatus::ReadyForEvaluation => {
                return Err(RivoraError::validation(
                    "implementation is already ready for evaluation",
                ));
            }
            other => {
                return Err(RivoraError::validation(format!(
                    "cannot mark implementation ready from status {}",
                    other.as_str()
                )));
            }
        };
        self.store.append_implementation_record(&next)?;
        Ok(next)
    }

    /// Withdraw an Implementation Record with an explicit reason.
    pub fn withdraw_implementation(
        &self,
        investigation_id: InvestigationId,
        record_id: ObjectId,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<ImplementationRecord> {
        let actor = actor.into();
        let reason = reason.into();
        let current = self.get_latest_implementation(investigation_id, record_id)?;
        let next = current.transitioned(
            ImplementationStatus::Withdrawn,
            &actor,
            &reason,
            Utc::now(),
        )?;
        self.store.append_implementation_record(&next)?;
        Ok(next)
    }

    /// Supersede an Implementation Record with an explicit successor.
    pub fn supersede_implementation(
        &self,
        investigation_id: InvestigationId,
        record_id: ObjectId,
        successor_id: ObjectId,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<ImplementationRecord> {
        let actor = actor.into();
        let reason = reason.into();
        let current = self.get_latest_implementation(investigation_id, record_id)?;
        let successor = self
            .store
            .load_implementation_record(&investigation_id, &successor_id)?;
        if successor.investigation_id != investigation_id {
            return Err(RivoraError::validation(
                "successor implementation must belong to the same Investigation",
            ));
        }
        if successor.lineage_id == current.lineage_id {
            return Err(RivoraError::validation(
                "successor must be a different implementation lineage",
            ));
        }
        let mut next = current.transitioned(
            ImplementationStatus::Superseded,
            &actor,
            &reason,
            Utc::now(),
        )?;
        next.superseding_record_id = Some(successor.id);
        self.store.append_implementation_record(&next)?;
        Ok(next)
    }

    /// List Implementation Records for an Investigation.
    pub fn list_implementation_records(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<ImplementationListing> {
        self.store.load_investigation(&investigation_id)?;
        self.store.list_implementation_records(&investigation_id)
    }

    /// Load one Implementation Record snapshot.
    pub fn get_implementation_record(
        &self,
        investigation_id: InvestigationId,
        record_id: ObjectId,
    ) -> RivoraResult<ImplementationRecord> {
        self.store
            .load_implementation_record(&investigation_id, &record_id)
    }

    /// List all revisions in an Implementation Record lineage.
    pub fn list_implementation_revisions(
        &self,
        investigation_id: InvestigationId,
        lineage_id: ObjectId,
    ) -> RivoraResult<ImplementationListing> {
        self.store
            .list_implementation_revisions(&investigation_id, &lineage_id)
    }

    // -----------------------------------------------------------------------
    // Measured Learning Outcomes
    // -----------------------------------------------------------------------

    /// Create a Draft Measured Learning Outcome from a Proposal and Implementation.
    ///
    /// Seeds expected results from Proposal success criteria and verification plan.
    pub fn create_measured_learning_outcome(
        &self,
        investigation_id: InvestigationId,
        proposal_id: ObjectId,
        implementation_record_id: ObjectId,
        actor: impl Into<String>,
    ) -> RivoraResult<MeasuredLearningOutcome> {
        let actor = actor.into();
        if actor.trim().is_empty() {
            return Err(RivoraError::validation(
                "measured outcome creator actor is required",
            ));
        }
        self.store.load_investigation(&investigation_id)?;
        let proposal = self.load_proposal_snapshot(investigation_id, proposal_id)?;
        let implementation = self
            .store
            .load_implementation_record(&investigation_id, &implementation_record_id)?;
        if implementation.investigation_id != investigation_id {
            return Err(RivoraError::validation(
                "implementation record does not belong to the Investigation",
            ));
        }
        if implementation.proposal_lineage_id != proposal.lineage_id {
            return Err(RivoraError::validation(
                "implementation record is not linked to the same Proposal lineage",
            ));
        }
        if matches!(
            implementation.status,
            ImplementationStatus::Withdrawn | ImplementationStatus::Superseded
        ) {
            return Err(RivoraError::validation(
                "cannot create outcome for withdrawn or superseded implementation",
            ));
        }
        let expected = seed_expected_results(&proposal);
        if expected.is_empty() {
            return Err(RivoraError::validation(
                "proposal has no success criteria or verification plan success criteria to seed expected results",
            ));
        }
        let outcome = MeasuredLearningOutcome::draft(
            investigation_id,
            proposal.id,
            proposal.lineage_id,
            proposal.revision_number,
            implementation.id,
            implementation.lineage_id,
            expected,
            Provenance::now(actor.trim(), "runtime")
                .with_capability("create_measured_learning_outcome")
                .with_evidence(vec![proposal.id, implementation.id]),
        )?;
        self.store.append_measured_learning_outcome(&outcome)?;
        Ok(outcome)
    }

    /// Collect typed outcome evidence on a Measured Learning Outcome.
    pub fn collect_outcome_evidence(
        &self,
        investigation_id: InvestigationId,
        outcome_id: ObjectId,
        request: CollectOutcomeEvidenceRequest,
        actor: impl Into<String>,
    ) -> RivoraResult<MeasuredLearningOutcome> {
        let actor = actor.into();
        if actor.trim().is_empty() {
            return Err(RivoraError::validation(
                "outcome evidence actor is required",
            ));
        }
        if request.relation == OutcomeEvidenceRelation::IsDismissed {
            let reason = request.reason.as_deref().unwrap_or("").trim();
            if reason.is_empty() {
                return Err(RivoraError::validation(
                    "dismissal requires a non-empty reason",
                ));
            }
        }
        let current = self.get_latest_measured_outcome(investigation_id, outcome_id)?;
        let mut next = if current.status == MeasuredOutcomeStatus::Draft {
            current.transitioned(
                MeasuredOutcomeStatus::EvidenceCollection,
                &actor,
                "collecting outcome evidence",
                Utc::now(),
            )?
        } else {
            current.revised(&actor, "collecting outcome evidence", Utc::now())?
        };
        next.evidence_links.push(OutcomeEvidenceLink {
            object_id: request.object_id,
            relation: request.relation,
            expected_result_id: request.expected_result_id,
            reason: request.reason.map(|s| s.trim().to_string()),
            linked_at: Utc::now(),
            actor: actor.trim().into(),
        });
        next.provenance.capability = Some("collect_outcome_evidence".into());
        self.store.append_measured_learning_outcome(&next)?;
        Ok(next)
    }

    /// Create an immutable content revision of a Measured Learning Outcome.
    pub fn revise_measured_learning_outcome(
        &self,
        investigation_id: InvestigationId,
        outcome_id: ObjectId,
        request: ReviseMeasuredOutcomeRequest,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<MeasuredLearningOutcome> {
        let actor = actor.into();
        let reason = reason.into();
        let current = self.get_latest_measured_outcome(investigation_id, outcome_id)?;
        let mut next = current.revised(&actor, &reason, Utc::now())?;
        if let Some(observed) = request.observed_results {
            next.observed_results.extend(observed);
        }
        if let Some(questions) = request.unresolved_questions {
            next.unresolved_questions.extend(questions);
        }
        if let Some(follow_up) = request.recommended_follow_up {
            next.recommended_follow_up.extend(follow_up);
        }
        if let Some(language) = request.causal_language {
            next.causal_language = language;
        }
        next.provenance.capability = Some("revise_measured_learning_outcome".into());
        self.store.append_measured_learning_outcome(&next)?;
        Ok(next)
    }

    /// Transition a Measured Learning Outcome lifecycle state.
    pub fn transition_measured_learning_outcome(
        &self,
        investigation_id: InvestigationId,
        outcome_id: ObjectId,
        to: MeasuredOutcomeStatus,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<MeasuredLearningOutcome> {
        let actor = actor.into();
        let reason = reason.into();
        let current = self.get_latest_measured_outcome(investigation_id, outcome_id)?;
        let next = current.transitioned(to, &actor, &reason, Utc::now())?;
        self.store.append_measured_learning_outcome(&next)?;
        Ok(next)
    }

    /// Withdraw a Measured Learning Outcome.
    pub fn withdraw_measured_learning_outcome(
        &self,
        investigation_id: InvestigationId,
        outcome_id: ObjectId,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<MeasuredLearningOutcome> {
        self.transition_measured_learning_outcome(
            investigation_id,
            outcome_id,
            MeasuredOutcomeStatus::Withdrawn,
            actor,
            reason,
        )
    }

    /// Supersede a Measured Learning Outcome with an explicit successor.
    pub fn supersede_measured_learning_outcome(
        &self,
        investigation_id: InvestigationId,
        outcome_id: ObjectId,
        successor_id: ObjectId,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<MeasuredLearningOutcome> {
        let actor = actor.into();
        let reason = reason.into();
        let current = self.get_latest_measured_outcome(investigation_id, outcome_id)?;
        let successor = self
            .store
            .load_measured_learning_outcome(&investigation_id, &successor_id)?;
        if successor.lineage_id == current.lineage_id {
            return Err(RivoraError::validation(
                "successor must be a different measured outcome lineage",
            ));
        }
        let mut next = current.transitioned(
            MeasuredOutcomeStatus::Superseded,
            &actor,
            &reason,
            Utc::now(),
        )?;
        next.superseding_outcome_id = Some(successor.id);
        self.store.append_measured_learning_outcome(&next)?;
        Ok(next)
    }

    /// Deterministically evaluate a Measured Learning Outcome (RFC-023).
    ///
    /// Moves the Outcome to `Evaluated` (or keeps `UnderEvaluation` when blocked)
    /// with a full evaluation report stored on the revision.
    pub fn evaluate_measured_learning_outcome(
        &self,
        investigation_id: InvestigationId,
        outcome_id: ObjectId,
        actor: impl Into<String>,
    ) -> RivoraResult<MeasuredLearningOutcome> {
        let actor = actor.into();
        if actor.trim().is_empty() {
            return Err(RivoraError::validation("evaluation actor is required"));
        }
        let current = self.get_latest_measured_outcome(investigation_id, outcome_id)?;
        if matches!(
            current.status,
            MeasuredOutcomeStatus::Verified
                | MeasuredOutcomeStatus::Archived
                | MeasuredOutcomeStatus::Withdrawn
                | MeasuredOutcomeStatus::Superseded
        ) {
            return Err(RivoraError::validation(format!(
                "cannot evaluate measured outcome in status {}",
                current.status.as_str()
            )));
        }

        // Evaluate against the exact implementation snapshot linked at creation.
        // Fall back to latest revision in that lineage if the snapshot is missing.
        let implementation = match self.store.load_implementation_record(
            &investigation_id,
            &current.implementation_record_id,
        ) {
            Ok(record) => record,
            Err(RivoraError::ObjectNotFound(_)) => {
                let listing = self.store.list_implementation_revisions(
                    &investigation_id,
                    &current.implementation_lineage_id,
                )?;
                listing
                    .records
                    .into_iter()
                    .max_by_key(|r| r.revision_number)
                    .ok_or(RivoraError::ObjectNotFound(current.implementation_record_id))?
            }
            Err(error) => return Err(error),
        };

        let evaluation = evaluate_outcome_deterministic(&current, &implementation);
        let now = Utc::now();
        let reason = if evaluation.blocked {
            "partial evaluation; remaining blockers"
        } else {
            "deterministic evaluation complete"
        };
        let mut next = current.revised(&actor, reason, now)?;

        if evaluation.blocked {
            next.status = MeasuredOutcomeStatus::UnderEvaluation;
            if current.status != MeasuredOutcomeStatus::UnderEvaluation {
                next.transitions
                    .push(crate::domain::MeasuredOutcomeTransition {
                        from: current.status,
                        to: MeasuredOutcomeStatus::UnderEvaluation,
                        actor: actor.trim().into(),
                        reason: reason.into(),
                        at: now,
                    });
            }
        } else {
            next.status = MeasuredOutcomeStatus::Evaluated;
            if current.status != MeasuredOutcomeStatus::Evaluated {
                next.transitions
                    .push(crate::domain::MeasuredOutcomeTransition {
                        from: current.status,
                        to: MeasuredOutcomeStatus::Evaluated,
                        actor: actor.trim().into(),
                        reason: reason.into(),
                        at: now,
                    });
            }
        }

        next.classification = evaluation.classification;
        next.confidence = evaluation.confidence_breakdown.final_confidence;
        next.confidence_breakdown = evaluation.confidence_breakdown;
        next.assessments = evaluation.assessments;
        next.regressions = evaluation.regressions;
        next.contradictions = evaluation.contradictions;
        next.unresolved_questions = evaluation.unresolved_questions;
        next.causal_language = evaluation.causal_language;
        next.lessons = evaluation.lessons;
        next.recommended_follow_up = evaluation.recommended_follow_up;
        next.evaluation_report = Some(OutcomeEvaluationReport {
            verification_ready: evaluation.verification_ready && !evaluation.blocked,
            steps: evaluation.steps,
            method: EVALUATION_METHOD.into(),
            evaluated_at: now,
        });
        next.provenance.capability = Some("evaluate_measured_learning_outcome".into());
        self.store.append_measured_learning_outcome(&next)?;
        Ok(next)
    }

    /// Explicitly verify a Measured Learning Outcome (requires Evaluated + actor + reason).
    pub fn verify_measured_learning_outcome(
        &self,
        investigation_id: InvestigationId,
        outcome_id: ObjectId,
        actor: impl Into<String>,
        reason: impl Into<String>,
        override_readiness: bool,
        override_reason: Option<String>,
    ) -> RivoraResult<MeasuredLearningOutcome> {
        let current = self.get_latest_measured_outcome(investigation_id, outcome_id)?;
        let next = current.verified(
            actor,
            reason,
            Utc::now(),
            override_readiness,
            override_reason,
        )?;
        self.store.append_measured_learning_outcome(&next)?;
        Ok(next)
    }

    /// List Measured Learning Outcomes for an Investigation.
    pub fn list_measured_learning_outcomes(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<MeasuredOutcomeListing> {
        self.store.load_investigation(&investigation_id)?;
        self.store
            .list_measured_learning_outcomes(&investigation_id)
    }

    /// Load one Measured Learning Outcome snapshot.
    pub fn get_measured_learning_outcome(
        &self,
        investigation_id: InvestigationId,
        outcome_id: ObjectId,
    ) -> RivoraResult<MeasuredLearningOutcome> {
        self.store
            .load_measured_learning_outcome(&investigation_id, &outcome_id)
    }

    /// List all revisions in a Measured Learning Outcome lineage.
    pub fn list_measured_outcome_revisions(
        &self,
        investigation_id: InvestigationId,
        lineage_id: ObjectId,
    ) -> RivoraResult<MeasuredOutcomeListing> {
        self.store
            .list_measured_outcome_revisions(&investigation_id, &lineage_id)
    }

    /// Trace Proposal → Implementation → Measured Learning Outcome.
    pub fn trace_measured_learning_outcome(
        &self,
        investigation_id: InvestigationId,
        outcome_id: ObjectId,
    ) -> RivoraResult<MeasuredOutcomeTrace> {
        let outcome = self
            .store
            .load_measured_learning_outcome(&investigation_id, &outcome_id)?;
        Ok(MeasuredOutcomeTrace {
            investigation_id,
            proposal_id: outcome.proposal_id,
            implementation_record_id: outcome.implementation_record_id,
            outcome_id: outcome.id,
            classification: outcome.classification,
            status: outcome.status,
            explanation: "Accepted Proposal ≠ Implementation Record ≠ Evaluated Measured Learning Outcome ≠ Verified Measured Learning Outcome. Evaluation never applies changes or mutates external systems.".into(),
        })
    }

    /// Export a Measured Learning Outcome as Markdown.
    pub fn export_measured_learning_outcome_markdown(
        &self,
        investigation_id: InvestigationId,
        outcome_id: ObjectId,
    ) -> RivoraResult<String> {
        let outcome = self
            .store
            .load_measured_learning_outcome(&investigation_id, &outcome_id)?;
        Ok(format_outcome_markdown(&outcome))
    }

    /// Export a Measured Learning Outcome as JSON.
    pub fn export_measured_learning_outcome_json(
        &self,
        investigation_id: InvestigationId,
        outcome_id: ObjectId,
    ) -> RivoraResult<String> {
        let outcome = self
            .store
            .load_measured_learning_outcome(&investigation_id, &outcome_id)?;
        serde_json::to_string_pretty(&outcome)
            .map_err(|e| RivoraError::serialization(e.to_string()))
    }

    // -----------------------------------------------------------------------
    // Learning Patterns (RFC-024)
    // -----------------------------------------------------------------------

    /// Derive Learning Patterns from verified, historically eligible Outcomes.
    ///
    /// Groups by proposal category and expected-result signature. Counts each
    /// Outcome lineage once (latest eligible verified revision).
    pub fn derive_learning_patterns(
        &self,
        actor: impl Into<String>,
    ) -> RivoraResult<Vec<LearningPattern>> {
        let actor = actor.into();
        if actor.trim().is_empty() {
            return Err(RivoraError::validation(
                "pattern derivation actor is required",
            ));
        }
        let investigations = self.store.list_investigations()?;
        // lineage_id -> latest verified eligible outcome
        let mut by_lineage: std::collections::HashMap<ObjectId, MeasuredLearningOutcome> =
            std::collections::HashMap::new();
        for inv_id in investigations {
            let listing = self.store.list_measured_learning_outcomes(&inv_id)?;
            for outcome in listing.outcomes {
                if outcome.status != MeasuredOutcomeStatus::Verified {
                    continue;
                }
                if !outcome.historical_learning_eligible {
                    continue;
                }
                by_lineage
                    .entry(outcome.lineage_id)
                    .and_modify(|existing| {
                        if outcome.revision_number > existing.revision_number {
                            *existing = outcome.clone();
                        }
                    })
                    .or_insert(outcome);
            }
        }

        // Group by signature: category + sorted expected-result source texts.
        let mut groups: std::collections::BTreeMap<String, Vec<MeasuredLearningOutcome>> =
            std::collections::BTreeMap::new();
        for outcome in by_lineage.into_values() {
            let proposal = self
                .store
                .load_proposal(&outcome.investigation_id, &outcome.proposal_id)
                .ok();
            let category = proposal
                .as_ref()
                .map(|p| p.category.as_str().to_string())
                .unwrap_or_else(|| "unknown".into());
            let mut criteria: Vec<String> = outcome
                .expected_results
                .iter()
                .map(|e| e.source_text.to_lowercase())
                .collect();
            criteria.sort();
            criteria.dedup();
            let signature = format!("{}:{}", category, criteria.join("|"));
            groups.entry(signature).or_default().push(outcome);
        }

        let mut derived = Vec::new();
        for (signature, outcomes) in groups {
            if outcomes.is_empty() {
                continue;
            }
            let category = signature.split(':').next().map(|s| s.to_string());
            let title = format!(
                "Historical pattern for {} ({} outcomes)",
                category.as_deref().unwrap_or("unknown"),
                outcomes.len()
            );
            let mut pattern = LearningPattern::derived(
                title,
                signature,
                category,
                Provenance::now(actor.trim(), "runtime")
                    .with_capability("derive_learning_patterns"),
            )?;

            let mut first = outcomes[0].created_at;
            let mut last = outcomes[0].updated_at;
            for outcome in &outcomes {
                first = first.min(outcome.created_at);
                last = last.max(outcome.updated_at);
                match outcome.classification {
                    OutcomeClassification::Successful => {
                        pattern.classification_counts.successful += 1;
                        pattern.supporting_outcome_ids.push(outcome.id);
                    }
                    OutcomeClassification::PartiallySuccessful => {
                        pattern.classification_counts.partially_successful += 1;
                        pattern.supporting_outcome_ids.push(outcome.id);
                    }
                    OutcomeClassification::Mixed => {
                        pattern.classification_counts.mixed += 1;
                        pattern.mixed_outcome_ids.push(outcome.id);
                    }
                    OutcomeClassification::Unsuccessful => {
                        pattern.classification_counts.unsuccessful += 1;
                        pattern.contradicting_outcome_ids.push(outcome.id);
                    }
                    OutcomeClassification::Regressed => {
                        pattern.classification_counts.regressed += 1;
                        pattern.contradicting_outcome_ids.push(outcome.id);
                    }
                    OutcomeClassification::Inconclusive => {
                        pattern.classification_counts.inconclusive += 1;
                    }
                    OutcomeClassification::NotImplemented => {
                        pattern.classification_counts.not_implemented += 1;
                    }
                    OutcomeClassification::Invalidated => {
                        pattern.classification_counts.invalidated += 1;
                    }
                    OutcomeClassification::Pending => {}
                }
            }
            pattern.first_observed = first;
            pattern.last_observed = last;

            let support = pattern.classification_counts.successful
                + pattern.classification_counts.partially_successful;
            let contradict = pattern.classification_counts.unsuccessful
                + pattern.classification_counts.regressed;
            let total = outcomes.len() as f64;
            pattern.confidence = Confidence::new(if total > 0.0 {
                support as f64 / total
            } else {
                0.0
            });
            pattern.status = if contradict > 0 && support > 0 {
                PatternStatus::Contested
            } else if support >= 2 {
                PatternStatus::Supported
            } else {
                PatternStatus::Emerging
            };

            // Collect scope from affected components on linked proposals when available.
            let mut scope = Vec::new();
            for outcome in &outcomes {
                if let Ok(proposal) = self
                    .store
                    .load_proposal(&outcome.investigation_id, &outcome.proposal_id)
                {
                    for c in proposal.affected_components {
                        if !scope.contains(&c) {
                            scope.push(c);
                        }
                    }
                }
            }
            pattern.scope = scope;

            self.store.append_learning_pattern(&pattern)?;
            derived.push(pattern);
        }
        Ok(derived)
    }

    /// List all Learning Patterns.
    pub fn list_learning_patterns(&self) -> RivoraResult<Vec<LearningPattern>> {
        self.store.list_learning_patterns()
    }

    /// Load one Learning Pattern.
    pub fn get_learning_pattern(&self, pattern_id: ObjectId) -> RivoraResult<LearningPattern> {
        self.store.load_learning_pattern(&pattern_id)
    }

    /// Retire a Learning Pattern with an explicit reason.
    pub fn retire_learning_pattern(
        &self,
        pattern_id: ObjectId,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> RivoraResult<LearningPattern> {
        let current = self.store.load_learning_pattern(&pattern_id)?;
        // Prefer latest non-retired by walking parents is unnecessary; id is the snapshot.
        let next = current.retired(actor, reason, Utc::now())?;
        self.store.append_learning_pattern(&next)?;
        Ok(next)
    }

    /// Explain historical Pattern influence for a Proposal (advisory only).
    pub fn explain_historical_influence(
        &self,
        investigation_id: InvestigationId,
        proposal_id: ObjectId,
    ) -> RivoraResult<HistoricalInfluenceExplanation> {
        let proposal = self.load_proposal_snapshot(investigation_id, proposal_id)?;
        let patterns = self.store.list_learning_patterns()?;
        let category = proposal.category.as_str();
        let mut considered = Vec::new();
        let mut aggregate = 0.0;
        for pattern in patterns {
            if pattern.status == PatternStatus::Retired {
                continue;
            }
            let relevant = pattern
                .proposal_category
                .as_deref()
                .map(|c| c == category)
                .unwrap_or(false)
                || pattern.signature.starts_with(&format!("{category}:"));
            if !relevant {
                continue;
            }
            let support = pattern.classification_counts.successful as f64
                + 0.5 * pattern.classification_counts.partially_successful as f64;
            let harm = pattern.classification_counts.unsuccessful as f64
                + pattern.classification_counts.regressed as f64;
            let total = (support + harm).max(1.0);
            let magnitude = ((support - harm) / total) * pattern.confidence.value();
            // Contested patterns have limited influence.
            let magnitude = if pattern.status == PatternStatus::Contested {
                magnitude * 0.25
            } else {
                magnitude
            };
            aggregate += magnitude;
            considered.push(HistoricalPatternInfluence {
                pattern_id: pattern.id,
                relevance: format!(
                    "Matches proposal category '{category}' with pattern status {}",
                    pattern.status.as_str()
                ),
                magnitude,
                direction: if magnitude >= 0.0 {
                    "supports".into()
                } else {
                    "warns".into()
                },
                supporting_outcome_ids: pattern.supporting_outcome_ids.clone(),
                contradicting_outcome_ids: pattern.contradicting_outcome_ids.clone(),
            });
        }
        // Current evidence remains primary — never silently suppress.
        let current_evidence_overrode = !proposal.supporting_evidence.is_empty() && aggregate < 0.0;
        let explanation = if considered.is_empty() {
            "No historical Learning Patterns matched this Proposal category. Current Investigation evidence remains primary.".into()
        } else {
            format!(
                "Considered {} historical Learning Pattern(s). Aggregate advisory influence {:.3}. Historical success never proves present correctness; current evidence can override history.",
                considered.len(),
                aggregate
            )
        };
        Ok(HistoricalInfluenceExplanation {
            proposal_id: proposal.id,
            patterns_considered: considered,
            aggregate_influence: aggregate,
            current_evidence_overrode,
            explanation,
        })
    }

    /// Export a Learning Pattern as Markdown.
    pub fn export_learning_pattern_markdown(
        &self,
        pattern_id: ObjectId,
    ) -> RivoraResult<String> {
        let pattern = self.store.load_learning_pattern(&pattern_id)?;
        Ok(format_pattern_markdown(&pattern))
    }

    /// Export a Learning Pattern as JSON.
    pub fn export_learning_pattern_json(&self, pattern_id: ObjectId) -> RivoraResult<String> {
        let pattern = self.store.load_learning_pattern(&pattern_id)?;
        serde_json::to_string_pretty(&pattern)
            .map_err(|e| RivoraError::serialization(e.to_string()))
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn load_proposal_snapshot(
        &self,
        investigation_id: InvestigationId,
        proposal_id: ObjectId,
    ) -> RivoraResult<ImprovementProposal> {
        // Accept either exact snapshot or resolve latest in lineage if needed.
        match self.store.load_proposal(&investigation_id, &proposal_id) {
            Ok(p) => Ok(p),
            Err(RivoraError::ObjectNotFound(_)) => {
                // Try treat proposal_id as lineage id.
                let listing = self
                    .store
                    .list_proposal_revisions(&investigation_id, &proposal_id)?;
                listing
                    .proposals
                    .into_iter()
                    .max_by_key(|p| p.revision_number)
                    .ok_or(RivoraError::ObjectNotFound(proposal_id))
            }
            Err(e) => Err(e),
        }
    }

    fn get_latest_implementation(
        &self,
        investigation_id: InvestigationId,
        record_id: ObjectId,
    ) -> RivoraResult<ImplementationRecord> {
        let current = self
            .store
            .load_implementation_record(&investigation_id, &record_id)?;
        let listing = self
            .store
            .list_implementation_revisions(&investigation_id, &current.lineage_id)?;
        listing
            .records
            .into_iter()
            .max_by_key(|r| r.revision_number)
            .ok_or(RivoraError::ObjectNotFound(record_id))
    }

    fn get_latest_measured_outcome(
        &self,
        investigation_id: InvestigationId,
        outcome_id: ObjectId,
    ) -> RivoraResult<MeasuredLearningOutcome> {
        let current = self
            .store
            .load_measured_learning_outcome(&investigation_id, &outcome_id)?;
        let listing = self
            .store
            .list_measured_outcome_revisions(&investigation_id, &current.lineage_id)?;
        listing
            .outcomes
            .into_iter()
            .max_by_key(|o| o.revision_number)
            .ok_or(RivoraError::ObjectNotFound(outcome_id))
    }
}

const EVALUATION_METHOD: &str = "measured_outcome_evaluation_v1";

/// Internal evaluation result.
struct EvaluationResult {
    classification: OutcomeClassification,
    assessments: Vec<ExpectedResultAssessment>,
    confidence_breakdown: ConfidenceBreakdown,
    regressions: Vec<crate::domain::RegressionRecord>,
    contradictions: Vec<crate::domain::ContradictionRecord>,
    unresolved_questions: Vec<String>,
    causal_language: CausalLanguage,
    lessons: Vec<crate::domain::LessonRecord>,
    recommended_follow_up: Vec<String>,
    verification_ready: bool,
    blocked: bool,
    steps: Vec<String>,
}

/// Seed expected results from Proposal success criteria and verification plan.
pub fn seed_expected_results(proposal: &ImprovementProposal) -> Vec<ExpectedResultSpec> {
    let mut results = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for criterion in proposal
        .success_criteria
        .iter()
        .chain(proposal.verification_plan.success_criteria.iter())
    {
        let key = criterion.trim().to_lowercase();
        if key.is_empty() || !seen.insert(key) {
            continue;
        }
        let kind = infer_result_kind(criterion);
        results.push(ExpectedResultSpec {
            id: ObjectId::new(),
            description: criterion.trim().into(),
            kind,
            metric: None,
            target: None,
            tolerance: None,
            requires_baseline: requires_baseline_for(kind),
            weight: 1.0,
            required: true,
            verification_method: None,
            source_text: criterion.trim().into(),
        });
    }
    results
}

fn infer_result_kind(text: &str) -> ExpectedResultKind {
    let lower = text.to_lowercase();
    if lower.contains("latency") || lower.contains("duration") || lower.contains("p99") {
        ExpectedResultKind::LatencyDuration
    } else if lower.contains("error rate") || lower.contains("reliability") {
        ExpectedResultKind::ReliabilityRate
    } else if lower.contains("test") {
        ExpectedResultKind::TestResult
    } else if lower.contains("count") || lower.contains("frequency") {
        ExpectedResultKind::CountFrequency
    } else if lower.contains("threshold") || lower.contains("%") || lower.contains("percent") {
        ExpectedResultKind::NumericThreshold
    } else if lower.contains("improve") || lower.contains("reduce") || lower.contains("increase") {
        ExpectedResultKind::DirectionalImprovement
    } else {
        ExpectedResultKind::HumanAssessment
    }
}

fn requires_baseline_for(kind: ExpectedResultKind) -> bool {
    matches!(
        kind,
        ExpectedResultKind::DirectionalImprovement
            | ExpectedResultKind::LatencyDuration
            | ExpectedResultKind::ReliabilityRate
            | ExpectedResultKind::CountFrequency
            | ExpectedResultKind::NumericThreshold
    )
}

/// Deterministic evaluation rules (RFC-023). Pure and unit-testable.
pub fn evaluate_outcome_deterministic(
    outcome: &MeasuredLearningOutcome,
    implementation: &ImplementationRecord,
) -> EvaluationResult {
    let mut steps = Vec::new();
    steps.push(format!(
        "Loaded implementation {} status={}",
        implementation.id,
        implementation.status.as_str()
    ));

    // Implementation proof check.
    let implementation_proven = implementation_is_proven(implementation, outcome);
    steps.push(format!(
        "Implementation proven: {implementation_proven} (status={}, evidence={}, refs={})",
        implementation.status.as_str(),
        implementation.evidence_ids.len() + outcome.evidence_links.iter().filter(|l| l.relation == OutcomeEvidenceRelation::ConfirmsImplementation).count(),
        implementation.references.len()
    ));

    if !implementation_proven {
        steps.push("Classification = NotImplemented (implementation not proven)".into());
        let mut breakdown = ConfidenceBreakdown::pending();
        breakdown.penalties.push(ConfidencePenalty {
            name: "missing_implementation_proof".into(),
            amount: 0.5,
            explanation: "Implementation is not proven by status, references, or confirming evidence".into(),
        });
        breakdown.final_confidence = Confidence::new(0.2);
        return EvaluationResult {
            classification: OutcomeClassification::NotImplemented,
            assessments: outcome
                .expected_results
                .iter()
                .map(|e| ExpectedResultAssessment {
                    expected_result_id: e.id,
                    kind: ResultAssessmentKind::NotMeasured,
                    reason: "Implementation not proven".into(),
                    confidence: Confidence::new(0.2),
                    evidence_ids: Vec::new(),
                    baseline_compared: false,
                    contradictions: Vec::new(),
                    missing_evidence: vec!["implementation proof".into()],
                })
                .collect(),
            confidence_breakdown: breakdown,
            regressions: Vec::new(),
            contradictions: Vec::new(),
            unresolved_questions: vec![
                "Implementation not proven; link references or confirming evidence".into(),
            ],
            causal_language: CausalLanguage::ObservedAfterImplementation,
            lessons: Vec::new(),
            recommended_follow_up: vec![
                "Confirm external implementation with typed references and evidence".into(),
            ],
            verification_ready: false,
            blocked: false,
            steps,
        };
    }

    // Evidence inventory.
    let has_baseline = outcome
        .evidence_links
        .iter()
        .any(|l| l.relation == OutcomeEvidenceRelation::IsBaseline);
    let has_post = outcome
        .evidence_links
        .iter()
        .any(|l| l.relation == OutcomeEvidenceRelation::IsPostChange);
    let disputes_impl = outcome
        .evidence_links
        .iter()
        .any(|l| l.relation == OutcomeEvidenceRelation::DisputesImplementation);
    let regression_links: Vec<_> = outcome
        .evidence_links
        .iter()
        .filter(|l| l.relation == OutcomeEvidenceRelation::IndicatesRegression)
        .cloned()
        .collect();
    let support_links: Vec<_> = outcome
        .evidence_links
        .iter()
        .filter(|l| l.relation == OutcomeEvidenceRelation::SupportsExpectedResult)
        .cloned()
        .collect();
    let contradict_links: Vec<_> = outcome
        .evidence_links
        .iter()
        .filter(|l| l.relation == OutcomeEvidenceRelation::ContradictsExpectedResult)
        .cloned()
        .collect();

    steps.push(format!(
        "Evidence: baseline={has_baseline} post={has_post} support={} contradict={} regression={}",
        support_links.len(),
        contradict_links.len(),
        regression_links.len()
    ));

    if disputes_impl {
        steps.push("Classification = Invalidated (implementation disputed)".into());
        return EvaluationResult {
            classification: OutcomeClassification::Invalidated,
            assessments: default_assessments(
                outcome,
                ResultAssessmentKind::Invalid,
                "Implementation disputed by evidence",
            ),
            confidence_breakdown: simple_breakdown(0.3, "implementation disputed"),
            regressions: Vec::new(),
            contradictions: Vec::new(),
            unresolved_questions: vec!["Resolve disputed implementation evidence".into()],
            causal_language: CausalLanguage::ObservedAfterImplementation,
            lessons: Vec::new(),
            recommended_follow_up: vec!["Resolve implementation dispute before re-evaluation".into()],
            verification_ready: false,
            blocked: false,
            steps,
        };
    }

    // Per-expected-result assessments.
    let mut assessments = Vec::new();
    let mut unresolved = Vec::new();
    for expected in &outcome.expected_results {
        let related_support: Vec<_> = support_links
            .iter()
            .filter(|l| l.expected_result_id.is_none() || l.expected_result_id == Some(expected.id))
            .collect();
        let related_contradict: Vec<_> = contradict_links
            .iter()
            .filter(|l| l.expected_result_id.is_none() || l.expected_result_id == Some(expected.id))
            .collect();
        let related_regression: Vec<_> = regression_links
            .iter()
            .filter(|l| l.expected_result_id.is_none() || l.expected_result_id == Some(expected.id))
            .collect();

        let mut missing = Vec::new();
        if expected.requires_baseline && !has_baseline {
            missing.push("baseline evidence".into());
        }
        if !has_post && related_support.is_empty() {
            missing.push("post-change or supporting evidence".into());
        }

        let (kind, reason, confidence) = if !related_regression.is_empty() {
            (
                ResultAssessmentKind::Regressed,
                "Regression evidence linked for this expected result".into(),
                Confidence::new(0.7),
            )
        } else if !related_contradict.is_empty() && !related_support.is_empty() {
            (
                ResultAssessmentKind::Inconclusive,
                "Both supporting and contradicting evidence present".into(),
                Confidence::new(0.4),
            )
        } else if !related_contradict.is_empty() {
            (
                ResultAssessmentKind::NotSatisfied,
                "Contradicting evidence without supporting evidence".into(),
                Confidence::new(0.65),
            )
        } else if !related_support.is_empty() && missing.is_empty() {
            (
                ResultAssessmentKind::Satisfied,
                "Supporting evidence present without contradictions".into(),
                Confidence::new(0.75),
            )
        } else if !related_support.is_empty() && !missing.is_empty() {
            (
                ResultAssessmentKind::PartiallySatisfied,
                format!(
                    "Supporting evidence present but missing: {}",
                    missing.join(", ")
                ),
                Confidence::new(0.55),
            )
        } else if !missing.is_empty() {
            unresolved.push(format!(
                "Expected result '{}': missing {}",
                expected.description,
                missing.join(", ")
            ));
            (
                ResultAssessmentKind::NotMeasured,
                format!("Missing evidence: {}", missing.join(", ")),
                Confidence::new(0.2),
            )
        } else {
            (
                ResultAssessmentKind::Inconclusive,
                "Insufficient evidence to assess".into(),
                Confidence::new(0.3),
            )
        };

        steps.push(format!(
            "Expected '{}': {}",
            expected.description,
            kind.as_str()
        ));

        assessments.push(ExpectedResultAssessment {
            expected_result_id: expected.id,
            kind,
            reason,
            confidence,
            evidence_ids: related_support
                .iter()
                .chain(related_contradict.iter())
                .chain(related_regression.iter())
                .map(|l| l.object_id)
                .collect(),
            baseline_compared: has_baseline && expected.requires_baseline,
            contradictions: related_contradict
                .iter()
                .map(|l| {
                    l.reason
                        .clone()
                        .unwrap_or_else(|| "contradicting evidence".into())
                })
                .collect(),
            missing_evidence: missing,
        });
    }

    // Build regressions from links.
    let regressions: Vec<_> = regression_links
        .iter()
        .map(|link| crate::domain::RegressionRecord {
            id: ObjectId::new(),
            regression_type: crate::domain::RegressionType::Other,
            severity: MaterialitySeverity::Material,
            confidence: Confidence::new(0.7),
            description: link
                .reason
                .clone()
                .unwrap_or_else(|| "Regression indicated by linked evidence".into()),
            baseline: None,
            observed: None,
            evidence_ids: vec![link.object_id],
            affected_component: None,
            expected_result_id: link.expected_result_id,
            material: true,
            guardrail_violated: false,
            follow_up: Some("Investigate and mitigate regression".into()),
        })
        .collect();

    let contradictions: Vec<_> = contradict_links
        .iter()
        .filter(|l| {
            support_links.iter().any(|s| {
                s.expected_result_id == l.expected_result_id
                    || (s.expected_result_id.is_none() && l.expected_result_id.is_none())
            })
        })
        .map(|link| crate::domain::ContradictionRecord {
            id: ObjectId::new(),
            description: link
                .reason
                .clone()
                .unwrap_or_else(|| "Supporting and contradicting evidence coexist".into()),
            severity: MaterialitySeverity::Moderate,
            confidence: Confidence::new(0.6),
            evidence_ids: vec![link.object_id],
            expected_result_id: link.expected_result_id,
            resolved: false,
            resolution: None,
        })
        .collect();

    // Overall classification (deterministic policy table).
    let classification = classify_overall(&assessments, &regressions, has_baseline, has_post);
    steps.push(format!(
        "Overall classification = {}",
        classification.as_str()
    ));

    let confidence_breakdown =
        build_confidence_breakdown(implementation, has_baseline, has_post, &assessments, &regressions, &contradictions);

    let causal_language = if classification == OutcomeClassification::Successful
        && confidence_breakdown.final_confidence.value() >= 0.8
        && has_baseline
        && has_post
    {
        CausalLanguage::ConsistentWithExpectedMechanism
    } else if has_post {
        CausalLanguage::CorrelatedWithImplementation
    } else {
        CausalLanguage::ObservedAfterImplementation
    };
    steps.push(format!(
        "Causal language = {}",
        causal_language.as_str()
    ));

    let lessons = derive_lessons(classification, &assessments);
    let recommended_follow_up = derive_follow_up(classification, &unresolved, &regressions);

    // Blocked only when entirely unmeasured and no classification path.
    let all_not_measured = assessments
        .iter()
        .all(|a| a.kind == ResultAssessmentKind::NotMeasured);
    let blocked = all_not_measured && classification == OutcomeClassification::Inconclusive;

    let verification_ready = !blocked
        && !matches!(
            classification,
            OutcomeClassification::Pending | OutcomeClassification::NotImplemented
        )
        && confidence_breakdown.final_confidence.value() >= 0.4
        && contradictions.iter().all(|c| c.resolved || c.severity != MaterialitySeverity::Critical);

    steps.push(format!(
        "Verification ready: {verification_ready} (blocked={blocked})"
    ));

    EvaluationResult {
        classification,
        assessments,
        confidence_breakdown,
        regressions,
        contradictions,
        unresolved_questions: unresolved,
        causal_language,
        lessons,
        recommended_follow_up,
        verification_ready,
        blocked,
        steps,
    }
}

fn implementation_is_proven(
    implementation: &ImplementationRecord,
    outcome: &MeasuredLearningOutcome,
) -> bool {
    if matches!(
        implementation.status,
        ImplementationStatus::Withdrawn | ImplementationStatus::Superseded
    ) {
        return false;
    }
    let confirms = outcome
        .evidence_links
        .iter()
        .any(|l| l.relation == OutcomeEvidenceRelation::ConfirmsImplementation);
    !implementation.references.is_empty()
        || !implementation.evidence_ids.is_empty()
        || confirms
        || matches!(
            implementation.status,
            ImplementationStatus::EvidenceLinked | ImplementationStatus::ReadyForEvaluation
        )
        || (implementation.status == ImplementationStatus::Reported
            && !implementation.summary.is_empty()
            && matches!(
                implementation.source,
                ImplementationSource::GitCommit
                    | ImplementationSource::PullRequest
                    | ImplementationSource::Deployment
                    | ImplementationSource::Patch
            ))
}

fn classify_overall(
    assessments: &[ExpectedResultAssessment],
    regressions: &[crate::domain::RegressionRecord],
    has_baseline: bool,
    has_post: bool,
) -> OutcomeClassification {
    let material_regressed = regressions.iter().any(|r| r.material)
        || assessments
            .iter()
            .any(|a| a.kind == ResultAssessmentKind::Regressed);

    if material_regressed {
        let any_satisfied = assessments.iter().any(|a| {
            matches!(
                a.kind,
                ResultAssessmentKind::Satisfied | ResultAssessmentKind::PartiallySatisfied
            )
        });
        return if any_satisfied {
            OutcomeClassification::Mixed
        } else {
            OutcomeClassification::Regressed
        };
    }

    let required: Vec<_> = assessments.iter().collect();
    if required.is_empty() {
        return OutcomeClassification::Inconclusive;
    }

    let all_satisfied = required
        .iter()
        .all(|a| a.kind == ResultAssessmentKind::Satisfied);
    let any_not_satisfied = required
        .iter()
        .any(|a| a.kind == ResultAssessmentKind::NotSatisfied);
    let any_partial = required
        .iter()
        .any(|a| a.kind == ResultAssessmentKind::PartiallySatisfied);
    let any_inconclusive = required.iter().any(|a| {
        matches!(
            a.kind,
            ResultAssessmentKind::Inconclusive | ResultAssessmentKind::NotMeasured
        )
    });
    let satisfied_count = required
        .iter()
        .filter(|a| {
            matches!(
                a.kind,
                ResultAssessmentKind::Satisfied | ResultAssessmentKind::PartiallySatisfied
            )
        })
        .count();
    let failed_count = required
        .iter()
        .filter(|a| a.kind == ResultAssessmentKind::NotSatisfied)
        .count();

    if all_satisfied && !material_regressed {
        return OutcomeClassification::Successful;
    }
    if any_not_satisfied && satisfied_count > 0 {
        return OutcomeClassification::Mixed;
    }
    if any_not_satisfied && satisfied_count == 0 {
        return OutcomeClassification::Unsuccessful;
    }
    if any_partial && !any_not_satisfied {
        return OutcomeClassification::PartiallySuccessful;
    }
    if any_inconclusive {
        // Missing baseline with no measurements → Inconclusive
        if !has_baseline && !has_post {
            return OutcomeClassification::Inconclusive;
        }
        if satisfied_count > 0 && failed_count == 0 {
            return OutcomeClassification::PartiallySuccessful;
        }
        return OutcomeClassification::Inconclusive;
    }
    OutcomeClassification::Inconclusive
}

fn build_confidence_breakdown(
    implementation: &ImplementationRecord,
    has_baseline: bool,
    has_post: bool,
    assessments: &[ExpectedResultAssessment],
    regressions: &[crate::domain::RegressionRecord],
    contradictions: &[crate::domain::ContradictionRecord],
) -> ConfidenceBreakdown {
    let mut components = Vec::new();
    let impl_quality = if !implementation.references.is_empty() && !implementation.evidence_ids.is_empty()
    {
        0.9
    } else if !implementation.references.is_empty() || !implementation.evidence_ids.is_empty() {
        0.7
    } else {
        0.4
    };
    components.push(ConfidenceComponent {
        name: "implementation_evidence_quality".into(),
        value: impl_quality,
        explanation: "Quality of implementation proof via references and evidence".into(),
    });
    components.push(ConfidenceComponent {
        name: "baseline_evidence_quality".into(),
        value: if has_baseline { 0.85 } else { 0.2 },
        explanation: if has_baseline {
            "Baseline evidence is present".into()
        } else {
            "Baseline evidence is missing".into()
        },
    });
    components.push(ConfidenceComponent {
        name: "post_change_evidence_quality".into(),
        value: if has_post { 0.85 } else { 0.25 },
        explanation: if has_post {
            "Post-change evidence is present".into()
        } else {
            "Post-change evidence is missing".into()
        },
    });
    let measured = assessments
        .iter()
        .filter(|a| a.kind != ResultAssessmentKind::NotMeasured)
        .count();
    let completeness = if assessments.is_empty() {
        0.0
    } else {
        measured as f64 / assessments.len() as f64
    };
    components.push(ConfidenceComponent {
        name: "verification_completeness".into(),
        value: completeness,
        explanation: format!("{measured}/{} expected results measured", assessments.len()),
    });
    let consistency = if contradictions.is_empty() { 0.9 } else { 0.4 };
    components.push(ConfidenceComponent {
        name: "evidence_consistency".into(),
        value: consistency,
        explanation: format!("{} unresolved contradiction(s)", contradictions.len()),
    });
    components.push(ConfidenceComponent {
        name: "regression_coverage".into(),
        value: if regressions.is_empty() { 0.8 } else { 0.5 },
        explanation: format!("{} regression(s) recorded", regressions.len()),
    });

    let avg = components.iter().map(|c| c.value).sum::<f64>() / components.len() as f64;
    let mut penalties = Vec::new();
    if !has_baseline {
        penalties.push(ConfidencePenalty {
            name: "missing_baseline".into(),
            amount: 0.15,
            explanation: "Missing baseline reduces confidence".into(),
        });
    }
    if !has_post {
        penalties.push(ConfidencePenalty {
            name: "missing_post_change".into(),
            amount: 0.15,
            explanation: "Missing post-change evidence reduces confidence".into(),
        });
    }
    if !contradictions.is_empty() {
        penalties.push(ConfidencePenalty {
            name: "unresolved_contradictions".into(),
            amount: 0.1,
            explanation: "Unresolved contradictions reduce confidence".into(),
        });
    }
    let penalty_sum: f64 = penalties.iter().map(|p| p.amount).sum();
    let final_value = (avg - penalty_sum).clamp(0.0, 1.0);

    let mut hints = Vec::new();
    if !has_baseline {
        hints.push("Add baseline evidence".into());
    }
    if !has_post {
        hints.push("Add post-change evidence".into());
    }
    if !contradictions.is_empty() {
        hints.push("Resolve contradictions".into());
    }

    ConfidenceBreakdown {
        final_confidence: Confidence::new(final_value),
        components,
        penalties,
        improvement_hints: hints,
    }
}

fn simple_breakdown(value: f64, reason: &str) -> ConfidenceBreakdown {
    ConfidenceBreakdown {
        final_confidence: Confidence::new(value),
        components: vec![ConfidenceComponent {
            name: "overall".into(),
            value,
            explanation: reason.into(),
        }],
        penalties: Vec::new(),
        improvement_hints: Vec::new(),
    }
}

fn default_assessments(
    outcome: &MeasuredLearningOutcome,
    kind: ResultAssessmentKind,
    reason: &str,
) -> Vec<ExpectedResultAssessment> {
    outcome
        .expected_results
        .iter()
        .map(|e| ExpectedResultAssessment {
            expected_result_id: e.id,
            kind,
            reason: reason.into(),
            confidence: Confidence::new(0.3),
            evidence_ids: Vec::new(),
            baseline_compared: false,
            contradictions: Vec::new(),
            missing_evidence: Vec::new(),
        })
        .collect()
}

fn derive_lessons(
    classification: OutcomeClassification,
    assessments: &[ExpectedResultAssessment],
) -> Vec<crate::domain::LessonRecord> {
    let summary = match classification {
        OutcomeClassification::Successful => "Expected results were satisfied after implementation",
        OutcomeClassification::PartiallySuccessful => {
            "Some expected results were satisfied with bounded gaps"
        }
        OutcomeClassification::Mixed => "Benefits and harms coexisted after implementation",
        OutcomeClassification::Unsuccessful => "Required expectations were not satisfied",
        OutcomeClassification::Regressed => "Material regression was observed after implementation",
        OutcomeClassification::Inconclusive => "Evidence was insufficient for a firm conclusion",
        OutcomeClassification::NotImplemented => "Implementation was not proven",
        OutcomeClassification::Invalidated => "Measurement assumptions were invalidated",
        OutcomeClassification::Pending => "Outcome not yet evaluated",
    };
    vec![crate::domain::LessonRecord {
        id: ObjectId::new(),
        summary: summary.into(),
        conditions: assessments
            .iter()
            .map(|a| format!("{}: {}", a.expected_result_id, a.kind.as_str()))
            .collect(),
        evidence_strength: "Deterministic assessment from linked evidence".into(),
        proposal_category: None,
        applicability: vec![
            "Applies only under similar evidence and scope conditions".into(),
        ],
        exceptions: Vec::new(),
    }]
}

fn derive_follow_up(
    classification: OutcomeClassification,
    unresolved: &[String],
    regressions: &[crate::domain::RegressionRecord],
) -> Vec<String> {
    let mut follow = Vec::new();
    match classification {
        OutcomeClassification::Successful => {
            follow.push("Consider archiving after verification".into());
        }
        OutcomeClassification::PartiallySuccessful => {
            follow.push("Address remaining gaps before broader rollout".into());
        }
        OutcomeClassification::Mixed | OutcomeClassification::Regressed => {
            follow.push("Mitigate regressions before accepting broader impact".into());
        }
        OutcomeClassification::Unsuccessful => {
            follow.push("Revisit Proposal assumptions or roll back if safe".into());
        }
        OutcomeClassification::Inconclusive | OutcomeClassification::NotImplemented => {
            follow.push("Collect additional evidence and re-evaluate".into());
        }
        OutcomeClassification::Invalidated => {
            follow.push("Correct measurement assumptions and restart evaluation".into());
        }
        OutcomeClassification::Pending => {}
    }
    for q in unresolved {
        follow.push(format!("Resolve: {q}"));
    }
    for r in regressions {
        if let Some(fu) = &r.follow_up {
            follow.push(fu.clone());
        }
    }
    follow
}

fn format_outcome_markdown(outcome: &MeasuredLearningOutcome) -> String {
    let mut md = String::new();
    md.push_str("# Measured Learning Outcome\n\n");
    md.push_str(&format!("- **id**: {}\n", outcome.id));
    md.push_str(&format!("- **status**: {}\n", outcome.status.as_str()));
    md.push_str(&format!(
        "- **classification**: {}\n",
        outcome.classification.as_str()
    ));
    md.push_str(&format!(
        "- **confidence**: {:.3}\n",
        outcome.confidence.value()
    ));
    md.push_str(&format!(
        "- **causal language**: {}\n",
        outcome.causal_language.as_str()
    ));
    md.push_str(&format!("- **proposal**: {}\n", outcome.proposal_id));
    md.push_str(&format!(
        "- **implementation**: {}\n",
        outcome.implementation_record_id
    ));
    md.push_str("\n## Expected results\n\n");
    for e in &outcome.expected_results {
        md.push_str(&format!("- {} ({})\n", e.description, e.kind.as_str()));
    }
    md.push_str("\n## Assessments\n\n");
    for a in &outcome.assessments {
        md.push_str(&format!(
            "- {}: {} — {}\n",
            a.expected_result_id,
            a.kind.as_str(),
            a.reason
        ));
    }
    if !outcome.unresolved_questions.is_empty() {
        md.push_str("\n## Unresolved questions\n\n");
        for q in &outcome.unresolved_questions {
            md.push_str(&format!("- {q}\n"));
        }
    }
    md.push_str("\n## Boundary\n\n");
    md.push_str(
        "This Outcome never applies changes. Accepted Proposals are not Implementations; Evaluated Outcomes are not Verified Outcomes.\n",
    );
    md
}

fn format_pattern_markdown(pattern: &LearningPattern) -> String {
    let mut md = String::new();
    md.push_str("# Learning Pattern\n\n");
    md.push_str(&format!("- **id**: {}\n", pattern.id));
    md.push_str(&format!("- **title**: {}\n", pattern.title));
    md.push_str(&format!("- **signature**: {}\n", pattern.signature));
    md.push_str(&format!("- **status**: {}\n", pattern.status.as_str()));
    md.push_str(&format!(
        "- **confidence**: {:.3}\n",
        pattern.confidence.value()
    ));
    md.push_str(&format!(
        "- **supporting outcomes**: {}\n",
        pattern.supporting_outcome_ids.len()
    ));
    md.push_str(&format!(
        "- **contradicting outcomes**: {}\n",
        pattern.contradicting_outcome_ids.len()
    ));
    md.push_str("\n## Boundary\n\n");
    md.push_str(
        "Learning Patterns are advisory historical summaries. They never rewrite Investigations or prove present correctness.\n",
    );
    md
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ImplementationSource, Provenance};

    fn sample_proposal(inv: InvestigationId) -> ImprovementProposal {
        let mut p = ImprovementProposal::generated(
            inv,
            "Add config guard",
            "Reject malformed config",
            "Malformed config reaches Runtime",
            crate::domain::ProposalCategory::Configuration,
            crate::domain::ProposalPriority::High,
            Confidence::new(0.8),
            crate::domain::ProposalGenerationMethod::Deterministic,
            Provenance::now("test", "test"),
        )
        .unwrap();
        p.success_criteria = vec!["Malformed config is rejected".into()];
        p.verification_plan.success_criteria =
            vec!["Verification observes rejected payloads".into()];
        p
    }

    #[test]
    fn seed_expected_results_from_proposal_criteria() {
        let inv = InvestigationId::new();
        let proposal = sample_proposal(inv);
        let seeded = seed_expected_results(&proposal);
        assert_eq!(seeded.len(), 2);
        assert!(seeded.iter().any(|e| e.source_text.contains("Malformed")));
    }

    #[test]
    fn evaluation_not_implemented_without_proof() {
        let inv = InvestigationId::new();
        let proposal_id = ObjectId::new();
        let impl_id = ObjectId::new();
        let record = ImplementationRecord::reported(
            inv,
            proposal_id,
            proposal_id,
            1,
            "engineer",
            ImplementationSource::HumanDeclared,
            "maybe shipped",
            Provenance::now("engineer", "test"),
        )
        .unwrap();
        // No references, no evidence, HumanDeclared → not proven under strict check
        // Actually HumanDeclared with empty refs is NOT proven by our rules.
        let expected = ExpectedResultSpec {
            id: ObjectId::new(),
            description: "works".into(),
            kind: ExpectedResultKind::Boolean,
            metric: None,
            target: None,
            tolerance: None,
            requires_baseline: false,
            weight: 1.0,
            required: true,
            verification_method: None,
            source_text: "works".into(),
        };
        let outcome = MeasuredLearningOutcome::draft(
            inv,
            proposal_id,
            proposal_id,
            1,
            impl_id,
            impl_id,
            vec![expected],
            Provenance::now("runtime", "test"),
        )
        .unwrap();
        let result = evaluate_outcome_deterministic(&outcome, &record);
        assert_eq!(result.classification, OutcomeClassification::NotImplemented);
    }

    #[test]
    fn evaluation_successful_with_support_and_baseline() {
        let inv = InvestigationId::new();
        let proposal_id = ObjectId::new();
        let mut record = ImplementationRecord::reported(
            inv,
            proposal_id,
            proposal_id,
            1,
            "engineer",
            ImplementationSource::GitCommit,
            "shipped",
            Provenance::now("engineer", "test"),
        )
        .unwrap();
        record.references.push(ImplementationReference::CommitSha {
            sha: "abc123".into(),
        });
        record.status = ImplementationStatus::ReadyForEvaluation;

        let expected_id = ObjectId::new();
        let expected = ExpectedResultSpec {
            id: expected_id,
            description: "latency improves".into(),
            kind: ExpectedResultKind::DirectionalImprovement,
            metric: Some("p99".into()),
            target: Some("lower".into()),
            tolerance: None,
            requires_baseline: true,
            weight: 1.0,
            required: true,
            verification_method: None,
            source_text: "latency improves".into(),
        };
        let mut outcome = MeasuredLearningOutcome::draft(
            inv,
            proposal_id,
            proposal_id,
            1,
            record.id,
            record.lineage_id,
            vec![expected],
            Provenance::now("runtime", "test"),
        )
        .unwrap();
        let now = Utc::now();
        outcome.evidence_links = vec![
            OutcomeEvidenceLink {
                object_id: ObjectId::new(),
                relation: OutcomeEvidenceRelation::IsBaseline,
                expected_result_id: Some(expected_id),
                reason: None,
                linked_at: now,
                actor: "eng".into(),
            },
            OutcomeEvidenceLink {
                object_id: ObjectId::new(),
                relation: OutcomeEvidenceRelation::IsPostChange,
                expected_result_id: Some(expected_id),
                reason: None,
                linked_at: now,
                actor: "eng".into(),
            },
            OutcomeEvidenceLink {
                object_id: ObjectId::new(),
                relation: OutcomeEvidenceRelation::SupportsExpectedResult,
                expected_result_id: Some(expected_id),
                reason: Some("p99 down 20%".into()),
                linked_at: now,
                actor: "eng".into(),
            },
        ];
        let result = evaluate_outcome_deterministic(&outcome, &record);
        assert_eq!(result.classification, OutcomeClassification::Successful);
        assert!(result.verification_ready);
        assert!(!result.blocked);
    }

    #[test]
    fn evaluation_mixed_when_support_and_material_regression() {
        let inv = InvestigationId::new();
        let proposal_id = ObjectId::new();
        let mut record = ImplementationRecord::reported(
            inv,
            proposal_id,
            proposal_id,
            1,
            "engineer",
            ImplementationSource::PullRequest,
            "merged",
            Provenance::now("engineer", "test"),
        )
        .unwrap();
        record.references.push(ImplementationReference::PullRequest {
            reference: "42".into(),
        });

        let e1 = ObjectId::new();
        let e2 = ObjectId::new();
        let mut outcome = MeasuredLearningOutcome::draft(
            inv,
            proposal_id,
            proposal_id,
            1,
            record.id,
            record.lineage_id,
            vec![
                ExpectedResultSpec {
                    id: e1,
                    description: "feature works".into(),
                    kind: ExpectedResultKind::Boolean,
                    metric: None,
                    target: None,
                    tolerance: None,
                    requires_baseline: false,
                    weight: 1.0,
                    required: true,
                    verification_method: None,
                    source_text: "feature works".into(),
                },
                ExpectedResultSpec {
                    id: e2,
                    description: "no latency regression".into(),
                    kind: ExpectedResultKind::LatencyDuration,
                    metric: None,
                    target: None,
                    tolerance: None,
                    requires_baseline: true,
                    weight: 1.0,
                    required: true,
                    verification_method: None,
                    source_text: "no latency regression".into(),
                },
            ],
            Provenance::now("runtime", "test"),
        )
        .unwrap();
        let now = Utc::now();
        outcome.evidence_links = vec![
            OutcomeEvidenceLink {
                object_id: ObjectId::new(),
                relation: OutcomeEvidenceRelation::SupportsExpectedResult,
                expected_result_id: Some(e1),
                reason: None,
                linked_at: now,
                actor: "eng".into(),
            },
            OutcomeEvidenceLink {
                object_id: ObjectId::new(),
                relation: OutcomeEvidenceRelation::IndicatesRegression,
                expected_result_id: Some(e2),
                reason: Some("p99 worse".into()),
                linked_at: now,
                actor: "eng".into(),
            },
            OutcomeEvidenceLink {
                object_id: ObjectId::new(),
                relation: OutcomeEvidenceRelation::IsBaseline,
                expected_result_id: None,
                reason: None,
                linked_at: now,
                actor: "eng".into(),
            },
            OutcomeEvidenceLink {
                object_id: ObjectId::new(),
                relation: OutcomeEvidenceRelation::IsPostChange,
                expected_result_id: None,
                reason: None,
                linked_at: now,
                actor: "eng".into(),
            },
        ];
        let result = evaluate_outcome_deterministic(&outcome, &record);
        assert_eq!(result.classification, OutcomeClassification::Mixed);
    }
}
