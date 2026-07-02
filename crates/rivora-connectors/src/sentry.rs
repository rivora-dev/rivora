//! Read-only Sentry observability evidence connector.
//!
//! Feeds Sentry issue/error evidence into Rivora's evidence store. The
//! connector is strictly read-only: it only issues `GET` requests against the
//! Sentry REST API and never calls mutation endpoints (`POST`, `PUT`, `PATCH`,
//! `DELETE`). It never resolves, assigns, or mutes issues, never creates
//! alerts, and never modifies projects, releases, or organization settings.
//!
//! Authentication requires a `SENTRY_AUTH_TOKEN` environment variable
//! (alternatively `SENTRY_TOKEN`). Tokens are never stored in `.rivora/`,
//! never printed, and never written into evidence bodies, logs, or receipts.
//! Error messages are redacted so tokens cannot leak through `curl` stderr.
//!
//! The connector is metadata-first. It does not ingest raw stack traces,
//! raw event payloads, request bodies, request headers, cookies, auth
//! headers, user emails, usernames, IP addresses, session replay URLs, or
//! breadcrumbs by default.

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use rivora_errors::{Result, RivoraError};
use serde::{Deserialize, Deserializer};

use crate::{slug, EvidenceId, EvidenceItem, EvidenceKind, EvidenceSource};

/// Connector name written into [`EvidenceSource::connector`].
pub const SENTRY_CONNECTOR: &str = "sentry";
/// Default Sentry REST API base URL.
pub const SENTRY_API_BASE: &str = "https://sentry.io/api/0";

/// HTTP methods the Sentry connector is allowed to use. The connector only
/// ever issues `GET` requests.
#[must_use]
pub fn sentry_allowed_http_methods() -> &'static [&'static str] {
    &["GET"]
}

/// HTTP methods the Sentry connector must never use.
#[must_use]
pub fn sentry_forbidden_http_methods() -> &'static [&'static str] {
    &["POST", "PUT", "PATCH", "DELETE"]
}

/// Enforce the connector's GET-only boundary before any request is created.
pub fn ensure_sentry_read_only_method(method: &str) -> Result<()> {
    if method.eq_ignore_ascii_case("GET") {
        Ok(())
    } else {
        Err(RivoraError::provider(
            "sentry",
            format!("Sentry connector rejected non-read-only HTTP method {method}"),
        ))
    }
}

/// Replace exact and recognizable Sentry token values with `[redacted]`.
///
/// Used to scrub `curl` stderr and any other string before it can appear in an
/// error message. Returns `value` unchanged when `token` is empty.
#[must_use]
fn redact_token(value: &str, token: &str) -> String {
    let exact_redacted = if token.is_empty() {
        value.to_string()
    } else {
        value.replace(token, "[redacted]")
    };
    redact_token_like_values(&exact_redacted)
}

fn redact_token_like_values(value: &str) -> String {
    let parts = value.split_whitespace().collect::<Vec<_>>();
    let mut redacted = Vec::with_capacity(parts.len());
    let mut index = 0;
    while index < parts.len() {
        let part = parts[index];
        let trimmed = trim_sensitive_token(part);
        if trimmed.eq_ignore_ascii_case("bearer") {
            redacted.push("[redacted]".to_string());
            index += usize::from(index + 1 < parts.len()) + 1;
            continue;
        }
        if is_token_like(trimmed) {
            redacted.push(part.replace(trimmed, "[redacted]"));
        } else {
            redacted.push(part.to_string());
        }
        index += 1;
    }
    redacted.join(" ")
}

/// Sentry authentication configuration.
///
/// The token is held privately and never exposed through a getter. Use
/// [`Self::has_token`] to check whether a token is configured and
/// [`Self::redact`] to scrub strings before they can appear in errors or logs.
#[derive(Clone)]
pub struct SentryAuthConfig {
    token: Option<String>,
}

impl std::fmt::Debug for SentryAuthConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SentryAuthConfig")
            .field("token", &self.token.as_ref().map(|_| "[redacted]"))
            .finish()
    }
}

impl SentryAuthConfig {
    /// Read `SENTRY_AUTH_TOKEN` (preferred) or `SENTRY_TOKEN` from the
    /// environment.
    #[must_use]
    pub fn from_env() -> Self {
        let token = std::env::var("SENTRY_AUTH_TOKEN")
            .ok()
            .filter(|token| !token.trim().is_empty())
            .or_else(|| {
                std::env::var("SENTRY_TOKEN")
                    .ok()
                    .filter(|token| !token.trim().is_empty())
            });
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
            None => redact_token(value, ""),
        }
    }
}

/// A Sentry organization + project reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentryProjectRef {
    pub org: String,
    pub project: String,
}

impl SentryProjectRef {
    #[must_use]
    pub fn new(org: impl Into<String>, project: impl Into<String>) -> Self {
        Self {
            org: org.into(),
            project: project.into(),
        }
    }

    /// The repository label written into [`EvidenceSource`]. Uses
    /// `org/project` so evidence provenance is unambiguous.
    #[must_use]
    pub fn repository_label(&self) -> String {
        format!("{}/{}", self.org, self.project)
    }
}

/// Read-only Sentry API client contract.
///
/// Every method issues a `GET` request and returns the raw JSON body. The
/// trait intentionally exposes no mutation operations.
pub trait SentryClient {
    fn fetch_issues(
        &self,
        project: &SentryProjectRef,
        limit: usize,
        query: Option<&str>,
        environment: Option<&str>,
        since: Option<&str>,
    ) -> Result<String>;
}

/// Real Sentry REST API client backed by `curl`.
///
/// The token is passed to `curl` through stdin (`--config -`) so it never
/// appears in the process argument list and is not visible via `ps`. The
/// client only constructs `GET` requests.
#[derive(Debug, Clone)]
pub struct HttpSentryClient {
    auth: SentryAuthConfig,
    base_url: String,
}

