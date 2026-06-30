//! Deterministic sample memory records and indices for testing.

use std::collections::BTreeMap;

use rivora_types::{NonEmptyString, Version};

use crate::confidence::MemoryConfidence;
use crate::feedback::{FeedbackKind, FeedbackSource, FeedbackTargetType, HumanFeedback};
use crate::index::MemoryIndex;
use crate::kind::MemoryKind;
use crate::metadata::{MemoryMetadata, MemoryTimestamps, MemoryVersion};
use crate::provenance::MemoryProvenance;
use crate::record::MemoryRecord;
use crate::retention::{MemoryDecay, MemoryRetention, MemoryRetentionPolicy};
use crate::scope::MemoryScope;
use crate::source::MemorySource;
use crate::status::MemoryStatus;

pub fn provenance() -> MemoryProvenance {
    MemoryProvenance::new(
        "fixture-source",
        "0.1.0",
        "2026-06-25T12:00:00Z",
        "2026-06-25T12:00:00Z",
    )
    .unwrap()
}

pub fn confidence() -> MemoryConfidence {
    MemoryConfidence::new(0.8, "Fixture confidence", "2026-06-25T12:00:00Z").unwrap()
}

pub fn retention() -> MemoryRetention {
    MemoryRetention::new(MemoryRetentionPolicy::Permanent, "fixture retention").unwrap()
}

pub fn timestamps() -> MemoryTimestamps {
    MemoryTimestamps::new("2026-06-25T12:00:00Z").unwrap()
}

pub fn version() -> MemoryVersion {
    MemoryVersion::new(Version::new(1, 0, 0), 1)
}

pub fn organization_fact() -> MemoryRecord {
    let mut record = MemoryRecord::new(
        "mem-fixture-org-fact",
        MemoryKind::Fact,
        MemoryScope::Organization,
        "Organization uses AWS",
        "The organization uses AWS as its primary cloud provider.",
        MemorySource::Human,
        provenance(),
        confidence(),
        retention(),
        timestamps(),
        version(),
    )
    .unwrap()
    .with_subject_refs(vec![NonEmptyString::new("org-1").unwrap()]);
    record.activate();
    record
}

pub fn service_relationship_memory() -> MemoryRecord {
    let mut record = MemoryRecord::new(
        "mem-fixture-service-relationship",
        MemoryKind::ServiceRelationship,
        MemoryScope::Service,
        "API gateway depends on payments",
        "The api-gateway service depends on the payments service for checkout.",
        MemorySource::Graph,
        provenance(),
        confidence(),
        retention(),
        timestamps(),
        version(),
    )
    .unwrap()
    .with_subject_refs(vec![
        NonEmptyString::new("svc-api-gateway").unwrap(),
        NonEmptyString::new("svc-payment").unwrap(),
    ])
    .with_graph_node_ids(vec![
        "svc-api-gateway".to_string(),
        "svc-payment".to_string(),
    ])
    .with_graph_edge_ids(vec!["edge-depends-on".to_string()]);
    record.activate();
    record
}

pub fn incident_learning_memory() -> MemoryRecord {
    let mut record = MemoryRecord::new(
        "mem-fixture-incident-learning",
        MemoryKind::IncidentLearning,
        MemoryScope::Incident,
        "Latency incident root cause",
        "The latency incident was caused by a connection pool exhaustion in the payments service.",
        MemorySource::Receipt,
        provenance(),
        confidence(),
        retention(),
        timestamps(),
        version(),
    )
    .unwrap()
    .with_subject_refs(vec![
        NonEmptyString::new("inc-latency-001").unwrap(),
        NonEmptyString::new("svc-payment").unwrap(),
    ])
    .with_graph_node_ids(vec!["inc-latency-001".to_string()]);
    record.activate();
    record
}

pub fn deployment_learning_memory() -> MemoryRecord {
    let mut record = MemoryRecord::new(
        "mem-fixture-deployment-learning",
        MemoryKind::DeploymentLearning,
        MemoryScope::Deployment,
        "Deployment v2.1.0 reduced latency",
        "Deployment v2.1.0 of the payments service reduced p99 latency by 30 percent.",
        MemorySource::Receipt,
        provenance(),
        confidence(),
        retention(),
        timestamps(),
        version(),
    )
    .unwrap()
    .with_subject_refs(vec![
        NonEmptyString::new("dep-v2.1.0").unwrap(),
        NonEmptyString::new("svc-payment").unwrap(),
    ])
    .with_graph_node_ids(vec!["dep-v2.1.0".to_string()]);
    record.activate();
    record
}

