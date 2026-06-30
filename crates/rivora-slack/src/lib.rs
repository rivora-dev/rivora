//! Pure Slack reliability memory surface for Open Rivora.
//!
//! This crate contains typed Slack interaction contracts, deterministic
//! mention parsing, compact message/card rendering, and a bridge from Slack
//! actions to the pure [`rivora_adaptive::AdaptiveMemoryEngine`]. It performs
//! no Slack API calls and has no infrastructure execution path.

use rivora_adaptive::{
    AdaptiveMemoryEngine, MemoryCandidateRequest, RecallMatch, RecallQuery, RecallResult,
};
use rivora_errors::{Result, RivoraError};
use rivora_memory::{
    FeedbackKind, FeedbackSource, FeedbackTargetType, HumanFeedback, MemoryKind, MemoryRecord,
    MemoryScope, MemoryStatus,
};
use rivora_receipts::Receipt;
use serde::{Deserialize, Serialize};

const SLACK_SOURCE: &str = "slack";
const SLACK_SOURCE_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlackMentionRequest {
    pub channel_id: String,
    pub user_id: String,
    pub text: String,
    pub timestamp: String,
    pub thread_ts: Option<String>,
    pub service: Option<String>,
    pub topic: Option<String>,
    pub evidence_ids: Vec<String>,
    pub memory_records: Vec<MemoryRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlackMemoryAnswer {
    pub label: SlackMessageLabel,
    pub text: String,
    pub recall_cards: Vec<SlackRecallCard>,
    pub candidate_card: Option<SlackMemoryCandidateCard>,
    pub actions: Vec<SlackMessageAction>,
    pub receipt_ids: Vec<String>,
}

impl SlackMemoryAnswer {
    #[must_use]
    pub fn renders_infrastructure_mutation_actions(&self) -> bool {
        self.actions
            .iter()
            .any(SlackMessageAction::mutates_infrastructure)
            || self
                .recall_cards
                .iter()
                .flat_map(|card| card.actions.iter())
                .any(SlackMessageAction::mutates_infrastructure)
            || self
                .candidate_card
                .as_ref()
                .into_iter()
                .flat_map(|card| card.actions.iter())
                .any(SlackMessageAction::mutates_infrastructure)
    }

    #[must_use]
    pub fn render_text(&self) -> String {
        let mut lines = vec![format!("{}: {}", self.label.as_str(), self.text)];
        for card in &self.recall_cards {
            lines.push(card.render_text());
        }
        if let Some(card) = &self.candidate_card {
            lines.push(card.render_text());
        }
        lines.join("\n")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlackRecallCard {
    pub label: SlackMessageLabel,
    pub memory_id: String,
    pub title: String,
    pub score: f64,
    pub confidence: f64,
    pub match_reasons: Vec<String>,
    pub evidence_refs: Vec<String>,
    pub actions: Vec<SlackMessageAction>,
}

impl SlackRecallCard {
    #[must_use]
    pub fn from_match(recall_match: &RecallMatch) -> Self {
        Self {
            label: SlackMessageLabel::SimilarMemory,
            memory_id: recall_match.memory_id.clone(),
            title: recall_match.memory.title.as_str().to_string(),
            score: recall_match.score.value,
            confidence: recall_match.confidence,
            match_reasons: recall_match.matched_reasons.clone(),
            evidence_refs: recall_match.evidence_refs.clone(),
            actions: recall_actions(),
        }
    }

    #[must_use]
    pub fn render_text(&self) -> String {
        let reasons = if self.match_reasons.is_empty() {
            "no strong reasons recorded".to_string()
        } else {
            self.match_reasons.join("; ")
        };
        let evidence = if self.evidence_refs.is_empty() {
            "none".to_string()
        } else {
            self.evidence_refs.join(", ")
        };
        format!(
            "Similar memory: {} ({})\nscore: {:.2} confidence: {:.2}\nreasons: {}\nevidence: {}",
            self.title, self.memory_id, self.score, self.confidence, reasons, evidence
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlackMemoryCandidateCard {
    pub label: SlackMessageLabel,
    pub memory_id: String,
    pub summary: String,
    pub confidence: f64,
    pub evidence_refs: Vec<String>,
    pub proposed_status: MemoryStatus,
    pub actions: Vec<SlackMessageAction>,
}

impl SlackMemoryCandidateCard {
    #[must_use]
    pub fn from_record(record: &MemoryRecord) -> Self {
        Self {
            label: SlackMessageLabel::MemoryCandidate,
            memory_id: record.id.as_str().to_string(),
            summary: record.body.as_str().to_string(),
            confidence: record.confidence.score,
            evidence_refs: memory_evidence_refs(record),
            proposed_status: record.status,
            actions: candidate_actions(),
        }
    }

    #[must_use]
    pub fn render_text(&self) -> String {
        let evidence = if self.evidence_refs.is_empty() {
            "none".to_string()
        } else {
            self.evidence_refs.join(", ")
        };
        format!(
            "Memory candidate: {}\nstatus: {} confidence: {:.2}\nevidence: {}",
            self.summary,
            self.proposed_status.as_str(),
            self.confidence,
            evidence
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlackFeedbackAction {
    pub action: SlackFeedbackActionKind,
    pub actor_id: String,
    pub channel_id: String,
    pub timestamp: String,
    pub target_memory: MemoryRecord,
    pub note: Option<String>,
    pub correction_text: Option<String>,
}

impl SlackFeedbackAction {
    pub fn to_human_feedback(&self) -> Result<HumanFeedback> {
        let mut builder = HumanFeedback::builder()
            .id(feedback_id(self))
            .target_id(self.target_memory.id.as_str())
            .target_type(FeedbackTargetType::Memory)
            .actor(self.actor_id.clone())
            .source(FeedbackSource::Slack)
            .kind(self.action.feedback_kind())
            .timestamp(self.timestamp.clone());

        if let Some(note) = &self.note {
            builder = builder.note(note.clone());
        }
        if let Some(correction_text) = &self.correction_text {
            builder = builder.correction_text(correction_text.clone());
        }

        builder.build()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlackActionResponse {
    pub label: SlackMessageLabel,
    pub text: String,
    pub updated_memory: MemoryRecord,
    pub receipts: Vec<Receipt>,
    pub actions: Vec<SlackMessageAction>,
}

impl SlackActionResponse {
    #[must_use]
    pub fn renders_infrastructure_mutation_actions(&self) -> bool {
        self.actions
            .iter()
            .any(SlackMessageAction::mutates_infrastructure)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackFeedbackActionKind {
    Remember,
    Reject,
    Correct,
    NotUseful,
    NeedsMoreEvidence,
}

impl SlackFeedbackActionKind {
    #[must_use]
    pub fn feedback_kind(self) -> FeedbackKind {
        match self {
            Self::Remember => FeedbackKind::Approved,
            Self::Reject => FeedbackKind::Rejected,
            Self::Correct => FeedbackKind::Corrected,
            Self::NotUseful => FeedbackKind::NotUseful,
            Self::NeedsMoreEvidence => FeedbackKind::NeedsMoreEvidence,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Remember => "remember",
            Self::Reject => "reject",
            Self::Correct => "correct",
            Self::NotUseful => "not_useful",
            Self::NeedsMoreEvidence => "needs_more_evidence",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackMessageLabel {
    Observation,
    SimilarMemory,
    MemoryCandidate,
    Recommendation,
    NeedsReview,
}

impl SlackMessageLabel {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Observation => "Observation",
            Self::SimilarMemory => "Similar memory",
            Self::MemoryCandidate => "Memory candidate",
            Self::Recommendation => "Recommendation",
            Self::NeedsReview => "Needs review",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlackMessageAction {
    pub id: SlackFeedbackActionKind,
    pub label: String,
}

impl SlackMessageAction {
    #[must_use]
    pub fn new(id: SlackFeedbackActionKind, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
        }
    }

    #[must_use]
    pub fn mutates_infrastructure(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlackMentionIntent {
    WhatChanged,
    SeenBefore,
    Recall { query: String },
    RememberCandidate,
    Help,
}

#[derive(Debug, Clone, Default)]
pub struct SlackReliabilityMemoryApp {
    engine: AdaptiveMemoryEngine,
}

impl SlackReliabilityMemoryApp {
    #[must_use]
    pub fn new() -> Self {
        Self {
            engine: AdaptiveMemoryEngine::new(),
        }
    }

    pub fn handle_mention(&self, request: SlackMentionRequest) -> Result<SlackMemoryAnswer> {
        match parse_mention(&request.text) {
            SlackMentionIntent::WhatChanged => Ok(self.what_changed_answer(&request)),
            SlackMentionIntent::SeenBefore => self.recall_answer(&request, None),
            SlackMentionIntent::Recall { query } => self.recall_answer(&request, Some(query)),
            SlackMentionIntent::RememberCandidate => self.memory_candidate_answer(&request),
            SlackMentionIntent::Help => Ok(help_answer()),
        }
    }

    pub fn handle_feedback_action(
        &self,
        action: SlackFeedbackAction,
    ) -> Result<SlackActionResponse> {
        let feedback = action.to_human_feedback()?;
        let result = self
            .engine
            .apply_feedback(&action.target_memory, feedback)?;
        let status = result.memory.status.as_str();
        let label = match action.action {
            SlackFeedbackActionKind::Remember => SlackMessageLabel::MemoryCandidate,
            SlackFeedbackActionKind::Reject
            | SlackFeedbackActionKind::Correct
            | SlackFeedbackActionKind::NeedsMoreEvidence => SlackMessageLabel::NeedsReview,
            SlackFeedbackActionKind::NotUseful => SlackMessageLabel::SimilarMemory,
        };
        Ok(SlackActionResponse {
            label,
            text: format!(
                "{}: memory {} is now {}. Receipts: {}.",
                label.as_str(),
                result.memory.id.as_str(),
                status,
                result
                    .receipts
                    .iter()
                    .map(|receipt| receipt.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            updated_memory: result.memory.clone(),
            receipts: result.receipts,
            actions: actions_for_status(result.memory.status),
        })
    }

    fn what_changed_answer(&self, request: &SlackMentionRequest) -> SlackMemoryAnswer {
        SlackMemoryAnswer {
            label: SlackMessageLabel::Observation,
            text: format!(
                "I can explain from memory, but Phase 9 has no connectors yet. Ask `recall {}` to check similar memories, or `what should we remember` to draft a candidate.",
                request
                    .service
                    .as_deref()
                    .or(request.topic.as_deref())
                    .unwrap_or("<service/topic>")
            ),
            recall_cards: Vec::new(),
            candidate_card: None,
            actions: Vec::new(),
            receipt_ids: Vec::new(),
        }
    }

    fn recall_answer(
        &self,
        request: &SlackMentionRequest,
        explicit_query: Option<String>,
    ) -> Result<SlackMemoryAnswer> {
        let query_text = explicit_query
            .or_else(|| request.topic.clone())
            .or_else(|| request.service.clone())
            .unwrap_or_else(|| request.text.clone());
        let recall_query = RecallQuery {
            service: request.service.clone().or_else(|| first_token(&query_text)),
            kind: None,
            scope: None,
            symptoms: split_terms(&query_text),
            tags: split_terms(&query_text),
            evidence_ids: request.evidence_ids.clone(),
            source: Some(SLACK_SOURCE.to_string()),
            status: None,
            include_candidates: false,
            limit: 3,
            min_score: 0.01,
            generated_at: request.timestamp.clone(),
        };
        let result = self.engine.recall(recall_query, &request.memory_records)?;
        Ok(recall_answer_from_result(result))
    }

    fn memory_candidate_answer(&self, request: &SlackMentionRequest) -> Result<SlackMemoryAnswer> {
        let service = request
            .service
            .clone()
            .or_else(|| request.topic.clone())
            .unwrap_or_else(|| "slack-thread".to_string());
        let evidence_ids = if request.evidence_ids.is_empty() {
            vec![format!(
                "slack:{}:{}",
                request.channel_id,
                request.thread_ts.as_deref().unwrap_or(&request.timestamp)
            )]
        } else {
            request.evidence_ids.clone()
        };
        let summary = request
            .topic
            .clone()
            .unwrap_or_else(|| request.text.clone());
        let candidate = self.engine.propose_candidate(MemoryCandidateRequest {
            id: format!(
                "slack-memory-{}",
                sanitize_id(&format!("{}-{}", request.channel_id, request.timestamp))
            ),
            kind: MemoryKind::OperationalNote,
            scope: if request.service.is_some() {
                MemoryScope::Service
            } else {
                MemoryScope::Team
            },
            service,
            symptoms: split_terms(&summary),
            event_summary: summary,
            evidence_ids,
            source: SLACK_SOURCE.to_string(),
            source_version: SLACK_SOURCE_VERSION.to_string(),
            confidence: 0.4,
            observed_at: request.timestamp.clone(),
            learned_at: request.timestamp.clone(),
        })?;
        let card = SlackMemoryCandidateCard::from_record(&candidate.memory);
        Ok(SlackMemoryAnswer {
            label: SlackMessageLabel::MemoryCandidate,
            text: "Drafted a candidate memory for team review. It is not active until remembered."
                .to_string(),
            recall_cards: Vec::new(),
            candidate_card: Some(card),
            actions: candidate_actions(),
            receipt_ids: vec![candidate.receipt.id.as_str().to_string()],
        })
    }
}

pub fn parse_mention(text: &str) -> SlackMentionIntent {
    let normalized = normalize(text);
    if normalized.contains("what changed") {
        SlackMentionIntent::WhatChanged
    } else if normalized.contains("have we seen") {
        SlackMentionIntent::SeenBefore
    } else if let Some(rest) = normalized.strip_prefix("recall ") {
        SlackMentionIntent::Recall {
            query: rest.trim().to_string(),
        }
    } else if normalized == "recall" {
        SlackMentionIntent::SeenBefore
    } else if normalized.contains("what should we remember") {
        SlackMentionIntent::RememberCandidate
    } else {
        SlackMentionIntent::Help
    }
}

fn recall_answer_from_result(result: RecallResult) -> SlackMemoryAnswer {
    let receipt_ids = vec![result.receipt.id.as_str().to_string()];
    let recall_cards: Vec<SlackRecallCard> = result
        .matches
        .iter()
        .map(SlackRecallCard::from_match)
        .collect();
    if recall_cards.is_empty() {
        SlackMemoryAnswer {
            label: SlackMessageLabel::SimilarMemory,
            text: "I did not find a strong similar memory. This is safe to treat as unknown, not as evidence that it never happened.".to_string(),
            recall_cards,
            candidate_card: None,
            actions: vec![SlackMessageAction::new(
                SlackFeedbackActionKind::NeedsMoreEvidence,
                "Needs more evidence",
            )],
            receipt_ids,
        }
    } else {
        SlackMemoryAnswer {
            label: SlackMessageLabel::SimilarMemory,
            text: format!("Found {} similar memory match(es).", recall_cards.len()),
            recall_cards,
            candidate_card: None,
            actions: recall_actions(),
            receipt_ids,
        }
    }
}

fn help_answer() -> SlackMemoryAnswer {
    SlackMemoryAnswer {
        label: SlackMessageLabel::NeedsReview,
        text: "Try `what changed`, `have we seen this before`, `recall <service/topic>`, or `what should we remember`.".to_string(),
        recall_cards: Vec::new(),
        candidate_card: None,
        actions: Vec::new(),
        receipt_ids: Vec::new(),
    }
}

fn recall_actions() -> Vec<SlackMessageAction> {
    vec![
        SlackMessageAction::new(SlackFeedbackActionKind::Remember, "Remember this"),
        SlackMessageAction::new(SlackFeedbackActionKind::NotUseful, "Not useful"),
        SlackMessageAction::new(SlackFeedbackActionKind::Correct, "Correct"),
        SlackMessageAction::new(
            SlackFeedbackActionKind::NeedsMoreEvidence,
            "Needs more evidence",
        ),
    ]
}

fn candidate_actions() -> Vec<SlackMessageAction> {
    vec![
        SlackMessageAction::new(SlackFeedbackActionKind::Remember, "Remember"),
        SlackMessageAction::new(SlackFeedbackActionKind::Reject, "Reject"),
        SlackMessageAction::new(SlackFeedbackActionKind::Correct, "Correct"),
    ]
}

fn actions_for_status(status: MemoryStatus) -> Vec<SlackMessageAction> {
    match status {
        MemoryStatus::Candidate => candidate_actions(),
        MemoryStatus::Active => recall_actions(),
        MemoryStatus::Rejected | MemoryStatus::Corrected => vec![SlackMessageAction::new(
            SlackFeedbackActionKind::NeedsMoreEvidence,
            "Needs more evidence",
        )],
        _ => Vec::new(),
    }
}

fn feedback_id(action: &SlackFeedbackAction) -> String {
    format!(
        "feedback-slack-{}-{}-{}",
        action.action.as_str(),
        sanitize_id(action.target_memory.id.as_str()),
        sanitize_id(&action.timestamp)
    )
}

fn memory_evidence_refs(record: &MemoryRecord) -> Vec<String> {
    let mut refs = record.graph_node_ids.clone();
    refs.extend(record.graph_edge_ids.clone());
    refs.extend(record.receipt_ids.clone());
    refs.extend(record.provenance.graph_node_ids.clone());
    refs.extend(record.provenance.graph_edge_ids.clone());
    if let Some(receipt_id) = &record.provenance.receipt_id {
        refs.push(receipt_id.clone());
    }
    refs.sort();
    refs.dedup();
    refs
}

fn split_terms(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
        .map(str::trim)
        .filter(|term| term.len() > 2)
        .map(str::to_ascii_lowercase)
        .collect()
}

fn first_token(text: &str) -> Option<String> {
    split_terms(text).into_iter().next()
}

fn normalize(text: &str) -> String {
    text.trim().to_ascii_lowercase()
}

fn sanitize_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

pub fn validate_no_mutating_actions(actions: &[SlackMessageAction]) -> Result<()> {
    if actions
        .iter()
        .any(SlackMessageAction::mutates_infrastructure)
    {
        Err(RivoraError::invalid_value(
            "slack_actions",
            "Slack memory actions must not mutate infrastructure",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rivora_adaptive::MemoryCandidateRequest;

    fn request(text: &str, memories: Vec<MemoryRecord>) -> SlackMentionRequest {
        SlackMentionRequest {
            channel_id: "C123".to_string(),
            user_id: "U123".to_string(),
            text: text.to_string(),
            timestamp: "2026-06-25T12:00:00Z".to_string(),
            thread_ts: Some("1719331200.000100".to_string()),
            service: Some("payments".to_string()),
            topic: Some("payments latency spike".to_string()),
            evidence_ids: vec!["slack-thread-1".to_string()],
            memory_records: memories,
        }
    }

    fn active_memory(id: &str, service: &str, symptom: &str) -> MemoryRecord {
        let engine = AdaptiveMemoryEngine::new();
        let mut memory = engine
            .propose_candidate(MemoryCandidateRequest {
                id: id.to_string(),
                kind: MemoryKind::IncidentLearning,
                scope: MemoryScope::Service,
                service: service.to_string(),
                symptoms: vec![symptom.to_string(), "latency".to_string()],
                event_summary: format!("{service} had {symptom} after deploy"),
                evidence_ids: vec![format!("evidence-{id}")],
                source: "github".to_string(),
                source_version: "0.1.0".to_string(),
                confidence: 0.8,
                observed_at: "2026-06-25T11:00:00Z".to_string(),
                learned_at: "2026-06-25T11:01:00Z".to_string(),
            })
            .unwrap()
            .memory;
        memory.approve();
        memory
    }

    fn candidate_memory() -> MemoryRecord {
        let app = SlackReliabilityMemoryApp::new();
        let answer = app
            .handle_mention(request("what should we remember", Vec::new()))
            .unwrap();
        let card = answer.candidate_card.unwrap();
        let engine = AdaptiveMemoryEngine::new();
        engine
            .propose_candidate(MemoryCandidateRequest {
                id: card.memory_id,
                kind: MemoryKind::OperationalNote,
                scope: MemoryScope::Service,
                service: "payments".to_string(),
                symptoms: vec!["latency".to_string()],
                event_summary: "payments latency spike".to_string(),
                evidence_ids: vec!["slack-thread-1".to_string()],
                source: SLACK_SOURCE.to_string(),
                source_version: SLACK_SOURCE_VERSION.to_string(),
                confidence: 0.4,
                observed_at: "2026-06-25T12:00:00Z".to_string(),
                learned_at: "2026-06-25T12:00:00Z".to_string(),
            })
            .unwrap()
            .memory
    }

    #[test]
    fn mention_parsing_is_deterministic() {
        assert_eq!(
            parse_mention("what changed in payments?"),
            SlackMentionIntent::WhatChanged
        );
        assert_eq!(
            parse_mention("have we seen this before"),
            SlackMentionIntent::SeenBefore
        );
        assert_eq!(
            parse_mention("have we seen checkout latency before"),
            SlackMentionIntent::SeenBefore
        );
        assert_eq!(
            parse_mention("recall payments latency"),
            SlackMentionIntent::Recall {
                query: "payments latency".to_string()
            }
        );
        assert_eq!(
            parse_mention("what should we remember"),
            SlackMentionIntent::RememberCandidate
        );
        assert_eq!(parse_mention("hello"), SlackMentionIntent::Help);
    }

    #[test]
    fn fallback_help_response_is_safe_and_concise() {
        let app = SlackReliabilityMemoryApp::new();
        let answer = app.handle_mention(request("hello", Vec::new())).unwrap();

        assert_eq!(answer.label, SlackMessageLabel::NeedsReview);
        assert!(answer.text.contains("what changed"));
        assert!(!answer.renders_infrastructure_mutation_actions());
    }

    #[test]
    fn recall_card_renders_match_reasons() {
        let app = SlackReliabilityMemoryApp::new();
        let answer = app
            .handle_mention(request(
                "recall payments latency",
                vec![active_memory(
                    "mem-payments-latency",
                    "payments",
                    "latency spike",
                )],
            ))
            .unwrap();

        assert_eq!(answer.label, SlackMessageLabel::SimilarMemory);
        assert_eq!(answer.recall_cards.len(), 1);
        let rendered = answer.recall_cards[0].render_text();
        assert!(rendered.contains("reasons:"));
        assert!(rendered.contains("same service"));
        assert!(rendered.contains("confidence"));
    }

    #[test]
    fn empty_recall_result_renders_safely() {
        let app = SlackReliabilityMemoryApp::new();
        let answer = app
            .handle_mention(request("recall inventory queues", Vec::new()))
            .unwrap();

        assert!(answer.recall_cards.is_empty());
        assert!(answer.text.contains("did not find"));
        assert!(!answer.renders_infrastructure_mutation_actions());
    }

    #[test]
    fn memory_candidate_card_renders_review_state() {
        let app = SlackReliabilityMemoryApp::new();
        let answer = app
            .handle_mention(request("what should we remember", Vec::new()))
            .unwrap();

        let card = answer.candidate_card.as_ref().expect("candidate card");
        assert_eq!(card.label, SlackMessageLabel::MemoryCandidate);
        assert_eq!(card.proposed_status, MemoryStatus::Candidate);
        assert!(card.render_text().contains("status: candidate"));
        assert!(card
            .actions
            .iter()
            .any(|action| action.id == SlackFeedbackActionKind::Remember));
        assert!(!answer.renders_infrastructure_mutation_actions());
    }

    #[test]
    fn feedback_action_maps_to_human_feedback_kinds() {
        assert_eq!(
            SlackFeedbackActionKind::Remember.feedback_kind(),
            FeedbackKind::Approved
        );
        assert_eq!(
            SlackFeedbackActionKind::Reject.feedback_kind(),
            FeedbackKind::Rejected
        );
        assert_eq!(
            SlackFeedbackActionKind::Correct.feedback_kind(),
            FeedbackKind::Corrected
        );
        assert_eq!(
            SlackFeedbackActionKind::NotUseful.feedback_kind(),
            FeedbackKind::NotUseful
        );
        assert_eq!(
            SlackFeedbackActionKind::NeedsMoreEvidence.feedback_kind(),
            FeedbackKind::NeedsMoreEvidence
        );
    }

    #[test]
    fn slack_never_renders_infrastructure_mutation_actions() {
        let app = SlackReliabilityMemoryApp::new();
        let answers = [
            app.handle_mention(request("hello", Vec::new())).unwrap(),
            app.handle_mention(request(
                "recall payments latency",
                vec![active_memory(
                    "mem-payments-latency-safe",
                    "payments",
                    "latency spike",
                )],
            ))
            .unwrap(),
            app.handle_mention(request("what should we remember", Vec::new()))
                .unwrap(),
        ];

        for answer in answers {
            assert!(!answer.renders_infrastructure_mutation_actions());
            validate_no_mutating_actions(&answer.actions).unwrap();
        }
    }

    #[test]
    fn slack_actions_only_update_memory() {
        let app = SlackReliabilityMemoryApp::new();
        let memory = candidate_memory();
        let response = app
            .handle_feedback_action(SlackFeedbackAction {
                action: SlackFeedbackActionKind::Remember,
                actor_id: "U123".to_string(),
                channel_id: "C123".to_string(),
                timestamp: "2026-06-25T13:00:00Z".to_string(),
                target_memory: memory,
                note: Some("team reviewed this".to_string()),
                correction_text: None,
            })
            .unwrap();

        assert_eq!(response.updated_memory.status, MemoryStatus::Active);
        assert!(response.text.contains("memory"));
        assert!(!response.text.contains("rollback"));
        assert!(!response.renders_infrastructure_mutation_actions());
        validate_no_mutating_actions(&response.actions).unwrap();
    }

    #[test]
    fn correct_action_updates_memory_to_corrected() {
        let app = SlackReliabilityMemoryApp::new();
        let memory = active_memory("mem-correct-slack", "payments", "latency spike");
        let response = app
            .handle_feedback_action(SlackFeedbackAction {
                action: SlackFeedbackActionKind::Correct,
                actor_id: "U123".to_string(),
                channel_id: "C123".to_string(),
                timestamp: "2026-06-25T13:00:00Z".to_string(),
                target_memory: memory,
                note: Some("cause was connection pool".to_string()),
                correction_text: Some("connection pool saturation, not deploy shape".to_string()),
            })
            .unwrap();

        assert_eq!(response.updated_memory.status, MemoryStatus::Corrected);
        assert!(!response.renders_infrastructure_mutation_actions());
    }
}
