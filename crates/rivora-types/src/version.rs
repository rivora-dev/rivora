//! Semantic version primitive.
//!
//! [`Version`] is a thin newtype over [`semver::Version`] so the rest of
//! Rivora depends on a single, serialization-consistent version type rather
//! than raw strings or a third-party type leaking through every API.

use std::fmt;
use std::str::FromStr;

use rivora_errors::RivoraError;
use semver::Version as SemverVersion;
use serde::{Deserialize, Serialize};

/// A semantic version (major.minor.patch), with optional pre-release/build
/// metadata carried by the underlying [`semver::Version`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Version(SemverVersion);

impl Version {
    /// Creates a version from explicit numeric components.
    #[must_use]
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self(SemverVersion::new(major, minor, patch))
    }

    /// Parses a semantic version string.
    ///
    /// # Errors
    /// Returns [`RivoraError::InvalidVersion`] if the string is not valid semver.
    pub fn parse(input: impl AsRef<str>) -> Result<Self, RivoraError> {
        let input = input.as_ref();
        SemverVersion::parse(input)
            .map(Self)
            .map_err(|e| RivoraError::invalid_version(input, e.to_string()))
    }

    /// The underlying [`semver::Version`].
    #[must_use]
    pub fn as_semver(&self) -> &SemverVersion {
        &self.0
    }

    /// Consumes and returns the underlying [`semver::Version`].
    #[must_use]
    pub fn into_semver(self) -> SemverVersion {
        self.0
    }

    #[must_use]
    pub fn major(&self) -> u64 {
        self.0.major
    }

    #[must_use]
    pub fn minor(&self) -> u64 {
        self.0.minor
    }

    #[must_use]
    pub fn patch(&self) -> u64 {
        self.0.patch
    }
}

impl FromStr for Version {
    type Err = RivoraError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl TryFrom<String> for Version {
    type Error = RivoraError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl From<Version> for String {
    fn from(v: Version) -> String {
        v.0.to_string()
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_numeric_version() {
        let v = Version::new(1, 2, 3);
        assert_eq!(v.to_string(), "1.2.3");
        assert_eq!((v.major(), v.minor(), v.patch()), (1, 2, 3));
    }

    #[test]
    fn parse_accepts_semver() {
        let v = Version::parse("1.0.0").unwrap();
        assert_eq!(v.to_string(), "1.0.0");

        let pre = Version::parse("1.0.0-beta.1+build.42").unwrap();
        assert!(pre.to_string().contains("beta.1"));
    }

    #[test]
    fn parse_rejects_non_semver() {
        let err = Version::parse("not-a-version").unwrap_err();
        assert_eq!(err.kind(), rivora_errors::ErrorKind::InvalidVersion);
        assert!(err.to_string().contains("not-a-version"));
    }

    #[test]
    fn parse_rejects_missing_patch() {
        assert!(Version::parse("1.2").is_err());
    }

    #[test]
    fn serde_round_trips_as_string() {
        let v = Version::new(0, 1, 0);
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, "\"0.1.0\"");
        let back: Version = serde_json::from_str(&json).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn serde_rejects_invalid_version_string() {
        assert!(serde_json::from_str::<Version>("\"oops\"").is_err());
    }

    #[test]
    fn ordering_follows_semver() {
        assert!(Version::new(1, 0, 0) < Version::new(2, 0, 0));
        assert!(Version::new(2, 0, 0) < Version::new(2, 1, 0));
        assert!(Version::new(2, 1, 0) < Version::new(2, 1, 1));
    }

    #[test]
    fn from_str_works() {
        let v: Version = "3.4.5".parse().unwrap();
        assert_eq!(v.major(), 3);
    }
}
