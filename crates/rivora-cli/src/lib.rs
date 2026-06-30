//! Local CLI memory interface for Open Rivora.
//!
//! The CLI is intentionally local-first. It reads and writes only `.rivora/`
//! JSON files in the selected working directory and delegates memory behavior
//! to `rivora-adaptive`.

mod demo_fixtures;
pub mod slack_adapter;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub use slack_adapter::{
    normalize_app_mention_text, redact_slack_token_like_values, run_slack_command, run_slack_dev,
    slack_output_contains_infrastructure_action, SlackAppMentionEvent, SlackCommand,
    SlackDevOptions, SlackDoctorOptions, SlackPostMessageRequest, SlackSocketOptions,
    SlackTokenConfig,
};

use rivora_adaptive::{AdaptiveMemoryEngine, MemoryCandidateRequest, RecallQuery, RecallResult};
use rivora_connectors::{
    EvidenceIngestResult, EvidenceItem, EvidenceKind, GitHubAuthConfig, GitHubConnector,
    GitHubIngestRequest, GitHubIngestResult, GitHubRepositoryRef, HttpGitHubClient,
    HttpVercelClient, LocalGitConnector, VercelAuthConfig, VercelConnector, VercelIngestRequest,
    VercelIngestResult, VercelProjectRef,
};
use rivora_errors::{Result, RivoraError};
use rivora_memory::{
    FeedbackKind, FeedbackSource, FeedbackTargetType, HumanFeedback, MemoryKind, MemoryRecord,
    MemoryScope, MemoryStatus,
};
use rivora_receipts::Receipt;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

const STORE_DIR: &str = ".rivora";
const MEMORIES_FILE: &str = "memories.json";
const FEEDBACK_FILE: &str = "feedback.json";
const RECEIPTS_FILE: &str = "receipts.json";
const EVIDENCE_FILE: &str = "evidence.json";
const CLI_SOURCE: &str = "rivora-cli";
const CLI_VERSION: &str = "0.1.0";
const DEFAULT_TIMESTAMP: &str = "2026-06-28T00:00:00Z";
static DEMO_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalMemoryStore {
    pub root: PathBuf,
    #[serde(default)]
    store_dir_override: Option<PathBuf>,
}

