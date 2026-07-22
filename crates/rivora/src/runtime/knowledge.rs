//! Knowledge engine — deterministic derivation from Memory (RFC-007).

use crate::domain::{
    Confidence, DerivationMetadata, InvestigationId, InvestigationStatus, KnowledgeKind,
    KnowledgeObject, ObservationKind, Provenance,
};
use crate::error::{RivoraError, RivoraResult};
use crate::runtime::Runtime;

impl Runtime {
    /// Derive Knowledge from Investigation Memory.
    ///
    /// v0.1 uses deterministic, explainable rules (no cross-investigation semantics).
    /// Refreshes derived Knowledge when called after new Memory arrives.
    pub fn derive_knowledge(
        &self,
        investigation_id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<Vec<KnowledgeObject>> {
        let actor = actor.into();
        let inv = self.store.load_investigation(&investigation_id)?;
        let memory = self.store.list_memory(&investigation_id)?;
        if memory.is_empty() {
            return Err(RivoraError::Precondition(
                "cannot derive knowledge without memory".into(),
            ));
        }

        // Advance lifecycle toward Understanding.
        if inv.status == InvestigationStatus::Collecting
            || inv.status == InvestigationStatus::Created
        {
            self.ensure_status_at_least(investigation_id, InvestigationStatus::Understanding)?;
        }

        let observations = self.store.list_observations(&investigation_id)?;
        let provenance = Provenance::now(actor, "runtime")
            .with_capability("derive_knowledge")
            .with_evidence(memory.iter().map(|m| m.id).collect());

        let mut knowledge = Vec::new();

        // 1. Summary knowledge
        let summary = format!(
            "Investigation has {} memory record(s) from {} observation(s).",
            memory.len(),
            observations.len()
        );
        knowledge.push(KnowledgeObject::new(
            investigation_id,
            summary,
            KnowledgeKind::Summary,
            memory.iter().map(|m| m.id).collect(),
            Confidence::new(0.9),
            DerivationMetadata {
                method: "deterministic_summary_v1".into(),
                explanation: "Counts Memory and Observations for the Investigation.".into(),
            },
            provenance.clone(),
        ));

        // 2. Activity knowledge by observation kind
        let mut kind_counts: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        for obs in &observations {
            *kind_counts
                .entry(obs.kind.as_str().to_string())
                .or_default() += 1;
        }
        if !kind_counts.is_empty() {
            let parts: Vec<String> = kind_counts
                .iter()
                .map(|(k, n)| format!("{k}×{n}"))
                .collect();
            knowledge.push(KnowledgeObject::new(
                investigation_id,
                format!("Observed activity mix: {}.", parts.join(", ")),
                KnowledgeKind::Activity,
                memory.iter().map(|m| m.id).collect(),
                Confidence::new(0.85),
                DerivationMetadata {
                    method: "activity_histogram_v1".into(),
                    explanation: "Histogram of Observation kinds in the Investigation.".into(),
                },
                provenance.clone(),
            ));
        }

        // 3. Risk signals from known failure-like content
        let failure_signals: Vec<_> = observations
            .iter()
            .filter(|o| {
                let text = format!(
                    "{} {}",
                    o.summary.to_lowercase(),
                    o.payload.to_string().to_lowercase()
                );
                text.contains("fail")
                    || text.contains("error")
                    || text.contains("rollback")
                    || matches!(
                        o.kind,
                        ObservationKind::CheckResult | ObservationKind::TestOutput
                    ) && (text.contains("fail") || text.contains("error"))
            })
            .collect();

        if !failure_signals.is_empty() {
            let mem_ids: Vec<_> = memory
                .iter()
                .filter(|m| failure_signals.iter().any(|o| o.id == m.observation_id))
                .map(|m| m.id)
                .collect();
            knowledge.push(KnowledgeObject::new(
                investigation_id,
                format!(
                    "Detected {} potential failure-related observation(s).",
                    failure_signals.len()
                ),
                KnowledgeKind::RiskSignal,
                mem_ids,
                Confidence::new(0.75),
                DerivationMetadata {
                    method: "failure_keyword_scan_v1".into(),
                    explanation: "Scans Observation summaries/payloads for failure indicators."
                        .into(),
                },
                provenance.clone(),
            ));
        }

        // 4. Pattern: corrections present
        let corrections: Vec<_> = memory.iter().filter(|m| m.corrects.is_some()).collect();
        if !corrections.is_empty() {
            knowledge.push(KnowledgeObject::new(
                investigation_id,
                format!(
                    "{} correction record(s) refine earlier Memory without rewriting history.",
                    corrections.len()
                ),
                KnowledgeKind::Pattern,
                corrections.iter().map(|m| m.id).collect(),
                Confidence::certain(),
                DerivationMetadata {
                    method: "correction_scan_v1".into(),
                    explanation: "Identifies append-only correction Memory records.".into(),
                },
                provenance,
            ));
        }

        self.store
            .replace_knowledge(&investigation_id, &knowledge)?;
        Ok(knowledge)
    }

    /// List currently derived Knowledge for an Investigation.
    pub fn list_knowledge(
        &self,
        investigation_id: InvestigationId,
    ) -> RivoraResult<Vec<KnowledgeObject>> {
        let _ = self.store.load_investigation(&investigation_id)?;
        self.store.list_knowledge(&investigation_id)
    }
}