pub fn receipt_learning_memory() -> MemoryRecord {
    let mut record = MemoryRecord::new(
        "mem-fixture-receipt-learning",
        MemoryKind::ReceiptLearning,
        MemoryScope::Service,
        "Receipt explains connection pool behavior",
        "The reliability receipt confirmed that the payments service connection pool stabilizes after warm-up.",
        MemorySource::Receipt,
        provenance(),
        confidence(),
        retention(),
        timestamps(),
        version(),
    )
    .unwrap()
    .with_subject_refs(vec![NonEmptyString::new("svc-payment").unwrap()])
    .with_receipt_ids(vec!["receipt-fixture-001".to_string()]);
    record.activate();
    record
}

pub fn ability_learning_memory() -> MemoryRecord {
    let mut record = MemoryRecord::new(
        "mem-fixture-ability-learning",
        MemoryKind::AbilityLearning,
        MemoryScope::Ability,
        "Deployment validator ability is reliable",
        "The deployment-validator ability has a 95 percent success rate across observed runs.",
        MemorySource::Ability,
        provenance(),
        confidence(),
        retention(),
        timestamps(),
        version(),
    )
    .unwrap()
    .with_subject_refs(vec![
        NonEmptyString::new("ability-deploy-validator").unwrap()
    ])
    .with_receipt_ids(vec!["receipt-fixture-002".to_string()]);
    record.activate();
    record
}

pub fn expired_memory() -> MemoryRecord {
    let mut record = MemoryRecord::new(
        "mem-fixture-expired",
        MemoryKind::OperationalNote,
        MemoryScope::Service,
        "Expired operational note",
        "This operational note about a temporary override has expired.",
        MemorySource::System,
        provenance(),
        confidence(),
        MemoryRetention::new(MemoryRetentionPolicy::TimeBound, "time-bound retention")
            .unwrap()
            .with_expires_at("2026-06-20T12:00:00Z")
            .with_decay(MemoryDecay::Linear),
        timestamps(),
        version(),
    )
    .unwrap()
    .with_subject_refs(vec![NonEmptyString::new("svc-payment").unwrap()]);
    record.expire();
    record
}

pub fn superseded_memory() -> MemoryRecord {
    let mut record = MemoryRecord::new(
        "mem-fixture-superseded",
        MemoryKind::Fact,
        MemoryScope::Organization,
        "Superseded organization fact",
        "This organization fact has been superseded by a newer observation.",
        MemorySource::Human,
        provenance(),
        confidence(),
        retention(),
        timestamps(),
        version(),
    )
    .unwrap()
    .with_subject_refs(vec![NonEmptyString::new("org-1").unwrap()]);
    record.supersede("mem-fixture-org-fact");
    record
}

pub fn invalid_memory() -> MemoryRecord {
    MemoryRecord {
        id: NonEmptyString::new("mem-fixture-invalid").unwrap(),
        kind: MemoryKind::Fact,
        scope: MemoryScope::Organization,
        status: MemoryStatus::Invalid,
        title: NonEmptyString::new("Invalid memory").unwrap(),
        body: NonEmptyString::new("This memory fails validation").unwrap(),
        subject_refs: Vec::new(),
        graph_node_ids: Vec::new(),
        graph_edge_ids: Vec::new(),
        receipt_ids: vec![String::new()],
        source: MemorySource::Human,
        provenance: provenance(),
        confidence: confidence(),
        retention: retention(),
        timestamps: timestamps(),
        version: version(),
        labels: BTreeMap::new(),
        metadata: MemoryMetadata::default(),
        feedback_ids: Vec::new(),
    }
}

