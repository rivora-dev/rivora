//! The subject and summary of a reliability receipt.

use serde::{Deserialize, Serialize};

use rivora_types::NonEmptyString;

/// What a reliability receipt is about.
///
/// A subject is the entity (service, deployment, incident, etc.) that the
/// receipt is describing. It is distinct from the receipt `id` (which
/// identifies the receipt itself).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReceiptSubject {
    /// The kind of subject (e.g. `"service"`, `"deployment"`, `"incident"`).
    pub kind: NonEmptyString,
    /// A reference to the subject (e.g. a service ID, deployment ID).
    pub reference: NonEmptyString,
    /// A human-readable name for the subject (e.g. `"payment-service"`).
    pub display_name: NonEmptyString,
}

impl ReceiptSubject {
    /// Creates a new `ReceiptSubject` from validated string values.
    ///
    /// # Errors
    ///
    /// Returns an error if any field is empty or exceeds length limits.
    pub fn new(
        kind: impl Into<String>,
        reference: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Result<Self, rivora_errors::RivoraError> {
        Ok(Self {
            kind: NonEmptyString::new(kind.into())?,
            reference: NonEmptyString::new(reference.into())?,
            display_name: NonEmptyString::new(display_name.into())?,
        })
    }
}

/// A human-readable summary of a receipt's conclusion.
///
/// The title is a short headline; the description is a one-or-two-sentence
/// statement of the conclusion.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReceiptSummary {
    /// A short, human-readable headline.
    pub title: NonEmptyString,
    /// A one or two sentence statement of the conclusion.
    pub description: NonEmptyString,
}

impl ReceiptSummary {
    /// Creates a new `ReceiptSummary` from validated string values.
    ///
    /// # Errors
    ///
    /// Returns an error if either field is empty or exceeds length limits.
    pub fn new(
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Result<Self, rivora_errors::RivoraError> {
        Ok(Self {
            title: NonEmptyString::new(title.into())?,
            description: NonEmptyString::new(description.into())?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subject_rejects_empty_fields() {
        assert!(ReceiptSubject::new("", "ref", "name").is_err());
        assert!(ReceiptSubject::new("kind", "", "name").is_err());
        assert!(ReceiptSubject::new("kind", "ref", "").is_err());
    }

    #[test]
    fn subject_accepts_valid_fields() {
        let s = ReceiptSubject::new("service", "svc-1", "api-gateway").unwrap();
        assert_eq!(s.kind.as_str(), "service");
        assert_eq!(s.reference.as_str(), "svc-1");
        assert_eq!(s.display_name.as_str(), "api-gateway");
    }

    #[test]
    fn subject_round_trips_through_serde() {
        let s = ReceiptSubject::new("deployment", "dep-1", "deploy-1").unwrap();
        let json = serde_json::to_string(&s).unwrap();
        let back: ReceiptSubject = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn summary_rejects_empty_fields() {
        assert!(ReceiptSummary::new("", "description").is_err());
        assert!(ReceiptSummary::new("title", "").is_err());
    }

    #[test]
    fn summary_accepts_valid_fields() {
        let s = ReceiptSummary::new("Payment latency spike", "Latency increased 3x after deploy")
            .unwrap();
        assert_eq!(s.title.as_str(), "Payment latency spike");
        assert_eq!(s.description.as_str(), "Latency increased 3x after deploy");
    }

    #[test]
    fn summary_round_trips_through_serde() {
        let s = ReceiptSummary::new("title", "description").unwrap();
        let json = serde_json::to_string(&s).unwrap();
        let back: ReceiptSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }
}
