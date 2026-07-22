//! Recalled Context — historical intelligence for a current Investigation
//! (RFC-017).
//!
//! Recalled Context is owned by the current Investigation. It references a
//! source Investigation and selected Engineering Objects without merging
//! histories. Only **attached** context influences Evaluation and
//! Recommendation; suggested and dismissed records remain inspectable but
//! do not rewrite Memory, Knowledge, or historical conclusions.

use serde::{Deserialize, Serialize};

use crate::domain::{
    Confidence, InvestigationId, ObjectId, OutcomeDisposition, Provenance, RecallOrigin,
    RecalledContext, RecalledContextState, Recommendation, VerificationResult,
};
use crate::error::{RivoraError, RivoraResult};
use crate::runtime::graph::{
    failure_signatures, recommendation_signatures, repository_keys, verification_outcomes,
};
use crate::runtime::Runtime;

/// A detected engineering pattern spanning multiple Investigations (RFC-017).
///
/// Patterns are derived on demand from durable records; they are not
/// persisted and can always be recomputed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectedPattern {
    /// Pattern kind.
    pub kind: PatternKind,
    /// Normalized signature that identifies the pattern.
    pub signature: String,
    /// Human-readable description.
    pub description: String,
    /// Supporting Investigation identifiers (at least two).
    pub investigation_ids: Vec<InvestigationId>,
    /// Supporting Engineering Object identifiers.
    pub object_ids: Vec<ObjectId>,
    /// Occurrence count (typically number of supporting Investigations).
    pub occurrence_count: usize,
    /// Confidence in the pattern.
    pub confidence: Confidence,
    /// Derivation method identifier.
    pub derivation_method: String,
    /// Provenance of this derivation.
    pub provenance: Provenance,
}

/// v0.2 pattern kinds (RFC-017).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatternKind {
    /// Recurring failure signature across Investigations.
    RecurringFailureSignature,
    /// Repeated component or repository.
    RepeatedComponent,
    /// Recurring recommendation signature.
    RecurringRecommendation,
    /// Frequently inconclusive verification.
    FrequentInconclusiveVerification,
    /// Repeated successful mitigation (successful Learning Outcome).
    RepeatedSuccessfulMitigation,
    /// Repeated rejected recommendation.
    RepeatedRejectedRecommendation,
    /// Recurring connector evidence source.
    RecurringConnectorEvidence,
    /// Repeated relationship kind between Investigations.
    RepeatedRelationship,
}

impl PatternKind {
    /// Stable string form.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RecurringFailureSignature => "recurring_failure_signature",
            Self::RepeatedComponent => "repeated_component",
            Self::RecurringRecommendation => "recurring_recommendation",
            Self::FrequentInconclusiveVerification => "frequent_inconclusive_verification",
            Self::RepeatedSuccessfulMitigation => "repeated_successful_mitigation",
            Self::RepeatedRejectedRecommendation => "repeated_rejected_recommendation",
            Self::RecurringConnectorEvidence => "recurring_connector_evidence",
            Self::RepeatedRelationship => "repeated_relationship",
        }
    }
}

/// Minimal historical trend summary over durable records (RFC-017).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoricalTrend {
    /// Total Investigations considered.
    pub investigation_count: usize,
    /// Optional repository filter applied.
    pub repository_filter: Option<String>,
    /// Verification result distribution (pass / fail / inconclusive).
    pub verification: VerificationTrend,
    /// Learning Outcome distribution and recommendation success rate.
    pub learning: LearningTrend,
    /// Top repositories by Investigation count.
    pub top_repositories: Vec<CountItem>,
    /// Top failure signatures by Investigation count.
    pub top_failure_signatures: Vec<CountItem>,
    /// Human-readable summary.
    pub summary: String,
    /// Derivation method.
    pub derivation_method: String,
}

/// Verification pass/fail/inconclusive counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct VerificationTrend {
    /// Passing receipts.
    pub pass: usize,
    /// Failing receipts.
    pub fail: usize,
    /// Inconclusive receipts.
    pub inconclusive: usize,
}

