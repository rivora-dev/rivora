//! Non-empty text primitive.
//!
//! [`NonEmptyString`] replaces raw [`String`] for fields where an empty value
//! is meaningless (names, identifiers provided by humans, configuration
//! values). It serializes transparently as a string and validates on
//! deserialize.

use std::fmt;
use std::str::FromStr;

use rivora_errors::RivoraError;
use serde::{Deserialize, Serialize};

/// Maximum accepted length for a [`NonEmptyString`].
pub const MAX_TEXT_LEN: usize = 4096;

/// A string that is guaranteed non-empty (after trimming whitespace) and not
/// longer than [`MAX_TEXT_LEN`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct NonEmptyString(String);

impl NonEmptyString {
    /// Creates a non-empty string, rejecting blank or over-long input.
    ///
    /// # Errors
    /// Returns [`RivoraError::InvalidValue`] if the input is blank or exceeds
    /// [`MAX_TEXT_LEN`].
    pub fn new(value: impl Into<String>) -> Result<Self, RivoraError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(RivoraError::invalid_value(
                "non_empty_string",
                "must not be empty or blank",
            ));
        }
        if value.len() > MAX_TEXT_LEN {
            return Err(RivoraError::invalid_value(
                "non_empty_string",
                format!("must be at most {MAX_TEXT_LEN} characters"),
            ));
        }
        Ok(Self(value))
    }

    /// The inner string value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The inner string value (owned).
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl FromStr for NonEmptyString {
    type Err = RivoraError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl TryFrom<String> for NonEmptyString {
    type Error = RivoraError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<NonEmptyString> for String {
    fn from(s: NonEmptyString) -> String {
        s.0
    }
}

impl fmt::Display for NonEmptyString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for NonEmptyString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_accepts_non_blank() {
        let s = NonEmptyString::new("hello").unwrap();
        assert_eq!(s.as_str(), "hello");
    }

    #[test]
    fn new_rejects_empty() {
        assert!(NonEmptyString::new("").is_err());
    }

    #[test]
    fn new_rejects_blank_whitespace() {
        assert!(NonEmptyString::new("   \n\t").is_err());
    }

    #[test]
    fn new_rejects_too_long() {
        let long = "a".repeat(MAX_TEXT_LEN + 1);
        let err = NonEmptyString::new(long).unwrap_err();
        assert_eq!(err.kind(), rivora_errors::ErrorKind::InvalidValue);
        assert!(err.to_string().contains("at most"));
    }

    #[test]
    fn serde_round_trips() {
        let s = NonEmptyString::new("team-payments").unwrap();
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, "\"team-payments\"");
        let back: NonEmptyString = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn serde_rejects_blank_on_deserialize() {
        assert!(serde_json::from_str::<NonEmptyString>("\"   \"").is_err());
    }

    #[test]
    fn from_str_works() {
        let s: NonEmptyString = "ok".parse().unwrap();
        assert_eq!(s.as_str(), "ok");
    }

    #[test]
    fn as_ref_str_works() {
        let s = NonEmptyString::new("ok").unwrap();
        let r: &str = s.as_ref();
        assert_eq!(r, "ok");
    }
}
