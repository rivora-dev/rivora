//! Search and Recall — finding prior Investigations and engineering
//! evidence across Investigations (RFC-016).
//!
//! Retrieval is deterministic, local-first, and inspectable. Every
//! result explains itself: the score is always decomposed into the
//! ranking factors that produced it, with the Engineering Objects that
//! fired each factor. Search never writes; it is a derived read over
//! durable records and behaves identically after Runtime restart.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::{
    ConfirmationState, Investigation, InvestigationId, InvestigationRelationship,
    InvestigationStatus, LearningOutcome, ObjectId, OutcomeDisposition, Provenance,
    RelationshipEvidence, RelationshipKind, VerificationResult,
};
use crate::error::{RivoraError, RivoraResult};
use crate::runtime::embedding::cosine_similarity;
use crate::runtime::graph::{
    commit_keys, connector_sources, evaluation_categories, failure_signatures, file_path_keys,
    learning_dispositions, pull_request_keys, repository_keys, significant_tokens,
    verification_outcomes, ContextBundle,
};
use crate::runtime::Runtime;

// Ranking weights (RFC-016). Every factor that fires contributes its
// weight to the score and one `MatchedEvidence` entry to the result.
/// Exact Investigation ID match weight (short-circuits ranking).
pub const WEIGHT_EXACT_ID: f64 = 1.0;
/// Explicit relationship to the context Investigation weight.
pub const WEIGHT_EXPLICIT_RELATIONSHIP: f64 = 0.9;
/// Shared commit / pull request / file path weight.
pub const WEIGHT_SHARED_ARTIFACT: f64 = 0.6;
/// Matching failure signature weight.
pub const WEIGHT_FAILURE_SIGNATURE: f64 = 0.55;
/// Shared repository weight.
pub const WEIGHT_SHARED_REPOSITORY: f64 = 0.5;
/// Derived relationship to the context Investigation weight.
pub const WEIGHT_DERIVED_RELATIONSHIP: f64 = 0.45;
/// Recommendation outcome match weight.
pub const WEIGHT_OUTCOME_MATCH: f64 = 0.4;
/// Knowledge overlap maximum weight (scaled by kind-set jaccard).
pub const WEIGHT_KNOWLEDGE_OVERLAP: f64 = 0.35;
/// Text token overlap maximum weight (scaled by token coverage).
pub const WEIGHT_TEXT_OVERLAP: f64 = 0.35;
/// Semantic similarity maximum weight (scaled by cosine).
pub const WEIGHT_SEMANTIC_SIMILARITY: f64 = 0.3;
/// Evaluation category overlap weight.
pub const WEIGHT_EVALUATION_OVERLAP: f64 = 0.25;
/// Verification result overlap weight.
pub const WEIGHT_VERIFICATION_OVERLAP: f64 = 0.2;
/// Recency maximum weight (dataset-relative).
pub const WEIGHT_RECENCY: f64 = 0.1;
/// Structured filter match weight (status, source, date, relationship
/// kind filters that pass).
pub const WEIGHT_FILTER_MATCH: f64 = 0.1;
/// Multiplier applied when a human confirmed the relationship.
pub const CONFIRMED_MULTIPLIER: f64 = 1.2;
/// Minimum cosine for the semantic factor to fire. Hashed term-frequency
/// baselines produce small spurious similarity on unrelated text, so a
/// documented threshold keeps the factor inspectable.
pub const SEMANTIC_MIN_SIMILARITY: f64 = 0.15;

