//! Production hardening contracts for v0.9.
//!
//! Defines the supported operating envelope, failure classifications,
//! store health, and CLI exit-code mapping. These types document and
//! enforce realistic local/on-prem limits without introducing a new
//! Runtime subsystem.

use serde::{Deserialize, Serialize};

/// Current LocalStore schema version written to `store.json`.
///
/// v1 is the first explicit store manifest. Stores created before v0.9
/// open as schema version 1 with additive migration (lazy directories).
pub const STORE_SCHEMA_VERSION: u32 = 1;

/// Maximum store schema version this build can open.
pub const STORE_SCHEMA_VERSION_MAX: u32 = 1;

/// Stale lock age after which a lock file may be recovered if the
/// holding process is not alive (seconds).
pub const STALE_LOCK_SECS: u64 = 300;

/// Default CLI / Workspace list page size within the supported envelope.
pub const DEFAULT_LIST_LIMIT: usize = 100;

/// Hard maximum list/search results returned without explicit paging.
pub const MAX_LIST_LIMIT: usize = 1_000;

/// Maximum Observation / JSON payload size accepted for ingestion (bytes).
pub const MAX_PAYLOAD_BYTES: usize = 1_048_576; // 1 MiB

/// Maximum single Connector HTTP response body (bytes).
pub const MAX_CONNECTOR_RESPONSE_BYTES: usize = 1_048_576; // 1 MiB

/// Maximum Observations accepted in one Connector batch.
pub const MAX_EVENT_BATCH_SIZE: usize = 500;

/// Default Connector HTTP connect timeout (seconds).
pub const CONNECTOR_CONNECT_TIMEOUT_SECS: u64 = 5;

/// Default Connector HTTP request timeout (seconds).
pub const CONNECTOR_REQUEST_TIMEOUT_SECS: u64 = 30;

/// Supported prior LocalStore layouts that open without history loss.
pub const SUPPORTED_PRIOR_STORE_VERSIONS: &[&str] =
    &["0.1", "0.2", "0.3", "0.4", "0.5", "0.6", "0.7", "0.8"];

/// Named operating-envelope profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatingProfile {
    /// Local demos and unit-test scale.
    Small,
    /// Typical single-engineer active repository.
    Medium,
    /// Upper bound of officially supported local/on-prem use.
    LargeSupported,
}

impl OperatingProfile {
    /// Stable string identifier.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::LargeSupported => "large_supported",
        }
    }
}

/// Measurable support targets for a profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatingEnvelope {
    /// Profile name.
    pub profile: OperatingProfile,
    /// Approximate repository file count.
    pub max_repo_files: u64,
    /// Approximate repository disk size (bytes).
    pub max_repo_disk_bytes: u64,
    /// Commits inspected per local observe.
    pub max_commits_inspected: u64,
    /// Observations per Investigation.
    pub max_observations_per_investigation: u64,
    /// Memory records per Investigation.
    pub max_memory_per_investigation: u64,
    /// Lifecycle runs per Investigation.
    pub max_lifecycle_runs_per_investigation: u64,
    /// Investigations per store.
    pub max_investigations_per_store: u64,
    /// Registered Capabilities.
    pub max_registered_capabilities: u64,
    /// Active Connectors.
    pub max_active_connectors: u64,
    /// Engineering Loop artifacts per Investigation.
    pub max_loop_artifacts_per_investigation: u64,
    /// Relationships in the graph.
    pub max_relationships: u64,
    /// Learning patterns store-wide.
    pub max_learning_patterns: u64,
    /// Search index entry budget (records scanned).
    pub max_search_index_entries: u64,
    /// Workspace list rows before paging is required.
    pub max_workspace_list_rows: u64,
    /// Maximum Observation payload size (bytes).
    pub max_payload_bytes: u64,
    /// Maximum Connector event batch size.
    pub max_event_batch_size: u64,
    /// CLI rows before pagination is recommended.
    pub max_cli_rows_before_pagination: u64,
    /// Connector latency budget (milliseconds).
    pub max_connector_latency_ms: u64,
    /// Local disk growth budget for a large store (bytes).
    pub max_local_disk_growth_bytes: u64,
    /// CLI cold startup budget (milliseconds).
    pub max_cli_startup_ms: u64,
    /// Workspace cold startup budget (milliseconds).
    pub max_workspace_startup_ms: u64,
    /// Large Investigation load budget (milliseconds).
    pub max_large_investigation_load_ms: u64,
    /// Concurrent readers supported (advisory; exclusive write lock).
    pub max_concurrent_readers: u64,
    /// Concurrent writers supported (0 means exclusive single writer process).
    pub max_concurrent_writers: u64,
}