impl HttpSentryClient {
    #[must_use]
    pub fn new(auth: SentryAuthConfig) -> Self {
        Self {
            auth,
            base_url: SENTRY_API_BASE.to_string(),
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

    fn issues_path(
        &self,
        project: &SentryProjectRef,
        limit: usize,
        query: Option<&str>,
        environment: Option<&str>,
        since: Option<&str>,
    ) -> String {
        let mut path = format!(
            "/organizations/{}/issues/?project={}&limit={}",
            url_encode(&project.org),
            url_encode(&project.project),
            clamp_limit(limit),
        );
        if let Some(query) = query {
            path.push_str(&format!("&query={}", url_encode(query)));
        }
        if let Some(environment) = environment {
            path.push_str(&format!("&environment={}", url_encode(environment)));
        }
        if let Some(since_value) = since {
            if let Some(seconds) = parse_since_to_epoch_seconds(since_value) {
                path.push_str(&format!(
                    "&start={}",
                    url_encode(&epoch_secs_to_iso(seconds))
                ));
            }
        }
        path
    }

    fn get(&self, path: &str) -> Result<String> {
        ensure_sentry_read_only_method("GET")?;
        let config = self.request_config(path);
        let mut child = Command::new("curl")
            .arg("--config")
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| RivoraError::provider("sentry", format!("curl unavailable: {e}")))?;
        {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| RivoraError::provider("sentry", "could not open curl stdin pipe"))?;
            stdin.write_all(config.as_bytes()).map_err(|e| {
                RivoraError::provider("sentry", format!("curl config write failed: {e}"))
            })?;
        }
        let output = child
            .wait_with_output()
            .map_err(|e| RivoraError::provider("sentry", format!("curl did not finish: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let redacted = self.auth.redact(stderr.as_ref());
            let redacted_path = self.auth.redact(path);
            return Err(RivoraError::provider(
                "sentry",
                format!(
                    "Sentry API request failed for {}: {}",
                    redacted_path,
                    redacted.trim()
                ),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

impl SentryClient for HttpSentryClient {
    fn fetch_issues(
        &self,
        project: &SentryProjectRef,
        limit: usize,
        query: Option<&str>,
        environment: Option<&str>,
        since: Option<&str>,
    ) -> Result<String> {
        let path = self.issues_path(project, limit, query, environment, since);
        self.get(&path)
    }
}

/// Test double for [`SentryClient`] that returns preloaded fixture JSON without
/// any network access.
#[derive(Debug, Clone, Default)]
pub struct FixtureSentryClient {
    issues: Option<String>,
}

impl FixtureSentryClient {
    #[must_use]
    pub fn builder() -> FixtureSentryClientBuilder {
        FixtureSentryClientBuilder::default()
    }
}

impl SentryClient for FixtureSentryClient {
    fn fetch_issues(
        &self,
        _project: &SentryProjectRef,
        _limit: usize,
        _query: Option<&str>,
        _environment: Option<&str>,
        _since: Option<&str>,
    ) -> Result<String> {
        self.issues
            .clone()
            .ok_or_else(|| RivoraError::provider("sentry", "no fixture loaded for issues"))
    }
}

#[derive(Debug, Default, Clone)]
pub struct FixtureSentryClientBuilder {
    issues: Option<String>,
}

impl FixtureSentryClientBuilder {
    #[must_use]
    pub fn issues(mut self, fixture: impl Into<String>) -> Self {
        self.issues = Some(fixture.into());
        self
    }

    #[must_use]
    pub fn build(self) -> FixtureSentryClient {
        FixtureSentryClient {
            issues: self.issues,
        }
    }
}

fn clamp_limit(limit: usize) -> usize {
    limit.clamp(1, 100)
}

fn url_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push('+'),
            _ => {
                encoded.push('%');
                encoded.push_str(&format!("{:02X}", byte));
            }
        }
    }
    encoded
}

/// Request for Sentry evidence ingestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentryIngestRequest {
    pub project: SentryProjectRef,
    pub limit: usize,
    pub since: Option<String>,
    pub environment: Option<String>,
    pub query: Option<String>,
}

impl SentryIngestRequest {
    #[must_use]
    pub fn new(project: SentryProjectRef) -> Self {
        Self {
            project,
            limit: 20,
            since: None,
            environment: None,
            query: None,
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
    pub fn with_environment(mut self, environment: impl Into<String>) -> Self {
        self.environment = Some(environment.into());
        self
    }

    #[must_use]
    pub fn with_query(mut self, query: impl Into<String>) -> Self {
        self.query = Some(query.into());
        self
    }
}

/// Result of Sentry evidence ingestion.
#[derive(Debug, Clone, PartialEq)]
pub struct SentryIngestResult {
    pub repository: String,
    pub evidence: Vec<EvidenceItem>,
    pub issues: usize,
    pub topics: Vec<String>,
}

/// Read-only Sentry connector. Holds a boxed [`SentryClient`] so the CLI can
/// swap in a [`FixtureSentryClient`] for tests without generics leaking into
/// calling code.
pub struct SentryConnector {
    client: Box<dyn SentryClient>,
}

impl std::fmt::Debug for SentryConnector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SentryConnector").finish_non_exhaustive()
    }
}

impl SentryConnector {
    #[must_use]
    pub fn new(client: impl SentryClient + 'static) -> Self {
        Self {
            client: Box::new(client),
        }
    }

    pub fn ingest(&self, request: SentryIngestRequest) -> Result<SentryIngestResult> {
        if request.limit == 0 {
            return Err(RivoraError::invalid_value(
                "limit",
                "limit must be positive",
            ));
        }

        let project = request.project.clone();
        let repository = project.repository_label();
        let source = EvidenceSource::sentry(repository.clone());
        let limit = clamp_limit(request.limit);

        let mut evidence = Vec::new();
        let since_cutoff = request
            .since
            .as_deref()
            .map(parse_since_cutoff)
            .transpose()?;
        let raw = self.client.fetch_issues(
            &project,
            limit,
            request.query.as_deref(),
            request.environment.as_deref(),
            request.since.as_deref(),
        )?;
        let parsed = parse_issues(&raw)?;
        for issue in parsed {
            let item = issue_item(&source, &project, &issue, request.environment.as_deref())?;
            evidence.push(item);
        }

        evidence.sort_by(|a, b| a.id.cmp(&b.id));
        evidence.dedup_by(|a, b| a.id == b.id);
        if let Some(cutoff) = since_cutoff {
            evidence.retain(|item| evidence_is_after_cutoff(item, cutoff));
        }
        evidence.truncate(limit);

        let mut topics = std::collections::BTreeSet::new();
        for item in &evidence {
            collect_topics(item, &mut topics);
        }

        let issues = evidence.len();

        Ok(SentryIngestResult {
            repository,
            evidence,
            issues,
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
        .and_then(parse_sentry_timestamp)
        .is_some_and(|timestamp| timestamp >= cutoff)
}

fn parse_since_cutoff(value: &str) -> Result<i64> {
    let trimmed = value.trim();
    if let Some((amount, unit_seconds)) = relative_duration(trimmed) {
        let amount = amount.parse::<i64>().map_err(|_| {
            RivoraError::invalid_value(
                "sentry_since",
                "use an ISO timestamp or duration like 24h or 7d",
            )
        })?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| {
                RivoraError::invalid_value("sentry_since", "system clock is before unix epoch")
            })?
            .as_secs() as i64;
        return Ok(now - amount.saturating_mul(unit_seconds));
    }
    parse_sentry_timestamp(trimmed).ok_or_else(|| {
        RivoraError::invalid_value(
            "sentry_since",
            "use an ISO timestamp or duration like 24h or 7d",
        )
    })
}

fn parse_since_to_epoch_seconds(value: &str) -> Option<i64> {
    let trimmed = value.trim();
    if let Some((amount, unit_seconds)) = relative_duration(trimmed) {
        let amount = amount.parse::<i64>().ok()?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
        return Some(now - amount.saturating_mul(unit_seconds));
    }
    parse_sentry_timestamp(trimmed)
}

fn relative_duration(value: &str) -> Option<(&str, i64)> {
    value
        .strip_suffix('h')
        .map(|amount| (amount, 3_600))
        .or_else(|| value.strip_suffix('d').map(|amount| (amount, 86_400)))
}

fn parse_sentry_timestamp(value: &str) -> Option<i64> {
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

fn epoch_secs_to_iso(secs: i64) -> String {
    let days = secs / 86_400;
    let remainder = (secs % 86_400).unsigned_abs();
    let hour = remainder / 3_600;
    let minute = (remainder % 3_600) / 60;
    let second = remainder % 60;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, minute, second
    )
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

// --- Sentry API response shapes (only fields Rivora uses) -------------------
//
// These structs model the subset of Sentry REST API fields Rivora ingests.
// Fields are captured for forward compatibility even when not yet read,
// so `dead_code` is allowed on the DTOs.

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IssueResponse {
    id: Option<String>,
    short_id: Option<String>,
    title: Option<String>,
    culprit: Option<String>,
    level: Option<String>,
    status: Option<String>,
    platform: Option<String>,
    permalink: Option<String>,
    first_seen: Option<String>,
    last_seen: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_scalar")]
    count: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_scalar")]
    user_count: Option<String>,
    issue_type: Option<String>,
    #[serde(rename = "type")]
    legacy_type: Option<String>,
    matching_event_environment: Option<String>,
    tags: Option<Vec<IssueTag>>,
}

fn deserialize_optional_scalar<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(match value {
        Some(serde_json::Value::String(value)) => Some(value),
        Some(serde_json::Value::Number(value)) => Some(value.to_string()),
        _ => None,
    })
}

#[derive(Debug, Clone, Deserialize)]
struct IssueTag {
    key: Option<String>,
    value: Option<String>,
}

fn parse_issues(raw: &str) -> Result<Vec<IssueResponse>> {
    serde_json::from_str::<Vec<IssueResponse>>(raw).map_err(|error| {
        RivoraError::provider("sentry", format!("invalid Sentry issue response: {error}"))
    })
}

// --- Metadata allowlist -----------------------------------------------------

/// Allowed metadata keys for Sentry evidence. Only these keys are written
/// into the evidence body. This is intentionally narrow to avoid leaking
/// sensitive data.
const ALLOWED_METADATA_KEYS: &[&str] = &[
    "issue_id",
    "org",
    "project",
    "title",
    "culprit",
    "issue_type",
    "level",
    "status",
    "permalink",
    "first_seen",
    "last_seen",
    "count",
    "user_count",
    "environment",
    "release",
    "transaction",
    "platform",
];

/// Allowed tag keys that may be included in evidence. Tags not in this list
/// are silently dropped to avoid leaking PII or secrets.
const ALLOWED_TAG_KEYS: &[&str] = &["environment", "release", "transaction"];

#[must_use]
fn is_allowed_metadata_key(key: &str) -> bool {
    ALLOWED_METADATA_KEYS.contains(&key)
}

#[must_use]
fn is_allowed_tag_key(key: &str) -> bool {
    ALLOWED_TAG_KEYS.contains(&key)
}

fn extract_allowed_tag(tags: &Option<Vec<IssueTag>>, key: &str) -> Option<String> {
    if !is_allowed_tag_key(key) {
        return None;
    }
    tags.as_ref()?
        .iter()
        .find(|tag| tag.key.as_deref() == Some(key))
        .and_then(|tag| tag.value.clone())
        .filter(|value| !value.trim().is_empty())
}

// --- Evidence mapping -------------------------------------------------------

fn issue_item(
    source: &EvidenceSource,
    project: &SentryProjectRef,
    issue: &IssueResponse,
    requested_environment: Option<&str>,
) -> Result<EvidenceItem> {
    let issue_id = issue.id.clone().unwrap_or_default();
    if !is_safe_identifier(&issue_id) {
        return Err(RivoraError::invalid_value(
            "sentry_issue_id",
            "Sentry issue response did not include a safe id",
        ));
    }
    let short_id = issue
        .short_id
        .clone()
        .filter(|value| is_safe_identifier(value))
        .unwrap_or_default();
    let title = sanitize_metadata_value(
        &issue
            .title
            .clone()
            .unwrap_or_else(|| "Unknown error".to_string()),
    );
    let culprit = sanitize_metadata_value(&issue.culprit.clone().unwrap_or_default());
    let level = sanitize_metadata_value(issue.level.as_deref().unwrap_or("unknown"));
    let status = sanitize_metadata_value(issue.status.as_deref().unwrap_or("unresolved"));
    let platform = sanitize_metadata_value(&issue.platform.clone().unwrap_or_default());
    let permalink = safe_permalink(issue.permalink.as_deref());
    let first_seen = safe_timestamp(issue.first_seen.as_deref());
    let last_seen = safe_timestamp(issue.last_seen.as_deref());
    let count = safe_count(issue.count.as_deref());
    let user_count = safe_count(issue.user_count.as_deref());
    let issue_type = sanitize_metadata_value(
        issue
            .issue_type
            .as_deref()
            .or(issue.legacy_type.as_deref())
            .unwrap_or("error"),
    );

    let environment = extract_allowed_tag(&issue.tags, "environment")
        .or_else(|| issue.matching_event_environment.clone())
        .or_else(|| requested_environment.map(str::to_string))
        .map(|value| sanitize_metadata_value(&value))
        .unwrap_or_default();
    let release = extract_allowed_tag(&issue.tags, "release")
        .map(|value| sanitize_metadata_value(&value))
        .unwrap_or_default();
    let transaction = extract_allowed_tag(&issue.tags, "transaction")
        .map(|value| sanitize_metadata_value(&value))
        .unwrap_or_default();

    let org_slug = slug(&project.org);
    let project_slug = slug(&project.project);

    let display_id = if short_id.is_empty() {
        issue_id.clone()
    } else {
        short_id.clone()
    };

    let is_error = matches!(level.to_ascii_lowercase().as_str(), "error" | "fatal");
    let is_resolved = status.eq_ignore_ascii_case("resolved");

    let summary = if is_resolved {
        format!(
            "Sentry issue {} in {} resolved ({})",
            display_id, project.project, level
        )
    } else if is_error {
        format!(
            "Sentry issue {} in {}: {} ({}, {} events)",
            display_id, project.project, title, level, count
        )
    } else {
        format!(
            "Sentry issue {} in {} ({}, {})",
            display_id, project.project, level, status
        )
    };

    let body = render_allowed_metadata(&[
        ("issue_id", display_id.clone()),
        ("org", project.org.clone()),
        ("project", project.project.clone()),
        ("title", title.clone()),
        ("culprit", culprit),
        ("issue_type", issue_type),
        ("level", level.clone()),
        ("status", status.clone()),
        ("permalink", permalink.clone()),
        ("first_seen", first_seen.clone()),
        ("last_seen", last_seen.clone()),
        ("count", count),
        ("user_count", user_count),
        ("environment", value_or_unknown(&environment).to_string()),
        ("release", value_or_unknown(&release).to_string()),
        ("transaction", value_or_unknown(&transaction).to_string()),
        ("platform", value_or_unknown(&platform).to_string()),
    ]);

    let mut tags = vec![
        "sentry".to_string(),
        "observability".to_string(),
        "error".to_string(),
        "issue".to_string(),
    ];
    if is_error && !is_resolved {
        tags.push("failed".to_string());
    }
    if !environment.is_empty() {
        tags.push(slug(&environment));
    }
    if !release.is_empty() {
        tags.push(slug(&release));
    }
    tags.push(slug(&project.project));
    tags.push(level.to_ascii_lowercase());
    tags.push(format!("level-{}", slug(&level)));
    tags.push(format!("status-{}", slug(&status)));
    tags.sort();
    tags.dedup();

    let mut refs = vec![format!("issue:{}", issue_id)];
    if !permalink.is_empty() {
        refs.push(permalink.clone());
    }
    if !short_id.is_empty() {
        refs.push(format!("short_id:{}", short_id));
    }

    let service = slug(&project.project);
    let timestamp = if !last_seen.is_empty() {
        Some(last_seen.clone())
    } else if !first_seen.is_empty() {
        Some(first_seen.clone())
    } else {
        None
    };

    let confidence = if is_error && !is_resolved {
        0.9
    } else if is_resolved {
        0.75
    } else {
        0.8
    };

    Ok(EvidenceItem {
        id: EvidenceId::new(format!(
            "sentry:issue:{}:{}:{}",
            org_slug, project_slug, issue_id
        ))?,
        kind: EvidenceKind::SentryIssue,
        source: source.clone(),
        title: format!("Sentry issue {} ({})", display_id, level),
        summary,
        body,
        service: Some(service),
        files_changed: Vec::new(),
        timestamp,
        author: None,
        tags,
        refs,
        confidence,
    })
}

/// Redact common token and PII shapes from allowlisted, user-controlled
/// metadata such as issue titles, culprits, and transaction names.
fn sanitize_metadata_value(value: &str) -> String {
    redact_token_like_values(value)
        .split_whitespace()
        .map(|part| {
            let trimmed = trim_sensitive_token(part);
            if looks_sensitive(trimmed) {
                part.replace(trimmed, "[redacted]")
            } else {
                part.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn looks_sensitive(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains('@')
        || is_ipv4(value)
        || is_token_like(&lower)
        || lower.starts_with("/secret/")
        || lower.starts_with("/users/")
        || lower.starts_with("/home/")
        || lower.contains("\\users\\")
}

fn is_token_like(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.starts_with("sntrys_")
        || lower.starts_with("sentry_auth_token=")
        || lower.starts_with("sentry_token=")
}

fn trim_sensitive_token(value: &str) -> &str {
    value.trim_matches(|character: char| {
        !character.is_ascii_alphanumeric()
            && !matches!(character, '.' | '@' | '_' | '-' | '=' | '/' | '\\')
    })
}

fn is_safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
        && !is_token_like(value)
}

fn safe_count(value: Option<&str>) -> String {
    value
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0)
        .to_string()
}

fn safe_timestamp(value: Option<&str>) -> String {
    value
        .filter(|value| parse_sentry_timestamp(value).is_some())
        .map(str::to_string)
        .unwrap_or_default()
}

fn safe_permalink(value: Option<&str>) -> String {
    value
        .filter(|value| value.starts_with("https://") || value.starts_with("http://"))
        .map(sanitize_metadata_value)
        .unwrap_or_default()
}

fn value_or_unknown(value: &str) -> &str {
    if value.is_empty() {
        "unknown"
    } else {
        value
    }
}

fn render_allowed_metadata(metadata: &[(&str, String)]) -> String {
    metadata
        .iter()
        .filter(|(key, _)| is_allowed_metadata_key(key))
        .map(|(key, value)| format!("{}: {}", metadata_label(key), value))
        .collect::<Vec<_>>()
        .join("\n")
}

fn metadata_label(key: &str) -> &'static str {
    match key {
        "issue_id" => "Issue",
        "org" => "Org",
        "project" => "Project",
        "title" => "Title",
        "culprit" => "Culprit",
        "issue_type" => "Type",
        "level" => "Level",
        "status" => "Status",
        "permalink" => "Permalink",
        "first_seen" => "First seen",
        "last_seen" => "Last seen",
        "count" => "Count",
        "user_count" => "User count",
        "environment" => "Environment",
        "release" => "Release",
        "transaction" => "Transaction",
        "platform" => "Platform",
        _ => "Unknown",
    }
}

fn is_ipv4(value: &str) -> bool {
    let parts = value.split('.').collect::<Vec<_>>();
    parts.len() == 4
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.parse::<u8>().is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> SentryProjectRef {
        SentryProjectRef::new("my-org", "checkout-api")
    }

    fn default_request() -> SentryIngestRequest {
        SentryIngestRequest::new(project()).with_limit(10)
    }

    const FIXTURE_ISSUES: &str = r#"[
        {
            "id": "1001",
            "shortId": "CHECKOUT-ABC",
            "title": "TypeError: Cannot read property 'total' of undefined",
            "culprit": "checkout/handler at line 42",
            "level": "error",
            "status": "unresolved",
            "platform": "javascript",
            "permalink": "https://sentry.example/organizations/my-org/issues/1001/",
            "firstSeen": "2026-06-27T10:00:00Z",
            "lastSeen": "2026-06-27T10:45:00Z",
            "count": "142",
            "userCount": 38,
            "type": "error",
            "project": {
                "id": "proj_001",
                "slug": "checkout-api",
                "name": "Checkout API",
                "platform": "javascript"
            },
            "tags": [
                { "key": "environment", "value": "production" },
                { "key": "release", "value": "checkout-api@2.4.1" },
                { "key": "transaction", "value": "/api/checkout" },
                { "key": "level", "value": "error" },
                { "key": "secret_tag", "value": "should_not_appear" }
            ]
        },
        {
            "id": "1002",
            "shortId": "CHECKOUT-DEF",
            "title": "Error: Payment gateway timeout",
            "culprit": "payments/gateway at line 88",
            "level": "error",
            "status": "unresolved",
            "platform": "node",
            "permalink": "https://sentry.example/organizations/my-org/issues/1002/",
            "firstSeen": "2026-06-27T09:30:00Z",
            "lastSeen": "2026-06-27T10:50:00Z",
            "count": "67",
            "userCount": "12",
            "type": "error",
            "project": {
                "id": "proj_001",
                "slug": "checkout-api",
                "name": "Checkout API",
                "platform": "node"
            },
            "tags": [
                { "key": "environment", "value": "production" },
                { "key": "release", "value": "checkout-api@2.4.0" },
                { "key": "transaction", "value": "/api/payments" }
            ]
        },
        {
            "id": "1003",
            "shortId": "CHECKOUT-GHI",
            "title": "Warning: Deprecated API usage",
            "culprit": "legacy/adapter",
            "level": "warning",
            "status": "resolved",
            "platform": "python",
            "permalink": "https://sentry.example/organizations/my-org/issues/1003/",
            "firstSeen": "2026-06-26T08:00:00Z",
            "lastSeen": "2026-06-26T12:00:00Z",
            "count": "5",
            "userCount": "2",
            "type": "error",
            "project": {
                "id": "proj_001",
                "slug": "checkout-api",
                "name": "Checkout API",
                "platform": "python"
            },
            "tags": [
                { "key": "environment", "value": "staging" },
                { "key": "release", "value": "checkout-api@2.3.9" }
            ]
        }
    ]"#;

    #[test]
    fn parses_issue_fixture_into_evidence() {
        let client = FixtureSentryClient::builder()
            .issues(FIXTURE_ISSUES)
            .build();
        let connector = SentryConnector::new(client);
        let result = connector.ingest(default_request()).unwrap();

        assert_eq!(result.issues, 3);

        let error = result
            .evidence
            .iter()
            .find(|item| item.id.as_str() == "sentry:issue:my-org:checkout-api:1001")
            .unwrap();
        assert_eq!(error.kind, EvidenceKind::SentryIssue);
        assert!(error.summary.contains("TypeError"));
        assert!(error.tags.contains(&"sentry".to_string()));
        assert!(error.tags.contains(&"observability".to_string()));
        assert!(error.tags.contains(&"error".to_string()));
        assert!(error.tags.contains(&"failed".to_string()));
        assert!(error.tags.contains(&"production".to_string()));
        assert_eq!(error.confidence, 0.9);
        assert!(error.body.contains("Org: my-org"));
        assert!(error.body.contains("Project: checkout-api"));
        assert!(error.body.contains("Count: 142"));
        assert!(error.body.contains("User count: 38"));
        assert!(error.refs.iter().any(|r| r == "issue:1001"));
        assert!(error.refs.iter().any(|r| r == "short_id:CHECKOUT-ABC"));
    }

    #[test]
    fn parses_resolved_issue_evidence() {
        let client = FixtureSentryClient::builder()
            .issues(FIXTURE_ISSUES)
            .build();
        let connector = SentryConnector::new(client);
        let result = connector.ingest(default_request()).unwrap();

        let resolved = result
            .evidence
            .iter()
            .find(|item| item.id.as_str() == "sentry:issue:my-org:checkout-api:1003")
            .unwrap();
        assert!(resolved.summary.contains("resolved"));
        assert!(!resolved.tags.contains(&"failed".to_string()));
        assert_eq!(resolved.confidence, 0.75);
    }

    #[test]
    fn sentry_evidence_ids_are_stable_across_ingests() {
        let client = FixtureSentryClient::builder()
            .issues(FIXTURE_ISSUES)
            .build();
        let first = SentryConnector::new(client.clone())
            .ingest(default_request())
            .unwrap();
        let second = SentryConnector::new(client)
            .ingest(default_request())
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
    fn sentry_connector_deduplicates_by_id_within_one_ingest() {
        let client = FixtureSentryClient::builder()
            .issues(FIXTURE_ISSUES)
            .build();
        let connector = SentryConnector::new(client);
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
    fn since_filter_keeps_only_evidence_at_or_after_cutoff() {
        let client = FixtureSentryClient::builder()
            .issues(FIXTURE_ISSUES)
            .build();
        let connector = SentryConnector::new(client);
        let result = connector
            .ingest(SentryIngestRequest::new(project()).with_since("2026-06-27T00:00:00Z"))
            .unwrap();

        assert_eq!(result.issues, 2);
        assert!(result
            .evidence
            .iter()
            .all(|item| item.id.as_str() != "sentry:issue:my-org:checkout-api:1003"));
    }

    #[test]
    fn since_filter_rejects_unparseable_values() {
        let client = FixtureSentryClient::builder()
            .issues(FIXTURE_ISSUES)
            .build();
        let connector = SentryConnector::new(client);
        let result = connector.ingest(SentryIngestRequest::new(project()).with_since("recently"));
        assert!(result.is_err());
    }

    #[test]
    fn since_filter_accepts_hour_and_day_durations() {
        assert!(parse_since_cutoff("24h").is_ok());
        assert!(parse_since_cutoff("7d").is_ok());
        assert!(parse_since_to_epoch_seconds("24h").is_some());
    }

    #[test]
    fn missing_fixture_returns_provider_error() {
        let client = FixtureSentryClient::builder().build();
        let connector = SentryConnector::new(client);
        let result = connector.ingest(default_request());
        assert!(result.is_err());
    }

    #[test]
    fn redact_token_scrubs_secret_from_strings() {
        let auth = SentryAuthConfig::with_token("sentry_secret_token");
        assert_eq!(
            auth.redact("error: sentry_secret_token is invalid"),
            "error: [redacted] is invalid"
        );
        assert_eq!(
            redact_token("plain text", "sentry_secret_token"),
            "plain text"
        );
        assert_eq!(redact_token("plain text", ""), "plain text");
    }

    #[test]
    fn auth_debug_and_errors_never_expose_exact_or_token_like_values() {
        let auth = SentryAuthConfig::with_token("sntrys_configured_secret");
        let debug = format!("{auth:?}");
        let redacted = auth
            .redact("Authorization: Bearer sntrys_configured_secret; rejected sntrys_other_secret");

        assert!(!debug.contains("sntrys_configured_secret"), "{debug}");
        assert!(!redacted.contains("sntrys_configured_secret"), "{redacted}");
        assert!(!redacted.contains("sntrys_other_secret"), "{redacted}");
        assert!(!redacted.contains("Bearer"), "{redacted}");
        assert_eq!(
            SentryAuthConfig::with_token("").redact("Bearer sntrys_invalid"),
            "[redacted]"
        );
    }

    #[test]
    fn auth_env_var_behavior() {
        std::env::remove_var("SENTRY_AUTH_TOKEN");
        std::env::remove_var("SENTRY_TOKEN");
        let auth_empty = SentryAuthConfig::from_env();
        assert!(!auth_empty.has_token());

        std::env::set_var("SENTRY_TOKEN", "sentry_fallback");
        let auth_fallback = SentryAuthConfig::from_env();
        assert!(auth_fallback.has_token());

        std::env::set_var("SENTRY_AUTH_TOKEN", "sentry_primary");
        let auth_preferred = SentryAuthConfig::from_env();
        assert!(auth_preferred.has_token());
        assert_eq!(auth_preferred.redact("sentry_primary"), "[redacted]");
        assert_eq!(
            auth_preferred.redact("sentry_primary sentry_fallback"),
            "[redacted] sentry_fallback"
        );

        std::env::remove_var("SENTRY_AUTH_TOKEN");
        std::env::remove_var("SENTRY_TOKEN");
    }

    #[test]
    fn http_client_request_config_uses_only_get_and_carries_token_via_stdin() {
        let auth = SentryAuthConfig::with_token("sentry_secret_token");
        let client = HttpSentryClient::new(auth);
        let config =
            client.request_config("/organizations/my-org/issues/?project=checkout-api&limit=20");

        assert!(config.contains("request = \"GET\""));
        assert!(!config.contains("\"POST\""));
        assert!(!config.contains("\"PUT\""));
        assert!(!config.contains("\"PATCH\""));
        assert!(!config.contains("\"DELETE\""));
        assert!(config.contains("sentry_secret_token"));
        assert!(!client.auth.redact(&config).contains("sentry_secret_token"));
    }

    #[test]
    fn issue_list_path_uses_documented_read_endpoint_filters_and_cap() {
        let client = HttpSentryClient::new(SentryAuthConfig::with_token("token"));
        let default_path = client.issues_path(&project(), 1_000, None, None, None);
        let filtered_path = client.issues_path(
            &project(),
            20,
            Some("is:unresolved"),
            Some("production"),
            Some("2026-07-01T00:00:00Z"),
        );

        assert_eq!(
            default_path,
            "/organizations/my-org/issues/?project=checkout-api&limit=100"
        );
        assert!(!default_path.contains("query="));
        assert!(filtered_path.contains("query=is%3Aunresolved"));
        assert!(filtered_path.contains("environment=production"));
        assert!(filtered_path.contains("start=2026-07-01T00%3A00%3A00Z"));
    }

    #[test]
    fn sentry_connector_exposes_no_mutation_http_methods() {
        for method in sentry_forbidden_http_methods() {
            assert!(!sentry_allowed_http_methods().contains(method));
        }
        assert_eq!(sentry_allowed_http_methods(), &["GET"]);
        assert!(ensure_sentry_read_only_method("GET").is_ok());
        for method in sentry_forbidden_http_methods() {
            assert!(ensure_sentry_read_only_method(method).is_err());
        }
    }

    #[test]
    fn sentry_evidence_never_embeds_auth_token() {
        let fixture = r#"[
            {
                "id": "9999",
                "shortId": "LEAK-001",
                "title": "sntrys_should_not_leak in handler",
                "culprit": "handler",
                "level": "error",
                "status": "unresolved",
                "platform": "javascript",
                "permalink": "https://sentry.example/issues/9999/",
                "firstSeen": "2026-06-27T10:00:00Z",
                "lastSeen": "2026-06-27T10:00:00Z",
                "count": "1",
                "userCount": "1",
                "tags": []
            }
        ]"#;
        let client = FixtureSentryClient::builder().issues(fixture).build();
        let connector = SentryConnector::new(client);
        let result = connector.ingest(default_request()).unwrap();

        for item in &result.evidence {
            assert!(
                !item.body.contains("sntrys_should_not_leak"),
                "{}",
                item.body
            );
            assert!(!item.summary.contains("sntrys_should_not_leak"));
            assert!(!item
                .refs
                .iter()
                .any(|r| r.contains("sntrys_should_not_leak")));
        }
    }

    #[test]
    fn sentry_evidence_items_are_marked_sentry() {
        let client = FixtureSentryClient::builder()
            .issues(FIXTURE_ISSUES)
            .build();
        let connector = SentryConnector::new(client);
        let result = connector.ingest(default_request()).unwrap();

        for item in &result.evidence {
            assert!(item.is_sentry());
            assert_eq!(item.source.connector, SENTRY_CONNECTOR);
            assert!(item.source.read_only);
        }
    }

    #[test]
    fn connector_version_is_shared() {
        let source = EvidenceSource::sentry("my-org/checkout-api");
        assert_eq!(source.version, crate::CONNECTOR_VERSION);
    }

    #[test]
    fn project_repository_label_includes_org_and_project() {
        let project = SentryProjectRef::new("my-org", "checkout-api");
        assert_eq!(project.repository_label(), "my-org/checkout-api");
    }

    #[test]
    fn zero_limit_returns_error() {
        let client = FixtureSentryClient::builder()
            .issues(FIXTURE_ISSUES)
            .build();
        let connector = SentryConnector::new(client);
        let result = connector.ingest(SentryIngestRequest::new(project()).with_limit(0));
        assert!(result.is_err());
    }

    #[test]
    fn metadata_allowlist_filters_disallowed_tag_keys() {
        assert_eq!(
            ALLOWED_METADATA_KEYS,
            &[
                "issue_id",
                "org",
                "project",
                "title",
                "culprit",
                "issue_type",
                "level",
                "status",
                "permalink",
                "first_seen",
                "last_seen",
                "count",
                "user_count",
                "environment",
                "release",
                "transaction",
                "platform",
            ]
        );
        assert!(is_allowed_metadata_key("issue_id"));
        assert!(is_allowed_metadata_key("org"));
        assert!(is_allowed_metadata_key("title"));
        assert!(!is_allowed_metadata_key("stack_trace"));
        assert!(!is_allowed_metadata_key("request_body"));
        assert!(!is_allowed_metadata_key("user_email"));
        assert!(!is_allowed_metadata_key("ip_address"));
        assert_eq!(
            render_allowed_metadata(&[("request", "secret".to_string())]),
            ""
        );
    }

    #[test]
    fn tag_allowlist_filters_sensitive_tags() {
        assert_eq!(ALLOWED_TAG_KEYS, &["environment", "release", "transaction"]);
        assert!(is_allowed_tag_key("environment"));
        assert!(is_allowed_tag_key("release"));
        assert!(is_allowed_tag_key("transaction"));
        assert!(!is_allowed_tag_key("user_email"));
        assert!(!is_allowed_tag_key("ip_address"));
        assert!(!is_allowed_tag_key("secret_tag"));
    }

    #[test]
    fn sentry_evidence_does_not_include_disallowed_tags() {
        let client = FixtureSentryClient::builder()
            .issues(FIXTURE_ISSUES)
            .build();
        let connector = SentryConnector::new(client);
        let result = connector.ingest(default_request()).unwrap();

        for item in &result.evidence {
            assert!(
                !item.body.contains("secret_tag"),
                "evidence body should not contain disallowed tag: {}",
                item.body
            );
            assert!(
                !item.body.contains("should_not_appear"),
                "evidence body should not contain disallowed tag value: {}",
                item.body
            );
        }
    }

    #[test]
    fn sentry_evidence_does_not_contain_pii_or_secrets() {
        let client = FixtureSentryClient::builder()
            .issues(FIXTURE_ISSUES)
            .build();
        let connector = SentryConnector::new(client);
        let result = connector.ingest(default_request()).unwrap();

        for item in &result.evidence {
            assert!(!item.body.contains("user_email"));
            assert!(!item.body.contains("ip_address"));
            assert!(!item.body.contains("cookie"));
            assert!(!item.body.contains("session_replay"));
            assert!(!item.body.contains("breadcrumb"));
        }
    }

    #[test]
    fn malicious_nested_fields_and_sensitive_allowlisted_values_are_not_persisted() {
        let fixture = r#"[{
            "id": "7001",
            "shortId": "SAFE-7001",
            "title": "Failure for person@example.com from 192.0.2.1 Bearer title-secret",
            "culprit": "/secret/path sntrys_culprit_secret",
            "level": "error",
            "status": "unresolved",
            "platform": "javascript",
            "permalink": "javascript:alert('secret permalink')",
            "firstSeen": "secret first seen",
            "lastSeen": "2026-07-01T10:00:00Z",
            "count": "secret-count",
            "userCount": "person@example.com",
            "type": "default",
            "issueType": "error_generic",
            "tags": [
                {"key":"environment","value":"production"},
                {"key":"secret_tag","value":"secret arbitrary tag"}
            ],
            "user": {
                "email": "nested-person@example.com",
                "username": "private-user",
                "ip_address": "192.0.2.99"
            },
            "request": {
                "headers": {"Authorization": "Bearer request-secret"},
                "data": {"password": "secret-password"},
                "cookies": "secret-cookie"
            },
            "entries": [{
                "type": "exception",
                "data": {"values": [{"stacktrace": {"frames": [{"filename": "/secret/path"}]}}]}
            }],
            "contexts": {"secret": "secret-context"},
            "breadcrumbs": {"values": [{"message": "secret breadcrumb"}]},
            "replay_id": "secret-replay",
            "session": {"id": "secret-session"}
        }]"#;
        let result = SentryConnector::new(FixtureSentryClient::builder().issues(fixture).build())
            .ingest(default_request())
            .unwrap();
        let serialized = serde_json::to_string(&result.evidence).unwrap();

        for forbidden in [
            "person@example.com",
            "192.0.2.1",
            "title-secret",
            "sntrys_culprit_secret",
            "secret permalink",
            "secret first seen",
            "secret-count",
            "nested-person@example.com",
            "private-user",
            "192.0.2.99",
            "request-secret",
            "secret-password",
            "secret-cookie",
            "/secret/path",
            "secret-context",
            "secret breadcrumb",
            "secret-replay",
            "secret-session",
            "secret arbitrary tag",
        ] {
            assert!(
                !serialized.contains(forbidden),
                "persisted {forbidden}: {serialized}"
            );
        }
        assert!(serialized.contains("Type: error_generic"), "{serialized}");
        assert!(serialized.contains("Count: 0"), "{serialized}");
        assert!(serialized.contains("User count: 0"), "{serialized}");
        assert!(!serialized.contains("javascript:alert"), "{serialized}");
    }

    #[test]
    fn connector_caps_fixture_results_at_one_hundred() {
        let issues = (0..105)
            .map(|id| {
                serde_json::json!({
                    "id": id.to_string(),
                    "shortId": format!("ISSUE-{id}"),
                    "title": format!("Issue {id}"),
                    "level": "error",
                    "status": "unresolved",
                    "lastSeen": "2026-07-01T10:00:00Z",
                    "count": "1",
                    "userCount": "0"
                })
            })
            .collect::<Vec<_>>();
        let fixture = serde_json::to_string(&issues).unwrap();
        let result = SentryConnector::new(FixtureSentryClient::builder().issues(fixture).build())
            .ingest(SentryIngestRequest::new(project()).with_limit(1_000))
            .unwrap();

        assert_eq!(result.evidence.len(), 100);
        assert_eq!(result.issues, 100);
    }

    #[test]
    fn malformed_issue_response_is_an_error_not_an_empty_result() {
        let connector =
            SentryConnector::new(FixtureSentryClient::builder().issues("not-json").build());
        assert!(connector.ingest(default_request()).is_err());
    }

    #[test]
    fn since_filter_rejects_evidence_without_a_valid_timestamp() {
        let fixture = r#"[{"id":"8001","title":"No timestamp","level":"error"}]"#;
        let result = SentryConnector::new(FixtureSentryClient::builder().issues(fixture).build())
            .ingest(SentryIngestRequest::new(project()).with_since("2026-07-01T00:00:00Z"))
            .unwrap();
        assert!(result.evidence.is_empty());
    }
}
