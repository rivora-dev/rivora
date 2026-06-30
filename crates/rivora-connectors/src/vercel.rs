//! Read-only Vercel evidence connector.
//!
//! Feeds Vercel deployment evidence into Rivora's evidence store. The
//! connector is strictly read-only: it only issues `GET` requests against the
//! Vercel REST API and never calls mutation endpoints (`POST`, `PUT`, `PATCH`,
//! `DELETE`). It never creates, promotes, rolls back, or deletes deployments,
//! and it never reads or writes project environment variables.
//!
//! Authentication requires a `VERCEL_TOKEN` environment variable. Tokens are
//! never stored in `.rivora/`, never printed, and never written into evidence
//! bodies, logs, or receipts. Error messages are redacted so tokens cannot
//! leak through `curl` stderr.

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use rivora_errors::{Result, RivoraError};
use serde::Deserialize;

use crate::{slug, EvidenceId, EvidenceItem, EvidenceKind, EvidenceSource};

/// Connector name written into [`EvidenceSource::connector`].
pub const VERCEL_CONNECTOR: &str = "vercel";
/// Default Vercel REST API base URL.
pub const VERCEL_API_BASE: &str = "https://api.vercel.com";

/// HTTP methods the Vercel connector is allowed to use. The connector only
/// ever issues `GET` requests.
#[must_use]
pub fn vercel_allowed_http_methods() -> &'static [&'static str] {
    &["GET"]
}

/// HTTP methods the Vercel connector must never use.
#[must_use]
pub fn vercel_forbidden_http_methods() -> &'static [&'static str] {
    &["POST", "PUT", "PATCH", "DELETE"]
}

/// Replace any occurrence of `token` in `value` with `[redacted]`.
///
/// Used to scrub `curl` stderr and any other string before it can appear in an
/// error message. Returns `value` unchanged when `token` is empty.
#[must_use]
fn redact_token(value: &str, token: &str) -> String {
    if token.is_empty() {
        value.to_string()
    } else {
        value.replace(token, "[redacted]")
    }
}

/// Vercel authentication configuration.
///
/// The token is held privately and never exposed through a getter. Use
/// [`Self::has_token`] to check whether a token is configured and
/// [`Self::redact`] to scrub strings before they can appear in errors or logs.
#[derive(Debug, Clone)]
pub struct VercelAuthConfig {
    token: Option<String>,
}

impl VercelAuthConfig {
    /// Read the `VERCEL_TOKEN` environment variable if present and non-empty.
    #[must_use]
    pub fn from_env() -> Self {
        let token = std::env::var("VERCEL_TOKEN")
            .ok()
            .filter(|token| !token.trim().is_empty());
        Self { token }
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

/// A Vercel project reference: a project id-or-name with an optional team
/// id-or-slug.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VercelProjectRef {
    pub project: String,
    pub team: Option<String>,
}

impl VercelProjectRef {
    /// Parse a project reference. A bare project name is accepted; a
    /// `project@team` form is also accepted for convenience.
    pub fn parse(value: &str) -> Result<Self> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(RivoraError::invalid_value(
                "vercel_project",
                "expected a Vercel project id or name",
            ));
        }
        let (project, team) = if let Some((project, team)) = trimmed.split_once('@') {
            let project = project.trim();
            let team = team.trim();
            if project.is_empty() || team.is_empty() {
                return Err(RivoraError::invalid_value(
                    "vercel_project",
                    "project@team requires both a project and a team",
                ));
            }
            (project.to_string(), Some(team.to_string()))
        } else {
            (trimmed.to_string(), None)
        };
        Ok(Self { project, team })
    }

    #[must_use]
    pub fn new(project: impl Into<String>, team: Option<impl Into<String>>) -> Self {
        Self {
            project: project.into(),
            team: team.map(|t| t.into()),
        }
    }

    /// The repository label written into [`EvidenceSource`]. Uses the project
    /// name plus an optional team scope so evidence provenance is unambiguous.
    #[must_use]
    pub fn repository_label(&self) -> String {
        match &self.team {
            Some(team) => format!("{}@{}", self.project, team),
            None => self.project.clone(),
        }
    }
}

