//! Rivora CLI — thin Capability client (RFC-003).
//!
//! No Runtime business logic lives here. All reasoning is invoked via
//! `CapabilityService`.
//!
//! Bare `rivora` (no subcommand) launches the shared Workspace entrypoint
//! (`rivora_workspace::run_workspace`). Explicit subcommands remain one-shot CLI.

use std::path::PathBuf;
use std::process::ExitCode;

use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand, ValueEnum};
use rivora::domain::{
    Confidence, ImplementationRecord, ImplementationReference, ImplementationSource,
    ImprovementProposal, InvestigationId, InvestigationStatus, MeasuredLearningOutcome, ObjectId,
    ObservationKind, OutcomeDisposition, OutcomeEvidenceRelation, ProposalCategory,
    ProposalFeedbackCategory, ProposalPriority, ProposalStatus, ProposalTransitionAuthority,
    RelationshipKind, VerificationResult,
};
use rivora::runtime::execution::{CreateExecutionPlanRequest, ReviseExecutionPlanRequest};
use rivora::runtime::outcome::{
    CollectOutcomeEvidenceRequest, RecordImplementationRequest, ReviseImplementationRequest,
};
use rivora::runtime::proposal::{
    CreateProposalRequest, ProposalPortfolioFilter, RefineProposalRequest,
};
use rivora::runtime::search::{OutcomeFilter, SearchQuery, SearchResult};
use rivora::{
    CapabilityService, CliExitCode, ExecutionAction, ExecutionPrecondition, OperatingEnvelope,
    OperatingProfile, PerformanceBudget, ReplayContract, RivoraError,
};
use rivora_connectors::github::GitHubConnector;
use rivora_connectors::github_actions::{ConnectorStatusReport, GitHubActionsConnector};
use rivora_connectors::kubernetes::KubernetesConnector;
use rivora_connectors::local::LocalConnector;
use rivora_connectors::sentry::SentryConnector;
use rivora_connectors::NormalizedObservation;
use rivora_workspace::{err, open_capabilities, run_workspace, WorkspaceLaunchConfig};

const PROPOSAL_BOUNDARY: &str = "Proposal only — not applied, not implemented, not verified.";
const LEARNING_BOUNDARY: &str = "Measured Learning Outcome — external implementation recorded, never auto-applied; verified only with explicit actor+reason.";
const EXECUTION_BOUNDARY: &str = "Execution Through External Systems — only explicitly approved, bounded capabilities; Proposal acceptance ≠ execution approval.";

#[derive(Debug, Parser)]
#[command(
    name = "rivora",
    version,
    about = "Rivora — Engineering Understanding Platform. Run with no subcommand to open the Workspace.",
    after_help = "Run `rivora` with no subcommand to open the interactive Workspace.\nUse `rivora <command>` for one-shot CLI operations."
)]
struct Cli {
    /// Data directory for local Runtime storage.
    #[arg(long, global = true, default_value = ".rivora/data")]
    data_dir: PathBuf,

    /// Emit JSON instead of human-readable text (requires a subcommand).
    #[arg(long, global = true)]
    json: bool,

    /// When present, run the requested one-shot CLI command.
    /// When absent, launch the interactive Workspace (same path as `rivora-workspace`).
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Investigation lifecycle commands.
    Investigation {
        #[command(subcommand)]
        action: InvestigationCmd,
    },
    /// Ingest Observations (manual or via connectors).
    Observe {
        /// Investigation id.
        #[arg(long)]
        investigation: String,
        /// Observation summary (manual mode).
        #[arg(long)]
        summary: Option<String>,
        /// Observation kind (manual mode).
        #[arg(long, default_value = "event")]
        kind: String,
        /// JSON payload (manual mode).
        #[arg(long, default_value = "{}")]
        payload: String,
        /// Source name.
        #[arg(long, default_value = "cli")]
        source: String,
        /// Idempotency key.
        #[arg(long)]
        idempotency_key: Option<String>,
        /// Observe local project path with the local connector.
        #[arg(long)]
        local: Option<PathBuf>,
        /// Observe GitHub repository (`owner/repo`).
        #[arg(long)]
        github: Option<String>,
        /// Pull request number for GitHub connector.
        #[arg(long)]
        pr: Option<u64>,
        /// Load GitHub fixture JSON instead of calling the API.
        #[arg(long)]
        github_fixture: Option<PathBuf>,
    },
    /// Recall Investigation Memory, related evidence, or prior outcomes.
    Recall {
        /// Investigation id (Memory recall; combine with --evidence for
        /// related-evidence recall).
        #[arg(long)]
        investigation: Option<String>,
        /// Recall related evidence for the Investigation.
        #[arg(long)]
        evidence: bool,
        /// Prior-outcome recall: repository filter.
        #[arg(long)]
        repository: Option<String>,
        /// Prior-outcome recall: only this disposition.
        #[arg(long, value_enum)]
        outcome: Option<DispositionArg>,
        /// Prior-outcome recall: only Investigations related to this one.
        #[arg(long)]
        similar_to: Option<String>,
    },
    /// Search Investigations (text and/or structured filters).
    Search {
        /// Free-text query.
        query: Option<String>,
        /// Repository filter.
        #[arg(long)]
        repository: Option<String>,
        /// Status filter (e.g. collecting, completed).
        #[arg(long)]
        status: Option<String>,
        /// Connector source filter.
        #[arg(long)]
        source: Option<String>,
        /// Verification result filter: pass, fail, inconclusive.
        #[arg(long)]
        verification: Option<String>,
        /// Learning outcome filter.
        #[arg(long, value_enum)]
        outcome: Option<DispositionArg>,
        /// Changed-file path filter.
        #[arg(long)]
        file: Option<String>,
        /// Relationship kind filter (snake_case, e.g. shared_repository).
        #[arg(long)]
        relationship: Option<String>,
        /// Only Investigations created after this RFC3339 timestamp.
        #[arg(long)]
        after: Option<String>,
        /// Only Investigations created before this RFC3339 timestamp.
        #[arg(long)]
        before: Option<String>,
        /// Maximum number of results (default 100 within supported envelope).
        #[arg(long)]
        limit: Option<usize>,
        /// Explain a specific result instead of listing all matches.
        #[arg(long)]
        explain: Option<String>,
    },
    /// Generate Investigation timeline.
    Timeline {
        #[arg(long)]
        investigation: String,
    },
    /// Derive Knowledge from Memory.
    Knowledge {
        #[arg(long)]
        investigation: String,
    },
    /// Evaluate Investigation.
    Evaluate {
        #[arg(long)]
        investigation: String,
    },
    /// Verify conclusions.
    Verify {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        evaluation: Option<String>,
    },
    /// Generate Recommendations.
    Recommend {
        #[arg(long)]
        investigation: String,
    },
    /// Record a v0.1 Learning Outcome disposition (recommendation feedback).
    RecordOutcome {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        recommendation: Option<String>,
        #[arg(long, value_enum)]
        disposition: DispositionArg,
        #[arg(long, default_value = "")]
        notes: String,
        #[arg(long)]
        impact: Option<String>,
    },
    /// Implementation Records for external work associated with Proposals (v0.5).
    Implementation {
        #[command(subcommand)]
        action: ImplementationCmd,
    },
    /// Measured Learning Outcomes, patterns, and historical influence (v0.5).
    Learn {
        #[command(subcommand)]
        action: LearnCmd,
    },
    /// Run full pipeline: knowledge → evaluate → verify → recommend.
    Pipeline {
        #[arg(long)]
        investigation: String,
    },
    /// Detect Investigation patterns across durable records (RFC-017).
    Patterns,
    /// Summarize historical trends (RFC-017).
    Trends {
        /// Optional repository filter.
        #[arg(long)]
        repository: Option<String>,
    },
    /// Engineering assistance and Composite Capabilities (RFC-018 / RFC-019).
    Assist {
        #[command(subcommand)]
        action: AssistCmd,
    },
    /// Read-only connector operations (RFC-012).
    Connector {
        #[command(subcommand)]
        action: ConnectorCmd,
    },
    /// Generate an engineering report for an Investigation.
    Report {
        #[arg(long)]
        investigation: String,
    },
    /// Durable Improvement Proposal operations (proposal only; never applied).
    Proposal {
        #[command(subcommand)]
        action: ProposalCmd,
    },
    /// Controlled external execution (v0.6) — requires explicit plan approval.
    Execute {
        #[command(subcommand)]
        action: ExecuteCmd,
    },
    /// Capability descriptors, routing, coverage, and Engineering Loop inspection (v0.7/v0.8).
    Capability {
        #[command(subcommand)]
        action: CapabilityCmd,
    },
    /// Local diagnostics, store health, envelope, and recovery helpers (v0.9).
    Doctor {
        #[command(subcommand)]
        action: DoctorCmd,
    },
}

#[derive(Debug, Subcommand)]
enum DoctorCmd {
    /// Store integrity health report.
    Health,
    /// Sanitized diagnostic export (JSON always).
    Export {
        /// Optional path to write the export (stdout when omitted).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Backup the local store to a destination directory.
    Backup {
        /// Destination directory (must not already exist).
        dest: PathBuf,
    },
    /// Rebuild observation idempotency indexes from canonical records.
    RebuildIndexes,
    /// Recover a stale store lock when the holding process is gone.
    RecoverLock,
    /// Show the supported operating envelope for a profile.
    Envelope {
        /// Profile: small | medium | large_supported (default medium).
        #[arg(long, default_value = "medium")]
        profile: String,
    },
    /// List performance budgets.
    Budgets,
    /// List replay / idempotency contracts.
    ReplayContracts,
    /// Print stable CLI exit-code contract.
    ExitCodes,
}

#[derive(Debug, Subcommand)]
enum CapabilityCmd {
    /// List registered Capabilities with Engineering Loop participation.
    List,
    /// Show one Capability descriptor including loop participation.
    Show {
        /// Capability id (e.g. mock.record, github_actions.workflow_dispatch).
        id: String,
    },
    /// First-party Capability and Connector coverage/health report (v0.8).
    Coverage,
    /// Route Observations to compatible Capabilities (typed, deterministic).
    Route {
        #[arg(long)]
        investigation: String,
        /// Observation ids to route (repeatable).
        #[arg(long = "observation")]
        observations: Vec<String>,
    },
    /// Run the Engineering Loop for a completed execution attempt.
    Lifecycle {
        #[arg(long)]
        investigation: String,
        /// Execution Attempt id.
        #[arg(long)]
        attempt: String,
    },
    /// List Engineering Loop runs for an Investigation.
    LifecycleList {
        #[arg(long)]
        investigation: String,
    },
    /// Show one Engineering Loop run snapshot.
    LifecycleShow {
        #[arg(long)]
        investigation: String,
        /// Lifecycle run snapshot id.
        #[arg(long)]
        run: String,
    },
    /// Trace lineage from invocation/attempt through Engineering Loop stages.
    Trace {
        #[arg(long)]
        investigation: String,
        /// Attempt id, invocation id, or lifecycle run/lineage id.
        id: String,
    },
}

#[derive(Debug, Subcommand)]
enum ExecuteCmd {
    /// List registered execution capabilities.
    Capabilities,
    /// Show one execution capability.
    Capability {
        /// Capability id (e.g. mock.record, github.issue.comment).
        id: String,
    },
    /// Create a draft Execution Plan for an accepted Proposal.
    Plan {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        proposal: String,
        #[arg(long)]
        capability: String,
        #[arg(long, default_value = "mock")]
        target_system: String,
        #[arg(long, default_value = "sandbox")]
        environment: String,
        /// Action name supported by the capability. Repeat for an ordered multi-action plan.
        #[arg(long, required = true)]
        action: Vec<String>,
        /// Fallback JSON input applied to each action when --action-input is omitted.
        #[arg(long, default_value = "{}")]
        inputs: String,
        /// Per-action JSON input. Repeat once per --action, in the same order.
        #[arg(long = "action-input")]
        action_inputs: Vec<String>,
        /// JSON ExecutionPrecondition. Repeat to add multiple preconditions.
        #[arg(long)]
        precondition: Vec<String>,
        #[arg(long, default_value = "cli")]
        actor: String,
    },
    /// Validate a draft plan (→ ready_for_review).
    Validate {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        plan: String,
        #[arg(long)]
        reason: String,
        #[arg(long, default_value = "cli")]
        actor: String,
    },
    /// Preview / dry-run a plan (never mutates).
    Preview {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        plan: String,
    },
    /// Approve an exact plan revision for live execution.
    Approve {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        plan: String,
        #[arg(long)]
        reason: String,
        #[arg(long, default_value = "cli")]
        actor: String,
        /// One-time approval (default true).
        #[arg(long, default_value_t = true)]
        one_time: bool,
    },
    /// Reject a plan.
    Reject {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        plan: String,
        #[arg(long)]
        reason: String,
        #[arg(long, default_value = "cli")]
        actor: String,
    },
    /// Execute an approved plan. Requires --confirm for live runs.
    Run {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        plan: String,
        #[arg(long)]
        approval: String,
        #[arg(long)]
        idempotency_key: String,
        #[arg(long, default_value = "cli")]
        actor: String,
        /// Dry-run only (no external mutation).
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        /// Required for live execution.
        #[arg(long, default_value_t = false)]
        confirm: bool,
    },
    /// List execution plans for an Investigation.
    Plans {
        #[arg(long)]
        investigation: String,
    },
    /// List every immutable revision in a plan lineage.
    Revisions {
        #[arg(long)]
        investigation: String,
        /// Any plan snapshot in the lineage.
        #[arg(long)]
        plan: String,
    },
    /// Show one execution plan.
    Show {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        plan: String,
    },
    /// List execution attempts.
    Attempts {
        #[arg(long)]
        investigation: String,
    },
    /// Show one attempt.
    Attempt {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        attempt: String,
    },
    /// Independently verify an attempt.
    Verify {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        attempt: String,
        #[arg(long, default_value = "cli")]
        actor: String,
    },
    /// Trace plan → approval → attempt → receipt → verification → implementation → outcome.
    Trace {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        plan: String,
    },
    /// Explain policy for a plan.
    Policy {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        plan: String,
    },
    /// Cancel a non-terminal execution plan.
    Cancel {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        plan: String,
        #[arg(long)]
        reason: String,
        #[arg(long, default_value = "cli")]
        actor: String,
    },
    /// Create a separate draft rollback plan from an attempt's explicit metadata.
    RollbackPlan {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        attempt: String,
        #[arg(long, default_value = "cli")]
        actor: String,
    },
    /// List execution receipts for an Investigation.
    Receipts {
        #[arg(long)]
        investigation: String,
    },
    /// Export one execution receipt as JSON.
    ExportReceipt {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        receipt: String,
    },
    /// Export plan JSON.
    Export {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        plan: String,
    },
    /// Link attempt to Implementation Record.
    LinkImplementation {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        attempt: String,
        #[arg(long)]
        summary: String,
        #[arg(long, default_value = "cli")]
        actor: String,
    },
    /// Revise plan inputs (invalidates prior approval).
    Revise {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        plan: String,
        #[arg(long)]
        reason: String,
        #[arg(long, default_value = "{}")]
        inputs: String,
        #[arg(long, default_value = "cli")]
        actor: String,
    },
}

#[derive(Debug, Subcommand)]
enum ProposalCmd {
    /// Generate deterministic, evidence-backed Proposal alternatives.
    Generate {
        #[arg(long)]
        investigation: String,
    },
    /// Generate bounded alternatives for an improvement opportunity.
    Alternatives {
        #[arg(long)]
        investigation: String,
    },
    /// Compare two or more Proposals with inspectable factors.
    Compare {
        #[arg(long)]
        investigation: String,
        #[arg(required = true, num_args = 2..)]
        proposals: Vec<String>,
    },
    /// Prioritize the latest Proposals for an Investigation.
    Prioritize {
        #[arg(long)]
        investigation: String,
    },
    /// Show a Proposal's unexecuted Verification Plan.
    VerificationPlan {
        proposal: String,
        #[arg(long)]
        investigation: String,
    },
    /// Show a Proposal's bounded, unapplied implementation outline.
    ImplementationPlan {
        proposal: String,
        #[arg(long)]
        investigation: String,
    },
    /// Explain Proposal generation inputs and temporal provenance.
    Provenance {
        proposal: String,
        #[arg(long)]
        investigation: String,
    },
    /// Export a durable Proposal artifact to stdout without modifying a repository.
    Export {
        proposal: String,
        #[arg(long)]
        investigation: String,
        #[arg(long, value_enum, default_value = "markdown")]
        format: ProposalExportFormatArg,
    },
    /// Generate a bounded coding-agent implementation handoff as text only.
    Handoff {
        proposal: String,
        #[arg(long)]
        investigation: String,
    },
    /// Filter the Investigation-level Proposal portfolio.
    Portfolio {
        #[arg(long)]
        investigation: String,
        #[arg(long, value_enum)]
        status: Option<ProposalPortfolioStatusArg>,
        #[arg(long, value_enum)]
        priority: Option<ProposalPriorityArg>,
        #[arg(long, value_enum)]
        category: Option<ProposalCategoryArg>,
        #[arg(long)]
        source_recommendation: Option<String>,
        #[arg(long)]
        affected_component: Option<String>,
        #[arg(long)]
        unresolved_high_priority: bool,
    },
    /// Trace durable evidence and reasoning objects through a Proposal.
    Trace {
        proposal: String,
        #[arg(long)]
        investigation: String,
    },
    /// Create an explicit candidate (Draft unless validated evidence is cited).
    Create {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        summary: String,
        #[arg(long)]
        rationale: String,
        #[arg(long, value_enum)]
        category: ProposalCategoryArg,
        #[arg(long, value_enum, default_value = "medium")]
        priority: ProposalPriorityArg,
        #[arg(long, default_value_t = 0.5)]
        confidence: f64,
        #[arg(long = "supporting-evidence")]
        supporting_evidence: Vec<String>,
        #[arg(long = "contradicting-evidence")]
        contradicting_evidence: Vec<String>,
        #[arg(long = "source-recommendation")]
        source_recommendations: Vec<String>,
        #[arg(long = "affected-component")]
        affected_components: Vec<String>,
        #[arg(long = "affected-resource")]
        affected_resources: Vec<String>,
    },
    /// List the latest Proposal revision in each lineage.
    List {
        #[arg(long)]
        investigation: String,
    },
    /// Show one Proposal snapshot.
    Show {
        proposal: String,
        #[arg(long)]
        investigation: String,
    },
    /// Explain one Proposal and its no-application boundary.
    Explain {
        proposal: String,
        #[arg(long)]
        investigation: String,
    },
    /// Request an explicit Proposal lifecycle transition.
    Status {
        proposal: String,
        #[arg(long)]
        investigation: String,
        #[arg(long, value_enum)]
        status: ProposalStatusArg,
        #[arg(long)]
        reason: String,
    },
    /// Explicitly accept a Proposal for possible later implementation.
    Accept {
        proposal: String,
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        reason: String,
    },
    /// Explicitly reject a Proposal while preserving it.
    Reject {
        proposal: String,
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        reason: String,
    },
    /// Explicitly defer a Proposal while preserving it.
    Defer {
        proposal: String,
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        reason: String,
    },
    /// Explicitly withdraw a Proposal while preserving it.
    Withdraw {
        proposal: String,
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        reason: String,
    },
    /// Supersede a Proposal with another Proposal in the same Investigation.
    Supersede {
        proposal: String,
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        replacement: String,
        #[arg(long)]
        reason: String,
    },
    /// Refine Proposal content into a preserved successor revision.
    Refine {
        proposal: String,
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        summary: Option<String>,
        #[arg(long)]
        rationale: Option<String>,
        #[arg(long = "affected-component")]
        affected_components: Vec<String>,
        #[arg(long = "test")]
        tests: Vec<String>,
        #[arg(long)]
        reason: String,
    },
    /// Attach explicit feedback as a preserved Proposal revision.
    Feedback {
        proposal: String,
        #[arg(long)]
        investigation: String,
        #[arg(long, value_enum)]
        category: ProposalFeedbackCategoryArg,
        #[arg(long)]
        comment: String,
    },
    /// List all immutable revisions in a Proposal lineage.
    Revisions {
        lineage: String,
        #[arg(long)]
        investigation: String,
    },
}

#[derive(Debug, Subcommand)]
enum ImplementationCmd {
    /// Record that external implementation work was performed for a Proposal.
    Record {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        proposal: String,
        #[arg(long, value_enum)]
        source: ImplementationSourceArg,
        #[arg(long)]
        summary: String,
        /// Free-form reference value (used with --reference-kind).
        #[arg(long)]
        reference: Option<String>,
        #[arg(long, value_enum)]
        reference_kind: Option<ImplementationReferenceKindArg>,
        #[arg(long)]
        commit_sha: Option<String>,
        #[arg(long)]
        pr: Option<String>,
        #[arg(long)]
        note: Option<String>,
        #[arg(long)]
        observed_file: Vec<String>,
        #[arg(long)]
        observed_component: Vec<String>,
        #[arg(long, default_value = "")]
        declared_scope: String,
        #[arg(long, default_value = "cli")]
        actor: String,
    },
    /// List Implementation Records for an Investigation.
    List {
        #[arg(long)]
        investigation: String,
    },
    /// Show one Implementation Record.
    Show {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        implementation: String,
    },
    /// Revise an Implementation Record into a successor snapshot.
    Revise {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        implementation: String,
        #[arg(long)]
        summary: Option<String>,
        #[arg(long)]
        reason: String,
        #[arg(long, default_value = "cli")]
        actor: String,
    },
    /// Link evidence object identifiers to an Implementation Record.
    EvidenceAdd {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        implementation: String,
        #[arg(long = "evidence", required = true)]
        evidence: Vec<String>,
        #[arg(long)]
        reason: String,
        #[arg(long, default_value = "cli")]
        actor: String,
    },
    /// Mark an Implementation Record ready for Measured Outcome evaluation.
    Ready {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        implementation: String,
        #[arg(long)]
        reason: String,
        #[arg(long, default_value = "cli")]
        actor: String,
    },
    /// Withdraw an Implementation Record.
    Withdraw {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        implementation: String,
        #[arg(long)]
        reason: String,
        #[arg(long, default_value = "cli")]
        actor: String,
    },
}

#[derive(Debug, Subcommand)]
enum LearnCmd {
    /// Create a Draft Measured Learning Outcome.
    Create {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        proposal: String,
        #[arg(long)]
        implementation: String,
        #[arg(long, default_value = "cli")]
        actor: String,
    },
    /// Collect typed evidence on a Measured Learning Outcome.
    EvidenceAdd {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        outcome: String,
        #[arg(long)]
        evidence: String,
        #[arg(long, value_enum)]
        relation: OutcomeEvidenceRelationArg,
        #[arg(long)]
        expected_result: Option<String>,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long, default_value = "cli")]
        actor: String,
    },
    /// Deterministically evaluate a Measured Learning Outcome.
    Evaluate {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        outcome: String,
        #[arg(long, default_value = "cli")]
        actor: String,
    },
    /// Explicitly verify a Measured Learning Outcome (requires actor + reason).
    Verify {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        outcome: String,
        #[arg(long, default_value = "cli")]
        actor: String,
        #[arg(long)]
        reason: String,
        #[arg(long, default_value_t = false)]
        override_readiness: bool,
        #[arg(long)]
        override_reason: Option<String>,
    },
    /// List Measured Learning Outcomes for an Investigation.
    List {
        #[arg(long)]
        investigation: String,
    },
    /// Show one Measured Learning Outcome.
    Show {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        outcome: String,
    },
    /// Trace Proposal → Implementation → Measured Learning Outcome.
    Trace {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        outcome: String,
    },
    /// List immutable revisions for a Measured Learning Outcome lineage.
    History {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        outcome: String,
    },
    /// Withdraw a Measured Learning Outcome.
    Withdraw {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        outcome: String,
        #[arg(long)]
        reason: String,
        #[arg(long, default_value = "cli")]
        actor: String,
    },
    /// Export a Measured Learning Outcome.
    Export {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        outcome: String,
        #[arg(long, value_enum, default_value = "markdown")]
        format: LearningExportFormatArg,
    },
    /// List Learning Patterns (use --derive to derive first).
    Patterns {
        #[arg(long, default_value_t = false)]
        derive: bool,
        #[arg(long, default_value = "cli")]
        actor: String,
    },
    /// Show one Learning Pattern.
    PatternShow {
        #[arg(long)]
        pattern: String,
    },
    /// Export a Learning Pattern.
    PatternExport {
        #[arg(long)]
        pattern: String,
        #[arg(long, value_enum, default_value = "markdown")]
        format: LearningExportFormatArg,
    },
    /// Explain historical Learning Pattern influence for a Proposal.
    Influence {
        #[arg(long)]
        investigation: String,
        #[arg(long)]
        proposal: String,
    },
    /// Derive Learning Patterns from verified Measured Outcomes.
    DerivePatterns {
        #[arg(long, default_value = "cli")]
        actor: String,
    },
}

