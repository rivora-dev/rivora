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
//!     context/{object_id}.json
//!     workflows/{object_id}.json
//!     hypotheses/{object_id}.json
//!     assistance/readiness/{object_id}.json
//!     assistance/risks/{object_id}.json
//!     assistance/verification_suggestions/{object_id}.json
//!     assistance/root_cause/{object_id}.json
//!     assistance/reports/{object_id}.json
//!     proposals/{object_id}.json
//!     proposal_artifacts/{object_id}.json
//!     implementations/{object_id}.json
//!     learning_outcomes/{object_id}.json
//!     execution_plans/{object_id}.json
//!     execution_approvals/{object_id}.json
//!     execution_attempts/{object_id}.json
//!     execution_receipts/{object_id}.json
//!     execution_verifications/{object_id}.json
//!     lifecycle_runs/{object_id}.json
//!   graph/
//!     relationships/{object_id}.json
//!   learning/
//!     patterns/{pattern_id}.json
//! ```
//!
//! The `graph` area (RFC-015) is separate from per-Investigation
//! directories. It is created lazily on first relationship write, so
//! stores containing only v0.1 data keep working unchanged.
//! New v0.3+ directories are created lazily for the same reason.
//! v0.5 `implementations/`, `learning_outcomes/`, and root
//! `learning/patterns/` are also lazy and additive.
//! v0.6 execution directories are lazy and additive.
//! v0.7 `lifecycle_runs/` is lazy and additive.
//! v0.9 adds `store.json` manifest, exclusive process lock, durable
//! writes with unique temps + fsync, observation key indexes, and
//! corruption isolation for core history directories.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Process-local refcounts for store locks.
///
/// Multiple `LocalStore` handles in the same process may share one exclusive
/// lock (refcounted). Cross-process access remains exclusive via the lock file.
fn process_lock_table() -> &'static Mutex<HashMap<PathBuf, usize>> {
    static TABLE: OnceLock<Mutex<HashMap<PathBuf, usize>>> = OnceLock::new();
    TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

use crate::domain::{
    AssistedWorkflow, CapabilityLifecycleRun, CapabilityLifecycleRunListing, DeploymentReadiness,
    EngineeringReport, Evaluation, ExecutionApproval, ExecutionApprovalListing, ExecutionAttempt,
    ExecutionAttemptListing, ExecutionPlan, ExecutionPlanListing, ExecutionReceipt,
    ExecutionReceiptListing, ExecutionStorageDiagnostic, ExecutionVerification,
    ExecutionVerificationListing, Hypothesis, ImplementationListing, ImplementationRecord,
    ImplementationStorageDiagnostic, ImprovementProposal, Investigation, InvestigationId,
    InvestigationRelationship, KnowledgeObject, LearningOutcome, LearningPattern,
    LifecycleStorageDiagnostic, MeasuredLearningOutcome, MeasuredOutcomeListing,
    MeasuredOutcomeStorageDiagnostic, MemoryRecord, ObjectId, Observation, ProposalArtifact,
    ProposalArtifactListing, ProposalListing, RecalledContext, Recommendation, RiskForecast,
    RootCauseGuidance, StoreHealthReport, StoreManifest, StoreRecordDiagnostic, TimelineEntry,
    VerificationReceipt, VerificationSuggestion, STALE_LOCK_SECS, STORE_SCHEMA_VERSION,
    STORE_SCHEMA_VERSION_MAX, SUPPORTED_PRIOR_STORE_VERSIONS,
};
use crate::error::{RivoraError, RivoraResult};

use super::Store;

/// Exclusive process lock for a LocalStore root.
#[derive(Debug)]
struct StoreLock {
    path: PathBuf,
    /// When true, this handle only decremented the in-process refcount
    /// (another handle already owns the lock file).
    shared_in_process: bool,
}

