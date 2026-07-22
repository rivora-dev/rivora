//! Investigation Graph — relationships between Investigations (RFC-015).
//!
//! The graph is derived state: it records why Investigations are related
//! without ever merging, moving, or rewriting them. Derived relationships
//! have deterministic identifiers, so refresh is idempotent. Explicit
//! links are human-created and preserved across refreshes.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;

use crate::domain::{
    Confidence, ConfirmationState, DerivationMetadata, Evaluation, Investigation, InvestigationId,
    InvestigationRelationship, KnowledgeObject, LearningOutcome, MemoryRecord, ObjectId,
    Observation, ObservationKind, Provenance, Recommendation, RelationshipConfirmation,
    RelationshipEvidence, RelationshipKind, VerificationReceipt,
};
use crate::error::{RivoraError, RivoraResult};
use crate::runtime::Runtime;

/// A relationship plus the fully-loaded Investigation on the other side.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RelatedInvestigation {
    /// Relationship connecting the two Investigations.
    pub relationship: InvestigationRelationship,
    /// The related Investigation (opposite side of the queried one).
    pub related: Investigation,
}

/// A relationship plus a human-readable explanation of why it exists.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RelationshipExplanation {
    /// The relationship being explained.
    pub relationship: InvestigationRelationship,
    /// Explanation composed entirely from stored relationship fields.
    pub explanation: String,
}

impl Runtime {
    /// Link two Investigations explicitly (human-created relationship).
    ///
    /// Idempotent: if an explicit link already exists between the pair in
    /// either direction, the existing link is returned unchanged.
    pub fn link_investigations(
        &self,
        source: InvestigationId,
        target: InvestigationId,
        reason: Option<String>,
        actor: impl Into<String>,
    ) -> RivoraResult<InvestigationRelationship> {
        let actor = actor.into();
        if source == target {
            return Err(RivoraError::validation(
                "cannot link an investigation to itself",
            ));
        }
        let _ = self.store.load_investigation(&source)?;
        let _ = self.store.load_investigation(&target)?;

        if let Some(existing) = self.store.list_relationships()?.into_iter().find(|r| {
            r.kind == RelationshipKind::ExplicitLink && r.touches(source) && r.touches(target)
        }) {
            return Ok(existing);
        }

        let provenance =
            Provenance::now(actor.clone(), "runtime").with_capability("link_investigations");
        let relationship =
            InvestigationRelationship::explicit(source, target, reason, actor, provenance);
        self.store.save_relationship(&relationship)?;
        Ok(relationship)
    }

    /// Remove an explicit link between Investigations.
    ///
    /// Derived relationships cannot be unlinked; they are replaced or
    /// removed by [`Runtime::refresh_relationships`].
    pub fn unlink_investigation(
        &self,
        relationship_id: ObjectId,
        actor: impl Into<String>,
    ) -> RivoraResult<()> {
        let _: String = actor.into();
        let relationship = self.store.load_relationship(&relationship_id)?;
        if relationship.kind.is_derived() {
            return Err(RivoraError::Precondition(
                "only explicit links can be unlinked; derived relationships are removed by refresh"
                    .into(),
            ));
        }
        self.store.delete_relationship(&relationship_id)
    }

    /// List all relationships (derived and explicit) touching an Investigation.
    pub fn list_relationships(
        &self,
        id: InvestigationId,
    ) -> RivoraResult<Vec<InvestigationRelationship>> {
        let _ = self.store.load_investigation(&id)?;
        Ok(self
            .store
            .list_relationships()?
            .into_iter()
            .filter(|r| r.touches(id))
            .collect())
    }

    /// List related Investigations with the connecting relationships.
    ///
    /// Dismissed relationships are excluded; the opposite-side
    /// Investigation is loaded into each entry.
    pub fn list_related_investigations(
        &self,
        id: InvestigationId,
    ) -> RivoraResult<Vec<RelatedInvestigation>> {
        let relationships = self.list_relationships(id)?;
        let mut related = Vec::new();
        for relationship in relationships
            .into_iter()
            .filter(|r| r.confirmation.state != ConfirmationState::Dismissed)
        {
            let other = relationship.other_end(id).ok_or_else(|| {
                RivoraError::storage("relationship does not touch the investigation")
            })?;
            related.push(RelatedInvestigation {
                relationship,
                related: self.store.load_investigation(&other)?,
            });
        }
        Ok(related)
    }