#[derive(Debug, Subcommand)]
enum AssistCmd {
    /// List Composite Capability intents.
    Intents,
    /// Plan a Composite Capability workflow.
    Plan {
        /// Composite intent slug.
        intent: String,
        #[arg(long)]
        investigation: String,
    },
    /// Investigate an engineering problem (composite).
    Investigate { investigation: String },
    /// Assess deployment readiness (composite).
    Readiness { investigation: String },
    /// Explain a failure (composite).
    ExplainFailure { investigation: String },
    /// Generate ranked hypotheses.
    Hypotheses {
        #[arg(long)]
        investigation: String,
    },
    /// Recommend next verification steps.
    NextVerification {
        #[arg(long)]
        investigation: String,
    },
    /// Forecast risks.
    Risks {
        #[arg(long)]
        investigation: String,
    },
    /// Root-cause guidance.
    RootCause {
        #[arg(long)]
        investigation: String,
    },
    /// Prioritize recommendations.
    Prioritize {
        #[arg(long)]
        investigation: String,
    },
    /// Summarize investigation state.
    Summarize {
        #[arg(long)]
        investigation: String,
    },
    /// Workflow inspection and control.
    Workflow {
        #[command(subcommand)]
        action: WorkflowCmd,
    },
}

#[derive(Debug, Subcommand)]
enum WorkflowCmd {
    /// Show a workflow.
    Show {
        #[arg(long)]
        investigation: String,
        workflow: String,
    },
    /// List workflows for an Investigation.
    List {
        #[arg(long)]
        investigation: String,
    },
    /// Resume a workflow.
    Resume {
        #[arg(long)]
        investigation: String,
        workflow: String,
    },
    /// Cancel a workflow.
    Cancel {
        #[arg(long)]
        investigation: String,
        workflow: String,
        #[arg(long)]
        reason: Option<String>,
    },
    /// Explain a workflow.
    Explain {
        #[arg(long)]
        investigation: String,
        workflow: String,
    },
}

#[derive(Debug, Subcommand)]
enum ConnectorCmd {
    /// List available connectors.
    List,
    /// Show connector status (no secrets).
    Status {
        /// Connector id: github_actions | kubernetes | sentry | github | local
        connector: String,
    },
    /// Test connector configuration.
    Test {
        connector: String,
        #[arg(long)]
        repository: Option<String>,
        #[arg(long)]
        namespace: Option<String>,
        #[arg(long)]
        organization: Option<String>,
        #[arg(long)]
        project: Option<String>,
    },
    /// Collect / preview normalized observations (fixture or live).
    Collect {
        connector: String,
        /// Fixture JSON path (offline).
        #[arg(long)]
        fixture: Option<PathBuf>,
        #[arg(long)]
        repository: Option<String>,
        #[arg(long)]
        namespace: Option<String>,
        #[arg(long)]
        organization: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        path: Option<PathBuf>,
        /// When set, ingest into this Investigation.
        #[arg(long)]
        investigation: Option<String>,
        /// Preview only (default true unless --investigation is set with --ingest).
        #[arg(long)]
        ingest: bool,
    },
}

