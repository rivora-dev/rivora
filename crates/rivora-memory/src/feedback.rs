//! Human feedback on memory, receipts, recommendations, and recall results.
//!
//! [`HumanFeedback`] is a first-class type capturing how an engineer responded
//! to a piece of learned knowledge. Feedback drives the adaptive reliability
//! lifecycle: approvals promote candidates to active memories, rejections mark
//! memories terminal, and corrections refine memory content.

use serde::{Deserialize, Serialize};

use rivora_errors::RivoraError;
use rivora_types::NonEmptyString;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackKind {
    Approved,
    Rejected,
    Corrected,
    Useful,
    NotUseful,
    NeedsMoreEvidence,
    WrongCause,
    WrongService,
    WrongTimeWindow,
}

impl FeedbackKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Corrected => "corrected",
            Self::Useful => "useful",
            Self::NotUseful => "not_useful",
            Self::NeedsMoreEvidence => "needs_more_evidence",
            Self::WrongCause => "wrong_cause",
            Self::WrongService => "wrong_service",
            Self::WrongTimeWindow => "wrong_time_window",
        }
    }

    #[must_use]
    pub fn is_actionable(self) -> bool {
        matches!(self, Self::Approved | Self::Rejected | Self::Corrected)
    }
}

impl std::fmt::Display for FeedbackKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackTargetType {
    Memory,
    Receipt,
    Recommendation,
    RecallResult,
}

impl FeedbackTargetType {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Memory => "memory",
            Self::Receipt => "receipt",
            Self::Recommendation => "recommendation",
            Self::RecallResult => "recall_result",
        }
    }
}

impl std::fmt::Display for FeedbackTargetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackSource {
    Human,
    Slack,
    Cli,
    Api,
}

impl FeedbackSource {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Slack => "slack",
            Self::Cli => "cli",
            Self::Api => "api",
        }
    }
}

impl std::fmt::Display for FeedbackSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HumanFeedback {
    pub id: NonEmptyString,
    pub target_id: NonEmptyString,
    pub target_type: FeedbackTargetType,
    pub actor: NonEmptyString,
    pub source: FeedbackSource,
    pub kind: FeedbackKind,
    pub note: Option<NonEmptyString>,
    pub correction_text: Option<NonEmptyString>,
    pub confidence_adjustment: Option<f64>,
    pub timestamp: NonEmptyString,
}

