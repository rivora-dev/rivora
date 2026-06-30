//! Retention policy and decay configuration for memory records.

use serde::{Deserialize, Serialize};

use rivora_types::NonEmptyString;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryRetentionPolicy {
    Permanent,
    UntilSuperseded,
    TimeBound,
    ReviewRequired,
    Ephemeral,
    Unknown,
}

impl MemoryRetentionPolicy {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Permanent => "permanent",
            Self::UntilSuperseded => "until_superseded",
            Self::TimeBound => "time_bound",
            Self::ReviewRequired => "review_required",
            Self::Ephemeral => "ephemeral",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for MemoryRetentionPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryDecay {
    None,
    Linear,
    StepDown,
    ManualReview,
    Unknown,
}

impl MemoryDecay {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Linear => "linear",
            Self::StepDown => "step_down",
            Self::ManualReview => "manual_review",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for MemoryDecay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryRetention {
    pub policy: MemoryRetentionPolicy,
    pub expires_at: Option<NonEmptyString>,
    pub review_after: Option<NonEmptyString>,
    pub max_age_days: Option<u64>,
    pub decay: MemoryDecay,
    pub reason: NonEmptyString,
}

impl MemoryRetention {
    #[must_use]
    pub fn builder() -> crate::builders::MemoryRetentionBuilder {
        crate::builders::MemoryRetentionBuilder::new()
    }

    pub fn new(
        policy: MemoryRetentionPolicy,
        reason: impl Into<String>,
    ) -> Result<Self, rivora_errors::RivoraError> {
        Ok(Self {
            policy,
            expires_at: None,
            review_after: None,
            max_age_days: None,
            decay: MemoryDecay::None,
            reason: NonEmptyString::new(reason.into())?,
        })
    }

    #[must_use]
    pub fn with_expires_at(mut self, expires_at: impl Into<String>) -> Self {
        self.expires_at = NonEmptyString::new(expires_at.into()).ok();
        self
    }

    #[must_use]
    pub fn with_review_after(mut self, review_after: impl Into<String>) -> Self {
        self.review_after = NonEmptyString::new(review_after.into()).ok();
        self
    }

    #[must_use]
    pub fn with_max_age_days(mut self, max_age_days: u64) -> Self {
        self.max_age_days = Some(max_age_days);
        self
    }

    #[must_use]
    pub fn with_decay(mut self, decay: MemoryDecay) -> Self {
        self.decay = decay;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_as_str_is_lowercase_and_stable() {
        assert_eq!(MemoryRetentionPolicy::Permanent.as_str(), "permanent");
        assert_eq!(
            MemoryRetentionPolicy::UntilSuperseded.as_str(),
            "until_superseded"
        );
        assert_eq!(MemoryRetentionPolicy::TimeBound.as_str(), "time_bound");
        assert_eq!(
            MemoryRetentionPolicy::ReviewRequired.as_str(),
            "review_required"
        );
        assert_eq!(MemoryRetentionPolicy::Ephemeral.as_str(), "ephemeral");
        assert_eq!(MemoryRetentionPolicy::Unknown.as_str(), "unknown");
    }

    #[test]
    fn policy_serializes_as_snake_case_tag() {
        let json = serde_json::to_string(&MemoryRetentionPolicy::ReviewRequired).unwrap();
        assert_eq!(json, "\"review_required\"");
    }

    #[test]
    fn policy_round_trips_through_serde() {
        let policy = MemoryRetentionPolicy::UntilSuperseded;
        let json = serde_json::to_string(&policy).unwrap();
        let back: MemoryRetentionPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(back, policy);
    }

    #[test]
    fn policy_display_matches_as_str() {
        assert_eq!(MemoryRetentionPolicy::TimeBound.to_string(), "time_bound");
    }

    #[test]
    fn decay_as_str_is_lowercase_and_stable() {
        assert_eq!(MemoryDecay::None.as_str(), "none");
        assert_eq!(MemoryDecay::Linear.as_str(), "linear");
        assert_eq!(MemoryDecay::StepDown.as_str(), "step_down");
        assert_eq!(MemoryDecay::ManualReview.as_str(), "manual_review");
        assert_eq!(MemoryDecay::Unknown.as_str(), "unknown");
    }

    #[test]
    fn decay_serializes_as_snake_case_tag() {
        let json = serde_json::to_string(&MemoryDecay::StepDown).unwrap();
        assert_eq!(json, "\"step_down\"");
    }

    #[test]
    fn decay_round_trips_through_serde() {
        let decay = MemoryDecay::ManualReview;
        let json = serde_json::to_string(&decay).unwrap();
        let back: MemoryDecay = serde_json::from_str(&json).unwrap();
        assert_eq!(back, decay);
    }

    #[test]
    fn decay_display_matches_as_str() {
        assert_eq!(MemoryDecay::Linear.to_string(), "linear");
    }

    #[test]
    fn retention_rejects_empty_reason() {
        assert!(MemoryRetention::new(MemoryRetentionPolicy::Permanent, "").is_err());
    }

    #[test]
    fn retention_accepts_valid_fields() {
        let r = MemoryRetention::new(MemoryRetentionPolicy::Permanent, "fixture").unwrap();
        assert_eq!(r.policy, MemoryRetentionPolicy::Permanent);
        assert_eq!(r.decay, MemoryDecay::None);
        assert!(r.expires_at.is_none());
        assert!(r.review_after.is_none());
        assert!(r.max_age_days.is_none());
        assert_eq!(r.reason.as_str(), "fixture");
    }

    #[test]
    fn retention_with_optional_fields() {
        let r = MemoryRetention::new(MemoryRetentionPolicy::TimeBound, "time-bound")
            .unwrap()
            .with_expires_at("2026-06-25T12:00:00Z")
            .with_review_after("2026-07-25T12:00:00Z")
            .with_max_age_days(30)
            .with_decay(MemoryDecay::Linear);
        assert_eq!(
            r.expires_at.as_ref().unwrap().as_str(),
            "2026-06-25T12:00:00Z"
        );
        assert_eq!(
            r.review_after.as_ref().unwrap().as_str(),
            "2026-07-25T12:00:00Z"
        );
        assert_eq!(r.max_age_days, Some(30));
        assert_eq!(r.decay, MemoryDecay::Linear);
    }

    #[test]
    fn retention_round_trips_through_serde() {
        let r = MemoryRetention::new(MemoryRetentionPolicy::ReviewRequired, "needs review")
            .unwrap()
            .with_review_after("2026-07-25T12:00:00Z")
            .with_decay(MemoryDecay::ManualReview);
        let json = serde_json::to_string(&r).unwrap();
        let back: MemoryRetention = serde_json::from_str(&json).unwrap();
        assert_eq!(back, r);
    }
}