#[derive(Debug, Subcommand)]
enum InvestigationCmd {
    /// Create a new Investigation.
    Create {
        title: String,
        #[arg(long)]
        description: Option<String>,
    },
    /// Show an Investigation.
    Show { id: String },
    /// List Investigations.
    List,
    /// Complete an Investigation (must be in Learning).
    Complete {
        id: String,
        #[arg(long)]
        reason: Option<String>,
    },
    /// Reopen a completed Investigation.
    Reopen {
        id: String,
        #[arg(long)]
        reason: Option<String>,
    },
    /// List Investigations related to this one (RFC-015).
    Related { id: String },
    /// Create an explicit link between two Investigations.
    Link {
        source: String,
        target: String,
        #[arg(long)]
        reason: Option<String>,
    },
    /// Remove an explicit link (derived relationships refresh instead).
    Unlink { relationship_id: String },
    /// Explain why two Investigations are related.
    Relationship { relationship_id: String },
    /// Re-derive relationships for an Investigation.
    RefreshRelationships { id: String },
    /// Confirm a relationship as relevant.
    ConfirmRelationship { relationship_id: String },
    /// Dismiss a relationship as irrelevant.
    DismissRelationship { relationship_id: String },
    /// Find Investigations similar to this one.
    Similar {
        id: String,
        #[arg(long)]
        limit: Option<usize>,
    },
    /// List Recalled Context for an Investigation (RFC-017).
    Context { id: String },
    /// Suggest Recalled Context from related / similar Investigations.
    ContextSuggest { id: String },
    /// Attach historical context from a source Investigation (or confirm a suggestion).
    ContextAttach {
        /// Current Investigation id.
        id: String,
        /// Source Investigation id (manual attach).
        #[arg(long)]
        source: Option<String>,
        /// Existing Recalled Context id (confirm suggested).
        #[arg(long)]
        context: Option<String>,
        /// Reason for attachment.
        #[arg(long)]
        reason: Option<String>,
    },
    /// Dismiss a Recalled Context record.
    ContextDismiss { id: String, context: String },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ImplementationSourceArg {
    HumanDeclared,
    GitCommit,
    PullRequest,
    Patch,
    Deployment,
    ConfigurationChange,
    RunbookExecution,
    ExternalAgent,
    Other,
}

impl From<ImplementationSourceArg> for ImplementationSource {
    fn from(value: ImplementationSourceArg) -> Self {
        match value {
            ImplementationSourceArg::HumanDeclared => Self::HumanDeclared,
            ImplementationSourceArg::GitCommit => Self::GitCommit,
            ImplementationSourceArg::PullRequest => Self::PullRequest,
            ImplementationSourceArg::Patch => Self::Patch,
            ImplementationSourceArg::Deployment => Self::Deployment,
            ImplementationSourceArg::ConfigurationChange => Self::ConfigurationChange,
            ImplementationSourceArg::RunbookExecution => Self::RunbookExecution,
            ImplementationSourceArg::ExternalAgent => Self::ExternalAgent,
            ImplementationSourceArg::Other => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ImplementationReferenceKindArg {
    CommitSha,
    PullRequest,
    Branch,
    DeploymentId,
    BuildId,
    IncidentId,
    WorkflowRun,
    ArtifactPath,
    ExternalUri,
    HumanNote,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutcomeEvidenceRelationArg {
    SupportsExpectedResult,
    ContradictsExpectedResult,
    IndicatesRegression,
    ConfirmsImplementation,
    DisputesImplementation,
    IsBaseline,
    IsPostChange,
    IsInconclusive,
    IsSuperseded,
    IsDismissed,
    Baseline,
    PostChange,
    Supports,
    Contradicts,
    Regression,
}

impl From<OutcomeEvidenceRelationArg> for OutcomeEvidenceRelation {
    fn from(value: OutcomeEvidenceRelationArg) -> Self {
        match value {
            OutcomeEvidenceRelationArg::SupportsExpectedResult
            | OutcomeEvidenceRelationArg::Supports => Self::SupportsExpectedResult,
            OutcomeEvidenceRelationArg::ContradictsExpectedResult
            | OutcomeEvidenceRelationArg::Contradicts => Self::ContradictsExpectedResult,
            OutcomeEvidenceRelationArg::IndicatesRegression
            | OutcomeEvidenceRelationArg::Regression => Self::IndicatesRegression,
            OutcomeEvidenceRelationArg::ConfirmsImplementation => Self::ConfirmsImplementation,
            OutcomeEvidenceRelationArg::DisputesImplementation => Self::DisputesImplementation,
            OutcomeEvidenceRelationArg::IsBaseline | OutcomeEvidenceRelationArg::Baseline => {
                Self::IsBaseline
            }
            OutcomeEvidenceRelationArg::IsPostChange | OutcomeEvidenceRelationArg::PostChange => {
                Self::IsPostChange
            }
            OutcomeEvidenceRelationArg::IsInconclusive => Self::IsInconclusive,
            OutcomeEvidenceRelationArg::IsSuperseded => Self::IsSuperseded,
            OutcomeEvidenceRelationArg::IsDismissed => Self::IsDismissed,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum LearningExportFormatArg {
    Markdown,
    Json,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DispositionArg {
    Accepted,
    Rejected,
    Ignored,
    Successful,
    Unsuccessful,
}

impl From<DispositionArg> for OutcomeDisposition {
    fn from(value: DispositionArg) -> Self {
        match value {
            DispositionArg::Accepted => Self::Accepted,
            DispositionArg::Rejected => Self::Rejected,
            DispositionArg::Ignored => Self::Ignored,
            DispositionArg::Successful => Self::Successful,
            DispositionArg::Unsuccessful => Self::Unsuccessful,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ProposalCategoryArg {
    Code,
    Configuration,
    Testing,
    Reliability,
    Performance,
    Security,
    Observability,
    Infrastructure,
    DeveloperExperience,
    Process,
    Documentation,
}

impl From<ProposalCategoryArg> for ProposalCategory {
    fn from(value: ProposalCategoryArg) -> Self {
        match value {
            ProposalCategoryArg::Code => Self::Code,
            ProposalCategoryArg::Configuration => Self::Configuration,
            ProposalCategoryArg::Testing => Self::Testing,
            ProposalCategoryArg::Reliability => Self::Reliability,
            ProposalCategoryArg::Performance => Self::Performance,
            ProposalCategoryArg::Security => Self::Security,
            ProposalCategoryArg::Observability => Self::Observability,
            ProposalCategoryArg::Infrastructure => Self::Infrastructure,
            ProposalCategoryArg::DeveloperExperience => Self::DeveloperExperience,
            ProposalCategoryArg::Process => Self::Process,
            ProposalCategoryArg::Documentation => Self::Documentation,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ProposalPriorityArg {
    Critical,
    High,
    Medium,
    Low,
    Exploratory,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ProposalExportFormatArg {
    Markdown,
    Json,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ProposalPortfolioStatusArg {
    Draft,
    Proposed,
    UnderReview,
    Accepted,
    Rejected,
    Deferred,
    Superseded,
    Withdrawn,
}

impl From<ProposalPortfolioStatusArg> for ProposalStatus {
    fn from(value: ProposalPortfolioStatusArg) -> Self {
        match value {
            ProposalPortfolioStatusArg::Draft => Self::Draft,
            ProposalPortfolioStatusArg::Proposed => Self::Proposed,
            ProposalPortfolioStatusArg::UnderReview => Self::UnderReview,
            ProposalPortfolioStatusArg::Accepted => Self::Accepted,
            ProposalPortfolioStatusArg::Rejected => Self::Rejected,
            ProposalPortfolioStatusArg::Deferred => Self::Deferred,
            ProposalPortfolioStatusArg::Superseded => Self::Superseded,
            ProposalPortfolioStatusArg::Withdrawn => Self::Withdrawn,
        }
    }
}

impl From<ProposalPriorityArg> for ProposalPriority {
    fn from(value: ProposalPriorityArg) -> Self {
        match value {
            ProposalPriorityArg::Critical => Self::Critical,
            ProposalPriorityArg::High => Self::High,
            ProposalPriorityArg::Medium => Self::Medium,
            ProposalPriorityArg::Low => Self::Low,
            ProposalPriorityArg::Exploratory => Self::Exploratory,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ProposalStatusArg {
    Proposed,
    UnderReview,
}

impl From<ProposalStatusArg> for ProposalStatus {
    fn from(value: ProposalStatusArg) -> Self {
        match value {
            ProposalStatusArg::Proposed => Self::Proposed,
            ProposalStatusArg::UnderReview => Self::UnderReview,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ProposalFeedbackCategoryArg {
    TooBroad,
    TooRisky,
    TooExpensive,
    InsufficientEvidence,
    WrongComponent,
    MissingAlternative,
    MissingTest,
    ViolatesArchitecture,
    ShouldSplit,
    ShouldCombine,
    NeedsVerification,
    Other,
}

impl From<ProposalFeedbackCategoryArg> for ProposalFeedbackCategory {
    fn from(value: ProposalFeedbackCategoryArg) -> Self {
        match value {
            ProposalFeedbackCategoryArg::TooBroad => Self::TooBroad,
            ProposalFeedbackCategoryArg::TooRisky => Self::TooRisky,
            ProposalFeedbackCategoryArg::TooExpensive => Self::TooExpensive,
            ProposalFeedbackCategoryArg::InsufficientEvidence => Self::InsufficientEvidence,
            ProposalFeedbackCategoryArg::WrongComponent => Self::WrongComponent,
            ProposalFeedbackCategoryArg::MissingAlternative => Self::MissingAlternative,
            ProposalFeedbackCategoryArg::MissingTest => Self::MissingTest,
            ProposalFeedbackCategoryArg::ViolatesArchitecture => Self::ViolatesArchitecture,
            ProposalFeedbackCategoryArg::ShouldSplit => Self::ShouldSplit,
            ProposalFeedbackCategoryArg::ShouldCombine => Self::ShouldCombine,
            ProposalFeedbackCategoryArg::NeedsVerification => Self::NeedsVerification,
            ProposalFeedbackCategoryArg::Other => Self::Other,
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run_cli(cli) {
        Ok(code) => code,
        Err(failure) => {
            if failure.json {
                eprintln!(
                    "{}",
                    serde_json::to_string_pretty(&failure.error.to_json_value()).unwrap_or_else(
                        |_| format!(r#"{{"error":true,"message":"{}"}}"#, failure.error)
                    )
                );
            } else {
                eprintln!(
                    "error: {} (code={}, class={}, retryable={})",
                    failure.error,
                    failure.error.code(),
                    failure.error.failure_class().as_str(),
                    failure.error.is_retryable()
                );
            }
            ExitCode::from(failure.error.exit_code().code())
        }
    }
}

/// Structured CLI failure carrying JSON preference.
struct CliFailure {
    error: RivoraError,
    json: bool,
}

fn run_cli(cli: Cli) -> Result<ExitCode, CliFailure> {
    let json = cli.json;
    match run(cli) {
        Ok(code) => Ok(code),
        Err(message) => Err(CliFailure {
            // Preserve structured errors when possible; otherwise map generic messages.
            error: classify_cli_message(&message),
            json,
        }),
    }
}

fn classify_cli_message(message: &str) -> RivoraError {
    let lower = message.to_ascii_lowercase();
    if lower.contains("store lock") || lower.contains("store already locked") {
        RivoraError::store_locked(message.to_string())
    } else if lower.contains("schema mismatch") {
        RivoraError::SchemaMismatch {
            found: 0,
            supported_max: 1,
        }
    } else if lower.contains("not found") {
        RivoraError::validation(message.to_string())
    } else if lower.contains("payload too large") {
        RivoraError::payload_too_large(message.to_string())
    } else if lower.contains("rate limit") {
        RivoraError::RateLimited(message.to_string())
    } else if lower.contains("timeout") {
        RivoraError::timeout(message.to_string())
    } else if lower.contains("auth") || lower.contains("token") && lower.contains("required") {
        RivoraError::auth_failure(message.to_string())
    } else if lower.contains("partial") {
        RivoraError::partial(message.to_string())
    } else if lower.contains("unsupported") {
        RivoraError::unsupported(message.to_string())
    } else if lower.contains("policy") {
        RivoraError::PolicyDenial(message.to_string())
    } else {
        RivoraError::validation(message.to_string())
    }
}

fn run(cli: Cli) -> Result<ExitCode, String> {
    // Bare invocation: launch the shared Workspace (canonical interactive entrypoint).
    let Some(command) = cli.command else {
        if cli.json {
            return Err(
                "--json requires a CLI subcommand; bare `rivora` launches the interactive Workspace"
                    .to_string(),
            );
        }
        run_workspace(WorkspaceLaunchConfig::interactive(cli.data_dir))?;
        return Ok(ExitCode::SUCCESS);
    };

    // Doctor recover-lock must run before open (lock may block open).
    if let Commands::Doctor {
        action: DoctorCmd::RecoverLock,
    } = &command
    {
        let recovered = rivora::storage::LocalStore::recover_stale_lock(&cli.data_dir)
            .map_err(|e| e.to_string())?;
        if cli.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "recovered": recovered,
                    "data_dir": cli.data_dir.display().to_string(),
                }))
                .map_err(|e| e.to_string())?
            );
        } else if recovered {
            println!("Recovered stale lock at {}", cli.data_dir.display());
        } else {
            println!("No lock file to recover at {}", cli.data_dir.display());
        }
        return Ok(ExitCode::SUCCESS);
    }

    let caps = open_capabilities(&cli.data_dir)?;

    let mut exit = ExitCode::SUCCESS;
    match command {
        Commands::Doctor { action } => match action {
            DoctorCmd::Health => {
                let report = caps.store_health().map_err(err)?;
                print_value(cli.json, &report, || {
                    format!(
                        "Store health\n  root: {}\n  schema: {}\n  lock_held: {}\n  investigations: {}\n  observations: {}\n  memory: {}\n  corrupt: {}\n  disk_bytes: {}\n  migration: {}",
                        report.root,
                        report.schema_version,
                        report.lock_held,
                        report.investigation_count,
                        report.observation_count,
                        report.memory_count,
                        report.corrupt_records.len(),
                        report.disk_bytes,
                        report.migration_status
                    )
                });
                if !report.is_healthy() {
                    exit = ExitCode::from(CliExitCode::CorruptStore.code());
                }
            }
            DoctorCmd::Export { out } => {
                let export = caps.diagnostic_export().map_err(err)?;
                let text = serde_json::to_string_pretty(&export).map_err(|e| e.to_string())?;
                if let Some(path) = out {
                    std::fs::write(&path, text.as_bytes()).map_err(|e| e.to_string())?;
                    if !cli.json {
                        println!("Wrote diagnostic export to {}", path.display());
                    } else {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&serde_json::json!({
                                "wrote": path.display().to_string()
                            }))
                            .map_err(|e| e.to_string())?
                        );
                    }
                } else {
                    println!("{text}");
                }
            }
            DoctorCmd::Backup { dest } => {
                caps.backup_store(&dest).map_err(err)?;
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "backup": dest.display().to_string()
                        }))
                        .map_err(|e| e.to_string())?
                    );
                } else {
                    println!("Backup written to {}", dest.display());
                }
            }
            DoctorCmd::RebuildIndexes => {
                let n = caps.rebuild_observation_indexes().map_err(err)?;
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({ "rebuilt": n }))
                            .map_err(|e| e.to_string())?
                    );
                } else {
                    println!("Rebuilt {n} observation idempotency index entries");
                }
            }
            DoctorCmd::RecoverLock => {
                // Handled before open_capabilities.
            }
            DoctorCmd::Envelope { profile } => {
                let profile = match profile.to_ascii_lowercase().as_str() {
                    "small" => OperatingProfile::Small,
                    "large" | "large_supported" => OperatingProfile::LargeSupported,
                    _ => OperatingProfile::Medium,
                };
                let envelope = OperatingEnvelope::for_profile(profile);
                print_value(cli.json, &envelope, || {
                    format!(
                        "Operating envelope ({})\n  max investigations/store: {}\n  max observations/investigation: {}\n  max payload bytes: {}\n  max connector latency ms: {}\n  concurrent writers: {}",
                        envelope.profile.as_str(),
                        envelope.max_investigations_per_store,
                        envelope.max_observations_per_investigation,
                        envelope.max_payload_bytes,
                        envelope.max_connector_latency_ms,
                        envelope.max_concurrent_writers
                    )
                });
            }
            DoctorCmd::Budgets => {
                let budgets = PerformanceBudget::v0_9_budgets();
                print_value(cli.json, &budgets, || {
                    let mut lines = vec!["Performance budgets (v0.9):".to_string()];
                    for b in &budgets {
                        lines.push(format!(
                            "  {} target={}ms max={}ms variance={}",
                            b.scenario, b.target_ms, b.max_ms, b.variance_tolerance
                        ));
                    }
                    lines.join("\n")
                });
            }
            DoctorCmd::ReplayContracts => {
                let contracts = ReplayContract::v0_9_contracts();
                print_value(cli.json, &contracts, || {
                    let mut lines = vec!["Replay contracts (v0.9):".to_string()];
                    for c in &contracts {
                        lines.push(format!(
                            "  {} — reuses_lineage={} dry_run_suppresses_live={}",
                            c.operation, c.reuses_lineage, c.dry_run_suppresses_live
                        ));
                    }
                    lines.join("\n")
                });
            }
            DoctorCmd::ExitCodes => {
                let codes = vec![
                    ("success", CliExitCode::Success.code()),
                    ("internal", CliExitCode::Internal.code()),
                    ("validation", CliExitCode::Validation.code()),
                    ("not_found", CliExitCode::NotFound.code()),
                    ("unsupported", CliExitCode::Unsupported.code()),
                    ("blocked", CliExitCode::Blocked.code()),
                    ("partial", CliExitCode::Partial.code()),
                    ("provider_failure", CliExitCode::ProviderFailure.code()),
                    ("auth_failure", CliExitCode::AuthFailure.code()),
                    ("timeout", CliExitCode::Timeout.code()),
                    ("corrupt_store", CliExitCode::CorruptStore.code()),
                    ("schema_mismatch", CliExitCode::SchemaMismatch.code()),
                    ("lock_conflict", CliExitCode::LockConflict.code()),
                    ("policy_denial", CliExitCode::PolicyDenial.code()),
                    (
                        "verification_failure",
                        CliExitCode::VerificationFailure.code(),
                    ),
                ];
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&codes).map_err(|e| e.to_string())?
                    );
                } else {
                    println!("CLI exit codes:");
                    for (name, code) in codes {
                        println!("  {code:>3}  {name}");
                    }
                }
            }
        },
        Commands::Investigation { action } => match action {
            InvestigationCmd::Create { title, description } => {
                let inv = caps
                    .create_investigation(title, description, "cli")
                    .map_err(err)?;
                print_value(cli.json, &inv, || {
                    format!(
                        "Created investigation {}\n  title: {}\n  status: {}",
                        inv.id, inv.title, inv.status
                    )
                });
            }
            InvestigationCmd::Show { id } => {
                let id = parse_inv(&id)?;
                let inv = caps.open_investigation(id).map_err(err)?;
                let memory = caps.recall_memory(id).map_err(err)?;
                let knowledge = caps.list_knowledge(id).map_err(err)?;
                let evaluations = caps.list_evaluations(id).map_err(err)?;
                let verifications = caps.list_verifications(id).map_err(err)?;
                let recommendations = caps.list_recommendations(id).map_err(err)?;
                let learning = caps.list_learning(id).map_err(err)?;
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "investigation": inv,
                            "memory_count": memory.len(),
                            "knowledge_count": knowledge.len(),
                            "evaluation_count": evaluations.len(),
                            "verification_count": verifications.len(),
                            "recommendation_count": recommendations.len(),
                            "learning_count": learning.len(),
                        }))
                        .map_err(|e| e.to_string())?
                    );
                } else {
                    println!("Investigation {}", inv.id);
                    println!("  title:  {}", inv.title);
                    println!("  status: {}", inv.status);
                    if let Some(d) = &inv.description {
                        println!("  description: {d}");
                    }
                    println!("  memory: {}", memory.len());
                    println!("  knowledge: {}", knowledge.len());
                    println!("  evaluations: {}", evaluations.len());
                    println!("  verifications: {}", verifications.len());
                    println!("  recommendations: {}", recommendations.len());
                    println!("  learning: {}", learning.len());
                    if !inv.transitions.is_empty() {
                        println!("  transitions:");
                        for t in &inv.transitions {
                            println!("    {} → {} ({})", t.from, t.to, t.at.to_rfc3339());
                        }
                    }
                }
            }
            InvestigationCmd::List => {
                let ids = caps.list_investigations().map_err(err)?;
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&ids).map_err(|e| e.to_string())?
                    );
                } else if ids.is_empty() {
                    println!("No investigations found.");
                } else {
                    let total = ids.len();
                    let limit = rivora::DEFAULT_LIST_LIMIT.min(total);
                    for id in ids.into_iter().take(limit) {
                        let inv = caps.open_investigation(id).map_err(err)?;
                        println!("{}  [{}]  {}", inv.id, inv.status, inv.title);
                    }
                    if total > limit {
                        println!(
                            "… showing {limit} of {total} (default page size {}); narrow with search",
                            rivora::DEFAULT_LIST_LIMIT
                        );
                    }
                }
            }
            InvestigationCmd::Complete { id, reason } => {
                let inv = caps
                    .complete_investigation(parse_inv(&id)?, reason)
                    .map_err(err)?;
                print_value(cli.json, &inv, || {
                    format!("Completed investigation {} ({})", inv.id, inv.status)
                });
            }
            InvestigationCmd::Reopen { id, reason } => {
                let inv = caps
                    .reopen_investigation(parse_inv(&id)?, reason)
                    .map_err(err)?;
                print_value(cli.json, &inv, || {
                    format!("Reopened investigation {} ({})", inv.id, inv.status)
                });
            }
            InvestigationCmd::Related { id } => {
                let related = caps
                    .list_related_investigations(parse_inv(&id)?)
                    .map_err(err)?;
                print_value(cli.json, &related, || {
                    if related.is_empty() {
                        "No related Investigations.".into()
                    } else {
                        related
                            .iter()
                            .map(|r| {
                                format!(
                                    "{}  [{}]  {}  [{}]  {}  (confidence {:.0}%, {})",
                                    r.relationship.id,
                                    r.relationship.kind.as_str(),
                                    r.related.id,
                                    r.related.status,
                                    r.related.title,
                                    r.relationship.confidence.value() * 100.0,
                                    r.relationship.confirmation.state.as_str()
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    }
                });
            }
            InvestigationCmd::Link {
                source,
                target,
                reason,
            } => {
                let relationship = caps
                    .link_investigations(parse_inv(&source)?, parse_inv(&target)?, reason, "cli")
                    .map_err(err)?;
                print_value(cli.json, &relationship, || {
                    format!(
                        "Linked {} ↔ {} ({})",
                        relationship.source_investigation_id,
                        relationship.target_investigation_id,
                        relationship.id
                    )
                });
            }
            InvestigationCmd::Unlink { relationship_id } => {
                let id = parse_obj(&relationship_id)?;
                caps.unlink_investigation(id, "cli").map_err(err)?;
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({ "unlinked": id }))
                            .map_err(|e| e.to_string())?
                    );
                } else {
                    println!("Unlinked {id}");
                }
            }
            InvestigationCmd::Relationship { relationship_id } => {
                let explanation = caps
                    .explain_relationship(parse_obj(&relationship_id)?)
                    .map_err(err)?;
                print_value(cli.json, &explanation, || explanation.explanation.clone());
            }
            InvestigationCmd::RefreshRelationships { id } => {
                let relationships = caps
                    .refresh_relationships(parse_inv(&id)?, "cli")
                    .map_err(err)?;
                print_value(cli.json, &relationships, || {
                    let mut out = format!("{} relationship(s):", relationships.len());
                    for r in &relationships {
                        out.push_str(&format!(
                            "\n  [{}]  {} ↔ {}  (confidence {:.0}%, {})",
                            r.kind.as_str(),
                            r.source_investigation_id,
                            r.target_investigation_id,
                            r.confidence.value() * 100.0,
                            r.confirmation.state.as_str()
                        ));
                    }
                    out
                });
            }
            InvestigationCmd::ConfirmRelationship { relationship_id } => {
                let relationship = caps
                    .confirm_relationship(parse_obj(&relationship_id)?, "cli")
                    .map_err(err)?;
                print_value(cli.json, &relationship, || {
                    format!("Confirmed relationship {}", relationship.id)
                });
            }
            InvestigationCmd::DismissRelationship { relationship_id } => {
                let relationship = caps
                    .dismiss_relationship(parse_obj(&relationship_id)?, "cli")
                    .map_err(err)?;
                print_value(cli.json, &relationship, || {
                    format!("Dismissed relationship {}", relationship.id)
                });
            }
            InvestigationCmd::Similar { id, limit } => {
                let results = caps
                    .find_similar_investigations(parse_inv(&id)?, limit)
                    .map_err(err)?;
                print_value(cli.json, &results, || print_search_results(&results));
            }
            InvestigationCmd::Context { id } => {
                let contexts = caps.list_recalled_context(parse_inv(&id)?).map_err(err)?;
                print_value(cli.json, &contexts, || print_recalled_contexts(&contexts));
            }
            InvestigationCmd::ContextSuggest { id } => {
                let contexts = caps
                    .suggest_recalled_context(parse_inv(&id)?, "cli")
                    .map_err(err)?;
                print_value(cli.json, &contexts, || print_recalled_contexts(&contexts));
            }
            InvestigationCmd::ContextAttach {
                id,
                source,
                context,
                reason,
            } => {
                let inv = parse_inv(&id)?;
                let attached = match (source, context) {
                    (Some(source), None) => caps
                        .attach_recalled_context_from_source(
                            inv,
                            parse_inv(&source)?,
                            reason,
                            "cli",
                        )
                        .map_err(err)?,
                    (None, Some(context_id)) => caps
                        .attach_recalled_context(inv, parse_obj(&context_id)?, "cli")
                        .map_err(err)?,
                    _ => {
                        return Err(
                            "context-attach requires exactly one of --source or --context".into(),
                        );
                    }
                };
                print_value(cli.json, &attached, || {
                    format!(
                        "Attached context {} from {} ({})",
                        attached.id,
                        attached.source_investigation_id,
                        attached.state.as_str()
                    )
                });
            }
            InvestigationCmd::ContextDismiss { id, context } => {
                let dismissed = caps
                    .dismiss_recalled_context(parse_inv(&id)?, parse_obj(&context)?, "cli")
                    .map_err(err)?;
                print_value(cli.json, &dismissed, || {
                    format!(
                        "Dismissed context {} ({})",
                        dismissed.id,
                        dismissed.state.as_str()
                    )
                });
            }
        },
        Commands::Observe {
            investigation,
            summary,
            kind,
            payload,
            source,
            idempotency_key,
            local,
            github,
            pr,
            github_fixture,
        } => {
            let inv_id = parse_inv(&investigation)?;
            let mut observations: Vec<NormalizedObservation> = Vec::new();

            if let Some(path) = local {
                let connector = LocalConnector::new(path);
                observations.extend(connector.observe().map_err(|e| e.to_string())?);
            }
            if let Some(fixture_path) = github_fixture {
                let raw = std::fs::read_to_string(fixture_path).map_err(|e| e.to_string())?;
                let fixture: serde_json::Value =
                    serde_json::from_str(&raw).map_err(|e| e.to_string())?;
                observations.extend(
                    GitHubConnector::observe_from_fixture(&fixture).map_err(|e| e.to_string())?,
                );
            } else if let Some(repo) = github {
                let mut connector = GitHubConnector::new(repo);
                if let Some(n) = pr {
                    connector = connector.with_pull_request(n);
                }
                observations.extend(connector.observe().map_err(|e| e.to_string())?);
            }

            if let Some(summary) = summary {
                let payload_value: serde_json::Value =
                    serde_json::from_str(&payload).map_err(|e| format!("payload json: {e}"))?;
                observations.push(NormalizedObservation::new(
                    parse_kind(&kind),
                    summary,
                    payload_value,
                    source,
                    Utc::now(),
                    idempotency_key,
                    "cli",
                ));
            }

            if observations.is_empty() {
                return Err(
                    "provide --summary, --local <path>, --github <owner/repo>, or --github-fixture"
                        .into(),
                );
            }

            let mut ingested = Vec::new();
            for obs in observations {
                let (observation, memory, replay) = caps
                    .ingest_observation(
                        inv_id,
                        obs.kind,
                        obs.summary,
                        obs.payload,
                        obs.source,
                        obs.observed_at,
                        obs.idempotency_key,
                        "cli",
                    )
                    .map_err(err)?;
                ingested.push(serde_json::json!({
                    "observation_id": observation.id,
                    "memory_id": memory.id,
                    "summary": observation.summary,
                    "idempotent_replay": replay,
                }));
            }

            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&ingested).map_err(|e| e.to_string())?
                );
            } else {
                println!("Ingested {} observation(s):", ingested.len());
                for item in ingested {
                    println!(
                        "  {}  {}{}",
                        item["observation_id"],
                        item["summary"].as_str().unwrap_or(""),
                        if item["idempotent_replay"].as_bool() == Some(true) {
                            " (replay)"
                        } else {
                            ""
                        }
                    );
                }
            }
        }
        Commands::Recall {
            investigation,
            evidence,
            repository,
            outcome,
            similar_to,
        } => {
            let has_outcome_filters =
                repository.is_some() || outcome.is_some() || similar_to.is_some();
            match (investigation, evidence, has_outcome_filters) {
                (Some(id), true, _) => {
                    let recalled = caps.recall_related_evidence(parse_inv(&id)?).map_err(err)?;
                    print_value(cli.json, &recalled, || {
                        if recalled.is_empty() {
                            "No related evidence.".into()
                        } else {
                            recalled
                                .iter()
                                .map(|r| {
                                    format!(
                                        "[{}] from {}\n  {}",
                                        r.relationship_kind.as_str(),
                                        r.investigation_id,
                                        r.explanation.lines().next().unwrap_or("")
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("\n")
                        }
                    });
                }
                (Some(id), false, false) => {
                    let memory = caps.recall_memory(parse_inv(&id)?).map_err(err)?;
                    print_value(cli.json, &memory, || {
                        if memory.is_empty() {
                            "No memory records.".into()
                        } else {
                            memory
                                .iter()
                                .map(|m| {
                                    format!(
                                        "{}  {}  {}",
                                        m.recorded_at.to_rfc3339(),
                                        m.id,
                                        m.summary
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("\n")
                        }
                    });
                }
                (None, false, true) => {
                    let outcomes = caps
                        .recall_prior_outcomes(OutcomeFilter {
                            repository,
                            similar_to: similar_to.map(|s| parse_inv(&s)).transpose()?,
                            disposition: outcome.map(Into::into),
                        })
                        .map_err(err)?;
                    print_value(cli.json, &outcomes, || {
                        if outcomes.is_empty() {
                            "No prior outcomes.".into()
                        } else {
                            outcomes
                                .iter()
                                .map(|o| {
                                    format!(
                                        "{}  [{}]  {} — {}{}",
                                        o.investigation_id,
                                        o.outcome.disposition.as_str(),
                                        o.investigation_title,
                                        o.outcome.notes,
                                        o.recommendation_summary
                                            .as_ref()
                                            .map(|s| format!(" (re: {s})"))
                                            .unwrap_or_default()
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("\n")
                        }
                    });
                }
                _ => {
                    return Err("provide --investigation, --investigation with --evidence, \
                         or outcome filters (--repository/--outcome/--similar-to)"
                        .into())
                }
            }
        }
        Commands::Search {
            query,
            repository,
            status,
            source,
            verification,
            outcome,
            file,
            relationship,
            after,
            before,
            limit,
            explain,
        } => {
            let search_query = SearchQuery {
                text: query,
                investigation_id: None,
                repository,
                status: status.map(|s| parse_status(&s)).transpose()?,
                connector_source: source,
                verification_result: verification.map(|v| parse_verification(&v)).transpose()?,
                outcome: outcome.map(Into::into),
                relationship_kind: relationship
                    .map(|r| parse_relationship_kind(&r))
                    .transpose()?,
                file,
                created_after: after.map(|d| parse_datetime(&d)).transpose()?,
                created_before: before.map(|d| parse_datetime(&d)).transpose()?,
                // Default to the supported envelope list limit so CLI never
                // silently dumps unbounded result sets.
                limit: Some(
                    limit
                        .unwrap_or(rivora::DEFAULT_LIST_LIMIT)
                        .min(rivora::MAX_LIST_LIMIT),
                ),
            };
            if let Some(id) = explain {
                let result = caps
                    .explain_search_result(parse_inv(&id)?, search_query)
                    .map_err(err)?;
                print_value(cli.json, &result, || {
                    format!(
                        "{}  [{}]  {}\n  score: {:.2}\n  {}",
                        result.investigation_id,
                        result.status,
                        result.title,
                        result.score,
                        result.explanation
                    )
                });
            } else {
                let results = caps.search_investigations(search_query).map_err(err)?;
                print_value(cli.json, &results, || print_search_results(&results));
            }
        }
        Commands::Timeline { investigation } => {
            let timeline = caps
                .generate_timeline(parse_inv(&investigation)?)
                .map_err(err)?;
            print_value(cli.json, &timeline, || {
                timeline
                    .iter()
                    .map(|e| format!("{}  [{}]  {}", e.at.to_rfc3339(), e.source, e.summary))
                    .collect::<Vec<_>>()
                    .join("\n")
            });
        }
        Commands::Knowledge { investigation } => {
            let knowledge = caps
                .derive_knowledge(parse_inv(&investigation)?, "cli")
                .map_err(err)?;
            print_value(cli.json, &knowledge, || {
                knowledge
                    .iter()
                    .map(|k| {
                        format!(
                            "{}  [{:?}]  {} (confidence {:.0}%)",
                            k.id,
                            k.kind,
                            k.summary,
                            k.confidence.value() * 100.0
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            });
        }
        Commands::Evaluate { investigation } => {
            let evaluations = caps
                .evaluate_investigation(parse_inv(&investigation)?, "cli")
                .map_err(err)?;
            print_value(cli.json, &evaluations, || {
                evaluations
                    .iter()
                    .map(|e| {
                        format!(
                            "{}  [{:?}/{}]  {}\n    {}",
                            e.id,
                            e.assessment_type,
                            e.severity.as_str(),
                            e.summary,
                            e.explanation
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            });
        }
        Commands::Verify {
            investigation,
            evaluation,
        } => {
            let inv = parse_inv(&investigation)?;
            if let Some(eval) = evaluation {
                let receipt = caps
                    .verify_conclusion(inv, Some(parse_obj(&eval)?), "cli")
                    .map_err(err)?;
                print_value(cli.json, &receipt, || {
                    format!(
                        "{}  {}  {}\n  {}",
                        receipt.id,
                        receipt.result.as_str(),
                        receipt.subject,
                        receipt.reason
                    )
                });
            } else {
                let receipts = caps.verify_all(inv, "cli").map_err(err)?;
                print_value(cli.json, &receipts, || {
                    receipts
                        .iter()
                        .map(|r| {
                            format!(
                                "{}  {}  {}\n  {}",
                                r.id,
                                r.result.as_str(),
                                r.subject,
                                r.reason
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                });
            }
        }
        Commands::Recommend { investigation } => {
            let recs = caps
                .generate_recommendation(parse_inv(&investigation)?, "cli")
                .map_err(err)?;
            print_value(cli.json, &recs, || {
                recs.iter()
                    .map(|r| {
                        format!(
                            "{}  [{}]  {}\n  rationale: {}",
                            r.id,
                            r.status.as_str(),
                            r.summary,
                            r.rationale
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            });
        }
        Commands::RecordOutcome {
            investigation,
            recommendation,
            disposition,
            notes,
            impact,
        } => {
            let outcome = caps
                .record_outcome(
                    parse_inv(&investigation)?,
                    recommendation.map(|s| parse_obj(&s)).transpose()?,
                    disposition.into(),
                    notes,
                    impact,
                    "cli",
                )
                .map_err(err)?;
            print_value(cli.json, &outcome, || {
                format!(
                    "Recorded learning {} ({})",
                    outcome.id,
                    outcome.disposition.as_str()
                )
            });
        }
        Commands::Pipeline { investigation } => {
            let result = caps
                .run_full_pipeline(parse_inv(&investigation)?, "cli")
                .map_err(err)?;
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "knowledge": result.knowledge,
                        "evaluations": result.evaluations,
                        "verifications": result.verifications,
                        "recommendations": result.recommendations,
                    }))
                    .map_err(|e| e.to_string())?
                );
            } else {
                println!("Knowledge: {}", result.knowledge.len());
                println!("Evaluations: {}", result.evaluations.len());
                println!("Verifications: {}", result.verifications.len());
                println!("Recommendations: {}", result.recommendations.len());
                if let Some(rec) = result.recommendations.first() {
                    println!("\nTop recommendation:\n  {}", rec.summary);
                    println!("  {}", rec.rationale);
                }
            }
        }
        Commands::Patterns => {
            let patterns = caps.detect_patterns("cli").map_err(err)?;
            print_value(cli.json, &patterns, || {
                if patterns.is_empty() {
                    "No patterns detected.".into()
                } else {
                    patterns
                        .iter()
                        .map(|p| {
                            format!(
                                "[{}]  {}  ({} investigations, confidence {:.0}%)\n  {}",
                                p.kind.as_str(),
                                p.signature,
                                p.occurrence_count,
                                p.confidence.value() * 100.0,
                                p.description
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            });
        }
        Commands::Trends { repository } => {
            let trend = caps.summarize_historical_trend(repository).map_err(err)?;
            print_value(cli.json, &trend, || {
                let mut out = trend.summary.clone();
                if !trend.top_failure_signatures.is_empty() {
                    out.push_str("\nTop failure signatures:");
                    for item in &trend.top_failure_signatures {
                        out.push_str(&format!("\n  {} ({})", item.label, item.count));
                    }
                }
                out
            });
        }
        Commands::Assist { action } => match action {
            AssistCmd::Intents => {
                let defs = caps.list_composite_capabilities();
                print_value(cli.json, &defs, || {
                    defs.iter()
                        .map(|d| {
                            format!(
                                "{} — {}\n  cores: {}",
                                d.id,
                                d.description,
                                d.core_capabilities.join(" → ")
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                });
            }
            AssistCmd::Plan {
                intent,
                investigation,
            } => {
                let wf = caps
                    .plan_workflow(parse_inv(&investigation)?, intent, "cli")
                    .map_err(err)?;
                print_value(cli.json, &wf, || {
                    format!(
                        "Planned workflow {} intent={} steps={}",
                        wf.id,
                        wf.intent,
                        wf.steps.len()
                    )
                });
            }
            AssistCmd::Investigate { investigation } => {
                let wf = caps
                    .run_composite(
                        parse_inv(&investigation)?,
                        "investigate_engineering_problem",
                        "cli",
                    )
                    .map_err(err)?;
                print_value(cli.json, &wf, || {
                    format!(
                        "Workflow {} status={}\n{}",
                        wf.id,
                        wf.status.as_str(),
                        wf.summary.as_deref().unwrap_or("")
                    )
                });
            }
            AssistCmd::Readiness { investigation } => {
                let id = parse_inv(&investigation)?;
                let wf = caps
                    .run_composite(id, "assess_deployment_readiness", "cli")
                    .map_err(err)?;
                print_value(cli.json, &wf, || {
                    format!(
                        "Readiness workflow {} status={}\n{}",
                        wf.id,
                        wf.status.as_str(),
                        wf.summary.as_deref().unwrap_or("")
                    )
                });
            }
            AssistCmd::ExplainFailure { investigation } => {
                let wf = caps
                    .run_composite(parse_inv(&investigation)?, "explain_failure", "cli")
                    .map_err(err)?;
                print_value(cli.json, &wf, || {
                    format!(
                        "Explain-failure workflow {} status={}\n{}",
                        wf.id,
                        wf.status.as_str(),
                        wf.summary.as_deref().unwrap_or("")
                    )
                });
            }
            AssistCmd::Hypotheses { investigation } => {
                let hyps = caps
                    .generate_hypotheses(parse_inv(&investigation)?, "cli")
                    .map_err(err)?;
                print_value(cli.json, &hyps, || {
                    hyps.iter()
                        .map(|h| {
                            format!(
                                "{}. [{} conf={:.0}%] {}",
                                h.rank,
                                h.status.as_str(),
                                h.confidence.value() * 100.0,
                                h.statement
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                });
            }
            AssistCmd::NextVerification { investigation } => {
                let suggestions = caps
                    .recommend_next_verification(parse_inv(&investigation)?, "cli")
                    .map_err(err)?;
                print_value(cli.json, &suggestions, || {
                    suggestions
                        .iter()
                        .map(|s| {
                            format!(
                                "{}. {} — {} ({})",
                                s.rank,
                                s.claim,
                                s.method,
                                s.feasibility.as_str()
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                });
            }
            AssistCmd::Risks { investigation } => {
                let forecast = caps
                    .forecast_risk(parse_inv(&investigation)?, "cli")
                    .map_err(err)?;
                print_value(cli.json, &forecast, || {
                    let mut out = forecast.summary.clone();
                    for item in &forecast.items {
                        out.push_str(&format!(
                            "\n- {} [{}]: {}",
                            item.category.as_str(),
                            item.severity.as_str(),
                            item.mitigation
                        ));
                    }
                    out
                });
            }
            AssistCmd::RootCause { investigation } => {
                let guidance = caps
                    .generate_root_cause_guidance(parse_inv(&investigation)?, "cli")
                    .map_err(err)?;
                print_value(cli.json, &guidance, || guidance.guidance.clone());
            }
            AssistCmd::Prioritize { investigation } => {
                let ranked = caps
                    .prioritize_recommendations(parse_inv(&investigation)?, "cli")
                    .map_err(err)?;
                print_value(cli.json, &ranked, || {
                    ranked
                        .iter()
                        .map(|r| {
                            format!(
                                "{}. score={:.3} {} — {}",
                                r.rank, r.score, r.summary, r.explanation
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                });
            }
            AssistCmd::Summarize { investigation } => {
                let summary = caps
                    .summarize_investigation_state(parse_inv(&investigation)?, "cli")
                    .map_err(err)?;
                print_value(cli.json, &summary, || summary.summary.clone());
            }
            AssistCmd::Workflow { action } => match action {
                WorkflowCmd::Show {
                    investigation,
                    workflow,
                } => {
                    let wf = caps
                        .open_workflow(parse_inv(&investigation)?, parse_obj(&workflow)?)
                        .map_err(err)?;
                    print_value(cli.json, &wf, || {
                        format!(
                            "Workflow {} intent={} status={}\n{}",
                            wf.id,
                            wf.intent,
                            wf.status.as_str(),
                            wf.summary.as_deref().unwrap_or("")
                        )
                    });
                }
                WorkflowCmd::List { investigation } => {
                    let list = caps
                        .list_workflows(parse_inv(&investigation)?)
                        .map_err(err)?;
                    print_value(cli.json, &list, || {
                        list.iter()
                            .map(|w| {
                                format!(
                                    "{}  {}  [{}]  steps={}",
                                    w.id,
                                    w.intent,
                                    w.status.as_str(),
                                    w.steps.len()
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    });
                }
                WorkflowCmd::Resume {
                    investigation,
                    workflow,
                } => {
                    let wf = caps
                        .resume_workflow(parse_inv(&investigation)?, parse_obj(&workflow)?, "cli")
                        .map_err(err)?;
                    print_value(cli.json, &wf, || {
                        format!("Resumed {} status={}", wf.id, wf.status.as_str())
                    });
                }
                WorkflowCmd::Cancel {
                    investigation,
                    workflow,
                    reason,
                } => {
                    let wf = caps
                        .cancel_workflow(
                            parse_inv(&investigation)?,
                            parse_obj(&workflow)?,
                            reason,
                            "cli",
                        )
                        .map_err(err)?;
                    print_value(cli.json, &wf, || {
                        format!("Cancelled {} status={}", wf.id, wf.status.as_str())
                    });
                }
                WorkflowCmd::Explain {
                    investigation,
                    workflow,
                } => {
                    let text = caps
                        .explain_workflow(parse_inv(&investigation)?, parse_obj(&workflow)?)
                        .map_err(err)?;
                    if cli.json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(
                                &serde_json::json!({ "explanation": text })
                            )
                            .map_err(|e| e.to_string())?
                        );
                    } else {
                        println!("{text}");
                    }
                }
            },
        },
        Commands::Connector { action } => match action {
            ConnectorCmd::List => {
                let list = vec![
                    ConnectorStatusReport {
                        id: "local".into(),
                        category: "local".into(),
                        configured: true,
                        read_only: true,
                        details: "local project observer".into(),
                    },
                    ConnectorStatusReport {
                        id: "github".into(),
                        category: "code".into(),
                        configured: std::env::var("GITHUB_TOKEN").is_ok(),
                        read_only: true,
                        details: "GitHub repository/PR observer".into(),
                    },
                    GitHubActionsConnector::new("owner/repo").status(),
                    KubernetesConnector::new("default").status(),
                    SentryConnector::new("org", "project").status(),
                ];
                print_value(cli.json, &list, || {
                    list.iter()
                        .map(|c| {
                            format!(
                                "{}  [{}]  configured={}  read_only={}  {}",
                                c.id, c.category, c.configured, c.read_only, c.details
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                });
            }
            ConnectorCmd::Status { connector } => {
                let report = connector_status(&connector)?;
                print_value(cli.json, &report, || {
                    format!(
                        "{} [{}] configured={} read_only={}\n{}",
                        report.id,
                        report.category,
                        report.configured,
                        report.read_only,
                        report.details
                    )
                });
            }
            ConnectorCmd::Test {
                connector,
                repository,
                namespace,
                organization,
                project,
            } => {
                let msg = connector_test(
                    &connector,
                    repository.as_deref(),
                    namespace.as_deref(),
                    organization.as_deref(),
                    project.as_deref(),
                )?;
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({ "result": msg }))
                            .map_err(|e| e.to_string())?
                    );
                } else {
                    println!("{msg}");
                }
            }
            ConnectorCmd::Collect {
                connector,
                fixture,
                repository,
                namespace,
                organization,
                project,
                path,
                investigation,
                ingest,
            } => {
                let observations = connector_collect(
                    &connector,
                    fixture.as_ref(),
                    repository.as_deref(),
                    namespace.as_deref(),
                    organization.as_deref(),
                    project.as_deref(),
                    path.as_ref(),
                )?;
                if let Some(inv) = investigation {
                    if ingest {
                        let inv_id = parse_inv(&inv)?;
                        let mut receipts = Vec::new();
                        for obs in &observations {
                            let (observation, memory, replay) = caps
                                .ingest_observation(
                                    inv_id,
                                    obs.kind.clone(),
                                    obs.summary.clone(),
                                    obs.payload.clone(),
                                    obs.source.clone(),
                                    obs.observed_at,
                                    obs.idempotency_key.clone(),
                                    "cli",
                                )
                                .map_err(err)?;
                            receipts.push(serde_json::json!({
                                "observation_id": observation.id,
                                "memory_id": memory.id,
                                "summary": observation.summary,
                                "idempotent_replay": replay,
                            }));
                        }
                        print_value(cli.json, &receipts, || {
                            format!("Ingested {} observation(s).", receipts.len())
                        });
                    } else {
                        print_value(
                            cli.json,
                            &observations
                                .iter()
                                .map(|o| {
                                    serde_json::json!({
                                        "kind": o.kind.as_str(),
                                        "summary": o.summary,
                                        "source": o.source,
                                        "idempotency_key": o.idempotency_key,
                                    })
                                })
                                .collect::<Vec<_>>(),
                            || {
                                format!(
                                    "Preview {} observation(s). Pass --ingest to write Memory.",
                                    observations.len()
                                )
                            },
                        );
                    }
                } else {
                    print_value(
                        cli.json,
                        &observations
                            .iter()
                            .map(|o| {
                                serde_json::json!({
                                    "kind": o.kind.as_str(),
                                    "summary": o.summary,
                                    "source": o.source,
                                    "idempotency_key": o.idempotency_key,
                                })
                            })
                            .collect::<Vec<_>>(),
                        || {
                            observations
                                .iter()
                                .map(|o| format!("[{}] {}", o.kind.as_str(), o.summary))
                                .collect::<Vec<_>>()
                                .join("\n")
                        },
                    );
                }
            }
        },
        Commands::Report { investigation } => {
            let report = caps
                .generate_engineering_report(parse_inv(&investigation)?, "cli")
                .map_err(err)?;
            if cli.json {
                print_value(true, &report, String::new);
            } else {
                println!("{}", report.markdown);
            }
        }
        Commands::Proposal { action } => match action {
            ProposalCmd::Generate { investigation } => {
                let proposals = caps
                    .generate_improvement_proposals(parse_inv(&investigation)?, "cli")
                    .map_err(err)?;
                print_generated_proposals(cli.json, &proposals);
            }
            ProposalCmd::Alternatives { investigation } => {
                let proposals = caps
                    .generate_proposal_alternatives(parse_inv(&investigation)?, "cli")
                    .map_err(err)?;
                print_generated_proposals(cli.json, &proposals);
            }
            ProposalCmd::Compare {
                investigation,
                proposals,
            } => {
                let proposal_ids = proposals
                    .iter()
                    .map(|proposal| parse_obj(proposal))
                    .collect::<Result<Vec<_>, _>>()?;
                let comparison = caps
                    .compare_improvement_proposals(parse_inv(&investigation)?, proposal_ids)
                    .map_err(err)?;
                print_proposal_comparison(cli.json, &comparison);
            }
            ProposalCmd::Prioritize { investigation } => {
                let comparison = caps
                    .prioritize_improvement_proposals(parse_inv(&investigation)?)
                    .map_err(err)?;
                print_proposal_comparison(cli.json, &comparison);
            }
            ProposalCmd::VerificationPlan {
                proposal,
                investigation,
            } => {
                let plan = caps
                    .generate_proposal_verification_plan(
                        parse_inv(&investigation)?,
                        parse_obj(&proposal)?,
                    )
                    .map_err(err)?;
                print_proposal_value(cli.json, &plan, || {
                    let mut sections = Vec::new();
                    sections.push(format!("Claims:\n{}", print_lines(&plan.claims)));
                    sections.push(format!(
                        "Preconditions:\n{}",
                        print_lines(&plan.preconditions)
                    ));
                    sections.push(format!("Tests:\n{}", print_lines(&plan.tests)));
                    sections.push(format!("Checks:\n{}", print_lines(&plan.checks)));
                    sections.push(format!(
                        "Success criteria:\n{}",
                        print_lines(&plan.success_criteria)
                    ));
                    sections.push(format!(
                        "Failure criteria:\n{}",
                        print_lines(&plan.failure_criteria)
                    ));
                    sections
                        .push("Verification Plan is proposed work; it was not executed.".into());
                    sections.push(PROPOSAL_BOUNDARY.into());
                    sections.join("\n")
                });
            }
            ProposalCmd::ImplementationPlan {
                proposal,
                investigation,
            } => {
                let outline = caps
                    .generate_proposal_implementation_outline(
                        parse_inv(&investigation)?,
                        parse_obj(&proposal)?,
                    )
                    .map_err(err)?;
                if cli.json {
                    print_value(
                        true,
                        &serde_json::json!({
                            "outline": outline,
                            "boundary": PROPOSAL_BOUNDARY,
                        }),
                        String::new,
                    );
                } else {
                    println!("Implementation outline:\n{}", print_lines(&outline));
                    println!("Expected scope only; no implementation was applied.");
                    println!("{PROPOSAL_BOUNDARY}");
                }
            }
            ProposalCmd::Provenance {
                proposal,
                investigation,
            } => {
                let explanation = caps
                    .explain_improvement_proposal_provenance(
                        parse_inv(&investigation)?,
                        parse_obj(&proposal)?,
                    )
                    .map_err(err)?;
                if cli.json {
                    print_value(
                        true,
                        &serde_json::json!({
                            "provenance": explanation,
                            "boundary": PROPOSAL_BOUNDARY,
                        }),
                        String::new,
                    );
                } else {
                    println!("{explanation}");
                }
            }
            ProposalCmd::Export {
                proposal,
                investigation,
                format,
            } => {
                let artifact = caps
                    .generate_proposal_artifact(
                        parse_inv(&investigation)?,
                        parse_obj(&proposal)?,
                        "cli",
                    )
                    .map_err(err)?;
                match format {
                    ProposalExportFormatArg::Markdown => println!("{}", artifact.markdown),
                    ProposalExportFormatArg::Json => {
                        print_value(true, &artifact, String::new);
                    }
                }
            }
            ProposalCmd::Handoff {
                proposal,
                investigation,
            } => {
                let handoff = caps
                    .generate_coding_agent_handoff(
                        parse_inv(&investigation)?,
                        parse_obj(&proposal)?,
                    )
                    .map_err(err)?;
                println!("{handoff}");
            }
            ProposalCmd::Portfolio {
                investigation,
                status,
                priority,
                category,
                source_recommendation,
                affected_component,
                unresolved_high_priority,
            } => {
                let proposals = caps
                    .proposal_portfolio(
                        parse_inv(&investigation)?,
                        ProposalPortfolioFilter {
                            status: status.map(Into::into),
                            priority: priority.map(Into::into),
                            category: category.map(Into::into),
                            source_recommendation_id: source_recommendation
                                .as_deref()
                                .map(parse_obj)
                                .transpose()?,
                            affected_component,
                            unresolved_high_priority,
                        },
                    )
                    .map_err(err)?;
                if cli.json {
                    print_value(
                        true,
                        &serde_json::json!({
                            "proposals": proposals,
                            "boundary": PROPOSAL_BOUNDARY,
                        }),
                        String::new,
                    );
                } else {
                    if proposals.is_empty() {
                        println!("No matching Improvement Proposals.");
                    } else {
                        for proposal in proposals {
                            println!(
                                "{}  [{} / {}]  {}  (revision {})",
                                proposal.id,
                                proposal.status.as_str(),
                                proposal.priority.as_str(),
                                proposal.title,
                                proposal.revision_number,
                            );
                        }
                    }
                    println!("{PROPOSAL_BOUNDARY}");
                }
            }
            ProposalCmd::Trace {
                proposal,
                investigation,
            } => {
                let trace = caps
                    .trace_improvement_proposal(parse_inv(&investigation)?, parse_obj(&proposal)?)
                    .map_err(err)?;
                print_proposal_value(cli.json, &trace, || {
                    format!(
                        "Observation ({}) → Memory ({}) → Knowledge ({}) → Evaluation ({}) → Verification ({}) → Recommendation ({}) → Improvement Proposal {}\n{}\n{}",
                        trace.observation_ids.len(),
                        trace.memory_ids.len(),
                        trace.knowledge_ids.len(),
                        trace.evaluation_ids.len(),
                        trace.verification_ids.len(),
                        trace.recommendation_ids.len(),
                        trace.proposal_id,
                        trace.explanation,
                        PROPOSAL_BOUNDARY,
                    )
                });
            }
            ProposalCmd::Create {
                investigation,
                title,
                summary,
                rationale,
                category,
                priority,
                confidence,
                supporting_evidence,
                contradicting_evidence,
                source_recommendations,
                affected_components,
                affected_resources,
            } => {
                if !confidence.is_finite() || !(0.0..=1.0).contains(&confidence) {
                    return Err("proposal confidence must be between 0.0 and 1.0".into());
                }
                let proposal = caps
                    .create_improvement_proposal(
                        parse_inv(&investigation)?,
                        CreateProposalRequest {
                            title,
                            summary,
                            rationale,
                            category: category.into(),
                            priority: priority.into(),
                            confidence: Confidence::new(confidence),
                            supporting_evidence_ids: supporting_evidence
                                .iter()
                                .map(|value| parse_obj(value))
                                .collect::<Result<Vec<_>, _>>()?,
                            contradicting_evidence_ids: contradicting_evidence
                                .iter()
                                .map(|value| parse_obj(value))
                                .collect::<Result<Vec<_>, _>>()?,
                            source_recommendation_ids: source_recommendations
                                .iter()
                                .map(|value| parse_obj(value))
                                .collect::<Result<Vec<_>, _>>()?,
                            affected_components,
                            affected_resources,
                        },
                        "cli",
                    )
                    .map_err(err)?;
                print_proposal_value(cli.json, &proposal, || print_proposal(&proposal));
            }
            ProposalCmd::List { investigation } => {
                let listing = caps
                    .list_improvement_proposals(parse_inv(&investigation)?)
                    .map_err(err)?;
                print_proposal_value(cli.json, &listing, || {
                    let mut output = if listing.proposals.is_empty() {
                        "No Improvement Proposals.".into()
                    } else {
                        listing
                            .proposals
                            .iter()
                            .map(|proposal| {
                                format!(
                                    "{}  [{} / {}]  {}  (revision {})",
                                    proposal.id,
                                    proposal.status.as_str(),
                                    proposal.priority.as_str(),
                                    proposal.title,
                                    proposal.revision_number,
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    };
                    if !listing.diagnostics.is_empty() {
                        output.push_str(&format!(
                            "\nWarning: {} corrupted Proposal record(s) isolated.",
                            listing.diagnostics.len()
                        ));
                    }
                    output.push('\n');
                    output.push_str(PROPOSAL_BOUNDARY);
                    output
                });
            }
            ProposalCmd::Show {
                proposal,
                investigation,
            } => {
                let proposal = caps
                    .get_improvement_proposal(parse_inv(&investigation)?, parse_obj(&proposal)?)
                    .map_err(err)?;
                print_proposal_value(cli.json, &proposal, || print_proposal(&proposal));
            }
            ProposalCmd::Explain {
                proposal,
                investigation,
            } => {
                let explanation = caps
                    .explain_improvement_proposal(parse_inv(&investigation)?, parse_obj(&proposal)?)
                    .map_err(err)?;
                print_value(cli.json, &explanation, || explanation.clone());
            }
            ProposalCmd::Status {
                proposal,
                investigation,
                status,
                reason,
            } => {
                let proposal =
                    transition_proposal(&caps, &investigation, &proposal, status.into(), reason)?;
                print_proposal_value(cli.json, &proposal, || print_proposal(&proposal));
            }
            ProposalCmd::Accept {
                proposal,
                investigation,
                reason,
            } => {
                let proposal = transition_proposal(
                    &caps,
                    &investigation,
                    &proposal,
                    ProposalStatus::Accepted,
                    reason,
                )?;
                print_proposal_value(cli.json, &proposal, || print_proposal(&proposal));
            }
            ProposalCmd::Reject {
                proposal,
                investigation,
                reason,
            } => {
                let proposal = transition_proposal(
                    &caps,
                    &investigation,
                    &proposal,
                    ProposalStatus::Rejected,
                    reason,
                )?;
                print_proposal_value(cli.json, &proposal, || print_proposal(&proposal));
            }
            ProposalCmd::Defer {
                proposal,
                investigation,
                reason,
            } => {
                let proposal = transition_proposal(
                    &caps,
                    &investigation,
                    &proposal,
                    ProposalStatus::Deferred,
                    reason,
                )?;
                print_proposal_value(cli.json, &proposal, || print_proposal(&proposal));
            }
            ProposalCmd::Withdraw {
                proposal,
                investigation,
                reason,
            } => {
                let proposal = transition_proposal(
                    &caps,
                    &investigation,
                    &proposal,
                    ProposalStatus::Withdrawn,
                    reason,
                )?;
                print_proposal_value(cli.json, &proposal, || print_proposal(&proposal));
            }
            ProposalCmd::Supersede {
                proposal,
                investigation,
                replacement,
                reason,
            } => {
                let proposal = caps
                    .supersede_improvement_proposal(
                        parse_inv(&investigation)?,
                        parse_obj(&proposal)?,
                        parse_obj(&replacement)?,
                        "cli",
                        reason,
                    )
                    .map_err(err)?;
                print_proposal_value(cli.json, &proposal, || print_proposal(&proposal));
            }
            ProposalCmd::Refine {
                proposal,
                investigation,
                title,
                summary,
                rationale,
                affected_components,
                tests,
                reason,
            } => {
                let request = RefineProposalRequest {
                    title,
                    summary,
                    rationale,
                    affected_components: (!affected_components.is_empty())
                        .then_some(affected_components),
                    test_strategy: (!tests.is_empty()).then_some(tests),
                };
                let proposal = caps
                    .refine_improvement_proposal(
                        parse_inv(&investigation)?,
                        parse_obj(&proposal)?,
                        request,
                        "cli",
                        reason,
                    )
                    .map_err(err)?;
                print_proposal_value(cli.json, &proposal, || print_proposal(&proposal));
            }
            ProposalCmd::Feedback {
                proposal,
                investigation,
                category,
                comment,
            } => {
                let proposal = caps
                    .add_improvement_proposal_feedback(
                        parse_inv(&investigation)?,
                        parse_obj(&proposal)?,
                        category.into(),
                        comment,
                        "cli",
                    )
                    .map_err(err)?;
                print_proposal_value(cli.json, &proposal, || print_proposal(&proposal));
            }
            ProposalCmd::Revisions {
                lineage,
                investigation,
            } => {
                let listing = caps
                    .list_improvement_proposal_revisions(
                        parse_inv(&investigation)?,
                        parse_obj(&lineage)?,
                    )
                    .map_err(err)?;
                print_proposal_value(cli.json, &listing, || {
                    let mut output = listing
                        .proposals
                        .iter()
                        .map(|proposal| {
                            format!(
                                "revision {}  {}  [{}]  {}",
                                proposal.revision_number,
                                proposal.id,
                                proposal.status.as_str(),
                                proposal.title,
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    output.push('\n');
                    output.push_str(PROPOSAL_BOUNDARY);
                    output
                });
            }
        },
        Commands::Execute { action } => match action {
            ExecuteCmd::Capabilities => {
                let list = caps.list_execution_capabilities();
                print_value(cli.json, &list, || {
                    let mut out = list
                        .iter()
                        .map(|c| {
                            format!(
                                "{}  risk={}  dry_run={}  actions=[{}]\n  {}",
                                c.capability_id,
                                c.risk_level.as_str(),
                                c.supports_dry_run,
                                c.supported_actions.join(", "),
                                c.description
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    out.push('\n');
                    out.push_str(EXECUTION_BOUNDARY);
                    out
                });
            }
            ExecuteCmd::Capability { id } => {
                let desc = caps.show_execution_capability(&id).map_err(err)?;
                print_value(cli.json, &desc, || format_capability_descriptor(&desc));
            }
            ExecuteCmd::Plan {
                investigation,
                proposal,
                capability,
                target_system,
                environment,
                action,
                inputs,
                action_inputs,
                precondition,
                actor,
            } => {
                let inputs_val: serde_json::Value = serde_json::from_str(&inputs).map_err(err)?;
                if !action_inputs.is_empty() && action_inputs.len() != action.len() {
                    return Err(format!(
                        "--action-input count ({}) must match --action count ({})",
                        action_inputs.len(),
                        action.len()
                    ));
                }
                let action_values = if action_inputs.is_empty() {
                    vec![inputs_val.clone(); action.len()]
                } else {
                    action_inputs
                        .iter()
                        .map(|value| serde_json::from_str(value).map_err(err))
                        .collect::<Result<Vec<serde_json::Value>, String>>()?
                };
                let actions = action
                    .into_iter()
                    .zip(action_values)
                    .enumerate()
                    .map(|(index, (action_name, inputs))| ExecutionAction {
                        action_id: format!("a{}", index + 1),
                        action_name,
                        inputs,
                        continue_on_failure: false,
                    })
                    .collect();
                let preconditions = precondition
                    .iter()
                    .map(|value| serde_json::from_str::<ExecutionPrecondition>(value).map_err(err))
                    .collect::<Result<Vec<_>, String>>()?;
                let plan = caps
                    .create_execution_plan(
                        parse_inv(&investigation)?,
                        CreateExecutionPlanRequest {
                            proposal_id: parse_obj(&proposal)?,
                            capability_id: capability,
                            target_system,
                            target_environment: environment,
                            actions,
                            inputs: inputs_val,
                            expected_effects: vec![],
                            preconditions,
                            supports_dry_run: true,
                        },
                        actor,
                    )
                    .map_err(err)?;
                print_value(cli.json, &plan, || {
                    format!(
                        "Execution Plan {} rev {} [{}]\n  capability: {}\n  env: {}\n  proposal: {}\n{}",
                        plan.id,
                        plan.revision_number,
                        plan.status.as_str(),
                        plan.capability_id,
                        plan.target_environment,
                        plan.proposal_id,
                        EXECUTION_BOUNDARY
                    )
                });
            }
            ExecuteCmd::Validate {
                investigation,
                plan,
                reason,
                actor,
            } => {
                let plan = caps
                    .validate_execution_plan(
                        parse_inv(&investigation)?,
                        parse_obj(&plan)?,
                        actor,
                        reason,
                    )
                    .map_err(err)?;
                print_value(cli.json, &plan, || {
                    format!(
                        "Plan {} → {} (rev {})\n{}",
                        plan.id,
                        plan.status.as_str(),
                        plan.revision_number,
                        EXECUTION_BOUNDARY
                    )
                });
            }
            ExecuteCmd::Preview {
                investigation,
                plan,
            } => {
                let preview = caps
                    .preview_execution_plan(parse_inv(&investigation)?, parse_obj(&plan)?)
                    .map_err(err)?;
                print_value(cli.json, &preview, || {
                    format!(
                        "Preview target: {}\n  mutations: {}\n  simulated: {}\n  policy: {}\n{}",
                        preview.target,
                        preview.expected_mutations.join("; "),
                        preview.simulated,
                        preview.policy_decision.decision.as_str(),
                        EXECUTION_BOUNDARY
                    )
                });
            }
            ExecuteCmd::Approve {
                investigation,
                plan,
                reason,
                actor,
                one_time,
            } => {
                let (plan, approval) = caps
                    .approve_execution_plan(
                        parse_inv(&investigation)?,
                        parse_obj(&plan)?,
                        actor,
                        reason,
                        vec![],
                        vec![],
                        None,
                        one_time,
                    )
                    .map_err(err)?;
                print_value(
                    cli.json,
                    &serde_json::json!({"plan": plan, "approval": approval}),
                    || {
                        format!(
                            "Approved plan {} rev {}\n  approval: {}\n  one_time: {}\n{}",
                            plan.id,
                            plan.revision_number,
                            approval.id,
                            approval.one_time,
                            EXECUTION_BOUNDARY
                        )
                    },
                );
            }
            ExecuteCmd::Reject {
                investigation,
                plan,
                reason,
                actor,
            } => {
                let plan = caps
                    .reject_execution_plan(
                        parse_inv(&investigation)?,
                        parse_obj(&plan)?,
                        actor,
                        reason,
                    )
                    .map_err(err)?;
                print_value(cli.json, &plan, || {
                    format!(
                        "Plan {} → {}\n{}",
                        plan.id,
                        plan.status.as_str(),
                        EXECUTION_BOUNDARY
                    )
                });
            }
            ExecuteCmd::Run {
                investigation,
                plan,
                approval,
                idempotency_key,
                actor,
                dry_run,
                confirm,
            } => {
                if !dry_run && !confirm {
                    return Err("live execution requires --confirm (or pass --dry-run)".into());
                }
                let attempt = caps
                    .execute_plan(
                        parse_inv(&investigation)?,
                        parse_obj(&plan)?,
                        parse_obj(&approval)?,
                        actor,
                        idempotency_key,
                        dry_run,
                    )
                    .map_err(err)?;
                print_value(cli.json, &attempt, || {
                    format!(
                        "Attempt {} [{}] dry_run={}\n  completed: {:?}\n  failed: {:?}\n  retry_safety: {}\n{}",
                        attempt.id,
                        attempt.status.as_str(),
                        attempt.dry_run,
                        attempt.completed_actions,
                        attempt.failed_actions,
                        attempt.retry_safety.as_str(),
                        EXECUTION_BOUNDARY
                    )
                });
            }
            ExecuteCmd::Plans { investigation } => {
                let listing = caps
                    .list_execution_plans(parse_inv(&investigation)?)
                    .map_err(err)?;
                print_value(cli.json, &listing, || {
                    let mut out = listing
                        .plans
                        .iter()
                        .map(|p| {
                            format!(
                                "{} rev {} [{}] {}",
                                p.id,
                                p.revision_number,
                                p.status.as_str(),
                                p.capability_id
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    out.push('\n');
                    out.push_str(EXECUTION_BOUNDARY);
                    out
                });
            }
            ExecuteCmd::Revisions {
                investigation,
                plan,
            } => {
                let investigation_id = parse_inv(&investigation)?;
                let snapshot = caps
                    .get_execution_plan(investigation_id, parse_obj(&plan)?)
                    .map_err(err)?;
                let listing = caps
                    .list_execution_plan_revisions(investigation_id, snapshot.lineage_id)
                    .map_err(err)?;
                print_value(cli.json, &listing, || {
                    let mut out = listing
                        .plans
                        .iter()
                        .map(|revision| {
                            format!(
                                "revision {}  {}  [{}]  capability={} target={}:{}",
                                revision.revision_number,
                                revision.id,
                                revision.status.as_str(),
                                revision.capability_id,
                                revision.target_system,
                                revision.target_environment
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    out.push('\n');
                    out.push_str(EXECUTION_BOUNDARY);
                    out
                });
            }
            ExecuteCmd::Show {
                investigation,
                plan,
            } => {
                let plan = caps
                    .get_execution_plan(parse_inv(&investigation)?, parse_obj(&plan)?)
                    .map_err(err)?;
                print_value(cli.json, &plan, || {
                    format!(
                        "Plan {} rev {} [{}]\n  capability: {}\n  env: {}\n  actions: {}\n{}",
                        plan.id,
                        plan.revision_number,
                        plan.status.as_str(),
                        plan.capability_id,
                        plan.target_environment,
                        plan.actions
                            .iter()
                            .map(|a| a.action_name.clone())
                            .collect::<Vec<_>>()
                            .join(", "),
                        EXECUTION_BOUNDARY
                    )
                });
            }
            ExecuteCmd::Attempts { investigation } => {
                let listing = caps
                    .list_execution_attempts(parse_inv(&investigation)?)
                    .map_err(err)?;
                print_value(cli.json, &listing, || {
                    listing
                        .attempts
                        .iter()
                        .map(|a| {
                            format!(
                                "{} [{}] plan={} key={}",
                                a.id,
                                a.status.as_str(),
                                a.plan_id,
                                a.idempotency_key
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                });
            }
            ExecuteCmd::Attempt {
                investigation,
                attempt,
            } => {
                let attempt = caps
                    .get_execution_attempt(parse_inv(&investigation)?, parse_obj(&attempt)?)
                    .map_err(err)?;
                print_value(cli.json, &attempt, || {
                    format!(
                        "Attempt {} [{}]\n  completed: {:?}\n  failed: {:?}\n  skipped: {:?}\n  uncertain: {:?}\n  retry safety: {}\n  receipts: {}\n  errors: {}\n{}",
                        attempt.id,
                        attempt.status.as_str(),
                        attempt.completed_actions,
                        attempt.failed_actions,
                        attempt.skipped_actions,
                        attempt.uncertain_actions,
                        attempt.retry_safety.as_str(),
                        attempt.receipt_ids.len(),
                        attempt.errors.join("; "),
                        EXECUTION_BOUNDARY
                    )
                });
            }
            ExecuteCmd::Verify {
                investigation,
                attempt,
                actor,
            } => {
                let v = caps
                    .verify_execution_attempt(
                        parse_inv(&investigation)?,
                        parse_obj(&attempt)?,
                        actor,
                    )
                    .map_err(err)?;
                print_value(cli.json, &v, || {
                    format!(
                        "Verification {} [{}] confidence={:.2}\n  contradictions: {}\n{}",
                        v.id,
                        v.status.as_str(),
                        v.confidence.value(),
                        v.contradictions.join("; "),
                        EXECUTION_BOUNDARY
                    )
                });
            }
            ExecuteCmd::Trace {
                investigation,
                plan,
            } => {
                let trace = caps
                    .trace_execution(parse_inv(&investigation)?, parse_obj(&plan)?)
                    .map_err(err)?;
                print_value(cli.json, &trace, || {
                    format!(
                        "Plan {} rev {} [{}]\n  approvals: {}\n  attempts: {}\n  receipts: {}\n  verifications: {}\n  implementation record: {}\n  measured outcome: {}\n  {}\n{}",
                        trace.plan_id,
                        trace.plan_revision_number,
                        trace.plan_status.as_str(),
                        trace.approval_ids.len(),
                        trace.attempt_ids.len(),
                        trace.receipt_ids.len(),
                        trace.verification_ids.len(),
                        trace
                            .implementation_record_id
                            .map(|id| id.to_string())
                            .unwrap_or_else(|| "-".into()),
                        trace
                            .measured_outcome_id
                            .map(|id| id.to_string())
                            .unwrap_or_else(|| "-".into()),
                        trace.explanation,
                        EXECUTION_BOUNDARY
                    )
                });
            }
            ExecuteCmd::Policy {
                investigation,
                plan,
            } => {
                let policy = caps
                    .explain_execution_policy(parse_inv(&investigation)?, parse_obj(&plan)?)
                    .map_err(err)?;
                print_value(cli.json, &policy, || {
                    format!(
                        "Policy: {} (risk {})\n  dry_run: {} live: {}\n  {}\n{}",
                        policy.decision.as_str(),
                        policy.risk_level.as_str(),
                        policy.dry_run_permitted,
                        policy.live_execution_permitted,
                        policy.reasons.join("; "),
                        EXECUTION_BOUNDARY
                    )
                });
            }
            ExecuteCmd::Cancel {
                investigation,
                plan,
                reason,
                actor,
            } => {
                let plan = caps
                    .cancel_execution_plan(
                        parse_inv(&investigation)?,
                        parse_obj(&plan)?,
                        actor,
                        reason,
                    )
                    .map_err(err)?;
                print_value(cli.json, &plan, || {
                    format!(
                        "Cancelled plan {} rev {} [{}]\n{}",
                        plan.id,
                        plan.revision_number,
                        plan.status.as_str(),
                        EXECUTION_BOUNDARY
                    )
                });
            }
            ExecuteCmd::RollbackPlan {
                investigation,
                attempt,
                actor,
            } => {
                let plan = caps
                    .create_rollback_plan(parse_inv(&investigation)?, parse_obj(&attempt)?, actor)
                    .map_err(err)?;
                print_value(cli.json, &plan, || {
                    format!(
                        "Rollback plan {} rev {} [{}]\n  capability: {}\n  target: {}:{}\n  approval required before execution\n{}",
                        plan.id,
                        plan.revision_number,
                        plan.status.as_str(),
                        plan.capability_id,
                        plan.target_system,
                        plan.target_environment,
                        EXECUTION_BOUNDARY
                    )
                });
            }
            ExecuteCmd::Receipts { investigation } => {
                let listing = caps
                    .list_execution_receipts(parse_inv(&investigation)?)
                    .map_err(err)?;
                print_value(cli.json, &listing, || {
                    let mut out = listing
                        .receipts
                        .iter()
                        .map(|receipt| {
                            format!(
                                "{} [{}] attempt={} action={} capability={}",
                                receipt.id,
                                receipt.result_status.as_str(),
                                receipt.attempt_id,
                                receipt.action_name,
                                receipt.capability_id
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    out.push('\n');
                    out.push_str(EXECUTION_BOUNDARY);
                    out
                });
            }
            ExecuteCmd::ExportReceipt {
                investigation,
                receipt,
            } => {
                let json = caps
                    .export_execution_receipt(parse_inv(&investigation)?, parse_obj(&receipt)?)
                    .map_err(err)?;
                println!("{json}");
            }
            ExecuteCmd::Export {
                investigation,
                plan,
            } => {
                let json = caps
                    .export_execution_plan(parse_inv(&investigation)?, parse_obj(&plan)?)
                    .map_err(err)?;
                println!("{json}");
            }
            ExecuteCmd::LinkImplementation {
                investigation,
                attempt,
                summary,
                actor,
            } => {
                let record = caps
                    .link_execution_to_implementation(
                        parse_inv(&investigation)?,
                        parse_obj(&attempt)?,
                        actor,
                        summary,
                    )
                    .map_err(err)?;
                print_value(cli.json, &record, || {
                    format!(
                        "Implementation Record {} linked from execution\n{}",
                        record.id, EXECUTION_BOUNDARY
                    )
                });
            }
            ExecuteCmd::Revise {
                investigation,
                plan,
                reason,
                inputs,
                actor,
            } => {
                let inputs_val: serde_json::Value = serde_json::from_str(&inputs).map_err(err)?;
                let plan = caps
                    .revise_execution_plan(
                        parse_inv(&investigation)?,
                        parse_obj(&plan)?,
                        ReviseExecutionPlanRequest {
                            inputs: Some(inputs_val),
                            ..Default::default()
                        },
                        actor,
                        reason,
                    )
                    .map_err(err)?;
                print_value(cli.json, &plan, || {
                    format!(
                        "Revised plan {} rev {} [{}]\n{}",
                        plan.id,
                        plan.revision_number,
                        plan.status.as_str(),
                        EXECUTION_BOUNDARY
                    )
                });
            }
        },
        Commands::Capability { action } => match action {
            CapabilityCmd::List => {
                let list = caps.list_execution_capabilities();
                print_value(cli.json, &list, || {
                    list.iter()
                        .map(|c| {
                            format!(
                                "{}  v{}  risk={}  loop=[M:{} E:{} V:{} I:{} L:{}]\n  {}",
                                c.capability_id,
                                c.version,
                                c.risk_level.as_str(),
                                c.engineering_loop.memory.as_str(),
                                c.engineering_loop.evaluation.as_str(),
                                c.engineering_loop.verification.as_str(),
                                c.engineering_loop.improvement.as_str(),
                                c.engineering_loop.learning.as_str(),
                                c.description
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                });
            }
            CapabilityCmd::Show { id } => {
                let desc = caps.show_execution_capability(&id).map_err(err)?;
                print_value(cli.json, &desc, || format_capability_descriptor(&desc));
            }
            CapabilityCmd::Coverage => {
                let report = caps.capability_coverage_report();
                print_value(cli.json, &report, || format_capability_coverage(&report));
            }
            CapabilityCmd::Route {
                investigation,
                observations,
            } => {
                let inv = parse_inv(&investigation)?;
                let obs_ids: Result<Vec<_>, _> =
                    observations.iter().map(|s| parse_obj(s)).collect();
                let obs_ids = obs_ids?;
                let decision = caps
                    .route_observations_to_capabilities(inv, &obs_ids)
                    .map_err(err)?;
                print_value(cli.json, &decision, || {
                    let mut out = format!(
                        "Routing decision\n  unsupported: {}\n  ambiguous: {}\n  input_types: [{}]\n",
                        decision.unsupported,
                        decision.ambiguous,
                        decision.input_types.join(", ")
                    );
                    for m in &decision.matches {
                        out.push_str(&format!(
                            "  match rank={}  {} v{}  types=[{}]\n    {}\n",
                            m.rank,
                            m.capability_id,
                            m.version,
                            m.matched_input_types.join(", "),
                            m.reason
                        ));
                    }
                    for r in &decision.reasons {
                        out.push_str(&format!("  reason: {r}\n"));
                    }
                    out
                });
            }
            CapabilityCmd::Lifecycle {
                investigation,
                attempt,
            } => {
                let run = caps
                    .run_capability_lifecycle_for_attempt(
                        parse_inv(&investigation)?,
                        parse_obj(&attempt)?,
                        "cli",
                    )
                    .map_err(err)?;
                print_value(cli.json, &run, || format_lifecycle_run(&run));
            }
            CapabilityCmd::LifecycleList { investigation } => {
                let listing = caps
                    .list_lifecycle_runs(parse_inv(&investigation)?)
                    .map_err(err)?;
                print_value(cli.json, &listing, || {
                    if listing.runs.is_empty() {
                        return "No Engineering Loop runs.".into();
                    }
                    listing
                        .runs
                        .iter()
                        .map(|r| {
                            format!(
                                "{}  lineage={} rev={}  [{}]  cap={}  inv={}",
                                r.id,
                                r.lineage_id,
                                r.revision_number,
                                r.status.as_str(),
                                r.capability_id,
                                r.invocation_id
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                });
            }
            CapabilityCmd::LifecycleShow { investigation, run } => {
                let run = caps
                    .get_lifecycle_run(parse_inv(&investigation)?, parse_obj(&run)?)
                    .map_err(err)?;
                print_value(cli.json, &run, || format_lifecycle_run(&run));
            }
            CapabilityCmd::Trace { investigation, id } => {
                let trace = caps
                    .trace_capability_lifecycle(parse_inv(&investigation)?, &id)
                    .map_err(err)?;
                print_value(cli.json, &trace, || format_lifecycle_trace(&trace));
            }
        },
        Commands::Implementation { action } => match action {
            ImplementationCmd::Record {
                investigation,
                proposal,
                source,
                summary,
                reference,
                reference_kind,
                commit_sha,
                pr,
                note,
                observed_file,
                observed_component,
                declared_scope,
                actor,
            } => {
                let mut references = Vec::new();
                if let Some(sha) = commit_sha {
                    references.push(ImplementationReference::CommitSha { sha });
                }
                if let Some(pr_ref) = pr {
                    references.push(ImplementationReference::PullRequest { reference: pr_ref });
                }
                if let Some(note_text) = note {
                    references.push(ImplementationReference::HumanNote { note: note_text });
                }
                if let (Some(value), Some(kind)) = (reference, reference_kind) {
                    references.push(build_implementation_reference(kind, value)?);
                }
                let record = caps
                    .record_external_implementation(
                        parse_inv(&investigation)?,
                        parse_obj(&proposal)?,
                        RecordImplementationRequest {
                            source: source.into(),
                            summary,
                            references,
                            implemented_at: None,
                            observed_files: observed_file,
                            observed_components: observed_component,
                            declared_scope,
                        },
                        actor,
                    )
                    .map_err(err)?;
                print_learning_value(cli.json, &record, || print_implementation(&record));
            }
            ImplementationCmd::List { investigation } => {
                let listing = caps
                    .list_implementation_records(parse_inv(&investigation)?)
                    .map_err(err)?;
                print_learning_value(cli.json, &listing, || {
                    let mut output = if listing.records.is_empty() {
                        "No Implementation Records.".into()
                    } else {
                        listing
                            .records
                            .iter()
                            .map(|record| {
                                format!(
                                    "{}  [{} / {}]  {}  (revision {})",
                                    record.id,
                                    record.status.as_str(),
                                    record.source.as_str(),
                                    record.summary,
                                    record.revision_number,
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    };
                    output.push('\n');
                    output.push_str(LEARNING_BOUNDARY);
                    output
                });
            }
            ImplementationCmd::Show {
                investigation,
                implementation,
            } => {
                let record = caps
                    .get_implementation_record(
                        parse_inv(&investigation)?,
                        parse_obj(&implementation)?,
                    )
                    .map_err(err)?;
                print_learning_value(cli.json, &record, || print_implementation(&record));
            }
            ImplementationCmd::Revise {
                investigation,
                implementation,
                summary,
                reason,
                actor,
            } => {
                let record = caps
                    .revise_implementation_record(
                        parse_inv(&investigation)?,
                        parse_obj(&implementation)?,
                        ReviseImplementationRequest {
                            summary,
                            ..ReviseImplementationRequest::default()
                        },
                        actor,
                        reason,
                    )
                    .map_err(err)?;
                print_learning_value(cli.json, &record, || print_implementation(&record));
            }
            ImplementationCmd::EvidenceAdd {
                investigation,
                implementation,
                evidence,
                reason,
                actor,
            } => {
                let evidence_ids = evidence
                    .iter()
                    .map(|value| parse_obj(value))
                    .collect::<Result<Vec<_>, _>>()?;
                let record = caps
                    .link_implementation_evidence(
                        parse_inv(&investigation)?,
                        parse_obj(&implementation)?,
                        evidence_ids,
                        actor,
                        reason,
                    )
                    .map_err(err)?;
                print_learning_value(cli.json, &record, || print_implementation(&record));
            }
            ImplementationCmd::Ready {
                investigation,
                implementation,
                reason,
                actor,
            } => {
                let record = caps
                    .mark_implementation_ready(
                        parse_inv(&investigation)?,
                        parse_obj(&implementation)?,
                        actor,
                        reason,
                    )
                    .map_err(err)?;
                print_learning_value(cli.json, &record, || print_implementation(&record));
            }
            ImplementationCmd::Withdraw {
                investigation,
                implementation,
                reason,
                actor,
            } => {
                let record = caps
                    .withdraw_implementation(
                        parse_inv(&investigation)?,
                        parse_obj(&implementation)?,
                        actor,
                        reason,
                    )
                    .map_err(err)?;
                print_learning_value(cli.json, &record, || print_implementation(&record));
            }
        },
        Commands::Learn { action } => match action {
            LearnCmd::Create {
                investigation,
                proposal,
                implementation,
                actor,
            } => {
                let outcome = caps
                    .create_measured_learning_outcome(
                        parse_inv(&investigation)?,
                        parse_obj(&proposal)?,
                        parse_obj(&implementation)?,
                        actor,
                    )
                    .map_err(err)?;
                print_learning_value(cli.json, &outcome, || print_measured_outcome(&outcome));
            }
            LearnCmd::EvidenceAdd {
                investigation,
                outcome,
                evidence,
                relation,
                expected_result,
                reason,
                actor,
            } => {
                let outcome = caps
                    .collect_outcome_evidence(
                        parse_inv(&investigation)?,
                        parse_obj(&outcome)?,
                        CollectOutcomeEvidenceRequest {
                            object_id: parse_obj(&evidence)?,
                            relation: relation.into(),
                            expected_result_id: expected_result
                                .as_deref()
                                .map(parse_obj)
                                .transpose()?,
                            reason,
                        },
                        actor,
                    )
                    .map_err(err)?;
                print_learning_value(cli.json, &outcome, || print_measured_outcome(&outcome));
            }
            LearnCmd::Evaluate {
                investigation,
                outcome,
                actor,
            } => {
                let outcome = caps
                    .evaluate_measured_learning_outcome(
                        parse_inv(&investigation)?,
                        parse_obj(&outcome)?,
                        actor,
                    )
                    .map_err(err)?;
                print_learning_value(cli.json, &outcome, || print_measured_outcome(&outcome));
            }
            LearnCmd::Verify {
                investigation,
                outcome,
                actor,
                reason,
                override_readiness,
                override_reason,
            } => {
                let outcome = caps
                    .verify_measured_learning_outcome(
                        parse_inv(&investigation)?,
                        parse_obj(&outcome)?,
                        actor,
                        reason,
                        override_readiness,
                        override_reason,
                    )
                    .map_err(err)?;
                print_learning_value(cli.json, &outcome, || print_measured_outcome(&outcome));
            }
            LearnCmd::List { investigation } => {
                let listing = caps
                    .list_measured_learning_outcomes(parse_inv(&investigation)?)
                    .map_err(err)?;
                print_learning_value(cli.json, &listing, || {
                    let mut output = if listing.outcomes.is_empty() {
                        "No Measured Learning Outcomes.".into()
                    } else {
                        listing
                            .outcomes
                            .iter()
                            .map(|outcome| {
                                format!(
                                    "{}  [{} / {}]  proposal {}  impl {}  (revision {})",
                                    outcome.id,
                                    outcome.status.as_str(),
                                    outcome.classification.as_str(),
                                    outcome.proposal_id,
                                    outcome.implementation_record_id,
                                    outcome.revision_number,
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    };
                    output.push('\n');
                    output.push_str(LEARNING_BOUNDARY);
                    output
                });
            }
            LearnCmd::Show {
                investigation,
                outcome,
            } => {
                let outcome = caps
                    .get_measured_learning_outcome(parse_inv(&investigation)?, parse_obj(&outcome)?)
                    .map_err(err)?;
                print_learning_value(cli.json, &outcome, || print_measured_outcome(&outcome));
            }
            LearnCmd::Trace {
                investigation,
                outcome,
            } => {
                let trace = caps
                    .trace_measured_learning_outcome(
                        parse_inv(&investigation)?,
                        parse_obj(&outcome)?,
                    )
                    .map_err(err)?;
                print_learning_value(cli.json, &trace, || {
                    format!(
                        "Proposal {} → Implementation {} → Measured Outcome {}\n  classification: {}\n  status: {}\n  {}\n{}",
                        trace.proposal_id,
                        trace.implementation_record_id,
                        trace.outcome_id,
                        trace.classification.as_str(),
                        trace.status.as_str(),
                        trace.explanation,
                        LEARNING_BOUNDARY,
                    )
                });
            }
            LearnCmd::History {
                investigation,
                outcome,
            } => {
                let current = caps
                    .get_measured_learning_outcome(parse_inv(&investigation)?, parse_obj(&outcome)?)
                    .map_err(err)?;
                let listing = caps
                    .list_measured_outcome_revisions(parse_inv(&investigation)?, current.lineage_id)
                    .map_err(err)?;
                print_learning_value(cli.json, &listing, || {
                    let mut output = listing
                        .outcomes
                        .iter()
                        .map(|item| {
                            format!(
                                "revision {}  {}  [{} / {}]",
                                item.revision_number,
                                item.id,
                                item.status.as_str(),
                                item.classification.as_str(),
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    output.push('\n');
                    output.push_str(LEARNING_BOUNDARY);
                    output
                });
            }
            LearnCmd::Withdraw {
                investigation,
                outcome,
                reason,
                actor,
            } => {
                let outcome = caps
                    .withdraw_measured_learning_outcome(
                        parse_inv(&investigation)?,
                        parse_obj(&outcome)?,
                        actor,
                        reason,
                    )
                    .map_err(err)?;
                print_learning_value(cli.json, &outcome, || print_measured_outcome(&outcome));
            }
            LearnCmd::Export {
                investigation,
                outcome,
                format,
            } => {
                let inv = parse_inv(&investigation)?;
                let outcome_id = parse_obj(&outcome)?;
                match format {
                    LearningExportFormatArg::Markdown => {
                        let markdown = caps
                            .export_measured_learning_outcome_markdown(inv, outcome_id)
                            .map_err(err)?;
                        println!("{markdown}");
                    }
                    LearningExportFormatArg::Json => {
                        let json = caps
                            .export_measured_learning_outcome_json(inv, outcome_id)
                            .map_err(err)?;
                        println!("{json}");
                    }
                }
            }
            LearnCmd::Patterns { derive, actor } => {
                if derive {
                    let derived = caps.derive_learning_patterns(actor).map_err(err)?;
                    print_learning_value(cli.json, &derived, || format_learning_patterns(&derived));
                } else {
                    let patterns = caps.list_learning_patterns().map_err(err)?;
                    print_learning_value(cli.json, &patterns, || {
                        format_learning_patterns(&patterns)
                    });
                }
            }
            LearnCmd::PatternShow { pattern } => {
                let pattern = caps
                    .get_learning_pattern(parse_obj(&pattern)?)
                    .map_err(err)?;
                print_learning_value(cli.json, &pattern, || {
                    format!(
                        "Pattern {} [{}]\n  {}\n  signature: {}\n  confidence: {:.0}%\n  supporting outcomes: {}\n  contradicting outcomes: {}\n{}",
                        pattern.id,
                        pattern.status.as_str(),
                        pattern.title,
                        pattern.signature,
                        pattern.confidence.value() * 100.0,
                        pattern.supporting_outcome_ids.len(),
                        pattern.contradicting_outcome_ids.len(),
                        LEARNING_BOUNDARY,
                    )
                });
            }
            LearnCmd::PatternExport { pattern, format } => {
                let pattern_id = parse_obj(&pattern)?;
                match format {
                    LearningExportFormatArg::Markdown => {
                        let markdown = caps
                            .export_learning_pattern_markdown(pattern_id)
                            .map_err(err)?;
                        println!("{markdown}");
                    }
                    LearningExportFormatArg::Json => {
                        let json = caps.export_learning_pattern_json(pattern_id).map_err(err)?;
                        println!("{json}");
                    }
                }
            }
            LearnCmd::Influence {
                investigation,
                proposal,
            } => {
                let influence = caps
                    .explain_historical_influence(parse_inv(&investigation)?, parse_obj(&proposal)?)
                    .map_err(err)?;
                print_learning_value(cli.json, &influence, || {
                    let mut output = influence.explanation.clone();
                    for item in &influence.patterns_considered {
                        output.push_str(&format!(
                            "\n  • pattern {}  {}  magnitude={:.3} — {}",
                            item.pattern_id, item.direction, item.magnitude, item.relevance
                        ));
                    }
                    output.push('\n');
                    output.push_str(LEARNING_BOUNDARY);
                    output
                });
            }
            LearnCmd::DerivePatterns { actor } => {
                let derived = caps.derive_learning_patterns(actor).map_err(err)?;
                print_learning_value(cli.json, &derived, || format_learning_patterns(&derived));
            }
        },
    }

    Ok(exit)
}

fn transition_proposal(
    caps: &CapabilityService,
    investigation: &str,
    proposal: &str,
    status: ProposalStatus,
    reason: String,
) -> Result<ImprovementProposal, String> {
    caps.update_improvement_proposal_status(
        parse_inv(investigation)?,
        parse_obj(proposal)?,
        status,
        "cli",
        reason,
        ProposalTransitionAuthority::ExternalCaller,
    )
    .map_err(err)
}

fn print_generated_proposals(json: bool, proposals: &[ImprovementProposal]) {
    if json {
        print_value(
            true,
            &serde_json::json!({
                "proposals": proposals,
                "boundary": PROPOSAL_BOUNDARY,
            }),
            String::new,
        );
        return;
    }
    println!("Generated {} Proposal alternative(s):", proposals.len());
    for proposal in proposals {
        println!(
            "  {}  [{} / {}]  {}\n    {}",
            proposal.id,
            proposal.status.as_str(),
            proposal.priority.as_str(),
            proposal.title,
            proposal.summary,
        );
    }
    println!("Alternatives are uncertain candidates and are not guaranteed correct.");
    println!("{PROPOSAL_BOUNDARY}");
}

fn print_proposal_comparison(json: bool, comparison: &rivora::domain::ProposalComparison) {
    if json {
        print_proposal_value(true, comparison, String::new);
        return;
    }
    for ranked in &comparison.ranked {
        println!(
            "{}. {} score={:.3}",
            ranked.rank, ranked.proposal_id, ranked.score
        );
        for factor in &ranked.factors {
            println!(
                "    {} weight={:.2} contribution={:.3} — {}",
                factor.name, factor.weight, factor.contribution, factor.explanation
            );
        }
        println!("    {}", ranked.explanation);
    }
    println!("{}", comparison.explanation);
    println!("Ranking is guidance, not a guaranteed correct implementation.");
    println!("{PROPOSAL_BOUNDARY}");
}

fn print_lines(lines: &[String]) -> String {
    if lines.is_empty() {
        "  none specified".into()
    } else {
        lines
            .iter()
            .map(|line| format!("  • {line}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn print_proposal_value<T: serde::Serialize>(
    json: bool,
    value: &T,
    human: impl FnOnce() -> String,
) {
    if !json {
        println!("{}", human());
        return;
    }
    match serde_json::to_value(value) {
        Ok(mut structured) => {
            if let Some(object) = structured.as_object_mut() {
                object.insert(
                    "boundary".into(),
                    serde_json::Value::String(PROPOSAL_BOUNDARY.into()),
                );
            }
            print_value(true, &structured, String::new);
        }
        Err(error) => eprintln!("error encoding json: {error}"),
    }
}

fn print_proposal(proposal: &ImprovementProposal) -> String {
    let implementation = proposal
        .external_implementation_reference
        .as_deref()
        .map(|reference| format!("manually referenced as {reference}; not verified"))
        .unwrap_or_else(|| "not recorded".into());
    format!(
        "Proposal {} revision {} [{} / {}]\n  {}\n  summary: {}\n  rationale: {}\n  supporting evidence: {}\n  contradicting evidence: {}\n  implemented externally: {}\n  verified outcome: not established by Proposal state\n{}",
        proposal.id,
        proposal.revision_number,
        proposal.status.as_str(),
        proposal.priority.as_str(),
        proposal.title,
        proposal.summary,
        proposal.rationale,
        proposal.supporting_evidence.len(),
        proposal.contradicting_evidence.len(),
        implementation,
        PROPOSAL_BOUNDARY,
    )
}

fn connector_status(name: &str) -> Result<ConnectorStatusReport, String> {
    match name {
        "github_actions" | "actions" | "ci" => {
            Ok(GitHubActionsConnector::new("owner/repo").status())
        }
        "kubernetes" | "k8s" => Ok(KubernetesConnector::new("default").status()),
        "sentry" => Ok(SentryConnector::new("org", "project").status()),
        "github" => Ok(ConnectorStatusReport {
            id: "github".into(),
            category: "code".into(),
            configured: std::env::var("GITHUB_TOKEN")
                .ok()
                .filter(|s| !s.is_empty())
                .is_some(),
            read_only: true,
            details: "GitHub connector (repository/PR)".into(),
        }),
        "local" => Ok(ConnectorStatusReport {
            id: "local".into(),
            category: "local".into(),
            configured: true,
            read_only: true,
            details: "Local project connector".into(),
        }),
        other => Err(format!("unknown connector: {other}")),
    }
}

fn connector_test(
    name: &str,
    repository: Option<&str>,
    namespace: Option<&str>,
    organization: Option<&str>,
    project: Option<&str>,
) -> Result<String, String> {
    match name {
        "github_actions" | "actions" | "ci" => {
            GitHubActionsConnector::new(repository.unwrap_or("owner/repo"))
                .test_configuration()
                .map_err(err)
        }
        "kubernetes" | "k8s" => KubernetesConnector::new(namespace.unwrap_or("default"))
            .test_configuration()
            .map_err(err),
        "sentry" => {
            SentryConnector::new(organization.unwrap_or("org"), project.unwrap_or("project"))
                .test_configuration()
                .map_err(err)
        }
        "github" | "local" => Ok(format!("{name}: available (read-only)")),
        other => Err(format!("unknown connector: {other}")),
    }
}

fn connector_collect(
    name: &str,
    fixture: Option<&PathBuf>,
    repository: Option<&str>,
    namespace: Option<&str>,
    organization: Option<&str>,
    project: Option<&str>,
    path: Option<&PathBuf>,
) -> Result<Vec<NormalizedObservation>, String> {
    match name {
        "github_actions" | "actions" | "ci" => {
            if let Some(path) = fixture {
                let raw = std::fs::read_to_string(path).map_err(err)?;
                let value: serde_json::Value = serde_json::from_str(&raw).map_err(err)?;
                GitHubActionsConnector::observe_from_fixture(&value).map_err(err)
            } else {
                GitHubActionsConnector::new(repository.unwrap_or("owner/repo"))
                    .observe()
                    .map_err(err)
            }
        }
        "kubernetes" | "k8s" => {
            if let Some(path) = fixture {
                let raw = std::fs::read_to_string(path).map_err(err)?;
                let value: serde_json::Value = serde_json::from_str(&raw).map_err(err)?;
                KubernetesConnector::observe_from_fixture(&value).map_err(err)
            } else {
                KubernetesConnector::new(namespace.unwrap_or("default"))
                    .observe()
                    .map_err(err)
            }
        }
        "sentry" => {
            if let Some(path) = fixture {
                let raw = std::fs::read_to_string(path).map_err(err)?;
                let value: serde_json::Value = serde_json::from_str(&raw).map_err(err)?;
                SentryConnector::observe_from_fixture(&value).map_err(err)
            } else {
                SentryConnector::new(organization.unwrap_or("org"), project.unwrap_or("project"))
                    .observe()
                    .map_err(err)
            }
        }
        "local" => {
            let root = path.cloned().unwrap_or_else(|| PathBuf::from("."));
            LocalConnector::new(root).observe().map_err(err)
        }
        "github" => {
            if let Some(path) = fixture {
                let raw = std::fs::read_to_string(path).map_err(err)?;
                let value: serde_json::Value = serde_json::from_str(&raw).map_err(err)?;
                GitHubConnector::observe_from_fixture(&value).map_err(err)
            } else {
                GitHubConnector::new(repository.ok_or("repository required for github")?)
                    .observe()
                    .map_err(err)
            }
        }
        other => Err(format!("unknown connector: {other}")),
    }
}

fn parse_inv(s: &str) -> Result<InvestigationId, String> {
    s.parse().map_err(err)
}

fn parse_obj(s: &str) -> Result<ObjectId, String> {
    s.parse().map_err(err)
}

fn parse_kind(s: &str) -> ObservationKind {
    match s.to_ascii_lowercase().as_str() {
        "event" => ObservationKind::Event,
        "repository" => ObservationKind::Repository,
        "commit" => ObservationKind::Commit,
        "git_status" | "git-status" => ObservationKind::GitStatus,
        "changed_files" | "changed-files" => ObservationKind::ChangedFiles,
        "pull_request" | "pr" => ObservationKind::PullRequest,
        "check" | "check_result" => ObservationKind::CheckResult,
        "test" | "test_output" => ObservationKind::TestOutput,
        "issue" => ObservationKind::Issue,
        "user" | "user_input" => ObservationKind::UserInput,
        "local_event" => ObservationKind::LocalEvent,
        "workflow_run" | "workflow-run" | "ci" => ObservationKind::WorkflowRun,
        "infrastructure" | "infra" | "k8s" => ObservationKind::Infrastructure,
        "observability" | "alert" | "sentry" => ObservationKind::Observability,
        other => ObservationKind::Other(other.into()),
    }
}

fn parse_status(s: &str) -> Result<InvestigationStatus, String> {
    match s.to_ascii_lowercase().as_str() {
        "created" => Ok(InvestigationStatus::Created),
        "collecting" => Ok(InvestigationStatus::Collecting),
        "understanding" => Ok(InvestigationStatus::Understanding),
        "evaluating" => Ok(InvestigationStatus::Evaluating),
        "verifying" => Ok(InvestigationStatus::Verifying),
        "recommending" => Ok(InvestigationStatus::Recommending),
        "learning" => Ok(InvestigationStatus::Learning),
        "completed" => Ok(InvestigationStatus::Completed),
        other => Err(format!("unknown status: {other}")),
    }
}

fn parse_verification(s: &str) -> Result<VerificationResult, String> {
    match s.to_ascii_lowercase().as_str() {
        "pass" => Ok(VerificationResult::Pass),
        "fail" => Ok(VerificationResult::Fail),
        "inconclusive" => Ok(VerificationResult::Inconclusive),
        other => Err(format!("unknown verification result: {other}")),
    }
}

fn parse_relationship_kind(s: &str) -> Result<RelationshipKind, String> {
    match s.to_ascii_lowercase().as_str() {
        "shared_repository" => Ok(RelationshipKind::SharedRepository),
        "shared_commit" => Ok(RelationshipKind::SharedCommit),
        "shared_pull_request" => Ok(RelationshipKind::SharedPullRequest),
        "shared_file_path" => Ok(RelationshipKind::SharedFilePath),
        "shared_connector_source" => Ok(RelationshipKind::SharedConnectorSource),
        "similar_observations" => Ok(RelationshipKind::SimilarObservations),
        "shared_evaluation_category" => Ok(RelationshipKind::SharedEvaluationCategory),
        "related_verification_outcome" => Ok(RelationshipKind::RelatedVerificationOutcome),
        "repeated_failure_signature" => Ok(RelationshipKind::RepeatedFailureSignature),
        "related_recommendation" => Ok(RelationshipKind::RelatedRecommendation),
        "related_learning_outcome" => Ok(RelationshipKind::RelatedLearningOutcome),
        "explicit_link" => Ok(RelationshipKind::ExplicitLink),
        other => Err(format!("unknown relationship kind: {other}")),
    }
}

fn parse_datetime(s: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| format!("invalid RFC3339 timestamp `{s}`: {e}"))
}

fn print_recalled_contexts(contexts: &[rivora::domain::RecalledContext]) -> String {
    if contexts.is_empty() {
        return "No recalled context.".into();
    }
    contexts
        .iter()
        .map(|c| {
            format!(
                "{}  [{}]  from {}  ({})\n  reason: {}\n  {}\n  {}",
                c.id,
                c.state.as_str(),
                c.source_investigation_id,
                c.origin.as_str(),
                c.reason,
                c.evidence_summary,
                c.explanation
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn print_search_results(results: &[SearchResult]) -> String {
    if results.is_empty() {
        return "No matching Investigations.".into();
    }
    results
        .iter()
        .map(|r| {
            format!(
                "{}  [{}]  {}  (score {:.2})\n    {}",
                r.investigation_id, r.status, r.title, r.score, r.explanation
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_implementation_reference(
    kind: ImplementationReferenceKindArg,
    value: String,
) -> Result<ImplementationReference, String> {
    Ok(match kind {
        ImplementationReferenceKindArg::CommitSha => {
            ImplementationReference::CommitSha { sha: value }
        }
        ImplementationReferenceKindArg::PullRequest => {
            ImplementationReference::PullRequest { reference: value }
        }
        ImplementationReferenceKindArg::Branch => ImplementationReference::Branch { name: value },
        ImplementationReferenceKindArg::DeploymentId => {
            ImplementationReference::DeploymentId { id: value }
        }
        ImplementationReferenceKindArg::BuildId => ImplementationReference::BuildId { id: value },
        ImplementationReferenceKindArg::IncidentId => {
            ImplementationReference::IncidentId { id: value }
        }
        ImplementationReferenceKindArg::WorkflowRun => {
            ImplementationReference::WorkflowRun { id: value }
        }
        ImplementationReferenceKindArg::ArtifactPath => {
            ImplementationReference::ArtifactPath { path: value }
        }
        ImplementationReferenceKindArg::ExternalUri => {
            ImplementationReference::ExternalUri { uri: value }
        }
        ImplementationReferenceKindArg::HumanNote => {
            ImplementationReference::HumanNote { note: value }
        }
    })
}

fn print_learning_value<T: serde::Serialize>(
    json: bool,
    value: &T,
    human: impl FnOnce() -> String,
) {
    if !json {
        println!("{}", human());
        return;
    }
    match serde_json::to_value(value) {
        Ok(mut structured) => {
            if let Some(object) = structured.as_object_mut() {
                object.insert(
                    "boundary".into(),
                    serde_json::Value::String(LEARNING_BOUNDARY.into()),
                );
            }
            print_value(true, &structured, String::new);
        }
        Err(error) => eprintln!("error encoding json: {error}"),
    }
}

fn print_implementation(record: &ImplementationRecord) -> String {
    format!(
        "Implementation {} revision {} [{} / {}]\n  proposal: {}\n  summary: {}\n  references: {}\n  evidence: {}\n  scope: {}\n{}",
        record.id,
        record.revision_number,
        record.status.as_str(),
        record.source.as_str(),
        record.proposal_id,
        record.summary,
        record.references.len(),
        record.evidence_ids.len(),
        if record.declared_scope.is_empty() {
            "none declared"
        } else {
            record.declared_scope.as_str()
        },
        LEARNING_BOUNDARY,
    )
}

fn print_measured_outcome(outcome: &MeasuredLearningOutcome) -> String {
    let report = outcome
        .evaluation_report
        .as_ref()
        .map(|report| {
            format!(
                "evaluation: verification_ready={} method={}",
                report.verification_ready, report.method
            )
        })
        .unwrap_or_else(|| "evaluation: not yet run".into());
    format!(
        "Measured Outcome {} revision {} [{} / {}]\n  proposal: {}\n  implementation: {}\n  confidence: {:.0}%\n  expected results: {}\n  assessments: {}\n  evidence links: {}\n  regressions: {}\n  {}\n  historical learning eligible: {}\n{}",
        outcome.id,
        outcome.revision_number,
        outcome.status.as_str(),
        outcome.classification.as_str(),
        outcome.proposal_id,
        outcome.implementation_record_id,
        outcome.confidence.value() * 100.0,
        outcome.expected_results.len(),
        outcome.assessments.len(),
        outcome.evidence_links.len(),
        outcome.regressions.len(),
        report,
        outcome.historical_learning_eligible,
        LEARNING_BOUNDARY,
    )
}

fn format_learning_patterns(patterns: &[rivora::domain::LearningPattern]) -> String {
    let mut output = if patterns.is_empty() {
        "No Learning Patterns.".into()
    } else {
        patterns
            .iter()
            .map(|pattern| {
                format!(
                    "{}  [{}]  {}  (confidence {:.0}%)",
                    pattern.id,
                    pattern.status.as_str(),
                    pattern.signature,
                    pattern.confidence.value() * 100.0,
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    output.push('\n');
    output.push_str(LEARNING_BOUNDARY);
    output
}

fn format_capability_descriptor(desc: &rivora::ExecutionCapabilityDescriptor) -> String {
    let loop_lines = format!(
        "  Engineering Loop:\n    Memory        {}\n    Evaluation    {}\n    Verification  {}\n    Improvement   {}\n    Learning      {}",
        desc.engineering_loop.memory.as_str(),
        desc.engineering_loop.evaluation.as_str(),
        desc.engineering_loop.verification.as_str(),
        desc.engineering_loop.improvement.as_str(),
        desc.engineering_loop.learning.as_str(),
    );
    let limitations = if desc.limitations.is_empty() {
        "  limitations: (none)".to_string()
    } else {
        format!(
            "  limitations:\n{}",
            desc.limitations
                .iter()
                .map(|l| format!("    - {l}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    format!(
        "{}\n  name: {}\n  provider: {}\n  operation: {}\n  risk: {}\n  mutating: {}\n  version: {}\n  dry_run: {}\n  actions: {}\n  permissions: [{}]\n  accepted_input_types: [{}]\n  output_types: [{}]\n  provider_independent: {}\n  complete: {}\n  {}\n{}\n{}\n{}",
        desc.capability_id,
        desc.display_name(),
        desc.provider,
        desc.operation,
        desc.risk_level.as_str(),
        desc.mutating,
        desc.version,
        desc.supports_dry_run,
        desc.supported_actions.join(", "),
        desc.permissions.join(", "),
        desc.accepted_input_types.join(", "),
        desc.output_types.join(", "),
        desc.provider_independent,
        desc.is_complete(),
        desc.description,
        loop_lines,
        limitations,
        EXECUTION_BOUNDARY
    )
}

fn format_capability_coverage(report: &rivora::CapabilityCoverageReport) -> String {
    let mut out = format!("{}\n", report.summary);
    out.push_str(&format!(
        "  first_party: {}/{} registered\n  descriptors_complete: {}\n  lifecycle_declared: {}\n",
        report.first_party_registered,
        report.first_party_expected,
        report.all_descriptors_complete,
        report.all_lifecycle_declared
    ));
    out.push_str("Capabilities:\n");
    for c in &report.capabilities {
        out.push_str(&format!(
            "  {}  v{}  {}/{}  complete={}  loop=[M:{} E:{} V:{} I:{} L:{}]  types=[{}]\n",
            c.capability_id,
            c.version,
            c.provider,
            c.operation,
            c.descriptor_complete,
            c.memory,
            c.evaluation,
            c.verification,
            c.improvement,
            c.learning,
            c.accepted_input_types.join(", ")
        ));
    }
    out.push_str("Connectors:\n");
    for conn in &report.connectors {
        out.push_str(&format!(
            "  {}  provider={}  read_only={}  kinds=[{}]  fixture={}\n",
            conn.connector_id,
            conn.provider,
            conn.read_only,
            conn.emitted_kinds.join(", "),
            conn.fixture_support
        ));
    }
    if !report.gaps.is_empty() {
        out.push_str("Gaps:\n");
        for g in &report.gaps {
            out.push_str(&format!("  - {g}\n"));
        }
    }
    out.push_str(EXECUTION_BOUNDARY);
    out
}

fn format_lifecycle_run(run: &rivora::CapabilityLifecycleRun) -> String {
    let mut out = format!(
        "Capability Engineering Loop\n  run: {}\n  lineage: {} rev {}\n  status: {}\n  capability: {}\n  invocation: {}\n  plan: {}\n  attempt: {}\n  stages:\n",
        run.id,
        run.lineage_id,
        run.revision_number,
        run.status.as_str(),
        run.capability_id,
        run.invocation_id,
        run.plan_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "-".into()),
        run.attempt_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "-".into()),
    );
    for stage in &run.stages {
        out.push_str(&format!(
            "    {:<12}  status={:<14}  participation={:<14}",
            stage.stage.as_str(),
            stage.status.as_str(),
            stage.participation.as_str(),
        ));
        if let Some(detail) = &stage.detail {
            out.push_str(&format!("  {detail}"));
        }
        if let Some(error) = &stage.error {
            out.push_str(&format!("  error={error}"));
        }
        if !stage.artifact_ids.is_empty() {
            out.push_str(&format!(
                "  artifacts=[{}]",
                stage
                    .artifact_ids
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        out.push('\n');
    }
    out.push_str(&format!("  {}\n", run.explanation));
    out.push_str(
        "Connectors provide normalized facts. Capabilities contribute typed context. Runtime owns reasoning.\n",
    );
    out
}

fn format_lifecycle_trace(trace: &rivora::CapabilityLifecycleTrace) -> String {
    let mut out = format!(
        "Lifecycle trace\n  capability: {}\n  invocation: {}\n  status: {}\n  plan: {}\n  attempt: {}\n  run: {}\n  stages:\n",
        trace.capability_id,
        trace.invocation_id,
        trace
            .status
            .map(|s| s.as_str().to_string())
            .unwrap_or_else(|| "none".into()),
        trace
            .plan_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "-".into()),
        trace
            .attempt_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "-".into()),
        trace
            .run_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "-".into()),
    );
    for stage in &trace.stages {
        out.push_str(&format!(
            "    {}  {}\n",
            stage.stage.as_str(),
            stage.status.as_str()
        ));
    }
    out.push_str(&format!("  {}\n", trace.explanation));
    out
}

fn print_value<T: serde::Serialize>(json: bool, value: &T, human: impl FnOnce() -> String) {
    if json {
        match serde_json::to_string_pretty(value) {
            Ok(s) => println!("{s}"),
            Err(e) => eprintln!("error encoding json: {e}"),
        }
    } else {
        println!("{}", human());
    }
}
