//! Read-only GitHub evidence connector.
//!
//! Feeds GitHub pull requests, issues, workflow runs, releases, and
//! deployments into Rivora's evidence store. The connector is strictly
//! read-only: it only issues `GET` requests against the GitHub REST API and
//! never calls mutation endpoints (`POST`, `PUT`, `PATCH`, `DELETE`).
//!
//! Authentication is optional. A `GITHUB_TOKEN` environment variable is
//! recommended for private repositories and higher rate limits, but public
//! repositories can be ingested anonymously. Tokens are never stored in
//! `.rivora/`, never printed, and never written into evidence bodies, logs, or
//! receipts. Error messages are redacted to ensure tokens cannot leak through
//! `curl` stderr.

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use rivora_errors::{Result, RivoraError};
use serde::Deserialize;

use crate::{slug, EvidenceId, EvidenceItem, EvidenceKind, EvidenceSource};

/// Connector name written into [`EvidenceSource::connector`].
pub const GITHUB_CONNECTOR: &str = "github";
/// Default GitHub REST API base URL.
pub const GITHUB_API_BASE: &str = "https://api.github.com";

/// HTTP methods the GitHub connector is allowed to use. The connector only
/// ever issues `GET` requests.
#[must_use]
pub fn github_allowed_http_methods() -> &'static [&'static str] {
    &["GET"]
}

/// HTTP methods the GitHub connector must never use.
#[must_use]
pub fn github_forbidden_http_methods() -> &'static [&'static str] {
    &["POST", "PUT", "PATCH", "DELETE"]
}

/// Replace any occurrence of `token` in `value` with `[redacted]`.
///
/// Used to scrub `curl` stderr and any other string before it can appear in an
/// error message. Returns `value` unchanged when `token` is empty.
#[must_use]
pub fn redact_token(value: &str, token: &str) -> String {
    if token.is_empty() {
        value.to_string()
    } else {
        value.replace(token, "[redacted]")
    }
}

/// GitHub authentication configuration.
///
/// The token is held privately and never exposed through a getter. Use
/// [`Self::has_token`] to check whether a token is configured and
/// [`Self::redact`] to scrub strings before they can appear in errors or logs.
#[derive(Debug, Clone)]
pub struct GitHubAuthConfig {
    token: Option<String>,
}

impl GitHubAuthConfig {
    /// Read the `GITHUB_TOKEN` environment variable if present and non-empty.
    #[must_use]
    pub fn from_env() -> Self {
        let token = std::env::var("GITHUB_TOKEN")
            .ok()
            .filter(|token| !token.trim().is_empty());
        Self { token }
    }

    /// Anonymous access for public repositories.
    #[must_use]
    pub fn anonymous() -> Self {
        Self { token: None }
    }

    /// Explicit token. Primarily useful for tests; production code should use
    /// [`Self::from_env`].
    #[must_use]
    pub fn with_token(token: impl Into<String>) -> Self {
        let token = token.into();
        Self {
            token: if token.trim().is_empty() {
                None
            } else {
                Some(token)
            },
        }
    }

    #[must_use]
    pub fn has_token(&self) -> bool {
        self.token.is_some()
    }

    /// Scrub any occurrence of the configured token from `value`.
    #[must_use]
    pub fn redact(&self, value: &str) -> String {
        match &self.token {
            Some(token) => redact_token(value, token),
            None => value.to_string(),
        }
    }
}

/// A GitHub repository reference of the form `owner/name`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GitHubRepositoryRef {
    pub owner: String,
    pub name: String,
}

impl GitHubRepositoryRef {
    /// Parse `owner/name` into a [`GitHubRepositoryRef`].
    pub fn parse(value: &str) -> Result<Self> {
        let trimmed = value.trim();
        let (owner, name) = trimmed
            .split_once('/')
            .and_then(|(owner, name)| {
                let owner = owner.trim();
                let name = name.trim();
                (!owner.is_empty() && !name.is_empty()).then_some((owner, name))
            })
            .ok_or_else(|| {
                RivoraError::invalid_value(
                    "github_repo",
                    "expected repository reference in the form owner/name",
                )
            })?;
        Ok(Self {
            owner: owner.to_string(),
            name: name.to_string(),
        })
    }

    #[must_use]
    pub fn new(owner: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            owner: owner.into(),
            name: name.into(),
        }
    }

    #[must_use]
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

/// Read-only GitHub API client contract.
///
/// Every method issues a `GET` request and returns the raw JSON body. The
/// trait intentionally exposes no mutation operations.
pub trait GitHubClient {
    fn fetch_pull_requests(&self, repo: &GitHubRepositoryRef, limit: usize) -> Result<String>;
    fn fetch_issues(&self, repo: &GitHubRepositoryRef, limit: usize) -> Result<String>;
    fn fetch_workflow_runs(&self, repo: &GitHubRepositoryRef, limit: usize) -> Result<String>;
    fn fetch_releases(&self, repo: &GitHubRepositoryRef, limit: usize) -> Result<String>;
    fn fetch_deployments(&self, repo: &GitHubRepositoryRef, limit: usize) -> Result<String>;
}

/// Real GitHub REST API client backed by `curl`.
///
/// The token is passed to `curl` through stdin (`--config -`) so it never
/// appears in the process argument list and is not visible via `ps`. The
/// client only constructs `GET` requests.
#[derive(Debug, Clone)]
pub struct HttpGitHubClient {
    auth: GitHubAuthConfig,
    base_url: String,
}

impl HttpGitHubClient {
    #[must_use]
    pub fn new(auth: GitHubAuthConfig) -> Self {
        Self {
            auth,
            base_url: GITHUB_API_BASE.to_string(),
        }
    }

    /// Build the `curl` `--config -` body for a `GET` request against `path`.
    /// The configured token (if any) is included here so it can be piped to
    /// `curl` over stdin instead of as a process argument.
    pub(crate) fn request_config(&self, path: &str) -> String {
        let url = format!("{}{}", self.base_url, path);
        let mut config = String::new();
        config.push_str(&format!("url = \"{url}\"\n"));
        config.push_str("silent\n");
        config.push_str("show-error\n");
        config.push_str("fail\n");
        config.push_str("request = \"GET\"\n");
        config.push_str("header = \"Accept: application/vnd.github+json\"\n");
        config.push_str("header = \"X-GitHub-Api-Version: 2022-11-28\"\n");
        config.push_str("header = \"User-Agent: rivora-connectors\"\n");
        if let Some(token) = self.auth.token.as_ref() {
            config.push_str(&format!("header = \"Authorization: Bearer {token}\"\n"));
        }
        config
    }

