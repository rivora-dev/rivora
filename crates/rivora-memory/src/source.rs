//! Memory source discriminators for the context memory model.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemorySource {
    Human,
    Receipt,
    Graph,
    Ability,
    Connector,
    Inference,
    System,
    Unknown,
}

impl MemorySource {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Receipt => "receipt",
            Self::Graph => "graph",
            Self::Ability => "ability",
            Self::Connector => "connector",
            Self::Inference => "inference",
            Self::System => "system",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for MemorySource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_as_str_is_lowercase_and_stable() {
        assert_eq!(MemorySource::Human.as_str(), "human");
        assert_eq!(MemorySource::Receipt.as_str(), "receipt");
        assert_eq!(MemorySource::Graph.as_str(), "graph");
        assert_eq!(MemorySource::Ability.as_str(), "ability");
        assert_eq!(MemorySource::Connector.as_str(), "connector");
        assert_eq!(MemorySource::Inference.as_str(), "inference");
        assert_eq!(MemorySource::System.as_str(), "system");
        assert_eq!(MemorySource::Unknown.as_str(), "unknown");
    }

    #[test]
    fn source_serializes_as_snake_case_tag() {
        let json = serde_json::to_string(&MemorySource::Connector).unwrap();
        assert_eq!(json, "\"connector\"");
    }

    #[test]
    fn source_round_trips_through_serde() {
        let source = MemorySource::Inference;
        let json = serde_json::to_string(&source).unwrap();
        let back: MemorySource = serde_json::from_str(&json).unwrap();
        assert_eq!(back, source);
    }

    #[test]
    fn source_display_matches_as_str() {
        assert_eq!(MemorySource::Receipt.to_string(), "receipt");
    }
}