/// A structured and/or free-text search over Investigations (RFC-016).
///
/// Structured fields are conjunctive hard filters; `text` drives
/// lexical and semantic scoring. An empty query (no text and no
/// filters) is rejected.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SearchQuery {
    /// Free-text query over human-readable fields.
    pub text: Option<String>,
    /// Exact Investigation identifier (short-circuits search).
    pub investigation_id: Option<InvestigationId>,
    /// Repository name filter (normalized lowercase).
    pub repository: Option<String>,
    /// Lifecycle status filter.
    pub status: Option<InvestigationStatus>,
    /// Connector source filter (observation `source`).
    pub connector_source: Option<String>,
    /// Verification result filter.
    pub verification_result: Option<VerificationResult>,
    /// Learning Outcome disposition filter.
    pub outcome: Option<OutcomeDisposition>,
    /// Relationship kind filter (any stored relationship touching it).
    pub relationship_kind: Option<RelationshipKind>,
    /// Changed-file path filter.
    pub file: Option<String>,
    /// Only Investigations created at or after this time.
    pub created_after: Option<DateTime<Utc>>,
    /// Only Investigations created at or before this time.
    pub created_before: Option<DateTime<Utc>>,
    /// Maximum number of results.
    pub limit: Option<usize>,
}

impl SearchQuery {
    /// True when no text and no filters are set (`limit` is ignored).
    pub fn is_empty(&self) -> bool {
        self.text.is_none()
            && self.investigation_id.is_none()
            && self.repository.is_none()
            && self.status.is_none()
            && self.connector_source.is_none()
            && self.verification_result.is_none()
            && self.outcome.is_none()
            && self.relationship_kind.is_none()
            && self.file.is_none()
            && self.created_after.is_none()
            && self.created_before.is_none()
    }
}

/// One ranking factor that fired for a search result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RankingFactor {
    /// Exact Investigation ID match.
    ExactId,
    /// Explicit relationship to the context Investigation.
    ExplicitRelationship,
    /// Derived relationship to the context Investigation.
    DerivedRelationship,
    /// Shared commit, pull request, or file path.
    SharedArtifact,
    /// Matching failure signature.
    FailureSignatureMatch,
    /// Shared repository.
    SharedRepository,
    /// Matching Learning Outcome disposition.
    OutcomeMatch,
    /// Overlapping Knowledge kinds.
    KnowledgeOverlap,
    /// Lexical token overlap.
    TextOverlap,
    /// Semantic (embedding) similarity.
    SemanticSimilarity,
    /// Overlapping Evaluation categories.
    EvaluationOverlap,
    /// Overlapping Verification results.
    VerificationOverlap,
    /// Structured filter match (status, source, date, relationship kind).
    FilterMatch,
    /// Dataset-relative recency.
    Recency,
}

impl RankingFactor {
    /// Stable string form for explanations and tests.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ExactId => "exact_id",
            Self::ExplicitRelationship => "explicit_relationship",
            Self::DerivedRelationship => "derived_relationship",
            Self::SharedArtifact => "shared_artifact",
            Self::FailureSignatureMatch => "failure_signature_match",
            Self::SharedRepository => "shared_repository",
            Self::OutcomeMatch => "outcome_match",
            Self::KnowledgeOverlap => "knowledge_overlap",
            Self::TextOverlap => "text_overlap",
            Self::SemanticSimilarity => "semantic_similarity",
            Self::EvaluationOverlap => "evaluation_overlap",
            Self::VerificationOverlap => "verification_overlap",
            Self::FilterMatch => "filter_match",
            Self::Recency => "recency",
        }
    }
}

/// Evidence for one fired ranking factor: what matched and which
/// Engineering Objects were involved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchedEvidence {
    /// The ranking factor that fired.
    pub factor: RankingFactor,
    /// Human-readable detail (e.g. "repository `acme/app`").
    pub detail: String,
    /// Engineering Objects that fired the factor.
    pub object_ids: Vec<ObjectId>,
}

/// One explained search result (RFC-016).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    /// Matched Investigation identifier.
    pub investigation_id: InvestigationId,
    /// Investigation title.
    pub title: String,
    /// Lifecycle status.
    pub status: InvestigationStatus,
    /// Rank score in `[0.0, 1.0]` (1.0 for exact id matches).
    pub score: f64,
    /// Human-readable relevance explanation.
    pub explanation: String,
    /// Factors that produced the score, with supporting objects.
    pub matched_evidence: Vec<MatchedEvidence>,
    /// Stored relationship to the context Investigation, when present.
    pub relationship: Option<InvestigationRelationship>,
    /// Learning Outcome dispositions recorded on the Investigation.
    pub outcomes: Vec<OutcomeDisposition>,
    /// Creation time.
    pub created_at: DateTime<Utc>,
    /// Last update time.
    pub updated_at: DateTime<Utc>,
    /// Source provenance of the Investigation.
    pub provenance: Provenance,
}

