//! Read-only Cloudflare evidence connector.
//!
//! Feeds Cloudflare Pages and Workers deployment evidence into Rivora's
//! evidence store. The connector is strictly read-only: it only issues `GET`
//! requests against the Cloudflare REST API and never calls mutation endpoints
//! (`POST`, `PUT`, `PATCH`, `DELETE`). It never creates, promotes, rolls back,
//! or deletes deployments, and it never reads or writes environment variables,
//! secrets, KV, R2, D1, or Queues.
//!
//! Authentication requires a `CLOUDFLARE_API_TOKEN` environment variable
//! (alternatively `CF_API_TOKEN`). Tokens are never stored in `.rivora/`,
//! never printed, and never written into evidence bodies, logs, or receipts.
//! Error messages are redacted so tokens cannot leak through `curl` stderr.

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use rivora_errors::{Result, RivoraError};
use serde::Deserialize;

use crate::{slug, EvidenceId, EvidenceItem, EvidenceKind, EvidenceSource};

/// Connector name written into [`EvidenceSource::connector`].
pub const CLOUDFLARE_CONNECTOR: &str = "cloudflare";
/// Default Cloudflare REST API base URL.
pub const CLOUDFLARE_API_BASE: &str = "https://api.cloudflare.com/client/v4";

/// HTTP methods the Cloudflare connector is allowed to use.
#[must_use]
pub fn cloudflare_allowed_http_methods() -> &'static [&'static str] {
    &["GET"]
}

/// HTTP methods the Cloudflare connector must never use.
#[must_use]
pub fn cloudflare_forbidden_http_methods() -> &'static [&'static str] {
    &["POST", "PUT", "PATCH", "DELETE"]
}

/// Replace any occurrence of `token` in `value` with `[redacted]`.
#[must_use]
fn redact_token(value: &str, token: &str) -> String {
    if token.is_empty() {
        value.to_string()
    } else {
        value.replace(token, "[redacted]")
    }
}

/// Cloudflare authentication configuration.
///
/// The token is held privately and never exposed through a getter.
#[derive(Debug, Clone)]
pub struct CloudflareAuthConfig {
    token: Option<String>,
}

impl CloudflareAuthConfig {
    /// Read `CLOUDFLARE_API_TOKEN` (preferred) or `CF_API_TOKEN` from the
    /// environment.
    #[must_use]
    pub fn from_env() -> Self {
        let token = std::env::var("CLOUDFLARE_API_TOKEN")
            .ok()
            .filter(|token| !token.trim().is_empty())
            .or_else(|| {
                std::env::var("CF_API_TOKEN")
                    .ok()
                    .filter(|token| !token.trim().is_empty())
            });
        Self { token }
    }

    /// Explicit token. Primarily useful for tests.
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

/// Which Cloudflare platform to ingest deployments from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudflarePlatform {
    Pages,
    Workers,
}

impl CloudflarePlatform {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pages => "pages",
            Self::Workers => "workers",
        }
    }
}

/// A Cloudflare account + resource reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudflareTarget {
    pub account_id: String,
    pub platform: CloudflarePlatform,
    pub resource: String,
}

impl CloudflareTarget {
    #[must_use]
    pub fn pages(account_id: impl Into<String>, project_name: impl Into<String>) -> Self {
        Self {
            account_id: account_id.into(),
            platform: CloudflarePlatform::Pages,
            resource: project_name.into(),
        }
    }

    #[must_use]
    pub fn worker(account_id: impl Into<String>, script_name: impl Into<String>) -> Self {
        Self {
            account_id: account_id.into(),
            platform: CloudflarePlatform::Workers,
            resource: script_name.into(),
        }
    }

    #[must_use]
    pub fn repository_label(&self) -> String {
        format!(
            "{}/{}:{}",
            self.account_id,
            self.platform.as_str(),
            self.resource
        )
    }
}

/// Read-only Cloudflare API client contract.
pub trait CloudflareClient {
    fn fetch_pages_deployments(&self, target: &CloudflareTarget, limit: usize) -> Result<String>;

    fn fetch_worker_deployments(&self, target: &CloudflareTarget, limit: usize) -> Result<String>;
}

/// Real Cloudflare REST API client backed by `curl`.
#[derive(Debug, Clone)]
pub struct HttpCloudflareClient {
    auth: CloudflareAuthConfig,
    base_url: String,
}