    /// Explain why two Investigations are related, from stored fields alone.
    pub fn explain_relationship(
        &self,
        relationship_id: ObjectId,
    ) -> RivoraResult<RelationshipExplanation> {
        let relationship = self.store.load_relationship(&relationship_id)?;
        let mut explanation = format!(
            "Relationship {} ({}) connects investigations {} and {} with {:.0}% confidence.\n",
            relationship.id,
            relationship.kind.as_str(),
            relationship.source_investigation_id,
            relationship.target_investigation_id,
            relationship.confidence.value() * 100.0,
        );
        let _ = writeln!(
            explanation,
            "Evidence ({} item(s)):",
            relationship.evidence.len()
        );
        for (index, evidence) in relationship.evidence.iter().enumerate() {
            let objects = evidence
                .object_ids
                .iter()
                .map(ObjectId::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(
                explanation,
                "  {}. {} (objects: {})",
                index + 1,
                evidence.description,
                objects
            );
        }
        let _ = writeln!(
            explanation,
            "Derivation: method `{}` — {}",
            relationship.derivation.method, relationship.derivation.explanation
        );
        let _ = write!(
            explanation,
            "Confirmation: {}",
            relationship.confirmation.state.as_str()
        );
        if let Some(actor) = &relationship.confirmation.actor {
            let _ = write!(explanation, " by {actor}");
        }
        if let Some(at) = &relationship.confirmation.at {
            let _ = write!(explanation, " at {at}");
        }
        Ok(RelationshipExplanation {
            relationship,
            explanation,
        })
    }

    /// Re-derive all derived relationships between one Investigation and
    /// every other Investigation (RFC-015 refresh).
    ///
    /// Refresh is idempotent: derived relationship identity is
    /// deterministic, so unchanged evidence reproduces the same records,
    /// and human confirmation state, provenance, and creation time are
    /// preserved. Stale derived relationships are removed; explicit links
    /// are never touched. Investigation records are never modified.
    pub fn refresh_relationships(
        &self,
        id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<Vec<InvestigationRelationship>> {
        let actor = actor.into();
        let _ = self.store.load_investigation(&id)?;

        // Load the full durable context for every Investigation.
        let mut bundles: HashMap<InvestigationId, ContextBundle> = HashMap::new();
        for other in self.store.list_investigations()? {
            bundles.insert(other, self.load_context_bundle(&other)?);
        }
        let target = bundles
            .get(&id)
            .ok_or(RivoraError::InvestigationNotFound(id))?;

        // Derive candidate relationships for each pair involving `id`.
        let mut candidates: Vec<InvestigationRelationship> = Vec::new();
        for (other_id, other) in &bundles {
            if *other_id == id {
                continue;
            }
            candidates.extend(derive_pair(id, target, *other_id, other, &actor));
        }

        // Reconcile with the stored graph.
        let existing = self.store.list_relationships()?;
        let candidate_ids: HashSet<ObjectId> = candidates.iter().map(|c| c.id).collect();
        for mut candidate in candidates {
            if let Some(prior) = existing.iter().find(|e| e.id == candidate.id) {
                // Same deterministic identity: preserve human state and origin.
                candidate.confirmation = prior.confirmation.clone();
                candidate.provenance = prior.provenance.clone();
                candidate.created_at = prior.created_at;
            }
            self.store.save_relationship(&candidate)?;
        }
        for stale in existing
            .iter()
            .filter(|e| e.kind.is_derived() && e.touches(id) && !candidate_ids.contains(&e.id))
        {
            self.store.delete_relationship(&stale.id)?;
        }

        self.list_relationships(id)
    }

    /// Mark a relationship confirmed by a human reviewer.
    ///
    /// Confirmation state, provenance, and creation time survive
    /// subsequent refreshes while the underlying evidence still holds.
    pub fn confirm_relationship(
        &self,
        relationship_id: ObjectId,
        actor: impl Into<String>,
    ) -> RivoraResult<InvestigationRelationship> {
        let mut relationship = self.store.load_relationship(&relationship_id)?;
        relationship.confirmation = RelationshipConfirmation::confirmed(actor.into());
        self.store.save_relationship(&relationship)?;
        Ok(relationship)
    }

    /// Mark a relationship dismissed by a human reviewer.
    ///
    /// Dismissed relationships remain stored and listed, but are hidden
    /// from related-investigation views.
    pub fn dismiss_relationship(
        &self,
        relationship_id: ObjectId,
        actor: impl Into<String>,
    ) -> RivoraResult<InvestigationRelationship> {
        let mut relationship = self.store.load_relationship(&relationship_id)?;
        relationship.confirmation = RelationshipConfirmation::dismissed(actor.into());
        self.store.save_relationship(&relationship)?;
        Ok(relationship)
    }

    /// Load the full durable context used for relationship derivation.
    pub(crate) fn load_context_bundle(&self, id: &InvestigationId) -> RivoraResult<ContextBundle> {
        Ok(ContextBundle {
            observations: self.store.list_observations(id)?,
            memory: self.store.list_memory(id)?,
            knowledge: self.store.list_knowledge(id)?,
            evaluations: self.store.list_evaluations(id)?,
            verifications: self.store.list_verifications(id)?,
            recommendations: self.store.list_recommendations(id)?,
            learning: self.store.list_learning(id)?,
        })
    }
}

/// Full durable Engineering Object context for one Investigation.
pub(crate) struct ContextBundle {
    pub(crate) observations: Vec<Observation>,
    pub(crate) memory: Vec<MemoryRecord>,
    pub(crate) knowledge: Vec<KnowledgeObject>,
    pub(crate) evaluations: Vec<Evaluation>,
    pub(crate) verifications: Vec<VerificationReceipt>,
    pub(crate) recommendations: Vec<Recommendation>,
    pub(crate) learning: Vec<LearningOutcome>,
}

/// A shared artifact discovered between two Investigations.
pub(crate) struct SharedArtifact {
    pub(crate) key: String,
    pub(crate) description: String,
    pub(crate) a_ids: Vec<ObjectId>,
    pub(crate) b_ids: Vec<ObjectId>,
}

/// Stopwords excluded from significant-token comparison.
const STOPWORDS: [&str; 12] = [
    "this", "that", "with", "from", "have", "been", "were", "when", "then", "than", "into", "your",
];

/// Derive all candidate relationships for one pair of Investigations.
///
/// Emits at most one relationship per kind; multiple shared artifacts
/// become multiple evidence items on that single relationship.
fn derive_pair(
    a_id: InvestigationId,
    a: &ContextBundle,
    b_id: InvestigationId,
    b: &ContextBundle,
    actor: &str,
) -> Vec<InvestigationRelationship> {
    let mut candidates = Vec::new();

    push_candidate(
        &mut candidates,
        RelationshipKind::SharedRepository,
        a_id,
        b_id,
        0.9,
        intersect(
            &repository_keys(&a.observations),
            &repository_keys(&b.observations),
            |key| format!("Both investigations observed repository `{key}`"),
        ),
        "shared_repository_v1",
        "Compares normalized repository names from Repository observations \
         (payload `full_name`/`name`, else first backtick-quoted summary token).",
        actor,
    );

    push_candidate(
        &mut candidates,
        RelationshipKind::SharedCommit,
        a_id,
        b_id,
        0.95,
        intersect(
            &commit_keys(&a.observations),
            &commit_keys(&b.observations),
            |key| format!("Both investigations observed commit `{key}`"),
        ),
        "shared_commit_v1",
        "Compares commit SHAs from Commit observations \
         (payload `sha`, else hex prefix in the summary after `Commit `).",
        actor,
    );

    push_candidate(
        &mut candidates,
        RelationshipKind::SharedPullRequest,
        a_id,
        b_id,
        0.95,
        intersect(
            &pull_request_keys(&a.observations),
            &pull_request_keys(&b.observations),
            |key| format!("Both investigations observed pull request `{key}`"),
        ),
        "shared_pull_request_v1",
        "Compares pull request numbers from PullRequest observations, \
         scoped by repository when the payload names one.",
        actor,
    );

    let file_artifacts = intersect(
        &file_path_keys(&a.observations),
        &file_path_keys(&b.observations),
        |key| format!("Both investigations observed changed file `{key}`"),
    );
    let file_confidence = (0.7 + 0.05 * file_artifacts.len().saturating_sub(1) as f64).min(0.9);
    push_candidate(
        &mut candidates,
        RelationshipKind::SharedFilePath,
        a_id,
        b_id,
        file_confidence,
        file_artifacts,
        "shared_file_path_v1",
        "Compares changed file paths from ChangedFiles observation payloads \
         (0.7 base confidence + 0.05 per extra shared path, capped at 0.9).",
        actor,
    );

    push_candidate(
        &mut candidates,
        RelationshipKind::SharedConnectorSource,
        a_id,
        b_id,
        0.4,
        intersect(
            &connector_sources(&a.observations),
            &connector_sources(&b.observations),
            |key| format!("Both investigations were observed through connector source `{key}`"),
        ),
        "shared_connector_source_v1",
        "Compares connector source names across all observations.",
        actor,
    );

    let (similar_confidence, similar_artifacts) =
        similar_observations(&a.observations, &b.observations);
    push_candidate(
        &mut candidates,
        RelationshipKind::SimilarObservations,
        a_id,
        b_id,
        similar_confidence,
        similar_artifacts,
        "similar_observations_v1",
        "Compares observation kind sets (jaccard >= 0.5) and significant summary \
         tokens (>= 2 shared alphanumeric tokens of length >= 4, stopwords \
         excluded). Confidence is 0.3 + 0.5 * jaccard.",
        actor,
    );

    push_candidate(
        &mut candidates,
        RelationshipKind::SharedEvaluationCategory,
        a_id,
        b_id,
        0.5,
        intersect(
            &evaluation_categories(&a.evaluations),
            &evaluation_categories(&b.evaluations),
            |key| format!("Both investigations share evaluation category `{key}`"),
        ),
        "shared_evaluation_category_v1",
        "Compares (assessment type, severity) pairs across Evaluations.",
        actor,
    );

    push_candidate(
        &mut candidates,
        RelationshipKind::RelatedVerificationOutcome,
        a_id,
        b_id,
        0.45,
        intersect(
            &verification_outcomes(&a.verifications),
            &verification_outcomes(&b.verifications),
            |key| format!("Both investigations recorded verification outcome `{key}`"),
        ),
        "related_verification_outcome_v1",
        "Compares Verification Receipt outcomes.",
        actor,
    );

    push_candidate(
        &mut candidates,
        RelationshipKind::RepeatedFailureSignature,
        a_id,
        b_id,
        0.85,
        intersect(
            &failure_signatures(&a.observations),
            &failure_signatures(&b.observations),
            |key| format!("Both investigations recorded failure signature `{key}`"),
        ),
        "repeated_failure_signature_v1",
        "Normalizes failure signatures from failing CheckResult/TestOutput \
         observations (`kind:name` from payload `name`/`conclusion`/`status`, \
         plus `rollback` when the text mentions a rollback).",
        actor,
    );

    push_candidate(
        &mut candidates,
        RelationshipKind::RelatedRecommendation,
        a_id,
        b_id,
        0.6,
        intersect(
            &recommendation_signatures(&a.recommendations),
            &recommendation_signatures(&b.recommendations),
            |key| format!("Both investigations produced recommendation signature `{key}`"),
        ),
        "related_recommendation_v1",
        "Compares deterministic recommendation signatures \
         (`remediate_failure_signals`, `continue_monitoring`, else first four words).",
        actor,
    );

    push_candidate(
        &mut candidates,
        RelationshipKind::RelatedLearningOutcome,
        a_id,
        b_id,
        0.55,
        intersect(
            &learning_dispositions(&a.learning),
            &learning_dispositions(&b.learning),
            |key| format!("Both investigations recorded learning disposition `{key}`"),
        ),
        "related_learning_outcome_v1",
        "Compares Learning Outcome dispositions.",
        actor,
    );

    candidates
}

/// Build and push one derived relationship candidate from shared artifacts.
#[allow(clippy::too_many_arguments)]
fn push_candidate(
    candidates: &mut Vec<InvestigationRelationship>,
    kind: RelationshipKind,
    a_id: InvestigationId,
    b_id: InvestigationId,
    confidence: f64,
    artifacts: Vec<SharedArtifact>,
    method: &str,
    explanation: &str,
    actor: &str,
) {
    if artifacts.is_empty() {
        return;
    }
    let identity_key = format!(
        "{}|{}",
        kind.as_str(),
        artifacts
            .iter()
            .map(|a| a.key.as_str())
            .collect::<Vec<_>>()
            .join(",")
    );
    let evidence: Vec<RelationshipEvidence> = artifacts
        .into_iter()
        .map(|artifact| {
            let mut ids = artifact.a_ids;
            ids.extend(artifact.b_ids);
            RelationshipEvidence::new(artifact.description, ids)
        })
        .collect();
    let provenance = Provenance::now(actor, "runtime")
        .with_capability("refresh_relationships")
        .with_evidence(
            evidence
                .iter()
                .flat_map(|e| e.object_ids.iter().copied())
                .collect(),
        );
    candidates.push(InvestigationRelationship::derived(
        kind,
        a_id,
        b_id,
        Confidence::new(confidence),
        evidence,
        DerivationMetadata {
            method: method.into(),
            explanation: explanation.into(),
        },
        provenance,
        &identity_key,
    ));
}

/// Intersect two per-side artifact maps (key → object ids) into shared artifacts.
pub(crate) fn intersect(
    a: &BTreeMap<String, Vec<ObjectId>>,
    b: &BTreeMap<String, Vec<ObjectId>>,
    describe: impl Fn(&str) -> String,
) -> Vec<SharedArtifact> {
    a.iter()
        .filter_map(|(key, a_ids)| {
            b.get(key).map(|b_ids| SharedArtifact {
                key: key.clone(),
                description: describe(key),
                a_ids: a_ids.clone(),
                b_ids: b_ids.clone(),
            })
        })
        .collect()
}

/// Record an object id under an optional artifact key.
fn record(map: &mut BTreeMap<String, Vec<ObjectId>>, key: Option<String>, id: ObjectId) {
    if let Some(key) = key {
        map.entry(key).or_default().push(id);
    }
}

/// Trimmed, non-empty string value of a payload field.
fn payload_str(payload: &serde_json::Value, field: &str) -> Option<String> {
    payload
        .get(field)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Render a serde snake_case enum as its wire string.
fn snake_value<T: serde::Serialize + std::fmt::Debug>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{value:?}"))
}

/// Normalized repository keys from Repository observations.
pub(crate) fn repository_keys(observations: &[Observation]) -> BTreeMap<String, Vec<ObjectId>> {
    let mut map: BTreeMap<String, Vec<ObjectId>> = BTreeMap::new();
    for observation in observations
        .iter()
        .filter(|o| o.kind == ObservationKind::Repository)
    {
        record(&mut map, repository_key(observation), observation.id);
    }
    map
}

fn repository_key(observation: &Observation) -> Option<String> {
    for field in ["full_name", "name"] {
        if let Some(value) = payload_str(&observation.payload, field) {
            return Some(value.to_lowercase());
        }
    }
    let start = observation.summary.find('`')?;
    let rest = &observation.summary[start + 1..];
    let end = rest.find('`')?;
    let token = rest[..end].trim();
    (!token.is_empty()).then(|| token.to_lowercase())
}

/// Normalized commit keys from Commit observations.
pub(crate) fn commit_keys(observations: &[Observation]) -> BTreeMap<String, Vec<ObjectId>> {
    let mut map: BTreeMap<String, Vec<ObjectId>> = BTreeMap::new();
    for observation in observations
        .iter()
        .filter(|o| o.kind == ObservationKind::Commit)
    {
        record(&mut map, commit_key(observation), observation.id);
    }
    map
}

fn commit_key(observation: &Observation) -> Option<String> {
    if let Some(sha) = payload_str(&observation.payload, "sha") {
        return Some(sha.to_lowercase());
    }
    let marker = "Commit ";
    let start = observation.summary.find(marker)? + marker.len();
    let hex: String = observation.summary[start..]
        .chars()
        .take_while(|c| c.is_ascii_hexdigit())
        .collect();
    (!hex.is_empty()).then(|| hex.to_lowercase())
}

/// Normalized pull request keys from PullRequest observations.
pub(crate) fn pull_request_keys(observations: &[Observation]) -> BTreeMap<String, Vec<ObjectId>> {
    let mut map: BTreeMap<String, Vec<ObjectId>> = BTreeMap::new();
    for observation in observations
        .iter()
        .filter(|o| o.kind == ObservationKind::PullRequest)
    {
        record(&mut map, pull_request_key(observation), observation.id);
    }
    map
}

fn pull_request_key(observation: &Observation) -> Option<String> {
    let number = observation.payload.get("number")?.as_u64()?;
    for field in ["repository", "repo", "full_name"] {
        if let Some(repo) = payload_str(&observation.payload, field) {
            return Some(format!("{}#{number}", repo.to_lowercase()));
        }
    }
    Some(format!("#{number}"))
}

/// Changed file paths from ChangedFiles observation payloads.
pub(crate) fn file_path_keys(observations: &[Observation]) -> BTreeMap<String, Vec<ObjectId>> {
    let mut map: BTreeMap<String, Vec<ObjectId>> = BTreeMap::new();
    for observation in observations
        .iter()
        .filter(|o| o.kind == ObservationKind::ChangedFiles)
    {
        if let Some(files) = observation.payload.get("files").and_then(|v| v.as_array()) {
            for file in files
                .iter()
                .filter_map(|f| f.as_str())
                .map(str::trim)
                .filter(|f| !f.is_empty())
            {
                map.entry(file.to_string())
                    .or_default()
                    .push(observation.id);
            }
        }
    }
    map
}

/// Connector source names across all observations.
pub(crate) fn connector_sources(observations: &[Observation]) -> BTreeMap<String, Vec<ObjectId>> {
    let mut map: BTreeMap<String, Vec<ObjectId>> = BTreeMap::new();
    for observation in observations {
        map.entry(observation.source.clone())
            .or_default()
            .push(observation.id);
    }
    map
}

/// (Assessment type, severity) category keys from Evaluations.
pub(crate) fn evaluation_categories(evaluations: &[Evaluation]) -> BTreeMap<String, Vec<ObjectId>> {
    let mut map: BTreeMap<String, Vec<ObjectId>> = BTreeMap::new();
    for evaluation in evaluations {
        map.entry(format!(
            "{}/{}",
            snake_value(&evaluation.assessment_type),
            evaluation.severity.as_str()
        ))
        .or_default()
        .push(evaluation.id);
    }
    map
}

/// Outcome keys from Verification Receipts.
pub(crate) fn verification_outcomes(
    verifications: &[VerificationReceipt],
) -> BTreeMap<String, Vec<ObjectId>> {
    let mut map: BTreeMap<String, Vec<ObjectId>> = BTreeMap::new();
    for receipt in verifications {
        map.entry(receipt.result.as_str().to_string())
            .or_default()
            .push(receipt.id);
    }
    map
}

/// Normalized failure signatures from failing CheckResult/TestOutput observations.
pub(crate) fn failure_signatures(observations: &[Observation]) -> BTreeMap<String, Vec<ObjectId>> {
    let mut map: BTreeMap<String, Vec<ObjectId>> = BTreeMap::new();
    for observation in observations.iter().filter(|o| {
        matches!(
            o.kind,
            ObservationKind::CheckResult | ObservationKind::TestOutput
        )
    }) {
        let text = format!(
            "{} {}",
            observation.summary.to_lowercase(),
            observation.payload.to_string().to_lowercase()
        );
        if !(text.contains("fail") || text.contains("error")) {
            continue;
        }
        let name = payload_str(&observation.payload, "name")
            .or_else(|| payload_str(&observation.payload, "conclusion"))
            .or_else(|| payload_str(&observation.payload, "status"))
            .unwrap_or_else(|| "test_output".to_string());
        map.entry(format!(
            "{}:{}",
            observation.kind.as_str(),
            name.to_lowercase()
        ))
        .or_default()
        .push(observation.id);
        if text.contains("rollback") {
            map.entry("rollback".to_string())
                .or_default()
                .push(observation.id);
        }
    }
    map
}

/// Deterministic signature keys from Recommendations.
pub(crate) fn recommendation_signatures(
    recommendations: &[Recommendation],
) -> BTreeMap<String, Vec<ObjectId>> {
    let mut map: BTreeMap<String, Vec<ObjectId>> = BTreeMap::new();
    for recommendation in recommendations {
        record(
            &mut map,
            Some(recommendation_signature(&recommendation.summary)).filter(|s| !s.is_empty()),
            recommendation.id,
        );
    }
    map
}

fn recommendation_signature(summary: &str) -> String {
    let summary = summary.to_lowercase();
    if summary.contains("remediate failure") {
        return "remediate_failure_signals".to_string();
    }
    if summary.contains("continue monitoring") {
        return "continue_monitoring".to_string();
    }
    summary
        .split_whitespace()
        .take(4)
        .collect::<Vec<_>>()
        .join("_")
}

/// Disposition keys from Learning Outcomes.
pub(crate) fn learning_dispositions(
    learning: &[LearningOutcome],
) -> BTreeMap<String, Vec<ObjectId>> {
    let mut map: BTreeMap<String, Vec<ObjectId>> = BTreeMap::new();
    for outcome in learning {
        map.entry(outcome.disposition.as_str().to_string())
            .or_default()
            .push(outcome.id);
    }
    map
}

/// Lowercase alphanumeric tokens of length >= 4, minus stopwords.
pub(crate) fn significant_tokens(text: &str) -> BTreeSet<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 4 && !STOPWORDS.contains(w))
        .map(str::to_string)
        .collect()
}

