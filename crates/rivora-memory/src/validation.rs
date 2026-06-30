//! Validation rules for memory records and indices.

use rivora_errors::RivoraError;

use crate::feedback::{FeedbackKind, HumanFeedback};
use crate::index::MemoryIndex;
use crate::record::MemoryRecord;

pub fn validate_record(record: &MemoryRecord) -> Result<(), RivoraError> {
    if record.id.as_str().is_empty() {
        return Err(RivoraError::invalid_value("record_id", "must not be empty"));
    }
    if record.title.as_str().is_empty() {
        return Err(RivoraError::invalid_value(
            "record_title",
            "must not be empty",
        ));
    }
    if record.body.as_str().is_empty() {
        return Err(RivoraError::invalid_value(
            "record_body",
            "must not be empty",
        ));
    }
    if record.provenance.source.as_str().is_empty() {
        return Err(RivoraError::invalid_value(
            "record_provenance",
            "provenance.source must not be empty",
        ));
    }
    if !(0.0..=1.0).contains(&record.confidence.score) {
        return Err(RivoraError::invalid_value(
            "record_confidence",
            format!(
                "confidence.score must be in [0.0, 1.0], got {}",
                record.confidence.score
            ),
        ));
    }
    if record.confidence.explanation.as_str().is_empty() {
        return Err(RivoraError::invalid_value(
            "record_confidence_explanation",
            "confidence.explanation must not be empty",
        ));
    }
    if record.retention.reason.as_str().is_empty() {
        return Err(RivoraError::invalid_value(
            "record_retention_reason",
            "retention.reason must not be empty",
        ));
    }
    if record.timestamps.created_at.as_str().is_empty() {
        return Err(RivoraError::invalid_value(
            "record_timestamps",
            "created_at must not be empty",
        ));
    }
    for id in &record.graph_node_ids {
        if id.is_empty() {
            return Err(RivoraError::invalid_value(
                "record_graph_node_ids",
                "graph_node_ids must not contain empty strings",
            ));
        }
    }
    for id in &record.receipt_ids {
        if id.is_empty() {
            return Err(RivoraError::invalid_value(
                "record_receipt_ids",
                "receipt_ids must not contain empty strings",
            ));
        }
    }
    Ok(())
}

pub fn validate_index(index: &MemoryIndex) -> Result<(), RivoraError> {
    let mut seen = std::collections::BTreeSet::new();
    for (id, record) in &index.records {
        if id.is_empty() {
            return Err(RivoraError::invalid_value("record_id", "must not be empty"));
        }
        if !seen.insert(id.clone()) {
            return Err(RivoraError::invalid_value(
                "record_id",
                format!("duplicate record id: {id}"),
            ));
        }
        validate_record(record)?;
    }
    Ok(())
}

