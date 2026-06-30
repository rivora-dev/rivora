//! Pure adaptive memory engine for Open Rivora.
//!
//! The engine proposes candidate memories, recalls similar past situations,
//! applies human feedback to memory records, and returns receipt-backed
//! decisions. It performs no I/O and emits only memory-oriented
//! recommendations.

use std::collections::{BTreeMap, BTreeSet};

use rivora_errors::{Result, RivoraError};
use rivora_memory::{
    FeedbackKind, FeedbackTargetType, HumanFeedback, MemoryConfidence, MemoryConfidenceLevel,
    MemoryDecay, MemoryKind, MemoryMetadata, MemoryProvenance, MemoryRecord, MemoryRetention,
    MemoryRetentionPolicy, MemoryScope, MemorySource, MemoryStatus, MemoryTimestamps,
    MemoryVersion,
};
use rivora_receipts::{
    Confidence, Evidence, EvidenceKind, EvidenceSource, ReasoningStep, Receipt, ReceiptKind,
    ReceiptProvenance, ReceiptStatus, ReceiptSubject, ReceiptSummary, ReceiptTimestamps,
    ReceiptVersion, Risk, RiskLevel, SuggestedAction,
};
use rivora_types::{NonEmptyString, Version};
use serde::{Deserialize, Serialize};