/// Observation-similarity signal: kind-set jaccard plus token overlap.
///
/// Returns `(confidence, artifacts)`; artifacts are empty unless
/// jaccard >= 0.5 and at least two significant tokens are shared.
fn similar_observations(a: &[Observation], b: &[Observation]) -> (f64, Vec<SharedArtifact>) {
    let a_kinds: BTreeSet<&str> = a.iter().map(|o| o.kind.as_str()).collect();
    let b_kinds: BTreeSet<&str> = b.iter().map(|o| o.kind.as_str()).collect();
    let union: BTreeSet<&str> = a_kinds.union(&b_kinds).copied().collect();
    if union.is_empty() {
        return (0.0, Vec::new());
    }
    let shared_kinds: BTreeSet<&str> = a_kinds.intersection(&b_kinds).copied().collect();
    let jaccard = shared_kinds.len() as f64 / union.len() as f64;

    let mut a_tokens: BTreeMap<String, Vec<ObjectId>> = BTreeMap::new();
    let mut b_tokens: BTreeMap<String, Vec<ObjectId>> = BTreeMap::new();
    for observation in a {
        for token in significant_tokens(&observation.summary) {
            a_tokens.entry(token).or_default().push(observation.id);
        }
    }
    for observation in b {
        for token in significant_tokens(&observation.summary) {
            b_tokens.entry(token).or_default().push(observation.id);
        }
    }
    let mut token_artifacts = intersect(&a_tokens, &b_tokens, |token| {
        format!("Both investigations mention `{token}` in observation summaries")
    });
    if jaccard < 0.5 || token_artifacts.len() < 2 {
        return (0.0, Vec::new());
    }

    let kind_list = shared_kinds
        .iter()
        .map(|k| (*k).to_string())
        .collect::<Vec<_>>();
    let mut artifacts = vec![SharedArtifact {
        key: format!("kinds:{}", kind_list.join(",")),
        description: format!(
            "Both investigations share observation kinds ({}) with jaccard {jaccard:.2}",
            kind_list.join(", ")
        ),
        a_ids: a
            .iter()
            .filter(|o| shared_kinds.contains(o.kind.as_str()))
            .map(|o| o.id)
            .collect(),
        b_ids: b
            .iter()
            .filter(|o| shared_kinds.contains(o.kind.as_str()))
            .map(|o| o.id)
            .collect(),
    }];
    artifacts.append(&mut token_artifacts);
    (0.3 + 0.5 * jaccard, artifacts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn observation(
        kind: ObservationKind,
        summary: &str,
        payload: serde_json::Value,
    ) -> Observation {
        Observation::new(
            InvestigationId::new(),
            kind,
            summary,
            payload,
            "test",
            Utc::now(),
            None,
            Provenance::now("tester", "test"),
        )
        .unwrap()
    }

    #[test]
    fn repository_key_prefers_payload_then_backticks() {
        let full = observation(
            ObservationKind::Repository,
            "whatever",
            serde_json::json!({"full_name": "Acme/App", "name": "ignored"}),
        );
        assert_eq!(repository_key(&full).as_deref(), Some("acme/app"));

        let named = observation(
            ObservationKind::Repository,
            "whatever",
            serde_json::json!({"name": "Acme/App"}),
        );
        assert_eq!(repository_key(&named).as_deref(), Some("acme/app"));

        let backtick = observation(
            ObservationKind::Repository,
            "Observed repository `Acme/App` today",
            serde_json::json!({}),
        );
        assert_eq!(repository_key(&backtick).as_deref(), Some("acme/app"));

        let missing = observation(ObservationKind::Repository, "no key", serde_json::json!({}));
        assert_eq!(repository_key(&missing), None);
    }

    #[test]
    fn commit_key_uses_sha_then_summary_hex() {
        let sha = observation(
            ObservationKind::Commit,
            "whatever",
            serde_json::json!({"sha": "ABC123"}),
        );
        assert_eq!(commit_key(&sha).as_deref(), Some("abc123"));

        let summary = observation(
            ObservationKind::Commit,
            "Commit deadbeefcafe landed on main",
            serde_json::json!({}),
        );
        assert_eq!(commit_key(&summary).as_deref(), Some("deadbeefcafe"));

        let missing = observation(
            ObservationKind::Commit,
            "no sha here",
            serde_json::json!({}),
        );
        assert_eq!(commit_key(&missing), None);
    }

    #[test]
    fn pull_request_key_scopes_by_repository_when_known() {
        let scoped = observation(
            ObservationKind::PullRequest,
            "pr",
            serde_json::json!({"number": 42, "repository": "Acme/App"}),
        );
        assert_eq!(pull_request_key(&scoped).as_deref(), Some("acme/app#42"));

        let unscoped = observation(
            ObservationKind::PullRequest,
            "pr",
            serde_json::json!({"number": 7}),
        );
        assert_eq!(pull_request_key(&unscoped).as_deref(), Some("#7"));

        let missing = observation(ObservationKind::PullRequest, "pr", serde_json::json!({}));
        assert_eq!(pull_request_key(&missing), None);
    }

    #[test]
    fn failure_signature_normalizes_names_and_rollback() {
        let check = observation(
            ObservationKind::CheckResult,
            "Check build failed",
            serde_json::json!({"name": "Build", "conclusion": "failure"}),
        );
        let signatures = failure_signatures(std::slice::from_ref(&check));
        assert!(signatures.contains_key("check_result:build"));

        let status_only = observation(
            ObservationKind::TestOutput,
            "tests error out",
            serde_json::json!({"status": "Failed"}),
        );
        let signatures = failure_signatures(&[status_only]);
        assert!(signatures.contains_key("test_output:failed"));

        let rollback = observation(
            ObservationKind::CheckResult,
            "deploy rollback failed",
            serde_json::json!({"name": "deploy"}),
        );
        let signatures = failure_signatures(&[rollback]);
        assert!(signatures.contains_key("check_result:deploy"));
        assert!(signatures.contains_key("rollback"));

        // Passing checks produce no failure signature.
        let passing = observation(
            ObservationKind::CheckResult,
            "Check build succeeded",
            serde_json::json!({"name": "build", "conclusion": "success"}),
        );
        assert!(failure_signatures(&[passing]).is_empty());
    }

    #[test]
    fn recommendation_signature_is_deterministic() {
        assert_eq!(
            recommendation_signature("Investigate and remediate failure signals before promoting."),
            "remediate_failure_signals"
        );
        assert_eq!(
            recommendation_signature("Continue monitoring; no urgent remediation indicated."),
            "continue_monitoring"
        );
        assert_eq!(
            recommendation_signature("Review Investigation Evidence Carefully please now"),
            "review_investigation_evidence_carefully"
        );
    }

    #[test]
    fn similar_observations_requires_kind_and_token_overlap() {
        let shared_a = [
            observation(
                ObservationKind::CheckResult,
                "build failed badly",
                serde_json::json!({}),
            ),
            observation(
                ObservationKind::Repository,
                "deploy broken today",
                serde_json::json!({}),
            ),
        ];
        let shared_b = [
            observation(
                ObservationKind::CheckResult,
                "build failed again",
                serde_json::json!({}),
            ),
            observation(
                ObservationKind::Repository,
                "deploy broken twice",
                serde_json::json!({}),
            ),
        ];
        let (confidence, artifacts) = similar_observations(&shared_a, &shared_b);
        assert!((confidence - 0.8).abs() < f64::EPSILON); // 0.3 + 0.5 * 1.0
        assert!(!artifacts.is_empty());

        // Token overlap below threshold: only one shared significant token.
        let thin_a = [observation(
            ObservationKind::CheckResult,
            "build failed badly",
            serde_json::json!({}),
        )];
        let thin_b = [observation(
            ObservationKind::CheckResult,
            "build succeeded quietly",
            serde_json::json!({}),
        )];
        let (_, artifacts) = similar_observations(&thin_a, &thin_b);
        assert!(artifacts.is_empty());

        // Kind overlap below threshold: jaccard 1/3 < 0.5.
        let disjoint_a = [observation(
            ObservationKind::CheckResult,
            "build failed badly",
            serde_json::json!({}),
        )];
        let disjoint_b = [
            observation(
                ObservationKind::Repository,
                "build failed badly",
                serde_json::json!({}),
            ),
            observation(
                ObservationKind::Commit,
                "build failed badly",
                serde_json::json!({}),
            ),
        ];
        let (_, artifacts) = similar_observations(&disjoint_a, &disjoint_b);
        assert!(artifacts.is_empty());
    }
}
