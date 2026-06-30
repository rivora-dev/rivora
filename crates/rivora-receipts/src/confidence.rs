//! A receipt's confidence value.
//!
//! Confidence is **never** a bare numeric score. It is always accompanied by
//! a `method` (formula version), a list of contributing factors, a list of
//! limiting factors, and an explicit uncertainty statement. This makes
//! confidence auditable and prevents over-trusting single-number scores.

use serde::{Deserialize, Serialize};

use rivora_types::NonEmptyString;

/// A qualitative confidence level derived from the numeric score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceLevel {
    /// Very low confidence (score < 0.4).
    Low,
    /// Medium confidence (0.4 <= score < 0.7).
    Medium,
    /// High confidence (score >= 0.7).
    High,
}

impl ConfidenceLevel {
    /// Derives the qualitative level from a numeric score in `[0.0, 1.0]`.
    #[must_use]
    pub fn from_score(score: f64) -> Self {
        if score >= 0.7 {
            Self::High
        } else if score >= 0.4 {
            Self::Medium
        } else {
            Self::Low
        }
    }

    /// Returns the snake_case string tag for this level.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

impl std::fmt::Display for ConfidenceLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A typed, auditable confidence value.
///
/// A confidence value is always accompanied by a `method` string (the
/// formula or heuristic used to compute the score), a list of contributing
/// factors (what increased confidence), a list of limiting factors (what
/// reduced confidence), and an explicit uncertainty statement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Confidence {
    /// The numeric score in `[0.0, 1.0]`.
    pub score: f64,
    /// The qualitative level derived from the score.
    pub level: ConfidenceLevel,
    /// A versioned identifier of the method used to compute the score
    /// (e.g. `"pattern-frequency-weighted-v1"`). Required: a confidence
    /// without a method is **invalid** per the spec.
    pub method: NonEmptyString,
    /// Factors that contributed to the confidence score.
    pub contributing_factors: Vec<NonEmptyString>,
    /// Factors that limited the confidence score.
    pub limiting_factors: Vec<NonEmptyString>,
    /// An explicit, free-text uncertainty statement. Required.
    pub uncertainty: NonEmptyString,
}

impl Confidence {
    /// Creates a new `Confidence` with required fields.
    ///
    /// # Errors
    ///
    /// Returns an error if `score` is not in `[0.0, 1.0]`, if `method` is
    /// empty, or if `uncertainty` is empty.
    pub fn new(
        score: f64,
        method: impl Into<String>,
        uncertainty: impl Into<String>,
    ) -> Result<Self, rivora_errors::RivoraError> {
        if !(0.0..=1.0).contains(&score) {
            return Err(rivora_errors::RivoraError::invalid_value(
                "score",
                format!("must be in [0.0, 1.0], got {score}"),
            ));
        }
        Ok(Self {
            score,
            level: ConfidenceLevel::from_score(score),
            method: NonEmptyString::new(method.into())?,
            contributing_factors: Vec::new(),
            limiting_factors: Vec::new(),
            uncertainty: NonEmptyString::new(uncertainty.into())?,
        })
    }

    /// Builder-style setter for `contributing_factors`.
    #[must_use]
    pub fn with_contributing_factors(mut self, factors: Vec<NonEmptyString>) -> Self {
        self.contributing_factors = factors;
        self
    }

    /// Builder-style setter for `limiting_factors`.
    #[must_use]
    pub fn with_limiting_factors(mut self, factors: Vec<NonEmptyString>) -> Self {
        self.limiting_factors = factors;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_from_score_thresholds() {
        assert_eq!(ConfidenceLevel::from_score(0.0), ConfidenceLevel::Low);
        assert_eq!(ConfidenceLevel::from_score(0.39), ConfidenceLevel::Low);
        assert_eq!(ConfidenceLevel::from_score(0.4), ConfidenceLevel::Medium);
        assert_eq!(ConfidenceLevel::from_score(0.69), ConfidenceLevel::Medium);
        assert_eq!(ConfidenceLevel::from_score(0.7), ConfidenceLevel::High);
        assert_eq!(ConfidenceLevel::from_score(1.0), ConfidenceLevel::High);
    }

    #[test]
    fn level_ordering() {
        assert!(ConfidenceLevel::Low < ConfidenceLevel::Medium);
        assert!(ConfidenceLevel::Medium < ConfidenceLevel::High);
    }

    #[test]
    fn confidence_rejects_invalid_score() {
        let result = Confidence::new(1.5, "method-v1", "uncertainty");
        assert!(result.is_err());
        let result = Confidence::new(-0.1, "method-v1", "uncertainty");
        assert!(result.is_err());
    }

    #[test]
    fn confidence_rejects_empty_method() {
        let result = Confidence::new(0.5, "", "uncertainty");
        assert!(result.is_err());
    }

    #[test]
    fn confidence_rejects_empty_uncertainty() {
        let result = Confidence::new(0.5, "method-v1", "");
        assert!(result.is_err());
    }

    #[test]
    fn confidence_accepts_valid_fields() {
        let c = Confidence::new(0.85, "pattern-frequency-v1", "Limited data").unwrap();
        assert_eq!(c.score, 0.85);
        assert_eq!(c.level, ConfidenceLevel::High);
    }

    #[test]
    fn confidence_level_derived_from_score() {
        let low = Confidence::new(0.2, "m", "u").unwrap();
        assert_eq!(low.level, ConfidenceLevel::Low);
        let mid = Confidence::new(0.5, "m", "u").unwrap();
        assert_eq!(mid.level, ConfidenceLevel::Medium);
        let high = Confidence::new(0.9, "m", "u").unwrap();
        assert_eq!(high.level, ConfidenceLevel::High);
    }

    #[test]
    fn confidence_round_trips_through_serde() {
        let c = Confidence::new(0.85, "pattern-frequency-v1", "Limited data")
            .unwrap()
            .with_contributing_factors(vec![NonEmptyString::new("stable pattern").unwrap()])
            .with_limiting_factors(vec![NonEmptyString::new("small sample").unwrap()]);
        let json = serde_json::to_string(&c).unwrap();
        let back: Confidence = serde_json::from_str(&json).unwrap();
        assert_eq!(back.score, c.score);
        assert_eq!(back.method, c.method);
        assert_eq!(back.uncertainty, c.uncertainty);
    }

    #[test]
    fn level_serializes_as_snake_case() {
        let json = serde_json::to_string(&ConfidenceLevel::High).unwrap();
        assert_eq!(json, "\"high\"");
    }
}