impl HumanFeedback {
    #[must_use]
    pub fn builder() -> crate::builders::HumanFeedbackBuilder {
        crate::builders::HumanFeedbackBuilder::new()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        target_id: impl Into<String>,
        target_type: FeedbackTargetType,
        actor: impl Into<String>,
        source: FeedbackSource,
        kind: FeedbackKind,
        timestamp: impl Into<String>,
    ) -> Result<Self, RivoraError> {
        Ok(Self {
            id: NonEmptyString::new(id.into())?,
            target_id: NonEmptyString::new(target_id.into())?,
            target_type,
            actor: NonEmptyString::new(actor.into())?,
            source,
            kind,
            note: None,
            correction_text: None,
            confidence_adjustment: None,
            timestamp: NonEmptyString::new(timestamp.into())?,
        })
    }

    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = NonEmptyString::new(note.into()).ok();
        self
    }

    #[must_use]
    pub fn with_correction_text(mut self, correction_text: impl Into<String>) -> Self {
        self.correction_text = NonEmptyString::new(correction_text.into()).ok();
        self
    }

    #[must_use]
    pub fn with_confidence_adjustment(mut self, confidence_adjustment: f64) -> Self {
        self.confidence_adjustment = Some(confidence_adjustment);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feedback_kind_as_str_is_lowercase_and_stable() {
        assert_eq!(FeedbackKind::Approved.as_str(), "approved");
        assert_eq!(FeedbackKind::Rejected.as_str(), "rejected");
        assert_eq!(FeedbackKind::Corrected.as_str(), "corrected");
        assert_eq!(FeedbackKind::Useful.as_str(), "useful");
        assert_eq!(FeedbackKind::NotUseful.as_str(), "not_useful");
        assert_eq!(
            FeedbackKind::NeedsMoreEvidence.as_str(),
            "needs_more_evidence"
        );
        assert_eq!(FeedbackKind::WrongCause.as_str(), "wrong_cause");
        assert_eq!(FeedbackKind::WrongService.as_str(), "wrong_service");
        assert_eq!(FeedbackKind::WrongTimeWindow.as_str(), "wrong_time_window");
    }

    #[test]
    fn feedback_target_type_as_str_is_lowercase_and_stable() {
        assert_eq!(FeedbackTargetType::Memory.as_str(), "memory");
        assert_eq!(FeedbackTargetType::Receipt.as_str(), "receipt");
        assert_eq!(
            FeedbackTargetType::Recommendation.as_str(),
            "recommendation"
        );
        assert_eq!(FeedbackTargetType::RecallResult.as_str(), "recall_result");
    }

    #[test]
    fn feedback_source_as_str_is_lowercase_and_stable() {
        assert_eq!(FeedbackSource::Human.as_str(), "human");
        assert_eq!(FeedbackSource::Slack.as_str(), "slack");
        assert_eq!(FeedbackSource::Cli.as_str(), "cli");
        assert_eq!(FeedbackSource::Api.as_str(), "api");
    }

    #[test]
    fn feedback_kind_display_matches_as_str() {
        assert_eq!(FeedbackKind::Approved.to_string(), "approved");
        assert_eq!(FeedbackKind::NotUseful.to_string(), "not_useful");
    }

    #[test]
    fn feedback_target_type_display_matches_as_str() {
        assert_eq!(FeedbackTargetType::Memory.to_string(), "memory");
        assert_eq!(
            FeedbackTargetType::RecallResult.to_string(),
            "recall_result"
        );
    }

    #[test]
    fn feedback_source_display_matches_as_str() {
        assert_eq!(FeedbackSource::Slack.to_string(), "slack");
        assert_eq!(FeedbackSource::Cli.to_string(), "cli");
    }

    #[test]
    fn feedback_kind_round_trips_through_serde() {
        let variants = [
            FeedbackKind::Approved,
            FeedbackKind::Rejected,
            FeedbackKind::Corrected,
            FeedbackKind::Useful,
            FeedbackKind::NotUseful,
            FeedbackKind::NeedsMoreEvidence,
            FeedbackKind::WrongCause,
            FeedbackKind::WrongService,
            FeedbackKind::WrongTimeWindow,
        ];
        for kind in variants {
            let json = serde_json::to_string(&kind).unwrap();
            let back: FeedbackKind = serde_json::from_str(&json).unwrap();
            assert_eq!(back, kind);
        }
    }

    #[test]
    fn feedback_target_type_round_trips_through_serde() {
        let variants = [
            FeedbackTargetType::Memory,
            FeedbackTargetType::Receipt,
            FeedbackTargetType::Recommendation,
            FeedbackTargetType::RecallResult,
        ];
        for target_type in variants {
            let json = serde_json::to_string(&target_type).unwrap();
            let back: FeedbackTargetType = serde_json::from_str(&json).unwrap();
            assert_eq!(back, target_type);
        }
    }

    #[test]
    fn feedback_source_round_trips_through_serde() {
        let variants = [
            FeedbackSource::Human,
            FeedbackSource::Slack,
            FeedbackSource::Cli,
            FeedbackSource::Api,
        ];
        for source in variants {
            let json = serde_json::to_string(&source).unwrap();
            let back: FeedbackSource = serde_json::from_str(&json).unwrap();
            assert_eq!(back, source);
        }
    }

    #[test]
    fn is_actionable_returns_true_for_status_changing_kinds() {
        assert!(FeedbackKind::Approved.is_actionable());
        assert!(FeedbackKind::Rejected.is_actionable());
        assert!(FeedbackKind::Corrected.is_actionable());
        assert!(!FeedbackKind::Useful.is_actionable());
        assert!(!FeedbackKind::NotUseful.is_actionable());
        assert!(!FeedbackKind::NeedsMoreEvidence.is_actionable());
        assert!(!FeedbackKind::WrongCause.is_actionable());
        assert!(!FeedbackKind::WrongService.is_actionable());
        assert!(!FeedbackKind::WrongTimeWindow.is_actionable());
    }

    #[test]
    fn new_rejects_empty_id() {
        let result = HumanFeedback::new(
            "",
            "mem-1",
            FeedbackTargetType::Memory,
            "actor-1",
            FeedbackSource::Human,
            FeedbackKind::Approved,
            "2026-06-25T12:00:00Z",
        );
        assert!(result.is_err());
    }

    #[test]
    fn new_rejects_empty_target_id() {
        let result = HumanFeedback::new(
            "fb-1",
            "",
            FeedbackTargetType::Memory,
            "actor-1",
            FeedbackSource::Human,
            FeedbackKind::Approved,
            "2026-06-25T12:00:00Z",
        );
        assert!(result.is_err());
    }

    #[test]
    fn new_rejects_empty_actor() {
        let result = HumanFeedback::new(
            "fb-1",
            "mem-1",
            FeedbackTargetType::Memory,
            "",
            FeedbackSource::Human,
            FeedbackKind::Approved,
            "2026-06-25T12:00:00Z",
        );
        assert!(result.is_err());
    }

    #[test]
    fn new_rejects_empty_timestamp() {
        let result = HumanFeedback::new(
            "fb-1",
            "mem-1",
            FeedbackTargetType::Memory,
            "actor-1",
            FeedbackSource::Human,
            FeedbackKind::Approved,
            "",
        );
        assert!(result.is_err());
    }

    #[test]
    fn new_accepts_valid_fields() {
        let feedback = HumanFeedback::new(
            "fb-1",
            "mem-1",
            FeedbackTargetType::Memory,
            "actor-1",
            FeedbackSource::Human,
            FeedbackKind::Approved,
            "2026-06-25T12:00:00Z",
        )
        .unwrap();
        assert_eq!(feedback.id.as_str(), "fb-1");
        assert_eq!(feedback.target_id.as_str(), "mem-1");
        assert_eq!(feedback.target_type, FeedbackTargetType::Memory);
        assert_eq!(feedback.actor.as_str(), "actor-1");
        assert_eq!(feedback.source, FeedbackSource::Human);
        assert_eq!(feedback.kind, FeedbackKind::Approved);
        assert!(feedback.note.is_none());
        assert!(feedback.correction_text.is_none());
        assert!(feedback.confidence_adjustment.is_none());
        assert_eq!(feedback.timestamp.as_str(), "2026-06-25T12:00:00Z");
    }

    #[test]
    fn with_note_sets_note() {
        let feedback = HumanFeedback::new(
            "fb-1",
            "mem-1",
            FeedbackTargetType::Memory,
            "actor-1",
            FeedbackSource::Human,
            FeedbackKind::Approved,
            "2026-06-25T12:00:00Z",
        )
        .unwrap()
        .with_note("looks good");
        assert_eq!(feedback.note.as_ref().unwrap().as_str(), "looks good");
    }

    #[test]
    fn with_correction_text_sets_correction_text() {
        let feedback = HumanFeedback::new(
            "fb-1",
            "mem-1",
            FeedbackTargetType::Memory,
            "actor-1",
            FeedbackSource::Human,
            FeedbackKind::Corrected,
            "2026-06-25T12:00:00Z",
        )
        .unwrap()
        .with_correction_text("corrected body");
        assert_eq!(
            feedback.correction_text.as_ref().unwrap().as_str(),
            "corrected body"
        );
    }

    #[test]
    fn with_confidence_adjustment_sets_value() {
        let feedback = HumanFeedback::new(
            "fb-1",
            "mem-1",
            FeedbackTargetType::Memory,
            "actor-1",
            FeedbackSource::Human,
            FeedbackKind::Approved,
            "2026-06-25T12:00:00Z",
        )
        .unwrap()
        .with_confidence_adjustment(0.9);
        assert!((feedback.confidence_adjustment.unwrap() - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn feedback_round_trips_through_serde() {
        let feedback = HumanFeedback::new(
            "fb-1",
            "mem-1",
            FeedbackTargetType::Memory,
            "actor-1",
            FeedbackSource::Slack,
            FeedbackKind::Corrected,
            "2026-06-25T12:00:00Z",
        )
        .unwrap()
        .with_note("note")
        .with_correction_text("fixed")
        .with_confidence_adjustment(0.65);
        let json = serde_json::to_string(&feedback).unwrap();
        let back: HumanFeedback = serde_json::from_str(&json).unwrap();
        assert_eq!(back, feedback);
    }
}