impl LocalMemoryStore {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            store_dir_override: None,
        }
    }

    #[must_use]
    pub fn with_store_dir(root: impl Into<PathBuf>, store_dir: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let store_dir = store_dir.into();
        let store_dir = if store_dir.is_absolute() {
            store_dir
        } else {
            root.join(store_dir)
        };
        Self {
            root,
            store_dir_override: Some(store_dir),
        }
    }

    #[must_use]
    pub fn store_dir(&self) -> PathBuf {
        self.store_dir_override
            .clone()
            .unwrap_or_else(|| self.root.join(STORE_DIR))
    }

    #[must_use]
    pub fn memories_path(&self) -> PathBuf {
        self.store_dir().join(MEMORIES_FILE)
    }

    #[must_use]
    pub fn feedback_path(&self) -> PathBuf {
        self.store_dir().join(FEEDBACK_FILE)
    }

    #[must_use]
    pub fn receipts_path(&self) -> PathBuf {
        self.store_dir().join(RECEIPTS_FILE)
    }

    #[must_use]
    pub fn evidence_path(&self) -> PathBuf {
        self.store_dir().join(EVIDENCE_FILE)
    }

    pub fn init(&self) -> Result<StoreSnapshot> {
        fs::create_dir_all(self.store_dir())?;
        init_array_file(&self.memories_path())?;
        init_array_file(&self.feedback_path())?;
        init_array_file(&self.receipts_path())?;
        init_array_file(&self.evidence_path())?;
        self.load()
    }

    pub fn load(&self) -> Result<StoreSnapshot> {
        Ok(StoreSnapshot {
            memories: read_array(&self.memories_path())?,
            feedback: read_array(&self.feedback_path())?,
            receipts: read_array(&self.receipts_path())?,
            evidence: read_array_or_empty(&self.evidence_path())?,
        })
    }

    pub fn save_memories(&self, memories: &[MemoryRecord]) -> Result<()> {
        self.ensure_initialized()?;
        write_array(&self.memories_path(), memories)
    }

    pub fn append_feedback(&self, feedback: HumanFeedback) -> Result<()> {
        self.ensure_initialized()?;
        let mut entries: Vec<HumanFeedback> = read_array(&self.feedback_path())?;
        entries.push(feedback);
        write_array(&self.feedback_path(), &entries)
    }

    pub fn append_receipts(&self, receipts: impl IntoIterator<Item = Receipt>) -> Result<()> {
        self.ensure_initialized()?;
        let mut entries: Vec<Receipt> = read_array(&self.receipts_path())?;
        entries.extend(receipts);
        write_array(&self.receipts_path(), &entries)
    }

    pub fn append_evidence(
        &self,
        evidence: impl IntoIterator<Item = EvidenceItem>,
    ) -> Result<usize> {
        self.ensure_initialized()?;
        let mut entries: Vec<EvidenceItem> = read_array_or_empty(&self.evidence_path())?;
        let mut added = 0;
        for item in evidence {
            if entries.iter().any(|existing| existing.id == item.id) {
                continue;
            }
            entries.push(item);
            added += 1;
        }
        entries.sort_by(|a, b| a.id.cmp(&b.id));
        write_array(&self.evidence_path(), &entries)?;
        Ok(added)
    }

    fn ensure_initialized(&self) -> Result<()> {
        fs::create_dir_all(self.store_dir())?;
        init_array_file(&self.memories_path())?;
        init_array_file(&self.feedback_path())?;
        init_array_file(&self.receipts_path())?;
        init_array_file(&self.evidence_path())
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct StoreSnapshot {
    pub memories: Vec<MemoryRecord>,
    pub feedback: Vec<HumanFeedback>,
    pub receipts: Vec<Receipt>,
    pub evidence: Vec<EvidenceItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Init,
    Demo(DemoOptions),
    Remember(RememberOptions),
    Recall(RecallOptions),
    Feedback(FeedbackOptions),
    Ingest(IngestOptions),
    Evidence(EvidenceCommand),
    Slack(SlackCommand),
    Ask(String),
    Status,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RememberOptions {
    pub service: Option<String>,
    pub summary: Option<String>,
    pub from_evidence: Option<String>,
    pub symptoms: Vec<String>,
    pub tags: Vec<String>,
    pub evidence: Vec<String>,
    pub source: Option<String>,
    pub confidence: Option<String>,
    pub approve: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DemoOptions {
    pub scenario: DemoScenario,
    pub keep: bool,
    pub json: bool,
    pub store: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DemoScenario {
    #[default]
    Basic,
    CheckoutIncident,
    ReleaseRegression,
    WorkflowFailure,
}

impl DemoScenario {
    pub const ALL: [Self; 4] = [
        Self::Basic,
        Self::CheckoutIncident,
        Self::ReleaseRegression,
        Self::WorkflowFailure,
    ];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Basic => "basic",
            Self::CheckoutIncident => "checkout-incident",
            Self::ReleaseRegression => "release-regression",
            Self::WorkflowFailure => "workflow-failure",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "basic" => Ok(Self::Basic),
            "checkout-incident" => Ok(Self::CheckoutIncident),
            "release-regression" => Ok(Self::ReleaseRegression),
            "workflow-failure" => Ok(Self::WorkflowFailure),
            other => Err(RivoraError::invalid_value(
                "demo_scenario",
                format!(
                    "unknown demo scenario '{other}'; supported values: {}",
                    Self::ALL
                        .iter()
                        .map(|scenario| scenario.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )),
        }
    }

    fn config(self) -> DemoScenarioConfig {
        match self {
            Self::Basic => DemoScenarioConfig {
                selected_evidence_id: "github:pr:demo/checkout:128",
                recall_service: "checkout-api",
                recall_symptoms: &["latency"],
                recall_tags: &["inventory"],
                ask_prompt: "what changed in checkout?",
                slack_prompt: "have we seen checkout latency before?",
            },
            Self::CheckoutIncident => DemoScenarioConfig {
                selected_evidence_id: "github:pr:demo/checkout-incident:128",
                recall_service: "checkout-api",
                recall_symptoms: &["latency"],
                recall_tags: &["inventory"],
                ask_prompt: "have we seen checkout latency before?",
                slack_prompt: "have we seen checkout latency before?",
            },
            Self::ReleaseRegression => DemoScenarioConfig {
                selected_evidence_id: "github:pr:demo/release-regression:141",
                recall_service: "checkout-api",
                recall_symptoms: &["release", "regression"],
                recall_tags: &["retry-policy"],
                ask_prompt: "have we seen checkout release regressions before?",
                slack_prompt: "have we seen checkout release regressions before?",
            },
            Self::WorkflowFailure => DemoScenarioConfig {
                selected_evidence_id: "github:workflow:demo/workflow-failure:1152",
                recall_service: "billing-api",
                recall_symptoms: &["migration", "validation"],
                recall_tags: &["workflow"],
                ask_prompt: "what failed recently?",
                slack_prompt: "have we seen billing migration failures before?",
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct DemoScenarioConfig {
    selected_evidence_id: &'static str,
    recall_service: &'static str,
    recall_symptoms: &'static [&'static str],
    recall_tags: &'static [&'static str],
    ask_prompt: &'static str,
    slack_prompt: &'static str,
}

#[derive(Debug, Serialize)]
struct DemoSummary<'a> {
    demo: &'static str,
    scenario: &'static str,
    evidence_count: usize,
    selected_evidence_id: &'a str,
    memory_id: &'a str,
    final_memory_status: String,
    recall_match_count: usize,
    slack_dev_rendered: bool,
    message: &'static str,
    human_control_summary: &'static str,
    safety_summary: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RecallOptions {
    pub service: Option<String>,
    pub symptoms: Vec<String>,
    pub tags: Vec<String>,
    pub include_candidates: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackOptions {
    pub memory_id: String,
    pub kind: FeedbackCommandKind,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestOptions {
    Git(GitIngestOptions),
    GitHub(GitHubIngestOptions),
    Vercel(VercelIngestOptions),
    Fixture(FixtureIngestOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitIngestOptions {
    pub repo: PathBuf,
    pub since: Option<String>,
    pub limit: usize,
}

impl Default for GitIngestOptions {
    fn default() -> Self {
        Self {
            repo: PathBuf::from("."),
            since: None,
            limit: 20,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubIngestOptions {
    pub repo: String,
    pub limit: usize,
    pub since: Option<String>,
    pub pull_requests: bool,
    pub issues: bool,
    pub workflow_runs: bool,
    pub releases: bool,
    pub deployments: bool,
}

impl Default for GitHubIngestOptions {
    fn default() -> Self {
        Self {
            repo: String::new(),
            limit: 20,
            since: None,
            pull_requests: false,
            issues: false,
            workflow_runs: false,
            releases: false,
            deployments: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VercelIngestOptions {
    pub project: String,
    pub team: Option<String>,
    pub limit: usize,
    pub since: Option<String>,
}

impl Default for VercelIngestOptions {
    fn default() -> Self {
        Self {
            project: String::new(),
            team: None,
            limit: 20,
            since: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureIngestOptions {
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceCommand {
    List,
    Show(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedbackCommandKind {
    Approve,
    Reject,
    Correct,
    Useful,
    NotUseful,
    NeedsMoreEvidence,
}

impl FeedbackCommandKind {
    #[must_use]
    pub fn as_feedback_kind(self) -> FeedbackKind {
        match self {
            Self::Approve => FeedbackKind::Approved,
            Self::Reject => FeedbackKind::Rejected,
            Self::Correct => FeedbackKind::Corrected,
            Self::Useful => FeedbackKind::Useful,
            Self::NotUseful => FeedbackKind::NotUseful,
            Self::NeedsMoreEvidence => FeedbackKind::NeedsMoreEvidence,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::Reject => "reject",
            Self::Correct => "correct",
            Self::Useful => "useful",
            Self::NotUseful => "not-useful",
            Self::NeedsMoreEvidence => "needs-more-evidence",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AskIntent {
    Recall,
    RememberPrompt,
    WhatChanged,
    WhatMergedRecently,
    WhatFailedRecently,
    WhatChangedInGithub,
    WhatDeployedRecently,
    WhatFailedInVercel,
    WhatChangedInVercel,
    Help,
}

pub fn run<I, S>(args: I, cwd: &Path) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    let command = parse_command(&args)?;
    run_command(command, &LocalMemoryStore::new(cwd))
}

pub fn run_command(command: Command, store: &LocalMemoryStore) -> Result<String> {
    match command {
        Command::Init => init(store),
        Command::Demo(options) => demo(store, options),
        Command::Remember(options) => remember(store, options),
        Command::Recall(options) => recall(store, options),
        Command::Feedback(options) => feedback(store, options),
        Command::Ingest(options) => ingest(store, options),
        Command::Evidence(command) => evidence_command(store, command),
        Command::Slack(command) => run_slack_command(store, command),
        Command::Ask(prompt) => ask(store, &prompt),
        Command::Status => status(store),
        Command::Help => Ok(help_text()),
    }
}

pub fn parse_command(args: &[String]) -> Result<Command> {
    match args.first().map(String::as_str) {
        None | Some("help") | Some("--help") | Some("-h") => Ok(Command::Help),
        Some("init") => Ok(Command::Init),
        Some("demo") => Ok(Command::Demo(parse_demo_options(&args[1..])?)),
        Some("remember") => Ok(Command::Remember(parse_remember_options(&args[1..])?)),
        Some("recall") => Ok(Command::Recall(parse_recall_options(&args[1..])?)),
        Some("feedback") => Ok(Command::Feedback(parse_feedback_options(&args[1..])?)),
        Some("ingest") => Ok(Command::Ingest(parse_ingest_options(&args[1..])?)),
        Some("evidence") => Ok(Command::Evidence(parse_evidence_command(&args[1..])?)),
        Some("slack") => Ok(Command::Slack(parse_slack_command(&args[1..])?)),
        Some("ask") => {
            let prompt = args[1..].join(" ");
            if prompt.trim().is_empty() {
                Ok(Command::Help)
            } else {
                Ok(Command::Ask(prompt))
            }
        }
        Some("status") => Ok(Command::Status),
        Some(other) => Err(RivoraError::invalid_value(
            "command",
            format!("unsupported command '{other}'"),
        )),
    }
}

pub fn parse_ask_intent(prompt: &str) -> AskIntent {
    let normalized = normalize(prompt);
    if normalized.contains("have we seen") || normalized.starts_with("recall ") {
        AskIntent::Recall
    } else if normalized.contains("what should we remember") {
        AskIntent::RememberPrompt
    } else if normalized.contains("what merged recently") || normalized.contains("what merged") {
        AskIntent::WhatMergedRecently
    } else if normalized.contains("what failed recently") || normalized.contains("what failed") {
        AskIntent::WhatFailedRecently
    } else if normalized.contains("what changed in github") {
        AskIntent::WhatChangedInGithub
    } else if normalized.contains("what deployed recently") || normalized.contains("what deployed")
    {
        AskIntent::WhatDeployedRecently
    } else if normalized.contains("what failed in vercel")
        || normalized.contains("vercel") && normalized.contains("failed")
    {
        AskIntent::WhatFailedInVercel
    } else if normalized.contains("what changed in vercel")
        || normalized.contains("vercel") && normalized.contains("changed")
    {
        AskIntent::WhatChangedInVercel
    } else if normalized.contains("what changed") {
        AskIntent::WhatChanged
    } else {
        AskIntent::Help
    }
}

pub fn init(store: &LocalMemoryStore) -> Result<String> {
    let snapshot = store.init()?;
    Ok(format!(
        "Rivora initialized.\n\nLocal memory store:\n{}\n\nMemories: {}\nFeedback: {}\nReceipts: {}\nEvidence: {}",
        display_path(&store.memories_path()),
        snapshot.memories.len(),
        snapshot.feedback.len(),
        snapshot.receipts.len(),
        snapshot.evidence.len()
    ))
}

pub fn remember(store: &LocalMemoryStore, options: RememberOptions) -> Result<String> {
    let mut snapshot = store.init()?;
    let from_evidence_item;
    let request = if let Some(evidence_id) = &options.from_evidence {
        let evidence = snapshot
            .evidence
            .iter()
            .find(|item| item.id.as_str() == evidence_id)
            .ok_or_else(|| RivoraError::invalid_value("evidence_id", "evidence was not found"))?;
        from_evidence_item = Some(evidence.clone());
        candidate_request_from_evidence(evidence, &snapshot.memories)?
    } else {
        let service = required(options.service.as_deref(), "service")?;
        let summary = required(options.summary.as_deref(), "summary")?;
        let confidence = parse_confidence(options.confidence.as_deref())?;
        let id = next_memory_id(&snapshot.memories, service, summary);
        let evidence = if options.evidence.is_empty() {
            vec![format!("cli:{service}:{id}")]
        } else {
            options.evidence.clone()
        };

        let mut symptoms = options.symptoms.clone();
        symptoms.extend(options.tags.iter().cloned());

        from_evidence_item = None;
        MemoryCandidateRequest {
            id,
            kind: MemoryKind::OperationalNote,
            scope: MemoryScope::Service,
            service: service.to_string(),
            symptoms,
            event_summary: summary.to_string(),
            evidence_ids: evidence,
            source: options.source.unwrap_or_else(|| CLI_SOURCE.to_string()),
            source_version: CLI_VERSION.to_string(),
            confidence,
            observed_at: DEFAULT_TIMESTAMP.to_string(),
            learned_at: DEFAULT_TIMESTAMP.to_string(),
        }
    };

    let candidate = AdaptiveMemoryEngine::new().propose_candidate(request)?;

    let mut memory = candidate.memory;
    memory
        .receipt_ids
        .push(candidate.receipt.id.as_str().to_string());
    let mut receipts = vec![candidate.receipt];
    let mut feedback_recorded = None;

    if options.approve {
        let feedback = build_feedback(
            memory.id.as_str(),
            FeedbackKind::Approved,
            Some("Approved from rivora remember --approve"),
        )?;
        let applied = AdaptiveMemoryEngine::new().apply_feedback(&memory, feedback.clone())?;
        memory = applied.memory;
        memory.receipt_ids.extend(
            applied
                .receipts
                .iter()
                .map(|receipt| receipt.id.as_str().to_string()),
        );
        receipts.extend(applied.receipts);
        feedback_recorded = Some(feedback);
    }

    snapshot.memories.push(memory.clone());
    store.save_memories(&snapshot.memories)?;
    store.append_receipts(receipts)?;
    if let Some(feedback) = feedback_recorded {
        store.append_feedback(feedback)?;
    }

    Ok(match from_evidence_item {
        Some(evidence) => render_remembered_from_evidence(&memory, &evidence),
        None => render_remembered(&memory),
    })
}

pub fn recall(store: &LocalMemoryStore, options: RecallOptions) -> Result<String> {
    let result = execute_recall(store, options)?;
    Ok(render_recall_result(&result))
}

fn execute_recall(store: &LocalMemoryStore, options: RecallOptions) -> Result<RecallResult> {
    store.init()?;
    let snapshot = store.load()?;
    let service = options.service.clone();
    let result = AdaptiveMemoryEngine::new().recall(
        RecallQuery {
            service,
            symptoms: options.symptoms,
            tags: options.tags,
            include_candidates: options.include_candidates,
            limit: 10,
            min_score: 0.01,
            generated_at: DEFAULT_TIMESTAMP.to_string(),
            ..RecallQuery::default()
        },
        &snapshot.memories,
    )?;
    store.append_receipts([result.receipt.clone()])?;
    Ok(result)
}

pub fn feedback(store: &LocalMemoryStore, options: FeedbackOptions) -> Result<String> {
    let mut snapshot = store.init()?;
    let index = snapshot
        .memories
        .iter()
        .position(|memory| memory.id.as_str() == options.memory_id)
        .ok_or_else(|| RivoraError::invalid_value("memory_id", "memory was not found"))?;
    let kind = options.kind.as_feedback_kind();
    let feedback = build_feedback(&options.memory_id, kind, options.note.as_deref())?;
    let applied =
        AdaptiveMemoryEngine::new().apply_feedback(&snapshot.memories[index], feedback.clone())?;

    let mut updated = applied.memory;
    updated.receipt_ids.extend(
        applied
            .receipts
            .iter()
            .map(|receipt| receipt.id.as_str().to_string()),
    );
    snapshot.memories[index] = updated.clone();
    store.save_memories(&snapshot.memories)?;
    store.append_feedback(feedback)?;
    store.append_receipts(applied.receipts)?;

    Ok(format!(
        "Memory updated.\n\nMemory: {}\nStatus: {}\nFeedback: {}\n\nNo action was taken.",
        updated.id.as_str(),
        status_label(updated.status),
        options.kind.as_str()
    ))
}

pub fn ingest(store: &LocalMemoryStore, options: IngestOptions) -> Result<String> {
    match options {
        IngestOptions::Git(options) => ingest_git(store, options),
        IngestOptions::GitHub(options) => ingest_github(store, options),
        IngestOptions::Vercel(options) => ingest_vercel(store, options),
        IngestOptions::Fixture(options) => ingest_fixture(store, options),
    }
}

pub fn ingest_git(store: &LocalMemoryStore, options: GitIngestOptions) -> Result<String> {
    store.init()?;
    let repo_path = if options.repo.is_absolute() {
        options.repo.clone()
    } else {
        store.root.join(&options.repo)
    };
    let connector = LocalGitConnector::new(&repo_path);
    let result = connector.ingest_recent(options.since.clone(), options.limit)?;
    let added = store.append_evidence(result.evidence.clone())?;
    Ok(render_ingest_result(&result, added, store))
}

pub fn ingest_github(store: &LocalMemoryStore, options: GitHubIngestOptions) -> Result<String> {
    let auth = GitHubAuthConfig::from_env();
    let connector = GitHubConnector::new(HttpGitHubClient::new(auth));
    ingest_github_with_connector(store, options, &connector)
}

/// Core GitHub ingest path that accepts an injected connector. Tests use this
/// with a `FixtureGitHubClient` so no live GitHub network access is
/// required.
pub fn ingest_github_with_connector(
    store: &LocalMemoryStore,
    options: GitHubIngestOptions,
    connector: &GitHubConnector,
) -> Result<String> {
    store.init()?;
    let repo = GitHubRepositoryRef::parse(&options.repo)?;
    let mut request = GitHubIngestRequest::new(repo).with_limit(options.limit);
    if let Some(since) = options.since.clone() {
        request = request.with_since(since);
    }
    if options.pull_requests {
        request = request.with_pull_requests();
    }
    if options.issues {
        request = request.with_issues();
    }
    if options.workflow_runs {
        request = request.with_workflow_runs();
    }
    if options.releases {
        request = request.with_releases();
    }
    if options.deployments {
        request = request.with_deployments();
    }
    let result = connector.ingest(request)?;
    let added = store.append_evidence(result.evidence.clone())?;
    Ok(render_github_ingest_result(&result, added, store))
}

pub fn ingest_vercel(store: &LocalMemoryStore, options: VercelIngestOptions) -> Result<String> {
    if options.project.trim().is_empty() {
        return Err(RivoraError::invalid_value(
            "vercel_project",
            "rivora ingest vercel requires --project <project-id-or-name>",
        ));
    }
    let auth = VercelAuthConfig::from_env();
    if !auth.has_token() {
        return Err(RivoraError::invalid_value(
            "vercel_token",
            "Missing VERCEL_TOKEN.\n\nCreate a Vercel access token and run:\n\nexport VERCEL_TOKEN=...\n\nThen try:\nrivora ingest vercel --project <project> --limit 20\n\nNo infrastructure actions were taken.",
        ));
    }
    let connector = VercelConnector::new(HttpVercelClient::new(auth));
    ingest_vercel_with_connector(store, options, &connector)
}

/// Core Vercel ingest path that accepts an injected connector. Tests use this
/// with a `FixtureVercelClient` so no live Vercel network access is required.
pub fn ingest_vercel_with_connector(
    store: &LocalMemoryStore,
    options: VercelIngestOptions,
    connector: &VercelConnector,
) -> Result<String> {
    store.init()?;
    let project = VercelProjectRef::parse(&options.project)?;
    let project = VercelProjectRef::new(project.project, options.team.or(project.team));
    let mut request = VercelIngestRequest::new(project).with_limit(options.limit);
    if let Some(since) = options.since.clone() {
        request = request.with_since(since);
    }
    let result = connector.ingest(request)?;
    let added = store.append_evidence(result.evidence.clone())?;
    Ok(render_vercel_ingest_result(&result, added, store))
}

pub fn ingest_fixture(store: &LocalMemoryStore, options: FixtureIngestOptions) -> Result<String> {
    store.init()?;
    let path = if options.path.is_absolute() {
        options.path
    } else {
        store.root.join(options.path)
    };
    let raw = fs::read_to_string(&path)?;
    ingest_fixture_data(store, &raw, &display_path(&path))
}

fn ingest_packaged_demo_fixture(
    store: &LocalMemoryStore,
    scenario: DemoScenario,
) -> Result<String> {
    store.init()?;
    ingest_fixture_data(
        store,
        demo_fixtures::packaged_demo_fixture(scenario),
        &format!("packaged scenario '{}'", scenario.as_str()),
    )
}

fn ingest_fixture_data(store: &LocalMemoryStore, raw: &str, label: &str) -> Result<String> {
    let evidence: Vec<EvidenceItem> = serde_json::from_str(raw).map_err(|error| {
        RivoraError::invalid_value(
            "fixture_evidence",
            format!("fixture evidence must be a JSON array of evidence items: {error}"),
        )
    })?;
    let added = store.append_evidence(evidence.clone())?;
    Ok(render_fixture_ingest_result(
        label,
        evidence.len(),
        added,
        store,
    ))
}

pub fn evidence_command(store: &LocalMemoryStore, command: EvidenceCommand) -> Result<String> {
    store.init()?;
    match command {
        EvidenceCommand::List => evidence_list(store),
        EvidenceCommand::Show(id) => evidence_show(store, &id),
    }
}

pub fn evidence_list(store: &LocalMemoryStore) -> Result<String> {
    let snapshot = store.load()?;
    if snapshot.evidence.is_empty() {
        return Ok(
            "No evidence found yet.\n\nTry:\nrivora ingest git --repo . --limit 20\n\nOr run:\nrivora demo\n\nNo infrastructure actions were taken."
                .to_string(),
        );
    }

    let mut output = format!("Local evidence: {}\n", snapshot.evidence.len());
    for item in snapshot.evidence.iter().take(20) {
        output.push_str(&format!(
            "\n* {}\n  Kind: {}\n  Summary: {}",
            item.id.as_str(),
            item.kind.as_str(),
            item.summary
        ));
        if let Some(service) = &item.service {
            output.push_str(&format!("\n  Topic: {service}"));
        }
    }
    output.push_str("\n\nNo infrastructure actions were taken.");
    Ok(output)
}

pub fn evidence_show(store: &LocalMemoryStore, id: &str) -> Result<String> {
    let snapshot = store.load()?;
    let item = snapshot
        .evidence
        .iter()
        .find(|item| item.id.as_str() == id)
        .ok_or_else(|| RivoraError::invalid_value("evidence_id", "evidence was not found"))?;
    Ok(render_evidence_item(item))
}

pub fn ask(store: &LocalMemoryStore, prompt: &str) -> Result<String> {
    match parse_ask_intent(prompt) {
        AskIntent::Recall => {
            let recall_target = recall_target_from_prompt(prompt);
            recall(
                store,
                RecallOptions {
                    service: recall_target,
                    symptoms: symptoms_from_prompt(prompt),
                    tags: Vec::new(),
                    include_candidates: true,
                },
            )
        }
        AskIntent::RememberPrompt => {
            let service = service_after_about(prompt);
            if let Some(service) = service {
                Ok(format!(
                    "This may be worth remembering.\n\nNeeds review: provide a summary and evidence before Rivora creates a candidate.\n\nTry:\nrivora remember --service {service} --summary \"...\" --evidence \"...\"\n\nNo action was taken."
                ))
            } else {
                Ok(format!(
                    "This may be worth remembering.\n\nNeeds review: provide service, summary, and evidence before Rivora creates a candidate.\n\nTry:\n{}\n\nNo action was taken.",
                    remember_example()
                ))
            }
        }
        AskIntent::WhatChanged => ask_what_changed(store, prompt),
        AskIntent::WhatMergedRecently => ask_what_merged_recently(store),
        AskIntent::WhatFailedRecently => ask_what_failed_recently(store),
        AskIntent::WhatChangedInGithub => ask_what_changed_in_github(store),
        AskIntent::WhatDeployedRecently => ask_what_deployed_recently(store),
        AskIntent::WhatFailedInVercel => ask_what_failed_in_vercel(store),
        AskIntent::WhatChangedInVercel => ask_what_changed_in_vercel(store),
        AskIntent::Help => Ok(help_text()),
    }
}

pub fn demo(store: &LocalMemoryStore, options: DemoOptions) -> Result<String> {
    let scenario = options.scenario;
    let scenario_config = scenario.config();
    let explicit_root = options
        .store
        .as_ref()
        .map(|path| absolute_or_rooted(&store.root, path));
    let temp_root = if explicit_root.is_none() {
        Some(new_demo_temp_dir()?)
    } else {
        None
    };
    let demo_root = explicit_root
        .as_deref()
        .or(temp_root.as_deref())
        .ok_or_else(|| RivoraError::invalid_value("demo_store", "demo store was not available"))?;
    let demo_store = LocalMemoryStore::new(demo_root);

    let mut output = format!(
        "Rivora Demo\n\nScenario: {}\nThis demo uses packaged fixture data.\nNo tokens are required.\nNo network is required.\nNo data leaves your machine.\nEvidence is not memory until approved.\nNo infrastructure actions will be taken.\n\n",
        scenario.as_str()
    );
    output.push_str("1. Ingesting demo evidence...\n");
    let ingest_output = ingest_packaged_demo_fixture(&demo_store, scenario)?;
    let snapshot = demo_store.load()?;
    let selected_evidence_id = snapshot
        .evidence
        .iter()
        .find(|item| item.id.as_str() == scenario_config.selected_evidence_id)
        .map(|item| item.id.as_str().to_string())
        .ok_or_else(|| {
            RivoraError::invalid_value(
                "demo_fixture",
                format!(
                    "scenario '{}' did not contain selected evidence '{}'",
                    scenario.as_str(),
                    scenario_config.selected_evidence_id
                ),
            )
        })?;
    output.push_str("2. Creating a memory candidate...\n");
    let remember_output = remember(
        &demo_store,
        RememberOptions {
            from_evidence: Some(selected_evidence_id.clone()),
            ..RememberOptions::default()
        },
    )?;
    let memory_id = demo_store
        .load()?
        .memories
        .first()
        .map(|memory| memory.id.as_str().to_string())
        .ok_or_else(|| RivoraError::invalid_value("demo_memory", "demo memory was not created"))?;
    output.push_str("3. Approving the memory...\n");
    let approve_output = feedback(
        &demo_store,
        FeedbackOptions {
            memory_id: memory_id.clone(),
            kind: FeedbackCommandKind::Approve,
            note: Some("Approved during local Rivora demo".to_string()),
        },
    )?;
    output.push_str("4. Recalling similar memory...\n");
    let recall_result = execute_recall(
        &demo_store,
        RecallOptions {
            service: Some(scenario_config.recall_service.to_string()),
            symptoms: scenario_config
                .recall_symptoms
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            tags: scenario_config
                .recall_tags
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            include_candidates: false,
        },
    )?;
    let recall_output = render_recall_result(&recall_result);
    output.push_str("5. Asking Rivora...\n");
    let ask_output = ask(&demo_store, scenario_config.ask_prompt)?;
    output.push_str("6. Rendering Slack-style response...\n");
    let slack_output = run_slack_dev(
        &demo_store,
        SlackDevOptions {
            text: scenario_config.slack_prompt.to_string(),
            channel: "CDEMO".to_string(),
            user: "UDEMO".to_string(),
            bot_user_id: None,
        },
    )?;

    if options.json {
        let snapshot = demo_store.load()?;
        let final_memory = snapshot
            .memories
            .iter()
            .find(|memory| memory.id.as_str() == memory_id)
            .ok_or_else(|| {
                RivoraError::invalid_value("demo_memory", "approved demo memory was not found")
            })?;
        output = serde_json::to_string_pretty(&DemoSummary {
            demo: "complete",
            scenario: scenario.as_str(),
            evidence_count: snapshot.evidence.len(),
            selected_evidence_id: &selected_evidence_id,
            memory_id: &memory_id,
            final_memory_status: status_label(final_memory.status).to_ascii_lowercase(),
            recall_match_count: recall_result.matches.len(),
            slack_dev_rendered: true,
            message: "Evidence -> Memory Candidate -> Human Approval -> Recall",
            human_control_summary: "Evidence is not memory until approved.",
            safety_summary: "No infrastructure actions were taken.",
        })?;
    } else {
        output.push_str("\nRecent evidence:\n");
        output.push_str(&compact_section(&ingest_output));
        output.push_str("\n\nMemory candidate:\n");
        output.push_str(&compact_section(&remember_output));
        output.push_str("\n\nHuman approval:\n");
        output.push_str(&compact_section(&approve_output));
        output.push_str("\n\nRecall:\n");
        output.push_str(&compact_section(&recall_output));
        output.push_str("\n\nAsk:\n");
        output.push_str(&compact_section(&ask_output));
        output.push_str("\n\nSlack dev:\n");
        output.push_str(&compact_section(&slack_output));
        output.push_str("\n\nDemo complete.\n\nYou just saw:\nEvidence -> Memory Candidate -> Human Approval -> Recall\n\nEvidence is not memory until a human approves it.\nNo root cause was inferred without evidence.\nNo infrastructure actions were taken.");
        if options.keep || options.store.is_some() {
            output.push_str(&format!(
                "\n\nDemo store kept at:\n{}",
                display_path(&demo_store.store_dir())
            ));
        }
    }

    if !options.keep && options.store.is_none() {
        fs::remove_dir_all(demo_root)?;
    }

    Ok(output)
}

pub fn status(store: &LocalMemoryStore) -> Result<String> {
    let snapshot = store.init()?;
    let counts = StatusCounts::from_memories(&snapshot.memories);
    Ok(format!(
        "Rivora status\n\nStore: {}/\n\nMemories:\n\n* Total: {}\n* Candidate: {}\n* Active: {}\n* Rejected: {}\n* Corrected: {}\n\nEvidence: {}\nFeedback: {}\nReceipts: {}",
        STORE_DIR,
        counts.total,
        counts.candidate,
        counts.active,
        counts.rejected,
        counts.corrected,
        snapshot.evidence.len(),
        snapshot.feedback.len(),
        snapshot.receipts.len()
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StatusCounts {
    pub total: usize,
    pub candidate: usize,
    pub active: usize,
    pub rejected: usize,
    pub corrected: usize,
}

impl StatusCounts {
    #[must_use]
    pub fn from_memories(memories: &[MemoryRecord]) -> Self {
        let mut counts = Self {
            total: memories.len(),
            ..Self::default()
        };
        for memory in memories {
            match memory.status {
                MemoryStatus::Candidate => counts.candidate += 1,
                MemoryStatus::Active => counts.active += 1,
                MemoryStatus::Rejected => counts.rejected += 1,
                MemoryStatus::Corrected => counts.corrected += 1,
                _ => {}
            }
        }
        counts
    }
}

#[must_use]
pub fn output_contains_infrastructure_action(output: &str) -> bool {
    let normalized = normalize(output);
    [
        "rollback action",
        "remediation action",
        "deploy action",
        "deployment action",
        "scale action",
        "restart action",
        "execute rollback",
        "execute remediation",
        "execute deploy",
        "execute restart",
        "mutate infrastructure",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn parse_remember_options(args: &[String]) -> Result<RememberOptions> {
    let mut options = RememberOptions::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--service" => options.service = Some(flag_value(args, &mut i, "--service")?),
            "--summary" => options.summary = Some(flag_value(args, &mut i, "--summary")?),
            "--from-evidence" => {
                options.from_evidence = Some(flag_value(args, &mut i, "--from-evidence")?);
            }
            "--symptom" => options
                .symptoms
                .push(flag_value(args, &mut i, "--symptom")?),
            "--tag" => options.tags.push(flag_value(args, &mut i, "--tag")?),
            "--evidence" => options
                .evidence
                .push(flag_value(args, &mut i, "--evidence")?),
            "--source" => options.source = Some(flag_value(args, &mut i, "--source")?),
            "--confidence" => options.confidence = Some(flag_value(args, &mut i, "--confidence")?),
            "--approve" => {
                options.approve = true;
                i += 1;
            }
            other => {
                return Err(RivoraError::invalid_value(
                    "remember_flag",
                    format!("unsupported remember flag '{other}'"),
                ));
            }
        }
    }
    Ok(options)
}

fn parse_recall_options(args: &[String]) -> Result<RecallOptions> {
    let mut options = RecallOptions::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--symptom" => options
                .symptoms
                .push(flag_value(args, &mut i, "--symptom")?),
            "--tag" => options.tags.push(flag_value(args, &mut i, "--tag")?),
            "--include-candidates" => {
                options.include_candidates = true;
                i += 1;
            }
            value if value.starts_with("--") => {
                return Err(RivoraError::invalid_value(
                    "recall_flag",
                    format!("unsupported recall flag '{value}'"),
                ));
            }
            value => {
                if options.service.is_some() {
                    return Err(RivoraError::invalid_value(
                        "recall_service",
                        "recall accepts one service/topic argument",
                    ));
                }
                options.service = Some(value.to_string());
                i += 1;
            }
        }
    }
    Ok(options)
}

fn parse_ingest_options(args: &[String]) -> Result<IngestOptions> {
    match args.first().map(String::as_str) {
        Some("git") => Ok(IngestOptions::Git(parse_git_ingest_options(&args[1..])?)),
        Some("github") => Ok(IngestOptions::GitHub(parse_github_ingest_options(
            &args[1..],
        )?)),
        Some("vercel") => Ok(IngestOptions::Vercel(parse_vercel_ingest_options(
            &args[1..],
        )?)),
        Some("fixture") => Ok(IngestOptions::Fixture(parse_fixture_ingest_options(
            &args[1..],
        )?)),
        Some(other) => Err(RivoraError::invalid_value(
            "ingest_connector",
            format!("unsupported ingest connector '{other}'"),
        )),
        None => Err(RivoraError::invalid_value(
            "ingest_connector",
            "usage: rivora ingest git | rivora ingest github --repo owner/name | rivora ingest vercel --project <project> | rivora ingest fixture --path examples/demo/evidence.json",
        )),
    }
}

fn parse_demo_options(args: &[String]) -> Result<DemoOptions> {
    let mut options = DemoOptions::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--keep" => {
                options.keep = true;
                i += 1;
            }
            "--json" => {
                options.json = true;
                i += 1;
            }
            "--scenario" => {
                options.scenario = DemoScenario::parse(&flag_value(args, &mut i, "--scenario")?)?
            }
            "--store" => options.store = Some(PathBuf::from(flag_value(args, &mut i, "--store")?)),
            other => {
                return Err(RivoraError::invalid_value(
                    "demo_flag",
                    format!("unsupported demo flag '{other}'"),
                ));
            }
        }
    }
    Ok(options)
}

fn parse_git_ingest_options(args: &[String]) -> Result<GitIngestOptions> {
    let mut options = GitIngestOptions::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--repo" => options.repo = PathBuf::from(flag_value(args, &mut i, "--repo")?),
            "--since" => options.since = Some(flag_value(args, &mut i, "--since")?),
            "--limit" => {
                let value = flag_value(args, &mut i, "--limit")?;
                options.limit = value.parse::<usize>().map_err(|_| {
                    RivoraError::invalid_value("limit", "limit must be a positive integer")
                })?;
            }
            other => {
                return Err(RivoraError::invalid_value(
                    "ingest_git_flag",
                    format!("unsupported git ingest flag '{other}'"),
                ));
            }
        }
    }
    Ok(options)
}

fn parse_github_ingest_options(args: &[String]) -> Result<GitHubIngestOptions> {
    let mut options = GitHubIngestOptions::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--repo" => options.repo = flag_value(args, &mut i, "--repo")?,
            "--since" => options.since = Some(flag_value(args, &mut i, "--since")?),
            "--limit" => {
                let value = flag_value(args, &mut i, "--limit")?;
                options.limit = value.parse::<usize>().map_err(|_| {
                    RivoraError::invalid_value("limit", "limit must be a positive integer")
                })?;
            }
            "--pull-requests" => {
                options.pull_requests = true;
                i += 1;
            }
            "--issues" => {
                options.issues = true;
                i += 1;
            }
            "--workflow-runs" => {
                options.workflow_runs = true;
                i += 1;
            }
            "--releases" => {
                options.releases = true;
                i += 1;
            }
            "--deployments" => {
                options.deployments = true;
                i += 1;
            }
            other => {
                return Err(RivoraError::invalid_value(
                    "ingest_github_flag",
                    format!("unsupported github ingest flag '{other}'"),
                ));
            }
        }
    }
    if options.repo.trim().is_empty() {
        return Err(RivoraError::invalid_value(
            "github_repo",
            "rivora ingest github requires --repo owner/name",
        ));
    }
    Ok(options)
}

fn parse_vercel_ingest_options(args: &[String]) -> Result<VercelIngestOptions> {
    let mut options = VercelIngestOptions::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--project" => options.project = flag_value(args, &mut i, "--project")?,
            "--team" => options.team = Some(flag_value(args, &mut i, "--team")?),
            "--since" => options.since = Some(flag_value(args, &mut i, "--since")?),
            "--limit" => {
                let value = flag_value(args, &mut i, "--limit")?;
                options.limit = value.parse::<usize>().map_err(|_| {
                    RivoraError::invalid_value("limit", "limit must be a positive integer")
                })?;
            }
            other => {
                return Err(RivoraError::invalid_value(
                    "ingest_vercel_flag",
                    format!("unsupported vercel ingest flag '{other}'"),
                ));
            }
        }
    }
    if options.project.trim().is_empty() {
        return Err(RivoraError::invalid_value(
            "vercel_project",
            "rivora ingest vercel requires --project <project-id-or-name>",
        ));
    }
    Ok(options)
}

fn parse_fixture_ingest_options(args: &[String]) -> Result<FixtureIngestOptions> {
    let mut path = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--path" => path = Some(PathBuf::from(flag_value(args, &mut i, "--path")?)),
            other => {
                return Err(RivoraError::invalid_value(
                    "ingest_fixture_flag",
                    format!("unsupported fixture ingest flag '{other}'"),
                ));
            }
        }
    }
    Ok(FixtureIngestOptions {
        path: path.ok_or_else(|| {
            RivoraError::invalid_value(
                "fixture_path",
                "rivora ingest fixture requires --path examples/demo/evidence.json",
            )
        })?,
    })
}

fn parse_evidence_command(args: &[String]) -> Result<EvidenceCommand> {
    match args.first().map(String::as_str) {
        Some("list") => Ok(EvidenceCommand::List),
        Some("show") => {
            let id = args.get(1).ok_or_else(|| {
                RivoraError::invalid_value("evidence_id", "usage: rivora evidence show <id>")
            })?;
            Ok(EvidenceCommand::Show(id.clone()))
        }
        Some(other) => Err(RivoraError::invalid_value(
            "evidence_command",
            format!("unsupported evidence command '{other}'"),
        )),
        None => Err(RivoraError::invalid_value(
            "evidence_command",
            "usage: rivora evidence list",
        )),
    }
}

fn parse_slack_command(args: &[String]) -> Result<SlackCommand> {
    match args.first().map(String::as_str) {
        Some("dev") => {
            let mut text = None;
            let mut channel = "CLOCAL".to_string();
            let mut user = "ULOCAL".to_string();
            let mut bot_user_id = None;
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--text" => text = Some(flag_value(args, &mut i, "--text")?),
                    "--channel" => channel = flag_value(args, &mut i, "--channel")?,
                    "--user" => user = flag_value(args, &mut i, "--user")?,
                    "--bot-user-id" => {
                        bot_user_id = Some(flag_value(args, &mut i, "--bot-user-id")?)
                    }
                    other => {
                        return Err(RivoraError::invalid_value(
                            "slack_dev_flag",
                            format!("unsupported slack dev flag '{other}'"),
                        ));
                    }
                }
            }
            Ok(SlackCommand::Dev(SlackDevOptions {
                text: required(text.as_deref(), "text")?.to_string(),
                channel,
                user,
                bot_user_id,
            }))
        }
        Some("doctor") => {
            let mut live = false;
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--live" => {
                        live = true;
                        i += 1;
                    }
                    other => {
                        return Err(RivoraError::invalid_value(
                            "slack_doctor_flag",
                            format!("unsupported slack doctor flag '{other}'"),
                        ));
                    }
                }
            }
            Ok(SlackCommand::Doctor(SlackDoctorOptions { live }))
        }
        Some("socket") => {
            if args.len() > 1 {
                return Err(RivoraError::invalid_value(
                    "slack_socket_flag",
                    "rivora slack socket does not accept flags yet",
                ));
            }
            Ok(SlackCommand::Socket(SlackSocketOptions))
        }
        Some(other) => Err(RivoraError::invalid_value(
            "slack_command",
            format!("unsupported slack command '{other}'"),
        )),
        None => Err(RivoraError::invalid_value(
            "slack_command",
            "usage: rivora slack doctor | rivora slack dev --text \"...\" | rivora slack socket",
        )),
    }
}

fn parse_feedback_options(args: &[String]) -> Result<FeedbackOptions> {
    if args.len() < 2 {
        return Err(RivoraError::invalid_value(
            "feedback",
            "usage: rivora feedback <memory-id> <kind>",
        ));
    }
    let memory_id = args[0].clone();
    let kind = parse_feedback_kind(&args[1])?;
    let mut note = None;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--note" => note = Some(flag_value(args, &mut i, "--note")?),
            other => {
                return Err(RivoraError::invalid_value(
                    "feedback_flag",
                    format!("unsupported feedback flag '{other}'"),
                ));
            }
        }
    }
    Ok(FeedbackOptions {
        memory_id,
        kind,
        note,
    })
}

fn parse_feedback_kind(value: &str) -> Result<FeedbackCommandKind> {
    match value {
        "approve" => Ok(FeedbackCommandKind::Approve),
        "reject" => Ok(FeedbackCommandKind::Reject),
        "correct" => Ok(FeedbackCommandKind::Correct),
        "useful" => Ok(FeedbackCommandKind::Useful),
        "not-useful" => Ok(FeedbackCommandKind::NotUseful),
        "needs-more-evidence" => Ok(FeedbackCommandKind::NeedsMoreEvidence),
        other => Err(RivoraError::invalid_value(
            "feedback_kind",
            format!("unsupported feedback kind '{other}'"),
        )),
    }
}

fn flag_value(args: &[String], index: &mut usize, flag: &'static str) -> Result<String> {
    let value = args
        .get(*index + 1)
        .filter(|value| !value.starts_with("--"))
        .ok_or_else(|| RivoraError::invalid_value(flag, "flag requires a value"))?
        .clone();
    *index += 2;
    Ok(value)
}

fn required<'a>(value: Option<&'a str>, field: &'static str) -> Result<&'a str> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| RivoraError::invalid_value(field, format!("{field} is required")))
}

fn parse_confidence(value: Option<&str>) -> Result<f64> {
    match value {
        None | Some("medium") => Ok(0.6),
        Some("low") => Ok(0.3),
        Some("high") => Ok(0.85),
        Some(raw) => raw.parse::<f64>().map_err(|_| {
            RivoraError::invalid_value("confidence", "use low, medium, high, or a numeric score")
        }),
    }
}

fn build_feedback(
    memory_id: &str,
    kind: FeedbackKind,
    note: Option<&str>,
) -> Result<HumanFeedback> {
    let feedback_id = format!("feedback-{}-{memory_id}", kind.as_str());
    let mut feedback = HumanFeedback::new(
        feedback_id,
        memory_id,
        FeedbackTargetType::Memory,
        "local-engineer",
        FeedbackSource::Cli,
        kind,
        DEFAULT_TIMESTAMP,
    )?;
    if let Some(note) = note {
        feedback = feedback.with_note(note);
        if kind == FeedbackKind::Corrected {
            feedback = feedback.with_correction_text(note);
        }
    } else if kind == FeedbackKind::Corrected {
        feedback = feedback.with_correction_text("Corrected from CLI feedback");
    }
    Ok(feedback)
}

fn candidate_request_from_evidence(
    evidence: &EvidenceItem,
    memories: &[MemoryRecord],
) -> Result<MemoryCandidateRequest> {
    let service = evidence
        .service
        .clone()
        .or_else(|| evidence.tags.first().cloned())
        .unwrap_or_else(|| "local-git".to_string());
    let summary = if evidence.summary.trim().is_empty() {
        evidence.title.clone()
    } else {
        evidence.summary.clone()
    };
    Ok(MemoryCandidateRequest {
        id: next_memory_id(memories, &service, &summary),
        kind: MemoryKind::OperationalNote,
        scope: MemoryScope::Service,
        service,
        symptoms: evidence.tags.clone(),
        event_summary: summary,
        evidence_ids: vec![evidence.id.as_str().to_string()],
        source: evidence.source.connector.clone(),
        source_version: evidence.source.version.clone(),
        confidence: evidence.confidence,
        observed_at: evidence
            .timestamp
            .clone()
            .unwrap_or_else(|| DEFAULT_TIMESTAMP.to_string()),
        learned_at: DEFAULT_TIMESTAMP.to_string(),
    })
}

fn render_ingest_result(
    result: &EvidenceIngestResult,
    added: usize,
    store: &LocalMemoryStore,
) -> String {
    format!(
        "Rivora ingested Git evidence.\n\nRepository: {}\nEvidence items ingested: {}\nNew evidence items: {}\nCommits: {}\nFile changes: {}\nInferred topics:\n{}\n\nEvidence store:\n{}\n\nNo infrastructure actions were taken.",
        result.repository,
        result.evidence.len(),
        added,
        result.commits,
        result.file_changes,
        render_bullets(&result.topics),
        display_path(&store.evidence_path())
    )
}

fn render_github_ingest_result(
    result: &GitHubIngestResult,
    added: usize,
    store: &LocalMemoryStore,
) -> String {
    format!(
        "Rivora ingested GitHub evidence.\n\nRepository: {}\nEvidence items ingested: {}\nNew evidence items: {}\nPull requests: {}\nIssues: {}\nWorkflow runs: {}\nReleases: {}\nDeployments: {}\nInferred topics:\n{}\n\nEvidence store:\n{}\n\nGitHub access is read-only. No infrastructure actions were taken.",
        result.repository,
        result.evidence.len(),
        added,
        result.pull_requests,
        result.issues,
        result.workflow_runs,
        result.releases,
        result.deployments,
        render_bullets(&result.topics),
        display_path(&store.evidence_path())
    )
}

fn render_vercel_ingest_result(
    result: &VercelIngestResult,
    added: usize,
    store: &LocalMemoryStore,
) -> String {
    format!(
        "Rivora ingested Vercel evidence.\n\nProject: {}\nEvidence items ingested: {}\nNew evidence items: {}\nDeployments: {}\nInferred topics:\n{}\n\nEvidence store:\n{}\n\nVercel access is read-only. No infrastructure actions were taken.",
        result.repository,
        result.evidence.len(),
        added,
        result.deployments,
        render_bullets(&result.topics),
        display_path(&store.evidence_path())
    )
}

fn render_fixture_ingest_result(
    label: &str,
    ingested: usize,
    added: usize,
    store: &LocalMemoryStore,
) -> String {
    format!(
        "Rivora ingested fixture evidence.\n\nFixture: {}\nEvidence items ingested: {}\nNew evidence items: {}\nEvidence store:\n{}\n\nFixture evidence is local demo data. No infrastructure actions were taken.",
        label,
        ingested,
        added,
        display_path(&store.evidence_path())
    )
}

fn render_evidence_item(item: &EvidenceItem) -> String {
    format!(
        "Evidence: {}\nKind: {}\nSummary: {}\nTopic: {}\nConfidence: {}\nFiles:\n{}\n\nDetails:\n{}\n\nThis may be worth remembering.\n\nRun:\nrivora remember --from-evidence {}\n\nNo infrastructure actions were taken.",
        item.id.as_str(),
        item.kind.as_str(),
        item.summary,
        item.service.as_deref().unwrap_or("unknown"),
        confidence_label(item.confidence),
        render_bullets(&item.files_changed),
        item.body,
        item.id.as_str()
    )
}

fn ask_what_changed(store: &LocalMemoryStore, prompt: &str) -> Result<String> {
    store.init()?;
    let snapshot = store.load()?;
    let topic = service_after_in(prompt).or_else(|| service_after_about(prompt));
    let mut matches = snapshot
        .evidence
        .iter()
        .filter(|item| evidence_matches_topic(item, topic.as_deref()))
        .take(5)
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return Ok(
            "No evidence found yet.\n\nTry:\nrivora ingest git --repo . --limit 20\nrivora ingest github --repo owner/name --limit 20\nrivora ingest vercel --project <project> --limit 20\n\nOr run:\nrivora demo\n\nNo root cause was inferred.\nNo infrastructure actions were taken."
                .to_string(),
        );
    }
    matches.sort_by(|a, b| b.timestamp.cmp(&a.timestamp).then_with(|| a.id.cmp(&b.id)));
    let has_github = matches.iter().any(|item| item.is_github());
    let has_vercel = matches.iter().any(|item| item.is_vercel());
    let header = if has_github || has_vercel {
        "Rivora found recent evidence."
    } else {
        "Rivora found recent Git evidence."
    };
    let mut output = format!("{header}\n");
    let first_id = matches.first().map(|item| item.id.as_str().to_string());
    for item in matches {
        output.push_str(&format!(
            "\n* {}\n  {}\n  Evidence: {}",
            item.title,
            item.summary,
            item.id.as_str()
        ));
    }
    if let Some(first_id) = first_id {
        output.push_str(&format!(
            "\n\nThis may be worth remembering.\n\nRun:\nrivora remember --from-evidence {}\n\nNo root cause was inferred.\nNo infrastructure actions were taken.",
            first_id
        ));
    }
    Ok(output)
}

fn ask_what_merged_recently(store: &LocalMemoryStore) -> Result<String> {
    ask_github_evidence(
        store,
        "Rivora found recent GitHub merge evidence.",
        |item| item.kind == EvidenceKind::GitHubPullRequestMerged,
    )
}

fn ask_what_failed_recently(store: &LocalMemoryStore) -> Result<String> {
    ask_github_evidence(
        store,
        "Rivora found recent GitHub workflow failures.",
        |item| item.kind == EvidenceKind::GitHubWorkflowFailed,
    )
}

fn ask_what_changed_in_github(store: &LocalMemoryStore) -> Result<String> {
    ask_github_evidence(
        store,
        "Rivora found recent GitHub evidence.",
        EvidenceItem::is_github,
    )
}

fn ask_what_deployed_recently(store: &LocalMemoryStore) -> Result<String> {
    ask_vercel_evidence(
        store,
        "Rivora found recent Vercel deployment evidence.",
        EvidenceItem::is_vercel,
    )
}

fn ask_what_failed_in_vercel(store: &LocalMemoryStore) -> Result<String> {
    ask_vercel_evidence(
        store,
        "Rivora found recent Vercel deployment failures.",
        |item| item.is_vercel() && item.tags.iter().any(|tag| tag == "failed-deploy"),
    )
}

fn ask_what_changed_in_vercel(store: &LocalMemoryStore) -> Result<String> {
    ask_vercel_evidence(
        store,
        "Rivora found recent Vercel evidence.",
        EvidenceItem::is_vercel,
    )
}

fn ask_vercel_evidence(
    store: &LocalMemoryStore,
    header: &str,
    predicate: impl Fn(&EvidenceItem) -> bool,
) -> Result<String> {
    store.init()?;
    let snapshot = store.load()?;
    let mut matches: Vec<&EvidenceItem> = snapshot
        .evidence
        .iter()
        .filter(|item| predicate(item))
        .collect();
    if matches.is_empty() {
        return Ok(
            "Rivora did not find Vercel evidence yet.\n\nTry:\nrivora ingest vercel --project <project> --limit 20\n\nOr run:\nrivora demo\n\nNo root cause was inferred.\nNo infrastructure actions were taken."
                .to_string(),
        );
    }
    matches.sort_by(|a, b| b.timestamp.cmp(&a.timestamp).then_with(|| a.id.cmp(&b.id)));
    let mut output = format!("{header}\n");
    for item in matches.iter().take(5) {
        output.push_str(&format!(
            "\n* {}\n  {}\n  Evidence: {}",
            item.title,
            item.summary,
            item.id.as_str()
        ));
    }
    if let Some(first) = matches.first() {
        output.push_str(&format!(
            "\n\nThis may be worth remembering.\n\nRun:\nrivora remember --from-evidence {}\n\nNo root cause was inferred.\nNo infrastructure actions were taken.",
            first.id.as_str()
        ));
    }
    Ok(output)
}

fn ask_github_evidence(
    store: &LocalMemoryStore,
    header: &str,
    predicate: impl Fn(&EvidenceItem) -> bool,
) -> Result<String> {
    store.init()?;
    let snapshot = store.load()?;
    let mut matches: Vec<&EvidenceItem> = snapshot
        .evidence
        .iter()
        .filter(|item| predicate(item))
        .collect();
    if matches.is_empty() {
        return Ok(
            "Rivora did not find GitHub evidence yet.\n\nTry:\nrivora ingest github --repo owner/name --limit 20\n\nOr run:\nrivora demo\n\nNo root cause was inferred.\nNo infrastructure actions were taken."
                .to_string(),
        );
    }
    matches.sort_by(|a, b| b.timestamp.cmp(&a.timestamp).then_with(|| a.id.cmp(&b.id)));
    let mut output = format!("{header}\n");
    for item in matches.iter().take(5) {
        output.push_str(&format!(
            "\n* {}\n  {}\n  Evidence: {}",
            item.title,
            item.summary,
            item.id.as_str()
        ));
    }
    if let Some(first) = matches.first() {
        output.push_str(&format!(
            "\n\nThis may be worth remembering.\n\nRun:\nrivora remember --from-evidence {}\n\nNo root cause was inferred.\nNo infrastructure actions were taken.",
            first.id.as_str()
        ));
    }
    Ok(output)
}

fn render_remembered(memory: &MemoryRecord) -> String {
    let evidence = memory_evidence(memory);
    format!(
        "This may be worth remembering.\n\nMemory: {}\nStatus: {}\nSummary: {}\nConfidence: {}\nEvidence:\n{}\n\nNo action was taken.",
        memory.id.as_str(),
        status_label(memory.status),
        memory.body.as_str(),
        confidence_label(memory.confidence.score),
        render_bullets(&evidence)
    )
}

fn render_remembered_from_evidence(memory: &MemoryRecord, evidence: &EvidenceItem) -> String {
    let source_label = if evidence.is_github() {
        "GitHub"
    } else if evidence.is_vercel() {
        "Vercel"
    } else {
        "Git"
    };
    let evidence_refs = memory_evidence(memory);
    format!(
        "Memory candidate created from {source_label} evidence.\n\nMemory: {}\nSource: {}\nSummary: {}\nStatus: {}\nConfidence: {}\nEvidence:\n{}\n\nNo action was taken.",
        memory.id.as_str(),
        evidence.kind.label(),
        memory.body.as_str(),
        status_label(memory.status),
        confidence_label(memory.confidence.score),
        render_bullets(&evidence_refs)
    )
}

fn render_recall_result(result: &RecallResult) -> String {
    if result.matches.is_empty() {
        return format!(
            "No similar memories found.\n\nNo approved memories found for this query.\nEvidence is not memory until approved.\n\nTry:\n* {}\n* rivora recall <service> --include-candidates\n* rivora demo\n\nNo action was taken.",
            remember_example()
        );
    }

    let mut output = format!("Similar memories found: {}\n", result.matches.len());
    for (index, matched) in result.matches.iter().enumerate() {
        output.push_str(&format!(
            "\n{}. {}\n   Score: {}\n   Confidence: {}\n   Status: {}\n\n   Why it matched:\n{}\n\n   Evidence:\n{}\n",
            index + 1,
            matched.memory.title.as_str(),
            score_label(matched.score.value),
            confidence_label(matched.confidence),
            status_label(matched.memory.status),
            indent_bullets(&matched.matched_reasons, "   "),
            indent_bullets(&matched.evidence_refs, "   ")
        ));
    }
    output.push_str("\nNo action was taken.");
    output
}

fn help_text() -> String {
    format!(
        "Rivora local-first, evidence-backed reliability memory CLI\n\nCommands:\n* rivora demo\n* rivora demo --scenario <basic|checkout-incident|release-regression|workflow-failure>\n* rivora init\n* rivora ingest fixture --path examples/demo/evidence.json\n* rivora ingest git --repo . --limit 20\n* rivora ingest github --repo owner/name --limit 20\n* rivora ingest vercel --project <project> --limit 20\n* rivora evidence list\n* rivora evidence show <evidence-id>\n* {}\n* rivora remember --from-evidence <evidence-id>\n* rivora recall <service> --symptom latency --include-candidates\n* rivora feedback <memory-id> approve\n* rivora ask \"what changed?\"\n* rivora ask \"what merged recently?\"\n* rivora ask \"what failed recently?\"\n* rivora ask \"what deployed recently?\"\n* rivora ask \"what changed in github?\"\n* rivora ask \"what changed in vercel?\"\n* rivora ask \"have we seen checkout latency before?\"\n* rivora slack doctor\n* rivora slack dev --text \"what changed?\"\n* rivora slack socket\n* rivora status\n\nEvidence is not memory until a human approves it. Rivora proposes and updates memory only. No infrastructure actions are taken.",
        remember_example()
    )
}

fn remember_example() -> &'static str {
    "rivora remember --service checkout-api --summary \"Checkout latency increased\" --evidence deploy-2026-06-27"
}

fn score_label(score: f64) -> String {
    format!("{:.0}", score * 100.0)
}

fn confidence_label(confidence: f64) -> &'static str {
    if confidence >= 0.75 {
        "High"
    } else if confidence >= 0.45 {
        "Medium"
    } else {
        "Low"
    }
}

fn status_label(status: MemoryStatus) -> &'static str {
    match status {
        MemoryStatus::Candidate => "Candidate",
        MemoryStatus::Active => "Active",
        MemoryStatus::Rejected => "Rejected",
        MemoryStatus::Corrected => "Corrected",
        MemoryStatus::Superseded => "Superseded",
        MemoryStatus::Expired => "Expired",
        MemoryStatus::Archived => "Archived",
        MemoryStatus::Invalid => "Invalid",
        MemoryStatus::Draft => "Draft",
    }
}

fn render_bullets(values: &[String]) -> String {
    if values.is_empty() {
        "* none".to_string()
    } else {
        values
            .iter()
            .map(|value| format!("* {value}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn indent_bullets(values: &[String], indent: &str) -> String {
    if values.is_empty() {
        format!("{indent}* none")
    } else {
        values
            .iter()
            .map(|value| format!("{indent}* {value}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn memory_evidence(memory: &MemoryRecord) -> Vec<String> {
    let mut refs = memory.graph_node_ids.clone();
    refs.extend(memory.receipt_ids.clone());
    refs.sort();
    refs.dedup();
    refs
}

fn next_memory_id(memories: &[MemoryRecord], service: &str, summary: &str) -> String {
    let base = format!("mem-cli-{}-{}", slug(service), slug(summary));
    let candidate = format!("{base}-{}", memories.len() + 1);
    if memories
        .iter()
        .all(|memory| memory.id.as_str() != candidate.as_str())
    {
        return candidate;
    }

    for number in 1.. {
        let candidate = format!("{base}-{number}");
        if memories
            .iter()
            .all(|memory| memory.id.as_str() != candidate.as_str())
        {
            return candidate;
        }
    }
    unreachable!("unbounded candidate id search should always return")
}

fn recall_target_from_prompt(prompt: &str) -> Option<String> {
    let normalized = normalize(prompt);
    if let Some(rest) = normalized.strip_prefix("recall ") {
        return first_meaningful_token(rest);
    }
    for marker in ["have we seen ", "seen "] {
        if let Some(rest) = normalized.split_once(marker).map(|(_, rest)| rest) {
            return first_meaningful_token(rest);
        }
    }
    first_meaningful_token(&normalized)
}

fn service_after_about(prompt: &str) -> Option<String> {
    normalize(prompt)
        .split_once("about ")
        .and_then(|(_, rest)| first_meaningful_token(rest))
}

fn service_after_in(prompt: &str) -> Option<String> {
    normalize(prompt)
        .split_once(" in ")
        .and_then(|(_, rest)| first_meaningful_token(rest))
}

fn evidence_matches_topic(item: &EvidenceItem, topic: Option<&str>) -> bool {
    let Some(topic) = topic else {
        return true;
    };
    item.service.as_deref() == Some(topic)
        || item.tags.iter().any(|tag| tag == topic)
        || item.summary.to_ascii_lowercase().contains(topic)
        || item.body.to_ascii_lowercase().contains(topic)
}

fn symptoms_from_prompt(prompt: &str) -> Vec<String> {
    let normalized = normalize(prompt);
    ["latency", "error", "errors", "timeout", "timeouts", "cpu"]
        .iter()
        .filter(|term| normalized.contains(**term))
        .map(|term| (*term).to_string())
        .collect()
}

fn first_meaningful_token(value: &str) -> Option<String> {
    value
        .split_whitespace()
        .map(|token| token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-'))
        .find(|token| {
            !token.is_empty()
                && !matches!(
                    *token,
                    "this" | "before" | "about" | "we" | "have" | "seen" | "should"
                )
        })
        .map(ToString::to_string)
}

fn normalize(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn slug(value: &str) -> String {
    let slug = value
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "memory".to_string()
    } else {
        slug.chars().take(48).collect()
    }
}

fn display_path(path: &Path) -> String {
    if let Ok(relative) = path.strip_prefix(std::env::current_dir().unwrap_or_default()) {
        relative.display().to_string()
    } else {
        path.display().to_string()
    }
}

fn absolute_or_rooted(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

#[cfg(test)]
fn demo_fixture_path(scenario: DemoScenario) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap_or_else(|| Path::new("."))
        .join("examples/demo/scenarios")
        .join(scenario.as_str())
        .join("evidence.json")
}

fn new_demo_temp_dir() -> Result<PathBuf> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| RivoraError::invalid_value("demo_time", error.to_string()))?
        .as_nanos();
    let sequence = DEMO_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "rivora-demo-{}-{nanos}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&path)?;
    Ok(path)
}

fn compact_section(output: &str) -> String {
    output
        .lines()
        .take(12)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn init_array_file(path: &Path) -> Result<()> {
    if !path.exists() {
        fs::write(path, "[]\n")?;
    }
    Ok(())
}

fn read_array<T>(path: &Path) -> Result<Vec<T>>
where
    T: DeserializeOwned,
{
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = fs::read_to_string(path)?;
    if contents.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(&contents).map_err(Into::into)
}

fn read_array_or_empty<T>(path: &Path) -> Result<Vec<T>>
where
    T: DeserializeOwned,
{
    match read_array(path) {
        Ok(values) => Ok(values),
        Err(RivoraError::Serialization(_)) => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

fn write_array<T>(path: &Path, values: &[T]) -> Result<()>
where
    T: Serialize,
{
    let json = serde_json::to_string_pretty(values)?;
    fs::write(path, format!("{json}\n"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rivora_connectors::{
        EvidenceId, EvidenceKind, EvidenceSource, FixtureGitHubClient, GitHubConnector,
    };
    use std::process::Command as ProcessCommand;
    use tempfile::TempDir;

    fn temp_store() -> (TempDir, LocalMemoryStore) {
        let temp = TempDir::new().unwrap();
        let store = LocalMemoryStore::new(temp.path());
        (temp, store)
    }

    fn remember_args() -> Vec<String> {
        [
            "remember",
            "--service",
            "checkout-api",
            "--summary",
            "Checkout latency after inventory change",
            "--symptom",
            "latency",
            "--tag",
            "inventory",
            "--evidence",
            "deploy-2026-06-27",
        ]
        .iter()
        .map(|value| (*value).to_string())
        .collect()
    }

    fn remembered_memory(store: &LocalMemoryStore) -> MemoryRecord {
        remember(
            store,
            parse_remember_options(&remember_args()[1..]).unwrap(),
        )
        .unwrap();
        store.load().unwrap().memories[0].clone()
    }

    fn checkout_evidence() -> EvidenceItem {
        EvidenceItem {
            id: EvidenceId::new("git:commit:checkout123").unwrap(),
            kind: EvidenceKind::GitCommit,
            source: EvidenceSource::local_git("."),
            title: "Git commit checkout123".to_string(),
            summary: "Latency increased after inventory deploy".to_string(),
            body: "Commit checkout123 changed services/checkout/src/main.rs".to_string(),
            service: Some("checkout".to_string()),
            files_changed: vec!["services/checkout/src/main.rs".to_string()],
            timestamp: Some("2026-06-28T00:00:00Z".to_string()),
            author: Some("Ada Lovelace".to_string()),
            tags: vec!["checkout".to_string(), "inventory".to_string()],
            refs: vec!["checkout123".to_string()],
            confidence: 0.9,
        }
    }

    fn demo_fixture() -> PathBuf {
        demo_fixture_path(DemoScenario::Basic)
    }

    fn github_fixture_prs() -> &'static str {
        r#"[
            {
                "number": 128,
                "title": "Reduce checkout worker concurrency",
                "body": "Fixes #120. service:checkout",
                "user": { "login": "ada" },
                "merged_at": "2026-06-27T10:00:00Z",
                "updated_at": "2026-06-27T10:00:00Z",
                "state": "closed",
                "labels": [ { "name": "service:checkout" } ],
                "html_url": "https://github.com/owner/name/pull/128",
                "changed_files": 3
            }
        ]"#
    }

    fn github_fixture_issues() -> &'static str {
        r#"[
            {
                "number": 200,
                "title": "Checkout latency spike",
                "body": "area:checkout saw p99 latency",
                "user": { "login": "ada" },
                "state": "open",
                "updated_at": "2026-06-27T12:00:00Z",
                "labels": [ { "name": "area:checkout" } ],
                "html_url": "https://github.com/owner/name/issues/200"
            }
        ]"#
    }

    fn github_fixture_workflows() -> &'static str {
        r#"{
            "workflow_runs": [
                {
                    "id": 1001,
                    "name": "ci",
                    "head_branch": "main",
                    "head_sha": "abcdef1234567890",
                    "event": "push",
                    "status": "completed",
                    "conclusion": "failure",
                    "html_url": "https://github.com/owner/name/actions/runs/1001",
                    "created_at": "2026-06-27T08:00:00Z",
                    "updated_at": "2026-06-27T08:05:00Z",
                    "actor": { "login": "ada" }
                }
            ]
        }"#
    }

    fn github_fixture_releases() -> &'static str {
        r#"[
            {
                "id": 5001,
                "name": "Checkout v1.4",
                "tag_name": "checkout-v1.4",
                "body": "service:checkout release",
                "html_url": "https://github.com/owner/name/releases/tag/checkout-v1.4",
                "published_at": "2026-06-27T11:00:00Z",
                "author": { "login": "ada" }
            }
        ]"#
    }

    fn github_connector() -> GitHubConnector {
        GitHubConnector::new(
            FixtureGitHubClient::builder()
                .pull_requests(github_fixture_prs())
                .issues(github_fixture_issues())
                .workflow_runs(github_fixture_workflows())
                .releases(github_fixture_releases())
                .build(),
        )
    }

    fn github_ingest_options(repo: &str) -> GitHubIngestOptions {
        GitHubIngestOptions {
            repo: repo.to_string(),
            limit: 10,
            since: None,
            pull_requests: false,
            issues: false,
            workflow_runs: false,
            releases: false,
            deployments: false,
        }
    }

    fn create_git_repo() -> TempDir {
        let repo = TempDir::new().unwrap();
        run_setup_git(repo.path(), &["init"]);
        std::fs::create_dir_all(repo.path().join("services/checkout")).unwrap();
        std::fs::write(
            repo.path().join("services/checkout/main.rs"),
            "fn main() {}\n",
        )
        .unwrap();
        run_setup_git(repo.path(), &["add", "."]);
        run_setup_git(
            repo.path(),
            &[
                "-c",
                "user.name=Rivora Test",
                "-c",
                "user.email=rivora@example.invalid",
                "commit",
                "-m",
                "Update checkout service",
            ],
        );
        repo
    }

    fn run_setup_git(repo: &Path, args: &[&str]) {
        let output = ProcessCommand::new("git")
            .args(args)
            .current_dir(repo)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn init_creates_local_store_files() {
        let (_temp, store) = temp_store();
        let output = init(&store).unwrap();

        assert!(store.memories_path().exists());
        assert!(store.feedback_path().exists());
        assert!(store.receipts_path().exists());
        assert!(store.evidence_path().exists());
        assert!(output.contains("Rivora initialized."));
        assert!(output.contains("Memories: 0"));
    }

    #[test]
    fn init_is_idempotent_and_does_not_wipe_existing_data() {
        let (_temp, store) = temp_store();
        init(&store).unwrap();
        remembered_memory(&store);
        let output = init(&store).unwrap();

        assert!(output.contains("Memories: 1"));
        assert_eq!(store.load().unwrap().memories.len(), 1);
    }

    #[test]
    fn remember_creates_candidate_memory_by_default() {
        let (_temp, store) = temp_store();
        let output = run(remember_args(), &store.root).unwrap();
        let snapshot = store.load().unwrap();

        assert_eq!(snapshot.memories.len(), 1);
        assert_eq!(snapshot.memories[0].status, MemoryStatus::Candidate);
        assert_eq!(snapshot.receipts.len(), 1);
        assert!(output.contains("Status: Candidate"));
    }

    #[test]
    fn remember_approve_records_approval() {
        let (_temp, store) = temp_store();
        let mut args = remember_args();
        args.push("--approve".to_string());
        run(args, &store.root).unwrap();
        let snapshot = store.load().unwrap();

        assert_eq!(snapshot.memories[0].status, MemoryStatus::Active);
        assert_eq!(snapshot.feedback.len(), 1);
        assert!(snapshot.receipts.len() >= 3);
    }

    #[test]
    fn recall_ranks_relevant_memories() {
        let (_temp, store) = temp_store();
        let mut args = remember_args();
        args.push("--approve".to_string());
        run(args, &store.root).unwrap();

        let output = run(
            [
                "recall",
                "checkout-api",
                "--symptom",
                "latency",
                "--tag",
                "inventory",
            ],
            &store.root,
        )
        .unwrap();

        assert!(output.contains("Similar memories found: 1"));
        assert!(output.contains("same service: checkout-api"));
        assert!(output.contains("latency"));
    }

    #[test]
    fn recall_handles_no_matches_safely() {
        let (_temp, store) = temp_store();
        init(&store).unwrap();
        let output = run(["recall", "checkout-api"], &store.root).unwrap();

        assert!(output.contains("No similar memories found."));
        assert!(output.contains("No action was taken."));
    }

    #[test]
    fn feedback_approve_updates_memory_state() {
        let (_temp, store) = temp_store();
        let memory = remembered_memory(&store);

        let output = run(["feedback", memory.id.as_str(), "approve"], &store.root).unwrap();

        let snapshot = store.load().unwrap();
        assert_eq!(snapshot.memories[0].status, MemoryStatus::Active);
        assert_eq!(snapshot.feedback.len(), 1);
        assert!(output.contains("Status: Active"));
    }

    #[test]
    fn feedback_reject_updates_memory_state() {
        let (_temp, store) = temp_store();
        let memory = remembered_memory(&store);
        run(["feedback", memory.id.as_str(), "reject"], &store.root).unwrap();

        assert_eq!(
            store.load().unwrap().memories[0].status,
            MemoryStatus::Rejected
        );
    }

    #[test]
    fn feedback_correct_updates_memory_state_and_stores_note() {
        let (_temp, store) = temp_store();
        let memory = remembered_memory(&store);
        run(
            [
                "feedback",
                memory.id.as_str(),
                "correct",
                "--note",
                "Root cause was connection pool exhaustion",
            ],
            &store.root,
        )
        .unwrap();

        let snapshot = store.load().unwrap();
        assert_eq!(snapshot.memories[0].status, MemoryStatus::Corrected);
        assert_eq!(
            snapshot.feedback[0]
                .correction_text
                .as_ref()
                .map(|value| value.as_str()),
            Some("Root cause was connection pool exhaustion")
        );
    }

    #[test]
    fn ask_routes_have_we_seen_to_recall() {
        let (_temp, store) = temp_store();
        let mut args = remember_args();
        args.push("--approve".to_string());
        run(args, &store.root).unwrap();

        let output = run(
            ["ask", "have we seen checkout-api latency before?"],
            &store.root,
        )
        .unwrap();

        assert!(output.contains("Similar memories found: 1"));
    }

    #[test]
    fn ask_routes_unknown_prompts_to_help() {
        let (_temp, store) = temp_store();
        let output = run(["ask", "please do something magical"], &store.root).unwrap();

        assert!(output.contains("Rivora local-first, evidence-backed reliability memory CLI"));
        assert!(output.contains("rivora ask"));
    }

    #[test]
    fn status_reports_correct_counts() {
        let (_temp, store) = temp_store();
        let memory = remembered_memory(&store);
        run(["feedback", memory.id.as_str(), "approve"], &store.root).unwrap();

        let output = status(&store).unwrap();

        assert!(output.contains("* Total: 1"));
        assert!(output.contains("* Active: 1"));
        assert!(output.contains("Feedback: 1"));
    }

    #[test]
    fn ingest_git_creates_evidence_store() {
        let (_temp, store) = temp_store();
        let repo = create_git_repo();

        let output = run(
            [
                "ingest",
                "git",
                "--repo",
                repo.path().to_str().unwrap(),
                "--limit",
                "5",
            ],
            &store.root,
        )
        .unwrap();

        let snapshot = store.load().unwrap();
        assert!(store.evidence_path().exists());
        assert!(!snapshot.evidence.is_empty());
        assert!(output.contains("Rivora ingested Git evidence."));
        assert!(output.contains("Commits: 1"));
    }

    #[test]
    fn ingest_deduplicates_evidence_by_id() {
        let (_temp, store) = temp_store();
        let repo = create_git_repo();
        let args = [
            "ingest",
            "git",
            "--repo",
            repo.path().to_str().unwrap(),
            "--limit",
            "5",
        ];

        run(args, &store.root).unwrap();
        let first_count = store.load().unwrap().evidence.len();
        let output = run(args, &store.root).unwrap();
        let second_count = store.load().unwrap().evidence.len();

        assert_eq!(first_count, second_count);
        assert!(output.contains("New evidence items: 0"));
    }

    #[test]
    fn evidence_list_renders_stored_evidence() {
        let (_temp, store) = temp_store();
        store.append_evidence([checkout_evidence()]).unwrap();

        let output = run(["evidence", "list"], &store.root).unwrap();

        assert!(output.contains("Local evidence: 1"));
        assert!(output.contains("git:commit:checkout123"));
    }

    #[test]
    fn evidence_show_handles_valid_and_missing_ids() {
        let (_temp, store) = temp_store();
        store.append_evidence([checkout_evidence()]).unwrap();

        let output = run(["evidence", "show", "git:commit:checkout123"], &store.root).unwrap();
        let missing = run(["evidence", "show", "missing"], &store.root);

        assert!(output.contains("Evidence: git:commit:checkout123"));
        assert!(output.contains("rivora remember --from-evidence git:commit:checkout123"));
        assert!(missing.is_err());
    }

    #[test]
    fn remember_from_evidence_creates_candidate_memory() {
        let (_temp, store) = temp_store();
        store.append_evidence([checkout_evidence()]).unwrap();

        let output = run(
            ["remember", "--from-evidence", "git:commit:checkout123"],
            &store.root,
        )
        .unwrap();
        let snapshot = store.load().unwrap();

        assert_eq!(snapshot.memories.len(), 1);
        assert_eq!(snapshot.memories[0].status, MemoryStatus::Candidate);
        assert!(output.contains("Status: Candidate"));
        assert!(output.contains("git:commit:checkout123"));
    }

    #[test]
    fn ask_what_changed_reads_evidence_without_inventing_root_cause() {
        let (_temp, store) = temp_store();
        store.append_evidence([checkout_evidence()]).unwrap();

        let output = run(["ask", "what changed in checkout?"], &store.root).unwrap();

        assert!(output.contains("Rivora found recent Git evidence."));
        assert!(output.contains("git:commit:checkout123"));
        assert!(output.contains("No root cause was inferred."));
        assert!(!output.contains("caused by"));
    }

    #[test]
    fn rivora_store_is_ignored_by_git() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let gitignore = std::fs::read_to_string(root.join(".gitignore")).unwrap();

        assert!(gitignore.lines().any(|line| line.trim() == ".rivora/"));
    }

    #[test]
    fn cli_never_exposes_infrastructure_mutation_actions() {
        let (_temp, store) = temp_store();
        store.append_evidence([checkout_evidence()]).unwrap();
        let outputs = [
            init(&store).unwrap(),
            run(remember_args(), &store.root).unwrap(),
            run(["evidence", "list"], &store.root).unwrap(),
            run(["ask", "what changed in checkout?"], &store.root).unwrap(),
            run(
                ["recall", "checkout-api", "--include-candidates"],
                &store.root,
            )
            .unwrap(),
            run(["ask", "what changed?"], &store.root).unwrap(),
            status(&store).unwrap(),
        ];

        for output in outputs {
            assert!(!output_contains_infrastructure_action(&output), "{output}");
        }
    }

    #[test]
    fn cli_does_not_emit_mutating_action_verbs() {
        let (_temp, store) = temp_store();
        let output = help_text();

        for command in [
            "rivora demo",
            "rivora init",
            "rivora ingest git",
            "rivora ingest github",
            "rivora evidence list",
            "rivora remember",
            "rivora recall",
            "rivora feedback",
            "rivora ask",
            "rivora slack dev",
            "rivora slack doctor",
            "rivora slack socket",
            "rivora status",
        ] {
            assert!(output.contains(command), "missing help command: {command}");
        }
        assert!(output.contains("local-first"));
        assert!(output.contains("evidence-backed"));
        assert!(output.contains("Evidence is not memory until a human approves it."));
        assert!(output.contains("No infrastructure actions are taken."));
        assert!(!output_contains_infrastructure_action(&output));
        assert!(!output.contains("restart action"));
        assert!(!output.contains("scale action"));
        init(&store).unwrap();
    }

    #[test]
    fn launch_docs_reference_real_commands_and_empty_token_placeholders() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let readme = std::fs::read_to_string(root.join("README.md")).unwrap();
        let onboarding =
            std::fs::read_to_string(root.join("docs/DESIGN_PARTNER_ONBOARDING.md")).unwrap();
        let docs_index = std::fs::read_to_string(root.join("docs/README.md")).unwrap();

        for command in [
            "cargo install --path crates/rivora-cli",
            "rivora demo --scenario checkout-incident",
            "rivora ingest git --repo . --limit 20",
            "rivora evidence list",
            "rivora remember --from-evidence <evidence-id>",
            "rivora feedback <memory-id> approve",
            "rivora slack doctor",
            "rivora slack dev --text \"what changed?\"",
            "rivora slack socket",
        ] {
            assert!(
                readme.contains(command),
                "README missing command: {command}"
            );
        }

        for document in [
            "DEMO.md",
            "DESIGN_PARTNER_ONBOARDING.md",
            "SLACK_SELF_HOSTING.md",
            "EVIDENCE_CONNECTORS.md",
            "CLI_MEMORY_INTERFACE.md",
            "MEMORY_MODEL.md",
            "PRINCIPLES.md",
            "../SECURITY.md",
        ] {
            assert!(
                docs_index.contains(document),
                "docs index missing: {document}"
            );
        }

        for assignment in [
            "export GITHUB_TOKEN=",
            "export SLACK_BOT_TOKEN=",
            "export SLACK_APP_TOKEN=",
            "export SLACK_SIGNING_SECRET=",
        ] {
            assert!(onboarding.contains(assignment));
        }
        for nonempty_example in ["=ghp_", "=github_pat_", "=xoxb-", "=xapp-"] {
            assert!(!onboarding.contains(nonempty_example));
        }
    }

    #[test]
    fn parse_github_ingest_requires_repo() {
        let parsed = parse_command(&["ingest".to_string(), "github".to_string()]);
        assert!(parsed.is_err());

        let parsed = parse_command(&[
            "ingest".to_string(),
            "github".to_string(),
            "--repo".to_string(),
            "owner/name".to_string(),
        ])
        .unwrap();
        match parsed {
            Command::Ingest(IngestOptions::GitHub(opts)) => {
                assert_eq!(opts.repo, "owner/name");
                assert_eq!(opts.limit, 20);
            }
            other => panic!("expected github ingest, got {other:?}"),
        }
    }

    #[test]
    fn parse_github_ingest_flags_are_optional_source_selectors() {
        let parsed = parse_command(&[
            "ingest".to_string(),
            "github".to_string(),
            "--repo".to_string(),
            "owner/name".to_string(),
            "--limit".to_string(),
            "5".to_string(),
            "--pull-requests".to_string(),
            "--workflow-runs".to_string(),
        ])
        .unwrap();
        match parsed {
            Command::Ingest(IngestOptions::GitHub(opts)) => {
                assert_eq!(opts.limit, 5);
                assert!(opts.pull_requests);
                assert!(opts.workflow_runs);
                assert!(!opts.issues);
                assert!(!opts.releases);
                assert!(!opts.deployments);
            }
            other => panic!("expected github ingest, got {other:?}"),
        }
    }

    #[test]
    fn parse_slack_dev_and_socket_commands() {
        let dev = parse_command(&[
            "slack".to_string(),
            "dev".to_string(),
            "--text".to_string(),
            "what changed?".to_string(),
        ])
        .unwrap();
        match dev {
            Command::Slack(SlackCommand::Dev(options)) => {
                assert_eq!(options.text, "what changed?");
                assert_eq!(options.channel, "CLOCAL");
                assert_eq!(options.user, "ULOCAL");
            }
            other => panic!("expected slack dev command, got {other:?}"),
        }

        let socket = parse_command(&["slack".to_string(), "socket".to_string()]).unwrap();
        assert!(matches!(
            socket,
            Command::Slack(SlackCommand::Socket(SlackSocketOptions))
        ));
    }

    #[test]
    fn parse_demo_and_fixture_ingest_commands() {
        let demo = parse_command(&[
            "demo".to_string(),
            "--scenario".to_string(),
            "checkout-incident".to_string(),
            "--keep".to_string(),
            "--json".to_string(),
            "--store".to_string(),
            "tmp/demo-store".to_string(),
        ])
        .unwrap();
        match demo {
            Command::Demo(options) => {
                assert_eq!(options.scenario, DemoScenario::CheckoutIncident);
                assert!(options.keep);
                assert!(options.json);
                assert_eq!(options.store, Some(PathBuf::from("tmp/demo-store")));
            }
            other => panic!("expected demo command, got {other:?}"),
        }

        let fixture = parse_command(&[
            "ingest".to_string(),
            "fixture".to_string(),
            "--path".to_string(),
            "examples/demo/evidence.json".to_string(),
        ])
        .unwrap();
        match fixture {
            Command::Ingest(IngestOptions::Fixture(options)) => {
                assert_eq!(options.path, PathBuf::from("examples/demo/evidence.json"));
            }
            other => panic!("expected fixture ingest, got {other:?}"),
        }
    }

    #[test]
    fn fixture_evidence_loads_and_deduplicates() {
        let (_temp, store) = temp_store();
        let path = demo_fixture();

        let first = run(
            [
                "ingest",
                "fixture",
                "--path",
                path.to_str().expect("fixture path should be utf-8"),
            ],
            &store.root,
        )
        .unwrap();
        let second = run(
            [
                "ingest",
                "fixture",
                "--path",
                path.to_str().expect("fixture path should be utf-8"),
            ],
            &store.root,
        )
        .unwrap();
        let snapshot = store.load().unwrap();

        assert_eq!(snapshot.evidence.len(), 3);
        assert!(first.contains("Rivora ingested fixture evidence."));
        assert!(first.contains("New evidence items: 3"));
        assert!(second.contains("New evidence items: 0"));
    }

    #[test]
    fn fixture_ask_what_changed_suggests_newest_relevant_evidence() {
        let (_temp, store) = temp_store();
        ingest_fixture(
            &store,
            FixtureIngestOptions {
                path: demo_fixture(),
            },
        )
        .unwrap();

        let output = run(["ask", "what changed?"], &store.root).unwrap();

        assert!(output.contains("PR #128: Reduce checkout worker concurrency"));
        assert!(output.contains("rivora remember --from-evidence github:pr:demo/checkout:128"));
    }

    #[test]
    fn demo_runs_without_tokens_and_cleans_default_store() {
        let (_temp, store) = temp_store();
        let output = run(["demo"], &store.root).unwrap();

        assert!(output.contains("Rivora Demo"));
        assert!(output.contains("Scenario: basic"));
        assert!(output.contains("This demo uses packaged fixture data."));
        assert!(output.contains("No tokens are required."));
        assert!(output.contains("Evidence -> Memory Candidate -> Human Approval -> Recall"));
        assert!(output.contains("No infrastructure actions were taken."));
        assert!(!store.store_dir().exists());
        assert!(!output.contains("xoxb-"));
        assert!(!output.contains("xapp-"));
        assert!(!output.contains("ghp_"));
        assert!(!output_contains_infrastructure_action(&output), "{output}");
    }

    #[test]
    fn demo_scenarios_load_evidence_approve_memory_and_recall() {
        let temp = TempDir::new().unwrap();
        let cases = [
            ("basic", 3, "github:pr:demo/checkout:128"),
            (
                "checkout-incident",
                4,
                "github:pr:demo/checkout-incident:128",
            ),
            (
                "release-regression",
                4,
                "github:pr:demo/release-regression:141",
            ),
            (
                "workflow-failure",
                3,
                "github:workflow:demo/workflow-failure:1152",
            ),
        ];

        for (scenario, evidence_count, selected_evidence_id) in cases {
            let store_name = format!("demo-{scenario}");
            let output = run(
                [
                    "demo",
                    "--scenario",
                    scenario,
                    "--store",
                    store_name.as_str(),
                ],
                temp.path(),
            )
            .unwrap();
            let snapshot = LocalMemoryStore::new(temp.path().join(store_name))
                .load()
                .unwrap();

            assert_eq!(snapshot.evidence.len(), evidence_count, "{scenario}");
            assert_eq!(snapshot.memories.len(), 1, "{scenario}");
            assert_eq!(
                snapshot.memories[0].status,
                MemoryStatus::Active,
                "{scenario}"
            );
            assert_eq!(snapshot.feedback.len(), 1, "{scenario}");
            assert!(snapshot.receipts.len() >= 4, "{scenario}");
            assert!(
                output.contains(&format!("Scenario: {scenario}")),
                "{output}"
            );
            assert!(output.contains(selected_evidence_id), "{output}");
            assert!(output.contains("Similar memories found: 1"), "{output}");
            assert!(output.contains("Rivora Slack dev response"), "{output}");
            assert!(output.contains("No tokens are required."), "{output}");
            assert!(output.contains("No network is required."), "{output}");
            assert!(output.contains("Evidence is not memory until a human approves it."));
            assert!(output.contains("No infrastructure actions were taken."));
            assert!(!output_contains_infrastructure_action(&output), "{output}");
        }
    }

    #[test]
    fn every_demo_scenario_fixture_is_valid_and_nonempty() {
        for scenario in DemoScenario::ALL {
            let raw = fs::read_to_string(demo_fixture_path(scenario)).unwrap();
            let evidence: Vec<EvidenceItem> = serde_json::from_str(&raw).unwrap();
            assert!(!evidence.is_empty(), "{}", scenario.as_str());
            assert!(evidence.iter().all(|item| item.source.read_only));
            for token_prefix in ["xoxb-", "xapp-", "ghp_", "github_pat_"] {
                assert!(!raw.contains(token_prefix), "{}", scenario.as_str());
            }
        }
    }

    #[test]
    fn packaged_demo_fixtures_parse_and_match_documented_examples() {
        for scenario in DemoScenario::ALL {
            let packaged = demo_fixtures::packaged_demo_fixture(scenario);
            let evidence: Vec<EvidenceItem> = serde_json::from_str(packaged).unwrap();
            let example = fs::read_to_string(demo_fixture_path(scenario)).unwrap();

            assert!(!evidence.is_empty(), "{}", scenario.as_str());
            assert_eq!(packaged, example, "{}", scenario.as_str());
            assert!(evidence.iter().all(|item| item.source.read_only));
        }
    }

    #[test]
    fn packaged_demo_runs_from_outside_source_checkout_for_every_scenario() {
        let temp = TempDir::new().unwrap();
        assert!(!temp.path().join("examples/demo").exists());

        for scenario in DemoScenario::ALL {
            let output = run(["demo", "--scenario", scenario.as_str()], temp.path())
                .unwrap_or_else(|error| panic!("{}: {error:?}", scenario.as_str()));

            assert!(output.contains(&format!("Scenario: {}", scenario.as_str())));
            assert!(output.contains("This demo uses packaged fixture data."));
            assert!(output.contains("No tokens are required."));
            assert!(output.contains("No network is required."));
            assert!(output.contains("No infrastructure actions were taken."));
            assert!(!temp.path().join(".rivora").exists());
        }
    }

    #[test]
    fn unknown_demo_scenario_lists_supported_values() {
        let error = parse_command(&[
            "demo".to_string(),
            "--scenario".to_string(),
            "unknown".to_string(),
        ])
        .unwrap_err()
        .to_string();

        assert!(error.contains("unknown demo scenario 'unknown'"), "{error}");
        for scenario in DemoScenario::ALL {
            assert!(error.contains(scenario.as_str()), "{error}");
        }
    }

    #[test]
    fn demo_with_explicit_store_creates_candidate_approval_recall_and_slack_response() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        let output = run(["demo", "--store", "demo-root"], root).unwrap();
        let demo_store = LocalMemoryStore::new(root.join("demo-root"));
        let snapshot = demo_store.load().unwrap();

        assert_eq!(snapshot.evidence.len(), 3);
        assert_eq!(snapshot.memories.len(), 1);
        assert_eq!(snapshot.memories[0].status, MemoryStatus::Active);
        assert_eq!(snapshot.feedback.len(), 1);
        assert!(snapshot.receipts.len() >= 4);
        assert!(output.contains("Memory candidate created from GitHub evidence."));
        assert!(output.contains("Status: Active"));
        assert!(output.contains("Similar memories found: 1"));
        assert!(output.contains("Rivora Slack dev response"));
        assert!(output.contains("Demo store kept at:"));
    }

    #[test]
    fn demo_json_reports_core_loop_summary() {
        let (_temp, store) = temp_store();
        let output = run(
            ["demo", "--scenario", "checkout-incident", "--json"],
            &store.root,
        )
        .unwrap();
        let json: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(json["demo"], "complete");
        assert_eq!(json["scenario"], "checkout-incident");
        assert_eq!(json["evidence_count"], 4);
        assert_eq!(
            json["selected_evidence_id"],
            "github:pr:demo/checkout-incident:128"
        );
        assert!(json["memory_id"].as_str().is_some_and(|id| !id.is_empty()));
        assert_eq!(json["final_memory_status"], "active");
        assert_eq!(json["recall_match_count"], 1);
        assert_eq!(json["slack_dev_rendered"], true);
        assert_eq!(
            json["message"],
            "Evidence -> Memory Candidate -> Human Approval -> Recall"
        );
        assert_eq!(
            json["safety_summary"],
            "No infrastructure actions were taken."
        );
        assert_eq!(
            json["human_control_summary"],
            "Evidence is not memory until approved."
        );
        assert!(json.get("store").is_none());
        assert!(!store.store_dir().exists());
    }

    #[test]
    fn packaged_demo_json_is_compatible_for_every_scenario() {
        let temp = TempDir::new().unwrap();

        for scenario in DemoScenario::ALL {
            let output = run(
                ["demo", "--scenario", scenario.as_str(), "--json"],
                temp.path(),
            )
            .unwrap();
            let json: serde_json::Value = serde_json::from_str(&output).unwrap();

            assert_eq!(json["demo"], "complete");
            assert_eq!(json["scenario"], scenario.as_str());
            assert!(json["evidence_count"]
                .as_u64()
                .is_some_and(|count| count > 0));
            assert!(json["selected_evidence_id"]
                .as_str()
                .is_some_and(|id| !id.is_empty()));
            assert_eq!(json["final_memory_status"], "active");
            assert_eq!(json["recall_match_count"], 1);
            assert_eq!(json["slack_dev_rendered"], true);
            assert_eq!(
                json["safety_summary"],
                "No infrastructure actions were taken."
            );
            assert!(json.get("store").is_none());
            assert!(!temp.path().join(".rivora").exists());
        }
    }

    #[test]
    fn demo_script_exists_and_docs_reference_it() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let script = root.join("scripts/demo-local-memory-loop.sh");
        let script_body = std::fs::read_to_string(&script).unwrap_or_default();
        let docs = std::fs::read_to_string(root.join("docs/DEMO.md")).unwrap_or_default();

        assert!(script.exists());
        assert!(script_body.contains("RIVORA_DEMO_SCENARIO"));
        assert!(script_body.contains("SCENARIO"));
        assert!(script_body.contains("demo --scenario"));
        assert!(!script_body.contains("examples/demo/scenarios"));
        assert!(docs.contains("scripts/demo-local-memory-loop.sh"));
        assert!(docs.contains("scripts/demo-local-memory-loop.sh checkout-incident"));
    }

    #[test]
    fn ingest_github_stores_evidence_using_fixture_client() {
        let (_temp, store) = temp_store();
        let connector = github_connector();
        let output =
            ingest_github_with_connector(&store, github_ingest_options("owner/name"), &connector)
                .unwrap();

        let snapshot = store.load().unwrap();
        assert!(store.evidence_path().exists());
        assert!(!snapshot.evidence.is_empty());
        assert!(output.contains("Rivora ingested GitHub evidence."));
        assert!(output.contains("Repository: owner/name"));
        assert!(output.contains("Pull requests: 1"));
        assert!(output.contains("Workflow runs: 1"));
        assert!(output.contains("No infrastructure actions were taken."));
        assert!(snapshot
            .evidence
            .iter()
            .any(|item| item.id.as_str() == "github:pr:owner/name:128"));
    }

    #[test]
    fn ingest_github_deduplicates_evidence_across_ingests() {
        let (_temp, store) = temp_store();
        let connector = github_connector();
        let options = github_ingest_options("owner/name");

        ingest_github_with_connector(&store, options.clone(), &connector).unwrap();
        let first_count = store.load().unwrap().evidence.len();
        let output = ingest_github_with_connector(&store, options, &connector).unwrap();
        let second_count = store.load().unwrap().evidence.len();

        assert_eq!(first_count, second_count);
        assert!(output.contains("New evidence items: 0"));
    }

    #[test]
    fn remember_from_github_evidence_creates_candidate_mentioning_github() {
        let (_temp, store) = temp_store();
        let connector = github_connector();
        ingest_github_with_connector(&store, github_ingest_options("owner/name"), &connector)
            .unwrap();

        let output = run(
            ["remember", "--from-evidence", "github:pr:owner/name:128"],
            &store.root,
        )
        .unwrap();
        let snapshot = store.load().unwrap();

        assert_eq!(snapshot.memories.len(), 1);
        assert_eq!(snapshot.memories[0].status, MemoryStatus::Candidate);
        assert!(output.contains("Memory candidate created from GitHub evidence."));
        assert!(output.contains("Source: GitHub PR merged"));
        assert!(output.contains("Status: Candidate"));
        assert!(output.contains("github:pr:owner/name:128"));
        assert!(output.contains("No action was taken."));
    }

    #[test]
    fn ask_what_merged_recently_uses_github_pr_evidence() {
        let (_temp, store) = temp_store();
        let connector = github_connector();
        ingest_github_with_connector(&store, github_ingest_options("owner/name"), &connector)
            .unwrap();

        let output = run(["ask", "what merged recently?"], &store.root).unwrap();

        assert!(output.contains("Rivora found recent GitHub merge evidence."));
        assert!(output.contains("PR #128 merged"));
        assert!(output.contains("github:pr:owner/name:128"));
        assert!(output.contains("rivora remember --from-evidence github:pr:owner/name:128"));
        assert!(output.contains("No infrastructure actions were taken."));
    }

    #[test]
    fn ask_what_failed_recently_uses_workflow_failure_evidence() {
        let (_temp, store) = temp_store();
        let connector = github_connector();
        ingest_github_with_connector(&store, github_ingest_options("owner/name"), &connector)
            .unwrap();

        let output = run(["ask", "what failed recently?"], &store.root).unwrap();

        assert!(output.contains("Rivora found recent GitHub workflow failures."));
        assert!(output.contains("github:workflow:owner/name:1001"));
        assert!(output.contains("failure"));
        assert!(output.contains("No root cause was inferred."));
    }

    #[test]
    fn ask_what_changed_in_github_surfaces_github_evidence() {
        let (_temp, store) = temp_store();
        let connector = github_connector();
        ingest_github_with_connector(&store, github_ingest_options("owner/name"), &connector)
            .unwrap();

        let output = run(["ask", "what changed in github?"], &store.root).unwrap();

        assert!(output.contains("Rivora found recent GitHub evidence."));
        assert!(output.contains("github:pr:owner/name:128"));
        assert!(!output.contains("Rivora found recent Git evidence."));
    }

    #[test]
    fn ask_what_changed_in_checkout_surfaces_github_pr_evidence() {
        let (_temp, store) = temp_store();
        let connector = github_connector();
        ingest_github_with_connector(&store, github_ingest_options("owner/name"), &connector)
            .unwrap();

        let output = run(["ask", "what changed in checkout?"], &store.root).unwrap();

        assert!(output.contains("Rivora found recent evidence."));
        assert!(output.contains("Reduce checkout worker concurrency"));
    }

    #[test]
    fn ask_what_merged_recently_without_evidence_suggests_ingest() {
        let (_temp, store) = temp_store();
        let output = run(["ask", "what merged recently?"], &store.root).unwrap();
        assert!(output.contains("did not find GitHub evidence yet."));
        assert!(output.contains("rivora ingest github --repo owner/name"));
    }

    #[test]
    fn github_evidence_store_never_contains_a_token() {
        let (_temp, store) = temp_store();
        std::env::set_var("GITHUB_TOKEN", "ghp_cli_secret_token");
        let connector = github_connector();
        let output =
            ingest_github_with_connector(&store, github_ingest_options("owner/name"), &connector)
                .unwrap();
        std::env::remove_var("GITHUB_TOKEN");

        assert!(!output.contains("ghp_cli_secret_token"));
        let raw = std::fs::read_to_string(store.evidence_path()).unwrap();
        assert!(!raw.contains("ghp_cli_secret_token"));
        let snapshot = store.load().unwrap();
        for item in snapshot.evidence {
            assert!(!item.body.contains("ghp_"));
            assert!(!item.summary.contains("ghp_"));
            assert!(!item.refs.iter().any(|r| r.contains("ghp_")));
        }
    }

    #[test]
    fn cli_github_outputs_never_expose_infrastructure_mutation_actions() {
        let (_temp, store) = temp_store();
        let connector = github_connector();
        ingest_github_with_connector(&store, github_ingest_options("owner/name"), &connector)
            .unwrap();

        let outputs = [
            ingest_github_with_connector(&store, github_ingest_options("owner/name"), &connector)
                .unwrap(),
            run(["evidence", "list"], &store.root).unwrap(),
            run(
                ["evidence", "show", "github:pr:owner/name:128"],
                &store.root,
            )
            .unwrap(),
            run(["ask", "what merged recently?"], &store.root).unwrap(),
            run(["ask", "what failed recently?"], &store.root).unwrap(),
            run(["ask", "what changed in github?"], &store.root).unwrap(),
            run(
                ["remember", "--from-evidence", "github:pr:owner/name:128"],
                &store.root,
            )
            .unwrap(),
        ];

        for output in outputs {
            assert!(!output_contains_infrastructure_action(&output), "{output}");
            assert!(!output.contains("ghp_"), "token leaked: {output}");
        }
    }

    #[test]
    fn parse_slack_doctor_command() {
        let parsed = parse_command(&["slack".to_string(), "doctor".to_string()]).unwrap();
        assert!(matches!(
            parsed,
            Command::Slack(SlackCommand::Doctor(SlackDoctorOptions { live: false }))
        ));

        let parsed = parse_command(&[
            "slack".to_string(),
            "doctor".to_string(),
            "--live".to_string(),
        ])
        .unwrap();
        assert!(matches!(
            parsed,
            Command::Slack(SlackCommand::Doctor(SlackDoctorOptions { live: true }))
        ));
    }
}