/// Historical evidence recalled from a related Investigation
/// (read-only; attachment is defined by RFC-017).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecalledEvidence {
    /// Source Investigation the evidence comes from.
    pub investigation_id: InvestigationId,
    /// Relationship that justified the recall.
    pub relationship_id: ObjectId,
    /// Kind of that relationship.
    pub relationship_kind: RelationshipKind,
    /// Why the two Investigations are related.
    pub explanation: String,
    /// Supporting evidence items with Engineering Object ids.
    pub evidence: Vec<RelationshipEvidence>,
}

/// Filters for recalling prior Learning Outcomes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OutcomeFilter {
    /// Only Investigations that observed this repository.
    pub repository: Option<String>,
    /// Only Investigations related to this one.
    pub similar_to: Option<InvestigationId>,
    /// Only this outcome disposition.
    pub disposition: Option<OutcomeDisposition>,
}

/// One prior Learning Outcome with its Investigation context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriorOutcome {
    /// Investigation that recorded the outcome.
    pub investigation_id: InvestigationId,
    /// Title of that Investigation.
    pub investigation_title: String,
    /// The Learning Outcome itself.
    pub outcome: LearningOutcome,
    /// Summary of the Recommendation the outcome refers to, when known.
    pub recommendation_summary: Option<String>,
}

/// The human-readable corpus used for text and semantic matching.
fn search_corpus(inv: &Investigation, bundle: &ContextBundle) -> String {
    let mut corpus = inv.title.clone();
    if let Some(description) = &inv.description {
        let _ = write!(corpus, " {description}");
    }
    for observation in &bundle.observations {
        let _ = write!(corpus, " {}", observation.summary);
    }
    for memory in &bundle.memory {
        let _ = write!(corpus, " {}", memory.summary);
    }
    for knowledge in &bundle.knowledge {
        let _ = write!(corpus, " {}", knowledge.summary);
    }
    for recommendation in &bundle.recommendations {
        let _ = write!(corpus, " {}", recommendation.summary);
    }
    corpus
}

/// Deterministic result ordering: score, then recency, then id.
fn sort_results(results: &mut [SearchResult]) {
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.created_at.cmp(&a.created_at))
            .then_with(|| {
                a.investigation_id
                    .to_string()
                    .cmp(&b.investigation_id.to_string())
            })
    });
}

/// Compose the human-readable explanation from fired factors.
fn compose_explanation(score: f64, fired: &[MatchedEvidence]) -> String {
    let mut explanation = String::from("Matched: ");
    for (index, factor) in fired.iter().enumerate() {
        if index > 0 {
            explanation.push_str("; ");
        }
        explanation.push_str(&factor.detail);
    }
    let _ = write!(explanation, ". Score {score:.2}.");
    explanation
}

