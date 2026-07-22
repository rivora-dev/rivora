//! Evaluation engine — deterministic assessments (RFC-008).

use crate::domain::{
    AssessmentType, Confidence, Evaluation, InvestigationId, InvestigationStatus, KnowledgeKind,
    Provenance, Severity,
};
use crate::error::{RivoraError, RivoraResult};
use crate::runtime::Runtime;

impl Runtime {
    /// Evaluate Investigation Knowledge and produce explainable assessments.
    pub fn evaluate_investigation(
        &self,
        investigation_id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<Vec<Evaluation>> {
        let actor = actor.into();
        let _inv = self.store.load_investigation(&investigation_id)?;
        let mut knowledge = self.store.list_knowledge(&investigation_id)?;
        if knowledge.is_empty() {
            // Derive if needed for a smoother capability flow.
            knowledge = self.derive_knowledge(investigation_id, actor.clone())?;
        }
        if knowledge.is_empty() {
            return Err(RivoraError::Precondition(
                "cannot evaluate without knowledge".into(),
            ));
        }

        self.ensure_status_at_least(investigation_id, InvestigationStatus::Evaluating)?;

        let memory = self.store.list_memory(&investigation_id)?;
        let memory_ids: Vec<_> = memory.iter().map(|m| m.id).collect();
        let knowledge_ids: Vec<_> = knowledge.iter().map(|k| k.id).collect();

        let has_risk = knowledge
            .iter()
            .any(|k| matches!(k.kind, KnowledgeKind::RiskSignal));
        let memory_count = memory.len();

        let provenance = Provenance::now(actor, "runtime")
            .with_capability("evaluate_investigation")
            .with_evidence(knowledge_ids.clone());

        let mut evaluations = Vec::new();

        // Risk assessment
        let (risk_severity, risk_summary, risk_conf, risk_expl) = if has_risk {
            (
                Severity::High,
                "Elevated risk: failure-related signals present in Memory.",
                Confidence::new(0.8),
                "Knowledge contains RiskSignal derived from failure indicators in Observations."
                    .to_string(),
            )
        } else if memory_count == 0 {
            (
                Severity::Medium,
                "Insufficient Memory to assess risk confidently.",
                Confidence::new(0.3),
                "No Memory records available.".to_string(),
            )
        } else {
            (
                Severity::Low,
                "No strong failure signals detected in current Knowledge.",
                Confidence::new(0.7),
                "Activity Knowledge present without RiskSignal objects.".to_string(),
            )
        };

        evaluations.push(Evaluation::new(
            investigation_id,
            AssessmentType::Risk,
            risk_summary,
            risk_severity,
            risk_conf,
            knowledge_ids.clone(),
            memory_ids.clone(),
            risk_expl,
            provenance.clone(),
        ));

        // Health assessment
        let health_severity = match memory_count {
            0 => Severity::Medium,
            1..=2 => Severity::Low,
            _ => {
                if has_risk {
                    Severity::Medium
                } else {
                    Severity::Info
                }
            }
        };
        evaluations.push(Evaluation::new(
            investigation_id,
            AssessmentType::Health,
            format!(
                "Investigation health based on {memory_count} Memory record(s) and {} Knowledge object(s).",
                knowledge.len()
            ),
            health_severity,
            Confidence::new(0.75),
            knowledge_ids.clone(),
            memory_ids.clone(),
            "Health is derived from Memory volume and presence of risk signals.",
            provenance.clone(),
        ));

        // Confidence assessment
        let conf_value = if memory_count >= 3 {
            0.85
        } else if memory_count >= 1 {
            0.6
        } else {
            0.2
        };
        evaluations.push(Evaluation::new(
            investigation_id,
            AssessmentType::Confidence,
            format!(
                "Understanding confidence is {:.0}% based on available evidence volume.",
                conf_value * 100.0
            ),
            Severity::Info,
            Confidence::new(conf_value),
            knowledge_ids.clone(),
            memory_ids.clone(),
            "Confidence scales with the number of Memory records supporting Knowledge.",
            provenance.clone(),
        ));

        // Readiness
        let readiness_severity = if has_risk {
            Severity::Medium
        } else {
            Severity::Low
        };
        evaluations.push(Evaluation::new(
            investigation_id,
            AssessmentType::Readiness,
            if has_risk {
                "Ready for verification of failure-related conclusions."
            } else {
                "Ready for routine verification of current understanding."
            },
            readiness_severity,
            Confidence::new(0.7),
            knowledge_ids,
            memory_ids,
            "Readiness indicates whether Evaluation outputs are prepared for Verification.",
            provenance,
        ));

        for evaluation in &evaluations {
            self.store.append_evaluation(evaluation)?;
        }

        Ok(evaluations)
    }

    /// List Evaluations for an Investigation.
    pub fn list_evaluations(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<Vec<Evaluation>> {
        let _ = self.store.load_investigation(&investigation_id)?;
        self.store.list_evaluations(&investigation_id)
    }
}
