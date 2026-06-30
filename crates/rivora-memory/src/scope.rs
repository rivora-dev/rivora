//! Memory scope discriminators for the context memory model.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScope {
    Organization,
    Team,
    Service,
    Environment,
    Repository,
    Incident,
    Deployment,
    Ability,
    Global,
    Unknown,
}

impl MemoryScope {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Organization => "organization",
            Self::Team => "team",
            Self::Service => "service",
            Self::Environment => "environment",
            Self::Repository => "repository",
            Self::Incident => "incident",
            Self::Deployment => "deployment",
            Self::Ability => "ability",
            Self::Global => "global",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for MemoryScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_as_str_is_lowercase_and_stable() {
        assert_eq!(MemoryScope::Organization.as_str(), "organization");
        assert_eq!(MemoryScope::Team.as_str(), "team");
        assert_eq!(MemoryScope::Service.as_str(), "service");
        assert_eq!(MemoryScope::Environment.as_str(), "environment");
        assert_eq!(MemoryScope::Repository.as_str(), "repository");
        assert_eq!(MemoryScope::Incident.as_str(), "incident");
        assert_eq!(MemoryScope::Deployment.as_str(), "deployment");
        assert_eq!(MemoryScope::Ability.as_str(), "ability");
        assert_eq!(MemoryScope::Global.as_str(), "global");
        assert_eq!(MemoryScope::Unknown.as_str(), "unknown");
    }

    #[test]
    fn scope_serializes_as_snake_case_tag() {
        let json = serde_json::to_string(&MemoryScope::Organization).unwrap();
        assert_eq!(json, "\"organization\"");
    }

    #[test]
    fn scope_round_trips_through_serde() {
        let scope = MemoryScope::Deployment;
        let json = serde_json::to_string(&scope).unwrap();
        let back: MemoryScope = serde_json::from_str(&json).unwrap();
        assert_eq!(back, scope);
    }

    #[test]
    fn scope_display_matches_as_str() {
        assert_eq!(MemoryScope::Ability.to_string(), "ability");
    }
}
