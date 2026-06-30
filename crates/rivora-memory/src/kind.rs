//! Memory kind discriminators for the context memory model.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    Fact,
    Pattern,
    Preference,
    Convention,
    IncidentLearning,
    DeploymentLearning,
    ServiceRelationship,
    OperationalNote,
    RunbookKnowledge,
    TeamKnowledge,
    RiskKnowledge,
    ReceiptLearning,
    AbilityLearning,
    Unknown,
}

impl MemoryKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fact => "fact",
            Self::Pattern => "pattern",
            Self::Preference => "preference",
            Self::Convention => "convention",
            Self::IncidentLearning => "incident_learning",
            Self::DeploymentLearning => "deployment_learning",
            Self::ServiceRelationship => "service_relationship",
            Self::OperationalNote => "operational_note",
            Self::RunbookKnowledge => "runbook_knowledge",
            Self::TeamKnowledge => "team_knowledge",
            Self::RiskKnowledge => "risk_knowledge",
            Self::ReceiptLearning => "receipt_learning",
            Self::AbilityLearning => "ability_learning",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for MemoryKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_as_str_is_lowercase_and_stable() {
        assert_eq!(MemoryKind::Fact.as_str(), "fact");
        assert_eq!(MemoryKind::Pattern.as_str(), "pattern");
        assert_eq!(MemoryKind::Preference.as_str(), "preference");
        assert_eq!(MemoryKind::Convention.as_str(), "convention");
        assert_eq!(MemoryKind::IncidentLearning.as_str(), "incident_learning");
        assert_eq!(
            MemoryKind::DeploymentLearning.as_str(),
            "deployment_learning"
        );
        assert_eq!(
            MemoryKind::ServiceRelationship.as_str(),
            "service_relationship"
        );
        assert_eq!(MemoryKind::OperationalNote.as_str(), "operational_note");
        assert_eq!(MemoryKind::RunbookKnowledge.as_str(), "runbook_knowledge");
        assert_eq!(MemoryKind::TeamKnowledge.as_str(), "team_knowledge");
        assert_eq!(MemoryKind::RiskKnowledge.as_str(), "risk_knowledge");
        assert_eq!(MemoryKind::ReceiptLearning.as_str(), "receipt_learning");
        assert_eq!(MemoryKind::AbilityLearning.as_str(), "ability_learning");
        assert_eq!(MemoryKind::Unknown.as_str(), "unknown");
    }

    #[test]
    fn kind_serializes_as_snake_case_tag() {
        let json = serde_json::to_string(&MemoryKind::IncidentLearning).unwrap();
        assert_eq!(json, "\"incident_learning\"");
    }

    #[test]
    fn kind_round_trips_through_serde() {
        let kind = MemoryKind::DeploymentLearning;
        let json = serde_json::to_string(&kind).unwrap();
        let back: MemoryKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back, kind);
    }

    #[test]
    fn kind_display_matches_as_str() {
        assert_eq!(MemoryKind::AbilityLearning.to_string(), "ability_learning");
    }

    #[test]
    fn all_variants_round_trip_through_serde() {
        let variants = [
            MemoryKind::Fact,
            MemoryKind::Pattern,
            MemoryKind::Preference,
            MemoryKind::Convention,
            MemoryKind::IncidentLearning,
            MemoryKind::DeploymentLearning,
            MemoryKind::ServiceRelationship,
            MemoryKind::OperationalNote,
            MemoryKind::RunbookKnowledge,
            MemoryKind::TeamKnowledge,
            MemoryKind::RiskKnowledge,
            MemoryKind::ReceiptLearning,
            MemoryKind::AbilityLearning,
            MemoryKind::Unknown,
        ];
        for kind in variants {
            let json = serde_json::to_string(&kind).unwrap();
            let back: MemoryKind = serde_json::from_str(&json).unwrap();
            assert_eq!(back, kind);
        }
    }
}