impl Drop for StoreLock {
    fn drop(&mut self) {
        let mut table = process_lock_table()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let count = table.get(&self.path).copied().unwrap_or(0);
        if count > 1 {
            table.insert(self.path.clone(), count - 1);
            return;
        }
        table.remove(&self.path);
        // Only the last in-process holder removes the lock file.
        if !self.shared_in_process || count == 1 {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Local directory-based store.
#[derive(Debug)]
pub struct LocalStore {
    root: PathBuf,
    /// Held exclusive lock (released on drop).
    _lock: Option<StoreLock>,
}

impl LocalStore {
    /// Open or create a store at `root`, acquiring an exclusive process lock.
    pub fn open(root: impl Into<PathBuf>) -> RivoraResult<Self> {
        Self::open_with_options(root, true)
    }

    /// Open without acquiring the exclusive lock (read-only diagnostics / tests).
    ///
    /// Concurrent use with a locked writer is unsupported for mutations; this
    /// path exists for health inspection and stale-lock recovery helpers.
    pub fn open_unlocked(root: impl Into<PathBuf>) -> RivoraResult<Self> {
        Self::open_with_options(root, false)
    }

    fn open_with_options(root: impl Into<PathBuf>, acquire_lock: bool) -> RivoraResult<Self> {
        let root = root.into();
        fs::create_dir_all(root.join("investigations"))
            .map_err(|e| RivoraError::storage(format!("failed to create store root: {e}")))?;
        let lock = if acquire_lock {
            Some(Self::acquire_lock(&root)?)
        } else {
            None
        };
        let store = Self { root, _lock: lock };
        store.ensure_manifest()?;
        store.cleanup_orphan_temps()?;
        Ok(store)
    }

    /// Store root path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Whether this instance holds the exclusive store lock.
    pub fn lock_held(&self) -> bool {
        self._lock.is_some()
    }

    /// Force-release a stale lock file when the holding process is gone.
    ///
    /// Safe recovery only: refuses to remove a lock owned by a live process
    /// younger than [`STALE_LOCK_SECS`].
    pub fn recover_stale_lock(root: impl AsRef<Path>) -> RivoraResult<bool> {
        let lock_path = root.as_ref().join(".rivora.lock");
        if !lock_path.exists() {
            return Ok(false);
        }
        let content = fs::read_to_string(&lock_path)
            .map_err(|e| RivoraError::storage(format!("failed to read lock file: {e}")))?;
        let (pid, created_at) = parse_lock_contents(&content)?;
        if process_alive(pid) && !lock_is_stale(created_at) {
            return Err(RivoraError::store_locked(format!(
                "store lock held by live process {pid} (not stale)"
            )));
        }
        fs::remove_file(&lock_path)
            .map_err(|e| RivoraError::storage(format!("failed to remove stale lock: {e}")))?;
        Ok(true)
    }

    fn acquire_lock(root: &Path) -> RivoraResult<StoreLock> {
        let lock_path = root.join(".rivora.lock");
        let key = root
            .canonicalize()
            .unwrap_or_else(|_| root.to_path_buf())
            .join(".rivora.lock");

        // Same-process re-entrant lock: share one exclusive lock file.
        {
            let mut table = process_lock_table()
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(count) = table.get_mut(&key) {
                *count += 1;
                return Ok(StoreLock {
                    path: key,
                    shared_in_process: true,
                });
            }
        }

        if lock_path.exists() {
            let content = fs::read_to_string(&lock_path).unwrap_or_default();
            if let Ok((pid, created_at)) = parse_lock_contents(&content) {
                // Another live process holds the lock.
                if pid != std::process::id() && process_alive(pid) && !lock_is_stale(created_at) {
                    return Err(RivoraError::store_locked(format!(
                        "store already locked by process {pid} at {}",
                        lock_path.display()
                    )));
                }
                // Same process without table entry (e.g. after panic) or stale/dead — reclaim.
                let _ = fs::remove_file(&lock_path);
            } else {
                // Unparseable lock — reclaim if stale mtime.
                if let Ok(meta) = fs::metadata(&lock_path) {
                    if let Ok(modified) = meta.modified() {
                        if let Ok(age) = SystemTime::now().duration_since(modified) {
                            if age.as_secs() < STALE_LOCK_SECS {
                                return Err(RivoraError::store_locked(format!(
                                    "unparseable store lock at {} (not stale)",
                                    lock_path.display()
                                )));
                            }
                        }
                    }
                }
                let _ = fs::remove_file(&lock_path);
            }
        }
        let pid = std::process::id();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let body = format!("pid={pid}\ncreated_at={now}\n");
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(mut file) => {
                file.write_all(body.as_bytes())
                    .map_err(|e| RivoraError::storage(format!("failed to write lock file: {e}")))?;
                file.sync_all()
                    .map_err(|e| RivoraError::storage(format!("failed to sync lock file: {e}")))?;
                let mut table = process_lock_table()
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                table.insert(key.clone(), 1);
                Ok(StoreLock {
                    path: key,
                    shared_in_process: false,
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(
                RivoraError::store_locked(format!("store lock race at {}", lock_path.display())),
            ),
            Err(e) => Err(RivoraError::storage(format!("failed to create lock: {e}"))),
        }
    }

    fn ensure_manifest(&self) -> RivoraResult<()> {
        let path = self.root.join("store.json");
        if path.exists() {
            let manifest: StoreManifest = self.read_json(&path)?;
            if manifest.schema_version > STORE_SCHEMA_VERSION_MAX {
                return Err(RivoraError::SchemaMismatch {
                    found: manifest.schema_version,
                    supported_max: STORE_SCHEMA_VERSION_MAX,
                });
            }
            let mut updated = manifest;
            updated.last_opened_at = chrono::Utc::now().to_rfc3339();
            if updated.schema_version < STORE_SCHEMA_VERSION {
                // Additive migration: bump manifest only; directories remain lazy.
                updated.schema_version = STORE_SCHEMA_VERSION;
                updated.rivora_version = env!("CARGO_PKG_VERSION").to_string();
            }
            self.write_json(&path, &updated)?;
            return Ok(());
        }
        let manifest = StoreManifest::new_now(env!("CARGO_PKG_VERSION"));
        self.write_json(&path, &manifest)
    }

    fn cleanup_orphan_temps(&self) -> RivoraResult<()> {
        // Best-effort: remove leftover *.tmp files under the store root.
        let mut stack = vec![self.root.clone()];
        while let Some(dir) = stack.pop() {
            let entries = match fs::read_dir(&dir) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.ends_with(".tmp") || n.contains(".tmp."))
                    .unwrap_or(false)
                {
                    let _ = fs::remove_file(&path);
                }
            }
        }
        Ok(())
    }

    /// Compute a store health report (isolates corrupt records).
    pub fn health_report(&self) -> RivoraResult<StoreHealthReport> {
        let mut corrupt = Vec::new();
        let mut observation_count = 0u64;
        let mut memory_count = 0u64;
        let mut lifecycle_run_count = 0u64;
        let ids = self.list_investigations()?;
        for id in &ids {
            let (obs, obs_diag) =
                self.list_json_dir_isolated::<Observation>(&self.inv_dir(id).join("observations"))?;
            observation_count += obs.len() as u64;
            for d in obs_diag {
                corrupt.push(StoreRecordDiagnostic {
                    path: d.0,
                    error: d.1,
                    kind: "observation".into(),
                });
            }
            let (mem, mem_diag) =
                self.list_json_dir_isolated::<MemoryRecord>(&self.inv_dir(id).join("memory"))?;
            memory_count += mem.len() as u64;
            for d in mem_diag {
                corrupt.push(StoreRecordDiagnostic {
                    path: d.0,
                    error: d.1,
                    kind: "memory".into(),
                });
            }
            let listing = self.list_lifecycle_runs(id)?;
            lifecycle_run_count += listing.runs.len() as u64;
            for d in listing.diagnostics {
                corrupt.push(StoreRecordDiagnostic {
                    path: d.path,
                    error: d.error,
                    kind: "lifecycle_run".into(),
                });
            }
        }
        let relationship_count = self.list_relationships()?.len() as u64;
        let learning_pattern_count = self.list_learning_patterns()?.len() as u64;
        let schema_version = if self.root.join("store.json").exists() {
            self.read_json::<StoreManifest>(&self.root.join("store.json"))?
                .schema_version
        } else {
            STORE_SCHEMA_VERSION
        };
        let disk_bytes = dir_size(&self.root).unwrap_or(0);
        let lock_path = self.root.join(".rivora.lock");
        Ok(StoreHealthReport {
            root: self.root.display().to_string(),
            schema_version,
            lock_held: self.lock_held(),
            lock_path: if lock_path.exists() {
                Some(lock_path.display().to_string())
            } else {
                None
            },
            investigation_count: ids.len() as u64,
            observation_count,
            memory_count,
            lifecycle_run_count,
            relationship_count,
            learning_pattern_count,
            disk_bytes,
            corrupt_records: corrupt,
            orphan_temp_files: Vec::new(),
            notes: vec![
                "Local-only diagnostics; no remote telemetry.".into(),
                "Corrupt records are isolated where possible; healthy siblings remain readable."
                    .into(),
            ],
            migration_status: if schema_version <= STORE_SCHEMA_VERSION_MAX {
                "compatible".into()
            } else {
                "incompatible".into()
            },
            supported_prior_versions: SUPPORTED_PRIOR_STORE_VERSIONS
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
        })
    }

    /// Sanitized diagnostic export as JSON value (no secrets).
    pub fn diagnostic_export(&self) -> RivoraResult<serde_json::Value> {
        let health = self.health_report()?;
        Ok(serde_json::json!({
            "schema_version": 1,
            "rivora_version": env!("CARGO_PKG_VERSION"),
            "health": health,
            "operating_envelope": crate::domain::OperatingEnvelope::medium(),
            "replay_contracts": crate::domain::ReplayContract::v0_9_contracts(),
            "performance_budgets": crate::domain::PerformanceBudget::v0_9_budgets(),
        }))
    }

    /// Create a simple directory backup of the store (copy tree).
    pub fn backup_to(&self, dest: impl AsRef<Path>) -> RivoraResult<()> {
        let dest = dest.as_ref();
        if dest.exists() {
            return Err(RivoraError::validation(format!(
                "backup destination already exists: {}",
                dest.display()
            )));
        }
        copy_dir_excluding_lock(&self.root, dest)?;
        Ok(())
    }

    /// Rebuild observation idempotency indexes from canonical records.
    pub fn rebuild_observation_indexes(&self) -> RivoraResult<u64> {
        let mut rebuilt = 0u64;
        for id in self.list_investigations()? {
            let obs = self.list_observations(&id)?;
            let index_dir = self.inv_dir(&id).join("indexes").join("observation_keys");
            if index_dir.exists() {
                let _ = fs::remove_dir_all(&index_dir);
            }
            fs::create_dir_all(&index_dir)
                .map_err(|e| RivoraError::storage(format!("failed to create index dir: {e}")))?;
            for o in obs {
                if let Some(key) = o.idempotency_key.as_deref() {
                    let path = index_dir.join(format!("{}.json", stable_key_hash(key)));
                    let body = serde_json::json!({
                        "object_id": o.id.to_string(),
                        "idempotency_key": key,
                    });
                    self.write_json(&path, &body)?;
                    rebuilt += 1;
                }
            }
        }
        Ok(rebuilt)
    }

    fn inv_dir(&self, id: &InvestigationId) -> PathBuf {
        self.root.join("investigations").join(id.to_string())
    }

    fn ensure_inv_dirs(&self, id: &InvestigationId) -> RivoraResult<PathBuf> {
        let dir = self.inv_dir(id);
        // Only create single-level object directories here. Nested
        // `assistance/*` paths are created lazily by write_json so
        // stores with v0.1/v0.2 layouts keep a flat investigation tree
        // until assistance objects are written.
        for sub in [
            "observations",
            "memory",
            "knowledge",
            "evaluations",
            "verifications",
            "recommendations",
            "learning",
            "context",
            "workflows",
            "hypotheses",
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
        // Unique temp name avoids concurrent same-path writers clobbering
        // a shared `.json.tmp` file. sync_all before rename improves durability.
        let tmp = path.with_extension(format!("{}.tmp", ObjectId::new()));
        {
            let mut file = fs::File::create(&tmp)
                .map_err(|e| RivoraError::storage(format!("failed to create temp file: {e}")))?;
            file.write_all(&data)
                .map_err(|e| RivoraError::storage(format!("failed to write temp file: {e}")))?;
            file.sync_all()
                .map_err(|e| RivoraError::storage(format!("failed to sync temp file: {e}")))?;
        }
        fs::rename(&tmp, path).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            RivoraError::storage(format!("failed to finalize write: {e}"))
        })?;
        Ok(())
    }

    fn write_json_new<T: serde::Serialize>(&self, path: &Path, value: &T) -> RivoraResult<bool> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| RivoraError::storage(format!("failed to create parent dir: {e}")))?;
        }
        let data = serde_json::to_vec_pretty(value)
            .map_err(|e| RivoraError::serialization(e.to_string()))?;
        let tmp = path.with_extension(format!("{}.tmp", ObjectId::new()));
        {
            let mut file = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&tmp)
                .map_err(|e| RivoraError::storage(format!("failed to create temp file: {e}")))?;
            file.write_all(&data)
                .map_err(|e| RivoraError::storage(format!("failed to write temp file: {e}")))?;
            file.sync_all()
                .map_err(|e| RivoraError::storage(format!("failed to sync temp file: {e}")))?;
        }
        // Prefer exclusive create via hard_link; fall back to create_new + rename
        // when hard links are unavailable (e.g. some network filesystems).
        let linked = match fs::hard_link(&tmp, path) {
            Ok(()) => true,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => false,
            Err(_) => {
                match fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                {
                    Ok(mut file) => {
                        if let Err(e) = file.write_all(&data) {
                            let _ = fs::remove_file(path);
                            let _ = fs::remove_file(&tmp);
                            return Err(RivoraError::storage(format!(
                                "failed to atomically append {}: {e}",
                                path.display()
                            )));
                        }
                        let _ = file.sync_all();
                        true
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => false,
                    Err(error) => {
                        let _ = fs::remove_file(&tmp);
                        return Err(RivoraError::storage(format!(
                            "failed to atomically append {}: {error}",
                            path.display()
                        )));
                    }
                }
            }
        };
        let _ = fs::remove_file(&tmp);
        Ok(linked)
    }

    fn read_json<T: serde::de::DeserializeOwned>(&self, path: &Path) -> RivoraResult<T> {
        let data = fs::read(path)
            .map_err(|e| RivoraError::storage(format!("failed to read {}: {e}", path.display())))?;
        serde_json::from_slice(&data).map_err(|e| RivoraError::serialization(e.to_string()))
    }

    fn list_json_dir<T: serde::de::DeserializeOwned>(&self, dir: &Path) -> RivoraResult<Vec<T>> {
        let (items, _diag) = self.list_json_dir_isolated(dir)?;
        Ok(items)
    }

    /// List JSON objects, isolating corrupt files instead of failing the whole directory.
    #[allow(clippy::type_complexity)]
    fn list_json_dir_isolated<T: serde::de::DeserializeOwned>(
        &self,
        dir: &Path,
    ) -> RivoraResult<(Vec<T>, Vec<(String, String)>)> {
        if !dir.exists() {
            return Ok((Vec::new(), Vec::new()));
        }
        let mut items = Vec::new();
        let mut diagnostics = Vec::new();
        let entries = fs::read_dir(dir).map_err(|e| {
            RivoraError::storage(format!("failed to read dir {}: {e}", dir.display()))
        })?;
        for entry in entries {
            let entry = entry
                .map_err(|e| RivoraError::storage(format!("failed to read dir entry: {e}")))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            // Skip temp files that might still be present.
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.contains(".tmp"))
                .unwrap_or(false)
            {
                continue;
            }
            match self.read_json::<T>(&path) {
                Ok(item) => items.push(item),
                Err(error) => diagnostics.push((path.display().to_string(), error.to_string())),
            }
        }
        Ok((items, diagnostics))
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

    fn proposals_dir(&self, id: &InvestigationId) -> PathBuf {
        self.inv_dir(id).join("proposals")
    }

    fn proposal_path(&self, investigation_id: &InvestigationId, id: &ObjectId) -> PathBuf {
        self.proposals_dir(investigation_id)
            .join(format!("{id}.json"))
    }

    fn proposal_artifacts_dir(&self, id: &InvestigationId) -> PathBuf {
        self.inv_dir(id).join("proposal_artifacts")
    }

    fn proposal_artifact_path(&self, investigation_id: &InvestigationId, id: &ObjectId) -> PathBuf {
        self.proposal_artifacts_dir(investigation_id)
            .join(format!("{id}.json"))
    }

    fn implementations_dir(&self, id: &InvestigationId) -> PathBuf {
        self.inv_dir(id).join("implementations")
    }

    fn implementation_path(&self, investigation_id: &InvestigationId, id: &ObjectId) -> PathBuf {
        self.implementations_dir(investigation_id)
            .join(format!("{id}.json"))
    }

    fn learning_outcomes_dir(&self, id: &InvestigationId) -> PathBuf {
        self.inv_dir(id).join("learning_outcomes")
    }

    fn measured_outcome_path(&self, investigation_id: &InvestigationId, id: &ObjectId) -> PathBuf {
        self.learning_outcomes_dir(investigation_id)
            .join(format!("{id}.json"))
    }

    fn learning_patterns_dir(&self) -> PathBuf {
        self.root.join("learning").join("patterns")
    }

    fn learning_pattern_path(&self, id: &ObjectId) -> PathBuf {
        self.learning_patterns_dir().join(format!("{id}.json"))
    }

    fn execution_plans_dir(&self, id: &InvestigationId) -> PathBuf {
        self.inv_dir(id).join("execution_plans")
    }

    fn execution_plan_path(&self, investigation_id: &InvestigationId, id: &ObjectId) -> PathBuf {
        self.execution_plans_dir(investigation_id)
            .join(format!("{id}.json"))
    }

    fn execution_approvals_dir(&self, id: &InvestigationId) -> PathBuf {
        self.inv_dir(id).join("execution_approvals")
    }

    fn execution_approval_path(
        &self,
        investigation_id: &InvestigationId,
        id: &ObjectId,
    ) -> PathBuf {
        self.execution_approvals_dir(investigation_id)
            .join(format!("{id}.json"))
    }

    fn execution_attempts_dir(&self, id: &InvestigationId) -> PathBuf {
        self.inv_dir(id).join("execution_attempts")
    }

    fn execution_attempt_path(&self, investigation_id: &InvestigationId, id: &ObjectId) -> PathBuf {
        self.execution_attempts_dir(investigation_id)
            .join(format!("{id}.json"))
    }

    fn execution_receipts_dir(&self, id: &InvestigationId) -> PathBuf {
        self.inv_dir(id).join("execution_receipts")
    }

    fn execution_receipt_path(&self, investigation_id: &InvestigationId, id: &ObjectId) -> PathBuf {
        self.execution_receipts_dir(investigation_id)
            .join(format!("{id}.json"))
    }

    fn execution_verifications_dir(&self, id: &InvestigationId) -> PathBuf {
        self.inv_dir(id).join("execution_verifications")
    }

    fn execution_verification_path(
        &self,
        investigation_id: &InvestigationId,
        id: &ObjectId,
    ) -> PathBuf {
        self.execution_verifications_dir(investigation_id)
            .join(format!("{id}.json"))
    }

    fn list_execution_plans_isolated(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<ExecutionPlanListing> {
        let dir = self.execution_plans_dir(id);
        if !dir.exists() {
            return Ok(ExecutionPlanListing::default());
        }
        let entries = fs::read_dir(&dir).map_err(|e| {
            RivoraError::storage(format!("failed to read dir {}: {e}", dir.display()))
        })?;
        let mut listing = ExecutionPlanListing::default();
        for entry in entries {
            let entry = entry
                .map_err(|e| RivoraError::storage(format!("failed to read dir entry: {e}")))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            match self.read_json::<ExecutionPlan>(&path) {
                Ok(plan) if plan.investigation_id == *id => listing.plans.push(plan),
                Ok(_) => listing.diagnostics.push(ExecutionStorageDiagnostic {
                    path: path.display().to_string(),
                    error: "execution plan investigation ownership mismatch".into(),
                }),
                Err(error) => listing.diagnostics.push(ExecutionStorageDiagnostic {
                    path: path.display().to_string(),
                    error: error.to_string(),
                }),
            }
        }
        listing.plans.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then_with(|| a.revision_number.cmp(&b.revision_number))
                .then_with(|| a.id.to_string().cmp(&b.id.to_string()))
        });
        listing.diagnostics.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(listing)
    }

    fn list_execution_attempts_isolated(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<ExecutionAttemptListing> {
        let dir = self.execution_attempts_dir(id);
        if !dir.exists() {
            return Ok(ExecutionAttemptListing::default());
        }
        let entries = fs::read_dir(&dir).map_err(|e| {
            RivoraError::storage(format!("failed to read dir {}: {e}", dir.display()))
        })?;
        let mut listing = ExecutionAttemptListing::default();
        for entry in entries {
            let entry = entry
                .map_err(|e| RivoraError::storage(format!("failed to read dir entry: {e}")))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            match self.read_json::<ExecutionAttempt>(&path) {
                Ok(attempt) if attempt.investigation_id == *id => listing.attempts.push(attempt),
                Ok(_) => listing.diagnostics.push(ExecutionStorageDiagnostic {
                    path: path.display().to_string(),
                    error: "execution attempt investigation ownership mismatch".into(),
                }),
                Err(error) => listing.diagnostics.push(ExecutionStorageDiagnostic {
                    path: path.display().to_string(),
                    error: error.to_string(),
                }),
            }
        }
        listing.attempts.sort_by(|a, b| {
            a.started_at
                .cmp(&b.started_at)
                .then_with(|| a.id.to_string().cmp(&b.id.to_string()))
        });
        listing.diagnostics.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(listing)
    }

    fn list_execution_receipts_isolated(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<ExecutionReceiptListing> {
        let dir = self.execution_receipts_dir(id);
        if !dir.exists() {
            return Ok(ExecutionReceiptListing::default());
        }
        let entries = fs::read_dir(&dir).map_err(|e| {
            RivoraError::storage(format!("failed to read dir {}: {e}", dir.display()))
        })?;
        let mut listing = ExecutionReceiptListing::default();
        for entry in entries {
            let entry = entry
                .map_err(|e| RivoraError::storage(format!("failed to read dir entry: {e}")))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            match self.read_json::<ExecutionReceipt>(&path) {
                Ok(receipt) if receipt.investigation_id == *id => listing.receipts.push(receipt),
                Ok(_) => listing.diagnostics.push(ExecutionStorageDiagnostic {
                    path: path.display().to_string(),
                    error: "execution receipt investigation ownership mismatch".into(),
                }),
                Err(error) => listing.diagnostics.push(ExecutionStorageDiagnostic {
                    path: path.display().to_string(),
                    error: error.to_string(),
                }),
            }
        }
        listing.receipts.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then_with(|| a.id.to_string().cmp(&b.id.to_string()))
        });
        listing.diagnostics.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(listing)
    }

    fn list_proposals_isolated(&self, id: &InvestigationId) -> RivoraResult<ProposalListing> {
        use crate::domain::ProposalStorageDiagnostic;

        let dir = self.proposals_dir(id);
        if !dir.exists() {
            return Ok(ProposalListing::default());
        }
        let entries = fs::read_dir(&dir).map_err(|e| {
            RivoraError::storage(format!("failed to read dir {}: {e}", dir.display()))
        })?;
        let mut listing = ProposalListing::default();
        for entry in entries {
            let entry = entry
                .map_err(|e| RivoraError::storage(format!("failed to read dir entry: {e}")))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            match self.read_json::<ImprovementProposal>(&path) {
                Ok(proposal) if proposal.investigation_id == *id => {
                    listing.proposals.push(proposal)
                }
                Ok(_) => listing.diagnostics.push(ProposalStorageDiagnostic {
                    path: path.display().to_string(),
                    error: "proposal investigation ownership mismatch".into(),
                }),
                Err(error) => listing.diagnostics.push(ProposalStorageDiagnostic {
                    path: path.display().to_string(),
                    error: error.to_string(),
                }),
            }
        }
        listing.proposals.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then_with(|| a.revision_number.cmp(&b.revision_number))
                .then_with(|| a.id.to_string().cmp(&b.id.to_string()))
        });
        listing.diagnostics.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(listing)
    }

    fn list_implementations_isolated(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<ImplementationListing> {
        let dir = self.implementations_dir(id);
        if !dir.exists() {
            return Ok(ImplementationListing::default());
        }
        let entries = fs::read_dir(&dir).map_err(|e| {
            RivoraError::storage(format!("failed to read dir {}: {e}", dir.display()))
        })?;
        let mut listing = ImplementationListing::default();
        for entry in entries {
            let entry = entry
                .map_err(|e| RivoraError::storage(format!("failed to read dir entry: {e}")))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            match self.read_json::<ImplementationRecord>(&path) {
                Ok(record) if record.investigation_id == *id => listing.records.push(record),
                Ok(_) => listing.diagnostics.push(ImplementationStorageDiagnostic {
                    path: path.display().to_string(),
                    error: "implementation record investigation ownership mismatch".into(),
                }),
                Err(error) => listing.diagnostics.push(ImplementationStorageDiagnostic {
                    path: path.display().to_string(),
                    error: error.to_string(),
                }),
            }
        }
        listing.records.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then_with(|| a.revision_number.cmp(&b.revision_number))
                .then_with(|| a.id.to_string().cmp(&b.id.to_string()))
        });
        listing.diagnostics.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(listing)
    }

    fn list_measured_outcomes_isolated(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<MeasuredOutcomeListing> {
        let dir = self.learning_outcomes_dir(id);
        if !dir.exists() {
            return Ok(MeasuredOutcomeListing::default());
        }
        let entries = fs::read_dir(&dir).map_err(|e| {
            RivoraError::storage(format!("failed to read dir {}: {e}", dir.display()))
        })?;
        let mut listing = MeasuredOutcomeListing::default();
        for entry in entries {
            let entry = entry
                .map_err(|e| RivoraError::storage(format!("failed to read dir entry: {e}")))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            match self.read_json::<MeasuredLearningOutcome>(&path) {
                Ok(outcome) if outcome.investigation_id == *id => listing.outcomes.push(outcome),
                Ok(_) => listing.diagnostics.push(MeasuredOutcomeStorageDiagnostic {
                    path: path.display().to_string(),
                    error: "measured learning outcome investigation ownership mismatch".into(),
                }),
                Err(error) => listing.diagnostics.push(MeasuredOutcomeStorageDiagnostic {
                    path: path.display().to_string(),
                    error: error.to_string(),
                }),
            }
        }
        listing.outcomes.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then_with(|| a.revision_number.cmp(&b.revision_number))
                .then_with(|| a.id.to_string().cmp(&b.id.to_string()))
        });
        listing.diagnostics.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(listing)
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
        // Claim idempotency key first so concurrent same-key ingests cannot double-write.
        if let Some(key) = observation.idempotency_key.as_deref() {
            let claimed =
                self.claim_observation_key(&observation.investigation_id, key, &observation.id)?;
            if !claimed {
                return Err(RivoraError::conflict(format!(
                    "observation idempotency key already claimed: {key}"
                )));
            }
        }
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
        // Use exclusive create for append-only observation bodies.
        if !self.write_json_new(&path, observation)? {
            return Err(RivoraError::storage(format!(
                "observation {} already exists",
                observation.id
            )));
        }
        Ok(())
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
        // Prefer durable key index; fall back to scan for pre-v0.9 stores.
        let index_path = self
            .inv_dir(investigation_id)
            .join("indexes")
            .join("observation_keys")
            .join(format!("{}.json", stable_key_hash(key)));
        if index_path.exists() {
            if let Ok(value) = self.read_json::<serde_json::Value>(&index_path) {
                if let Some(id_str) = value.get("object_id").and_then(|v| v.as_str()) {
                    if let Ok(object_id) = id_str.parse::<ObjectId>() {
                        let path = self.object_path(investigation_id, "observations", &object_id);
                        if path.exists() {
                            return Ok(Some(self.read_json(&path)?));
                        }
                    }
                }
            }
        }
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
        if !self.write_json_new(&path, record)? {
            return Err(RivoraError::storage(format!(
                "memory record {} already exists (append-only)",
                record.id
            )));
        }
        Ok(())
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

    fn save_recalled_context(&self, context: &RecalledContext) -> RivoraResult<()> {
        self.ensure_inv_dirs(&context.investigation_id)?;
        let path = self.object_path(&context.investigation_id, "context", &context.id);
        self.write_json(&path, context)
    }

    fn load_recalled_context(
        &self,
        investigation_id: &InvestigationId,
        id: &ObjectId,
    ) -> RivoraResult<RecalledContext> {
        let path = self.object_path(investigation_id, "context", id);
        if !path.exists() {
            return Err(RivoraError::ObjectNotFound(*id));
        }
        self.read_json(&path)
    }

    fn list_recalled_context(&self, id: &InvestigationId) -> RivoraResult<Vec<RecalledContext>> {
        let mut items: Vec<RecalledContext> =
            self.list_json_dir(&self.inv_dir(id).join("context"))?;
        items.sort_by_key(|c| c.recalled_at);
        Ok(items)
    }

    fn save_workflow(&self, workflow: &AssistedWorkflow) -> RivoraResult<()> {
        self.ensure_inv_dirs(&workflow.investigation_id)?;
        let path = self.object_path(&workflow.investigation_id, "workflows", &workflow.id);
        self.write_json(&path, workflow)
    }

    fn load_workflow(
        &self,
        investigation_id: &InvestigationId,
        id: &ObjectId,
    ) -> RivoraResult<AssistedWorkflow> {
        let path = self.object_path(investigation_id, "workflows", id);
        if !path.exists() {
            return Err(RivoraError::ObjectNotFound(*id));
        }
        self.read_json(&path)
    }

    fn list_workflows(&self, id: &InvestigationId) -> RivoraResult<Vec<AssistedWorkflow>> {
        let mut items: Vec<AssistedWorkflow> =
            self.list_json_dir(&self.inv_dir(id).join("workflows"))?;
        items.sort_by_key(|w| w.planned_at);
        Ok(items)
    }

    fn append_hypothesis(&self, hypothesis: &Hypothesis) -> RivoraResult<()> {
        self.ensure_inv_dirs(&hypothesis.investigation_id)?;
        let path = self.object_path(&hypothesis.investigation_id, "hypotheses", &hypothesis.id);
        if path.exists() {
            return Err(RivoraError::storage(format!(
                "hypothesis {} already exists",
                hypothesis.id
            )));
        }
        self.write_json(&path, hypothesis)
    }

    fn list_hypotheses(&self, id: &InvestigationId) -> RivoraResult<Vec<Hypothesis>> {
        let mut items: Vec<Hypothesis> =
            self.list_json_dir(&self.inv_dir(id).join("hypotheses"))?;
        items.sort_by(|a, b| {
            a.rank
                .cmp(&b.rank)
                .then_with(|| a.generated_at.cmp(&b.generated_at))
        });
        Ok(items)
    }

    fn append_verification_suggestion(
        &self,
        suggestion: &VerificationSuggestion,
    ) -> RivoraResult<()> {
        self.ensure_inv_dirs(&suggestion.investigation_id)?;
        let path = self.object_path(
            &suggestion.investigation_id,
            "assistance/verification_suggestions",
            &suggestion.id,
        );
        if path.exists() {
            return Err(RivoraError::storage(format!(
                "verification suggestion {} already exists",
                suggestion.id
            )));
        }
        self.write_json(&path, suggestion)
    }

    fn list_verification_suggestions(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<Vec<VerificationSuggestion>> {
        let mut items: Vec<VerificationSuggestion> =
            self.list_json_dir(&self.inv_dir(id).join("assistance/verification_suggestions"))?;
        items.sort_by(|a, b| {
            a.rank
                .cmp(&b.rank)
                .then_with(|| a.generated_at.cmp(&b.generated_at))
        });
        Ok(items)
    }

    fn append_deployment_readiness(&self, readiness: &DeploymentReadiness) -> RivoraResult<()> {
        self.ensure_inv_dirs(&readiness.investigation_id)?;
        let path = self.object_path(
            &readiness.investigation_id,
            "assistance/readiness",
            &readiness.id,
        );
        if path.exists() {
            return Err(RivoraError::storage(format!(
                "deployment readiness {} already exists",
                readiness.id
            )));
        }
        self.write_json(&path, readiness)
    }

    fn list_deployment_readiness(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<Vec<DeploymentReadiness>> {
        let mut items: Vec<DeploymentReadiness> =
            self.list_json_dir(&self.inv_dir(id).join("assistance/readiness"))?;
        items.sort_by_key(|r| r.assessed_at);
        Ok(items)
    }

    fn append_risk_forecast(&self, forecast: &RiskForecast) -> RivoraResult<()> {
        self.ensure_inv_dirs(&forecast.investigation_id)?;
        let path = self.object_path(&forecast.investigation_id, "assistance/risks", &forecast.id);
        if path.exists() {
            return Err(RivoraError::storage(format!(
                "risk forecast {} already exists",
                forecast.id
            )));
        }
        self.write_json(&path, forecast)
    }

    fn list_risk_forecasts(&self, id: &InvestigationId) -> RivoraResult<Vec<RiskForecast>> {
        let mut items: Vec<RiskForecast> =
            self.list_json_dir(&self.inv_dir(id).join("assistance/risks"))?;
        items.sort_by_key(|r| r.forecasted_at);
        Ok(items)
    }

    fn append_root_cause_guidance(&self, guidance: &RootCauseGuidance) -> RivoraResult<()> {
        self.ensure_inv_dirs(&guidance.investigation_id)?;
        let path = self.object_path(
            &guidance.investigation_id,
            "assistance/root_cause",
            &guidance.id,
        );
        if path.exists() {
            return Err(RivoraError::storage(format!(
                "root cause guidance {} already exists",
                guidance.id
            )));
        }
        self.write_json(&path, guidance)
    }

    fn list_root_cause_guidance(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<Vec<RootCauseGuidance>> {
        let mut items: Vec<RootCauseGuidance> =
            self.list_json_dir(&self.inv_dir(id).join("assistance/root_cause"))?;
        items.sort_by_key(|g| g.generated_at);
        Ok(items)
    }

    fn append_engineering_report(&self, report: &EngineeringReport) -> RivoraResult<()> {
        self.ensure_inv_dirs(&report.investigation_id)?;
        let path = self.object_path(&report.investigation_id, "assistance/reports", &report.id);
        if path.exists() {
            return Err(RivoraError::storage(format!(
                "engineering report {} already exists",
                report.id
            )));
        }
        self.write_json(&path, report)
    }

    fn list_engineering_reports(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<Vec<EngineeringReport>> {
        let mut items: Vec<EngineeringReport> =
            self.list_json_dir(&self.inv_dir(id).join("assistance/reports"))?;
        items.sort_by_key(|r| r.generated_at);
        Ok(items)
    }

    fn append_proposal(&self, proposal: &ImprovementProposal) -> RivoraResult<()> {
        let path = self.proposal_path(&proposal.investigation_id, &proposal.id);
        if path.exists() {
            return Err(RivoraError::storage(format!(
                "proposal snapshot {} already exists (immutable)",
                proposal.id
            )));
        }
        self.write_json(&path, proposal)
    }

    fn load_proposal(
        &self,
        investigation_id: &InvestigationId,
        id: &ObjectId,
    ) -> RivoraResult<ImprovementProposal> {
        let path = self.proposal_path(investigation_id, id);
        if !path.exists() {
            return Err(RivoraError::ObjectNotFound(*id));
        }
        let proposal: ImprovementProposal = self.read_json(&path)?;
        if proposal.investigation_id != *investigation_id {
            return Err(RivoraError::validation(
                "proposal investigation ownership mismatch",
            ));
        }
        Ok(proposal)
    }

    fn list_proposals(&self, id: &InvestigationId) -> RivoraResult<ProposalListing> {
        self.list_proposals_isolated(id)
    }

    fn list_proposal_revisions(
        &self,
        id: &InvestigationId,
        lineage_id: &ObjectId,
    ) -> RivoraResult<ProposalListing> {
        let mut listing = self.list_proposals_isolated(id)?;
        listing.proposals.retain(|p| p.lineage_id == *lineage_id);
        listing.proposals.sort_by(|a, b| {
            a.revision_number
                .cmp(&b.revision_number)
                .then_with(|| a.id.to_string().cmp(&b.id.to_string()))
        });
        Ok(listing)
    }

    fn append_proposal_artifact(&self, artifact: &ProposalArtifact) -> RivoraResult<()> {
        let path = self.proposal_artifact_path(&artifact.investigation_id, &artifact.id);
        if path.exists() {
            return Err(RivoraError::storage(format!(
                "proposal artifact {} already exists",
                artifact.id
            )));
        }
        self.write_json(&path, artifact)
    }

    fn list_proposal_artifacts(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<ProposalArtifactListing> {
        use crate::domain::ProposalStorageDiagnostic;

        let dir = self.proposal_artifacts_dir(id);
        if !dir.exists() {
            return Ok(ProposalArtifactListing::default());
        }
        let entries = fs::read_dir(&dir).map_err(|error| {
            RivoraError::storage(format!("failed to read dir {}: {error}", dir.display()))
        })?;
        let mut listing = ProposalArtifactListing::default();
        for entry in entries {
            let entry = entry.map_err(|error| {
                RivoraError::storage(format!("failed to read dir entry: {error}"))
            })?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            match self.read_json::<ProposalArtifact>(&path) {
                Ok(artifact) if artifact.investigation_id == *id => {
                    listing.artifacts.push(artifact)
                }
                Ok(_) => listing.diagnostics.push(ProposalStorageDiagnostic {
                    path: path.display().to_string(),
                    error: "proposal artifact investigation ownership mismatch".into(),
                }),
                Err(error) => listing.diagnostics.push(ProposalStorageDiagnostic {
                    path: path.display().to_string(),
                    error: error.to_string(),
                }),
            }
        }
        listing.artifacts.sort_by(|a, b| {
            a.generated_at
                .cmp(&b.generated_at)
                .then_with(|| a.id.to_string().cmp(&b.id.to_string()))
        });
        listing.diagnostics.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(listing)
    }

    fn append_implementation_record(&self, record: &ImplementationRecord) -> RivoraResult<()> {
        let path = self.implementation_path(&record.investigation_id, &record.id);
        if path.exists() {
            return Err(RivoraError::storage(format!(
                "implementation record snapshot {} already exists (immutable)",
                record.id
            )));
        }
        self.write_json(&path, record)
    }

    fn load_implementation_record(
        &self,
        investigation_id: &InvestigationId,
        id: &ObjectId,
    ) -> RivoraResult<ImplementationRecord> {
        let path = self.implementation_path(investigation_id, id);
        if !path.exists() {
            return Err(RivoraError::ObjectNotFound(*id));
        }
        let record: ImplementationRecord = self.read_json(&path)?;
        if record.investigation_id != *investigation_id {
            return Err(RivoraError::validation(
                "implementation record investigation ownership mismatch",
            ));
        }
        Ok(record)
    }

    fn list_implementation_records(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<ImplementationListing> {
        self.list_implementations_isolated(id)
    }

    fn list_implementation_revisions(
        &self,
        id: &InvestigationId,
        lineage_id: &ObjectId,
    ) -> RivoraResult<ImplementationListing> {
        let mut listing = self.list_implementations_isolated(id)?;
        listing.records.retain(|r| r.lineage_id == *lineage_id);
        listing.records.sort_by(|a, b| {
            a.revision_number
                .cmp(&b.revision_number)
                .then_with(|| a.id.to_string().cmp(&b.id.to_string()))
        });
        Ok(listing)
    }

    fn append_measured_learning_outcome(
        &self,
        outcome: &MeasuredLearningOutcome,
    ) -> RivoraResult<()> {
        let path = self.measured_outcome_path(&outcome.investigation_id, &outcome.id);
        if path.exists() {
            return Err(RivoraError::storage(format!(
                "measured learning outcome snapshot {} already exists (immutable)",
                outcome.id
            )));
        }
        self.write_json(&path, outcome)
    }

    fn load_measured_learning_outcome(
        &self,
        investigation_id: &InvestigationId,
        id: &ObjectId,
    ) -> RivoraResult<MeasuredLearningOutcome> {
        let path = self.measured_outcome_path(investigation_id, id);
        if !path.exists() {
            return Err(RivoraError::ObjectNotFound(*id));
        }
        let outcome: MeasuredLearningOutcome = self.read_json(&path)?;
        if outcome.investigation_id != *investigation_id {
            return Err(RivoraError::validation(
                "measured learning outcome investigation ownership mismatch",
            ));
        }
        Ok(outcome)
    }

    fn list_measured_learning_outcomes(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<MeasuredOutcomeListing> {
        self.list_measured_outcomes_isolated(id)
    }

    fn list_measured_outcome_revisions(
        &self,
        id: &InvestigationId,
        lineage_id: &ObjectId,
    ) -> RivoraResult<MeasuredOutcomeListing> {
        let mut listing = self.list_measured_outcomes_isolated(id)?;
        listing.outcomes.retain(|o| o.lineage_id == *lineage_id);
        listing.outcomes.sort_by(|a, b| {
            a.revision_number
                .cmp(&b.revision_number)
                .then_with(|| a.id.to_string().cmp(&b.id.to_string()))
        });
        Ok(listing)
    }

    fn append_learning_pattern(&self, pattern: &LearningPattern) -> RivoraResult<()> {
        let path = self.learning_pattern_path(&pattern.id);
        if path.exists() {
            return Err(RivoraError::storage(format!(
                "learning pattern {} already exists (immutable)",
                pattern.id
            )));
        }
        self.write_json(&path, pattern)
    }

    fn load_learning_pattern(&self, id: &ObjectId) -> RivoraResult<LearningPattern> {
        let path = self.learning_pattern_path(id);
        if !path.exists() {
            return Err(RivoraError::ObjectNotFound(*id));
        }
        self.read_json(&path)
    }

    fn list_learning_patterns(&self) -> RivoraResult<Vec<LearningPattern>> {
        let dir = self.learning_patterns_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut patterns: Vec<LearningPattern> = self.list_json_dir(&dir)?;
        patterns.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then_with(|| a.id.to_string().cmp(&b.id.to_string()))
        });
        Ok(patterns)
    }

    fn append_execution_plan(&self, plan: &ExecutionPlan) -> RivoraResult<()> {
        let path = self.execution_plan_path(&plan.investigation_id, &plan.id);
        if path.exists() {
            return Err(RivoraError::storage(format!(
                "execution plan snapshot {} already exists (immutable)",
                plan.id
            )));
        }
        self.write_json(&path, plan)
    }

    fn load_execution_plan(
        &self,
        investigation_id: &InvestigationId,
        id: &ObjectId,
    ) -> RivoraResult<ExecutionPlan> {
        let path = self.execution_plan_path(investigation_id, id);
        if !path.exists() {
            return Err(RivoraError::ObjectNotFound(*id));
        }
        let plan: ExecutionPlan = self.read_json(&path)?;
        if plan.investigation_id != *investigation_id {
            return Err(RivoraError::validation(
                "execution plan investigation ownership mismatch",
            ));
        }
        Ok(plan)
    }

    fn list_execution_plans(&self, id: &InvestigationId) -> RivoraResult<ExecutionPlanListing> {
        self.list_execution_plans_isolated(id)
    }

    fn list_execution_plan_revisions(
        &self,
        id: &InvestigationId,
        lineage_id: &ObjectId,
    ) -> RivoraResult<ExecutionPlanListing> {
        let mut listing = self.list_execution_plans_isolated(id)?;
        listing.plans.retain(|p| p.lineage_id == *lineage_id);
        listing.plans.sort_by(|a, b| {
            a.revision_number
                .cmp(&b.revision_number)
                .then_with(|| a.id.to_string().cmp(&b.id.to_string()))
        });
        Ok(listing)
    }

    fn save_execution_approval(&self, approval: &ExecutionApproval) -> RivoraResult<()> {
        let path = self.execution_approval_path(&approval.investigation_id, &approval.id);
        self.write_json(&path, approval)
    }

    fn load_execution_approval(
        &self,
        investigation_id: &InvestigationId,
        id: &ObjectId,
    ) -> RivoraResult<ExecutionApproval> {
        let path = self.execution_approval_path(investigation_id, id);
        if !path.exists() {
            return Err(RivoraError::ObjectNotFound(*id));
        }
        let approval: ExecutionApproval = self.read_json(&path)?;
        if approval.investigation_id != *investigation_id {
            return Err(RivoraError::validation(
                "execution approval investigation ownership mismatch",
            ));
        }
        Ok(approval)
    }

    fn list_execution_approvals(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<ExecutionApprovalListing> {
        let dir = self.execution_approvals_dir(id);
        if !dir.exists() {
            return Ok(ExecutionApprovalListing::default());
        }
        let mut listing = ExecutionApprovalListing::default();
        let entries = fs::read_dir(&dir).map_err(|e| {
            RivoraError::storage(format!("failed to read dir {}: {e}", dir.display()))
        })?;
        for entry in entries {
            let entry = entry
                .map_err(|e| RivoraError::storage(format!("failed to read dir entry: {e}")))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            match self.read_json::<ExecutionApproval>(&path) {
                Ok(approval) if approval.investigation_id == *id => {
                    listing.approvals.push(approval);
                }
                Ok(_) => listing.diagnostics.push(ExecutionStorageDiagnostic {
                    path: path.display().to_string(),
                    error: "execution approval investigation ownership mismatch".into(),
                }),
                Err(error) => listing.diagnostics.push(ExecutionStorageDiagnostic {
                    path: path.display().to_string(),
                    error: error.to_string(),
                }),
            }
        }
        listing.approvals.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then_with(|| a.id.to_string().cmp(&b.id.to_string()))
        });
        listing.diagnostics.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(listing)
    }

    fn append_execution_attempt(&self, attempt: &ExecutionAttempt) -> RivoraResult<()> {
        let path = self.execution_attempt_path(&attempt.investigation_id, &attempt.id);
        if !self.write_json_new(&path, attempt)? {
            return Err(RivoraError::storage(format!(
                "execution attempt {} already exists (immutable)",
                attempt.id
            )));
        }
        Ok(())
    }

    fn try_reserve_execution_attempt(&self, attempt: &ExecutionAttempt) -> RivoraResult<bool> {
        let path = self.execution_attempt_path(&attempt.investigation_id, &attempt.id);
        self.write_json_new(&path, attempt)
    }

    fn load_execution_attempt(
        &self,
        investigation_id: &InvestigationId,
        id: &ObjectId,
    ) -> RivoraResult<ExecutionAttempt> {
        let path = self.execution_attempt_path(investigation_id, id);
        if !path.exists() {
            return Err(RivoraError::ObjectNotFound(*id));
        }
        let attempt: ExecutionAttempt = self.read_json(&path)?;
        if attempt.investigation_id != *investigation_id {
            return Err(RivoraError::validation(
                "execution attempt investigation ownership mismatch",
            ));
        }
        Ok(attempt)
    }

    fn list_execution_attempts(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<ExecutionAttemptListing> {
        self.list_execution_attempts_isolated(id)
    }

    fn append_execution_receipt(&self, receipt: &ExecutionReceipt) -> RivoraResult<()> {
        let path = self.execution_receipt_path(&receipt.investigation_id, &receipt.id);
        if path.exists() {
            return Err(RivoraError::storage(format!(
                "execution receipt {} already exists (immutable)",
                receipt.id
            )));
        }
        self.write_json(&path, receipt)
    }

    fn load_execution_receipt(
        &self,
        investigation_id: &InvestigationId,
        id: &ObjectId,
    ) -> RivoraResult<ExecutionReceipt> {
        let path = self.execution_receipt_path(investigation_id, id);
        if !path.exists() {
            return Err(RivoraError::ObjectNotFound(*id));
        }
        let receipt: ExecutionReceipt = self.read_json(&path)?;
        if receipt.investigation_id != *investigation_id {
            return Err(RivoraError::validation(
                "execution receipt investigation ownership mismatch",
            ));
        }
        Ok(receipt)
    }

    fn list_execution_receipts(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<ExecutionReceiptListing> {
        self.list_execution_receipts_isolated(id)
    }

    fn append_execution_verification(
        &self,
        verification: &ExecutionVerification,
    ) -> RivoraResult<()> {
        let path =
            self.execution_verification_path(&verification.investigation_id, &verification.id);
        if path.exists() {
            return Err(RivoraError::storage(format!(
                "execution verification {} already exists (immutable)",
                verification.id
            )));
        }
        self.write_json(&path, verification)
    }

    fn load_execution_verification(
        &self,
        investigation_id: &InvestigationId,
        id: &ObjectId,
    ) -> RivoraResult<ExecutionVerification> {
        let path = self.execution_verification_path(investigation_id, id);
        if !path.exists() {
            return Err(RivoraError::ObjectNotFound(*id));
        }
        let verification: ExecutionVerification = self.read_json(&path)?;
        if verification.investigation_id != *investigation_id {
            return Err(RivoraError::validation(
                "execution verification investigation ownership mismatch",
            ));
        }
        Ok(verification)
    }

    fn list_execution_verifications(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<ExecutionVerificationListing> {
        let dir = self.execution_verifications_dir(id);
        if !dir.exists() {
            return Ok(ExecutionVerificationListing::default());
        }
        let mut listing = ExecutionVerificationListing::default();
        let entries = fs::read_dir(&dir).map_err(|e| {
            RivoraError::storage(format!("failed to read dir {}: {e}", dir.display()))
        })?;
        for entry in entries {
            let entry = entry
                .map_err(|e| RivoraError::storage(format!("failed to read dir entry: {e}")))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            match self.read_json::<ExecutionVerification>(&path) {
                Ok(verification) if verification.investigation_id == *id => {
                    listing.verifications.push(verification);
                }
                Ok(_) => listing.diagnostics.push(ExecutionStorageDiagnostic {
                    path: path.display().to_string(),
                    error: "execution verification investigation ownership mismatch".into(),
                }),
                Err(error) => listing.diagnostics.push(ExecutionStorageDiagnostic {
                    path: path.display().to_string(),
                    error: error.to_string(),
                }),
            }
        }
        listing.verifications.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then_with(|| a.id.to_string().cmp(&b.id.to_string()))
        });
        listing.diagnostics.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(listing)
    }

    fn try_consume_execution_approval(&self, approval: &ExecutionApproval) -> RivoraResult<bool> {
        let marker = self
            .execution_approvals_dir(&approval.investigation_id)
            .join(format!("{}.consumed", approval.id));
        self.write_json_new(&marker, &serde_json::json!({"approval_id": approval.id}))
    }

    fn append_lifecycle_run(&self, run: &CapabilityLifecycleRun) -> RivoraResult<()> {
        let path = self.lifecycle_run_path(&run.investigation_id, &run.id);
        if path.exists() {
            return Err(RivoraError::storage(format!(
                "lifecycle run snapshot {} already exists (immutable)",
                run.id
            )));
        }
        self.write_json(&path, run)
    }

    fn load_lifecycle_run(
        &self,
        investigation_id: &InvestigationId,
        id: &ObjectId,
    ) -> RivoraResult<CapabilityLifecycleRun> {
        let path = self.lifecycle_run_path(investigation_id, id);
        if !path.exists() {
            return Err(RivoraError::ObjectNotFound(*id));
        }
        let run: CapabilityLifecycleRun = self.read_json(&path)?;
        if run.investigation_id != *investigation_id {
            return Err(RivoraError::validation(
                "lifecycle run investigation ownership mismatch",
            ));
        }
        Ok(run)
    }

    fn list_lifecycle_runs(
        &self,
        id: &InvestigationId,
    ) -> RivoraResult<CapabilityLifecycleRunListing> {
        let dir = self.lifecycle_runs_dir(id);
        if !dir.exists() {
            return Ok(CapabilityLifecycleRunListing::default());
        }
        let mut listing = CapabilityLifecycleRunListing::default();
        let entries = fs::read_dir(&dir).map_err(|e| {
            RivoraError::storage(format!("failed to read dir {}: {e}", dir.display()))
        })?;
        for entry in entries {
            let entry = entry
                .map_err(|e| RivoraError::storage(format!("failed to read dir entry: {e}")))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            match self.read_json::<CapabilityLifecycleRun>(&path) {
                Ok(run) if run.investigation_id == *id => listing.runs.push(run),
                Ok(_) => listing.diagnostics.push(LifecycleStorageDiagnostic {
                    path: path.display().to_string(),
                    error: "lifecycle run investigation ownership mismatch".into(),
                }),
                Err(error) => listing.diagnostics.push(LifecycleStorageDiagnostic {
                    path: path.display().to_string(),
                    error: error.to_string(),
                }),
            }
        }
        listing.runs.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then_with(|| a.lineage_id.to_string().cmp(&b.lineage_id.to_string()))
                .then_with(|| a.revision_number.cmp(&b.revision_number))
                .then_with(|| a.id.to_string().cmp(&b.id.to_string()))
        });
        listing.diagnostics.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(listing)
    }

    fn find_lifecycle_run_by_idempotency(
        &self,
        investigation_id: &InvestigationId,
        key: &str,
    ) -> RivoraResult<Option<CapabilityLifecycleRun>> {
        let listing = self.list_lifecycle_runs(investigation_id)?;
        let mut matches: Vec<_> = listing
            .runs
            .into_iter()
            .filter(|r| r.idempotency_key == key)
            .collect();
        matches.sort_by(|a, b| {
            a.revision_number
                .cmp(&b.revision_number)
                .then_with(|| a.created_at.cmp(&b.created_at))
        });
        Ok(matches.pop())
    }

    fn health_report(&self) -> RivoraResult<StoreHealthReport> {
        LocalStore::health_report(self)
    }

    fn diagnostic_export(&self) -> RivoraResult<serde_json::Value> {
        LocalStore::diagnostic_export(self)
    }

    fn backup_to(&self, dest: &std::path::Path) -> RivoraResult<()> {
        LocalStore::backup_to(self, dest)
    }

    fn rebuild_observation_indexes(&self) -> RivoraResult<u64> {
        LocalStore::rebuild_observation_indexes(self)
    }
}

impl LocalStore {
    fn lifecycle_runs_dir(&self, id: &InvestigationId) -> PathBuf {
        self.inv_dir(id).join("lifecycle_runs")
    }

    fn lifecycle_run_path(&self, investigation_id: &InvestigationId, id: &ObjectId) -> PathBuf {
        self.lifecycle_runs_dir(investigation_id)
            .join(format!("{id}.json"))
    }

    /// Atomically claim an observation idempotency key. Returns false if already claimed.
    fn claim_observation_key(
        &self,
        investigation_id: &InvestigationId,
        key: &str,
        object_id: &ObjectId,
    ) -> RivoraResult<bool> {
        let index_dir = self
            .inv_dir(investigation_id)
            .join("indexes")
            .join("observation_keys");
        fs::create_dir_all(&index_dir)
            .map_err(|e| RivoraError::storage(format!("failed to create key index: {e}")))?;
        let path = index_dir.join(format!("{}.json", stable_key_hash(key)));
        let body = serde_json::json!({
            "object_id": object_id.to_string(),
            "idempotency_key": key,
        });
        self.write_json_new(&path, &body)
    }
}

fn parse_lock_contents(content: &str) -> RivoraResult<(u32, u64)> {
    let mut pid = None;
    let mut created_at = None;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("pid=") {
            pid = rest.trim().parse().ok();
        } else if let Some(rest) = line.strip_prefix("created_at=") {
            created_at = rest.trim().parse().ok();
        }
    }
    match (pid, created_at) {
        (Some(p), Some(c)) => Ok((p, c)),
        _ => Err(RivoraError::storage("unparseable lock file")),
    }
}

fn lock_is_stale(created_at: u64) -> bool {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    now.saturating_sub(created_at) >= STALE_LOCK_SECS
}

fn process_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    // Same process always counts as alive.
    if pid == std::process::id() {
        return true;
    }
    #[cfg(unix)]
    {
        // `kill -0` checks process existence without delivering a signal.
        use std::process::{Command, Stdio};
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(true)
    }
    #[cfg(not(unix))]
    {
        // Conservative: treat foreign PIDs as alive so we do not steal locks
        // on platforms without a cheap existence probe.
        let _ = pid;
        true
    }
}