    fn get(&self, path: &str) -> Result<String> {
        let config = self.request_config(path);
        let mut child = Command::new("curl")
            .arg("--config")
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| RivoraError::provider("github", format!("curl unavailable: {e}")))?;
        {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| RivoraError::provider("github", "could not open curl stdin pipe"))?;
            stdin.write_all(config.as_bytes()).map_err(|e| {
                RivoraError::provider("github", format!("curl config write failed: {e}"))
            })?;
        }
        let output = child
            .wait_with_output()
            .map_err(|e| RivoraError::provider("github", format!("curl did not finish: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let redacted = self.auth.redact(stderr.as_ref());
            return Err(RivoraError::provider(
                "github",
                format!(
                    "GitHub API request failed for {}: {}",
                    path,
                    redacted.trim()
                ),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

impl GitHubClient for HttpGitHubClient {
    fn fetch_pull_requests(&self, repo: &GitHubRepositoryRef, limit: usize) -> Result<String> {
        self.get(&format!(
            "/repos/{}/pulls?state=all&sort=updated&direction=desc&per_page={}",
            repo.full_name(),
            clamp_per_page(limit)
        ))
    }

    fn fetch_issues(&self, repo: &GitHubRepositoryRef, limit: usize) -> Result<String> {
        self.get(&format!(
            "/repos/{}/issues?state=all&sort=updated&direction=desc&per_page={}",
            repo.full_name(),
            clamp_per_page(limit)
        ))
    }

    fn fetch_workflow_runs(&self, repo: &GitHubRepositoryRef, limit: usize) -> Result<String> {
        self.get(&format!(
            "/repos/{}/actions/runs?per_page={}",
            repo.full_name(),
            clamp_per_page(limit)
        ))
    }

    fn fetch_releases(&self, repo: &GitHubRepositoryRef, limit: usize) -> Result<String> {
        self.get(&format!(
            "/repos/{}/releases?per_page={}",
            repo.full_name(),
            clamp_per_page(limit)
        ))
    }

    fn fetch_deployments(&self, repo: &GitHubRepositoryRef, limit: usize) -> Result<String> {
        self.get(&format!(
            "/repos/{}/deployments?per_page={}",
            repo.full_name(),
            clamp_per_page(limit)
        ))
    }
}

/// Test double for [`GitHubClient`] that returns preloaded fixture JSON without
/// any network access.
#[derive(Debug, Clone, Default)]
pub struct FixtureGitHubClient {
    pull_requests: Option<String>,
    issues: Option<String>,
    workflow_runs: Option<String>,
    releases: Option<String>,
    deployments: Option<String>,
}

impl FixtureGitHubClient {
    #[must_use]
    pub fn builder() -> FixtureGitHubClientBuilder {
        FixtureGitHubClientBuilder::default()
    }
}

impl GitHubClient for FixtureGitHubClient {
    fn fetch_pull_requests(&self, _repo: &GitHubRepositoryRef, _limit: usize) -> Result<String> {
        fixture_response(&self.pull_requests, "pull_requests")
    }

    fn fetch_issues(&self, _repo: &GitHubRepositoryRef, _limit: usize) -> Result<String> {
        fixture_response(&self.issues, "issues")
    }

    fn fetch_workflow_runs(&self, _repo: &GitHubRepositoryRef, _limit: usize) -> Result<String> {
        fixture_response(&self.workflow_runs, "workflow_runs")
    }

    fn fetch_releases(&self, _repo: &GitHubRepositoryRef, _limit: usize) -> Result<String> {
        fixture_response(&self.releases, "releases")
    }

    fn fetch_deployments(&self, _repo: &GitHubRepositoryRef, _limit: usize) -> Result<String> {
        fixture_response(&self.deployments, "deployments")
    }
}

#[derive(Debug, Default, Clone)]
pub struct FixtureGitHubClientBuilder {
    pull_requests: Option<String>,
    issues: Option<String>,
    workflow_runs: Option<String>,
    releases: Option<String>,
    deployments: Option<String>,
}

impl FixtureGitHubClientBuilder {
    #[must_use]
    pub fn pull_requests(mut self, fixture: impl Into<String>) -> Self {
        self.pull_requests = Some(fixture.into());
        self
    }

    #[must_use]
    pub fn issues(mut self, fixture: impl Into<String>) -> Self {
        self.issues = Some(fixture.into());
        self
    }

    #[must_use]
    pub fn workflow_runs(mut self, fixture: impl Into<String>) -> Self {
        self.workflow_runs = Some(fixture.into());
        self
    }

    #[must_use]
    pub fn releases(mut self, fixture: impl Into<String>) -> Self {
        self.releases = Some(fixture.into());
        self
    }

    #[must_use]
    pub fn deployments(mut self, fixture: impl Into<String>) -> Self {
        self.deployments = Some(fixture.into());
        self
    }

    #[must_use]
    pub fn build(self) -> FixtureGitHubClient {
        FixtureGitHubClient {
            pull_requests: self.pull_requests,
            issues: self.issues,
            workflow_runs: self.workflow_runs,
            releases: self.releases,
            deployments: self.deployments,
        }
    }
}

fn fixture_response(value: &Option<String>, name: &str) -> Result<String> {
    value
        .clone()
        .ok_or_else(|| RivoraError::provider("github", format!("no fixture loaded for {name}")))
}

fn clamp_per_page(limit: usize) -> usize {
    limit.clamp(1, 100)
}

/// Request for GitHub evidence ingestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubIngestRequest {
    pub repo: GitHubRepositoryRef,
    pub limit: usize,
    pub since: Option<String>,
    pub pull_requests: bool,
    pub issues: bool,
    pub workflow_runs: bool,
    pub releases: bool,
    pub deployments: bool,
}

impl GitHubIngestRequest {
    #[must_use]
    pub fn new(repo: GitHubRepositoryRef) -> Self {
        Self {
            repo,
            limit: 20,
            since: None,
            pull_requests: false,
            issues: false,
            workflow_runs: false,
            releases: false,
            deployments: false,
        }
    }

    #[must_use]
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    #[must_use]
    pub fn with_since(mut self, since: impl Into<String>) -> Self {
        self.since = Some(since.into());
        self
    }

    #[must_use]
    pub fn with_pull_requests(mut self) -> Self {
        self.pull_requests = true;
        self
    }

    #[must_use]
    pub fn with_issues(mut self) -> Self {
        self.issues = true;
        self
    }

    #[must_use]
    pub fn with_workflow_runs(mut self) -> Self {
        self.workflow_runs = true;
        self
    }

    #[must_use]
    pub fn with_releases(mut self) -> Self {
        self.releases = true;
        self
    }

    #[must_use]
    pub fn with_deployments(mut self) -> Self {
        self.deployments = true;
        self
    }

    /// True when no source flags were set. The connector uses this to apply
    /// the default source set.
    #[must_use]
    pub fn no_sources_selected(&self) -> bool {
        !self.pull_requests
            && !self.issues
            && !self.workflow_runs
            && !self.releases
            && !self.deployments
    }
}

/// Result of GitHub evidence ingestion.
#[derive(Debug, Clone, PartialEq)]
pub struct GitHubIngestResult {
    pub repository: String,
    pub evidence: Vec<EvidenceItem>,
    pub pull_requests: usize,
    pub issues: usize,
    pub workflow_runs: usize,
    pub releases: usize,
    pub deployments: usize,
    pub topics: Vec<String>,
}

/// Read-only GitHub connector. Holds a boxed [`GitHubClient`] so the CLI can
/// swap in a [`FixtureGitHubClient`] for tests without generics leaking into
/// calling code.
pub struct GitHubConnector {
    client: Box<dyn GitHubClient>,
}

impl std::fmt::Debug for GitHubConnector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitHubConnector").finish_non_exhaustive()
    }
}

impl GitHubConnector {
    #[must_use]
    pub fn new(client: impl GitHubClient + 'static) -> Self {
        Self {
            client: Box::new(client),
        }
    }

    pub fn ingest(&self, request: GitHubIngestRequest) -> Result<GitHubIngestResult> {
        if request.limit == 0 {
            return Err(RivoraError::invalid_value(
                "limit",
                "limit must be positive",
            ));
        }

        let repo = request.repo.clone();
        let repository = repo.full_name();
        let source = EvidenceSource::github(repository.clone());
        let limit = request.limit;

        let (want_prs, want_issues, want_workflows, want_releases, want_deployments) =
            if request.no_sources_selected() {
                (true, true, true, true, false)
            } else {
                (
                    request.pull_requests,
                    request.issues,
                    request.workflow_runs,
                    request.releases,
                    request.deployments,
                )
            };

        let mut evidence = Vec::new();
        let since_cutoff = request
            .since
            .as_deref()
            .map(parse_since_cutoff)
            .transpose()?;
        let mut topics = std::collections::BTreeSet::new();
        let mut pull_requests = 0;
        let mut issues = 0;
        let mut workflow_runs = 0;
        let mut releases = 0;
        let mut deployments = 0;

        if want_prs {
            let raw = self.client.fetch_pull_requests(&repo, limit)?;
            let prs = parse_pull_requests(&raw);
            pull_requests = prs.len();
            for pr in prs {
                let item = pull_request_item(&source, &repo, &pr)?;
                collect_topics(&item, &mut topics);
                evidence.push(item);
            }
        }

        if want_issues {
            let raw = self.client.fetch_issues(&repo, limit)?;
            let parsed = parse_issues(&raw);
            issues = parsed.len();
            for issue in parsed {
                let item = issue_item(&source, &repo, &issue)?;
                collect_topics(&item, &mut topics);
                evidence.push(item);
            }
        }

        if want_workflows {
            let raw = self.client.fetch_workflow_runs(&repo, limit)?;
            let runs = parse_workflow_runs(&raw);
            workflow_runs = runs.len();
            for run in runs {
                let item = workflow_run_item(&source, &repo, &run)?;
                collect_topics(&item, &mut topics);
                evidence.push(item);
            }
        }

        if want_releases {
            let raw = self.client.fetch_releases(&repo, limit)?;
            let parsed = parse_releases(&raw);
            releases = parsed.len();
            for release in parsed {
                let item = release_item(&source, &repo, &release)?;
                collect_topics(&item, &mut topics);
                evidence.push(item);
            }
        }

        if want_deployments {
            let raw = self.client.fetch_deployments(&repo, limit)?;
            let parsed = parse_deployments(&raw);
            deployments = parsed.len();
            for deployment in parsed {
                let item = deployment_item(&source, &repo, &deployment)?;
                collect_topics(&item, &mut topics);
                evidence.push(item);
            }
        }

        evidence.sort_by(|a, b| a.id.cmp(&b.id));
        evidence.dedup_by(|a, b| a.id == b.id);
        if let Some(cutoff) = since_cutoff {
            evidence.retain(|item| evidence_is_after_cutoff(item, cutoff));
            topics.clear();
            for item in &evidence {
                collect_topics(item, &mut topics);
            }
            pull_requests = evidence
                .iter()
                .filter(|item| {
                    matches!(
                        item.kind,
                        EvidenceKind::GitHubPullRequest | EvidenceKind::GitHubPullRequestMerged
                    )
                })
                .count();
            issues = evidence
                .iter()
                .filter(|item| item.kind == EvidenceKind::GitHubIssue)
                .count();
            workflow_runs = evidence
                .iter()
                .filter(|item| {
                    matches!(
                        item.kind,
                        EvidenceKind::GitHubWorkflowRun
                            | EvidenceKind::GitHubWorkflowFailed
                            | EvidenceKind::GitHubWorkflowSucceeded
                    )
                })
                .count();
            releases = evidence
                .iter()
                .filter(|item| item.kind == EvidenceKind::GitHubRelease)
                .count();
            deployments = evidence
                .iter()
                .filter(|item| item.kind == EvidenceKind::GitHubDeployment)
                .count();
        }

        Ok(GitHubIngestResult {
            repository,
            evidence,
            pull_requests,
            issues,
            workflow_runs,
            releases,
            deployments,
            topics: topics.into_iter().collect(),
        })
    }
}

fn collect_topics(item: &EvidenceItem, topics: &mut std::collections::BTreeSet<String>) {
    if let Some(service) = &item.service {
        topics.insert(service.clone());
    }
    for tag in &item.tags {
        topics.insert(tag.clone());
    }
}

fn evidence_is_after_cutoff(item: &EvidenceItem, cutoff: i64) -> bool {
    item.timestamp
        .as_deref()
        .and_then(parse_github_timestamp)
        .is_none_or(|timestamp| timestamp >= cutoff)
}

fn parse_since_cutoff(value: &str) -> Result<i64> {
    let trimmed = value.trim();
    if let Some(days) = trimmed.strip_suffix('d') {
        let days = days.parse::<i64>().map_err(|_| {
            RivoraError::invalid_value(
                "github_since",
                "use an ISO timestamp or relative days like 7d",
            )
        })?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| {
                RivoraError::invalid_value("github_since", "system clock is before unix epoch")
            })?
            .as_secs() as i64;
        return Ok(now - days.saturating_mul(86_400));
    }
    parse_github_timestamp(trimmed).ok_or_else(|| {
        RivoraError::invalid_value(
            "github_since",
            "use an ISO timestamp or relative days like 7d",
        )
    })
}

fn parse_github_timestamp(value: &str) -> Option<i64> {
    let value = value.trim();
    let date_time = value.strip_suffix('Z').unwrap_or(value);
    let (date, time) = date_time.split_once('T').unwrap_or((date_time, "00:00:00"));
    let mut date_parts = date.split('-');
    let year = date_parts.next()?.parse::<i64>().ok()?;
    let month = date_parts.next()?.parse::<i64>().ok()?;
    let day = date_parts.next()?.parse::<i64>().ok()?;
    let mut time_parts = time.split(':');
    let hour = time_parts.next().unwrap_or("0").parse::<i64>().ok()?;
    let minute = time_parts.next().unwrap_or("0").parse::<i64>().ok()?;
    let second = time_parts
        .next()
        .unwrap_or("0")
        .split('.')
        .next()
        .unwrap_or("0")
        .parse::<i64>()
        .ok()?;
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || !(0..=23).contains(&hour)
        || !(0..=59).contains(&minute)
        || !(0..=60).contains(&second)
    {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second)
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

// --- GitHub API response shapes (only fields Rivora uses) -------------------
//
// These structs model the subset of GitHub REST API fields Rivora ingests.
// Some fields are captured for forward compatibility even when not yet read,
// so `dead_code` is allowed on the DTOs.

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct PullRequestResponse {
    number: u64,
    title: String,
    body: Option<String>,
    user: Option<UserRef>,
    merged_at: Option<String>,
    updated_at: Option<String>,
    state: Option<String>,
    labels: Option<Vec<LabelRef>>,
    html_url: Option<String>,
    changed_files: Option<u64>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct IssueResponse {
    number: u64,
    title: String,
    body: Option<String>,
    user: Option<UserRef>,
    state: Option<String>,
    updated_at: Option<String>,
    labels: Option<Vec<LabelRef>>,
    html_url: Option<String>,
    /// Present when the issue endpoint item is actually a pull request.
    pull_request: Option<PullRequestMarker>,
}

#[derive(Debug, Clone, Deserialize)]
struct PullRequestMarker {
    #[allow(dead_code)]
    merged_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct WorkflowRunsEnvelope {
    workflow_runs: Option<Vec<WorkflowRunResponse>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct WorkflowRunResponse {
    id: u64,
    name: Option<String>,
    head_branch: Option<String>,
    head_sha: Option<String>,
    event: Option<String>,
    status: Option<String>,
    conclusion: Option<String>,
    html_url: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
    actor: Option<UserRef>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct ReleaseResponse {
    id: u64,
    name: Option<String>,
    tag_name: Option<String>,
    body: Option<String>,
    html_url: Option<String>,
    published_at: Option<String>,
    author: Option<UserRef>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct DeploymentResponse {
    id: u64,
    environment: Option<String>,
    #[serde(rename = "ref")]
    ref_name: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
    creator: Option<UserRef>,
}

#[derive(Debug, Clone, Deserialize)]
struct UserRef {
    login: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct LabelRef {
    name: Option<String>,
}

#[must_use]
fn parse_pull_requests(raw: &str) -> Vec<PullRequestResponse> {
    serde_json::from_str(raw).unwrap_or_default()
}

#[must_use]
fn parse_issues(raw: &str) -> Vec<IssueResponse> {
    let all: Vec<IssueResponse> = serde_json::from_str(raw).unwrap_or_default();
    all.into_iter()
        .filter(|issue| issue.pull_request.is_none())
        .collect()
}

#[must_use]
fn parse_workflow_runs(raw: &str) -> Vec<WorkflowRunResponse> {
    serde_json::from_str::<WorkflowRunsEnvelope>(raw)
        .ok()
        .and_then(|envelope| envelope.workflow_runs)
        .unwrap_or_default()
}

#[must_use]
fn parse_releases(raw: &str) -> Vec<ReleaseResponse> {
    serde_json::from_str(raw).unwrap_or_default()
}

#[must_use]
fn parse_deployments(raw: &str) -> Vec<DeploymentResponse> {
    serde_json::from_str(raw).unwrap_or_default()
}

// --- Evidence mapping -------------------------------------------------------

fn pull_request_item(
    source: &EvidenceSource,
    repo: &GitHubRepositoryRef,
    pr: &PullRequestResponse,
) -> Result<EvidenceItem> {
    let merged = pr.merged_at.is_some();
    let kind = if merged {
        EvidenceKind::GitHubPullRequestMerged
    } else {
        EvidenceKind::GitHubPullRequest
    };
    let labels = label_names(&pr.labels);
    let linked = extract_linked_issues(pr.body.as_deref());
    let service = infer_github_service(&labels, pr.title.as_str(), pr.body.as_deref());
    let author = pr.user.as_ref().and_then(|user| user.login.clone());
    let url = pr.html_url.clone().unwrap_or_default();
    let timestamp = pr.merged_at.clone().or_else(|| pr.updated_at.clone());
    let summary = if merged {
        format!("PR #{} merged: \"{}\"", pr.number, pr.title)
    } else {
        format!("PR #{}: \"{}\"", pr.number, pr.title)
    };
    let body = format!(
        "Repository: {}\nAuthor: {}\nMerged: {}\nUpdated: {}\nChanged files: {}\nLabels: {}\nLinked issues: {}\nURL: {}",
        repo.full_name(),
        author.as_deref().unwrap_or("unknown"),
        pr.merged_at.as_deref().unwrap_or("no"),
        pr.updated_at.as_deref().unwrap_or("unknown"),
        pr.changed_files.unwrap_or(0),
        if labels.is_empty() { "none".to_string() } else { labels.join(", ") },
        if linked.is_empty() { "none".to_string() } else { linked.join(", ") },
        url
    );

    let mut tags = labels.clone();
    tags.extend(linked.iter().cloned());
    let mut refs = vec![format!("pr:{}", pr.number), url.clone()];
    if merged {
        refs.push("merged".to_string());
    }

    Ok(EvidenceItem {
        id: EvidenceId::new(format!("github:pr:{}:{}", repo.full_name(), pr.number))?,
        kind,
        source: source.clone(),
        title: format!("PR #{}: {}", pr.number, pr.title),
        summary,
        body,
        service,
        files_changed: Vec::new(),
        timestamp,
        author,
        tags,
        refs,
        confidence: if merged { 0.85 } else { 0.8 },
    })
}

fn issue_item(
    source: &EvidenceSource,
    repo: &GitHubRepositoryRef,
    issue: &IssueResponse,
) -> Result<EvidenceItem> {
    let labels = label_names(&issue.labels);
    let service = infer_github_service(&labels, issue.title.as_str(), issue.body.as_deref());
    let author = issue.user.as_ref().and_then(|user| user.login.clone());
    let url = issue.html_url.clone().unwrap_or_default();
    let state = issue.state.clone().unwrap_or_else(|| "open".to_string());
    let summary = format!("Issue #{} ({}): \"{}\"", issue.number, state, issue.title);
    let body = format!(
        "Repository: {}\nAuthor: {}\nState: {}\nUpdated: {}\nLabels: {}\nURL: {}",
        repo.full_name(),
        author.as_deref().unwrap_or("unknown"),
        state,
        issue.updated_at.as_deref().unwrap_or("unknown"),
        if labels.is_empty() {
            "none".to_string()
        } else {
            labels.join(", ")
        },
        url
    );

    Ok(EvidenceItem {
        id: EvidenceId::new(format!(
            "github:issue:{}:{}",
            repo.full_name(),
            issue.number
        ))?,
        kind: EvidenceKind::GitHubIssue,
        source: source.clone(),
        title: format!("Issue #{}: {}", issue.number, issue.title),
        summary,
        body,
        service,
        files_changed: Vec::new(),
        timestamp: issue.updated_at.clone(),
        author,
        tags: labels,
        refs: vec![format!("issue:{}", issue.number), url],
        confidence: 0.75,
    })
}

fn workflow_run_item(
    source: &EvidenceSource,
    repo: &GitHubRepositoryRef,
    run: &WorkflowRunResponse,
) -> Result<EvidenceItem> {
    let conclusion = run.conclusion.as_deref().unwrap_or("unknown");
    let kind = match conclusion {
        "failure" | "cancelled" | "timed_out" | "action_required" => {
            EvidenceKind::GitHubWorkflowFailed
        }
        "success" => EvidenceKind::GitHubWorkflowSucceeded,
        _ => EvidenceKind::GitHubWorkflowRun,
    };
    let name = run.name.clone().unwrap_or_else(|| "workflow".to_string());
    let branch = run
        .head_branch
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let event = run.event.clone().unwrap_or_else(|| "unknown".to_string());
    let sha = run.head_sha.clone().unwrap_or_default();
    let actor = run.actor.as_ref().and_then(|user| user.login.clone());
    let url = run.html_url.clone().unwrap_or_default();
    let service = infer_github_service(&[], name.as_str(), Some(branch.as_str()));
    let summary = format!(
        "Workflow \"{}\" {} on {} ({})",
        name, conclusion, branch, event
    );
    let body = format!(
        "Repository: {}\nWorkflow: {}\nConclusion: {}\nStatus: {}\nBranch: {}\nCommit: {}\nEvent: {}\nActor: {}\nURL: {}",
        repo.full_name(),
        name,
        conclusion,
        run.status.as_deref().unwrap_or("unknown"),
        branch,
        short_sha(&sha),
        event,
        actor.as_deref().unwrap_or("unknown"),
        url
    );

    let mut tags = vec![
        name.clone(),
        branch.clone(),
        event.clone(),
        conclusion.to_string(),
    ];
    tags.sort();
    tags.dedup();

    Ok(EvidenceItem {
        id: EvidenceId::new(format!("github:workflow:{}:{}", repo.full_name(), run.id))?,
        kind,
        source: source.clone(),
        title: format!("Workflow \"{}\" {}", name, conclusion),
        summary,
        body,
        service,
        files_changed: Vec::new(),
        timestamp: run.updated_at.clone().or_else(|| run.created_at.clone()),
        author: actor,
        tags,
        refs: vec![format!("run:{}", run.id), url, sha],
        confidence: if kind == EvidenceKind::GitHubWorkflowFailed {
            0.9
        } else {
            0.85
        },
    })
}

fn release_item(
    source: &EvidenceSource,
    repo: &GitHubRepositoryRef,
    release: &ReleaseResponse,
) -> Result<EvidenceItem> {
    let tag = release
        .tag_name
        .clone()
        .unwrap_or_else(|| "untagged".to_string());
    let name = release.name.clone().unwrap_or_else(|| tag.clone());
    let author = release.author.as_ref().and_then(|user| user.login.clone());
    let url = release.html_url.clone().unwrap_or_default();
    let service = infer_github_service(&[], tag.as_str(), Some(name.as_str()));
    let summary = format!("Release {} published", tag);
    let body = format!(
        "Repository: {}\nTag: {}\nName: {}\nPublished: {}\nAuthor: {}\nURL: {}",
        repo.full_name(),
        tag,
        name,
        release.published_at.as_deref().unwrap_or("unknown"),
        author.as_deref().unwrap_or("unknown"),
        url
    );

    Ok(EvidenceItem {
        id: EvidenceId::new(format!(
            "github:release:{}:{}",
            repo.full_name(),
            release.id
        ))?,
        kind: EvidenceKind::GitHubRelease,
        source: source.clone(),
        title: format!("Release {}: {}", tag, name),
        summary,
        body,
        service,
        files_changed: Vec::new(),
        timestamp: release.published_at.clone(),
        author,
        tags: vec![tag.clone()],
        refs: vec![format!("release:{}", release.id), url, tag],
        confidence: 0.8,
    })
}

fn deployment_item(
    source: &EvidenceSource,
    repo: &GitHubRepositoryRef,
    deployment: &DeploymentResponse,
) -> Result<EvidenceItem> {
    let environment = deployment
        .environment
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let ref_name = deployment
        .ref_name
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let creator = deployment
        .creator
        .as_ref()
        .and_then(|user| user.login.clone());
    let service = infer_github_service(&[], environment.as_str(), Some(ref_name.as_str()));
    let summary = format!("Deployed {} to {}", ref_name, environment);
    let body = format!(
        "Repository: {}\nEnvironment: {}\nRef: {}\nCreated: {}\nCreator: {}",
        repo.full_name(),
        environment,
        ref_name,
        deployment.created_at.as_deref().unwrap_or("unknown"),
        creator.as_deref().unwrap_or("unknown")
    );

    Ok(EvidenceItem {
        id: EvidenceId::new(format!(
            "github:deployment:{}:{}",
            repo.full_name(),
            deployment.id
        ))?,
        kind: EvidenceKind::GitHubDeployment,
        source: source.clone(),
        title: format!("Deployment to {} ({})", environment, ref_name),
        summary,
        body,
        service,
        files_changed: Vec::new(),
        timestamp: deployment
            .updated_at
            .clone()
            .or_else(|| deployment.created_at.clone()),
        author: creator,
        tags: vec![environment.clone(), ref_name.clone()],
        refs: vec![format!("deployment:{}", deployment.id)],
        confidence: 0.75,
    })
}

fn label_names(labels: &Option<Vec<LabelRef>>) -> Vec<String> {
    labels
        .as_ref()
        .map(|labels| {
            labels
                .iter()
                .filter_map(|label| label.name.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// Infer a service/topic from explicit signals only:
/// `service:<name>` / `area:<name>` labels, or the same pattern in text.
fn infer_github_service(labels: &[String], title: &str, body: Option<&str>) -> Option<String> {
    for label in labels {
        if let Some(name) = label
            .strip_prefix("service:")
            .or_else(|| label.strip_prefix("area:"))
        {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                return Some(slug(trimmed));
            }
        }
    }
    for text in [title, body.unwrap_or("")] {
        if let Some(name) = find_prefixed(text, "service:").or_else(|| find_prefixed(text, "area:"))
        {
            return Some(slug(&name));
        }
    }
    None
}

fn find_prefixed(text: &str, prefix: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let mut search = lower.as_str();
    while let Some(start) = search.find(prefix) {
        let rest = &search[start + prefix.len()..];
        let token = rest
            .split(|c: char| {
                !c.is_ascii_alphanumeric() && c != '-' && c != '_' && c != '.' && c != '/'
            })
            .next()?;
        let trimmed = token.trim_end_matches('/');
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
        search = &search[start + prefix.len()..];
    }
    None
}

/// Extract `#<number>` references from text, including those preceded by
/// `fixes`, `closes`, `resolves`, or `relates to`.
#[must_use]
pub fn extract_linked_issues(body: Option<&str>) -> Vec<String> {
    let Some(body) = body else {
        return Vec::new();
    };
    let lower = body.to_ascii_lowercase();
    let keywords = ["fixes", "closes", "resolves", "relates to", "refs"];
    let mut linked = Vec::new();
    for keyword in keywords {
        let mut search = lower.as_str();
        while let Some(pos) = search.find(keyword) {
            let rest = &search[pos + keyword.len()..];
            if let Some(number) = rest
                .trim_start()
                .strip_prefix('#')
                .and_then(|rest| rest.split(|c: char| !c.is_ascii_digit()).next())
            {
                if !number.is_empty() {
                    linked.push(format!("#{number}"));
                }
            }
            search = &search[pos + keyword.len()..];
        }
    }
    linked.sort();
    linked.dedup();
    linked
}

fn short_sha(sha: &str) -> String {
    sha.chars().take(7).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> GitHubRepositoryRef {
        GitHubRepositoryRef::new("owner", "name")
    }

    fn default_request() -> GitHubIngestRequest {
        GitHubIngestRequest::new(repo()).with_limit(10)
    }

    const FIXTURE_PRS: &str = r#"[
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
        },
        {
            "number": 129,
            "title": "Draft inventory sync",
            "body": "Draft work",
            "user": { "login": "grace" },
            "merged_at": null,
            "updated_at": "2026-06-28T00:00:00Z",
            "state": "open",
            "labels": [],
            "html_url": "https://github.com/owner/name/pull/129",
            "changed_files": 1
        }
    ]"#;

    const FIXTURE_ISSUES: &str = r#"[
        {
            "number": 200,
            "title": "Checkout latency spike",
            "body": "area:checkout saw p99 latency",
            "user": { "login": "ada" },
            "state": "open",
            "updated_at": "2026-06-27T12:00:00Z",
            "labels": [ { "name": "area:checkout" }, { "name": "incident" } ],
            "html_url": "https://github.com/owner/name/issues/200"
        },
        {
            "number": 128,
            "title": "Pull request leaked into issues endpoint",
            "body": null,
            "state": "closed",
            "updated_at": "2026-06-27T10:00:00Z",
            "labels": [],
            "html_url": "https://github.com/owner/name/pull/128",
            "pull_request": { "merged_at": "2026-06-27T10:00:00Z" }
        }
    ]"#;

    const FIXTURE_WORKFLOW_RUNS: &str = r#"{
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
            },
            {
                "id": 1002,
                "name": "deploy",
                "head_branch": "release",
                "head_sha": "fedcba0987654321",
                "event": "push",
                "status": "completed",
                "conclusion": "success",
                "html_url": "https://github.com/owner/name/actions/runs/1002",
                "created_at": "2026-06-27T09:00:00Z",
                "updated_at": "2026-06-27T09:10:00Z",
                "actor": { "login": "grace" }
            }
        ]
    }"#;

    const FIXTURE_RELEASES: &str = r#"[
        {
            "id": 5001,
            "name": "Checkout v1.4",
            "tag_name": "checkout-v1.4",
            "body": "service:checkout release",
            "html_url": "https://github.com/owner/name/releases/tag/checkout-v1.4",
            "published_at": "2026-06-27T11:00:00Z",
            "author": { "login": "ada" }
        }
    ]"#;

    const FIXTURE_DEPLOYMENTS: &str = r#"[
        {
            "id": 7001,
            "environment": "production",
            "ref": "checkout-v1.4",
            "created_at": "2026-06-27T11:30:00Z",
            "updated_at": "2026-06-27T11:35:00Z",
            "creator": { "login": "ada" }
        }
    ]"#;

    #[test]
    fn parses_pull_request_fixture_into_evidence() {
        let prs = parse_pull_requests(FIXTURE_PRS);
        assert_eq!(prs.len(), 2);

        let client = FixtureGitHubClient::builder()
            .pull_requests(FIXTURE_PRS)
            .build();
        let connector = GitHubConnector::new(client);
        let result = connector
            .ingest(GitHubIngestRequest::new(repo()).with_pull_requests())
            .unwrap();

        assert_eq!(result.pull_requests, 2);
        let merged = result
            .evidence
            .iter()
            .find(|item| item.kind == EvidenceKind::GitHubPullRequestMerged)
            .unwrap();
        assert_eq!(merged.id.as_str(), "github:pr:owner/name:128");
        assert!(merged.summary.contains("PR #128 merged"));
        assert_eq!(merged.service.as_deref(), Some("checkout"));
        assert!(merged.tags.contains(&"#120".to_string()));
        assert!(merged.body.contains("Linked issues: #120"));
        assert!(merged.refs.contains(&"merged".to_string()));
    }

    #[test]
    fn parses_issue_fixture_and_filters_out_pull_requests() {
        let parsed = parse_issues(FIXTURE_ISSUES);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].number, 200);

        let client = FixtureGitHubClient::builder()
            .issues(FIXTURE_ISSUES)
            .build();
        let connector = GitHubConnector::new(client);
        let result = connector
            .ingest(GitHubIngestRequest::new(repo()).with_issues())
            .unwrap();

        assert_eq!(result.issues, 1);
        assert_eq!(
            result.evidence[0].id.as_str(),
            "github:issue:owner/name:200"
        );
        assert_eq!(result.evidence[0].service.as_deref(), Some("checkout"));
        assert!(result.evidence[0].tags.contains(&"incident".to_string()));
    }

    #[test]
    fn parses_workflow_runs_into_failed_and_succeeded_evidence() {
        let runs = parse_workflow_runs(FIXTURE_WORKFLOW_RUNS);
        assert_eq!(runs.len(), 2);

        let client = FixtureGitHubClient::builder()
            .workflow_runs(FIXTURE_WORKFLOW_RUNS)
            .build();
        let connector = GitHubConnector::new(client);
        let result = connector
            .ingest(GitHubIngestRequest::new(repo()).with_workflow_runs())
            .unwrap();

        assert_eq!(result.workflow_runs, 2);
        let failed = result
            .evidence
            .iter()
            .find(|item| item.kind == EvidenceKind::GitHubWorkflowFailed)
            .unwrap();
        assert_eq!(failed.id.as_str(), "github:workflow:owner/name:1001");
        assert!(failed.summary.contains("failure"));
        assert!(failed.body.contains("Commit: abcdef1"));
        assert_eq!(failed.confidence, 0.9);

        let succeeded = result
            .evidence
            .iter()
            .find(|item| item.kind == EvidenceKind::GitHubWorkflowSucceeded)
            .unwrap();
        assert!(succeeded.summary.contains("success"));
    }

    #[test]
    fn parses_releases_and_deployments_into_evidence() {
        let client = FixtureGitHubClient::builder()
            .releases(FIXTURE_RELEASES)
            .deployments(FIXTURE_DEPLOYMENTS)
            .build();
        let connector = GitHubConnector::new(client);
        let result = connector
            .ingest(
                GitHubIngestRequest::new(repo())
                    .with_releases()
                    .with_deployments(),
            )
            .unwrap();

        assert_eq!(result.releases, 1);
        assert_eq!(result.deployments, 1);
        assert_eq!(
            result.evidence[0].id.as_str(),
            "github:deployment:owner/name:7001"
        );
        assert_eq!(
            result.evidence[1].id.as_str(),
            "github:release:owner/name:5001"
        );
    }

    #[test]
    fn github_evidence_ids_are_stable_across_ingests() {
        let client = FixtureGitHubClient::builder()
            .pull_requests(FIXTURE_PRS)
            .workflow_runs(FIXTURE_WORKFLOW_RUNS)
            .build();
        let connector = GitHubConnector::new(client.clone());
        let first = connector
            .ingest(
                GitHubIngestRequest::new(repo())
                    .with_pull_requests()
                    .with_workflow_runs(),
            )
            .unwrap();
        let connector2 = GitHubConnector::new(client);
        let second = connector2
            .ingest(
                GitHubIngestRequest::new(repo())
                    .with_pull_requests()
                    .with_workflow_runs(),
            )
            .unwrap();

        let first_ids: Vec<_> = first
            .evidence
            .iter()
            .map(|item| item.id.as_str().to_string())
            .collect();
        let second_ids: Vec<_> = second
            .evidence
            .iter()
            .map(|item| item.id.as_str().to_string())
            .collect();
        assert_eq!(first_ids, second_ids);
    }

    #[test]
    fn github_connector_deduplicates_by_id_within_one_ingest() {
        let client = FixtureGitHubClient::builder()
            .pull_requests(FIXTURE_PRS)
            .issues(FIXTURE_ISSUES)
            .workflow_runs(FIXTURE_WORKFLOW_RUNS)
            .releases(FIXTURE_RELEASES)
            .build();
        let connector = GitHubConnector::new(client);
        let result = connector.ingest(default_request()).unwrap();
        let mut ids: Vec<_> = result
            .evidence
            .iter()
            .map(|item| item.id.as_str().to_string())
            .collect();
        ids.sort();
        let mut deduped = ids.clone();
        deduped.dedup();
        assert_eq!(ids, deduped, "duplicate evidence ids within one ingest");
    }

    #[test]
    fn default_ingest_selects_prs_issues_workflows_and_releases() {
        let client = FixtureGitHubClient::builder()
            .pull_requests(FIXTURE_PRS)
            .issues(FIXTURE_ISSUES)
            .workflow_runs(FIXTURE_WORKFLOW_RUNS)
            .releases(FIXTURE_RELEASES)
            .build();
        let connector = GitHubConnector::new(client);
        let request = GitHubIngestRequest::new(repo());
        assert!(request.no_sources_selected());
        let result = connector.ingest(request).unwrap();

        assert!(result.pull_requests > 0);
        assert!(result.issues > 0);
        assert!(result.workflow_runs > 0);
        assert!(result.releases > 0);
        assert_eq!(result.deployments, 0);
    }

    #[test]
    fn since_filter_keeps_only_evidence_at_or_after_cutoff() {
        let client = FixtureGitHubClient::builder()
            .pull_requests(FIXTURE_PRS)
            .build();
        let connector = GitHubConnector::new(client);
        let result = connector
            .ingest(
                GitHubIngestRequest::new(repo())
                    .with_pull_requests()
                    .with_since("2026-06-28T00:00:00Z"),
            )
            .unwrap();

        assert_eq!(result.pull_requests, 1);
        assert_eq!(result.evidence.len(), 1);
        assert_eq!(result.evidence[0].id.as_str(), "github:pr:owner/name:129");
    }

    #[test]
    fn since_filter_rejects_unparseable_values() {
        let client = FixtureGitHubClient::builder()
            .pull_requests(FIXTURE_PRS)
            .build();
        let connector = GitHubConnector::new(client);
        let result = connector.ingest(
            GitHubIngestRequest::new(repo())
                .with_pull_requests()
                .with_since("recently"),
        );

        assert!(result.is_err());
    }

    #[test]
    fn missing_fixture_returns_provider_error() {
        let client = FixtureGitHubClient::builder().build();
        let connector = GitHubConnector::new(client);
        let result = connector.ingest(GitHubIngestRequest::new(repo()).with_pull_requests());
        assert!(result.is_err());
    }

    #[test]
    fn repository_ref_parses_owner_name() {
        let repo = GitHubRepositoryRef::parse("  sgr0691/Root  ").unwrap();
        assert_eq!(repo.owner, "sgr0691");
        assert_eq!(repo.name, "Root");
        assert_eq!(repo.full_name(), "sgr0691/Root");

        assert!(GitHubRepositoryRef::parse("no-slash").is_err());
        assert!(GitHubRepositoryRef::parse("/missing-owner").is_err());
        assert!(GitHubRepositoryRef::parse("missing-name/").is_err());
    }

    #[test]
    fn redact_token_scrubs_secret_from_strings() {
        let auth = GitHubAuthConfig::with_token("ghp_secret_token");
        assert_eq!(
            auth.redact("error: ghp_secret_token is invalid"),
            "error: [redacted] is invalid"
        );
        assert_eq!(redact_token("plain text", "ghp_secret_token"), "plain text");
        assert_eq!(redact_token("plain text", ""), "plain text");
    }

    #[test]
    fn auth_from_env_is_anonymous_when_unset() {
        std::env::remove_var("GITHUB_TOKEN");
        let auth = GitHubAuthConfig::from_env();
        assert!(!auth.has_token());
    }

    #[test]
    fn http_client_request_config_uses_only_get_and_carries_token_via_stdin() {
        let auth = GitHubAuthConfig::with_token("ghp_secret_token");
        let client = HttpGitHubClient::new(auth);
        let config = client.request_config("/repos/owner/name/pulls");

        assert!(config.contains("request = \"GET\""));
        assert!(!config.contains("\"POST\""));
        assert!(!config.contains("\"PUT\""));
        assert!(!config.contains("\"PATCH\""));
        assert!(!config.contains("\"DELETE\""));
        // Token is present in the config (piped to curl over stdin) but must
        // never appear in process args; redaction removes it for any output.
        assert!(config.contains("ghp_secret_token"));
        assert!(!client.auth.redact(&config).contains("ghp_secret_token"));
    }

    #[test]
    fn github_connector_exposes_no_mutation_http_methods() {
        for method in github_forbidden_http_methods() {
            assert!(!github_allowed_http_methods().contains(method));
        }
        assert_eq!(github_allowed_http_methods(), &["GET"]);
    }

    #[test]
    fn github_evidence_never_embeds_auth_token() {
        // A fixture body that accidentally mentions a token-like string must
        // still not carry the configured auth token into evidence.
        let fixture = r#"[
            {
                "number": 1,
                "title": "Rotate keys",
                "body": "service:checkout ghp_should_not_leak",
                "user": { "login": "ada" },
                "merged_at": null,
                "updated_at": "2026-06-28T00:00:00Z",
                "state": "open",
                "labels": [],
                "html_url": "https://github.com/owner/name/pull/1",
                "changed_files": 1
            }
        ]"#;
        let client = FixtureGitHubClient::builder()
            .pull_requests(fixture)
            .build();
        let connector = GitHubConnector::new(client);
        let result = connector
            .ingest(GitHubIngestRequest::new(repo()).with_pull_requests())
            .unwrap();

        for item in &result.evidence {
            assert!(!item.body.contains("ghp_should_not_leak"), "{}", item.body);
            assert!(!item.summary.contains("ghp_should_not_leak"));
            assert!(!item.refs.iter().any(|r| r.contains("ghp_should_not_leak")));
        }
    }

    #[test]
    fn extract_linked_issues_finds_keyword_refs() {
        let linked = extract_linked_issues(Some("Fixes #120 and closes #121. Relates to #130."));
        assert_eq!(linked, vec!["#120", "#121", "#130"]);
        assert!(extract_linked_issues(None).is_empty());
    }

    #[test]
    fn infer_service_from_labels_and_text() {
        assert_eq!(
            infer_github_service(&["service:checkout".to_string()], "", None),
            Some("checkout".to_string())
        );
        assert_eq!(
            infer_github_service(&[], "area:payments deployment", None),
            Some("payments".to_string())
        );
        assert_eq!(infer_github_service(&[], "no signals here", None), None);
    }

    #[test]
    fn github_evidence_items_are_marked_github() {
        let client = FixtureGitHubClient::builder()
            .pull_requests(FIXTURE_PRS)
            .build();
        let connector = GitHubConnector::new(client);
        let result = connector
            .ingest(GitHubIngestRequest::new(repo()).with_pull_requests())
            .unwrap();

        for item in &result.evidence {
            assert!(item.is_github());
            assert_eq!(item.source.connector, GITHUB_CONNECTOR);
            assert!(item.source.read_only);
        }
    }

    #[test]
    fn connector_version_is_shared() {
        let source = EvidenceSource::github("owner/name");
        assert_eq!(source.version, crate::CONNECTOR_VERSION);
    }
}
