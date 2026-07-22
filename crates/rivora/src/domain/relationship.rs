//! Investigation Relationships — the Investigation Graph (RFC-015).
//!
//! Relationships connect exactly two Investigations without merging them.
//! Derived relationships have deterministic identifiers so refresh is
//! idempotent; explicit links are human-created and durable.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Confidence, DerivationMetadata, InvestigationId, ObjectId, Provenance};

/// Kind of relationship between two Investigations (RFC-015 vocabulary).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipKind {
    /// Both Investigations observed the same repository.
    SharedRepository,
    /// Both Investigations observed the same commit.
    SharedCommit,
    /// Both Investigations observed the same pull request.
    SharedPullRequest,
    /// Both Investigations observed the same changed file path.
    SharedFilePath,
    /// Both Investigations were observed through the same connector source.
    SharedConnectorSource,
    /// Observation kinds and text overlap beyond a deterministic threshold.
    SimilarObservations,
    /// Evaluations share an assessment type and severity.
    SharedEvaluationCategory,
    /// Verification Receipts share an outcome.
    RelatedVerificationOutcome,
    /// The same normalized failure signature appears in both Investigations.
    RepeatedFailureSignature,
    /// Recommendations share a deterministic type signature.
    RelatedRecommendation,
    /// Learning Outcomes share a disposition.
    RelatedLearningOutcome,
    /// A human created the relationship directly.
    ExplicitLink,
}

impl RelationshipKind {
    /// Display name (snake_case, matching serde representation).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SharedRepository => "shared_repository",
            Self::SharedCommit => "shared_commit",
            Self::SharedPullRequest => "shared_pull_request",
            Self::SharedFilePath => "shared_file_path",
            Self::SharedConnectorSource => "shared_connector_source",
            Self::SimilarObservations => "similar_observations",
            Self::SharedEvaluationCategory => "shared_evaluation_category",
            Self::RelatedVerificationOutcome => "related_verification_outcome",
            Self::RepeatedFailureSignature => "repeated_failure_signature",
            Self::RelatedRecommendation => "related_recommendation",
            Self::RelatedLearningOutcome => "related_learning_outcome",
            Self::ExplicitLink => "explicit_link",
        }
    }

    /// Whether this kind is produced by deterministic derivation.
    ///
    /// Every kind except `ExplicitLink` is derived; explicit links are
    /// never created or removed by derivation refresh.
    pub fn is_derived(self) -> bool {
        !matches!(self, Self::ExplicitLink)
    }
}

/// One piece of evidence supporting a relationship.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelationshipEvidence {
    /// Human-readable description of the shared artifact.
    pub description: String,
    /// Engineering Objects on both sides that justify the relationship.
    pub object_ids: Vec<ObjectId>,
}

impl RelationshipEvidence {
    /// Construct an evidence item.
    pub fn new(description: impl Into<String>, object_ids: Vec<ObjectId>) -> Self {
        Self {
            description: description.into(),
            object_ids,
        }
    }
}

/// Human confirmation state of a relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationState {
    /// Not yet reviewed by a human.
    Unconfirmed,
    /// A human confirmed the relationship is meaningful.
    Confirmed,
    /// A human dismissed the relationship as not meaningful.
    Dismissed,
}

impl ConfirmationState {
    /// Display name (snake_case, matching serde representation).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unconfirmed => "unconfirmed",
            Self::Confirmed => "confirmed",
            Self::Dismissed => "dismissed",
        }
    }
}

/// Human confirmation record for a relationship.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelationshipConfirmation {
    /// Current confirmation state.
    pub state: ConfirmationState,
    /// Actor that last changed the state, if any.
    pub actor: Option<String>,
    /// When the state was last changed, if ever.
    pub at: Option<DateTime<Utc>>,
}

impl RelationshipConfirmation {
    /// Unreviewed relationship (default for derived relationships).
    pub fn unconfirmed() -> Self {
        Self {
            state: ConfirmationState::Unconfirmed,
            actor: None,
            at: None,
        }
    }

    /// Mark the relationship confirmed by `actor` now.
    pub fn confirmed(actor: impl Into<String>) -> Self {
        Self {
            state: ConfirmationState::Confirmed,
            actor: Some(actor.into()),
            at: Some(Utc::now()),
        }
    }

    /// Mark the relationship dismissed by `actor` now.
    pub fn dismissed(actor: impl Into<String>) -> Self {
        Self {
            state: ConfirmationState::Dismissed,
            actor: Some(actor.into()),
            at: Some(Utc::now()),
        }
    }
}

impl Default for RelationshipConfirmation {
    fn default() -> Self {
        Self::unconfirmed()
    }
}