/// Learning Outcome disposition counts and success rate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LearningTrend {
    /// Successful outcomes.
    pub successful: usize,
    /// Unsuccessful outcomes.
    pub unsuccessful: usize,
    /// Rejected outcomes.
    pub rejected: usize,
    /// Accepted (without measured success) outcomes.
    pub accepted: usize,
    /// Ignored outcomes.
    pub ignored: usize,
    /// Success rate among successful+unsuccessful, when any exist.
    pub success_rate: Option<f64>,
}

/// A counted label for trend tops.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CountItem {
    /// Label (repository name, signature, …).
    pub label: String,
    /// Count of Investigations or occurrences.
    pub count: usize,
}

/// Historical influence notes injected into Evaluation / Recommendation.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct HistoricalInfluence {
    /// Attached Recalled Context identifiers.
    pub context_ids: Vec<ObjectId>,
    /// Source Investigation identifiers.
    pub source_investigation_ids: Vec<InvestigationId>,
    /// Explanation lines citing historical context.
    pub notes: Vec<String>,
    /// Warning lines (e.g. prior unsuccessful recommendations).
    pub warnings: Vec<String>,
    /// Positive notes (e.g. prior successful outcomes).
    pub successes: Vec<String>,
}

impl Runtime {
    /// Suggest Recalled Context from related / similar Investigations.
    ///
    /// Creates `Suggested` records for sources that do not already have a
    /// non-dismissed Recalled Context entry. Idempotent for the same
    /// source while a suggested or attached record remains.
    pub fn suggest_recalled_context(
        &self,
        investigation_id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<Vec<RecalledContext>> {
        let actor = actor.into();
        let _ = self.store.load_investigation(&investigation_id)?;
        let existing = self.store.list_recalled_context(&investigation_id)?;
        let covered: std::collections::HashSet<InvestigationId> = existing
            .iter()
            .filter(|c| c.state != RecalledContextState::Dismissed)
            .map(|c| c.source_investigation_id)
            .collect();

        let mut created = Vec::new();

        // Prefer explicit relationship evidence first.
        for relationship in self.list_relationships(investigation_id)? {
            if relationship.confirmation.state == crate::domain::ConfirmationState::Dismissed {
                continue;
            }
            let source = relationship
                .other_end(investigation_id)
                .ok_or_else(|| RivoraError::storage("relationship missing other end"))?;
            if covered.contains(&source) {
                continue;
            }
            let explanation = self.explain_relationship(relationship.id)?.explanation;
            let object_ids: Vec<ObjectId> = relationship
                .evidence
                .iter()
                .flat_map(|e| e.object_ids.iter().copied())
                .collect();
            let summary = format!(
                "Related via {}: {}",
                relationship.kind.as_str(),
                relationship
                    .evidence
                    .first()
                    .map(|e| e.description.as_str())
                    .unwrap_or("shared engineering signals")
            );
            let provenance = Provenance::now(actor.clone(), "runtime")
                .with_capability("suggest_recalled_context")
                .with_evidence(object_ids.clone());
            let context = RecalledContext::new(
                investigation_id,
                source,
                object_ids,
                summary,
                format!("derived relationship {}", relationship.kind.as_str()),
                explanation,
                relationship.confidence,
                RecallOrigin::Automatic,
                RecalledContextState::Suggested,
                provenance,
            )?;
            self.store.save_recalled_context(&context)?;
            created.push(context);
        }

        // Supplement with similar Investigations that lack relationships.
        let similar = self.find_similar_investigations(investigation_id, Some(10))?;
        let existing_after = self.store.list_recalled_context(&investigation_id)?;
        let covered: std::collections::HashSet<InvestigationId> = existing_after
            .iter()
            .filter(|c| c.state != RecalledContextState::Dismissed)
            .map(|c| c.source_investigation_id)
            .collect();
        for result in similar {
            if covered.contains(&result.investigation_id) {
                continue;
            }
            let object_ids: Vec<ObjectId> = result
                .matched_evidence
                .iter()
                .flat_map(|m| m.object_ids.iter().copied())
                .collect();
            let summary = format!(
                "Similar Investigation `{}` (score {:.2})",
                result.title, result.score
            );
            let provenance = Provenance::now(actor.clone(), "runtime")
                .with_capability("suggest_recalled_context")
                .with_evidence(object_ids.clone());
            let context = RecalledContext::new(
                investigation_id,
                result.investigation_id,
                object_ids,
                summary,
                "similar investigation discovery",
                result.explanation,
                Confidence::new(result.score),
                RecallOrigin::Automatic,
                RecalledContextState::Suggested,
                provenance,
            )?;
            self.store.save_recalled_context(&context)?;
            created.push(context);
        }

        self.list_recalled_context(investigation_id)
    }

    /// Explicitly attach historical context from a source Investigation.
    ///
    /// Creates an `Attached` record (manual origin). Does not modify the
    /// source Investigation.
    pub fn attach_recalled_context_from_source(
        &self,
        investigation_id: InvestigationId,
        source_investigation_id: InvestigationId,
        reason: Option<String>,
        actor: impl Into<String>,
    ) -> RivoraResult<RecalledContext> {
        let actor = actor.into();
        let _ = self.store.load_investigation(&investigation_id)?;
        let source = self.store.load_investigation(&source_investigation_id)?;

        // Prefer upgrading an existing non-dismissed suggestion for the same source.
        if let Some(mut existing) = self
            .store
            .list_recalled_context(&investigation_id)?
            .into_iter()
            .find(|c| {
                c.source_investigation_id == source_investigation_id
                    && c.state != RecalledContextState::Dismissed
            })
        {
            existing.attach();
            if let Some(reason) = reason {
                if !reason.trim().is_empty() {
                    existing.reason = reason;
                }
            }
            self.store.save_recalled_context(&existing)?;
            return Ok(existing);
        }

        let bundle = self.load_context_bundle(&source_investigation_id)?;
        let mut object_ids: Vec<ObjectId> = Vec::new();
        object_ids.extend(bundle.knowledge.iter().map(|k| k.id));
        object_ids.extend(bundle.evaluations.iter().map(|e| e.id));
        object_ids.extend(bundle.verifications.iter().map(|v| v.id));
        object_ids.extend(bundle.recommendations.iter().map(|r| r.id));
        object_ids.extend(bundle.learning.iter().map(|l| l.id));
        object_ids.truncate(32);

        let reason = reason
            .filter(|r| !r.trim().is_empty())
            .unwrap_or_else(|| "explicitly selected prior investigation".into());
        let summary = format!(
            "Prior Investigation `{}` [{}]: {} knowledge, {} evaluations, {} outcomes",
            source.title,
            source.status,
            bundle.knowledge.len(),
            bundle.evaluations.len(),
            bundle.learning.len()
        );
        let explanation = format!(
            "Manually attached historical context from Investigation {} \
             (`{}`). Historical evidence is labeled and remains distinct \
             from current Investigation evidence.",
            source_investigation_id, source.title
        );
        let provenance = Provenance::now(actor, "runtime")
            .with_capability("attach_recalled_context")
            .with_evidence(object_ids.clone());
        let context = RecalledContext::new(
            investigation_id,
            source_investigation_id,
            object_ids,
            summary,
            reason,
            explanation,
            Confidence::new(0.85),
            RecallOrigin::Manual,
            RecalledContextState::Attached,
            provenance,
        )?;
        self.store.save_recalled_context(&context)?;
        Ok(context)
    }

    /// Attach (confirm) a suggested Recalled Context record.
    pub fn attach_recalled_context(
        &self,
        investigation_id: InvestigationId,
        context_id: ObjectId,
        actor: impl Into<String>,
    ) -> RivoraResult<RecalledContext> {
        let _: String = actor.into();
        let mut context = self
            .store
            .load_recalled_context(&investigation_id, &context_id)?;
        if context.state == RecalledContextState::Dismissed {
            return Err(RivoraError::Precondition(
                "cannot attach a dismissed recalled context; create a new one".into(),
            ));
        }
        context.attach();
        self.store.save_recalled_context(&context)?;
        Ok(context)
    }

    /// Dismiss a Recalled Context record (never influences reasoning).
    pub fn dismiss_recalled_context(
        &self,
        investigation_id: InvestigationId,
        context_id: ObjectId,
        actor: impl Into<String>,
    ) -> RivoraResult<RecalledContext> {
        let _: String = actor.into();
        let mut context = self
            .store
            .load_recalled_context(&investigation_id, &context_id)?;
        context.dismiss();
        self.store.save_recalled_context(&context)?;
        Ok(context)
    }

    /// List Recalled Context records for an Investigation.
    pub fn list_recalled_context(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<Vec<RecalledContext>> {
        let _ = self.store.load_investigation(&investigation_id)?;
        self.store.list_recalled_context(&investigation_id)
    }

    /// Detect Investigation patterns across durable records (on demand).
    pub fn detect_patterns(&self, actor: impl Into<String>) -> RivoraResult<Vec<DetectedPattern>> {
        let actor = actor.into();
        let mut by_failure: std::collections::BTreeMap<
            String,
            (Vec<InvestigationId>, Vec<ObjectId>),
        > = std::collections::BTreeMap::new();
        let mut by_repo: std::collections::BTreeMap<String, (Vec<InvestigationId>, Vec<ObjectId>)> =
            std::collections::BTreeMap::new();
        let mut by_rec: std::collections::BTreeMap<String, (Vec<InvestigationId>, Vec<ObjectId>)> =
            std::collections::BTreeMap::new();
        let mut inconclusive: (Vec<InvestigationId>, Vec<ObjectId>) = (Vec::new(), Vec::new());
        let mut successful: (Vec<InvestigationId>, Vec<ObjectId>) = (Vec::new(), Vec::new());
        let mut rejected: (Vec<InvestigationId>, Vec<ObjectId>) = (Vec::new(), Vec::new());
        let mut by_source: std::collections::BTreeMap<
            String,
            (Vec<InvestigationId>, Vec<ObjectId>),
        > = std::collections::BTreeMap::new();
        let mut by_rel: std::collections::BTreeMap<String, (Vec<InvestigationId>, Vec<ObjectId>)> =
            std::collections::BTreeMap::new();

        for id in self.store.list_investigations()? {
            let observations = self.store.list_observations(&id)?;
            for (sig, oids) in failure_signatures(&observations) {
                let entry = by_failure.entry(sig).or_default();
                if !entry.0.contains(&id) {
                    entry.0.push(id);
                }
                entry.1.extend(oids);
            }
            for (repo, oids) in repository_keys(&observations) {
                let entry = by_repo.entry(repo).or_default();
                if !entry.0.contains(&id) {
                    entry.0.push(id);
                }
                entry.1.extend(oids);
            }
            for obs in &observations {
                let entry = by_source.entry(obs.source.clone()).or_default();
                if !entry.0.contains(&id) {
                    entry.0.push(id);
                }
                entry.1.push(obs.id);
            }
            let recommendations = self.store.list_recommendations(&id)?;
            for (sig, oids) in recommendation_signatures(&recommendations) {
                let entry = by_rec.entry(sig).or_default();
                if !entry.0.contains(&id) {
                    entry.0.push(id);
                }
                entry.1.extend(oids);
            }
            let verifications = self.store.list_verifications(&id)?;
            let outcomes = verification_outcomes(&verifications);
            if let Some(oids) = outcomes.get(VerificationResult::Inconclusive.as_str()) {
                if !inconclusive.0.contains(&id) {
                    inconclusive.0.push(id);
                }
                inconclusive.1.extend(oids.iter().copied());
            }
            for outcome in self.store.list_learning(&id)? {
                match outcome.disposition {
                    OutcomeDisposition::Successful => {
                        if !successful.0.contains(&id) {
                            successful.0.push(id);
                        }
                        successful.1.push(outcome.id);
                    }
                    OutcomeDisposition::Rejected => {
                        if !rejected.0.contains(&id) {
                            rejected.0.push(id);
                        }
                        rejected.1.push(outcome.id);
                    }
                    _ => {}
                }
            }
        }

        for relationship in self.store.list_relationships()? {
            let key = relationship.kind.as_str().to_string();
            let entry = by_rel.entry(key).or_default();
            for inv in [
                relationship.source_investigation_id,
                relationship.target_investigation_id,
            ] {
                if !entry.0.contains(&inv) {
                    entry.0.push(inv);
                }
            }
            entry.1.push(relationship.id);
        }

        let provenance = Provenance::now(actor, "runtime").with_capability("detect_patterns");
        let mut patterns = Vec::new();

        push_patterns(
            &mut patterns,
            PatternKind::RecurringFailureSignature,
            &by_failure,
            "failure_signature_v1",
            |sig, n| format!("Failure signature `{sig}` appears in {n} Investigations"),
            &provenance,
            0.8,
        );
        push_patterns(
            &mut patterns,
            PatternKind::RepeatedComponent,
            &by_repo,
            "repository_component_v1",
            |sig, n| format!("Repository `{sig}` appears in {n} Investigations"),
            &provenance,
            0.75,
        );
        push_patterns(
            &mut patterns,
            PatternKind::RecurringRecommendation,
            &by_rec,
            "recommendation_signature_v1",
            |sig, n| format!("Recommendation signature `{sig}` appears in {n} Investigations"),
            &provenance,
            0.7,
        );
        push_patterns(
            &mut patterns,
            PatternKind::RecurringConnectorEvidence,
            &by_source,
            "connector_source_v1",
            |sig, n| format!("Connector source `{sig}` appears in {n} Investigations"),
            &provenance,
            0.6,
        );
        push_patterns(
            &mut patterns,
            PatternKind::RepeatedRelationship,
            &by_rel,
            "relationship_kind_v1",
            |sig, n| format!("Relationship kind `{sig}` touches {n} Investigations"),
            &provenance,
            0.65,
        );

        if inconclusive.0.len() >= 2 {
            patterns.push(DetectedPattern {
                kind: PatternKind::FrequentInconclusiveVerification,
                signature: "inconclusive".into(),
                description: format!(
                    "Inconclusive verification appears in {} Investigations",
                    inconclusive.0.len()
                ),
                occurrence_count: inconclusive.0.len(),
                investigation_ids: inconclusive.0,
                object_ids: dedupe_ids(inconclusive.1),
                confidence: Confidence::new(0.7),
                derivation_method: "inconclusive_verification_v1".into(),
                provenance: provenance.clone(),
            });
        }
        if successful.0.len() >= 2 {
            patterns.push(DetectedPattern {
                kind: PatternKind::RepeatedSuccessfulMitigation,
                signature: "successful".into(),
                description: format!(
                    "Successful Learning Outcomes appear in {} Investigations",
                    successful.0.len()
                ),
                occurrence_count: successful.0.len(),
                investigation_ids: successful.0,
                object_ids: dedupe_ids(successful.1),
                confidence: Confidence::new(0.75),
                derivation_method: "successful_mitigation_v1".into(),
                provenance: provenance.clone(),
            });
        }
        if rejected.0.len() >= 2 {
            patterns.push(DetectedPattern {
                kind: PatternKind::RepeatedRejectedRecommendation,
                signature: "rejected".into(),
                description: format!(
                    "Rejected Recommendations appear in {} Investigations",
                    rejected.0.len()
                ),
                occurrence_count: rejected.0.len(),
                investigation_ids: rejected.0,
                object_ids: dedupe_ids(rejected.1),
                confidence: Confidence::new(0.7),
                derivation_method: "rejected_recommendation_v1".into(),
                provenance,
            });
        }

        patterns.sort_by(|a, b| {
            b.occurrence_count
                .cmp(&a.occurrence_count)
                .then_with(|| a.kind.as_str().cmp(b.kind.as_str()))
                .then_with(|| a.signature.cmp(&b.signature))
        });
        Ok(patterns)
    }

    /// Summarize historical trends over durable records (on demand).
    pub fn summarize_historical_trend(
        &self,
        repository: Option<String>,
    ) -> RivoraResult<HistoricalTrend> {
        let repo_filter = repository.map(|r| r.to_lowercase());
        let mut investigation_count = 0usize;
        let mut verification = VerificationTrend::default();
        let mut learning = LearningTrend::default();
        let mut repo_counts: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        let mut failure_counts: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();

        for id in self.store.list_investigations()? {
            let observations = self.store.list_observations(&id)?;
            let repos = repository_keys(&observations);
            if let Some(filter) = &repo_filter {
                if !repos.contains_key(filter) {
                    continue;
                }
            }
            investigation_count += 1;
            for repo in repos.keys() {
                *repo_counts.entry(repo.clone()).or_default() += 1;
            }
            for sig in failure_signatures(&observations).keys() {
                *failure_counts.entry(sig.clone()).or_default() += 1;
            }
            for receipt in self.store.list_verifications(&id)? {
                match receipt.result {
                    VerificationResult::Pass => verification.pass += 1,
                    VerificationResult::Fail => verification.fail += 1,
                    VerificationResult::Inconclusive => verification.inconclusive += 1,
                }
            }
            for outcome in self.store.list_learning(&id)? {
                match outcome.disposition {
                    OutcomeDisposition::Successful => learning.successful += 1,
                    OutcomeDisposition::Unsuccessful => learning.unsuccessful += 1,
                    OutcomeDisposition::Rejected => learning.rejected += 1,
                    OutcomeDisposition::Accepted => learning.accepted += 1,
                    OutcomeDisposition::Ignored => learning.ignored += 1,
                }
            }
        }

        let decided = learning.successful + learning.unsuccessful;
        learning.success_rate = if decided > 0 {
            Some(learning.successful as f64 / decided as f64)
        } else {
            None
        };

        let top_repositories = top_counts(&repo_counts, 5);
        let top_failure_signatures = top_counts(&failure_counts, 5);
        let success_part = learning
            .success_rate
            .map(|r| format!(" recommendation success rate {:.0}%", r * 100.0))
            .unwrap_or_default();
        let summary = format!(
            "{investigation_count} Investigations; verification pass/fail/inconclusive \
             {}/{}/{};{} top repositories: {}.",
            verification.pass,
            verification.fail,
            verification.inconclusive,
            success_part,
            if top_repositories.is_empty() {
                "none".into()
            } else {
                top_repositories
                    .iter()
                    .map(|c| format!("{} ({})", c.label, c.count))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        );

        Ok(HistoricalTrend {
            investigation_count,
            repository_filter: repo_filter,
            verification,
            learning,
            top_repositories,
            top_failure_signatures,
            summary,
            derivation_method: "historical_trend_v1".into(),
        })
    }

    /// Collect historical influence from attached Recalled Context.
    pub(crate) fn historical_influence(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<HistoricalInfluence> {
        let attached: Vec<RecalledContext> = self
            .store
            .list_recalled_context(&investigation_id)?
            .into_iter()
            .filter(|c| c.influences_reasoning())
            .collect();
        if attached.is_empty() {
            return Ok(HistoricalInfluence::default());
        }

        let mut influence = HistoricalInfluence::default();
        for context in &attached {
            influence.context_ids.push(context.id);
            influence
                .source_investigation_ids
                .push(context.source_investigation_id);
            influence.notes.push(format!(
                "Historical context from Investigation {} (recalled as `{}`): {}",
                context.source_investigation_id, context.reason, context.evidence_summary
            ));

            // Surface prior Learning Outcomes without absorbing them as facts.
            for outcome in self.store.list_learning(&context.source_investigation_id)? {
                match outcome.disposition {
                    OutcomeDisposition::Unsuccessful => {
                        let rec_note = outcome
                            .recommendation_id
                            .and_then(|rid| {
                                self.store
                                    .load_recommendation(&context.source_investigation_id, &rid)
                                    .ok()
                                    .map(|r| r.summary)
                            })
                            .unwrap_or_else(|| "prior recommendation".into());
                        influence.warnings.push(format!(
                            "Prior Investigation {} recorded an unsuccessful outcome \
                             for recommendation-like guidance: `{rec_note}` ({})",
                            context.source_investigation_id, outcome.notes
                        ));
                    }
                    OutcomeDisposition::Successful => {
                        influence.successes.push(format!(
                            "Prior Investigation {} recorded a successful outcome: {}",
                            context.source_investigation_id, outcome.notes
                        ));
                    }
                    OutcomeDisposition::Rejected => {
                        influence.warnings.push(format!(
                            "Prior Investigation {} rejected a recommendation: {}",
                            context.source_investigation_id, outcome.notes
                        ));
                    }
                    _ => {}
                }
            }

            // Highlight prior inconclusive verification without changing
            // current Verification logic.
            for receipt in self
                .store
                .list_verifications(&context.source_investigation_id)?
            {
                if receipt.result == VerificationResult::Inconclusive {
                    influence.notes.push(format!(
                        "Prior Investigation {} had inconclusive verification: {}",
                        context.source_investigation_id, receipt.reason
                    ));
                    break;
                }
            }
        }
        Ok(influence)
    }

    /// Apply historical influence metadata to an Evaluation in place.
    pub(crate) fn apply_historical_influence_to_evaluation(
        evaluation: &mut crate::domain::Evaluation,
        influence: &HistoricalInfluence,
    ) {
        if influence.context_ids.is_empty() {
            return;
        }
        evaluation.metadata.insert(
            "recalled_context_ids".into(),
            serde_json::json!(influence
                .context_ids
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()),
        );
        evaluation.metadata.insert(
            "historical_source_investigation_ids".into(),
            serde_json::json!(influence
                .source_investigation_ids
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()),
        );
        evaluation
            .metadata
            .insert("historical_influence".into(), serde_json::json!(true));

        let mut extra = String::from(
            " Historical context (labeled; not current fact): current assessment \
             remains based on this Investigation's Memory and Knowledge.",
        );
        for note in influence
            .notes
            .iter()
            .chain(influence.warnings.iter())
            .chain(influence.successes.iter())
        {
            extra.push(' ');
            extra.push_str(note);
        }
        evaluation.explanation.push_str(&extra);
    }

    /// Apply historical influence metadata and rationale notes to a Recommendation.
    pub(crate) fn apply_historical_influence_to_recommendation(
        recommendation: &mut Recommendation,
        influence: &HistoricalInfluence,
    ) {
        if influence.context_ids.is_empty() {
            return;
        }
        recommendation.metadata.insert(
            "recalled_context_ids".into(),
            serde_json::json!(influence
                .context_ids
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()),
        );
        recommendation.metadata.insert(
            "historical_source_investigation_ids".into(),
            serde_json::json!(influence
                .source_investigation_ids
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()),
        );
        recommendation
            .metadata
            .insert("historical_influence".into(), serde_json::json!(true));
        if !influence.warnings.is_empty() {
            recommendation.metadata.insert(
                "historical_warnings".into(),
                serde_json::json!(influence.warnings),
            );
        }
        if !influence.successes.is_empty() {
            recommendation.metadata.insert(
                "historical_successes".into(),
                serde_json::json!(influence.successes),
            );
        }

        recommendation.rationale.push_str(
            " Historical intelligence (labeled; prior conclusions are not \
             repeated automatically):",
        );
        for warning in &influence.warnings {
            recommendation.rationale.push_str(" WARNING: ");
            recommendation.rationale.push_str(warning);
        }
        for success in &influence.successes {
            recommendation.rationale.push_str(" NOTE: ");
            recommendation.rationale.push_str(success);
        }
        for note in &influence.notes {
            recommendation.rationale.push(' ');
            recommendation.rationale.push_str(note);
        }
    }
}

fn push_patterns(
    patterns: &mut Vec<DetectedPattern>,
    kind: PatternKind,
    map: &std::collections::BTreeMap<String, (Vec<InvestigationId>, Vec<ObjectId>)>,
    method: &str,
    describe: impl Fn(&str, usize) -> String,
    provenance: &Provenance,
    confidence: f64,
) {
    for (signature, (invs, oids)) in map {
        if invs.len() < 2 {
            continue;
        }
        patterns.push(DetectedPattern {
            kind,
            signature: signature.clone(),
            description: describe(signature, invs.len()),
            investigation_ids: invs.clone(),
            object_ids: dedupe_ids(oids.clone()),
            occurrence_count: invs.len(),
            confidence: Confidence::new(confidence),
            derivation_method: method.into(),
            provenance: provenance.clone(),
        });
    }
}

fn dedupe_ids(ids: Vec<ObjectId>) -> Vec<ObjectId> {
    let mut seen = std::collections::HashSet::new();
    ids.into_iter().filter(|id| seen.insert(*id)).collect()
}

fn top_counts(map: &std::collections::BTreeMap<String, usize>, limit: usize) -> Vec<CountItem> {
    let mut items: Vec<CountItem> = map
        .iter()
        .map(|(label, count)| CountItem {
            label: label.clone(),
            count: *count,
        })
        .collect();
    items.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.label.cmp(&b.label)));
    items.truncate(limit);
    items
}