impl OperatingEnvelope {
    /// Small profile limits.
    pub fn small() -> Self {
        Self {
            profile: OperatingProfile::Small,
            max_repo_files: 500,
            max_repo_disk_bytes: 50 * 1024 * 1024,
            max_commits_inspected: 20,
            max_observations_per_investigation: 100,
            max_memory_per_investigation: 100,
            max_lifecycle_runs_per_investigation: 50,
            max_investigations_per_store: 50,
            max_registered_capabilities: 16,
            max_active_connectors: 5,
            max_loop_artifacts_per_investigation: 200,
            max_relationships: 100,
            max_learning_patterns: 50,
            max_search_index_entries: 5_000,
            max_workspace_list_rows: 50,
            max_payload_bytes: MAX_PAYLOAD_BYTES as u64,
            max_event_batch_size: 50,
            max_cli_rows_before_pagination: DEFAULT_LIST_LIMIT as u64,
            max_connector_latency_ms: 5_000,
            max_local_disk_growth_bytes: 100 * 1024 * 1024,
            max_cli_startup_ms: 500,
            max_workspace_startup_ms: 1_000,
            max_large_investigation_load_ms: 1_000,
            max_concurrent_readers: 1,
            max_concurrent_writers: 1,
        }
    }

    /// Medium profile limits.
    pub fn medium() -> Self {
        Self {
            profile: OperatingProfile::Medium,
            max_repo_files: 10_000,
            max_repo_disk_bytes: 500 * 1024 * 1024,
            max_commits_inspected: 100,
            max_observations_per_investigation: 1_000,
            max_memory_per_investigation: 1_000,
            max_lifecycle_runs_per_investigation: 200,
            max_investigations_per_store: 500,
            max_registered_capabilities: 32,
            max_active_connectors: 5,
            max_loop_artifacts_per_investigation: 2_000,
            max_relationships: 2_000,
            max_learning_patterns: 500,
            max_search_index_entries: 100_000,
            max_workspace_list_rows: DEFAULT_LIST_LIMIT as u64,
            max_payload_bytes: MAX_PAYLOAD_BYTES as u64,
            max_event_batch_size: MAX_EVENT_BATCH_SIZE as u64,
            max_cli_rows_before_pagination: DEFAULT_LIST_LIMIT as u64,
            max_connector_latency_ms: 15_000,
            max_local_disk_growth_bytes: 2 * 1024 * 1024 * 1024,
            max_cli_startup_ms: 1_000,
            max_workspace_startup_ms: 2_000,
            max_large_investigation_load_ms: 3_000,
            max_concurrent_readers: 1,
            max_concurrent_writers: 1,
        }
    }

    /// Large supported profile (upper bound of official support).
    pub fn large_supported() -> Self {
        Self {
            profile: OperatingProfile::LargeSupported,
            max_repo_files: 100_000,
            max_repo_disk_bytes: 5 * 1024 * 1024 * 1024,
            max_commits_inspected: 500,
            max_observations_per_investigation: 10_000,
            max_memory_per_investigation: 10_000,
            max_lifecycle_runs_per_investigation: 1_000,
            max_investigations_per_store: 5_000,
            max_registered_capabilities: 64,
            max_active_connectors: 8,
            max_loop_artifacts_per_investigation: 20_000,
            max_relationships: 20_000,
            max_learning_patterns: 5_000,
            max_search_index_entries: 1_000_000,
            max_workspace_list_rows: MAX_LIST_LIMIT as u64,
            max_payload_bytes: MAX_PAYLOAD_BYTES as u64,
            max_event_batch_size: MAX_EVENT_BATCH_SIZE as u64,
            max_cli_rows_before_pagination: DEFAULT_LIST_LIMIT as u64,
            max_connector_latency_ms: 30_000,
            max_local_disk_growth_bytes: 20 * 1024 * 1024 * 1024,
            max_cli_startup_ms: 2_000,
            max_workspace_startup_ms: 3_000,
            max_large_investigation_load_ms: 10_000,
            max_concurrent_readers: 1,
            max_concurrent_writers: 1,
        }
    }

    /// Return the envelope for a named profile.
    pub fn for_profile(profile: OperatingProfile) -> Self {
        match profile {
            OperatingProfile::Small => Self::small(),
            OperatingProfile::Medium => Self::medium(),
            OperatingProfile::LargeSupported => Self::large_supported(),
        }
    }
}