/// Choose the Vercel team query parameter for a team id-or-slug. Vercel team
/// ids start with `team_`; anything else is treated as a team slug.
fn team_query_param(team: &str) -> (&'static str, &str) {
    if team.starts_with("team_") {
        ("teamId", team)
    } else {
        ("slug", team)
    }
}

/// Read-only Vercel API client contract.
///
/// Every method issues a `GET` request and returns the raw JSON body. The
/// trait intentionally exposes no mutation operations.
pub trait VercelClient {
    fn fetch_deployments(&self, project: &VercelProjectRef, limit: usize) -> Result<String>;
}

/// Real Vercel REST API client backed by `curl`.
///
/// The token is passed to `curl` through stdin (`--config -`) so it never
/// appears in the process argument list and is not visible via `ps`. The
/// client only constructs `GET` requests.
#[derive(Debug, Clone)]
pub struct HttpVercelClient {
    auth: VercelAuthConfig,
    base_url: String,
}

impl HttpVercelClient {
    #[must_use]
    pub fn new(auth: VercelAuthConfig) -> Self {
        Self {
            auth,
            base_url: VERCEL_API_BASE.to_string(),
        }
    }

    /// Build the `curl` `--config -` body for a `GET` request against `path`.
    /// The configured token is included here so it can be piped to `curl` over
    /// stdin instead of as a process argument.
    pub(crate) fn request_config(&self, path: &str) -> String {
        let url = format!("{}{}", self.base_url, path);
        let mut config = String::new();
        config.push_str(&format!("url = \"{url}\"\n"));
        config.push_str("silent\n");
        config.push_str("show-error\n");
        config.push_str("fail\n");
        config.push_str("request = \"GET\"\n");
        config.push_str("header = \"Accept: application/json\"\n");
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
            .map_err(|e| RivoraError::provider("vercel", format!("curl unavailable: {e}")))?;
        {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| RivoraError::provider("vercel", "could not open curl stdin pipe"))?;
            stdin.write_all(config.as_bytes()).map_err(|e| {
                RivoraError::provider("vercel", format!("curl config write failed: {e}"))
            })?;
        }
        let output = child
            .wait_with_output()
            .map_err(|e| RivoraError::provider("vercel", format!("curl did not finish: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let redacted = self.auth.redact(stderr.as_ref());
            return Err(RivoraError::provider(
                "vercel",
                format!(
                    "Vercel API request failed for {}: {}",
                    path,
                    redacted.trim()
                ),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

impl VercelClient for HttpVercelClient {
    fn fetch_deployments(&self, project: &VercelProjectRef, limit: usize) -> Result<String> {
        let mut path = format!(
            "/v7/deployments?limit={}&projectId={}",
            clamp_limit(limit),
            project.project
        );
        if let Some(team) = &project.team {
            let (param, value) = team_query_param(team);
            path.push_str(&format!("&{param}={value}"));
        }
        self.get(&path)
    }
}

/// Test double for [`VercelClient`] that returns preloaded fixture JSON without
/// any network access.
#[derive(Debug, Clone, Default)]
pub struct FixtureVercelClient {
    deployments: Option<String>,
}

impl FixtureVercelClient {
    #[must_use]
    pub fn builder() -> FixtureVercelClientBuilder {
        FixtureVercelClientBuilder::default()
    }
}

impl VercelClient for FixtureVercelClient {
    fn fetch_deployments(&self, _project: &VercelProjectRef, _limit: usize) -> Result<String> {
        self.deployments
            .clone()
            .ok_or_else(|| RivoraError::provider("vercel", "no fixture loaded for deployments"))
    }
}

#[derive(Debug, Default, Clone)]
pub struct FixtureVercelClientBuilder {
    deployments: Option<String>,
}

impl FixtureVercelClientBuilder {
    #[must_use]
    pub fn deployments(mut self, fixture: impl Into<String>) -> Self {
        self.deployments = Some(fixture.into());
        self
    }

    #[must_use]
    pub fn build(self) -> FixtureVercelClient {
        FixtureVercelClient {
            deployments: self.deployments,
        }
    }
}

fn clamp_limit(limit: usize) -> usize {
    limit.clamp(1, 100)
}

/// Request for Vercel evidence ingestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VercelIngestRequest {
    pub project: VercelProjectRef,
    pub limit: usize,
    pub since: Option<String>,
    pub deployments: bool,
}

impl VercelIngestRequest {
    #[must_use]
    pub fn new(project: VercelProjectRef) -> Self {
        Self {
            project,
            limit: 20,
            since: None,
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
    pub fn with_deployments(mut self) -> Self {
        self.deployments = true;
        self
    }

    /// True when no source flags were set. The connector uses this to apply the
    /// default source set (deployments on).
    #[must_use]
    pub fn no_sources_selected(&self) -> bool {
        !self.deployments
    }
}

/// Result of Vercel evidence ingestion.
#[derive(Debug, Clone, PartialEq)]
pub struct VercelIngestResult {
    pub repository: String,
    pub evidence: Vec<EvidenceItem>,
    pub deployments: usize,
    pub topics: Vec<String>,
}

/// Read-only Vercel connector. Holds a boxed [`VercelClient`] so the CLI can
/// swap in a [`FixtureVercelClient`] for tests without generics leaking into
/// calling code.
pub struct VercelConnector {
    client: Box<dyn VercelClient>,
}

impl std::fmt::Debug for VercelConnector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VercelConnector").finish_non_exhaustive()
    }
}

impl VercelConnector {
    #[must_use]
    pub fn new(client: impl VercelClient + 'static) -> Self {
        Self {
            client: Box::new(client),
        }
    }

    pub fn ingest(&self, request: VercelIngestRequest) -> Result<VercelIngestResult> {
        if request.limit == 0 {
            return Err(RivoraError::invalid_value(
                "limit",
                "limit must be positive",
            ));
        }

        let project = request.project.clone();
        let repository = project.repository_label();
        let source = EvidenceSource::vercel(repository.clone());
        let limit = request.limit;
        let want_deployments = request.deployments || request.no_sources_selected();

        let mut evidence = Vec::new();
        let since_cutoff = request
            .since
            .as_deref()
            .map(parse_since_cutoff)
            .transpose()?;
        let mut topics = std::collections::BTreeSet::new();
        let mut deployments = 0;

        if want_deployments {
            let raw = self.client.fetch_deployments(&project, limit)?;
            let parsed = parse_deployments(&raw);
            deployments = parsed.len();
            for deployment in parsed {
                let item = deployment_item(&source, &project, &deployment)?;
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
            deployments = evidence
                .iter()
                .filter(|item| item.kind == EvidenceKind::VercelDeployment)
                .count();
        }

        Ok(VercelIngestResult {
            repository,
            evidence,
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
        .and_then(parse_vercel_timestamp)
        .is_none_or(|timestamp| timestamp >= cutoff)
}

fn parse_since_cutoff(value: &str) -> Result<i64> {
    let trimmed = value.trim();
    if let Some(days) = trimmed.strip_suffix('d') {
        let days = days.parse::<i64>().map_err(|_| {
            RivoraError::invalid_value(
                "vercel_since",
                "use an ISO timestamp or relative days like 7d",
            )
        })?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| {
                RivoraError::invalid_value("vercel_since", "system clock is before unix epoch")
            })?
            .as_secs() as i64;
        return Ok(now - days.saturating_mul(86_400));
    }
    parse_vercel_timestamp(trimmed).ok_or_else(|| {
        RivoraError::invalid_value(
            "vercel_since",
            "use an ISO timestamp or relative days like 7d",
        )
    })
}

fn parse_vercel_timestamp(value: &str) -> Option<i64> {
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

/// Inverse of [`days_from_civil`]: days-since-epoch to (year, month, day).
/// Howard Hinnant's `civil_from_days` algorithm.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Convert a Vercel millisecond-epoch timestamp into an ISO 8601 string so it
/// sorts consistently with Git/GitHub evidence timestamps.
fn ms_epoch_to_iso(ms: u64) -> String {
    let total_seconds = ms / 1000;
    let days = (total_seconds / 86_400) as i64;
    let remainder = total_seconds % 86_400;
    let hour = remainder / 3_600;
    let minute = (remainder % 3_600) / 60;
    let second = remainder % 60;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, minute, second
    )
}

// --- Vercel API response shapes (only fields Rivora uses) -------------------
//
// These structs model the subset of Vercel REST API fields Rivora ingests.
// Some fields are captured for forward compatibility even when not yet read,
// so `dead_code` is allowed on the DTOs. Vercel deployment timestamps
// (`created`, `createdAt`, `ready`) are millisecond-epoch numbers.

#[derive(Debug, Clone, Deserialize)]
struct DeploymentsEnvelope {
    deployments: Option<Vec<DeploymentResponse>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct DeploymentResponse {
    uid: Option<String>,
    name: Option<String>,
    project_id: Option<String>,
    url: Option<String>,
    created: Option<u64>,
    created_at: Option<u64>,
    building_at: Option<u64>,
    ready: Option<u64>,
    state: Option<String>,
    ready_state: Option<String>,
    target: Option<String>,
    meta: Option<serde_json::Map<String, serde_json::Value>>,
    inspector_url: Option<String>,
    #[serde(rename = "errorCode")]
    error_code: Option<String>,
    #[serde(rename = "errorMessage")]
    error_message: Option<String>,
    source: Option<String>,
}

#[must_use]
fn parse_deployments(raw: &str) -> Vec<DeploymentResponse> {
    serde_json::from_str::<DeploymentsEnvelope>(raw)
        .ok()
        .and_then(|envelope| envelope.deployments)
        .unwrap_or_default()
}

fn meta_string(
    meta: &Option<serde_json::Map<String, serde_json::Value>>,
    key: &str,
) -> Option<String> {
    meta.as_ref()?
        .get(key)?
        .as_str()
        .map(std::string::ToString::to_string)
}

// --- Evidence mapping -------------------------------------------------------

fn deployment_item(
    source: &EvidenceSource,
    project: &VercelProjectRef,
    deployment: &DeploymentResponse,
) -> Result<EvidenceItem> {
    let uid = deployment.uid.clone().unwrap_or_default();
    let name = deployment
        .name
        .clone()
        .or(Some(project.project.clone()))
        .unwrap_or_else(|| "unknown".to_string());
    let project_slug = slug(&project.project);
    let ready_state = deployment
        .ready_state
        .clone()
        .or_else(|| deployment.state.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let environment = deployment
        .target
        .clone()
        .filter(|target| !target.is_empty())
        .map(|target| target.to_ascii_lowercase())
        .unwrap_or_else(|| "preview".to_string());
    let is_production = environment == "production";
    let is_failed = matches!(ready_state.as_str(), "ERROR" | "CANCELED");
    let is_ready = ready_state == "READY";

    let branch = meta_string(&deployment.meta, "githubCommitRef")
        .or_else(|| meta_string(&deployment.meta, "branch"))
        .unwrap_or_else(|| "unknown".to_string());
    let commit_sha = meta_string(&deployment.meta, "githubCommitSha").unwrap_or_default();
    let _commit_message = meta_string(&deployment.meta, "githubCommitMessage").unwrap_or_default();
    let commit_author = meta_string(&deployment.meta, "githubCommitAuthorName")
        .or_else(|| meta_string(&deployment.meta, "commitAuthorName"));
    let repo_full = meta_string(&deployment.meta, "githubCommitRepoFullName");

    let url = deployment
        .url
        .as_deref()
        .filter(|url| !url.is_empty())
        .map(|url| {
            if url.starts_with("http://") || url.starts_with("https://") {
                url.to_string()
            } else {
                format!("https://{url}")
            }
        })
        .unwrap_or_default();
    let inspector_url = deployment.inspector_url.clone().unwrap_or_default();

    let created_ms = deployment.created_at.or(deployment.created).unwrap_or(0);
    let ready_ms = deployment.ready.unwrap_or(0);
    let timestamp = if created_ms > 0 {
        ms_epoch_to_iso(created_ms)
    } else {
        String::new()
    };

    let summary = if is_failed {
        format!("Vercel deployment for {} failed ({})", name, ready_state)
    } else if is_ready {
        format!("Vercel deployment for {} ready ({})", name, environment)
    } else {
        format!("Vercel deployment for {} {}", name, ready_state)
    };
    let body = format!(
        "Project: {}\nDeployment: {}\nState: {}\nEnvironment: {}\nBranch: {}\nCommit: {}\nAuthor: {}\nCreated: {}\nReady: {}\nURL: {}\nInspector: {}\nError code: {}\nError message: {}",
        name,
        uid,
        ready_state,
        environment,
        branch,
        short_sha(&commit_sha),
        commit_author.as_deref().unwrap_or("unknown"),
        if created_ms > 0 { timestamp.clone() } else { "unknown".to_string() },
        if ready_ms > 0 { ms_epoch_to_iso(ready_ms) } else { "unknown".to_string() },
        url,
        inspector_url,
        deployment.error_code.as_deref().unwrap_or("none"),
        deployment.error_message.as_deref().unwrap_or("none"),
    );

    let mut tags = vec![
        "vercel".to_string(),
        "deployment".to_string(),
        environment.clone(),
        ready_state.to_ascii_lowercase(),
    ];
    if is_failed {
        tags.push("failed-deploy".to_string());
    }
    if is_production {
        tags.push("production".to_string());
    } else {
        tags.push("preview".to_string());
    }
    if branch != "unknown" {
        tags.push(slug(&branch));
    }
    if !name.is_empty() {
        tags.push(name.clone());
    }
    tags.sort();
    tags.dedup();

    let mut refs = vec![format!("deployment:{}", uid)];
    if !url.is_empty() {
        refs.push(url.clone());
    }
    if !commit_sha.is_empty() {
        refs.push(commit_sha.clone());
    }
    if let Some(repo) = repo_full {
        refs.push(repo);
    }

    let service = slug(&name);

    let confidence = if is_failed {
        0.9
    } else if is_ready {
        0.85
    } else {
        0.75
    };

    Ok(EvidenceItem {
        id: EvidenceId::new(format!("vercel:deployment:{}:{}", project_slug, uid))?,
        kind: EvidenceKind::VercelDeployment,
        source: source.clone(),
        title: format!("Vercel deployment {} ({})", name, ready_state),
        summary,
        body,
        service: Some(service),
        files_changed: Vec::new(),
        timestamp: if timestamp.is_empty() {
            None
        } else {
            Some(timestamp)
        },
        author: commit_author,
        tags,
        refs,
        confidence,
    })
}

fn short_sha(sha: &str) -> String {
    sha.chars().take(7).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> VercelProjectRef {
        VercelProjectRef::new("my-app", None::<&str>)
    }

    fn default_request() -> VercelIngestRequest {
        VercelIngestRequest::new(project()).with_limit(10)
    }

    const FIXTURE_DEPLOYMENTS: &str = r#"{
        "deployments": [
            {
                "uid": "dpl_failed123",
                "name": "my-app",
                "projectId": "prj_abc",
                "url": "my-app-failed123.vercel.app",
                "created": 1782614400000,
                "createdAt": 1782614400000,
                "ready": 0,
                "state": "ERROR",
                "readyState": "ERROR",
                "target": "production",
                "meta": {
                    "githubCommitSha": "abcdef1234567890",
                    "githubCommitMessage": "Update checkout middleware",
                    "githubCommitAuthorName": "ada",
                    "githubCommitRef": "main",
                    "githubCommitRepoFullName": "owner/my-app"
                },
                "inspectorUrl": "https://vercel.com/owner/my-app/dpl_failed123",
                "errorCode": "BUILD_FAILED",
                "errorMessage": "Build failed"
            },
            {
                "uid": "dpl_ready456",
                "name": "my-app",
                "projectId": "prj_abc",
                "url": "my-app-ready456.vercel.app",
                "created": 1782700800000,
                "createdAt": 1782700800000,
                "ready": 1782700900000,
                "state": "READY",
                "readyState": "READY",
                "target": "production",
                "meta": {
                    "githubCommitSha": "fedcba0987654321",
                    "githubCommitMessage": "Release checkout v1.4",
                    "githubCommitAuthorName": "grace",
                    "githubCommitRef": "release",
                    "githubCommitRepoFullName": "owner/my-app"
                },
                "inspectorUrl": "https://vercel.com/owner/my-app/dpl_ready456",
                "errorCode": null,
                "errorMessage": null
            },
            {
                "uid": "dpl_preview789",
                "name": "my-app",
                "projectId": "prj_abc",
                "url": "my-app-preview789.vercel.app",
                "created": 1782787200000,
                "createdAt": 1782787200000,
                "ready": 1782787300000,
                "state": "READY",
                "readyState": "READY",
                "target": null,
                "meta": {
                    "githubCommitSha": "1122334455667788",
                    "githubCommitMessage": "Draft preview of payments",
                    "githubCommitAuthorName": "ada",
                    "githubCommitRef": "feature/payments",
                    "githubCommitRepoFullName": "owner/my-app"
                },
                "inspectorUrl": "https://vercel.com/owner/my-app/dpl_preview789"
            }
        ],
        "pagination": { "count": 3, "next": null, "prev": null }
    }"#;

    #[test]
    fn parses_deployment_fixture_into_evidence() {
        let deployments = parse_deployments(FIXTURE_DEPLOYMENTS);
        assert_eq!(deployments.len(), 3);

        let client = FixtureVercelClient::builder()
            .deployments(FIXTURE_DEPLOYMENTS)
            .build();
        let connector = VercelConnector::new(client);
        let result = connector.ingest(default_request()).unwrap();

        assert_eq!(result.deployments, 3);
        let failed = result
            .evidence
            .iter()
            .find(|item| item.refs.iter().any(|r| r == "deployment:dpl_failed123"))
            .unwrap();
        assert_eq!(failed.id.as_str(), "vercel:deployment:my-app:dpl_failed123");
        assert!(failed.summary.contains("failed"));
        assert!(failed.tags.contains(&"failed-deploy".to_string()));
        assert!(failed.tags.contains(&"production".to_string()));
        assert_eq!(failed.confidence, 0.9);
        assert!(failed.body.contains("Commit: abcdef1"));
        assert!(failed.body.contains("Error code: BUILD_FAILED"));
        assert_eq!(failed.author.as_deref(), Some("ada"));
    }

    #[test]
    fn parses_ready_production_deployment() {
        let client = FixtureVercelClient::builder()
            .deployments(FIXTURE_DEPLOYMENTS)
            .build();
        let connector = VercelConnector::new(client);
        let result = connector.ingest(default_request()).unwrap();

        let ready = result
            .evidence
            .iter()
            .find(|item| item.refs.iter().any(|r| r == "deployment:dpl_ready456"))
            .unwrap();
        assert!(ready.summary.contains("ready"));
        assert!(ready.tags.contains(&"production".to_string()));
        assert_eq!(ready.confidence, 0.85);
    }

    #[test]
    fn parses_preview_deployment() {
        let client = FixtureVercelClient::builder()
            .deployments(FIXTURE_DEPLOYMENTS)
            .build();
        let connector = VercelConnector::new(client);
        let result = connector.ingest(default_request()).unwrap();

        let preview = result
            .evidence
            .iter()
            .find(|item| item.refs.iter().any(|r| r == "deployment:dpl_preview789"))
            .unwrap();
        assert!(preview.tags.contains(&"preview".to_string()));
        assert!(!preview.tags.contains(&"production".to_string()));
        assert!(preview.tags.contains(&"feature-payments".to_string()));
    }

    #[test]
    fn vercel_evidence_ids_are_stable_across_ingests() {
        let client = FixtureVercelClient::builder()
            .deployments(FIXTURE_DEPLOYMENTS)
            .build();
        let connector = VercelConnector::new(client.clone());
        let first = connector.ingest(default_request()).unwrap();
        let connector2 = VercelConnector::new(client);
        let second = connector2.ingest(default_request()).unwrap();

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
    fn vercel_connector_deduplicates_by_id_within_one_ingest() {
        let client = FixtureVercelClient::builder()
            .deployments(FIXTURE_DEPLOYMENTS)
            .build();
        let connector = VercelConnector::new(client);
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
    fn default_ingest_selects_deployments() {
        let client = FixtureVercelClient::builder()
            .deployments(FIXTURE_DEPLOYMENTS)
            .build();
        let connector = VercelConnector::new(client);
        let request = VercelIngestRequest::new(project());
        assert!(request.no_sources_selected());
        let result = connector.ingest(request).unwrap();
        assert!(result.deployments > 0);
    }

    #[test]
    fn since_filter_keeps_only_evidence_at_or_after_cutoff() {
        let client = FixtureVercelClient::builder()
            .deployments(FIXTURE_DEPLOYMENTS)
            .build();
        let connector = VercelConnector::new(client);
        // 1751164800 == 2026-06-29T00:00:00Z (seconds); deployments use ms.
        let result = connector
            .ingest(VercelIngestRequest::new(project()).with_since("2026-06-29T00:00:00Z"))
            .unwrap();

        assert_eq!(result.deployments, 2);
        assert!(result
            .evidence
            .iter()
            .all(|item| item.id.as_str() != "vercel:deployment:my-app:dpl_failed123"));
    }

    #[test]
    fn since_filter_rejects_unparseable_values() {
        let client = FixtureVercelClient::builder()
            .deployments(FIXTURE_DEPLOYMENTS)
            .build();
        let connector = VercelConnector::new(client);
        let result = connector.ingest(VercelIngestRequest::new(project()).with_since("recently"));
        assert!(result.is_err());
    }

    #[test]
    fn missing_fixture_returns_provider_error() {
        let client = FixtureVercelClient::builder().build();
        let connector = VercelConnector::new(client);
        let result = connector.ingest(default_request());
        assert!(result.is_err());
    }

    #[test]
    fn project_ref_parses_bare_and_team_form() {
        let bare = VercelProjectRef::parse("my-app").unwrap();
        assert_eq!(bare.project, "my-app");
        assert!(bare.team.is_none());

        let scoped = VercelProjectRef::parse("my-app@my-team").unwrap();
        assert_eq!(scoped.project, "my-app");
        assert_eq!(scoped.team.as_deref(), Some("my-team"));
        assert_eq!(scoped.repository_label(), "my-app@my-team");

        assert!(VercelProjectRef::parse("").is_err());
        assert!(VercelProjectRef::parse("  ").is_err());
        assert!(VercelProjectRef::parse("@team").is_err());
        assert!(VercelProjectRef::parse("project@").is_err());
    }

    #[test]
    fn team_query_param_distinguishes_id_from_slug() {
        assert_eq!(team_query_param("team_abc"), ("teamId", "team_abc"));
        assert_eq!(team_query_param("my-team"), ("slug", "my-team"));
    }

    #[test]
    fn redact_token_scrubs_secret_from_strings() {
        let auth = VercelAuthConfig::with_token("vercel_secret_token");
        assert_eq!(
            auth.redact("error: vercel_secret_token is invalid"),
            "error: [redacted] is invalid"
        );
        assert_eq!(
            redact_token("plain text", "vercel_secret_token"),
            "plain text"
        );
        assert_eq!(redact_token("plain text", ""), "plain text");
    }

    #[test]
    fn auth_from_env_is_empty_when_unset() {
        std::env::remove_var("VERCEL_TOKEN");
        let auth = VercelAuthConfig::from_env();
        assert!(!auth.has_token());
    }

    #[test]
    fn http_client_request_config_uses_only_get_and_carries_token_via_stdin() {
        let auth = VercelAuthConfig::with_token("vercel_secret_token");
        let client = HttpVercelClient::new(auth);
        let config = client.request_config("/v7/deployments?limit=20&projectId=my-app");

        assert!(config.contains("request = \"GET\""));
        assert!(!config.contains("\"POST\""));
        assert!(!config.contains("\"PUT\""));
        assert!(!config.contains("\"PATCH\""));
        assert!(!config.contains("\"DELETE\""));
        assert!(config.contains("vercel_secret_token"));
        assert!(!client.auth.redact(&config).contains("vercel_secret_token"));
    }

    #[test]
    fn vercel_connector_exposes_no_mutation_http_methods() {
        for method in vercel_forbidden_http_methods() {
            assert!(!vercel_allowed_http_methods().contains(method));
        }
        assert_eq!(vercel_allowed_http_methods(), &["GET"]);
    }

    #[test]
    fn vercel_evidence_never_embeds_auth_token() {
        let fixture = r#"{
            "deployments": [
                {
                    "uid": "dpl_leak",
                    "name": "my-app",
                    "projectId": "prj_abc",
                    "url": "my-app-leak.vercel.app",
                    "created": 1751078400000,
                    "createdAt": 1751078400000,
                    "ready": 0,
                    "state": "READY",
                    "readyState": "READY",
                    "target": "production",
                    "meta": {
                        "githubCommitSha": "abcdef1234567890",
                        "githubCommitMessage": "service:checkout vercel_should_not_leak",
                        "githubCommitAuthorName": "ada",
                        "githubCommitRef": "main"
                    }
                }
            ],
            "pagination": { "count": 1, "next": null, "prev": null }
        }"#;
        let client = FixtureVercelClient::builder().deployments(fixture).build();
        let connector = VercelConnector::new(client);
        let result = connector.ingest(default_request()).unwrap();

        for item in &result.evidence {
            assert!(
                !item.body.contains("vercel_should_not_leak"),
                "{}",
                item.body
            );
            assert!(!item.summary.contains("vercel_should_not_leak"));
            assert!(!item
                .refs
                .iter()
                .any(|r| r.contains("vercel_should_not_leak")));
        }
    }

    #[test]
    fn vercel_evidence_items_are_marked_vercel() {
        let client = FixtureVercelClient::builder()
            .deployments(FIXTURE_DEPLOYMENTS)
            .build();
        let connector = VercelConnector::new(client);
        let result = connector.ingest(default_request()).unwrap();

        for item in &result.evidence {
            assert!(item.is_vercel());
            assert_eq!(item.source.connector, VERCEL_CONNECTOR);
            assert!(item.source.read_only);
        }
    }

    #[test]
    fn ms_epoch_to_iso_round_trips_through_timestamp_parser() {
        // 1751078400000 ms == 2025-06-28T02:40:00Z
        let iso = ms_epoch_to_iso(1_751_078_400_000);
        assert_eq!(iso, "2025-06-28T02:40:00Z");
        let epoch_seconds = parse_vercel_timestamp(&iso).unwrap();
        assert_eq!(epoch_seconds, 1_751_078_400);
    }

    #[test]
    fn connector_version_is_shared() {
        let source = EvidenceSource::vercel("my-app");
        assert_eq!(source.version, crate::CONNECTOR_VERSION);
    }
}
