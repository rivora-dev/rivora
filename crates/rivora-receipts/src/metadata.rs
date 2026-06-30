//! Metadata, provenance, timestamps, and version for a reliability receipt.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use rivora_types::{NonEmptyString, Version};

/// Additional structured metadata about a receipt (tags, labels).
///
/// Metadata is intentionally generic — no provider-specific fields are
/// hard-coded. Future provider crates can attach provider-specific labels
/// through the `labels` map without modifying the core schema.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct ReceiptMetadata {
    /// Free-form tags (e.g. `"payments"`, `"production"`).
    pub tags: Vec<NonEmptyString>,
    /// Structured key-value labels.
    pub labels: BTreeMap<NonEmptyString, NonEmptyString>,
}

impl ReceiptMetadata {
    /// Creates empty metadata.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder-style setter for `tags`.
    #[must_use]
    pub fn with_tags(mut self, tags: Vec<NonEmptyString>) -> Self {
        self.tags = tags;
        self
    }

    /// Builder-style setter for `labels`.
    #[must_use]
    pub fn with_labels(mut self, labels: BTreeMap<NonEmptyString, NonEmptyString>) -> Self {
        self.labels = labels;
        self
    }
}

/// A reference to the inference provider that contributed to a receipt.
///
/// Per the spec, inference references must not contain secrets or PII. The
/// `request_id` is the only identifier that may identify a specific request
/// to the provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InferenceRef {
    /// The provider identifier (e.g. `"anthropic"`, `"openai"`).
    pub provider: NonEmptyString,
    /// The model identifier (e.g. `"claude-opus-4"`).
    pub model: NonEmptyString,
    /// The model version (e.g. `"20250101"`).
    pub model_version: NonEmptyString,
    /// The temperature used (0 for determinism).
    pub temperature: f64,
    /// The request identifier (no secrets, no PII).
    pub request_id: NonEmptyString,
}

impl InferenceRef {
    /// Creates a new `InferenceRef`.
    ///
    /// # Errors
    ///
    /// Returns an error if any required string field is empty.
    pub fn new(
        provider: impl Into<String>,
        model: impl Into<String>,
        model_version: impl Into<String>,
        temperature: f64,
        request_id: impl Into<String>,
    ) -> Result<Self, rivora_errors::RivoraError> {
        Ok(Self {
            provider: NonEmptyString::new(provider.into())?,
            model: NonEmptyString::new(model.into())?,
            model_version: NonEmptyString::new(model_version.into())?,
            temperature,
            request_id: NonEmptyString::new(request_id.into())?,
        })
    }
}

/// A reference to the Ability that produced a receipt.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AbilityRef {
    /// The ability identifier (e.g. `"payment-deployment-validator"`).
    pub id: NonEmptyString,
    /// The ability version (semver).
    pub version: NonEmptyString,
    /// The ability lifecycle status (e.g. `"approved"`, `"draft"`).
    pub status: NonEmptyString,
}

impl AbilityRef {
    /// Creates a new `AbilityRef`.
    ///
    /// # Errors
    ///
    /// Returns an error if any required string field is empty.
    pub fn new(
        id: impl Into<String>,
        version: impl Into<String>,
        status: impl Into<String>,
    ) -> Result<Self, rivora_errors::RivoraError> {
        Ok(Self {
            id: NonEmptyString::new(id.into())?,
            version: NonEmptyString::new(version.into())?,
            status: NonEmptyString::new(status.into())?,
        })
    }
}

/// Provenance information — who or what produced this receipt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReceiptProvenance {
    /// The system that produced this receipt
    /// (e.g. `"adaptive-engine"`, `"ability-runtime"`).
    pub source: NonEmptyString,
    /// The version of the system that produced this receipt.
    pub source_version: NonEmptyString,
    /// Optional inference reference (present when an inference provider
    /// contributed).
    pub inference: Option<InferenceRef>,
    /// Optional ability reference (present when an ability produced this
    /// receipt).
    pub ability: Option<AbilityRef>,
}

impl ReceiptProvenance {
    /// Creates a new `ReceiptProvenance` from required fields.
    ///
    /// # Errors
    ///
    /// Returns an error if `source` or `source_version` is empty.
    pub fn new(
        source: impl Into<String>,
        source_version: impl Into<String>,
    ) -> Result<Self, rivora_errors::RivoraError> {
        Ok(Self {
            source: NonEmptyString::new(source.into())?,
            source_version: NonEmptyString::new(source_version.into())?,
            inference: None,
            ability: None,
        })
    }

    /// Builder-style setter for `inference`.
    #[must_use]
    pub fn with_inference(mut self, inference: InferenceRef) -> Self {
        self.inference = Some(inference);
        self
    }

    /// Builder-style setter for `ability`.
    #[must_use]
    pub fn with_ability(mut self, ability: AbilityRef) -> Self {
        self.ability = Some(ability);
        self
    }
}

