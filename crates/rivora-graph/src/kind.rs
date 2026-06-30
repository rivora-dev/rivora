//! Node and edge kind discriminators for the context graph.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Organization,
    Service,
    Deployment,
    Incident,
    Environment,
    Repository,
    Team,
    Owner,
    Dependency,
    Resource,
    Signal,
    Receipt,
    Ability,
    ExternalSystem,
    Unknown,
}

impl NodeKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Organization => "organization",
            Self::Service => "service",
            Self::Deployment => "deployment",
            Self::Incident => "incident",
            Self::Environment => "environment",
            Self::Repository => "repository",
            Self::Team => "team",
            Self::Owner => "owner",
            Self::Dependency => "dependency",
            Self::Resource => "resource",
            Self::Signal => "signal",
            Self::Receipt => "receipt",
            Self::Ability => "ability",
            Self::ExternalSystem => "external_system",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for NodeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Owns,
    DependsOn,
    DeployedTo,
    Triggered,
    Affected,
    Observed,
    Explains,
    Supports,
    Generated,
    RunsIn,
    BelongsTo,
    References,
    RelatedTo,
    Supersedes,
    Unknown,
}

impl EdgeKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owns => "owns",
            Self::DependsOn => "depends_on",
            Self::DeployedTo => "deployed_to",
            Self::Triggered => "triggered",
            Self::Affected => "affected",
            Self::Observed => "observed",
            Self::Explains => "explains",
            Self::Supports => "supports",
            Self::Generated => "generated",
            Self::RunsIn => "runs_in",
            Self::BelongsTo => "belongs_to",
            Self::References => "references",
            Self::RelatedTo => "related_to",
            Self::Supersedes => "supersedes",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for EdgeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_kind_as_str_is_lowercase_and_stable() {
        assert_eq!(NodeKind::Organization.as_str(), "organization");
        assert_eq!(NodeKind::Service.as_str(), "service");
        assert_eq!(NodeKind::Deployment.as_str(), "deployment");
        assert_eq!(NodeKind::Incident.as_str(), "incident");
        assert_eq!(NodeKind::Environment.as_str(), "environment");
        assert_eq!(NodeKind::Repository.as_str(), "repository");
        assert_eq!(NodeKind::Team.as_str(), "team");
        assert_eq!(NodeKind::Owner.as_str(), "owner");
        assert_eq!(NodeKind::Dependency.as_str(), "dependency");
        assert_eq!(NodeKind::Resource.as_str(), "resource");
        assert_eq!(NodeKind::Signal.as_str(), "signal");
        assert_eq!(NodeKind::Receipt.as_str(), "receipt");
        assert_eq!(NodeKind::Ability.as_str(), "ability");
        assert_eq!(NodeKind::ExternalSystem.as_str(), "external_system");
        assert_eq!(NodeKind::Unknown.as_str(), "unknown");
    }

    #[test]
    fn node_kind_serializes_as_snake_case_tag() {
        let json = serde_json::to_string(&NodeKind::ExternalSystem).unwrap();
        assert_eq!(json, "\"external_system\"");
    }

    #[test]
    fn node_kind_round_trips_through_serde() {
        let kind = NodeKind::Deployment;
        let json = serde_json::to_string(&kind).unwrap();
        let back: NodeKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back, kind);
    }

    #[test]
    fn node_kind_display_matches_as_str() {
        assert_eq!(NodeKind::Ability.to_string(), "ability");
    }

    #[test]
    fn edge_kind_as_str_is_lowercase_and_stable() {
        assert_eq!(EdgeKind::Owns.as_str(), "owns");
        assert_eq!(EdgeKind::DependsOn.as_str(), "depends_on");
        assert_eq!(EdgeKind::DeployedTo.as_str(), "deployed_to");
        assert_eq!(EdgeKind::Triggered.as_str(), "triggered");
        assert_eq!(EdgeKind::Affected.as_str(), "affected");
        assert_eq!(EdgeKind::Observed.as_str(), "observed");
        assert_eq!(EdgeKind::Explains.as_str(), "explains");
        assert_eq!(EdgeKind::Supports.as_str(), "supports");
        assert_eq!(EdgeKind::Generated.as_str(), "generated");
        assert_eq!(EdgeKind::RunsIn.as_str(), "runs_in");
        assert_eq!(EdgeKind::BelongsTo.as_str(), "belongs_to");
        assert_eq!(EdgeKind::References.as_str(), "references");
        assert_eq!(EdgeKind::RelatedTo.as_str(), "related_to");
        assert_eq!(EdgeKind::Supersedes.as_str(), "supersedes");
        assert_eq!(EdgeKind::Unknown.as_str(), "unknown");
    }

    #[test]
    fn edge_kind_serializes_as_snake_case_tag() {
        let json = serde_json::to_string(&EdgeKind::DeployedTo).unwrap();
        assert_eq!(json, "\"deployed_to\"");
    }

    #[test]
    fn edge_kind_round_trips_through_serde() {
        let kind = EdgeKind::DependsOn;
        let json = serde_json::to_string(&kind).unwrap();
        let back: EdgeKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back, kind);
    }

    #[test]
    fn edge_kind_display_matches_as_str() {
        assert_eq!(EdgeKind::DeployedTo.to_string(), "deployed_to");
    }
}
