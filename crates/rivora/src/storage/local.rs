//! Filesystem-backed local store.
//!
//! Layout:
//! ```text
//! root/
//!   investigations/{id}/
//!     investigation.json
//!     observations/{object_id}.json
//!     memory/{object_id}.json
//!     knowledge/{object_id}.json
//!     evaluations/{object_id}.json
//!     verifications/{object_id}.json
//!     recommendations/{object_id}.json
//!     learning/{object_id}.json
//!   graph/
//!     relationships/{object_id}.json
//! ```
//!
//! The `graph` area (RFC-015) is separate from per-Investigation
//! directories. It is created lazily on first relationship write, so
//! stores containing only v0.1 data keep working unchanged.

use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::{
    Evaluation, Investigation, InvestigationId, InvestigationRelationship, KnowledgeObject,
    LearningOutcome, MemoryRecord, ObjectId, Observation, Recommendation, TimelineEntry,
    VerificationReceipt,
};
use crate::error::{RivoraError, RivoraResult};

use super::Store;

/// Local directory-based store.
#[derive(Debug, Clone)]
pub struct LocalStore {
    root: PathBuf,
}

impl LocalStore {
    /// Open or create a store at `root`.
    pub fn open(root: impl Into<PathBuf>) -> RivoraResult<Self> {
        let root = root.into();
        fs::create_dir_all(root.join("investigations"))
            .map_err(|e| RivoraError::storage(format!("failed to create store root: {e}")))?;
        Ok(Self { root })
    }

    /// Store root path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn inv_dir(&self, id: &InvestigationId) -> PathBuf {
        self.root.join("investigations").join(id.to_string())
    }

    fn ensure_inv_dirs(&self, id: &InvestigationId) -> RivoraResult<PathBuf> {
        let dir = self.inv_dir(id);
        for sub in [
            "observations",
            "memory",
            "knowledge",
            "evaluations",
            "verifications",
            "recommendations",
            "learning",
        ] {
            fs::create_dir_all(dir.join(sub))
                .map_err(|e| RivoraError::storage(format!("failed to create {sub} dir: {e}")))?;
        }
        Ok(dir)
    }

    fn write_json<T: serde::Serialize>(&self, path: &Path, value: &T) -> RivoraResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| RivoraError::storage(format!("failed to create parent dir: {e}")))?;
        }
        let data = serde_json::to_vec_pretty(value)
            .map_err(|e| RivoraError::serialization(e.to_string()))?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, &data)
            .map_err(|e| RivoraError::storage(format!("failed to write temp file: {e}")))?;
        fs::rename(&tmp, path)
            .map_err(|e| RivoraError::storage(format!("failed to finalize write: {e}")))?;
        Ok(())
    }

    fn read_json<T: serde::de::DeserializeOwned>(&self, path: &Path) -> RivoraResult<T> {
        let data = fs::read(path)
            .map_err(|e| RivoraError::storage(format!("failed to read {}: {e}", path.display())))?;
        serde_json::from_slice(&data).map_err(|e| RivoraError::serialization(e.to_string()))
    }

    fn list_json_dir<T: serde::de::DeserializeOwned>(&self, dir: &Path) -> RivoraResult<Vec<T>> {
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut items = Vec::new();
        let entries = fs::read_dir(dir).map_err(|e| {
            RivoraError::storage(format!("failed to read dir {}: {e}", dir.display()))
        })?;
        for entry in entries {
            let entry = entry
                .map_err(|e| RivoraError::storage(format!("failed to read dir entry: {e}")))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                items.push(self.read_json(&path)?);
            }
        }
        Ok(items)
    }

    fn object_path(
        &self,
        investigation_id: &InvestigationId,
        kind: &str,
        id: &ObjectId,
    ) -> PathBuf {
        self.inv_dir(investigation_id)
            .join(kind)
            .join(format!("{id}.json"))
    }

    fn graph_relationships_dir(&self) -> PathBuf {
        self.root.join("graph").join("relationships")
    }

    /// Create the graph relationship directory on first use.
    ///
    /// `open` deliberately does not require this directory so stores
    /// holding only v0.1 data remain valid.
    fn ensure_graph_relationships_dir(&self) -> RivoraResult<PathBuf> {
        let dir = self.graph_relationships_dir();
        fs::create_dir_all(&dir)
            .map_err(|e| RivoraError::storage(format!("failed to create graph dir: {e}")))?;
        Ok(dir)
    }

    fn relationship_path(&self, id: &ObjectId) -> PathBuf {
        self.graph_relationships_dir().join(format!("{id}.json"))
    }
}

impl Store for LocalStore {
    fn save_investigation(&self, investigation: &Investigation) -> RivoraResult<()> {
        let dir = self.ensure_inv_dirs(&investigation.id)?;
        self.write_json(&dir.join("investigation.json"), investigation)
    }

