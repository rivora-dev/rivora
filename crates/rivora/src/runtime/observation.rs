//! Observation ingestion (Phase 2).

use crate::domain::{
    InvestigationStatus, MemoryRecord, Observation, ObservationKind, Provenance, MAX_PAYLOAD_BYTES,
};
use crate::error::{RivoraError, RivoraResult};
use crate::runtime::Runtime;
use chrono::{DateTime, Utc};

/// Input for ingesting a normalized Observation.
#[derive(Debug, Clone)]
pub struct IngestObservationRequest {
    /// Target Investigation.
    pub investigation_id: crate::domain::InvestigationId,
    /// Observation kind.
    pub kind: ObservationKind,
    /// Summary of what happened.
    pub summary: String,
    /// Structured payload.
    pub payload: serde_json::Value,
    /// Source system name.
    pub source: String,
    /// When the event occurred.
    pub observed_at: DateTime<Utc>,
    /// Optional idempotency key.
    pub idempotency_key: Option<String>,
    /// Actor performing ingestion.
    pub actor: String,
}

/// Result of observation ingestion.
#[derive(Debug, Clone)]
pub struct IngestObservationResult {
    /// The Observation (new or existing if idempotent hit).
    pub observation: Observation,
    /// Memory record created (or existing for idempotent hit).
    pub memory: MemoryRecord,
    /// Whether this was an idempotent replay.
    pub idempotent_replay: bool,
}

impl Runtime {
    /// Ingest a normalized Observation into Memory.
    ///
    /// - Validates required fields.
    /// - Associates with the Investigation.
    /// - Persists Observation and append-only Memory.
    /// - Idempotent when `idempotency_key` matches an existing Observation.
    pub fn ingest_observation(
        &self,
        request: IngestObservationRequest,
    ) -> RivoraResult<IngestObservationResult> {
        let mut inv = self.store.load_investigation(&request.investigation_id)?;

        // Bound payload size to the supported operating envelope.
        let payload_bytes = serde_json::to_vec(&request.payload)
            .map_err(|e| RivoraError::serialization(e.to_string()))?
            .len();
        if payload_bytes > MAX_PAYLOAD_BYTES {
            return Err(RivoraError::payload_too_large(format!(
                "observation payload is {payload_bytes} bytes; max is {MAX_PAYLOAD_BYTES}"
            )));
        }

        // Idempotency check first.
        if let Some(ref key) = request.idempotency_key {
            if let Some(result) =
                self.replay_observation_if_present(&request.investigation_id, key)?
            {
                return Ok(result);
            }
        }

        if inv.status == InvestigationStatus::Completed {
            return Err(RivoraError::OperationNotAllowed {
                status: inv.status,
                message: "cannot ingest into completed investigation; reopen first".into(),
            });
        }

        let provenance = Provenance::now(request.actor, request.source.clone())
            .with_capability("ingest_observation");

        // Validate Observation before mutating Investigation state.
        let observation = Observation::new(
            request.investigation_id,
            request.kind,
            request.summary.clone(),
            request.payload,
            request.source,
            request.observed_at,
            request.idempotency_key.clone(),
            provenance.clone(),
        )?;

        // Move Created → Collecting on first valid observation.
        if inv.status == InvestigationStatus::Created {
            inv.transition_to(
                InvestigationStatus::Collecting,
                Some("first observation ingested".into()),
            )?;
            self.store.save_investigation(&inv)?;
        }

        match self.store.append_observation(&observation) {
            Ok(()) => {}
            Err(RivoraError::Conflict(_)) => {
                // Concurrent claim of the same idempotency key — reuse winner.
                if let Some(ref key) = request.idempotency_key {
                    if let Some(result) =
                        self.replay_observation_if_present(&request.investigation_id, key)?
                    {
                        return Ok(result);
                    }
                }
                return Err(RivoraError::conflict(
                    "observation append conflict without recoverable idempotency key",
                ));
            }
            Err(e) => return Err(e),
        }

        let memory = MemoryRecord::from_observation(
            observation.id,
            observation.investigation_id,
            observation.summary.clone(),
            Utc::now(),
            provenance,
        );
        self.store.append_memory(&memory)?;

        Ok(IngestObservationResult {
            observation,
            memory,
            idempotent_replay: false,
        })
    }

    /// Ingest a correction as a new Observation + Memory that references prior Memory.
    pub fn ingest_correction(
        &self,
        investigation_id: crate::domain::InvestigationId,
        corrects_memory_id: crate::domain::ObjectId,
        summary: impl Into<String>,
        payload: serde_json::Value,
        actor: impl Into<String>,
    ) -> RivoraResult<IngestObservationResult> {
        let summary = summary.into();
        let actor = actor.into();
        let prior = self
            .store
            .list_memory(&investigation_id)?
            .into_iter()
            .find(|m| m.id == corrects_memory_id)
            .ok_or(RivoraError::ObjectNotFound(corrects_memory_id))?;

        let provenance = Provenance::now(actor, "runtime").with_capability("ingest_correction");
        let observation = Observation::new(
            investigation_id,
            ObservationKind::UserInput,
            summary.clone(),
            payload,
            "correction",
            Utc::now(),
            None,
            provenance.clone(),
        )?;
        self.store.append_observation(&observation)?;

        let memory = MemoryRecord::correction(
            observation.id,
            investigation_id,
            summary,
            prior.id,
            Utc::now(),
            provenance,
        );
        self.store.append_memory(&memory)?;

        Ok(IngestObservationResult {
            observation,
            memory,
            idempotent_replay: false,
        })
    }

    fn replay_observation_if_present(
        &self,
        investigation_id: &crate::domain::InvestigationId,
        key: &str,
    ) -> RivoraResult<Option<IngestObservationResult>> {
        let Some(existing) = self
            .store
            .find_observation_by_idempotency(investigation_id, key)?
        else {
            return Ok(None);
        };
        let memory = self
            .store
            .list_memory(investigation_id)?
            .into_iter()
            .find(|m| m.observation_id == existing.id)
            .ok_or_else(|| {
                RivoraError::Precondition("idempotent observation missing memory record".into())
            })?;
        Ok(Some(IngestObservationResult {
            observation: existing,
            memory,
            idempotent_replay: true,
        }))
    }
}