/// Explicit failure / completion classification for production operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    /// Safe to retry with the same idempotency key after an explicit request.
    Retryable,
    /// Must not be retried automatically.
    NonRetryable,
    /// Blocked by policy, lock, missing prerequisite, or authority.
    Blocked,
    /// Terminal failure with durable evidence.
    Failed,
    /// Some stages completed; overall work is incomplete.
    Partial,
    /// Outcome could not be determined (timeout, ambiguous transport).
    Inconclusive,
    /// Operation is not supported in this build or configuration.
    Unsupported,
    /// Intentionally deferred until measured evidence exists.
    Deferred,
    /// Requires human review before further action.
    RequiresManualReview,
}

impl FailureClass {
    /// Stable string form.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Retryable => "retryable",
            Self::NonRetryable => "non_retryable",
            Self::Blocked => "blocked",
            Self::Failed => "failed",
            Self::Partial => "partial",
            Self::Inconclusive => "inconclusive",
            Self::Unsupported => "unsupported",
            Self::Deferred => "deferred",
            Self::RequiresManualReview => "requires_manual_review",
        }
    }
}

/// Stable CLI exit codes for production automation (v0.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CliExitCode {
    /// Success (including intentional no-op replays that fully succeed).
    Success = 0,
    /// Validation / usage error.
    Validation = 2,
    /// Requested resource not found.
    NotFound = 3,
    /// Operation unsupported.
    Unsupported = 4,
    /// Blocked by policy, lock, or precondition.
    Blocked = 5,
    /// Partial completion — never reported as full success.
    Partial = 6,
    /// External provider / Connector failure.
    ProviderFailure = 7,
    /// Authentication / authorization failure against a provider.
    AuthFailure = 8,
    /// Timeout.
    Timeout = 9,
    /// Corrupt store or record.
    CorruptStore = 10,
    /// Schema mismatch / incompatible store version.
    SchemaMismatch = 11,
    /// Store lock conflict.
    LockConflict = 12,
    /// Policy denial (execution authority).
    PolicyDenial = 13,
    /// Verification failed (independent of API acceptance).
    VerificationFailure = 14,
    /// Internal / unexpected error.
    Internal = 1,
}

impl CliExitCode {
    /// Numeric code for `std::process::ExitCode`.
    pub fn code(self) -> u8 {
        self as u8
    }
}

/// Local store manifest written to `{root}/store.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreManifest {
    /// Schema version of the on-disk layout.
    pub schema_version: u32,
    /// Product version that last wrote the manifest.
    pub rivora_version: String,
    /// RFC3339 creation timestamp.
    pub created_at: String,
    /// RFC3339 last-open timestamp.
    pub last_opened_at: String,
}

impl StoreManifest {
    /// Create a new manifest for this build.
    pub fn new_now(rivora_version: impl Into<String>) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            schema_version: STORE_SCHEMA_VERSION,
            rivora_version: rivora_version.into(),
            created_at: now.clone(),
            last_opened_at: now,
        }
    }
}

/// One isolated corrupt or unreadable record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreRecordDiagnostic {
    /// Relative or absolute path of the record.
    pub path: String,
    /// Sanitized error message (no secrets).
    pub error: String,
    /// Object kind when known (`memory`, `observation`, …).
    pub kind: String,
}

/// Store integrity / operations health report (local only; no telemetry).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreHealthReport {
    /// Absolute store root path.
    pub root: String,
    /// Schema version currently on disk.
    pub schema_version: u32,
    /// Whether the process holds the exclusive store lock.
    pub lock_held: bool,
    /// Lock file path when present.
    pub lock_path: Option<String>,
    /// Investigation count.
    pub investigation_count: u64,
    /// Observation count (valid records only).
    pub observation_count: u64,
    /// Memory record count (valid records only).
    pub memory_count: u64,
    /// Lifecycle run count (valid records only).
    pub lifecycle_run_count: u64,
    /// Relationship count.
    pub relationship_count: u64,
    /// Learning pattern count.
    pub learning_pattern_count: u64,
    /// Approximate total bytes under the store root.
    pub disk_bytes: u64,
    /// Isolated corrupt / unreadable record diagnostics.
    pub corrupt_records: Vec<StoreRecordDiagnostic>,
    /// Orphan temporary files cleaned or remaining.
    pub orphan_temp_files: Vec<String>,
    /// Recent sanitized operational notes.
    pub notes: Vec<String>,
    /// Schema / migration status summary.
    pub migration_status: String,
    /// Supported prior versions that open safely.
    pub supported_prior_versions: Vec<String>,
}

impl StoreHealthReport {
    /// True when no corrupt records were observed.
    pub fn is_healthy(&self) -> bool {
        self.corrupt_records.is_empty()
    }
}