fn stable_key_hash(key: &str) -> String {
    // Deterministic non-crypto fingerprint for index filenames (not a security boundary).
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn dir_size(path: &Path) -> std::io::Result<u64> {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let meta = entry.metadata()?;
            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                total = total.saturating_add(meta.len());
            }
        }
    }
    Ok(total)
}

fn copy_dir_excluding_lock(src: &Path, dest: &Path) -> RivoraResult<()> {
    let src = src.canonicalize().unwrap_or_else(|_| src.to_path_buf());
    // Refuse nested destinations that would recurse into the live store.
    if dest.starts_with(&src) {
        return Err(RivoraError::validation(
            "backup destination must not be inside the store root",
        ));
    }
    fs::create_dir_all(dest)
        .map_err(|e| RivoraError::storage(format!("failed to create backup root: {e}")))?;
    let mut stack = vec![src.clone()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)
            .map_err(|e| RivoraError::storage(format!("failed to read for backup: {e}")))?
        {
            let entry =
                entry.map_err(|e| RivoraError::storage(format!("failed to read entry: {e}")))?;
            let path = entry.path();
            let name = entry.file_name();
            if name == ".rivora.lock" {
                continue;
            }
            let rel = path
                .strip_prefix(&src)
                .map_err(|e| RivoraError::storage(format!("backup path strip failed: {e}")))?;
            let target = dest.join(rel);
            if path.is_dir() {
                fs::create_dir_all(&target).map_err(|e| {
                    RivoraError::storage(format!("failed to create backup dir: {e}"))
                })?;
                stack.push(path);
            } else {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(|e| {
                        RivoraError::storage(format!("failed to create backup parent: {e}"))
                    })?;
                }
                fs::copy(&path, &target).map_err(|e| {
                    RivoraError::storage(format!("failed to copy {}: {e}", path.display()))
                })?;
            }
        }
    }
    Ok(())
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

        // Survives reopen (drop first holder to release exclusive lock).
        drop(store);
        let reopened = LocalStore::open(dir.path()).unwrap();
        assert_eq!(reopened.list_relationships().unwrap().len(), 2);
        assert_eq!(reopened.load_relationship(&rel_b.id).unwrap(), rel_b);

        reopened.delete_relationship(&rel_a.id).unwrap();
        let missing = reopened.load_relationship(&rel_a.id).unwrap_err();
        assert!(matches!(missing, RivoraError::ObjectNotFound(_)));
        let gone = reopened.delete_relationship(&rel_a.id).unwrap_err();
        assert!(matches!(gone, RivoraError::ObjectNotFound(_)));
        assert_eq!(reopened.list_relationships().unwrap().len(), 1);
    }

    #[test]
    fn same_process_lock_is_reentrant() {
        let dir = tempfile::tempdir().unwrap();
        let first = LocalStore::open(dir.path()).unwrap();
        // Same process may open again (shared refcount); cross-process is exclusive.
        let second = LocalStore::open(dir.path()).unwrap();
        assert!(first.lock_held());
        assert!(second.lock_held());
        drop(second);
        assert!(dir.path().join(".rivora.lock").exists());
        drop(first);
        assert!(!dir.path().join(".rivora.lock").exists());
    }

    #[test]
    fn stale_lock_can_be_recovered() {
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join(".rivora.lock");
        // Fake dead PID with old timestamp.
        fs::write(&lock_path, "pid=999999\ncreated_at=1\n").unwrap();
        assert!(LocalStore::recover_stale_lock(dir.path()).unwrap());
        let store = LocalStore::open(dir.path()).unwrap();
        assert!(store.lock_held());
    }

    #[test]
    fn corrupt_memory_is_isolated() {
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
            Some("k1".into()),
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
        // Plant a corrupt sibling.
        let bad = store
            .root()
            .join("investigations")
            .join(inv.id.to_string())
            .join("memory")
            .join("corrupt.json");
        fs::write(&bad, "{not-json").unwrap();
        let listed = store.list_memory(&inv.id).unwrap();
        assert_eq!(listed.len(), 1);
        let health = store.health_report().unwrap();
        assert!(!health.corrupt_records.is_empty());
    }

    #[test]
    fn future_schema_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalStore::open(dir.path()).unwrap();
        let path = store.root().join("store.json");
        drop(store);
        let future = serde_json::json!({
            "schema_version": 99,
            "rivora_version": "9.9.9",
            "created_at": "2020-01-01T00:00:00Z",
            "last_opened_at": "2020-01-01T00:00:00Z",
        });
        fs::write(&path, serde_json::to_vec_pretty(&future).unwrap()).unwrap();
        let err = LocalStore::open(dir.path()).unwrap_err();
        assert!(matches!(
            err,
            RivoraError::SchemaMismatch {
                found: 99,
                supported_max: _
            }
        ));
    }
}