impl HttpCloudflareClient {
    #[must_use]
    pub fn new(auth: CloudflareAuthConfig) -> Self {
        Self {
            auth,
            base_url: CLOUDFLARE_API_BASE.to_string(),
        }
    }

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
            .map_err(|e| RivoraError::provider("cloudflare", format!("curl unavailable: {e}")))?;
        {
            let mut stdin = child.stdin.take().ok_or_else(|| {
                RivoraError::provider("cloudflare", "could not open curl stdin pipe")
            })?;
            stdin.write_all(config.as_bytes()).map_err(|e| {
                RivoraError::provider("cloudflare", format!("curl config write failed: {e}"))
            })?;
        }
        let output = child.wait_with_output().map_err(|e| {
            RivoraError::provider("cloudflare", format!("curl did not finish: {e}"))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let redacted = self.auth.redact(stderr.as_ref());
            return Err(RivoraError::provider(
                "cloudflare",
                format!(
                    "Cloudflare API request failed for {}: {}",
                    path,
                    redacted.trim()
                ),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

impl CloudflareClient for HttpCloudflareClient {
    fn fetch_pages_deployments(&self, target: &CloudflareTarget, limit: usize) -> Result<String> {
        let path = format!(
            "/accounts/{}/pages/projects/{}/deployments?per_page={}",
            target.account_id,
            target.resource,
            clamp_limit(limit),
        );
        self.get(&path)
    }

    fn fetch_worker_deployments(&self, target: &CloudflareTarget, limit: usize) -> Result<String> {
        let path = format!(
            "/accounts/{}/workers/scripts/{}/deployments?per_page={}",
            target.account_id,
            target.resource,
            clamp_limit(limit),
        );
        self.get(&path)
    }
}

/// Test double for [`CloudflareClient`].
#[derive(Debug, Clone, Default)]
pub struct FixtureCloudflareClient {
    pages_deployments: Option<String>,
    worker_deployments: Option<String>,
}

impl FixtureCloudflareClient {
    #[must_use]
    pub fn builder() -> FixtureCloudflareClientBuilder {
        FixtureCloudflareClientBuilder::default()
    }
}

impl CloudflareClient for FixtureCloudflareClient {
    fn fetch_pages_deployments(&self, _target: &CloudflareTarget, _limit: usize) -> Result<String> {
        self.pages_deployments.clone().ok_or_else(|| {
            RivoraError::provider("cloudflare", "no fixture loaded for pages deployments")
        })
    }

    fn fetch_worker_deployments(
        &self,
        _target: &CloudflareTarget,
        _limit: usize,
    ) -> Result<String> {
        self.worker_deployments.clone().ok_or_else(|| {
            RivoraError::provider("cloudflare", "no fixture loaded for worker deployments")
        })
    }
}

#[derive(Debug, Default, Clone)]
pub struct FixtureCloudflareClientBuilder {
    pages_deployments: Option<String>,
    worker_deployments: Option<String>,
}

impl FixtureCloudflareClientBuilder {
    #[must_use]
    pub fn pages_deployments(mut self, fixture: impl Into<String>) -> Self {
        self.pages_deployments = Some(fixture.into());
        self
    }

    #[must_use]
    pub fn worker_deployments(mut self, fixture: impl Into<String>) -> Self {
        self.worker_deployments = Some(fixture.into());
        self
    }

    #[must_use]
    pub fn build(self) -> FixtureCloudflareClient {
        FixtureCloudflareClient {
            pages_deployments: self.pages_deployments,
            worker_deployments: self.worker_deployments,
        }
    }
}

fn clamp_limit(limit: usize) -> usize {
    limit.clamp(1, 100)
}

/// Request for Cloudflare evidence ingestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudflareIngestRequest {
    pub target: CloudflareTarget,
    pub limit: usize,
    pub since: Option<String>,
}

impl CloudflareIngestRequest {
    #[must_use]
    pub fn new(target: CloudflareTarget) -> Self {
        Self {
            target,
            limit: 20,
            since: None,
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
}

/// Result of Cloudflare evidence ingestion.
#[derive(Debug, Clone, PartialEq)]
pub struct CloudflareIngestResult {
    pub repository: String,
    pub platform: CloudflarePlatform,
    pub evidence: Vec<EvidenceItem>,
    pub deployments: usize,
    pub topics: Vec<String>,
}

/// Read-only Cloudflare connector.
pub struct CloudflareConnector {
    client: Box<dyn CloudflareClient>,
}

impl std::fmt::Debug for CloudflareConnector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudflareConnector")
            .finish_non_exhaustive()
    }
}

impl CloudflareConnector {
    #[must_use]
    pub fn new(client: impl CloudflareClient + 'static) -> Self {
        Self {
            client: Box::new(client),
        }
    }

    pub fn ingest(&self, request: CloudflareIngestRequest) -> Result<CloudflareIngestResult> {
        if request.limit == 0 {
            return Err(RivoraError::invalid_value(
                "limit",
                "limit must be positive",
            ));
        }

        let target = request.target.clone();
        let repository = target.repository_label();
        let source = EvidenceSource::cloudflare(repository.clone());
        let limit = request.limit;
        let platform = target.platform.clone();

        let mut evidence = Vec::new();
        let since_cutoff = request
            .since
            .as_deref()
            .map(parse_since_cutoff)
            .transpose()?;
        let mut topics = std::collections::BTreeSet::new();
        let mut deployments;

        match platform {
            CloudflarePlatform::Pages => {
                let raw = self.client.fetch_pages_deployments(&target, limit)?;
                let parsed = parse_pages_deployments(&raw);
                deployments = parsed.len();
                for deployment in parsed {
                    let item = pages_deployment_item(&source, &target, &deployment)?;
                    collect_topics(&item, &mut topics);
                    evidence.push(item);
                }
            }
            CloudflarePlatform::Workers => {
                let raw = self.client.fetch_worker_deployments(&target, limit)?;
                let parsed = parse_worker_deployments(&raw);
                deployments = parsed.len();
                for deployment in parsed {
                    let item = worker_deployment_item(&source, &target, &deployment)?;
                    collect_topics(&item, &mut topics);
                    evidence.push(item);
                }
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
            deployments = evidence.iter().filter(|item| item.is_cloudflare()).count();
        }

        Ok(CloudflareIngestResult {
            repository,
            platform,
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
        .and_then(parse_cloudflare_timestamp)
        .is_none_or(|timestamp| timestamp >= cutoff)
}

fn parse_since_cutoff(value: &str) -> Result<i64> {
    let trimmed = value.trim();
    if let Some(days) = trimmed.strip_suffix('d') {
        let days = days.parse::<i64>().map_err(|_| {
            RivoraError::invalid_value(
                "cloudflare_since",
                "use an ISO timestamp or relative days like 7d",
            )
        })?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| {
                RivoraError::invalid_value("cloudflare_since", "system clock is before unix epoch")
            })?
            .as_secs() as i64;
        return Ok(now - days.saturating_mul(86_400));
    }
    parse_cloudflare_timestamp(trimmed).ok_or_else(|| {
        RivoraError::invalid_value(
            "cloudflare_since",
            "use an ISO timestamp or relative days like 7d",
        )
    })
}

fn parse_cloudflare_timestamp(value: &str) -> Option<i64> {
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

fn iso_epoch_to_iso(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.contains('T') {
        return Some(trimmed.to_string());
    }
    let ms = trimmed.parse::<u64>().ok()?;
    Some(ms_epoch_to_iso(ms))
}

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

// --- Cloudflare API response shapes -----------------------------------------

#[derive(Debug, Clone, Deserialize)]
struct PagesDeploymentsEnvelope {
    result: Option<Vec<PagesDeploymentResponse>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct PagesDeploymentResponse {
    id: Option<String>,
    project_name: Option<String>,
    environment: Option<String>,
    deployment_trigger: Option<PagesDeploymentTrigger>,
    url: Option<String>,
    aliases: Option<Vec<String>>,
    stage: Option<String>,
    status: Option<String>,
    created_on: Option<String>,
    modified_on: Option<String>,
    latest_stage: Option<PagesLatestStage>,
    branches_deployed: Option<Vec<String>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct PagesDeploymentTrigger {
    r#type: Option<String>,
    metadata: Option<PagesTriggerMetadata>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct PagesTriggerMetadata {
    branch: Option<String>,
    commit_hash: Option<String>,
    commit_message: Option<String>,
    commit_author: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct PagesLatestStage {
    name: Option<String>,
    status: Option<String>,
    started_on: Option<String>,
    ended_on: Option<String>,
    failure_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct WorkersDeploymentsEnvelope {
    result: Option<Vec<WorkersDeploymentResponse>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct WorkersDeploymentResponse {
    id: Option<String>,
    script_name: Option<String>,
    annotations: Option<serde_json::Map<String, serde_json::Value>>,
    source: Option<String>,
    strategy: Option<String>,
    versions: Option<Vec<WorkersVersionRef>>,
    author_id: Option<String>,
    author_email: Option<String>,
    created_on: Option<String>,
    modified_on: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct WorkersVersionRef {
    version_id: Option<String>,
    percentage: Option<f64>,
}

#[must_use]
fn parse_pages_deployments(raw: &str) -> Vec<PagesDeploymentResponse> {
    serde_json::from_str::<PagesDeploymentsEnvelope>(raw)
        .ok()
        .and_then(|envelope| envelope.result)
        .unwrap_or_default()
}

#[must_use]
fn parse_worker_deployments(raw: &str) -> Vec<WorkersDeploymentResponse> {
    serde_json::from_str::<WorkersDeploymentsEnvelope>(raw)
        .ok()
        .and_then(|envelope| envelope.result)
        .unwrap_or_default()
}

// --- Evidence mapping -------------------------------------------------------

fn pages_deployment_item(
    source: &EvidenceSource,
    target: &CloudflareTarget,
    deployment: &PagesDeploymentResponse,
) -> Result<EvidenceItem> {
    let deployment_id = deployment.id.clone().unwrap_or_default();
    let project_name = deployment
        .project_name
        .clone()
        .or(Some(target.resource.clone()))
        .unwrap_or_else(|| "unknown".to_string());
    let project_slug = slug(&project_name);
    let environment = deployment
        .environment
        .clone()
        .filter(|env| !env.is_empty())
        .map(|env| env.to_ascii_lowercase())
        .unwrap_or_else(|| "preview".to_string());
    let is_production = environment == "production";
    let status = deployment
        .stage
        .clone()
        .or_else(|| deployment.status.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let is_failed = matches!(
        status.to_ascii_lowercase().as_str(),
        "failure" | "failed" | "canceled" | "cancelled"
    );

    let branch = deployment
        .deployment_trigger
        .as_ref()
        .and_then(|trigger| trigger.metadata.as_ref())
        .and_then(|meta| meta.branch.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let commit_sha = deployment
        .deployment_trigger
        .as_ref()
        .and_then(|trigger| trigger.metadata.as_ref())
        .and_then(|meta| meta.commit_hash.clone())
        .unwrap_or_default();
    let commit_author = deployment
        .deployment_trigger
        .as_ref()
        .and_then(|trigger| trigger.metadata.as_ref())
        .and_then(|meta| meta.commit_author.clone());

    let url = deployment.url.clone().unwrap_or_default();
    let aliases = deployment.aliases.clone().unwrap_or_default();

    let timestamp = deployment.created_on.as_deref().and_then(iso_epoch_to_iso);
    let modified = deployment.modified_on.as_deref().and_then(iso_epoch_to_iso);

    let failure_reason = deployment
        .latest_stage
        .as_ref()
        .and_then(|stage| stage.failure_code.clone())
        .unwrap_or_default();

    let summary = if is_failed {
        format!(
            "Cloudflare Pages deployment for {} failed ({})",
            project_name, status
        )
    } else {
        format!(
            "Cloudflare Pages deployment for {} ({})",
            project_name, environment
        )
    };

    let body = format!(
        "Project: {}\nDeployment: {}\nEnvironment: {}\nStage: {}\nBranch: {}\nCommit: {}\nAuthor: {}\nCreated: {}\nModified: {}\nURL: {}\nAliases: {}\nFailure: {}",
        project_name,
        deployment_id,
        environment,
        status,
        branch,
        short_sha(&commit_sha),
        commit_author.as_deref().unwrap_or("unknown"),
        timestamp.as_deref().unwrap_or("unknown"),
        modified.as_deref().unwrap_or("unknown"),
        url,
        if aliases.is_empty() { "none".to_string() } else { aliases.join(", ") },
        if failure_reason.is_empty() { "none".to_string() } else { failure_reason },
    );

    let mut tags = vec![
        "cloudflare".to_string(),
        "pages".to_string(),
        "deployment".to_string(),
    ];
    if is_failed {
        tags.push("failed-deploy".to_string());
    }
    if is_production {
        tags.push("production".to_string());
    } else {
        tags.push("preview".to_string());
    }
    tags.push(slug(&project_name));
    if branch != "unknown" {
        tags.push(slug(&branch));
    }
    tags.sort();
    tags.dedup();

    let mut refs = vec![format!("deployment:{}", deployment_id)];
    if !url.is_empty() {
        refs.push(url);
    }
    for alias in &aliases {
        if !alias.is_empty() {
            refs.push(alias.clone());
        }
    }
    if !commit_sha.is_empty() {
        refs.push(commit_sha);
    }

    let service = slug(&project_name);
    let confidence = if is_failed {
        0.9
    } else if status.eq_ignore_ascii_case("success") {
        0.85
    } else {
        0.75
    };

    Ok(EvidenceItem {
        id: EvidenceId::new(format!(
            "cloudflare:pages-deployment:{}:{}",
            project_slug, deployment_id
        ))?,
        kind: EvidenceKind::CloudflarePagesDeployment,
        source: source.clone(),
        title: format!("Cloudflare Pages deployment {} ({})", project_name, status),
        summary,
        body,
        service: Some(service),
        files_changed: Vec::new(),
        timestamp,
        author: commit_author,
        tags,
        refs,
        confidence,
    })
}

fn worker_deployment_item(
    source: &EvidenceSource,
    target: &CloudflareTarget,
    deployment: &WorkersDeploymentResponse,
) -> Result<EvidenceItem> {
    let deployment_id = deployment.id.clone().unwrap_or_default();
    let script_name = deployment
        .script_name
        .clone()
        .or(Some(target.resource.clone()))
        .unwrap_or_else(|| "unknown".to_string());
    let script_slug = slug(&script_name);

    let status = deployment
        .strategy
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let deploy_source = deployment
        .source
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let author = deployment
        .author_email
        .clone()
        .or_else(|| deployment.author_id.clone());

    let timestamp = deployment.created_on.as_deref().and_then(iso_epoch_to_iso);
    let modified = deployment.modified_on.as_deref().and_then(iso_epoch_to_iso);

    let annotation_message = deployment
        .annotations
        .as_ref()
        .and_then(|map| map.get("message"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let version_ids: Vec<String> = deployment
        .versions
        .as_ref()
        .unwrap_or(&Vec::new())
        .iter()
        .filter_map(|v| v.version_id.clone())
        .collect();

    let summary = format!(
        "Cloudflare Worker deployment for {} ({})",
        script_name, status
    );

    let body = format!(
        "Script: {}\nDeployment: {}\nSource: {}\nStrategy: {}\nAuthor: {}\nCreated: {}\nModified: {}\nVersions: {}\nAnnotation: {}",
        script_name,
        deployment_id,
        deploy_source,
        status,
        author.as_deref().unwrap_or("unknown"),
        timestamp.as_deref().unwrap_or("unknown"),
        modified.as_deref().unwrap_or("unknown"),
        if version_ids.is_empty() { "none".to_string() } else { version_ids.join(", ") },
        annotation_message.as_deref().unwrap_or("none"),
    );

    let mut tags = vec![
        "cloudflare".to_string(),
        "workers".to_string(),
        "worker".to_string(),
        "deployment".to_string(),
    ];
    tags.push(slug(&script_name));
    tags.sort();
    tags.dedup();

    let mut refs = vec![format!("deployment:{}", deployment_id)];
    for version_id in &version_ids {
        refs.push(format!("version:{}", version_id));
    }

    let service = slug(&script_name);
    let confidence = 0.8;

    Ok(EvidenceItem {
        id: EvidenceId::new(format!(
            "cloudflare:worker-deployment:{}:{}",
            script_slug, deployment_id
        ))?,
        kind: EvidenceKind::CloudflareWorkerDeployment,
        source: source.clone(),
        title: format!("Cloudflare Worker deployment {} ({})", script_name, status),
        summary,
        body,
        service: Some(service),
        files_changed: Vec::new(),
        timestamp,
        author,
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

    fn pages_target() -> CloudflareTarget {
        CloudflareTarget::pages("acc_123", "my-pages-app")
    }

    fn worker_target() -> CloudflareTarget {
        CloudflareTarget::worker("acc_123", "my-worker")
    }

    fn default_pages_request() -> CloudflareIngestRequest {
        CloudflareIngestRequest::new(pages_target()).with_limit(10)
    }

    fn default_worker_request() -> CloudflareIngestRequest {
        CloudflareIngestRequest::new(worker_target()).with_limit(10)
    }

    const FIXTURE_PAGES_DEPLOYMENTS: &str = r#"{
        "success": true,
        "errors": [],
        "result": [
            {
                "id": "dpl_pages_failed123",
                "project_name": "my-pages-app",
                "environment": "production",
                "deployment_trigger": {
                    "type": "github",
                    "metadata": {
                        "branch": "main",
                        "commit_hash": "abcdef1234567890",
                        "commit_message": "Update checkout middleware",
                        "commit_author": "ada"
                    }
                },
                "url": "https://my-pages-app.pages.dev",
                "aliases": ["my-pages-app.prod.pages.dev"],
                "stage": "failure",
                "status": "failure",
                "created_on": "2026-06-28T10:00:00Z",
                "modified_on": "2026-06-28T10:05:00Z",
                "latest_stage": {
                    "name": "deploy",
                    "status": "failure",
                    "started_on": "2026-06-28T10:00:00Z",
                    "ended_on": "2026-06-28T10:05:00Z",
                    "failure_code": "BUILD_FAILED"
                },
                "branches_deployed": ["main"]
            },
            {
                "id": "dpl_pages_ready456",
                "project_name": "my-pages-app",
                "environment": "production",
                "deployment_trigger": {
                    "type": "github",
                    "metadata": {
                        "branch": "release",
                        "commit_hash": "fedcba0987654321",
                        "commit_message": "Release checkout v1.4",
                        "commit_author": "grace"
                    }
                },
                "url": "https://my-pages-app.pages.dev",
                "aliases": [],
                "stage": "success",
                "status": "success",
                "created_on": "2026-06-29T10:00:00Z",
                "modified_on": "2026-06-29T10:02:00Z",
                "latest_stage": {
                    "name": "deploy",
                    "status": "success",
                    "started_on": "2026-06-29T10:00:00Z",
                    "ended_on": "2026-06-29T10:02:00Z"
                },
                "branches_deployed": ["release"]
            },
            {
                "id": "dpl_pages_preview789",
                "project_name": "my-pages-app",
                "environment": "preview",
                "deployment_trigger": {
                    "type": "github",
                    "metadata": {
                        "branch": "feature/payments",
                        "commit_hash": "1122334455667788",
                        "commit_message": "Draft preview of payments",
                        "commit_author": "ada"
                    }
                },
                "url": "https://feature-payments.my-pages-app.pages.dev",
                "aliases": [],
                "stage": "success",
                "status": "success",
                "created_on": "2026-06-29T14:00:00Z",
                "modified_on": "2026-06-29T14:01:00Z",
                "latest_stage": {
                    "name": "deploy",
                    "status": "success",
                    "started_on": "2026-06-29T14:00:00Z",
                    "ended_on": "2026-06-29T14:01:00Z"
                },
                "branches_deployed": ["feature/payments"]
            }
        ]
    }"#;

    const FIXTURE_WORKER_DEPLOYMENTS: &str = r#"{
        "success": true,
        "errors": [],
        "result": [
            {
                "id": "dpl_worker_001",
                "script_name": "my-worker",
                "source": "api",
                "strategy": "percentage",
                "versions": [
                    { "version_id": "v_abc123", "percentage": 100.0 }
                ],
                "author_id": "user_001",
                "author_email": "ada@example.com",
                "created_on": "2026-06-28T12:00:00Z",
                "modified_on": "2026-06-28T12:00:00Z",
                "annotations": {
                    "message": "Deploy worker v2.1"
                }
            },
            {
                "id": "dpl_worker_002",
                "script_name": "my-worker",
                "source": "ui",
                "strategy": "sequential",
                "versions": [
                    { "version_id": "v_def456", "percentage": 100.0 }
                ],
                "author_id": "user_002",
                "author_email": "grace@example.com",
                "created_on": "2026-06-29T08:00:00Z",
                "modified_on": "2026-06-29T08:00:00Z",
                "annotations": {}
            }
        ]
    }"#;

    #[test]
    fn parses_pages_deployment_fixture_into_evidence() {
        let client = FixtureCloudflareClient::builder()
            .pages_deployments(FIXTURE_PAGES_DEPLOYMENTS)
            .build();
        let connector = CloudflareConnector::new(client);
        let result = connector.ingest(default_pages_request()).unwrap();

        assert_eq!(result.deployments, 3);
        assert_eq!(result.platform, CloudflarePlatform::Pages);

        let failed = result
            .evidence
            .iter()
            .find(|item| {
                item.refs
                    .iter()
                    .any(|r| r == "deployment:dpl_pages_failed123")
            })
            .unwrap();
        assert_eq!(
            failed.id.as_str(),
            "cloudflare:pages-deployment:my-pages-app:dpl_pages_failed123"
        );
        assert!(failed.summary.contains("failed"));
        assert!(failed.tags.contains(&"failed-deploy".to_string()));
        assert!(failed.tags.contains(&"production".to_string()));
        assert!(failed.tags.contains(&"cloudflare".to_string()));
        assert!(failed.tags.contains(&"pages".to_string()));
        assert_eq!(failed.confidence, 0.9);
        assert!(failed.body.contains("Commit: abcdef1"));
        assert!(failed.body.contains("Failure: BUILD_FAILED"));
        assert_eq!(failed.author.as_deref(), Some("ada"));
    }

    #[test]
    fn parses_ready_production_pages_deployment() {
        let client = FixtureCloudflareClient::builder()
            .pages_deployments(FIXTURE_PAGES_DEPLOYMENTS)
            .build();
        let connector = CloudflareConnector::new(client);
        let result = connector.ingest(default_pages_request()).unwrap();

        let ready = result
            .evidence
            .iter()
            .find(|item| {
                item.refs
                    .iter()
                    .any(|r| r == "deployment:dpl_pages_ready456")
            })
            .unwrap();
        assert!(ready.summary.contains("production"));
        assert!(ready.tags.contains(&"production".to_string()));
        assert_eq!(ready.confidence, 0.85);
    }

    #[test]
    fn parses_preview_pages_deployment() {
        let client = FixtureCloudflareClient::builder()
            .pages_deployments(FIXTURE_PAGES_DEPLOYMENTS)
            .build();
        let connector = CloudflareConnector::new(client);
        let result = connector.ingest(default_pages_request()).unwrap();

        let preview = result
            .evidence
            .iter()
            .find(|item| {
                item.refs
                    .iter()
                    .any(|r| r == "deployment:dpl_pages_preview789")
            })
            .unwrap();
        assert!(preview.tags.contains(&"preview".to_string()));
        assert!(!preview.tags.contains(&"production".to_string()));
        assert!(preview.tags.contains(&"feature-payments".to_string()));
    }

    #[test]
    fn parses_worker_deployment_fixture_into_evidence() {
        let client = FixtureCloudflareClient::builder()
            .worker_deployments(FIXTURE_WORKER_DEPLOYMENTS)
            .build();
        let connector = CloudflareConnector::new(client);
        let result = connector.ingest(default_worker_request()).unwrap();

        assert_eq!(result.deployments, 2);
        assert_eq!(result.platform, CloudflarePlatform::Workers);

        let first = result
            .evidence
            .iter()
            .find(|item| item.refs.iter().any(|r| r == "deployment:dpl_worker_001"))
            .unwrap();
        assert_eq!(
            first.id.as_str(),
            "cloudflare:worker-deployment:my-worker:dpl_worker_001"
        );
        assert!(first.tags.contains(&"cloudflare".to_string()));
        assert!(first.tags.contains(&"workers".to_string()));
        assert!(first.tags.contains(&"worker".to_string()));
        assert!(first.tags.contains(&"deployment".to_string()));
        assert!(first.tags.contains(&"my-worker".to_string()));
        assert_eq!(first.author.as_deref(), Some("ada@example.com"));
        assert!(first.body.contains("Deploy worker v2.1"));
        assert!(first.refs.iter().any(|r| r == "version:v_abc123"));
    }

    #[test]
    fn cloudflare_evidence_ids_are_stable_across_ingests() {
        let client = FixtureCloudflareClient::builder()
            .pages_deployments(FIXTURE_PAGES_DEPLOYMENTS)
            .build();
        let first = CloudflareConnector::new(client.clone())
            .ingest(default_pages_request())
            .unwrap();
        let second = CloudflareConnector::new(client)
            .ingest(default_pages_request())
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
    fn cloudflare_connector_deduplicates_by_id_within_one_ingest() {
        let client = FixtureCloudflareClient::builder()
            .pages_deployments(FIXTURE_PAGES_DEPLOYMENTS)
            .build();
        let connector = CloudflareConnector::new(client);
        let result = connector.ingest(default_pages_request()).unwrap();
        let mut ids: Vec<_> = result
            .evidence
            .iter()
            .map(|item| item.id.as_str().to_string())
            .collect();
        ids.sort();
        let mut deduped = ids.clone();
        deduped.dedup();
        assert_eq!(ids, deduped);
    }

    #[test]
    fn since_filter_keeps_only_evidence_at_or_after_cutoff() {
        let client = FixtureCloudflareClient::builder()
            .pages_deployments(FIXTURE_PAGES_DEPLOYMENTS)
            .build();
        let connector = CloudflareConnector::new(client);
        let result = connector
            .ingest(CloudflareIngestRequest::new(pages_target()).with_since("2026-06-29T00:00:00Z"))
            .unwrap();

        assert_eq!(result.deployments, 2);
        assert!(result.evidence.iter().all(|item| item.id.as_str()
            != "cloudflare:pages-deployment:my-pages-app:dpl_pages_failed123"));
    }

    #[test]
    fn since_filter_rejects_unparseable_values() {
        let client = FixtureCloudflareClient::builder()
            .pages_deployments(FIXTURE_PAGES_DEPLOYMENTS)
            .build();
        let connector = CloudflareConnector::new(client);
        let result =
            connector.ingest(CloudflareIngestRequest::new(pages_target()).with_since("recently"));
        assert!(result.is_err());
    }

    #[test]
    fn missing_fixture_returns_provider_error() {
        let client = FixtureCloudflareClient::builder().build();
        let connector = CloudflareConnector::new(client);
        let result = connector.ingest(default_pages_request());
        assert!(result.is_err());
    }

    #[test]
    fn redact_token_scrubs_secret_from_strings() {
        let auth = CloudflareAuthConfig::with_token("cf_secret_token");
        assert_eq!(
            auth.redact("error: cf_secret_token is invalid"),
            "error: [redacted] is invalid"
        );
        assert_eq!(redact_token("plain text", "cf_secret_token"), "plain text");
        assert_eq!(redact_token("plain text", ""), "plain text");
    }

    #[test]
    fn auth_env_var_behavior() {
        std::env::remove_var("CLOUDFLARE_API_TOKEN");
        std::env::remove_var("CF_API_TOKEN");
        let auth_empty = CloudflareAuthConfig::from_env();
        assert!(!auth_empty.has_token());

        std::env::set_var("CF_API_TOKEN", "cf_fallback");
        let auth_fallback = CloudflareAuthConfig::from_env();
        assert!(auth_fallback.has_token());

        std::env::set_var("CLOUDFLARE_API_TOKEN", "cf_primary");
        let auth_preferred = CloudflareAuthConfig::from_env();
        assert!(auth_preferred.has_token());
        assert_eq!(auth_preferred.redact("cf_primary"), "[redacted]");

        std::env::remove_var("CLOUDFLARE_API_TOKEN");
        std::env::remove_var("CF_API_TOKEN");
    }

    #[test]
    fn http_client_request_config_uses_only_get_and_carries_token_via_stdin() {
        let auth = CloudflareAuthConfig::with_token("cf_secret_token");
        let client = HttpCloudflareClient::new(auth);
        let config = client
            .request_config("/accounts/acc_123/pages/projects/my-app/deployments?per_page=20");

        assert!(config.contains("request = \"GET\""));
        assert!(!config.contains("\"POST\""));
        assert!(!config.contains("\"PUT\""));
        assert!(!config.contains("\"PATCH\""));
        assert!(!config.contains("\"DELETE\""));
        assert!(config.contains("cf_secret_token"));
        assert!(!client.auth.redact(&config).contains("cf_secret_token"));
    }

    #[test]
    fn cloudflare_connector_exposes_no_mutation_http_methods() {
        for method in cloudflare_forbidden_http_methods() {
            assert!(!cloudflare_allowed_http_methods().contains(method));
        }
        assert_eq!(cloudflare_allowed_http_methods(), &["GET"]);
    }

    #[test]
    fn cloudflare_evidence_never_embeds_auth_token() {
        let fixture = r#"{
            "success": true,
            "errors": [],
            "result": [
                {
                    "id": "dpl_leak",
                    "project_name": "my-pages-app",
                    "environment": "production",
                    "deployment_trigger": {
                        "type": "github",
                        "metadata": {
                            "branch": "main",
                            "commit_hash": "abcdef1234567890",
                            "commit_message": "service:checkout cf_should_not_leak",
                            "commit_author": "ada"
                        }
                    },
                    "url": "https://my-pages-app.pages.dev",
                    "aliases": [],
                    "stage": "success",
                    "status": "success",
                    "created_on": "2026-06-28T10:00:00Z",
                    "modified_on": "2026-06-28T10:05:00Z"
                }
            ]
        }"#;
        let client = FixtureCloudflareClient::builder()
            .pages_deployments(fixture)
            .build();
        let connector = CloudflareConnector::new(client);
        let result = connector.ingest(default_pages_request()).unwrap();

        for item in &result.evidence {
            assert!(!item.body.contains("cf_should_not_leak"), "{}", item.body);
            assert!(!item.summary.contains("cf_should_not_leak"));
            assert!(!item.refs.iter().any(|r| r.contains("cf_should_not_leak")));
        }
    }

    #[test]
    fn cloudflare_pages_evidence_items_are_marked_cloudflare() {
        let client = FixtureCloudflareClient::builder()
            .pages_deployments(FIXTURE_PAGES_DEPLOYMENTS)
            .build();
        let connector = CloudflareConnector::new(client);
        let result = connector.ingest(default_pages_request()).unwrap();

        for item in &result.evidence {
            assert!(item.is_cloudflare());
            assert!(item.is_cloudflare_pages());
            assert!(!item.is_cloudflare_worker());
            assert_eq!(item.source.connector, CLOUDFLARE_CONNECTOR);
            assert!(item.source.read_only);
        }
    }

    #[test]
    fn cloudflare_worker_evidence_items_are_marked_cloudflare() {
        let client = FixtureCloudflareClient::builder()
            .worker_deployments(FIXTURE_WORKER_DEPLOYMENTS)
            .build();
        let connector = CloudflareConnector::new(client);
        let result = connector.ingest(default_worker_request()).unwrap();

        for item in &result.evidence {
            assert!(item.is_cloudflare());
            assert!(item.is_cloudflare_worker());
            assert!(!item.is_cloudflare_pages());
            assert_eq!(item.source.connector, CLOUDFLARE_CONNECTOR);
            assert!(item.source.read_only);
        }
    }

    #[test]
    fn connector_version_is_shared() {
        let source = EvidenceSource::cloudflare("my-app");
        assert_eq!(source.version, crate::CONNECTOR_VERSION);
    }

    #[test]
    fn target_repository_label_includes_platform() {
        let pages = CloudflareTarget::pages("acc_123", "my-app");
        assert_eq!(pages.repository_label(), "acc_123/pages:my-app");

        let worker = CloudflareTarget::worker("acc_123", "my-worker");
        assert_eq!(worker.repository_label(), "acc_123/workers:my-worker");
    }

    #[test]
    fn zero_limit_returns_error() {
        let client = FixtureCloudflareClient::builder()
            .pages_deployments(FIXTURE_PAGES_DEPLOYMENTS)
            .build();
        let connector = CloudflareConnector::new(client);
        let result = connector.ingest(CloudflareIngestRequest::new(pages_target()).with_limit(0));
        assert!(result.is_err());
    }
}