/// A durable, explainable relationship between two Investigations.
///
/// Relationships never merge, move, or rewrite Investigation history;
/// they only record why two Investigations are related.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvestigationRelationship {
    /// Stable relationship identifier.
    pub id: ObjectId,
    /// Source Investigation (canonically lower id for derived relationships).
    pub source_investigation_id: InvestigationId,
    /// Target Investigation (canonically higher id for derived relationships).
    pub target_investigation_id: InvestigationId,
    /// Relationship kind.
    pub kind: RelationshipKind,
    /// Confidence or strength of the relationship.
    pub confidence: Confidence,
    /// Supporting evidence (descriptions plus Engineering Objects).
    pub evidence: Vec<RelationshipEvidence>,
    /// Derivation method and explanation (versioned, like Knowledge).
    pub derivation: DerivationMetadata,
    /// Human confirmation state.
    pub confirmation: RelationshipConfirmation,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Provenance.
    pub provenance: Provenance,
}

impl InvestigationRelationship {
    /// Construct a derived relationship with a deterministic identifier.
    ///
    /// Endpoints are stored in canonical order (lexicographic by UUID
    /// string; the lower id becomes the source). The identifier is a
    /// UUIDv5 over kind, canonical endpoints, and `identity_key` — the
    /// derivation-supplied stable artifact key — so re-deriving over
    /// unchanged data reproduces the same relationship id.
    #[allow(clippy::too_many_arguments)]
    pub fn derived(
        kind: RelationshipKind,
        investigation_a: InvestigationId,
        investigation_b: InvestigationId,
        confidence: Confidence,
        evidence: Vec<RelationshipEvidence>,
        derivation: DerivationMetadata,
        provenance: Provenance,
        identity_key: &str,
    ) -> Self {
        let (source, target) = if investigation_a.to_string() <= investigation_b.to_string() {
            (investigation_a, investigation_b)
        } else {
            (investigation_b, investigation_a)
        };
        let key = format!(
            "rivora-relationship|{}|{}|{}|{}",
            kind.as_str(),
            source,
            target,
            identity_key
        );
        Self {
            id: ObjectId::from_uuid(Uuid::new_v5(&Uuid::NAMESPACE_URL, key.as_bytes())),
            source_investigation_id: source,
            target_investigation_id: target,
            kind,
            confidence,
            evidence,
            derivation,
            confirmation: RelationshipConfirmation::unconfirmed(),
            created_at: Utc::now(),
            provenance,
        }
    }

    /// Construct an explicit, human-created link with a random identifier.
    ///
    /// Explicit links preserve the direction chosen by the user, start
    /// confirmed by the linking actor, and are never created or removed
    /// by derivation refresh.
    pub fn explicit(
        source: InvestigationId,
        target: InvestigationId,
        reason: Option<String>,
        actor: impl Into<String>,
        provenance: Provenance,
    ) -> Self {
        let actor = actor.into();
        let explanation = match &reason {
            Some(reason) => format!("explicit user link: {reason}"),
            None => "explicit user link".to_string(),
        };
        let evidence_description = match &reason {
            Some(reason) => {
                format!("User `{actor}` explicitly linked these investigations: {reason}")
            }
            None => format!("User `{actor}` explicitly linked these investigations"),
        };
        Self {
            id: ObjectId::new(),
            source_investigation_id: source,
            target_investigation_id: target,
            kind: RelationshipKind::ExplicitLink,
            confidence: Confidence::certain(),
            evidence: vec![RelationshipEvidence::new(evidence_description, Vec::new())],
            derivation: DerivationMetadata {
                method: "explicit_link_v1".into(),
                explanation,
            },
            confirmation: RelationshipConfirmation::confirmed(actor),
            created_at: Utc::now(),
            provenance,
        }
    }

    /// Whether this relationship involves the given Investigation on either side.
    pub fn touches(&self, id: InvestigationId) -> bool {
        self.source_investigation_id == id || self.target_investigation_id == id
    }