impl Runtime {
    /// Search Investigations with structured filters and text (RFC-016).
    ///
    /// Structured filters are conjunctive; text adds lexical and
    /// semantic ranking factors. Every result carries an explanation
    /// and the factors that produced its score.
    pub fn search_investigations(&self, query: SearchQuery) -> RivoraResult<Vec<SearchResult>> {
        if query.is_empty() {
            return Err(RivoraError::validation(
                "search query is empty: provide text or at least one filter",
            ));
        }

        if let Some(id) = query.investigation_id {
            let inv = self.store.load_investigation(&id)?;
            let bundle = self.load_context_bundle(&id)?;
            let fired = vec![MatchedEvidence {
                factor: RankingFactor::ExactId,
                detail: format!("exact Investigation id `{id}`"),
                object_ids: Vec::new(),
            }];
            let explanation = compose_explanation(WEIGHT_EXACT_ID, &fired);
            return Ok(vec![build_result(
                &inv,
                &bundle,
                WEIGHT_EXACT_ID,
                explanation,
                fired,
                None,
            )]);
        }

        let mut rows = Vec::new();
        for id in self.store.list_investigations()? {
            let inv = self.store.load_investigation(&id)?;
            let bundle = self.load_context_bundle(&id)?;
            if !self.passes_filters(&inv, &bundle, &query)? {
                continue;
            }
            rows.push((inv, bundle));
        }

        let query_tokens = query
            .text
            .as_deref()
            .map(significant_tokens)
            .unwrap_or_default();
        let query_embedding = query.text.as_deref().map(|text| self.embedding.embed(text));

        let mut results = Vec::new();
        for (inv, bundle) in &rows {
            let mut score = 0.0;
            let mut fired = Vec::new();

            if let Some(repository) = &query.repository {
                let want = repository.to_lowercase();
                if let Some(ids) = repository_keys(&bundle.observations).get(&want) {
                    score += WEIGHT_SHARED_REPOSITORY;
                    fired.push(MatchedEvidence {
                        factor: RankingFactor::SharedRepository,
                        detail: format!("repository `{want}`"),
                        object_ids: ids.clone(),
                    });
                }
            }
            if let Some(disposition) = query.outcome {
                if let Some(ids) = learning_dispositions(&bundle.learning).get(disposition.as_str())
                {
                    score += WEIGHT_OUTCOME_MATCH;
                    fired.push(MatchedEvidence {
                        factor: RankingFactor::OutcomeMatch,
                        detail: format!("outcome `{}` recorded", disposition.as_str()),
                        object_ids: ids.clone(),
                    });
                }
            }
            if let Some(result) = query.verification_result {
                if let Some(ids) = verification_outcomes(&bundle.verifications).get(result.as_str())
                {
                    score += WEIGHT_VERIFICATION_OVERLAP;
                    fired.push(MatchedEvidence {
                        factor: RankingFactor::VerificationOverlap,
                        detail: format!("verification result `{}`", result.as_str()),
                        object_ids: ids.clone(),
                    });
                }
            }

            // Structured filters without a dedicated factor still explain
            // why the Investigation matched.
            if let Some(status) = query.status {
                score += WEIGHT_FILTER_MATCH;
                fired.push(MatchedEvidence {
                    factor: RankingFactor::FilterMatch,
                    detail: format!("status `{}`", status.as_str()),
                    object_ids: Vec::new(),
                });
            }
            if let Some(source) = &query.connector_source {
                score += WEIGHT_FILTER_MATCH;
                fired.push(MatchedEvidence {
                    factor: RankingFactor::FilterMatch,
                    detail: format!("connector source `{source}`"),
                    object_ids: Vec::new(),
                });
            }
            if query.created_after.is_some() || query.created_before.is_some() {
                score += WEIGHT_FILTER_MATCH;
                fired.push(MatchedEvidence {
                    factor: RankingFactor::FilterMatch,
                    detail: "creation date range".to_string(),
                    object_ids: Vec::new(),
                });
            }
            if let Some(kind) = query.relationship_kind {
                score += WEIGHT_FILTER_MATCH;
                fired.push(MatchedEvidence {
                    factor: RankingFactor::FilterMatch,
                    detail: format!("relationship kind `{}`", kind.as_str()),
                    object_ids: Vec::new(),
                });
            }
            if let Some(file) = &query.file {
                let keys = file_path_keys(&bundle.observations);
                if let Some(ids) = keys.get(file) {
                    score += WEIGHT_SHARED_ARTIFACT;
                    fired.push(MatchedEvidence {
                        factor: RankingFactor::SharedArtifact,
                        detail: format!("changed file `{file}`"),
                        object_ids: ids.clone(),
                    });
                }
            }

            if !query_tokens.is_empty() {
                let corpus = search_corpus(inv, bundle);
                let corpus_tokens = significant_tokens(&corpus);
                let matched: BTreeSet<String> =
                    query_tokens.intersection(&corpus_tokens).cloned().collect();
                if !matched.is_empty() {
                    let coverage = matched.len() as f64 / query_tokens.len() as f64;
                    let contribution = WEIGHT_TEXT_OVERLAP * coverage;
                    score += contribution;
                    let object_ids = bundle
                        .observations
                        .iter()
                        .filter(|o| !significant_tokens(&o.summary).is_disjoint(&matched))
                        .map(|o| o.id)
                        .collect();
                    fired.push(MatchedEvidence {
                        factor: RankingFactor::TextOverlap,
                        detail: format!(
                            "text tokens [{}] ({contribution:.2})",
                            matched.into_iter().collect::<Vec<_>>().join(", ")
                        ),
                        object_ids,
                    });
                }
                if let Some(embedding) = &query_embedding {
                    let similarity = cosine_similarity(embedding, &self.embedding.embed(&corpus));
                    if similarity >= SEMANTIC_MIN_SIMILARITY {
                        let contribution = WEIGHT_SEMANTIC_SIMILARITY * similarity;
                        score += contribution;
                        fired.push(MatchedEvidence {
                            factor: RankingFactor::SemanticSimilarity,
                            detail: format!(
                                "semantic similarity {similarity:.2} ({contribution:.2})"
                            ),
                            object_ids: Vec::new(),
                        });
                    }
                }
            }

            score += recency_contribution(&rows, inv, &mut fired);

            // Recency alone never surfaces a result.
            if score <= 0.0 || fired.iter().all(|f| f.factor == RankingFactor::Recency) {
                continue;
            }
            let score = score.min(1.0);
            let explanation = compose_explanation(score, &fired);
            results.push(build_result(inv, bundle, score, explanation, fired, None));
        }

        sort_results(&mut results);
        if let Some(limit) = query.limit {
            results.truncate(limit);
        }
        Ok(results)
    }

