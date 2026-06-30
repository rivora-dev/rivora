//! Risk assessment for a reliability receipt.

use serde::{Deserialize, Serialize};

use rivora_types::NonEmptyString;

/// A qualitative risk level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    /// Low risk — no significant impact expected.
    Low,
    /// Medium risk — some impact possible, mitigation likely available.
    Medium,
    /// High risk — significant impact possible, explicit approval required.
    High,
}

impl RiskLevel {
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

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A risk assessment for a receipt.
///
/// Risk is always explicit: a level plus a list of factors that contribute
/// to the risk. Risk is never hidden.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Risk {
    /// The qualitative risk level.
    pub level: RiskLevel,
    /// A free-text description of the risk.
    pub description: NonEmptyString,
    /// The services or systems affected by this risk (may be empty).
    pub affected_services: Vec<NonEmptyString>,
    /// Possible impacts of this risk (e.g. `"downtime"`, `"data loss"`).
    pub possible_impacts: Vec<NonEmptyString>,
    /// Suggested mitigation strategies (may be empty).
    pub mitigations: Vec<NonEmptyString>,
    /// Whether human approval is required to proceed given this risk.
    pub requires_approval: bool,
}

impl Risk {
    /// Creates a new `Risk` with required fields.
    ///
    /// # Errors
    ///
    /// Returns an error if `description` is empty.
    pub fn new(
        level: RiskLevel,
        description: impl Into<String>,
    ) -> Result<Self, rivora_errors::RivoraError> {
        let requires_approval = matches!(level, RiskLevel::Medium | RiskLevel::High);
        Ok(Self {
            level,
            description: NonEmptyString::new(description.into())?,
            affected_services: Vec::new(),
            possible_impacts: Vec::new(),
            mitigations: Vec::new(),
            requires_approval,
        })
    }

    /// Builder-style setter for `affected_services`.
    #[must_use]
    pub fn with_affected_services(mut self, services: Vec<NonEmptyString>) -> Self {
        self.affected_services = services;
        self
    }

    /// Builder-style setter for `possible_impacts`.
    #[must_use]
    pub fn with_possible_impacts(mut self, impacts: Vec<NonEmptyString>) -> Self {
        self.possible_impacts = impacts;
        self
    }

    /// Builder-style setter for `mitigations`.
    #[must_use]
    pub fn with_mitigations(mut self, mitigations: Vec<NonEmptyString>) -> Self {
        self.mitigations = mitigations;
        self
    }

    /// Builder-style setter for `requires_approval`.
    #[must_use]
    pub fn with_requires_approval(mut self, requires: bool) -> Self {
        self.requires_approval = requires;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn risk_level_as_str() {
        assert_eq!(RiskLevel::Low.as_str(), "low");
        assert_eq!(RiskLevel::Medium.as_str(), "medium");
        assert_eq!(RiskLevel::High.as_str(), "high");
    }

    #[test]
    fn risk_level_ordering() {
        assert!(RiskLevel::Low < RiskLevel::Medium);
        assert!(RiskLevel::Medium < RiskLevel::High);
    }

    #[test]
    fn medium_and_high_levels_require_approval_by_default() {
        let low = Risk::new(RiskLevel::Low, "minor").unwrap();
        assert!(!low.requires_approval);
        let med = Risk::new(RiskLevel::Medium, "moderate").unwrap();
        assert!(med.requires_approval);
        let high = Risk::new(RiskLevel::High, "severe").unwrap();
        assert!(high.requires_approval);
    }

    #[test]
    fn risk_rejects_empty_description() {
        let result = Risk::new(RiskLevel::Low, "");
        assert!(result.is_err());
    }

    #[test]
    fn risk_accepts_valid_fields() {
        let r = Risk::new(RiskLevel::Medium, "Service degradation possible")
            .unwrap()
            .with_affected_services(vec![NonEmptyString::new("api-gateway").unwrap()])
            .with_possible_impacts(vec![NonEmptyString::new("increased latency").unwrap()])
            .with_mitigations(vec![NonEmptyString::new("rollback").unwrap()]);
        assert_eq!(r.level, RiskLevel::Medium);
        assert!(r.requires_approval);
        assert_eq!(r.affected_services.len(), 1);
    }

    #[test]
    fn risk_round_trips_through_serde() {
        let r = Risk::new(RiskLevel::High, "severe").unwrap();
        let json = serde_json::to_string(&r).unwrap();
        let back: Risk = serde_json::from_str(&json).unwrap();
        assert_eq!(back, r);
    }

    #[test]
    fn risk_level_serializes_as_snake_case() {
        let json = serde_json::to_string(&RiskLevel::Medium).unwrap();
        assert_eq!(json, "\"medium\"");
    }
}