    /// The Investigation on the opposite side of `id`, when `id` is an endpoint.
    pub fn other_end(&self, id: InvestigationId) -> Option<InvestigationId> {
        if self.source_investigation_id == id {
            Some(self.target_investigation_id)
        } else if self.target_investigation_id == id {
            Some(self.source_investigation_id)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> (InvestigationId, InvestigationId) {
        let a = InvestigationId::from_uuid(
            Uuid::parse_str("00000000-0000-0000-0000-0000000000aa").unwrap(),
        );
        let b = InvestigationId::from_uuid(
            Uuid::parse_str("00000000-0000-0000-0000-0000000000bb").unwrap(),
        );
        (a, b)
    }

    fn derivation() -> DerivationMetadata {
        DerivationMetadata {
            method: "shared_repository_v1".into(),
            explanation: "Compares normalized repository names.".into(),
        }
    }

    #[test]
    fn derived_orders_endpoints_canonically() {
        let (lo, hi) = ids();
        let rel = InvestigationRelationship::derived(
            RelationshipKind::SharedRepository,
            hi,
            lo,
            Confidence::new(0.9),
            vec![RelationshipEvidence::new(
                "Both observed `acme/app`",
                vec![],
            )],
            derivation(),
            Provenance::now("tester", "runtime"),
            "shared_repository|acme/app",
        );
        assert_eq!(rel.source_investigation_id, lo);
        assert_eq!(rel.target_investigation_id, hi);
        assert_eq!(rel.confirmation.state, ConfirmationState::Unconfirmed);
        assert!(rel.kind.is_derived());
    }

    #[test]
    fn derived_id_is_deterministic_and_sensitive_to_key_parts() {
        let (lo, hi) = ids();
        let make = |kind, key: &str| {
            InvestigationRelationship::derived(
                kind,
                lo,
                hi,
                Confidence::new(0.9),
                vec![],
                derivation(),
                Provenance::now("tester", "runtime"),
                key,
            )
            .id
        };
        let base = make(
            RelationshipKind::SharedRepository,
            "shared_repository|acme/app",
        );
        // Argument order does not matter.
        let swapped = InvestigationRelationship::derived(
            RelationshipKind::SharedRepository,
            hi,
            lo,
            Confidence::new(0.9),
            vec![],
            derivation(),
            Provenance::now("tester", "runtime"),
            "shared_repository|acme/app",
        )
        .id;
        assert_eq!(base, swapped);
        // Kind and identity key both feed the id.
        assert_ne!(
            base,
            make(RelationshipKind::SharedCommit, "shared_repository|acme/app")
        );
        assert_ne!(
            base,
            make(
                RelationshipKind::SharedRepository,
                "shared_repository|acme/other"
            )
        );
    }

    #[test]
    fn explicit_link_is_confirmed_and_directional() {
        let (lo, hi) = ids();
        let rel = InvestigationRelationship::explicit(
            hi,
            lo,
            Some("same incident".into()),
            "oncall",
            Provenance::now("oncall", "runtime"),
        );
        assert_eq!(rel.kind, RelationshipKind::ExplicitLink);
        assert!(!rel.kind.is_derived());
        assert_eq!(rel.source_investigation_id, hi);
        assert_eq!(rel.target_investigation_id, lo);
        assert_eq!(rel.confidence, Confidence::certain());
        assert_eq!(rel.derivation.method, "explicit_link_v1");
        assert!(rel.derivation.explanation.contains("same incident"));
        assert_eq!(rel.confirmation.state, ConfirmationState::Confirmed);
        assert_eq!(rel.confirmation.actor.as_deref(), Some("oncall"));
        assert!(rel.confirmation.at.is_some());
        assert_eq!(rel.evidence.len(), 1);
        assert!(rel.evidence[0].description.contains("same incident"));
    }

    #[test]
    fn explicit_link_without_reason_uses_default_explanation() {
        let (lo, hi) = ids();
        let rel = InvestigationRelationship::explicit(
            lo,
            hi,
            None,
            "oncall",
            Provenance::now("oncall", "runtime"),
        );
        assert_eq!(rel.derivation.explanation, "explicit user link");
        assert!(rel.evidence[0].description.contains("explicitly linked"));
    }

    #[test]
    fn kind_and_confirmation_serde_are_snake_case() {
        assert_eq!(
            serde_json::to_value(RelationshipKind::RepeatedFailureSignature).unwrap(),
            serde_json::json!("repeated_failure_signature")
        );
        assert_eq!(
            serde_json::to_value(RelationshipKind::ExplicitLink).unwrap(),
            serde_json::json!("explicit_link")
        );
        assert_eq!(
            serde_json::to_value(ConfirmationState::Dismissed).unwrap(),
            serde_json::json!("dismissed")
        );
    }

    #[test]
    fn relationship_serde_round_trip() {
        let (lo, hi) = ids();
        let derived = InvestigationRelationship::derived(
            RelationshipKind::SharedRepository,
            lo,
            hi,
            Confidence::new(0.9),
            vec![RelationshipEvidence::new(
                "Both investigations observed repository `acme/app`",
                vec![ObjectId::new(), ObjectId::new()],
            )],
            derivation(),
            Provenance::now("tester", "runtime").with_capability("refresh_relationships"),
            "shared_repository|acme/app",
        );
        let json = serde_json::to_string_pretty(&derived).unwrap();
        let back: InvestigationRelationship = serde_json::from_str(&json).unwrap();
        assert_eq!(derived, back);

        let explicit = InvestigationRelationship::explicit(
            lo,
            hi,
            Some("related".into()),
            "oncall",
            Provenance::now("oncall", "runtime"),
        );
        let json = serde_json::to_string_pretty(&explicit).unwrap();
        let back: InvestigationRelationship = serde_json::from_str(&json).unwrap();
        assert_eq!(explicit, back);
    }

    #[test]
    fn endpoint_helpers() {
        let (lo, hi) = ids();
        let rel = InvestigationRelationship::explicit(
            lo,
            hi,
            None,
            "oncall",
            Provenance::now("oncall", "runtime"),
        );
        assert!(rel.touches(lo));
        assert!(rel.touches(hi));
        assert!(!rel.touches(InvestigationId::new()));
        assert_eq!(rel.other_end(lo), Some(hi));
        assert_eq!(rel.other_end(hi), Some(lo));
        assert_eq!(rel.other_end(InvestigationId::new()), None);
    }
}
