//! Memory lifecycle status for the context memory model.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryStatus {
    Candidate,
    Draft,
    Active,
    Rejected,
    Corrected,
    Superseded,
    Expired,
    Archived,
    Invalid,
}

impl MemoryStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Rejected => "rejected",
            Self::Corrected => "corrected",
            Self::Superseded => "superseded",
            Self::Expired => "expired",
            Self::Archived => "archived",
            Self::Invalid => "invalid",
        }
    }

    #[must_use]
    pub fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }

    #[must_use]
    pub fn is_expired(self) -> bool {
        matches!(self, Self::Expired)
    }

    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Archived | Self::Invalid | Self::Rejected)
    }

    #[must_use]
    pub fn is_candidate(self) -> bool {
        matches!(self, Self::Candidate)
    }

    #[must_use]
    pub fn is_rejected(self) -> bool {
        matches!(self, Self::Rejected)
    }

    #[must_use]
    pub fn is_corrected(self) -> bool {
        matches!(self, Self::Corrected)
    }

    #[must_use]
    pub fn is_approved(self) -> bool {
        matches!(self, Self::Active)
    }
}

impl std::fmt::Display for MemoryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_as_str_is_lowercase_and_stable() {
        assert_eq!(MemoryStatus::Candidate.as_str(), "candidate");
        assert_eq!(MemoryStatus::Draft.as_str(), "draft");
        assert_eq!(MemoryStatus::Active.as_str(), "active");
        assert_eq!(MemoryStatus::Rejected.as_str(), "rejected");
        assert_eq!(MemoryStatus::Corrected.as_str(), "corrected");
        assert_eq!(MemoryStatus::Superseded.as_str(), "superseded");
        assert_eq!(MemoryStatus::Expired.as_str(), "expired");
        assert_eq!(MemoryStatus::Archived.as_str(), "archived");
        assert_eq!(MemoryStatus::Invalid.as_str(), "invalid");
    }

    #[test]
    fn status_serializes_as_snake_case_tag() {
        let json = serde_json::to_string(&MemoryStatus::Superseded).unwrap();
        assert_eq!(json, "\"superseded\"");
    }

    #[test]
    fn status_round_trips_through_serde() {
        let status = MemoryStatus::Active;
        let json = serde_json::to_string(&status).unwrap();
        let back: MemoryStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, status);
    }

    #[test]
    fn status_display_matches_as_str() {
        assert_eq!(MemoryStatus::Draft.to_string(), "draft");
    }

    #[test]
    fn is_active_returns_true_only_for_active() {
        assert!(MemoryStatus::Active.is_active());
        assert!(!MemoryStatus::Draft.is_active());
        assert!(!MemoryStatus::Expired.is_active());
        assert!(!MemoryStatus::Archived.is_active());
        assert!(!MemoryStatus::Candidate.is_active());
        assert!(!MemoryStatus::Rejected.is_active());
        assert!(!MemoryStatus::Corrected.is_active());
    }

    #[test]
    fn is_expired_returns_true_only_for_expired() {
        assert!(MemoryStatus::Expired.is_expired());
        assert!(!MemoryStatus::Active.is_expired());
        assert!(!MemoryStatus::Archived.is_expired());
        assert!(!MemoryStatus::Candidate.is_expired());
        assert!(!MemoryStatus::Rejected.is_expired());
        assert!(!MemoryStatus::Corrected.is_expired());
    }

    #[test]
    fn is_terminal_returns_true_for_archived_invalid_and_rejected() {
        assert!(MemoryStatus::Archived.is_terminal());
        assert!(MemoryStatus::Invalid.is_terminal());
        assert!(MemoryStatus::Rejected.is_terminal());
        assert!(!MemoryStatus::Active.is_terminal());
        assert!(!MemoryStatus::Draft.is_terminal());
        assert!(!MemoryStatus::Superseded.is_terminal());
        assert!(!MemoryStatus::Expired.is_terminal());
        assert!(!MemoryStatus::Candidate.is_terminal());
        assert!(!MemoryStatus::Corrected.is_terminal());
    }

    #[test]
    fn is_candidate_returns_true_only_for_candidate() {
        assert!(MemoryStatus::Candidate.is_candidate());
        assert!(!MemoryStatus::Draft.is_candidate());
        assert!(!MemoryStatus::Active.is_candidate());
        assert!(!MemoryStatus::Rejected.is_candidate());
        assert!(!MemoryStatus::Corrected.is_candidate());
        assert!(!MemoryStatus::Superseded.is_candidate());
        assert!(!MemoryStatus::Expired.is_candidate());
        assert!(!MemoryStatus::Archived.is_candidate());
        assert!(!MemoryStatus::Invalid.is_candidate());
    }

    #[test]
    fn is_rejected_returns_true_only_for_rejected() {
        assert!(MemoryStatus::Rejected.is_rejected());
        assert!(!MemoryStatus::Draft.is_rejected());
        assert!(!MemoryStatus::Active.is_rejected());
        assert!(!MemoryStatus::Candidate.is_rejected());
        assert!(!MemoryStatus::Corrected.is_rejected());
        assert!(!MemoryStatus::Archived.is_rejected());
        assert!(!MemoryStatus::Invalid.is_rejected());
    }

    #[test]
    fn is_corrected_returns_true_only_for_corrected() {
        assert!(MemoryStatus::Corrected.is_corrected());
        assert!(!MemoryStatus::Draft.is_corrected());
        assert!(!MemoryStatus::Active.is_corrected());
        assert!(!MemoryStatus::Candidate.is_corrected());
        assert!(!MemoryStatus::Rejected.is_corrected());
        assert!(!MemoryStatus::Archived.is_corrected());
        assert!(!MemoryStatus::Invalid.is_corrected());
    }

    #[test]
    fn is_approved_returns_true_only_for_active() {
        assert!(MemoryStatus::Active.is_approved());
        assert!(!MemoryStatus::Draft.is_approved());
        assert!(!MemoryStatus::Candidate.is_approved());
        assert!(!MemoryStatus::Rejected.is_approved());
        assert!(!MemoryStatus::Corrected.is_approved());
        assert!(!MemoryStatus::Archived.is_approved());
        assert!(!MemoryStatus::Invalid.is_approved());
    }

    #[test]
    fn all_variants_round_trip_through_serde() {
        let variants = [
            MemoryStatus::Candidate,
            MemoryStatus::Draft,
            MemoryStatus::Active,
            MemoryStatus::Rejected,
            MemoryStatus::Corrected,
            MemoryStatus::Superseded,
            MemoryStatus::Expired,
            MemoryStatus::Archived,
            MemoryStatus::Invalid,
        ];
        for status in variants {
            let json = serde_json::to_string(&status).unwrap();
            let back: MemoryStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(back, status);
        }
    }
}
