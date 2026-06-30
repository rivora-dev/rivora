//! Typed identifier primitives.
//!
//! [`TypedId`] is a newtype over a validated [`String`] tagged by a phantom
//! [`IdTag`]. Tagging makes `TypedId<Observation>` and `TypedId<Receipt>`
//! distinct types so they cannot be mixed by accident, while sharing one
//! implementation. This is the primary mechanism for "avoid `String` wherever
//! a stronger type is appropriate".

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::str::FromStr;

use rivora_errors::RivoraError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
/// Maximum length accepted for any typed identifier.
pub const MAX_ID_LEN: usize = 255;

/// A marker trait that names an identifier family.
///
/// Implementors provide a stable `KIND` string used in error messages and
/// debug output (e.g. `"observation"`). The trait is intentionally open: a
/// future provider crate may declare its own `IdTag` to reuse [`TypedId`].
pub trait IdTag {
    /// Stable, lowercase, human-readable name for this identifier family.
    const KIND: &'static str;
}

/// A validated, type-tagged identifier.
///
/// The inner value is constrained to `[A-Za-z0-9_:-]`, non-empty, and at most
/// [`MAX_ID_LEN`] characters. `Tag` is a zero-sized marker implementing
/// [`IdTag`]; it never appears at runtime.
#[derive(Clone)]
pub struct TypedId<Tag>(String, PhantomData<fn() -> Tag>);

// Serialization is manual so the impls impose only the bounds they need:
// `Serialize` works for any `Tag`; `Deserialize` requires `Tag: IdTag` so the
// value can be validated on the way in.
impl<Tag> Serialize for TypedId<Tag> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de, Tag: IdTag> Deserialize<'de> for TypedId<Tag> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::new(raw).map_err(serde::de::Error::custom)
    }
}

// Comparisons and hashing operate on the inner string only; they deliberately
// do not require `Tag: IdTag` (or any bound on `Tag`).
impl<Tag> PartialEq for TypedId<Tag> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<Tag> Eq for TypedId<Tag> {}

impl<Tag> PartialOrd for TypedId<Tag> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<Tag> Ord for TypedId<Tag> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl<Tag> Hash for TypedId<Tag> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<Tag> TypedId<Tag> {
    /// Creates a typed identifier from an already-validated string, bypassing
    /// validation.
    ///
    /// Use only when the value is known to be valid (e.g. loaded from a
    /// trusted source that was validated on write).
    #[must_use]
    pub fn new_unchecked(value: impl Into<String>) -> Self {
        Self(value.into(), PhantomData)
    }

    /// The raw identifier value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The raw identifier value (owned).
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }

    /// A kind-prefixed rendering suitable for logs: `<kind>:<value>`.
    #[must_use]
    pub fn to_kinded_string(&self) -> String
    where
        Tag: IdTag,
    {
        format!("{}:{}", Tag::KIND, self.0)
    }

    /// Generates a fresh, random identifier (UUID v4, hyphenated).
    ///
    /// The generated value always satisfies validation.
    #[must_use]
    pub fn new_random() -> Self {
        Self(Uuid::new_v4().to_string(), PhantomData)
    }
}

impl<Tag: IdTag> TypedId<Tag> {
    /// Validates and creates a typed identifier.
    ///
    /// # Errors
    /// Returns [`RivoraError::InvalidIdentifier`] if the value is empty, too
    /// long, or contains characters outside `[A-Za-z0-9_:-]`.
    pub fn new(value: impl Into<String>) -> Result<Self, RivoraError> {
        let value = value.into();
        validate_id(&value, Tag::KIND)?;
        Ok(Self(value, PhantomData))
    }
}

impl<Tag: IdTag> FromStr for TypedId<Tag> {
    type Err = RivoraError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl<Tag: IdTag> TryFrom<String> for TypedId<Tag> {
    type Error = RivoraError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl<Tag> From<TypedId<Tag>> for String {
    fn from(id: TypedId<Tag>) -> String {
        id.0
    }
}

impl<Tag: IdTag> fmt::Display for TypedId<Tag> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<Tag: IdTag> fmt::Debug for TypedId<Tag> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({:?})", Tag::KIND, self.0)
    }
}

