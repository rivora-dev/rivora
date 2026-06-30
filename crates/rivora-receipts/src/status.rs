//! The lifecycle status of a reliability receipt.

use serde::{Deserialize, Serialize};

/// The current lifecycle status of a receipt.
///
/// A receipt's status changes over time as it is validated, superseded, or
/// archived. The `Valid` status implies the receipt has passed all
/// validation invariants in [`crate::validation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptStatus {
    /// A receipt that has not yet been validated.
    Draft,
    /// A receipt that has passed all validation invariants.
    Valid,
    /// A receipt that failed validation. Must not be surfaced to engineers.
    Invalid,
    /// A receipt that has been replaced by a newer one.
    Superseded,
    /// A receipt that has been retired from active use.
    Archived,
}

impl ReceiptStatus {
    /// Returns `true` if the status is [`Valid`](ReceiptStatus::Valid).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }

    /// Returns `true` if the status is [`Draft`](ReceiptStatus::Draft).
    #[must_use]
    pub fn is_draft(&self) -> bool {
        matches!(self, Self::Draft)
    }

    /// Returns `true` if the status is a terminal state
    /// ([`Invalid`](ReceiptStatus::Invalid),
    /// [`Superseded`](ReceiptStatus::Superseded), or
    /// [`Archived`](ReceiptStatus::Archived)).
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Invalid | Self::Superseded | Self::Archived)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_valid_checks() {
        assert!(ReceiptStatus::Valid.is_valid());
        assert!(!ReceiptStatus::Draft.is_valid());
        assert!(!ReceiptStatus::Invalid.is_valid());
    }

    #[test]
    fn is_draft_checks() {
        assert!(ReceiptStatus::Draft.is_draft());
        assert!(!ReceiptStatus::Valid.is_draft());
    }

    #[test]
    fn is_terminal_checks() {
        assert!(ReceiptStatus::Invalid.is_terminal());
        assert!(ReceiptStatus::Superseded.is_terminal());
        assert!(ReceiptStatus::Archived.is_terminal());
        assert!(!ReceiptStatus::Valid.is_terminal());
        assert!(!ReceiptStatus::Draft.is_terminal());
    }

    #[test]
    fn serializes_as_snake_case_tag() {
        let json = serde_json::to_string(&ReceiptStatus::Valid).unwrap();
        assert_eq!(json, "\"valid\"");
    }

    #[test]
    fn round_trips_through_serde() {
        let status = ReceiptStatus::Superseded;
        let json = serde_json::to_string(&status).unwrap();
        let back: ReceiptStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, status);
    }
}