/// Timestamps attached to a receipt.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReceiptTimestamps {
    /// ISO-8601 timestamp of when the receipt was created.
    pub created_at: NonEmptyString,
    /// Optional ISO-8601 timestamp of when the receipt was last updated.
    pub updated_at: Option<NonEmptyString>,
    /// Optional ISO-8601 timestamp after which the receipt is no longer
    /// considered current.
    pub expires_at: Option<NonEmptyString>,
}

impl ReceiptTimestamps {
    /// Creates a new `ReceiptTimestamps` with `created_at` set.
    ///
    /// # Errors
    ///
    /// Returns an error if `created_at` is empty.
    pub fn new(created_at: impl Into<String>) -> Result<Self, rivora_errors::RivoraError> {
        Ok(Self {
            created_at: NonEmptyString::new(created_at.into())?,
            updated_at: None,
            expires_at: None,
        })
    }

    /// Builder-style setter for `updated_at`.
    #[must_use]
    pub fn with_updated_at(mut self, updated_at: impl Into<String>) -> Self {
        self.updated_at = Some(NonEmptyString::new(updated_at.into()).unwrap());
        self
    }

    /// Builder-style setter for `expires_at`.
    #[must_use]
    pub fn with_expires_at(mut self, expires_at: impl Into<String>) -> Self {
        self.expires_at = Some(NonEmptyString::new(expires_at.into()).unwrap());
        self
    }
}

/// The version of a receipt (schema and API).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReceiptVersion {
    /// The receipt schema version (semver).
    pub schema: Version,
    /// Optional API version (e.g. `"v1"`).
    pub api: Option<NonEmptyString>,
}

impl ReceiptVersion {
    /// Creates a new `ReceiptVersion` with the given schema version.
    #[must_use]
    pub fn new(schema: Version) -> Self {
        Self { schema, api: None }
    }

    /// Builder-style setter for `api`.
    #[must_use]
    pub fn with_api(mut self, api: impl Into<String>) -> Self {
        self.api = Some(NonEmptyString::new(api.into()).unwrap());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_default_is_empty() {
        let m = ReceiptMetadata::default();
        assert!(m.tags.is_empty());
        assert!(m.labels.is_empty());
    }

    #[test]
    fn metadata_round_trips() {
        let mut labels = BTreeMap::new();
        labels.insert(
            NonEmptyString::new("env").unwrap(),
            NonEmptyString::new("prod").unwrap(),
        );
        let m = ReceiptMetadata::new()
            .with_tags(vec![NonEmptyString::new("payments").unwrap()])
            .with_labels(labels);
        let json = serde_json::to_string(&m).unwrap();
        let back: ReceiptMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(back, m);
    }

    #[test]
    fn inference_ref_rejects_empty_fields() {
        assert!(InferenceRef::new("", "m", "v", 0.0, "req").is_err());
        assert!(InferenceRef::new("p", "", "v", 0.0, "req").is_err());
    }

    #[test]
    fn inference_ref_round_trips() {
        let r = InferenceRef::new("anthropic", "claude-opus-4", "20250101", 0.0, "req-1").unwrap();
        let json = serde_json::to_string(&r).unwrap();
        let back: InferenceRef = serde_json::from_str(&json).unwrap();
        assert_eq!(back, r);
    }

    #[test]
    fn ability_ref_rejects_empty_fields() {
        assert!(AbilityRef::new("", "1.0.0", "approved").is_err());
    }

    #[test]
    fn provenance_rejects_empty_source() {
        assert!(ReceiptProvenance::new("", "0.1.0").is_err());
    }

    #[test]
    fn provenance_with_inference_and_ability() {
        let p = ReceiptProvenance::new("adaptive-engine", "0.1.0")
            .unwrap()
            .with_inference(InferenceRef::new("anthropic", "claude", "v1", 0.0, "req-1").unwrap())
            .with_ability(AbilityRef::new("payment-validator", "1.0.0", "approved").unwrap());
        assert!(p.inference.is_some());
        assert!(p.ability.is_some());
    }

    #[test]
    fn provenance_round_trips() {
        let p = ReceiptProvenance::new("adaptive-engine", "0.1.0").unwrap();
        let json = serde_json::to_string(&p).unwrap();
        let back: ReceiptProvenance = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn timestamps_rejects_empty_created_at() {
        assert!(ReceiptTimestamps::new("").is_err());
    }

    #[test]
    fn timestamps_round_trips() {
        let t = ReceiptTimestamps::new("2026-06-25T12:00:00Z")
            .unwrap()
            .with_updated_at("2026-06-25T13:00:00Z")
            .with_expires_at("2026-07-25T00:00:00Z");
        let json = serde_json::to_string(&t).unwrap();
        let back: ReceiptTimestamps = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn receipt_version_with_api() {
        let v = Version::new(1, 0, 0);
        let rv = ReceiptVersion::new(v.clone()).with_api("v1");
        assert_eq!(rv.schema, v);
        assert_eq!(rv.api.unwrap().as_str(), "v1");
    }

    #[test]
    fn receipt_version_round_trips() {
        let rv = ReceiptVersion::new(Version::new(1, 0, 0));
        let json = serde_json::to_string(&rv).unwrap();
        let back: ReceiptVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(back, rv);
    }
}