pub fn candidate_memory() -> MemoryRecord {
    MemoryRecord::new(
        "mem-fixture-candidate",
        MemoryKind::Fact,
        MemoryScope::Organization,
        "Candidate organization fact",
        "This organization fact is proposed and awaiting human review.",
        MemorySource::Human,
        provenance(),
        confidence(),
        retention(),
        timestamps(),
        version(),
    )
    .unwrap()
    .with_subject_refs(vec![NonEmptyString::new("org-1").unwrap()])
    .with_status(MemoryStatus::Candidate)
}

pub fn rejected_memory() -> MemoryRecord {
    let mut record = MemoryRecord::new(
        "mem-fixture-rejected",
        MemoryKind::Fact,
        MemoryScope::Organization,
        "Rejected organization fact",
        "This organization fact was reviewed and rejected by an engineer.",
        MemorySource::Human,
        provenance(),
        confidence(),
        retention(),
        timestamps(),
        version(),
    )
    .unwrap()
    .with_subject_refs(vec![NonEmptyString::new("org-1").unwrap()])
    .with_status(MemoryStatus::Candidate);
    record.reject("insufficient evidence");
    record
}

pub fn corrected_memory() -> MemoryRecord {
    let mut record = MemoryRecord::new(
        "mem-fixture-corrected",
        MemoryKind::Fact,
        MemoryScope::Organization,
        "Corrected organization fact",
        "This organization fact was corrected by an engineer after review.",
        MemorySource::Human,
        provenance(),
        confidence(),
        retention(),
        timestamps(),
        version(),
    )
    .unwrap()
    .with_subject_refs(vec![NonEmptyString::new("org-1").unwrap()]);
    record.activate();
    record.correct("corrected body content");
    record
}

pub fn approved_feedback() -> HumanFeedback {
    HumanFeedback::new(
        "fb-fixture-approved",
        "mem-fixture-org-fact",
        FeedbackTargetType::Memory,
        "engineer-1",
        FeedbackSource::Human,
        FeedbackKind::Approved,
        "2026-06-25T12:00:00Z",
    )
    .unwrap()
}

pub fn rejected_feedback() -> HumanFeedback {
    HumanFeedback::new(
        "fb-fixture-rejected",
        "mem-fixture-candidate",
        FeedbackTargetType::Memory,
        "engineer-1",
        FeedbackSource::Slack,
        FeedbackKind::Rejected,
        "2026-06-25T12:00:00Z",
    )
    .unwrap()
    .with_note("not enough evidence")
}

pub fn corrected_feedback() -> HumanFeedback {
    HumanFeedback::new(
        "fb-fixture-corrected",
        "mem-fixture-org-fact",
        FeedbackTargetType::Memory,
        "engineer-1",
        FeedbackSource::Cli,
        FeedbackKind::Corrected,
        "2026-06-25T12:00:00Z",
    )
    .unwrap()
    .with_correction_text("corrected body content")
}

pub fn useful_feedback() -> HumanFeedback {
    HumanFeedback::new(
        "fb-fixture-useful",
        "mem-fixture-org-fact",
        FeedbackTargetType::Memory,
        "engineer-1",
        FeedbackSource::Api,
        FeedbackKind::Useful,
        "2026-06-25T12:00:00Z",
    )
    .unwrap()
}

pub fn sample_index() -> MemoryIndex {
    let mut index = MemoryIndex::with_metadata(MemoryMetadata::new().with_organization_id("org-1"));
    index.add_record(organization_fact()).unwrap();
    index.add_record(service_relationship_memory()).unwrap();
    index.add_record(incident_learning_memory()).unwrap();
    index.add_record(receipt_learning_memory()).unwrap();
    index.add_record(ability_learning_memory()).unwrap();
    index.add_record(expired_memory()).unwrap();
    index.add_record(superseded_memory()).unwrap();
    index.add_record(candidate_memory()).unwrap();
    index.add_record(rejected_memory()).unwrap();
    index.add_record(corrected_memory()).unwrap();
    index
}