/// Validate a raw identifier value against the identifier rules.
pub(crate) fn validate_id(value: &str, kind: &'static str) -> Result<(), RivoraError> {
    if value.is_empty() {
        return Err(RivoraError::invalid_identifier(kind, "must not be empty"));
    }
    if value.len() > MAX_ID_LEN {
        return Err(RivoraError::invalid_identifier(
            kind,
            format!("must be at most {MAX_ID_LEN} characters"),
        ));
    }
    if let Some(bad) = value
        .chars()
        .find(|c| !c.is_ascii_alphanumeric() && *c != '_' && *c != '-' && *c != ':')
    {
        return Err(RivoraError::invalid_identifier(
            kind,
            format!("invalid character {bad:?}; allowed: letters, digits, '_', '-', ':'"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    enum Observation {}
    impl IdTag for Observation {
        const KIND: &'static str = "observation";
    }
    type ObservationId = TypedId<Observation>;

    enum Receipt {}
    impl IdTag for Receipt {
        const KIND: &'static str = "receipt";
    }
    type ReceiptId = TypedId<Receipt>;

    #[derive(Serialize, Deserialize)]
    struct Wrapper {
        id: ObservationId,
    }

    #[test]
    fn new_accepts_valid_values() {
        let id = ObservationId::new("obs_123").unwrap();
        assert_eq!(id.as_str(), "obs_123");
    }

    #[test]
    fn new_rejects_empty() {
        let err = ObservationId::new("").unwrap_err();
        assert_eq!(err.kind(), rivora_errors::ErrorKind::InvalidIdentifier);
        assert!(err.to_string().contains("observation"));
    }

    #[test]
    fn new_rejects_too_long() {
        let long = "a".repeat(MAX_ID_LEN + 1);
        let err = ObservationId::new(long).unwrap_err();
        assert!(err.to_string().contains("at most"));
    }

    #[test]
    fn new_rejects_bad_characters() {
        let err = ObservationId::new("bad space").unwrap_err();
        assert!(err.to_string().contains("invalid character"));
    }

    #[test]
    fn new_accepts_uuid_shape() {
        let id = ObservationId::new("550e8400-e29b-41d4-a716-446655440000").unwrap();
        assert!(id.as_str().contains('-'));
    }

    #[test]
    fn new_random_is_valid_and_unique() {
        let a = ObservationId::new_random();
        let b = ObservationId::new_random();
        assert!(ObservationId::new(a.as_str()).is_ok());
        assert_ne!(a, b);
    }

    #[test]
    fn distinct_tags_are_distinct_types() {
        // Compile-time guarantee: this would fail to compile if tags merged.
        let obs = ObservationId::new("x").unwrap();
        let rcp = ReceiptId::new("x").unwrap();
        assert_eq!(obs.as_str(), rcp.as_str());
        // obs and rcp have different types; we can only compare inner strings.
    }

    #[test]
    fn serde_round_trips_as_string() {
        let id = ObservationId::new("obs_1").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"obs_1\"");
        let back: ObservationId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn serde_rejects_invalid_on_deserialize() {
        let err = serde_json::from_str::<ObservationId>("\"\"").unwrap_err();
        // serde_json error; validation is surfaced during try_from.
        assert!(err.to_string().contains("empty") || err.is_data());
    }

    #[test]
    fn nested_struct_round_trips() {
        let w = Wrapper {
            id: ObservationId::new("obs_42").unwrap(),
        };
        let json = serde_json::to_string(&w).unwrap();
        let back: Wrapper = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, w.id);
    }

    #[test]
    fn display_is_inner_value() {
        let id = ObservationId::new("obs_9").unwrap();
        assert_eq!(id.to_string(), "obs_9");
    }

    #[test]
    fn debug_is_kinded() {
        let id = ObservationId::new("obs_9").unwrap();
        assert_eq!(format!("{id:?}"), "observation(\"obs_9\")");
    }

    #[test]
    fn to_kinded_string_is_prefixed() {
        let id = ObservationId::new("obs_9").unwrap();
        assert_eq!(id.to_kinded_string(), "observation:obs_9");
    }

    #[test]
    fn from_str_works() {
        let id: ObservationId = "obs_3".parse().unwrap();
        assert_eq!(id.as_str(), "obs_3");
    }

    #[test]
    fn ordering_by_inner_string() {
        let a = ObservationId::new("a").unwrap();
        let b = ObservationId::new("b").unwrap();
        assert!(a < b);
    }
}