/// Performance budget entry for a named scenario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerformanceBudget {
    /// Scenario name.
    pub scenario: String,
    /// Target duration in milliseconds.
    pub target_ms: u64,
    /// Maximum allowed duration in milliseconds.
    pub max_ms: u64,
    /// Acceptable relative variance (0.0–1.0).
    pub variance_tolerance: f64,
}

impl PerformanceBudget {
    /// Standard v0.9 budgets (medium profile hardware assumptions).
    pub fn v0_9_budgets() -> Vec<Self> {
        vec![
            Self {
                scenario: "cli_startup".into(),
                target_ms: 300,
                max_ms: 1_000,
                variance_tolerance: 0.5,
            },
            Self {
                scenario: "workspace_startup".into(),
                target_ms: 500,
                max_ms: 2_000,
                variance_tolerance: 0.5,
            },
            Self {
                scenario: "store_open".into(),
                target_ms: 50,
                max_ms: 250,
                variance_tolerance: 0.5,
            },
            Self {
                scenario: "investigation_list".into(),
                target_ms: 100,
                max_ms: 500,
                variance_tolerance: 0.5,
            },
            Self {
                scenario: "investigation_show".into(),
                target_ms: 50,
                max_ms: 250,
                variance_tolerance: 0.5,
            },
            Self {
                scenario: "ingestion".into(),
                target_ms: 20,
                max_ms: 100,
                variance_tolerance: 0.5,
            },
            Self {
                scenario: "duplicate_ingestion".into(),
                target_ms: 10,
                max_ms: 50,
                variance_tolerance: 0.5,
            },
            Self {
                scenario: "routing".into(),
                target_ms: 5,
                max_ms: 50,
                variance_tolerance: 0.5,
            },
            Self {
                scenario: "lifecycle_run".into(),
                target_ms: 50,
                max_ms: 250,
                variance_tolerance: 0.5,
            },
            Self {
                scenario: "lifecycle_trace".into(),
                target_ms: 20,
                max_ms: 100,
                variance_tolerance: 0.5,
            },
            Self {
                scenario: "search".into(),
                target_ms: 100,
                max_ms: 1_000,
                variance_tolerance: 0.5,
            },
            Self {
                scenario: "recall".into(),
                target_ms: 50,
                max_ms: 500,
                variance_tolerance: 0.5,
            },
            Self {
                scenario: "timeline".into(),
                target_ms: 50,
                max_ms: 500,
                variance_tolerance: 0.5,
            },
            Self {
                scenario: "relationship_derivation".into(),
                target_ms: 100,
                max_ms: 1_000,
                variance_tolerance: 0.5,
            },
            Self {
                scenario: "pattern_derivation".into(),
                target_ms: 100,
                max_ms: 1_000,
                variance_tolerance: 0.5,
            },
            Self {
                scenario: "proposal_generation".into(),
                target_ms: 100,
                max_ms: 1_000,
                variance_tolerance: 0.5,
            },
            Self {
                scenario: "persistence_read".into(),
                target_ms: 5,
                max_ms: 50,
                variance_tolerance: 0.5,
            },
            Self {
                scenario: "persistence_write".into(),
                target_ms: 10,
                max_ms: 50,
                variance_tolerance: 0.5,
            },
            Self {
                scenario: "index_rebuild".into(),
                target_ms: 200,
                max_ms: 2_000,
                variance_tolerance: 0.5,
            },
            Self {
                scenario: "diagnostic_export".into(),
                target_ms: 100,
                max_ms: 1_000,
                variance_tolerance: 0.5,
            },
        ]
    }
}

/// Replay / idempotency contract for a major operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayContract {
    /// Operation name.
    pub operation: String,
    /// Idempotency key scope description.
    pub key_scope: String,
    /// Whether replay reuses existing durable objects.
    pub reuses_lineage: bool,
    /// Whether dry-run can suppress a later live execution.
    pub dry_run_suppresses_live: bool,
    /// Whether retries may bypass policy/approval/exact revision.
    pub retry_bypasses_authority: bool,
    /// Safe resume after partial completion.
    pub resumes_partial: bool,
    /// Summary of expected behavior.
    pub behavior: String,
}