const ENGINE_NAME: &str = "rivora-adaptive-memory-engine";
const ENGINE_VERSION: &str = "0.1.0";
const SCORING_METHOD: &str = "deterministic-memory-recall-v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryCandidateRequest {
    pub id: String,
    pub kind: MemoryKind,
    pub scope: MemoryScope,
    pub service: String,
    pub symptoms: Vec<String>,
    pub event_summary: String,
    pub evidence_ids: Vec<String>,
    pub source: String,
    pub source_version: String,
    pub confidence: f64,
    pub observed_at: String,
    pub learned_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryCandidateResult {
    pub memory: MemoryRecord,
    pub receipt: Receipt,
    pub recommendations: Vec<MemoryRecommendation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecallQuery {
    pub service: Option<String>,
    pub kind: Option<MemoryKind>,
    pub scope: Option<MemoryScope>,
    pub symptoms: Vec<String>,
    pub tags: Vec<String>,
    pub evidence_ids: Vec<String>,
    pub source: Option<String>,
    pub status: Option<MemoryStatus>,
    pub include_candidates: bool,
    pub limit: usize,
    pub min_score: f64,
    pub generated_at: String,
}

impl Default for RecallQuery {
    fn default() -> Self {
        Self {
            service: None,
            kind: None,
            scope: None,
            symptoms: Vec::new(),
            tags: Vec::new(),
            evidence_ids: Vec::new(),
            source: None,
            status: None,
            include_candidates: false,
            limit: 10,
            min_score: 0.01,
            generated_at: "2026-06-25T12:00:00Z".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecallScore {
    pub value: f64,
    pub method: String,
    pub components: Vec<RecallScoreComponent>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecallScoreComponent {
    pub name: String,
    pub contribution: f64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecallMatch {
    pub memory_id: String,
    pub score: RecallScore,
    pub confidence: f64,
    pub matched_reasons: Vec<String>,
    pub evidence_refs: Vec<String>,
    pub memory: MemoryRecord,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecallResult {
    pub query: RecallQuery,
    pub matches: Vec<RecallMatch>,
    pub receipt: Receipt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryRecommendationKind {
    RememberThis,
    ReviewSimilarMemory,
    CorrectMemory,
    SupersedeStaleMemory,
    RejectLowConfidenceCandidate,
    RequestMoreEvidence,
}

impl MemoryRecommendationKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RememberThis => "remember_this",
            Self::ReviewSimilarMemory => "review_similar_memory",
            Self::CorrectMemory => "correct_memory",
            Self::SupersedeStaleMemory => "supersede_stale_memory",
            Self::RejectLowConfidenceCandidate => "reject_low_confidence_candidate",
            Self::RequestMoreEvidence => "request_more_evidence",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryRecommendation {
    pub kind: MemoryRecommendationKind,
    pub memory_id: Option<String>,
    pub rationale: String,
    pub confidence: f64,
}

impl MemoryRecommendation {
    #[must_use]
    pub fn mutates_infrastructure(&self) -> bool {
        false
    }

    pub fn to_read_only_action(&self) -> Result<SuggestedAction> {
        SuggestedAction::new(
            rivora_receipts::ActionKind::Analyze,
            self.kind.as_str(),
            self.rationale.clone(),
            "Engineer reviews memory context; no infrastructure action is executed.",
            RiskLevel::Low,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedbackApplicationResult {
    pub memory: MemoryRecord,
    pub receipts: Vec<Receipt>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AdaptiveMemoryEngine;

impl AdaptiveMemoryEngine {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    pub fn propose_candidate(
        &self,
        request: MemoryCandidateRequest,
    ) -> Result<MemoryCandidateResult> {
        validate_candidate_request(&request)?;

        let receipt_id = receipt_id("memory_candidate_created", &request.id);
        let mut provenance = MemoryProvenance::new(
            request.source.clone(),
            request.source_version.clone(),
            request.observed_at.clone(),
            request.learned_at.clone(),
        )?
        .with_graph_node_ids(request.evidence_ids.clone())
        .with_receipt_id(receipt_id.clone())
        .with_raw_ref(request.event_summary.clone());
        provenance.connector_ref = Some(request.source.clone());

        let mut labels = BTreeMap::new();
        labels.insert(non_empty("service")?, non_empty(&request.service)?);
        labels.insert(non_empty("source")?, non_empty(&request.source)?);

        let mut tags = vec![non_empty(&request.service)?];
        for symptom in &request.symptoms {
            tags.push(non_empty(symptom)?);
        }

        let title = format!("{} reliability memory candidate", request.service);
        let body = candidate_body(&request);
        let memory = MemoryRecord::builder()
            .id(request.id.clone())
            .kind(request.kind)
            .scope(request.scope)
            .status(MemoryStatus::Candidate)
            .title(title)
            .body(body)
            .subject_refs(vec![non_empty(&request.service)?])
            .graph_node_ids(request.evidence_ids.clone())
            .source(MemorySource::System)
            .provenance(provenance)
            .confidence(memory_confidence(
                request.confidence,
                "Initial candidate confidence from supplied reliability context",
                &request.learned_at,
            )?)
            .retention(
                MemoryRetention::new(
                    MemoryRetentionPolicy::ReviewRequired,
                    "Candidate memories require human review before becoming active",
                )?
                .with_decay(MemoryDecay::ManualReview),
            )
            .timestamps(MemoryTimestamps::new(request.learned_at.clone())?)
            .version(MemoryVersion::new(Version::new(1, 0, 0), 1))
            .labels(labels)
            .metadata(MemoryMetadata::new().with_tags(tags))
            .build()?;

        let receipt = self.memory_candidate_receipt(&request, &memory, &receipt_id)?;
        let recommendations = self.recommend_for_memory(&memory);

        Ok(MemoryCandidateResult {
            memory,
            receipt,
            recommendations,
        })
    }

    pub fn recall(&self, query: RecallQuery, records: &[MemoryRecord]) -> Result<RecallResult> {
        validate_recall_query(&query)?;

        let mut matches = Vec::new();
        for record in records {
            if !record_is_recallable(record, &query) {
                continue;
            }

            let (score, reasons, evidence_refs) = score_record(&query, record);
            if score.value < query.min_score {
                continue;
            }
            let confidence = clamp01((score.value * 0.7) + (record.confidence.score * 0.3));
            matches.push(RecallMatch {
                memory_id: record.id.as_str().to_string(),
                score,
                confidence,
                matched_reasons: reasons,
                evidence_refs,
                memory: record.clone(),
            });
        }

        matches.sort_by(|a, b| {
            b.score
                .value
                .total_cmp(&a.score.value)
                .then_with(|| b.confidence.total_cmp(&a.confidence))
                .then_with(|| a.memory_id.cmp(&b.memory_id))
        });
        matches.truncate(query.limit);

        let receipt = self.recall_receipt(&query, &matches)?;
        Ok(RecallResult {
            query,
            matches,
            receipt,
        })
    }

    pub fn apply_feedback(
        &self,
        record: &MemoryRecord,
        feedback: HumanFeedback,
    ) -> Result<FeedbackApplicationResult> {
        validate_feedback_target(record, &feedback)?;

        let mut updated = record.clone();
        updated.add_feedback(feedback.id.as_str());

        let feedback_receipt = self.feedback_receipt(&updated, &feedback)?;
        let transition_receipt = match feedback.kind {
            FeedbackKind::Approved => {
                updated.approve();
                apply_confidence_adjustment(&mut updated, feedback.confidence_adjustment, 0.1);
                Some(self.transition_receipt(&updated, &feedback, ReceiptKind::MemoryApproved)?)
            }
            FeedbackKind::Rejected => {
                updated.reject(feedback_note(&feedback));
                apply_confidence_adjustment(&mut updated, feedback.confidence_adjustment, -0.2);
                Some(self.transition_receipt(&updated, &feedback, ReceiptKind::MemoryRejected)?)
            }
            FeedbackKind::Corrected
            | FeedbackKind::WrongCause
            | FeedbackKind::WrongService
            | FeedbackKind::WrongTimeWindow => {
                updated.correct(correction_text(&feedback));
                apply_confidence_adjustment(&mut updated, feedback.confidence_adjustment, -0.1);
                Some(self.transition_receipt(&updated, &feedback, ReceiptKind::MemoryCorrected)?)
            }
            FeedbackKind::Useful => {
                apply_confidence_adjustment(&mut updated, feedback.confidence_adjustment, 0.05);
                None
            }
            FeedbackKind::NotUseful => {
                apply_confidence_adjustment(&mut updated, feedback.confidence_adjustment, -0.05);
                None
            }
            FeedbackKind::NeedsMoreEvidence => {
                apply_confidence_adjustment(&mut updated, feedback.confidence_adjustment, -0.1);
                updated
                    .labels
                    .insert(non_empty("needs_more_evidence")?, non_empty("true")?);
                None
            }
        };

        let mut receipts = vec![feedback_receipt];
        if let Some(receipt) = transition_receipt {
            receipts.push(receipt);
        }

        Ok(FeedbackApplicationResult {
            memory: updated,
            receipts,
        })
    }

    #[must_use]
    pub fn recommend_for_memory(&self, memory: &MemoryRecord) -> Vec<MemoryRecommendation> {
        match memory.status {
            MemoryStatus::Candidate if memory.confidence.score < 0.3 => vec![
                MemoryRecommendation {
                    kind: MemoryRecommendationKind::RequestMoreEvidence,
                    memory_id: Some(memory.id.as_str().to_string()),
                    rationale: "Candidate confidence is low; request more evidence before review."
                        .to_string(),
                    confidence: memory.confidence.score,
                },
                MemoryRecommendation {
                    kind: MemoryRecommendationKind::RejectLowConfidenceCandidate,
                    memory_id: Some(memory.id.as_str().to_string()),
                    rationale: "Low-confidence candidate should remain inactive unless an engineer confirms it."
                        .to_string(),
                    confidence: memory.confidence.score,
                },
            ],
            MemoryStatus::Candidate => vec![MemoryRecommendation {
                kind: MemoryRecommendationKind::RememberThis,
                memory_id: Some(memory.id.as_str().to_string()),
                rationale: "Review this candidate memory for possible approval.".to_string(),
                confidence: memory.confidence.score,
            }],
            MemoryStatus::Active => vec![MemoryRecommendation {
                kind: MemoryRecommendationKind::ReviewSimilarMemory,
                memory_id: Some(memory.id.as_str().to_string()),
                rationale: "Use this active memory as context for similar situations.".to_string(),
                confidence: memory.confidence.score,
            }],
            MemoryStatus::Corrected => vec![MemoryRecommendation {
                kind: MemoryRecommendationKind::CorrectMemory,
                memory_id: Some(memory.id.as_str().to_string()),
                rationale: "Review the correction trail before relying on this memory.".to_string(),
                confidence: memory.confidence.score,
            }],
            MemoryStatus::Superseded => vec![MemoryRecommendation {
                kind: MemoryRecommendationKind::SupersedeStaleMemory,
                memory_id: Some(memory.id.as_str().to_string()),
                rationale: "Prefer the newer memory that superseded this record.".to_string(),
                confidence: memory.confidence.score,
            }],
            _ => vec![MemoryRecommendation {
                kind: MemoryRecommendationKind::RequestMoreEvidence,
                memory_id: Some(memory.id.as_str().to_string()),
                rationale: "Memory is not active; ask an engineer for more context before use."
                    .to_string(),
                confidence: memory.confidence.score,
            }],
        }
    }

    fn memory_candidate_receipt(
        &self,
        request: &MemoryCandidateRequest,
        memory: &MemoryRecord,
        receipt_id: &str,
    ) -> Result<Receipt> {
        receipt(
            receipt_id,
            ReceiptKind::MemoryCandidateCreated,
            ReceiptStatus::Draft,
            ReceiptSubject::new("memory", memory.id.as_str(), memory.title.as_str())?,
            ReceiptSummary::new(
                "Memory candidate created",
                "The adaptive memory engine proposed a candidate memory for human review.",
            )?,
            evidence_from_ids(
                &request.evidence_ids,
                &request.source,
                &request.source_version,
                &request.observed_at,
                "Evidence supporting memory candidate",
                request.confidence,
            )?,
            vec![ReasoningStep::new(
                1,
                "Create candidate memory",
                "Reliability context was converted into a candidate memory; no approval was applied.",
                "Candidate memory awaits human review",
                0.2,
            )?],
            Confidence::new(
                request.confidence,
                "memory-candidate-generation-v1",
                "Candidate confidence is inherited from supplied context and requires human review.",
            )?,
            Risk::new(
                RiskLevel::Low,
                "Candidate creation only updates proposed memory state and does not mutate infrastructure.",
            )?,
            &request.learned_at,
        )
    }

    fn recall_receipt(&self, query: &RecallQuery, matches: &[RecallMatch]) -> Result<Receipt> {
        let evidence = if query.evidence_ids.is_empty() {
            vec![Evidence::new(
                EvidenceKind::Annotation,
                evidence_source(ENGINE_NAME, ENGINE_VERSION)?,
                "Recall query context",
                "Recall was evaluated against the supplied query and memory snapshot.",
                query.generated_at.clone(),
                if matches.is_empty() { 0.1 } else { 0.3 },
            )?
            .with_raw_ref("recall-query")]
        } else {
            evidence_from_ids(
                &query.evidence_ids,
                query.source.as_deref().unwrap_or(ENGINE_NAME),
                ENGINE_VERSION,
                &query.generated_at,
                "Evidence supplied with recall query",
                if matches.is_empty() { 0.1 } else { 0.3 },
            )?
        };

        let mut reasoning = vec![ReasoningStep::new(
            1,
            "Score candidate memories",
            "Records were scored with deterministic field overlap: service, scope, kind, symptoms, evidence, source, text, and status.",
            format!("{} memory matches passed the score threshold", matches.len()),
            if matches.is_empty() { 0.0 } else { 0.3 },
        )?];
        if let Some(top) = matches.first() {
            reasoning.push(ReasoningStep::new(
                2,
                "Rank recall matches",
                format!(
                    "Top match {} scored {:.2} because: {}",
                    top.memory_id,
                    top.score.value,
                    top.matched_reasons.join("; ")
                ),
                "Recall results are ranked for engineer review",
                0.2,
            )?);
        }

        receipt(
            &receipt_id(
                "recall_result",
                query.service.as_deref().unwrap_or("memory"),
            ),
            ReceiptKind::RecallResult,
            ReceiptStatus::Valid,
            ReceiptSubject::new(
                "recall_query",
                query.service.as_deref().unwrap_or("memory"),
                query.service.as_deref().unwrap_or("memory recall"),
            )?,
            ReceiptSummary::new(
                "Recall result produced",
                "The adaptive memory engine returned ranked memory matches for review.",
            )?,
            evidence,
            reasoning,
            Confidence::new(
                matches.first().map_or(0.0, |m| m.confidence),
                SCORING_METHOD,
                if matches.is_empty() {
                    "No memory exceeded the transparent score threshold."
                } else {
                    "Score is deterministic field overlap, not opaque embedding similarity."
                },
            )?,
            Risk::new(
                RiskLevel::Low,
                "Recall is informational and cannot execute remediation or mutate infrastructure.",
            )?,
            &query.generated_at,
        )
    }

    fn feedback_receipt(&self, memory: &MemoryRecord, feedback: &HumanFeedback) -> Result<Receipt> {
        receipt(
            &receipt_id("human_feedback_recorded", feedback.id.as_str()),
            ReceiptKind::HumanFeedbackRecorded,
            ReceiptStatus::Valid,
            ReceiptSubject::new("memory", memory.id.as_str(), memory.title.as_str())?,
            ReceiptSummary::new(
                "Human feedback recorded",
                format!(
                    "Feedback kind '{}' was recorded against memory '{}'.",
                    feedback.kind,
                    memory.id.as_str()
                ),
            )?,
            vec![feedback_evidence(feedback)?],
            vec![ReasoningStep::new(
                1,
                "Record feedback",
                "Engineer feedback was appended to the memory audit trail.",
                "Feedback is available for future memory review",
                0.2,
            )?],
            Confidence::new(
                0.9,
                "human-feedback-recording-v1",
                "Feedback is explicit human input; the engine does not infer approval.",
            )?,
            Risk::new(
                RiskLevel::Low,
                "Feedback recording affects memory only and cannot mutate infrastructure.",
            )?,
            feedback.timestamp.as_str(),
        )
    }

    fn transition_receipt(
        &self,
        memory: &MemoryRecord,
        feedback: &HumanFeedback,
        kind: ReceiptKind,
    ) -> Result<Receipt> {
        receipt(
            &receipt_id(kind.as_str(), memory.id.as_str()),
            kind,
            ReceiptStatus::Valid,
            ReceiptSubject::new("memory", memory.id.as_str(), memory.title.as_str())?,
            ReceiptSummary::new(
                format!("Memory {}", memory.status.as_str()),
                format!(
                    "Human feedback transitioned memory '{}' to '{}'.",
                    memory.id.as_str(),
                    memory.status.as_str()
                ),
            )?,
            vec![feedback_evidence(feedback)?],
            vec![ReasoningStep::new(
                1,
                "Apply memory lifecycle feedback",
                format!(
                    "Feedback '{}' was applied using the memory lifecycle helper.",
                    feedback.kind
                ),
                format!("Memory status is now '{}'", memory.status.as_str()),
                0.3,
            )?],
            Confidence::new(
                memory.confidence.score,
                "memory-feedback-transition-v1",
                "Transition confidence reflects human feedback and original memory confidence.",
            )?,
            Risk::new(
                RiskLevel::Low,
                "Memory lifecycle transition has no infrastructure side effects.",
            )?,
            feedback.timestamp.as_str(),
        )
    }
}

fn validate_candidate_request(request: &MemoryCandidateRequest) -> Result<()> {
    if request.evidence_ids.is_empty() {
        return Err(RivoraError::invalid_value(
            "evidence_ids",
            "candidate memory requires at least one evidence id",
        ));
    }
    if !(0.0..=1.0).contains(&request.confidence) {
        return Err(RivoraError::invalid_value(
            "confidence",
            "confidence must be in [0.0, 1.0]",
        ));
    }
    non_empty(&request.id)?;
    non_empty(&request.service)?;
    non_empty(&request.event_summary)?;
    non_empty(&request.source)?;
    non_empty(&request.source_version)?;
    non_empty(&request.observed_at)?;
    non_empty(&request.learned_at)?;
    Ok(())
}

fn validate_recall_query(query: &RecallQuery) -> Result<()> {
    if query.limit == 0 {
        return Err(RivoraError::invalid_value(
            "limit",
            "limit must be positive",
        ));
    }
    if !(0.0..=1.0).contains(&query.min_score) {
        return Err(RivoraError::invalid_value(
            "min_score",
            "min_score must be in [0.0, 1.0]",
        ));
    }
    non_empty(&query.generated_at)?;
    Ok(())
}

fn validate_feedback_target(record: &MemoryRecord, feedback: &HumanFeedback) -> Result<()> {
    if feedback.target_type != FeedbackTargetType::Memory {
        return Err(RivoraError::invalid_value(
            "feedback_target_type",
            "adaptive memory feedback must target a memory record",
        ));
    }
    if feedback.target_id.as_str() != record.id.as_str() {
        return Err(RivoraError::invalid_value(
            "feedback_target_id",
            "feedback target id does not match memory record id",
        ));
    }
    Ok(())
}

fn record_is_recallable(record: &MemoryRecord, query: &RecallQuery) -> bool {
    if let Some(status) = query.status {
        if record.status != status {
            return false;
        }
    } else if record.status != MemoryStatus::Active
        && !(query.include_candidates && record.status == MemoryStatus::Candidate)
    {
        return false;
    }

    if let Some(kind) = query.kind {
        if record.kind != kind {
            return false;
        }
    }
    if let Some(scope) = query.scope {
        if record.scope != scope {
            return false;
        }
    }
    true
}

fn score_record(
    query: &RecallQuery,
    record: &MemoryRecord,
) -> (RecallScore, Vec<String>, Vec<String>) {
    let mut components = Vec::new();
    let mut reasons = Vec::new();

    if let Some(service) = &query.service {
        if memory_mentions(record, service) {
            push_component(
                &mut components,
                &mut reasons,
                "service",
                0.22,
                format!("same service: {service}"),
            );
        }
    }
    if let Some(kind) = query.kind {
        if record.kind == kind {
            push_component(
                &mut components,
                &mut reasons,
                "kind",
                0.12,
                format!("same memory kind: {}", kind.as_str()),
            );
        }
    }
    if let Some(scope) = query.scope {
        if record.scope == scope {
            push_component(
                &mut components,
                &mut reasons,
                "scope",
                0.1,
                format!("same memory scope: {}", scope.as_str()),
            );
        }
    }

    let symptom_overlap = overlap(&normalize_all(&query.symptoms), &memory_terms(record));
    if !symptom_overlap.is_empty() {
        let contribution = (0.2 + (symptom_overlap.len() as f64 * 0.03)).min(0.3);
        push_component(
            &mut components,
            &mut reasons,
            "symptoms",
            contribution,
            format!("symptom overlap: {}", symptom_overlap.join(", ")),
        );
    }

    let tag_overlap = overlap(&normalize_all(&query.tags), &record_tags(record));
    if !tag_overlap.is_empty() {
        let contribution = (tag_overlap.len() as f64 * 0.04).min(0.12);
        push_component(
            &mut components,
            &mut reasons,
            "tags",
            contribution,
            format!("tag overlap: {}", tag_overlap.join(", ")),
        );
    }

    let evidence_overlap = overlap_strings(&query.evidence_ids, &memory_evidence_refs(record));
    if !evidence_overlap.is_empty() {
        let contribution = (0.08 + (evidence_overlap.len() as f64 * 0.03)).min(0.17);
        push_component(
            &mut components,
            &mut reasons,
            "evidence",
            contribution,
            format!("evidence overlap: {}", evidence_overlap.join(", ")),
        );
    }

    if let Some(source) = &query.source {
        if record.provenance.source.as_str() == source
            || record.provenance.connector_ref.as_deref() == Some(source.as_str())
        {
            push_component(
                &mut components,
                &mut reasons,
                "source",
                0.05,
                format!("same source: {source}"),
            );
        }
    }

    let text_overlap = overlap(&query_text_terms(query), &record_text_terms(record));
    if !text_overlap.is_empty() {
        let contribution = (text_overlap.len() as f64 * 0.025).min(0.12);
        push_component(
            &mut components,
            &mut reasons,
            "text",
            contribution,
            format!("text overlap: {}", text_overlap.join(", ")),
        );
    }

    match record.status {
        MemoryStatus::Active => push_component(
            &mut components,
            &mut reasons,
            "status",
            0.05,
            "active memory can influence recall".to_string(),
        ),
        MemoryStatus::Candidate if query.include_candidates => push_component(
            &mut components,
            &mut reasons,
            "status",
            0.02,
            "candidate memory surfaced for review".to_string(),
        ),
        _ => {}
    }

    let evidence_refs = memory_evidence_refs(record);
    let value = clamp01(components.iter().map(|c| c.contribution).sum::<f64>());
    (
        RecallScore {
            value,
            method: SCORING_METHOD.to_string(),
            components,
        },
        reasons,
        evidence_refs,
    )
}

fn push_component(
    components: &mut Vec<RecallScoreComponent>,
    reasons: &mut Vec<String>,
    name: &str,
    contribution: f64,
    reason: String,
) {
    components.push(RecallScoreComponent {
        name: name.to_string(),
        contribution,
        reason: reason.clone(),
    });
    reasons.push(reason);
}

#[allow(clippy::too_many_arguments)]
fn receipt(
    id: &str,
    kind: ReceiptKind,
    status: ReceiptStatus,
    subject: ReceiptSubject,
    summary: ReceiptSummary,
    evidence: Vec<Evidence>,
    reasoning: Vec<ReasoningStep>,
    confidence: Confidence,
    risk: Risk,
    timestamp: &str,
) -> Result<Receipt> {
    Receipt::builder()
        .id(id)
        .kind(kind)
        .status(status)
        .subject(subject)
        .summary(summary)
        .evidence(evidence)
        .reasoning(reasoning)
        .confidence(confidence)
        .risk(risk)
        .provenance(ReceiptProvenance::new(ENGINE_NAME, ENGINE_VERSION)?)
        .timestamps(ReceiptTimestamps::new(timestamp)?)
        .version(ReceiptVersion::new(Version::new(1, 0, 0)))
        .build()
}

fn evidence_from_ids(
    ids: &[String],
    provider: &str,
    version: &str,
    observed_at: &str,
    description: &str,
    contribution: f64,
) -> Result<Vec<Evidence>> {
    ids.iter()
        .map(|id| {
            Evidence::new(
                EvidenceKind::Observation,
                evidence_source(provider, version)?,
                format!("Evidence {id}"),
                description,
                observed_at,
                contribution.clamp(0.05, 1.0),
            )
            .map(|e| e.with_raw_ref(id))
        })
        .collect()
}

fn evidence_source(provider: &str, version: &str) -> Result<EvidenceSource> {
    Ok(EvidenceSource {
        provider: non_empty(provider)?,
        version: non_empty(version)?,
    })
}

fn feedback_evidence(feedback: &HumanFeedback) -> Result<Evidence> {
    Evidence::new(
        EvidenceKind::Annotation,
        evidence_source(feedback.source.as_str(), ENGINE_VERSION)?,
        format!("Feedback {}", feedback.id.as_str()),
        feedback_note(feedback),
        feedback.timestamp.as_str(),
        0.6,
    )
    .map(|e| e.with_raw_ref(feedback.id.as_str()))
}

fn memory_confidence(score: f64, explanation: &str, at: &str) -> Result<MemoryConfidence> {
    MemoryConfidence::new(score, explanation, at)
}

fn apply_confidence_adjustment(record: &mut MemoryRecord, adjustment: Option<f64>, default: f64) {
    let delta = adjustment.unwrap_or(default);
    record.confidence.score = clamp01(record.confidence.score + delta);
    record.confidence.level = MemoryConfidenceLevel::from_score(record.confidence.score);
}

fn candidate_body(request: &MemoryCandidateRequest) -> String {
    let symptoms = if request.symptoms.is_empty() {
        "none supplied".to_string()
    } else {
        request.symptoms.join(", ")
    };
    format!(
        "Service: {}\nSymptoms: {}\nSummary: {}",
        request.service, symptoms, request.event_summary
    )
}

fn feedback_note(feedback: &HumanFeedback) -> &str {
    feedback
        .note
        .as_ref()
        .map_or("human feedback recorded", |note| note.as_str())
}

fn correction_text(feedback: &HumanFeedback) -> &str {
    feedback.correction_text.as_ref().map_or_else(
        || {
            feedback
                .note
                .as_ref()
                .map_or("memory corrected by human feedback", |note| note.as_str())
        },
        |text| text.as_str(),
    )
}

fn receipt_id(prefix: &str, id: &str) -> String {
    let sanitized: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("receipt_{prefix}_{sanitized}")
}

fn non_empty(value: &str) -> Result<NonEmptyString> {
    NonEmptyString::new(value.to_string())
}

fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

fn memory_mentions(record: &MemoryRecord, needle: &str) -> bool {
    let normalized = normalize(needle);
    record
        .subject_refs
        .iter()
        .any(|s| normalize(s.as_str()) == normalized)
        || record
            .metadata
            .tags
            .iter()
            .any(|s| normalize(s.as_str()) == normalized)
        || record
            .labels
            .values()
            .any(|s| normalize(s.as_str()) == normalized)
        || normalize(record.title.as_str()).contains(&normalized)
        || normalize(record.body.as_str()).contains(&normalized)
}

fn record_tags(record: &MemoryRecord) -> BTreeSet<String> {
    let mut tags = BTreeSet::new();
    tags.extend(record.metadata.tags.iter().map(|s| normalize(s.as_str())));
    tags.extend(record.labels.keys().map(|s| normalize(s.as_str())));
    tags.extend(record.labels.values().map(|s| normalize(s.as_str())));
    tags
}

fn memory_terms(record: &MemoryRecord) -> BTreeSet<String> {
    let mut terms = record_tags(record);
    terms.extend(record_text_terms(record));
    terms.extend(record.subject_refs.iter().map(|s| normalize(s.as_str())));
    terms
}

fn record_text_terms(record: &MemoryRecord) -> BTreeSet<String> {
    let text = format!("{} {}", record.title.as_str(), record.body.as_str());
    tokenize(&text)
}

fn query_text_terms(query: &RecallQuery) -> BTreeSet<String> {
    let mut terms = normalize_all(&query.symptoms);
    terms.extend(normalize_all(&query.tags));
    if let Some(service) = &query.service {
        terms.extend(tokenize(service));
    }
    terms
}

fn memory_evidence_refs(record: &MemoryRecord) -> Vec<String> {
    let mut refs = BTreeSet::new();
    refs.extend(record.graph_node_ids.iter().cloned());
    refs.extend(record.graph_edge_ids.iter().cloned());
    refs.extend(record.receipt_ids.iter().cloned());
    refs.extend(record.provenance.graph_node_ids.iter().cloned());
    refs.extend(record.provenance.graph_edge_ids.iter().cloned());
    if let Some(receipt_id) = &record.provenance.receipt_id {
        refs.insert(receipt_id.clone());
    }
    refs.into_iter().collect()
}

fn normalize_all(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .flat_map(|value| tokenize(value))
        .filter(|value| !value.is_empty())
        .collect()
}

fn overlap(left: &BTreeSet<String>, right: &BTreeSet<String>) -> Vec<String> {
    left.intersection(right).cloned().collect()
}

fn overlap_strings(left: &[String], right: &[String]) -> Vec<String> {
    let left: BTreeSet<String> = left.iter().map(|s| normalize(s)).collect();
    let right: BTreeSet<String> = right.iter().map(|s| normalize(s)).collect();
    overlap(&left, &right)
}

fn tokenize(value: &str) -> BTreeSet<String> {
    value
        .split(|c: char| !c.is_ascii_alphanumeric())
        .map(normalize)
        .filter(|token| token.len() > 2)
        .collect()
}

fn normalize(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rivora_memory::{FeedbackSource, MemoryIndex};
    use rivora_receipts::validation::validate_receipt;

    fn candidate_request(
        id: &str,
        service: &str,
        symptom: &str,
        confidence: f64,
    ) -> MemoryCandidateRequest {
        MemoryCandidateRequest {
            id: id.to_string(),
            kind: MemoryKind::IncidentLearning,
            scope: MemoryScope::Service,
            service: service.to_string(),
            symptoms: vec![symptom.to_string(), "error rate".to_string()],
            event_summary: format!("{service} showed {symptom} after deployment"),
            evidence_ids: vec![format!("evidence-{id}")],
            source: "github".to_string(),
            source_version: "0.1.0".to_string(),
            confidence,
            observed_at: "2026-06-25T12:00:00Z".to_string(),
            learned_at: "2026-06-25T12:01:00Z".to_string(),
        }
    }

    fn active_memory(id: &str, service: &str, symptom: &str) -> MemoryRecord {
        let engine = AdaptiveMemoryEngine::new();
        let mut result = engine
            .propose_candidate(candidate_request(id, service, symptom, 0.75))
            .unwrap()
            .memory;
        result.approve();
        result
    }

    fn feedback(id: &str, target: &MemoryRecord, kind: FeedbackKind) -> HumanFeedback {
        HumanFeedback::builder()
            .id(id)
            .target_id(target.id.as_str())
            .target_type(FeedbackTargetType::Memory)
            .actor("sergio")
            .source(FeedbackSource::Cli)
            .kind(kind)
            .note("reviewed by on-call")
            .correction_text("corrected service scope")
            .timestamp("2026-06-25T13:00:00Z")
            .build()
            .unwrap()
    }

    fn assert_receipts_are_valid(receipts: &[Receipt]) {
        for receipt in receipts {
            validate_receipt(receipt).unwrap();
        }
    }

    #[test]
    fn candidate_generation_creates_candidate_memory() {
        let engine = AdaptiveMemoryEngine::new();
        let result = engine
            .propose_candidate(candidate_request(
                "mem-candidate-1",
                "payments",
                "latency spike",
                0.65,
            ))
            .unwrap();

        assert_eq!(result.memory.status, MemoryStatus::Candidate);
        assert_eq!(result.memory.kind, MemoryKind::IncidentLearning);
        assert_eq!(result.memory.scope, MemoryScope::Service);
        assert_eq!(result.memory.subject_refs[0].as_str(), "payments");
        assert_eq!(
            result.memory.graph_node_ids,
            vec!["evidence-mem-candidate-1"]
        );
        assert_eq!(
            result.memory.provenance.receipt_id.as_deref(),
            Some("receipt_memory_candidate_created_mem-candidate-1")
        );
        assert_receipts_are_valid(&[result.receipt]);
    }

    #[test]
    fn candidate_generation_emits_memory_candidate_created_receipt() {
        let engine = AdaptiveMemoryEngine::new();
        let result = engine
            .propose_candidate(candidate_request(
                "mem-candidate-2",
                "payments",
                "latency spike",
                0.65,
            ))
            .unwrap();

        assert_eq!(result.receipt.kind, ReceiptKind::MemoryCandidateCreated);
        assert!(!result.receipt.evidence.is_empty());
        assert!(!result.receipt.has_mutating_actions());
        assert_receipts_are_valid(&[result.receipt]);
    }

    #[test]
    fn recall_ranks_exact_service_and_symptom_matches_higher() {
        let engine = AdaptiveMemoryEngine::new();
        let exact = active_memory("mem-exact", "payments", "latency spike");
        let same_service = active_memory("mem-same-service", "payments", "disk full");
        let other_service = active_memory("mem-other", "search", "latency spike");

        let result = engine
            .recall(
                RecallQuery {
                    service: Some("payments".to_string()),
                    kind: Some(MemoryKind::IncidentLearning),
                    scope: Some(MemoryScope::Service),
                    symptoms: vec!["latency spike".to_string()],
                    limit: 3,
                    generated_at: "2026-06-25T14:00:00Z".to_string(),
                    ..RecallQuery::default()
                },
                &[same_service, other_service, exact],
            )
            .unwrap();

        assert_eq!(result.matches[0].memory_id, "mem-exact");
        assert!(result.matches[0].score.value > result.matches[1].score.value);
        assert_receipts_are_valid(&[result.receipt]);
    }

    #[test]
    fn recall_explains_match_reasons_and_evidence_refs() {
        let engine = AdaptiveMemoryEngine::new();
        let memory = active_memory("mem-explain", "payments", "latency spike");

        let result = engine
            .recall(
                RecallQuery {
                    service: Some("payments".to_string()),
                    kind: Some(MemoryKind::IncidentLearning),
                    scope: Some(MemoryScope::Service),
                    symptoms: vec!["latency spike".to_string()],
                    evidence_ids: vec!["evidence-mem-explain".to_string()],
                    limit: 1,
                    generated_at: "2026-06-25T14:00:00Z".to_string(),
                    ..RecallQuery::default()
                },
                &[memory],
            )
            .unwrap();

        let first = &result.matches[0];
        assert!(first
            .matched_reasons
            .iter()
            .any(|reason| reason.contains("same service")));
        assert!(first
            .matched_reasons
            .iter()
            .any(|reason| reason.contains("symptom overlap")));
        assert!(first
            .evidence_refs
            .iter()
            .any(|reference| reference == "evidence-mem-explain"));
        assert_eq!(result.receipt.kind, ReceiptKind::RecallResult);
        assert_receipts_are_valid(&[result.receipt]);
    }

    #[test]
    fn recall_handles_low_or_no_matches_safely() {
        let engine = AdaptiveMemoryEngine::new();
        let memory = active_memory("mem-no-match", "payments", "latency spike");

        let result = engine
            .recall(
                RecallQuery {
                    service: Some("inventory".to_string()),
                    symptoms: vec!["queue saturation".to_string()],
                    min_score: 0.4,
                    generated_at: "2026-06-25T14:00:00Z".to_string(),
                    ..RecallQuery::default()
                },
                &[memory],
            )
            .unwrap();

        assert!(result.matches.is_empty());
        assert_eq!(result.receipt.kind, ReceiptKind::RecallResult);
        assert!(!result.receipt.evidence.is_empty());
        assert!(!result.receipt.has_mutating_actions());
        assert_receipts_are_valid(&[result.receipt]);
    }

    #[test]
    fn approve_feedback_updates_memory_status_to_active() {
        let engine = AdaptiveMemoryEngine::new();
        let memory = engine
            .propose_candidate(candidate_request(
                "mem-approve",
                "payments",
                "latency spike",
                0.6,
            ))
            .unwrap()
            .memory;

        let result = engine
            .apply_feedback(
                &memory,
                feedback("feedback-approve", &memory, FeedbackKind::Approved),
            )
            .unwrap();

        assert_eq!(result.memory.status, MemoryStatus::Active);
        assert!(result
            .receipts
            .iter()
            .any(|receipt| receipt.kind == ReceiptKind::MemoryApproved));
        assert!(result
            .receipts
            .iter()
            .any(|receipt| receipt.kind == ReceiptKind::HumanFeedbackRecorded));
        assert_receipts_are_valid(&result.receipts);
    }

    #[test]
    fn reject_feedback_updates_memory_status_to_rejected() {
        let engine = AdaptiveMemoryEngine::new();
        let memory = engine
            .propose_candidate(candidate_request(
                "mem-reject",
                "payments",
                "latency spike",
                0.6,
            ))
            .unwrap()
            .memory;

        let result = engine
            .apply_feedback(
                &memory,
                feedback("feedback-reject", &memory, FeedbackKind::Rejected),
            )
            .unwrap();

        assert_eq!(result.memory.status, MemoryStatus::Rejected);
        assert!(result
            .receipts
            .iter()
            .any(|receipt| receipt.kind == ReceiptKind::MemoryRejected));
        assert_receipts_are_valid(&result.receipts);
    }

    #[test]
    fn correct_feedback_updates_memory_status_to_corrected() {
        let engine = AdaptiveMemoryEngine::new();
        let memory = active_memory("mem-correct", "payments", "latency spike");

        let result = engine
            .apply_feedback(
                &memory,
                feedback("feedback-correct", &memory, FeedbackKind::Corrected),
            )
            .unwrap();

        assert_eq!(result.memory.status, MemoryStatus::Corrected);
        assert!(result
            .receipts
            .iter()
            .any(|receipt| receipt.kind == ReceiptKind::MemoryCorrected));
        assert_receipts_are_valid(&result.receipts);
    }

    #[test]
    fn useful_and_needs_more_evidence_feedback_emit_feedback_receipts() {
        let engine = AdaptiveMemoryEngine::new();
        let memory = active_memory("mem-feedback", "payments", "latency spike");

        for kind in [
            FeedbackKind::Useful,
            FeedbackKind::NotUseful,
            FeedbackKind::NeedsMoreEvidence,
        ] {
            let result = engine
                .apply_feedback(
                    &memory,
                    feedback(&format!("feedback-{}", kind.as_str()), &memory, kind),
                )
                .unwrap();
            assert_eq!(result.receipts.len(), 1);
            assert_eq!(result.receipts[0].kind, ReceiptKind::HumanFeedbackRecorded);
            assert_receipts_are_valid(&result.receipts);
        }
    }

    #[test]
    fn engine_never_emits_infrastructure_action_recommendations() {
        let engine = AdaptiveMemoryEngine::new();
        let low_confidence = engine
            .propose_candidate(candidate_request(
                "mem-low",
                "payments",
                "latency spike",
                0.2,
            ))
            .unwrap();

        for recommendation in low_confidence.recommendations {
            assert!(!recommendation.mutates_infrastructure());
            let action = recommendation.to_read_only_action().unwrap();
            assert!(!action.mutates_infrastructure);
            assert!(action.read_only);
        }
        assert_receipts_are_valid(&[low_confidence.receipt]);
    }

    #[test]
    fn active_memories_only_are_recalled_by_default() {
        let engine = AdaptiveMemoryEngine::new();
        let active = active_memory("mem-active", "payments", "latency spike");
        let candidate = engine
            .propose_candidate(candidate_request(
                "mem-review",
                "payments",
                "latency spike",
                0.75,
            ))
            .unwrap()
            .memory;

        let result = engine
            .recall(
                RecallQuery {
                    service: Some("payments".to_string()),
                    symptoms: vec!["latency spike".to_string()],
                    generated_at: "2026-06-25T14:00:00Z".to_string(),
                    ..RecallQuery::default()
                },
                &[candidate, active],
            )
            .unwrap();

        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].memory_id, "mem-active");
        assert_receipts_are_valid(&[result.receipt]);
    }

    #[test]
    fn recall_works_with_memory_index_snapshot_records() {
        let engine = AdaptiveMemoryEngine::new();
        let mut index = MemoryIndex::new();
        index
            .add_record(active_memory("mem-indexed", "payments", "latency spike"))
            .unwrap();
        let snapshot = index.snapshot();

        let result = engine
            .recall(
                RecallQuery {
                    service: Some("payments".to_string()),
                    symptoms: vec!["latency spike".to_string()],
                    generated_at: "2026-06-25T14:00:00Z".to_string(),
                    ..RecallQuery::default()
                },
                &snapshot.records,
            )
            .unwrap();

        assert_eq!(result.matches.len(), 1);
        assert_receipts_are_valid(&[result.receipt]);
    }
}
