//! Graph-specific confidence values.

use serde::{Deserialize, Serialize};

use rivora_types::NonEmptyString;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphConfidenceLevel {
    Low,
    Medium,
    High,
}

impl GraphConfidenceLevel {
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

impl std::fmt::Display for GraphConfidenceLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphConfidence {
    pub score: f64,
    pub level: GraphConfidenceLevel,
    pub explanation: NonEmptyString,
    pub contributing_factors: Vec<NonEmptyString>,
    pub limiting_factors: Vec<NonEmptyString>,
}

impl GraphConfidence {
    pub fn new(
        score: f64,
        explanation: impl Into<String>,
    ) -> Result<Self, rivora_errors::RivoraError> {
        if !(0.0..=1.0).contains(&score) {
            return Err(rivora_errors::RivoraError::invalid_value(
                "score",
                format!("must be in [0.0, 1.0], got {score}"),
            ));
        }
        Ok(Self {
            score,
            level: GraphConfidenceLevel::from_score(score),
            explanation: NonEmptyString::new(explanation.into())?,
            contributing_factors: Vec::new(),
            limiting_factors: Vec::new(),
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
            GraphConfidenceLevel::from_score(0.0),
            GraphConfidenceLevel::Low
        );
        assert_eq!(
            GraphConfidenceLevel::from_score(0.39),
            GraphConfidenceLevel::Low
        );
        assert_eq!(
            GraphConfidenceLevel::from_score(0.4),
            GraphConfidenceLevel::Medium
        );
        assert_eq!(
            GraphConfidenceLevel::from_score(0.69),
            GraphConfidenceLevel::Medium
        );
        assert_eq!(
            GraphConfidenceLevel::from_score(0.7),
            GraphConfidenceLevel::High
        );
        assert_eq!(
            GraphConfidenceLevel::from_score(1.0),
            GraphConfidenceLevel::High
        );
    }

    #[test]
    fn level_ordering() {
        assert!(GraphConfidenceLevel::Low < GraphConfidenceLevel::Medium);
        assert!(GraphConfidenceLevel::Medium < GraphConfidenceLevel::High);
    }

    #[test]
    fn confidence_rejects_invalid_score() {
        assert!(GraphConfidence::new(1.5, "explanation").is_err());
        assert!(GraphConfidence::new(-0.1, "explanation").is_err());
    }

    #[test]
    fn confidence_rejects_empty_explanation() {
        assert!(GraphConfidence::new(0.5, "").is_err());
    }

    #[test]
    fn confidence_accepts_valid_fields() {
        let c = GraphConfidence::new(0.85, "Strong evidence").unwrap();
        assert_eq!(c.score, 0.85);
        assert_eq!(c.level, GraphConfidenceLevel::High);
    }

    #[test]
    fn confidence_level_derived_from_score() {
        let low = GraphConfidence::new(0.2, "weak").unwrap();
        assert_eq!(low.level, GraphConfidenceLevel::Low);
        let mid = GraphConfidence::new(0.5, "moderate").unwrap();
        assert_eq!(mid.level, GraphConfidenceLevel::Medium);
        let high = GraphConfidence::new(0.9, "strong").unwrap();
        assert_eq!(high.level, GraphConfidenceLevel::High);
    }

    #[test]
    fn confidence_with_factors() {
        let c = GraphConfidence::new(0.75, "evidence-based")
            .unwrap()
            .with_contributing_factors(vec![NonEmptyString::new("stable pattern").unwrap()])
            .with_limiting_factors(vec![NonEmptyString::new("small sample").unwrap()]);
        assert_eq!(c.contributing_factors.len(), 1);
        assert_eq!(c.limiting_factors.len(), 1);
    }

    #[test]
    fn confidence_round_trips_through_serde() {
        let c = GraphConfidence::new(0.85, "Strong evidence")
            .unwrap()
            .with_contributing_factors(vec![NonEmptyString::new("stable pattern").unwrap()])
            .with_limiting_factors(vec![NonEmptyString::new("small sample").unwrap()]);
        let json = serde_json::to_string(&c).unwrap();
        let back: GraphConfidence = serde_json::from_str(&json).unwrap();
        assert_eq!(back.score, c.score);
        assert_eq!(back.explanation, c.explanation);
    }

    #[test]
    fn level_serializes_as_snake_case() {
        let json = serde_json::to_string(&GraphConfidenceLevel::High).unwrap();
        assert_eq!(json, "\"high\"");
    }
}