impl ReplayContract {
    /// Canonical v0.9 replay contracts for major operations.
    pub fn v0_9_contracts() -> Vec<Self> {
        vec![
            Self {
                operation: "connector_event_ingest".into(),
                key_scope: "investigation + observation.idempotency_key".into(),
                reuses_lineage: true,
                dry_run_suppresses_live: false,
                retry_bypasses_authority: false,
                resumes_partial: true,
                behavior: "Duplicate keys return existing Observation and Memory; no new records".into(),
            },
            Self {
                operation: "cli_command_observe".into(),
                key_scope: "same as connector_event_ingest when key provided".into(),
                reuses_lineage: true,
                dry_run_suppresses_live: false,
                retry_bypasses_authority: false,
                resumes_partial: true,
                behavior: "Repeated CLI observe with same key is a no-op replay".into(),
            },
            Self {
                operation: "capability_routing".into(),
                key_scope: "deterministic input type identifiers".into(),
                reuses_lineage: true,
                dry_run_suppresses_live: false,
                retry_bypasses_authority: false,
                resumes_partial: true,
                behavior: "Routing is pure; repeated calls yield identical decisions".into(),
            },
            Self {
                operation: "capability_invocation".into(),
                key_scope: "execution attempt idempotency_key + mode".into(),
                reuses_lineage: true,
                dry_run_suppresses_live: false,
                retry_bypasses_authority: false,
                resumes_partial: true,
                behavior: "Duplicate live keys suppress mutation; dry-run keys never suppress live".into(),
            },
            Self {
                operation: "engineering_loop_orchestration".into(),
                key_scope: "lifecycle:{attempt.lineage_id()}".into(),
                reuses_lineage: true,
                dry_run_suppresses_live: false,
                retry_bypasses_authority: false,
                resumes_partial: true,
                behavior: "Identical lifecycle key reuses CapabilityLifecycleRun snapshot".into(),
            },
            Self {
                operation: "pattern_relationship_derivation".into(),
                key_scope: "derivation key / relationship natural key".into(),
                reuses_lineage: true,
                dry_run_suppresses_live: false,
                retry_bypasses_authority: false,
                resumes_partial: true,
                behavior: "Derived relationships upsert by natural key; patterns rebuild deterministically".into(),
            },
            Self {
                operation: "verification".into(),
                key_scope: "new receipt per explicit verification request".into(),
                reuses_lineage: false,
                dry_run_suppresses_live: false,
                retry_bypasses_authority: false,
                resumes_partial: true,
                behavior: "Verification is independent; repeats create durable receipts, never silent skip".into(),
            },
            Self {
                operation: "workspace_action".into(),
                key_scope: "same Runtime APIs as CLI".into(),
                reuses_lineage: true,
                dry_run_suppresses_live: false,
                retry_bypasses_authority: false,
                resumes_partial: true,
                behavior: "Workspace shares CapabilityService; no independent canonical state".into(),
            },
            Self {
                operation: "migration_index_rebuild".into(),
                key_scope: "store schema version + rebuild fingerprint".into(),
                reuses_lineage: true,
                dry_run_suppresses_live: false,
                retry_bypasses_authority: false,
                resumes_partial: true,
                behavior: "Migrations are additive; indexes rebuild from canonical records".into(),
            },
            Self {
                operation: "execution_dry_run_then_live".into(),
                key_scope: "mode=dry_run vs mode=live distinct keys".into(),
                reuses_lineage: false,
                dry_run_suppresses_live: false,
                retry_bypasses_authority: false,
                resumes_partial: true,
                behavior: "Dry-run never suppresses subsequent live execution".into(),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelopes_are_monotonic() {
        let s = OperatingEnvelope::small();
        let m = OperatingEnvelope::medium();
        let l = OperatingEnvelope::large_supported();
        assert!(s.max_investigations_per_store < m.max_investigations_per_store);
        assert!(m.max_investigations_per_store < l.max_investigations_per_store);
        assert!(s.max_memory_per_investigation <= m.max_memory_per_investigation);
    }

    #[test]
    fn budgets_cover_required_scenarios() {
        let names: Vec<_> = PerformanceBudget::v0_9_budgets()
            .into_iter()
            .map(|b| b.scenario)
            .collect();
        for required in [
            "cli_startup",
            "store_open",
            "ingestion",
            "duplicate_ingestion",
            "search",
            "lifecycle_run",
            "diagnostic_export",
        ] {
            assert!(names.iter().any(|n| n == required), "missing {required}");
        }
    }

    #[test]
    fn dry_run_never_suppresses_live_in_contracts() {
        for c in ReplayContract::v0_9_contracts() {
            assert!(
                !c.dry_run_suppresses_live,
                "{} must not let dry-run suppress live",
                c.operation
            );
            assert!(
                !c.retry_bypasses_authority,
                "{} must not bypass authority on retry",
                c.operation
            );
        }
    }
}
