//! The kind of reliability receipt.
//!
//! The discriminator tells consumers (CLI, Slack, future dashboards) how to
//! present and group receipts. Receipt kinds are stable across releases; new
//! kinds are added by extending this enum.

use serde::{Deserialize, Serialize};

/// What kind of reliability receipt this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptKind {
    /// A raw observation receipt — no conclusion, just structured evidence.
    Observation,
    /// An explanation of a reliability incident.
    IncidentExplanation,
    /// A review of a deployment (e.g. pre/post analysis).
    DeploymentReview,
    /// A recommendation (proposal requiring human approval).
    Recommendation,
    /// The result of an Ability run.
    AbilityRun,
    /// A periodic summary (daily, weekly, etc.).
    DailySummary,
    /// A system diagnostic report.
    SystemDiagnostic,
    /// A new memory candidate was created by the engine.
    MemoryCandidateCreated,
    /// A memory candidate was approved by a human.
    MemoryApproved,
    /// A memory candidate was rejected by a human.
    MemoryRejected,
    /// A memory was corrected by a human.
    MemoryCorrected,
    /// A memory was superseded by a newer one.
    MemorySuperseded,
    /// The result of a recall query.
    RecallResult,
    /// Human feedback was recorded on a memory or recommendation.
    HumanFeedbackRecorded,
    /// An unknown or unrecognized kind. Used as a safe fallback.
    Unknown,
}

impl ReceiptKind {
    /// Stable lowercase string tag for the kind.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Observation => "observation",
            Self::IncidentExplanation => "incident_explanation",
            Self::DeploymentReview => "deployment_review",
            Self::Recommendation => "recommendation",
            Self::AbilityRun => "ability_run",
            Self::DailySummary => "daily_summary",
            Self::SystemDiagnostic => "system_diagnostic",
            Self::MemoryCandidateCreated => "memory_candidate_created",
            Self::MemoryApproved => "memory_approved",
            Self::MemoryRejected => "memory_rejected",
            Self::MemoryCorrected => "memory_corrected",
            Self::MemorySuperseded => "memory_superseded",
            Self::RecallResult => "recall_result",
            Self::HumanFeedbackRecorded => "human_feedback_recorded",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for ReceiptKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_is_lowercase_and_stable() {
        assert_eq!(ReceiptKind::Observation.as_str(), "observation");
        assert_eq!(
            ReceiptKind::IncidentExplanation.as_str(),
            "incident_explanation"
        );
        assert_eq!(ReceiptKind::DeploymentReview.as_str(), "deployment_review");
        assert_eq!(ReceiptKind::Recommendation.as_str(), "recommendation");
        assert_eq!(ReceiptKind::AbilityRun.as_str(), "ability_run");
        assert_eq!(ReceiptKind::DailySummary.as_str(), "daily_summary");
        assert_eq!(ReceiptKind::SystemDiagnostic.as_str(), "system_diagnostic");
        assert_eq!(
            ReceiptKind::MemoryCandidateCreated.as_str(),
            "memory_candidate_created"
        );
        assert_eq!(ReceiptKind::MemoryApproved.as_str(), "memory_approved");
        assert_eq!(ReceiptKind::MemoryRejected.as_str(), "memory_rejected");
        assert_eq!(ReceiptKind::MemoryCorrected.as_str(), "memory_corrected");
        assert_eq!(ReceiptKind::MemorySuperseded.as_str(), "memory_superseded");
        assert_eq!(ReceiptKind::RecallResult.as_str(), "recall_result");
        assert_eq!(
            ReceiptKind::HumanFeedbackRecorded.as_str(),
            "human_feedback_recorded"
        );
        assert_eq!(ReceiptKind::Unknown.as_str(), "unknown");
    }

    #[test]
    fn serializes_as_snake_case_tag() {
        let json = serde_json::to_string(&ReceiptKind::IncidentExplanation).unwrap();
        assert_eq!(json, "\"incident_explanation\"");
    }

    #[test]
    fn round_trips_through_serde() {
        let kind = ReceiptKind::DeploymentReview;
        let json = serde_json::to_string(&kind).unwrap();
        let back: ReceiptKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back, kind);
    }

    #[test]
    fn new_memory_kinds_round_trip_through_serde() {
        let kinds = [
            ReceiptKind::MemoryCandidateCreated,
            ReceiptKind::MemoryApproved,
            ReceiptKind::MemoryRejected,
            ReceiptKind::MemoryCorrected,
            ReceiptKind::MemorySuperseded,
            ReceiptKind::RecallResult,
            ReceiptKind::HumanFeedbackRecorded,
        ];
        for kind in kinds {
            let json = serde_json::to_string(&kind).unwrap();
            let back: ReceiptKind = serde_json::from_str(&json).unwrap();
            assert_eq!(back, kind);
            assert_eq!(json, format!("\"{}\"", kind.as_str()));
        }
    }

    #[test]
    fn all_variants_round_trip_through_serde() {
        let kinds = [
            ReceiptKind::Observation,
            ReceiptKind::IncidentExplanation,
            ReceiptKind::DeploymentReview,
            ReceiptKind::Recommendation,
            ReceiptKind::AbilityRun,
            ReceiptKind::DailySummary,
            ReceiptKind::SystemDiagnostic,
            ReceiptKind::MemoryCandidateCreated,
            ReceiptKind::MemoryApproved,
            ReceiptKind::MemoryRejected,
            ReceiptKind::MemoryCorrected,
            ReceiptKind::MemorySuperseded,
            ReceiptKind::RecallResult,
            ReceiptKind::HumanFeedbackRecorded,
            ReceiptKind::Unknown,
        ];
        for kind in kinds {
            let json = serde_json::to_string(&kind).unwrap();
            let back: ReceiptKind = serde_json::from_str(&json).unwrap();
            assert_eq!(back, kind);
            assert_eq!(json, format!("\"{}\"", kind.as_str()));
        }
    }

    #[test]
    fn display_matches_as_str() {
        assert_eq!(ReceiptKind::AbilityRun.to_string(), "ability_run");
        assert_eq!(
            ReceiptKind::MemoryCandidateCreated.to_string(),
            "memory_candidate_created"
        );
        assert_eq!(
            ReceiptKind::HumanFeedbackRecorded.to_string(),
            "human_feedback_recorded"
        );
    }
}
