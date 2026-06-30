//! A single step in a receipt's reasoning trail.
//!
//! Reasoning steps are ordered, auditable summaries of how a conclusion was
//! reached. They are intentionally concise — they do not attempt to expose
//! private LLM chain-of-thought, but rather capture the structured reasoning
//! steps that an engineer can review and trust.

use serde::{Deserialize, Serialize};

use rivora_types::NonEmptyString;

/// A single step in a receipt's reasoning trail.
///
/// Steps are ordered (by `step` number, starting at 1) and each step
/// references evidence it consumed and a conclusion it reached.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReasoningStep {
    /// The step number, starting at 1. Steps must be strictly increasing.
    pub step: u32,
    /// A short title for this reasoning step.
    pub title: NonEmptyString,
    /// A concise explanation of what this step does and why.
    pub explanation: NonEmptyString,
    /// References to evidence consumed by this step (e.g. evidence IDs,
    /// titles, or source references). May be empty for steps that operate
    /// only on prior step outputs.
    pub input_evidence: Vec<NonEmptyString>,
    /// The conclusion or intermediate output of this step.
    pub output_conclusion: NonEmptyString,
    /// The impact of this step on the overall confidence, in `[-1.0, 1.0]`.
    /// Positive values increase confidence; negative values decrease it.
    pub confidence_impact: f64,
}

impl ReasoningStep {
    /// Creates a new `ReasoningStep` with required fields.
    ///
    /// # Errors
    ///
    /// Returns an error if `step` is 0, if `confidence_impact` is not in
    /// `[-1.0, 1.0]`, or if any required string field is empty.
    pub fn new(
        step: u32,
        title: impl Into<String>,
        explanation: impl Into<String>,
        output_conclusion: impl Into<String>,
        confidence_impact: f64,
    ) -> Result<Self, rivora_errors::RivoraError> {
        if step == 0 {
            return Err(rivora_errors::RivoraError::invalid_value(
                "step",
                "step number must be >= 1",
            ));
        }
        if !(-1.0..=1.0).contains(&confidence_impact) {
            return Err(rivora_errors::RivoraError::invalid_value(
                "confidence_impact",
                format!("must be in [-1.0, 1.0], got {confidence_impact}"),
            ));
        }
        Ok(Self {
            step,
            title: NonEmptyString::new(title.into())?,
            explanation: NonEmptyString::new(explanation.into())?,
            input_evidence: Vec::new(),
            output_conclusion: NonEmptyString::new(output_conclusion.into())?,
            confidence_impact,
        })
    }

    /// Builder-style setter for `input_evidence`.
    #[must_use]
    pub fn with_input_evidence(mut self, input_evidence: Vec<NonEmptyString>) -> Self {
        self.input_evidence = input_evidence;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_step_zero() {
        let result = ReasoningStep::new(0, "title", "explanation", "conclusion", 0.5);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_invalid_confidence_impact() {
        let result = ReasoningStep::new(1, "title", "explanation", "conclusion", 2.0);
        assert!(result.is_err());
        let result = ReasoningStep::new(1, "title", "explanation", "conclusion", -2.0);
        assert!(result.is_err());
    }

    #[test]
    fn accepts_valid_step() {
        let s = ReasoningStep::new(1, "title", "explanation", "conclusion", 0.3).unwrap();
        assert_eq!(s.step, 1);
        assert_eq!(s.confidence_impact, 0.3);
    }

    #[test]
    fn accepts_zero_confidence_impact() {
        let s = ReasoningStep::new(1, "title", "explanation", "conclusion", 0.0).unwrap();
        assert_eq!(s.confidence_impact, 0.0);
    }

    #[test]
    fn accepts_negative_confidence_impact() {
        let s = ReasoningStep::new(1, "title", "explanation", "conclusion", -0.5).unwrap();
        assert_eq!(s.confidence_impact, -0.5);
    }

    #[test]
    fn round_trips_through_serde() {
        let s = ReasoningStep::new(1, "title", "explanation", "conclusion", 0.3)
            .unwrap()
            .with_input_evidence(vec![NonEmptyString::new("ev-1").unwrap()]);
        let json = serde_json::to_string(&s).unwrap();
        let back: ReasoningStep = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn rejects_empty_title() {
        let result = ReasoningStep::new(1, "", "explanation", "conclusion", 0.0);
        assert!(result.is_err());
    }
}