pub fn empty_index() -> MemoryIndex {
    MemoryIndex::with_metadata(MemoryMetadata::new().with_organization_id("org-1"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feedback::FeedbackKind;
    use crate::validation::{validate_feedback, validate_record};

    #[test]
    fn organization_fact_is_active_fact() {
        let record = organization_fact();
        assert_eq!(record.kind, MemoryKind::Fact);
        assert_eq!(record.scope, MemoryScope::Organization);
        assert_eq!(record.status, MemoryStatus::Active);
        assert!(record.is_active());
    }

    #[test]
    fn service_relationship_is_service_scope() {
        let record = service_relationship_memory();
        assert_eq!(record.kind, MemoryKind::ServiceRelationship);
        assert_eq!(record.scope, MemoryScope::Service);
    }

    #[test]
    fn incident_learning_is_incident_scope() {
        let record = incident_learning_memory();
        assert_eq!(record.kind, MemoryKind::IncidentLearning);
        assert_eq!(record.scope, MemoryScope::Incident);
    }

    #[test]
    fn deployment_learning_is_deployment_scope() {
        let record = deployment_learning_memory();
        assert_eq!(record.kind, MemoryKind::DeploymentLearning);
        assert_eq!(record.scope, MemoryScope::Deployment);
    }

    #[test]
    fn receipt_learning_has_receipt_ids() {
        let record = receipt_learning_memory();
        assert_eq!(record.kind, MemoryKind::ReceiptLearning);
        assert_eq!(record.scope, MemoryScope::Service);
        assert!(!record.receipt_ids.is_empty());
    }

    #[test]
    fn ability_learning_is_ability_scope() {
        let record = ability_learning_memory();
        assert_eq!(record.kind, MemoryKind::AbilityLearning);
        assert_eq!(record.scope, MemoryScope::Ability);
    }

    #[test]
    fn expired_memory_is_expired_with_expires_at() {
        let record = expired_memory();
        assert_eq!(record.status, MemoryStatus::Expired);
        assert!(record.retention.expires_at.is_some());
    }

    #[test]
    fn superseded_memory_has_superseded_by_label() {
        let record = superseded_memory();
        assert_eq!(record.status, MemoryStatus::Superseded);
        let key = NonEmptyString::new("superseded_by").unwrap();
        assert!(record.labels.contains_key(&key));
    }

    #[test]
    fn invalid_memory_fails_validation() {
        let record = invalid_memory();
        assert!(validate_record(&record).is_err());
    }

    #[test]
    fn candidate_memory_is_candidate() {
        let record = candidate_memory();
        assert_eq!(record.status, MemoryStatus::Candidate);
        assert!(record.status.is_candidate());
        assert!(validate_record(&record).is_ok());
    }

    #[test]
    fn rejected_memory_is_rejected_with_reason() {
        let record = rejected_memory();
        assert_eq!(record.status, MemoryStatus::Rejected);
        assert!(record.status.is_rejected());
        assert!(record.status.is_terminal());
        let key = NonEmptyString::new("rejection_reason").unwrap();
        assert_eq!(
            record.labels.get(&key).unwrap().as_str(),
            "insufficient evidence"
        );
        assert!(validate_record(&record).is_ok());
    }

    #[test]
    fn corrected_memory_is_corrected_with_text() {
        let record = corrected_memory();
        assert_eq!(record.status, MemoryStatus::Corrected);
        assert!(record.status.is_corrected());
        let key = NonEmptyString::new("correction_text").unwrap();
        assert_eq!(
            record.labels.get(&key).unwrap().as_str(),
            "corrected body content"
        );
        assert!(validate_record(&record).is_ok());
    }

    #[test]
    fn approved_feedback_is_approved() {
        let feedback = approved_feedback();
        assert_eq!(feedback.kind, FeedbackKind::Approved);
        assert!(feedback.kind.is_actionable());
        assert!(validate_feedback(&feedback).is_ok());
    }

    #[test]
    fn rejected_feedback_is_rejected() {
        let feedback = rejected_feedback();
        assert_eq!(feedback.kind, FeedbackKind::Rejected);
        assert!(feedback.kind.is_actionable());
        assert!(validate_feedback(&feedback).is_ok());
    }

    #[test]
    fn corrected_feedback_is_corrected_with_text() {
        let feedback = corrected_feedback();
        assert_eq!(feedback.kind, FeedbackKind::Corrected);
        assert!(feedback.kind.is_actionable());
        assert!(feedback.correction_text.is_some());
        assert!(validate_feedback(&feedback).is_ok());
    }

    #[test]
    fn useful_feedback_is_useful() {
        let feedback = useful_feedback();
        assert_eq!(feedback.kind, FeedbackKind::Useful);
        assert!(!feedback.kind.is_actionable());
        assert!(validate_feedback(&feedback).is_ok());
    }

    #[test]
    fn sample_index_has_records() {
        let index = sample_index();
        assert!(index.record_count() > 0);
    }

    #[test]
    fn empty_index_has_no_records() {
        let index = empty_index();
        assert_eq!(index.record_count(), 0);
    }

    #[test]
    fn fixture_ids_are_deterministic() {
        assert_eq!(organization_fact().id.as_str(), "mem-fixture-org-fact");
        assert_eq!(
            service_relationship_memory().id.as_str(),
            "mem-fixture-service-relationship"
        );
        assert_eq!(
            incident_learning_memory().id.as_str(),
            "mem-fixture-incident-learning"
        );
        assert_eq!(
            deployment_learning_memory().id.as_str(),
            "mem-fixture-deployment-learning"
        );
        assert_eq!(
            receipt_learning_memory().id.as_str(),
            "mem-fixture-receipt-learning"
        );
        assert_eq!(
            ability_learning_memory().id.as_str(),
            "mem-fixture-ability-learning"
        );
        assert_eq!(expired_memory().id.as_str(), "mem-fixture-expired");
        assert_eq!(superseded_memory().id.as_str(), "mem-fixture-superseded");
        assert_eq!(invalid_memory().id.as_str(), "mem-fixture-invalid");
        assert_eq!(candidate_memory().id.as_str(), "mem-fixture-candidate");
        assert_eq!(rejected_memory().id.as_str(), "mem-fixture-rejected");
        assert_eq!(corrected_memory().id.as_str(), "mem-fixture-corrected");
        assert_eq!(approved_feedback().id.as_str(), "fb-fixture-approved");
        assert_eq!(rejected_feedback().id.as_str(), "fb-fixture-rejected");
        assert_eq!(corrected_feedback().id.as_str(), "fb-fixture-corrected");
        assert_eq!(useful_feedback().id.as_str(), "fb-fixture-useful");
    }

    #[test]
    fn fixtures_are_deterministic() {
        assert_eq!(organization_fact(), organization_fact());
        assert_eq!(service_relationship_memory(), service_relationship_memory());
        assert_eq!(incident_learning_memory(), incident_learning_memory());
        assert_eq!(deployment_learning_memory(), deployment_learning_memory());
        assert_eq!(receipt_learning_memory(), receipt_learning_memory());
        assert_eq!(ability_learning_memory(), ability_learning_memory());
        assert_eq!(expired_memory(), expired_memory());
        assert_eq!(superseded_memory(), superseded_memory());
        assert_eq!(invalid_memory(), invalid_memory());
        assert_eq!(candidate_memory(), candidate_memory());
        assert_eq!(rejected_memory(), rejected_memory());
        assert_eq!(corrected_memory(), corrected_memory());
        assert_eq!(approved_feedback(), approved_feedback());
        assert_eq!(rejected_feedback(), rejected_feedback());
        assert_eq!(corrected_feedback(), corrected_feedback());
        assert_eq!(useful_feedback(), useful_feedback());
        assert_eq!(sample_index(), sample_index());
        assert_eq!(empty_index(), empty_index());
    }

    #[test]
    fn valid_fixtures_pass_validation() {
        assert!(validate_record(&organization_fact()).is_ok());
        assert!(validate_record(&service_relationship_memory()).is_ok());
        assert!(validate_record(&incident_learning_memory()).is_ok());
        assert!(validate_record(&deployment_learning_memory()).is_ok());
        assert!(validate_record(&receipt_learning_memory()).is_ok());
        assert!(validate_record(&ability_learning_memory()).is_ok());
        assert!(validate_record(&expired_memory()).is_ok());
        assert!(validate_record(&superseded_memory()).is_ok());
        assert!(validate_record(&candidate_memory()).is_ok());
        assert!(validate_record(&rejected_memory()).is_ok());
        assert!(validate_record(&corrected_memory()).is_ok());
    }
}
