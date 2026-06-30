//! Memory-specific confidence values.

use serde::{Deserialize, Serialize};

use rivora_types::NonEmptyString;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryConfidenceLevel {
    Low,
    Medium,
    High,
}

impl MemoryConfidenceLevel {
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

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

impl std::fmt::Display for MemoryConfidenceLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryConfidence {
    pub score: f64,
    pub level: MemoryConfidenceLevel,
    pub explanation: NonEmptyString,
    pub contributing_factors: Vec<NonEmptyString>,
    pub limiting_factors: Vec<NonEmptyString>,
    pub last_evaluated_at: NonEmptyString,
}

impl MemoryConfidence {
    #[must_use]
    pub fn builder() -> crate::builders::MemoryConfidenceBuilder {
        crate::builders::MemoryConfidenceBuilder::new()
    }

    pub fn new(
        score: f64,
        explanation: impl Into<String>,
        last_evaluated_at: impl Into<String>,
    ) -> Result<Self, rivora_errors::RivoraError> {
        if !(0.0..=1.0).contains(&score) {
            return Err(rivora_errors::RivoraError::invalid_value(
                "score",
                format!("must be in [0.0, 1.0], got {score}"),
            ));
        }
        Ok(Self {
            score,
            level: MemoryConfidenceLevel::from_score(score),
            explanation: NonEmptyString::new(explanation.into())?,
            contributing_factors: Vec::new(),
            limiting_factors: Vec::new(),
            last_evaluated_at: NonEmptyString::new(last_evaluated_at.into())?,
        })
    }

    #[must_use]
    pub fn with_contributing_factors(mut self, factors: Vec<NonEmptyString>) -> Self {
        self.contributing_factors = factors;
        self
    }

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
        assert_eq!(
            MemoryConfidenceLevel::from_score(0.0),
            MemoryConfidenceLevel::Low
        );
        assert_eq!(
            MemoryConfidenceLevel::from_score(0.39),
            MemoryConfidenceLevel::Low
        );
        assert_eq!(
            MemoryConfidenceLevel::from_score(0.4),
            MemoryConfidenceLevel::Medium
        );
        assert_eq!(
            MemoryConfidenceLevel::from_score(0.69),
            MemoryConfidenceLevel::Medium
        );
        assert_eq!(
            MemoryConfidenceLevel::from_score(0.7),
            MemoryConfidenceLevel::High
        );
        assert_eq!(
            MemoryConfidenceLevel::from_score(1.0),
            MemoryConfidenceLevel::High
        );
    }

    #[test]
    fn level_ordering() {
        assert!(MemoryConfidenceLevel::Low < MemoryConfidenceLevel::Medium);
        assert!(MemoryConfidenceLevel::Medium < MemoryConfidenceLevel::High);
    }

    #[test]
    fn confidence_rejects_invalid_score() {
        assert!(MemoryConfidence::new(1.5, "explanation", "2026-06-25T12:00:00Z").is_err());
        assert!(MemoryConfidence::new(-0.1, "explanation", "2026-06-25T12:00:00Z").is_err());
    }

    #[test]
    fn confidence_rejects_empty_explanation() {
        assert!(MemoryConfidence::new(0.5, "", "2026-06-25T12:00:00Z").is_err());
    }

    #[test]
    fn confidence_rejects_empty_last_evaluated_at() {
        assert!(MemoryConfidence::new(0.5, "explanation", "").is_err());
    }

    #[test]
    fn confidence_accepts_valid_fields() {
        let c = MemoryConfidence::new(0.85, "Strong evidence", "2026-06-25T12:00:00Z").unwrap();
        assert_eq!(c.score, 0.85);
        assert_eq!(c.level, MemoryConfidenceLevel::High);
        assert_eq!(c.last_evaluated_at.as_str(), "2026-06-25T12:00:00Z");
    }

    #[test]
    fn confidence_level_derived_from_score() {
        let low = MemoryConfidence::new(0.2, "weak", "2026-06-25T12:00:00Z").unwrap();
        assert_eq!(low.level, MemoryConfidenceLevel::Low);
        let mid = MemoryConfidence::new(0.5, "moderate", "2026-06-25T12:00:00Z").unwrap();
        assert_eq!(mid.level, MemoryConfidenceLevel::Medium);
        let high = MemoryConfidence::new(0.9, "strong", "2026-06-25T12:00:00Z").unwrap();
        assert_eq!(high.level, MemoryConfidenceLevel::High);
    }

    #[test]
    fn confidence_with_factors() {
        let c = MemoryConfidence::new(0.75, "evidence-based", "2026-06-25T12:00:00Z")
            .unwrap()
            .with_contributing_factors(vec![NonEmptyString::new("stable pattern").unwrap()])
            .with_limiting_factors(vec![NonEmptyString::new("small sample").unwrap()]);
        assert_eq!(c.contributing_factors.len(), 1);
        assert_eq!(c.limiting_factors.len(), 1);
    }

    #[test]
    fn confidence_round_trips_through_serde() {
        let c = MemoryConfidence::new(0.85, "Strong evidence", "2026-06-25T12:00:00Z")
            .unwrap()
            .with_contributing_factors(vec![NonEmptyString::new("stable pattern").unwrap()])
            .with_limiting_factors(vec![NonEmptyString::new("small sample").unwrap()]);
        let json = serde_json::to_string(&c).unwrap();
        let back: MemoryConfidence = serde_json::from_str(&json).unwrap();
        assert_eq!(back.score, c.score);
        assert_eq!(back.explanation, c.explanation);
    }

    #[test]
    fn level_serializes_as_snake_case() {
        let json = serde_json::to_string(&MemoryConfidenceLevel::High).unwrap();
        assert_eq!(json, "\"high\"");
    }
}