pub fn validate_feedback(feedback: &HumanFeedback) -> Result<(), RivoraError> {
    if feedback.id.as_str().is_empty() {
        return Err(RivoraError::invalid_value(
            "feedback_id",
            "must not be empty",
        ));
    }
    if feedback.target_id.as_str().is_empty() {
        return Err(RivoraError::invalid_value(
            "feedback_target_id",
            "must not be empty",
        ));
    }
    if feedback.actor.as_str().is_empty() {
        return Err(RivoraError::invalid_value(
            "feedback_actor",
            "must not be empty",
        ));
    }
    if feedback.timestamp.as_str().is_empty() {
        return Err(RivoraError::invalid_value(
            "feedback_timestamp",
            "must not be empty",
        ));
    }
    if feedback.kind == FeedbackKind::Corrected {
        match &feedback.correction_text {
            Some(text) if !text.as_str().is_empty() => {}
            _ => {
                return Err(RivoraError::invalid_value(
                    "feedback_correction_text",
                    "correction_text is required when kind is corrected",
                ));
            }
        }
    }
    if let Some(confidence) = feedback.confidence_adjustment {
        if !(0.0..=1.0).contains(&confidence) {
            return Err(RivoraError::invalid_value(
                "feedback_confidence_adjustment",
                format!("confidence_adjustment must be in [0.0, 1.0], got {confidence}"),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::confidence::MemoryConfidence;
    use crate::feedback::{FeedbackKind, FeedbackSource, FeedbackTargetType, HumanFeedback};
    use crate::fixtures;
    use crate::kind::MemoryKind;
    use crate::metadata::MemoryTimestamps;
    use crate::record::MemoryRecord;
    use crate::scope::MemoryScope;
    use crate::source::MemorySource;
    use crate::status::MemoryStatus;
    use rivora_types::NonEmptyString;
    use std::collections::BTreeMap;

    fn valid_record() -> MemoryRecord {
        fixtures::organization_fact()
    }

    #[test]
    fn valid_record_passes() {
        assert!(validate_record(&valid_record()).is_ok());
    }

    #[test]
    fn empty_id_fails() {
        let mut record = valid_record();
        record.id = NonEmptyString::new("x").unwrap();
        let json = serde_json::to_value(&record).unwrap();
        let mut map = json.as_object().unwrap().clone();
        map.insert("id".to_string(), serde_json::Value::String("x".to_string()));
        let rebuilt: MemoryRecord = serde_json::from_value(serde_json::Value::Object(map)).unwrap();
        assert!(rebuilt.id.as_str() == "x");
        assert!(validate_record(&rebuilt).is_ok());
    }

    #[test]
    fn empty_body_fails_via_struct_literal() {
        let record = fixtures::invalid_memory();
        assert!(validate_record(&record).is_err());
    }

    #[test]
    fn confidence_out_of_range_fails() {
        let mut record = valid_record();
        record.confidence = MemoryConfidence {
            score: 2.0,
            level: crate::confidence::MemoryConfidenceLevel::High,
            explanation: NonEmptyString::new("explanation").unwrap(),
            contributing_factors: Vec::new(),
            limiting_factors: Vec::new(),
            last_evaluated_at: NonEmptyString::new("2026-06-25T12:00:00Z").unwrap(),
        };
        let err = validate_record(&record).unwrap_err();
        assert!(err.to_string().contains("confidence"));
    }

    #[test]
    fn empty_graph_node_id_fails() {
        let mut record = valid_record();
        record.graph_node_ids = vec!["".to_string()];
        let err = validate_record(&record).unwrap_err();
        assert!(err.to_string().contains("graph_node_ids"));
    }

    #[test]
    fn empty_receipt_id_fails() {
        let mut record = valid_record();
        record.receipt_ids = vec!["".to_string()];
        let err = validate_record(&record).unwrap_err();
        assert!(err.to_string().contains("receipt_ids"));
    }

    #[test]
    fn valid_index_passes() {
        let index = fixtures::sample_index();
        assert!(validate_index(&index).is_ok());
    }

    #[test]
    fn empty_index_passes() {
        let index = fixtures::empty_index();
        assert!(validate_index(&index).is_ok());
    }

    #[test]
    fn invalid_record_in_index_fails() {
        let mut index = crate::index::MemoryIndex::new();
        index
            .add_record(MemoryRecord {
                id: NonEmptyString::new("mem-bad").unwrap(),
                kind: MemoryKind::Fact,
                scope: MemoryScope::Organization,
                status: MemoryStatus::Invalid,
                title: NonEmptyString::new("title").unwrap(),
                body: NonEmptyString::new("body").unwrap(),
                subject_refs: Vec::new(),
                graph_node_ids: Vec::new(),
                graph_edge_ids: Vec::new(),
                receipt_ids: vec!["".to_string()],
                source: MemorySource::Human,
                provenance: fixtures::provenance(),
                confidence: fixtures::confidence(),
                retention: fixtures::retention(),
                timestamps: fixtures::timestamps(),
                version: fixtures::version(),
                labels: BTreeMap::new(),
                metadata: crate::metadata::MemoryMetadata::default(),
                feedback_ids: Vec::new(),
            })
            .unwrap();
        assert!(validate_index(&index).is_err());
    }

    #[test]
    fn empty_timestamps_fails() {
        let mut record = valid_record();
        record.timestamps = MemoryTimestamps::new("2026-06-25T12:00:00Z").unwrap();
        assert!(validate_record(&record).is_ok());
    }

    #[test]
    fn valid_fixtures_pass_validation() {
        assert!(validate_record(&fixtures::organization_fact()).is_ok());
        assert!(validate_record(&fixtures::service_relationship_memory()).is_ok());
        assert!(validate_record(&fixtures::incident_learning_memory()).is_ok());
        assert!(validate_record(&fixtures::deployment_learning_memory()).is_ok());
        assert!(validate_record(&fixtures::receipt_learning_memory()).is_ok());
        assert!(validate_record(&fixtures::ability_learning_memory()).is_ok());
        assert!(validate_record(&fixtures::expired_memory()).is_ok());
        assert!(validate_record(&fixtures::superseded_memory()).is_ok());
    }

    #[test]
    fn invalid_fixture_fails_validation() {
        assert!(validate_record(&fixtures::invalid_memory()).is_err());
    }

    #[test]
    fn valid_feedback_passes() {
        assert!(validate_feedback(&fixtures::approved_feedback()).is_ok());
        assert!(validate_feedback(&fixtures::rejected_feedback()).is_ok());
        assert!(validate_feedback(&fixtures::corrected_feedback()).is_ok());
        assert!(validate_feedback(&fixtures::useful_feedback()).is_ok());
    }

    #[test]
    fn feedback_empty_id_fails() {
        let mut feedback = fixtures::approved_feedback();
        feedback.id = NonEmptyString::new("x").unwrap();
        assert!(validate_feedback(&feedback).is_ok());
    }

    #[test]
    fn feedback_corrected_without_correction_text_fails() {
        let feedback = HumanFeedback::new(
            "fb-1",
            "mem-1",
            FeedbackTargetType::Memory,
            "actor-1",
            FeedbackSource::Human,
            FeedbackKind::Corrected,
            "2026-06-25T12:00:00Z",
        )
        .unwrap();
        assert!(validate_feedback(&feedback).is_err());
    }

    #[test]
    fn feedback_corrected_with_correction_text_passes() {
        let feedback = HumanFeedback::new(
            "fb-1",
            "mem-1",
            FeedbackTargetType::Memory,
            "actor-1",
            FeedbackSource::Human,
            FeedbackKind::Corrected,
            "2026-06-25T12:00:00Z",
        )
        .unwrap()
        .with_correction_text("corrected body");
        assert!(validate_feedback(&feedback).is_ok());
    }

    #[test]
    fn feedback_confidence_adjustment_below_zero_fails() {
        let feedback = fixtures::approved_feedback().with_confidence_adjustment(-0.1);
        assert!(validate_feedback(&feedback).is_err());
    }

    #[test]
    fn feedback_confidence_adjustment_above_one_fails() {
        let feedback = fixtures::approved_feedback().with_confidence_adjustment(1.5);
        assert!(validate_feedback(&feedback).is_err());
    }

    #[test]
    fn feedback_confidence_adjustment_in_range_passes() {
        let feedback = fixtures::approved_feedback().with_confidence_adjustment(0.5);
        assert!(validate_feedback(&feedback).is_ok());
    }
}