    /// Find Investigations similar to the given one using inspectable
    /// signals (RFC-016). The context Investigation is never returned.
    pub fn find_similar_investigations(
        &self,
        id: InvestigationId,
        limit: Option<usize>,
    ) -> RivoraResult<Vec<SearchResult>> {
        let context = self.store.load_investigation(&id)?;
        let context_bundle = self.load_context_bundle(&id)?;
        let relationships = self.store.list_relationships()?;

        let mut rows = Vec::new();
        for other_id in self.store.list_investigations()? {
            let inv = self.store.load_investigation(&other_id)?;
            let bundle = self.load_context_bundle(&other_id)?;
            rows.push((inv, bundle));
        }

        let mut results = Vec::new();
        for (inv, bundle) in &rows {
            if inv.id == id {
                continue;
            }
            let mut score = 0.0;
            let mut fired = Vec::new();
            let mut confirmed = false;

            let relationship = relationships
                .iter()
                .find(|r| {
                    r.touches(id)
                        && r.touches(inv.id)
                        && r.confirmation.state != ConfirmationState::Dismissed
                })
                .cloned();
            if let Some(rel) = &relationship {
                let explicit = rel.kind == RelationshipKind::ExplicitLink;
                score += if explicit {
                    WEIGHT_EXPLICIT_RELATIONSHIP
                } else {
                    WEIGHT_DERIVED_RELATIONSHIP
                };
                confirmed = rel.confirmation.state == ConfirmationState::Confirmed;
                fired.push(MatchedEvidence {
                    factor: if explicit {
                        RankingFactor::ExplicitRelationship
                    } else {
                        RankingFactor::DerivedRelationship
                    },
                    detail: format!(
                        "{} relationship `{}`{}",
                        rel.kind.as_str(),
                        rel.id,
                        if confirmed { " (confirmed)" } else { "" }
                    ),
                    object_ids: rel
                        .evidence
                        .iter()
                        .flat_map(|e| e.object_ids.iter().copied())
                        .collect(),
                });
            }

            let context_repos = repository_keys(&context_bundle.observations);
            let repos = repository_keys(&bundle.observations);
            let shared_repos: Vec<&String> = context_repos
                .keys()
                .filter(|k| repos.contains_key(*k))
                .collect();
            if !shared_repos.is_empty() {
                score += WEIGHT_SHARED_REPOSITORY;
                fired.push(MatchedEvidence {
                    factor: RankingFactor::SharedRepository,
                    detail: format!(
                        "shared repository {}",
                        shared_repos
                            .iter()
                            .map(|k| format!("`{k}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    object_ids: shared_repos
                        .iter()
                        .flat_map(|k| context_repos[*k].iter().copied())
                        .collect(),
                });
            }

            let mut artifact_ids = Vec::new();
            let mut artifact_details = Vec::new();
            for (label, context_map, map) in [
                (
                    "commit",
                    commit_keys(&context_bundle.observations),
                    commit_keys(&bundle.observations),
                ),
                (
                    "pull request",
                    pull_request_keys(&context_bundle.observations),
                    pull_request_keys(&bundle.observations),
                ),
                (
                    "file",
                    file_path_keys(&context_bundle.observations),
                    file_path_keys(&bundle.observations),
                ),
            ] {
                for key in context_map.keys().filter(|k| map.contains_key(*k)) {
                    artifact_details.push(format!("{label} `{key}`"));
                    artifact_ids.extend(context_map[key].iter().copied());
                }
            }
            if !artifact_details.is_empty() {
                score += WEIGHT_SHARED_ARTIFACT;
                fired.push(MatchedEvidence {
                    factor: RankingFactor::SharedArtifact,
                    detail: format!("shared {}", artifact_details.join(", ")),
                    object_ids: artifact_ids,
                });
            }

            let context_signatures = failure_signatures(&context_bundle.observations);
            let signatures = failure_signatures(&bundle.observations);
            let shared_signatures: Vec<&String> = context_signatures
                .keys()
                .filter(|k| signatures.contains_key(*k))
                .collect();
            if !shared_signatures.is_empty() {
                score += WEIGHT_FAILURE_SIGNATURE;
                fired.push(MatchedEvidence {
                    factor: RankingFactor::FailureSignatureMatch,
                    detail: format!(
                        "failure signature {}",
                        shared_signatures
                            .iter()
                            .map(|k| format!("`{k}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    object_ids: shared_signatures
                        .iter()
                        .flat_map(|k| context_signatures[*k].iter().copied())
                        .collect(),
                });
            }

            let context_kinds: BTreeSet<String> = context_bundle
                .knowledge
                .iter()
                .map(|k| format!("{:?}", k.kind))
                .collect();
            let kinds: BTreeSet<String> = bundle
                .knowledge
                .iter()
                .map(|k| format!("{:?}", k.kind))
                .collect();
            if !context_kinds.is_empty() && !kinds.is_empty() {
                let shared = context_kinds.intersection(&kinds).count();
                if shared > 0 {
                    let union = context_kinds.union(&kinds).count();
                    let contribution = WEIGHT_KNOWLEDGE_OVERLAP * shared as f64 / union as f64;
                    score += contribution;
                    fired.push(MatchedEvidence {
                        factor: RankingFactor::KnowledgeOverlap,
                        detail: format!(
                            "knowledge overlap {shared}/{union} kinds ({contribution:.2})"
                        ),
                        object_ids: bundle.knowledge.iter().map(|k| k.id).collect(),
                    });
                }
            }

            let context_categories = evaluation_categories(&context_bundle.evaluations);
            let categories = evaluation_categories(&bundle.evaluations);
            let shared_categories: Vec<&String> = context_categories
                .keys()
                .filter(|k| categories.contains_key(*k))
                .collect();
            if !shared_categories.is_empty() {
                score += WEIGHT_EVALUATION_OVERLAP;
                fired.push(MatchedEvidence {
                    factor: RankingFactor::EvaluationOverlap,
                    detail: format!(
                        "evaluation category {}",
                        shared_categories
                            .iter()
                            .map(|k| format!("`{k}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    object_ids: shared_categories
                        .iter()
                        .flat_map(|k| categories[*k].iter().copied())
                        .collect(),
                });
            }

            let context_results = verification_outcomes(&context_bundle.verifications);
            let result_map = verification_outcomes(&bundle.verifications);
            let shared_results: Vec<&String> = context_results
                .keys()
                .filter(|k| result_map.contains_key(*k))
                .collect();
            if !shared_results.is_empty() {
                score += WEIGHT_VERIFICATION_OVERLAP;
                fired.push(MatchedEvidence {
                    factor: RankingFactor::VerificationOverlap,
                    detail: format!(
                        "verification result {}",
                        shared_results
                            .iter()
                            .map(|k| format!("`{k}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    object_ids: shared_results
                        .iter()
                        .flat_map(|k| result_map[*k].iter().copied())
                        .collect(),
                });
            }

            let context_tokens = significant_tokens(&search_corpus(&context, &context_bundle));
            if !context_tokens.is_empty() {
                let tokens = significant_tokens(&search_corpus(inv, bundle));
                let matched = context_tokens.intersection(&tokens).count();
                if matched > 0 {
                    let coverage = matched as f64 / context_tokens.len() as f64;
                    let contribution = WEIGHT_TEXT_OVERLAP * coverage;
                    score += contribution;
                    fired.push(MatchedEvidence {
                        factor: RankingFactor::TextOverlap,
                        detail: format!("token overlap {matched} tokens ({contribution:.2})"),
                        object_ids: Vec::new(),
                    });
                }
            }

            score += recency_contribution(&rows, inv, &mut fired);

            if confirmed {
                score *= CONFIRMED_MULTIPLIER;
            }
            // Recency alone never surfaces a similar Investigation.
            if score <= 0.0 || fired.iter().all(|f| f.factor == RankingFactor::Recency) {
                continue;
            }
            let score = score.min(1.0);
            let explanation = compose_explanation(score, &fired);
            results.push(build_result(
                inv,
                bundle,
                score,
                explanation,
                fired,
                relationship,
            ));
        }

        sort_results(&mut results);
        if let Some(limit) = limit {
            results.truncate(limit);
        }
        Ok(results)
    }

    /// Explain why one Investigation appears under a query (RFC-016).
    pub fn explain_search_result(
        &self,
        investigation_id: InvestigationId,
        query: SearchQuery,
    ) -> RivoraResult<SearchResult> {
        let mut scoped = query;
        scoped.limit = None;
        self.search_investigations(scoped)?
            .into_iter()
            .find(|r| r.investigation_id == investigation_id)
            .ok_or_else(|| {
                RivoraError::Precondition("investigation does not match the query".into())
            })
    }

    /// Recall evidence from related Investigations (read-only; RFC-016).
    pub fn recall_related_evidence(
        &self,
        id: InvestigationId,
    ) -> RivoraResult<Vec<RecalledEvidence>> {
        let _ = self.store.load_investigation(&id)?;
        let mut recalled = Vec::new();
        for relationship in self.list_relationships(id)? {
            if relationship.confirmation.state == ConfirmationState::Dismissed {
                continue;
            }
            let other = relationship.other_end(id).ok_or_else(|| {
                RivoraError::storage("relationship does not touch the investigation")
            })?;
            let explanation = self.explain_relationship(relationship.id)?.explanation;
            recalled.push(RecalledEvidence {
                investigation_id: other,
                relationship_id: relationship.id,
                relationship_kind: relationship.kind,
                explanation,
                evidence: relationship.evidence,
            });
        }
        Ok(recalled)
    }

    /// Recall prior Learning Outcomes across Investigations (RFC-016).
    pub fn recall_prior_outcomes(&self, filter: OutcomeFilter) -> RivoraResult<Vec<PriorOutcome>> {
        let related_to: Option<std::collections::HashSet<InvestigationId>> = match filter.similar_to
        {
            Some(id) => {
                let _ = self.store.load_investigation(&id)?;
                Some(
                    self.list_relationships(id)?
                        .into_iter()
                        .filter(|r| r.confirmation.state != ConfirmationState::Dismissed)
                        .filter_map(|r| r.other_end(id))
                        .collect(),
                )
            }
            None => None,
        };

        let mut outcomes = Vec::new();
        for id in self.store.list_investigations()? {
            if let Some(related) = &related_to {
                if !related.contains(&id) {
                    continue;
                }
            }
            let inv = self.store.load_investigation(&id)?;
            if let Some(repository) = &filter.repository {
                let want = repository.to_lowercase();
                let observations = self.store.list_observations(&id)?;
                if !repository_keys(&observations).contains_key(&want) {
                    continue;
                }
            }
            for outcome in self.store.list_learning(&id)? {
                if let Some(disposition) = filter.disposition {
                    if outcome.disposition != disposition {
                        continue;
                    }
                }
                let recommendation_summary = outcome.recommendation_id.and_then(|rec_id| {
                    self.store
                        .load_recommendation(&id, &rec_id)
                        .ok()
                        .map(|r| r.summary)
                });
                outcomes.push(PriorOutcome {
                    investigation_id: id,
                    investigation_title: inv.title.clone(),
                    outcome,
                    recommendation_summary,
                });
            }
        }
        outcomes.sort_by_key(|o| std::cmp::Reverse(o.outcome.observed_at));
        Ok(outcomes)
    }

    /// Conjunctive hard filters of a search query.
    fn passes_filters(
        &self,
        inv: &Investigation,
        bundle: &ContextBundle,
        query: &SearchQuery,
    ) -> RivoraResult<bool> {
        if let Some(status) = query.status {
            if inv.status != status {
                return Ok(false);
            }
        }
        if let Some(after) = query.created_after {
            if inv.created_at < after {
                return Ok(false);
            }
        }
        if let Some(before) = query.created_before {
            if inv.created_at > before {
                return Ok(false);
            }
        }
        if let Some(repository) = &query.repository {
            if !repository_keys(&bundle.observations).contains_key(&repository.to_lowercase()) {
                return Ok(false);
            }
        }
        if let Some(source) = &query.connector_source {
            if !connector_sources(&bundle.observations).contains_key(source) {
                return Ok(false);
            }
        }
        if let Some(result) = query.verification_result {
            if !verification_outcomes(&bundle.verifications).contains_key(result.as_str()) {
                return Ok(false);
            }
        }
        if let Some(disposition) = query.outcome {
            if !learning_dispositions(&bundle.learning).contains_key(disposition.as_str()) {
                return Ok(false);
            }
        }
        if let Some(kind) = query.relationship_kind {
            let touches = self
                .store
                .list_relationships()?
                .iter()
                .any(|r| r.kind == kind && r.touches(inv.id));
            if !touches {
                return Ok(false);
            }
        }
        if let Some(file) = &query.file {
            if !file_path_keys(&bundle.observations).contains_key(file) {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// Build one explained search result for a matched Investigation.
fn build_result(
    inv: &Investigation,
    bundle: &ContextBundle,
    score: f64,
    explanation: String,
    matched_evidence: Vec<MatchedEvidence>,
    relationship: Option<InvestigationRelationship>,
) -> SearchResult {
    SearchResult {
        investigation_id: inv.id,
        title: inv.title.clone(),
        status: inv.status,
        score,
        explanation,
        matched_evidence,
        relationship,
        outcomes: bundle.learning.iter().map(|o| o.disposition).collect(),
        created_at: inv.created_at,
        updated_at: inv.updated_at,
        provenance: inv.provenance.clone(),
    }
}

/// Dataset-relative recency contribution in `[0, WEIGHT_RECENCY]`.
///
/// Deterministic for a fixed investigation set: the newest row gets the
/// full weight, the oldest gets none, and a single-row set gets none.
fn recency_contribution(
    rows: &[(Investigation, ContextBundle)],
    inv: &Investigation,
    fired: &mut Vec<MatchedEvidence>,
) -> f64 {
    let oldest = rows.iter().map(|(i, _)| i.created_at).min();
    let newest = rows.iter().map(|(i, _)| i.created_at).max();
    let (Some(oldest), Some(newest)) = (oldest, newest) else {
        return 0.0;
    };
    let span = (newest - oldest).num_milliseconds();
    if span <= 0 {
        return 0.0;
    }
    let age = (inv.created_at - oldest).num_milliseconds();
    let contribution = WEIGHT_RECENCY * age as f64 / span as f64;
    if contribution > 0.0 {
        fired.push(MatchedEvidence {
            factor: RankingFactor::Recency,
            detail: format!("recency ({contribution:.2})"),
            object_ids: Vec::new(),
        });
    }
    contribution
}
