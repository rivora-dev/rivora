//! Typed version kinds for Open Rivora.
//!
//! [`TypedVersion<Tag>`] wraps [`rivora_types::Version`] and tags it so a
//! [`SchemaVersion`] cannot be confused with an [`AbilityVersion`] or a
//! [`ConnectorVersion`]. This is the "Version types" foundational set; the
//! underlying semver mechanics live in [`rivora_types::Version`].

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::str::FromStr;

use rivora_errors::RivoraError;
use rivora_types::{IdTag, Version};
use serde::{Deserialize, Serialize};

/// A type-tagged semantic version.
#[derive(Clone)]
pub struct TypedVersion<Tag>(Version, PhantomData<fn() -> Tag>);

impl<Tag> TypedVersion<Tag> {
    /// Creates a version from explicit numeric components.
    #[must_use]
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self(Version::new(major, minor, patch), PhantomData)
    }

    /// Parses a semantic version string.
    ///
    /// # Errors
    /// Returns [`RivoraError::InvalidVersion`] if the string is not valid semver.
    pub fn parse(input: impl AsRef<str>) -> Result<Self, RivoraError> {
        Version::parse(input).map(|v| Self(v, PhantomData))
    }

    /// The underlying [`Version`].
    #[must_use]
    pub fn as_version(&self) -> &Version {
        &self.0
    }

    #[must_use]
    pub fn major(&self) -> u64 {
        self.0.major()
    }

    #[must_use]
    pub fn minor(&self) -> u64 {
        self.0.minor()
    }

    #[must_use]
    pub fn patch(&self) -> u64 {
        self.0.patch()
    }
}

impl<Tag> FromStr for TypedVersion<Tag> {
    type Err = RivoraError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl<Tag> Serialize for TypedVersion<Tag> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de, Tag> Deserialize<'de> for TypedVersion<Tag> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse(raw).map_err(serde::de::Error::custom)
    }
}

impl<Tag> PartialEq for TypedVersion<Tag> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<Tag> Eq for TypedVersion<Tag> {}

impl<Tag> PartialOrd for TypedVersion<Tag> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<Tag> Ord for TypedVersion<Tag> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl<Tag> Hash for TypedVersion<Tag> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<Tag: IdTag> fmt::Display for TypedVersion<Tag> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<Tag: IdTag> fmt::Debug for TypedVersion<Tag> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", Tag::KIND, self.0)
    }
}

/// Marker for schema versions (receipts, context-graph node/edge schemas).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Schema;
impl IdTag for Schema {
    const KIND: &'static str = "schema";
}

/// Marker for Ability versions. Reuses the [`crate::id::Ability`] domain marker.
// Use the same marker as AbilityId so "ability" is one domain concept.
pub type AbilityVersionTag = crate::id::Ability;

/// Marker for connector / source versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Connector;
impl IdTag for Connector {
    const KIND: &'static str = "connector";
}

/// Version of a versioned schema (e.g. receipt schema `1.0.0`).
pub type SchemaVersion = TypedVersion<Schema>;
/// Version of an Ability (immutable per version; see docs/06).
pub type AbilityVersion = TypedVersion<AbilityVersionTag>;
/// Version of a connector / source that produced an observation.
pub type ConnectorVersion = TypedVersion<Connector>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_accessors() {
        let v = SchemaVersion::parse("1.2.3").unwrap();
        assert_eq!((v.major(), v.minor(), v.patch()), (1, 2, 3));
        assert_eq!(v.to_string(), "1.2.3");
    }

    #[test]
    fn parse_rejects_invalid() {
        let err = SchemaVersion::parse("nope").unwrap_err();
        assert_eq!(err.kind(), rivora_errors::ErrorKind::InvalidVersion);
    }

    #[test]
    fn serde_round_trips() {
        let v = AbilityVersion::new(0, 1, 0);
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, "\"0.1.0\"");
        let back: AbilityVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn ordering_follows_semver() {
        assert!(SchemaVersion::new(1, 0, 0) < SchemaVersion::new(1, 0, 1));
    }

    #[test]
    fn debug_is_kinded() {
        let v = SchemaVersion::new(1, 0, 0);
        assert_eq!(format!("{v:?}"), "schema(1.0.0)");
    }

    #[test]
    fn distinct_version_kinds_do_not_mix() {
        // Compile-time: SchemaVersion and ConnectorVersion are distinct types.
        let s = SchemaVersion::new(1, 0, 0);
        let c = ConnectorVersion::new(1, 0, 0);
        assert_eq!(s.to_string(), c.to_string());
    }
}