    fn load_investigation(&self, id: &InvestigationId) -> RivoraResult<Investigation> {
        let path = self.inv_dir(id).join("investigation.json");
        if !path.exists() {
            return Err(RivoraError::InvestigationNotFound(*id));
        }
        self.read_json(&path)
    }

    fn list_investigations(&self) -> RivoraResult<Vec<InvestigationId>> {
        let dir = self.root.join("investigations");
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut ids = Vec::new();
        for entry in fs::read_dir(&dir)
            .map_err(|e| RivoraError::storage(format!("failed to list investigations: {e}")))?
        {
            let entry = entry
                .map_err(|e| RivoraError::storage(format!("failed to read dir entry: {e}")))?;
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                if let Ok(id) = entry.file_name().to_string_lossy().parse() {
                    ids.push(id);
                }
            }
        }
        ids.sort_by_key(|id: &InvestigationId| id.to_string());
        Ok(ids)
    }

    fn append_observation(&self, observation: &Observation) -> RivoraResult<()> {
        self.ensure_inv_dirs(&observation.investigation_id)?;
        let path = self.object_path(
            &observation.investigation_id,
            "observations",
            &observation.id,
        );
        if path.exists() {
            return Err(RivoraError::storage(format!(
                "observation {} already exists",
                observation.id
            )));
        }
        self.write_json(&path, observation)
    }

    fn list_observations(&self, id: &InvestigationId) -> RivoraResult<Vec<Observation>> {
        let mut items: Vec<Observation> =
            self.list_json_dir(&self.inv_dir(id).join("observations"))?;
        items.sort_by_key(|o| o.observed_at);
        Ok(items)
    }

    fn find_observation_by_idempotency(
        &self,
        investigation_id: &InvestigationId,
        key: &str,
    ) -> RivoraResult<Option<Observation>> {
        Ok(self
            .list_observations(investigation_id)?
            .into_iter()
            .find(|o| o.idempotency_key.as_deref() == Some(key)))
    }

    fn append_memory(&self, record: &MemoryRecord) -> RivoraResult<()> {
        self.ensure_inv_dirs(&record.investigation_id)?;
        let path = self.object_path(&record.investigation_id, "memory", &record.id);
        if path.exists() {
            return Err(RivoraError::storage(format!(
                "memory record {} already exists (append-only)",
                record.id
            )));
        }
        self.write_json(&path, record)
    }

    fn list_memory(&self, id: &InvestigationId) -> RivoraResult<Vec<MemoryRecord>> {
        let mut items: Vec<MemoryRecord> = self.list_json_dir(&self.inv_dir(id).join("memory"))?;
        items.sort_by_key(|m| m.recorded_at);
        Ok(items)
    }

    fn replace_knowledge(
        &self,
        investigation_id: &InvestigationId,
        objects: &[KnowledgeObject],
    ) -> RivoraResult<()> {
        let dir = self.ensure_inv_dirs(investigation_id)?.join("knowledge");
        // Knowledge is derived and refreshable — remove previous derived set.
        if dir.exists() {
            for entry in fs::read_dir(&dir)
                .map_err(|e| RivoraError::storage(format!("failed to read knowledge dir: {e}")))?
            {
                let entry = entry
                    .map_err(|e| RivoraError::storage(format!("failed to read entry: {e}")))?;
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    fs::remove_file(&path).map_err(|e| {
                        RivoraError::storage(format!("failed to remove old knowledge: {e}"))
                    })?;
                }
            }
        }
        for obj in objects {
            let path = self.object_path(investigation_id, "knowledge", &obj.id);
            self.write_json(&path, obj)?;
        }
        Ok(())
    }

    fn list_knowledge(&self, id: &InvestigationId) -> RivoraResult<Vec<KnowledgeObject>> {
        let mut items: Vec<KnowledgeObject> =
            self.list_json_dir(&self.inv_dir(id).join("knowledge"))?;
        items.sort_by_key(|k| k.derived_at);
        Ok(items)
    }

    fn append_evaluation(&self, evaluation: &Evaluation) -> RivoraResult<()> {
        self.ensure_inv_dirs(&evaluation.investigation_id)?;
        let path = self.object_path(&evaluation.investigation_id, "evaluations", &evaluation.id);
        if path.exists() {
            return Err(RivoraError::storage(format!(
                "evaluation {} already exists",
                evaluation.id
            )));
        }
        self.write_json(&path, evaluation)
    }

    fn list_evaluations(&self, id: &InvestigationId) -> RivoraResult<Vec<Evaluation>> {
        let mut items: Vec<Evaluation> =
            self.list_json_dir(&self.inv_dir(id).join("evaluations"))?;
        items.sort_by_key(|e| e.evaluated_at);
        Ok(items)
    }

    fn append_verification(&self, receipt: &VerificationReceipt) -> RivoraResult<()> {
        self.ensure_inv_dirs(&receipt.investigation_id)?;
        let path = self.object_path(&receipt.investigation_id, "verifications", &receipt.id);
        if path.exists() {
            return Err(RivoraError::storage(format!(
                "verification {} already exists",
                receipt.id
            )));
        }
        self.write_json(&path, receipt)
    }

    fn list_verifications(&self, id: &InvestigationId) -> RivoraResult<Vec<VerificationReceipt>> {
        let mut items: Vec<VerificationReceipt> =
            self.list_json_dir(&self.inv_dir(id).join("verifications"))?;
        items.sort_by_key(|v| v.verified_at);
        Ok(items)
    }

    fn append_recommendation(&self, recommendation: &Recommendation) -> RivoraResult<()> {
        self.ensure_inv_dirs(&recommendation.investigation_id)?;
        let path = self.object_path(
            &recommendation.investigation_id,
            "recommendations",
            &recommendation.id,
        );
        if path.exists() {
            return Err(RivoraError::storage(format!(
                "recommendation {} already exists",
                recommendation.id
            )));
        }
        self.write_json(&path, recommendation)
    }

    fn list_recommendations(&self, id: &InvestigationId) -> RivoraResult<Vec<Recommendation>> {
        let mut items: Vec<Recommendation> =
            self.list_json_dir(&self.inv_dir(id).join("recommendations"))?;
        items.sort_by_key(|r| r.recommended_at);
        Ok(items)
    }

    fn save_recommendation(&self, recommendation: &Recommendation) -> RivoraResult<()> {
        self.ensure_inv_dirs(&recommendation.investigation_id)?;
        let path = self.object_path(
            &recommendation.investigation_id,
            "recommendations",
            &recommendation.id,
        );
        self.write_json(&path, recommendation)
    }

    fn load_recommendation(
        &self,
        investigation_id: &InvestigationId,
        id: &ObjectId,
    ) -> RivoraResult<Recommendation> {
        let path = self.object_path(investigation_id, "recommendations", id);
        if !path.exists() {
            return Err(RivoraError::ObjectNotFound(*id));
        }
        self.read_json(&path)
    }

    fn append_learning(&self, outcome: &LearningOutcome) -> RivoraResult<()> {
        self.ensure_inv_dirs(&outcome.investigation_id)?;
        let path = self.object_path(&outcome.investigation_id, "learning", &outcome.id);
        if path.exists() {
            return Err(RivoraError::storage(format!(
                "learning outcome {} already exists",
                outcome.id
            )));
        }
        self.write_json(&path, outcome)
    }

    fn list_learning(&self, id: &InvestigationId) -> RivoraResult<Vec<LearningOutcome>> {
        let mut items: Vec<LearningOutcome> =
            self.list_json_dir(&self.inv_dir(id).join("learning"))?;
        items.sort_by_key(|l| l.observed_at);
        Ok(items)
    }

    fn timeline(&self, id: &InvestigationId) -> RivoraResult<Vec<TimelineEntry>> {
        let observations = self.list_observations(id)?;
        let obs_by_id: std::collections::HashMap<_, _> =
            observations.iter().map(|o| (o.id, o)).collect();
        let memory = self.list_memory(id)?;
        let mut entries: Vec<TimelineEntry> = memory
            .into_iter()
            .map(|m| {
                let source = obs_by_id
                    .get(&m.observation_id)
                    .map(|o| o.source.clone())
                    .unwrap_or_else(|| "unknown".into());
                let at = obs_by_id
                    .get(&m.observation_id)
                    .map(|o| o.observed_at)
                    .unwrap_or(m.recorded_at);
                TimelineEntry {
                    memory_id: m.id,
                    observation_id: m.observation_id,
                    at,
                    summary: m.summary,
                    source,
                }
            })
            .collect();
        entries.sort_by_key(|e| e.at);
        Ok(entries)
    }

    fn save_relationship(&self, relationship: &InvestigationRelationship) -> RivoraResult<()> {
        self.ensure_graph_relationships_dir()?;
        self.write_json(&self.relationship_path(&relationship.id), relationship)
    }

    fn load_relationship(&self, id: &ObjectId) -> RivoraResult<InvestigationRelationship> {
        let path = self.relationship_path(id);
        if !path.exists() {
            return Err(RivoraError::ObjectNotFound(*id));
        }
        self.read_json(&path)
    }

    fn list_relationships(&self) -> RivoraResult<Vec<InvestigationRelationship>> {
        let mut items: Vec<InvestigationRelationship> =
            self.list_json_dir(&self.graph_relationships_dir())?;
        items.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then_with(|| a.id.to_string().cmp(&b.id.to_string()))
        });
        Ok(items)
    }

    fn delete_relationship(&self, id: &ObjectId) -> RivoraResult<()> {
        let path = self.relationship_path(id);
        if !path.exists() {
            return Err(RivoraError::ObjectNotFound(*id));
        }
        fs::remove_file(&path)
            .map_err(|e| RivoraError::storage(format!("failed to delete relationship: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ObservationKind, Provenance};
    use chrono::Utc;

    #[test]
    fn investigation_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalStore::open(dir.path()).unwrap();
        let inv = Investigation::create("Test", None, Provenance::now("test", "test")).unwrap();
        store.save_investigation(&inv).unwrap();
        let loaded = store.load_investigation(&inv.id).unwrap();
        assert_eq!(loaded.title, "Test");
        assert_eq!(loaded.id, inv.id);
    }

    #[test]
    fn memory_is_append_only() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalStore::open(dir.path()).unwrap();
        let inv = Investigation::create("T", None, Provenance::now("t", "t")).unwrap();
        store.save_investigation(&inv).unwrap();
        let obs = Observation::new(
            inv.id,
            ObservationKind::Event,
            "happened",
            serde_json::json!({}),
            "test",
            Utc::now(),
            None,
            Provenance::now("t", "t"),
        )
        .unwrap();
        store.append_observation(&obs).unwrap();
        let mem = MemoryRecord::from_observation(
            obs.id,
            inv.id,
            "happened",
            Utc::now(),
            Provenance::now("t", "t"),
        );
        store.append_memory(&mem).unwrap();
        let err = store.append_memory(&mem).unwrap_err();
        assert!(matches!(err, RivoraError::Storage(_)));
    }

    #[test]
    fn serialization_of_all_objects() {
        let inv = Investigation::create("T", Some("d".into()), Provenance::now("a", "s")).unwrap();
        let json = serde_json::to_string(&inv).unwrap();
        let back: Investigation = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, inv.id);
    }

    #[test]
    fn relationship_round_trip() {
        use crate::domain::{
            Confidence, DerivationMetadata, InvestigationRelationship, RelationshipEvidence,
            RelationshipKind,
        };

        let dir = tempfile::tempdir().unwrap();
        let store = LocalStore::open(dir.path()).unwrap();
        let inv_a = InvestigationId::new();
        let inv_b = InvestigationId::new();

        // v0.1-style store without a graph area lists no relationships.
        assert!(!store.graph_relationships_dir().exists());
        assert!(store.list_relationships().unwrap().is_empty());

        let make = |kind: RelationshipKind, key: &str| {
            InvestigationRelationship::derived(
                kind,
                inv_a,
                inv_b,
                Confidence::new(0.9),
                vec![RelationshipEvidence::new("evidence", vec![ObjectId::new()])],
                DerivationMetadata {
                    method: "test_v1".into(),
                    explanation: "test".into(),
                },
                Provenance::now("tester", "runtime"),
                key,
            )
        };
        let rel_a = make(
            RelationshipKind::SharedRepository,
            "shared_repository|acme/app",
        );
        let rel_b = make(RelationshipKind::SharedCommit, "shared_commit|abc123");
        store.save_relationship(&rel_a).unwrap();
        store.save_relationship(&rel_b).unwrap();
        assert!(store.graph_relationships_dir().exists());

        // Save is an upsert.
        let mut updated = rel_a.clone();
        updated.confirmation = crate::domain::RelationshipConfirmation::confirmed("reviewer");
        store.save_relationship(&updated).unwrap();

        let loaded = store.load_relationship(&rel_a.id).unwrap();
        assert_eq!(
            loaded.confirmation.state,
            crate::domain::ConfirmationState::Confirmed
        );

        let all = store.list_relationships().unwrap();
        assert_eq!(all.len(), 2);
        let mut sorted = all.clone();
        sorted.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then_with(|| a.id.to_string().cmp(&b.id.to_string()))
        });
        assert_eq!(all, sorted, "list order must be deterministic");

        // Survives reopen.
        let reopened = LocalStore::open(dir.path()).unwrap();
        assert_eq!(reopened.list_relationships().unwrap().len(), 2);
        assert_eq!(reopened.load_relationship(&rel_b.id).unwrap(), rel_b);

        store.delete_relationship(&rel_a.id).unwrap();
        let missing = store.load_relationship(&rel_a.id).unwrap_err();
        assert!(matches!(missing, RivoraError::ObjectNotFound(_)));
        let gone = store.delete_relationship(&rel_a.id).unwrap_err();
        assert!(matches!(gone, RivoraError::ObjectNotFound(_)));
        assert_eq!(store.list_relationships().unwrap().len(), 1);
    }
}
