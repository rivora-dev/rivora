//! Read-only PlanetScale data-layer evidence connector.
//!
//! The connector calls only PlanetScale's branch and deploy-request list
//! endpoints. It never connects to a customer database, runs SQL, reads rows,
//! reads branch passwords, or calls mutation endpoints. API responses are
//! mapped through narrow metadata allowlists; raw schema, DDL, connection,
//! credential, row, and arbitrary metadata fields are ignored.

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use rivora_errors::{Result, RivoraError};
use serde::Deserialize;

use crate::{slug, EvidenceId, EvidenceItem, EvidenceKind, EvidenceSource};

pub const PLANETSCALE_CONNECTOR: &str = "planetscale";
pub const PLANETSCALE_API_BASE: &str = "https://api.planetscale.com/v1";

#[must_use]
pub fn planetscale_allowed_http_methods() -> &'static [&'static str] {
    &["GET"]
}

#[must_use]
pub fn planetscale_forbidden_http_methods() -> &'static [&'static str] {
    &["POST", "PUT", "PATCH", "DELETE"]
}

pub fn ensure_planetscale_read_only_method(method: &str) -> Result<()> {
    if method.eq_ignore_ascii_case("GET") {
        Ok(())
    } else {
        Err(RivoraError::provider(
            PLANETSCALE_CONNECTOR,
            format!("PlanetScale connector rejected non-read-only HTTP method {method}"),
        ))
    }
}

#[derive(Clone)]
pub struct PlanetScaleAuthConfig {
    service_token_id: Option<String>,
    service_token: Option<String>,
    oauth_token: Option<String>,
}

impl std::fmt::Debug for PlanetScaleAuthConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PlanetScaleAuthConfig")
            .field(
                "service_token_id",
                &self.service_token_id.as_ref().map(|_| "[redacted]"),
            )
            .field(
                "service_token",
                &self.service_token.as_ref().map(|_| "[redacted]"),
            )
            .field(
                "oauth_token",
                &self.oauth_token.as_ref().map(|_| "[redacted]"),
            )
            .finish()
    }
}

impl PlanetScaleAuthConfig {
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            service_token_id: nonempty_env("PLANETSCALE_SERVICE_TOKEN_ID"),
            service_token: nonempty_env("PLANETSCALE_SERVICE_TOKEN"),
            oauth_token: nonempty_env("PLANETSCALE_AUTH_TOKEN"),
        }
    }

    #[must_use]
    pub fn with_service_token(
        service_token_id: impl Into<String>,
        service_token: impl Into<String>,
    ) -> Self {
        Self {
            service_token_id: optional_nonempty(service_token_id.into()),
            service_token: optional_nonempty(service_token.into()),
            oauth_token: None,
        }
    }

    #[must_use]
    pub fn with_oauth_token(token: impl Into<String>) -> Self {
        Self {
            service_token_id: None,
            service_token: None,
            oauth_token: optional_nonempty(token.into()),
        }
    }

    #[must_use]
    pub fn has_token(&self) -> bool {
        self.configuration_error().is_none()
            && ((self.service_token_id.is_some() && self.service_token.is_some())
                || self.oauth_token.is_some())
    }

    #[must_use]
    pub fn configuration_error(&self) -> Option<&'static str> {
        match (&self.service_token_id, &self.service_token) {
            (Some(id), Some(token)) => {
                if credential_is_safe(id) && credential_is_safe(token) {
                    None
                } else {
                    Some("PlanetScale service-token credentials contain unsupported characters")
                }
            }
            (Some(_), None) => Some("Missing PLANETSCALE_SERVICE_TOKEN"),
            (None, Some(_)) => Some("Missing PLANETSCALE_SERVICE_TOKEN_ID"),
            (None, None) => self.oauth_token.as_ref().and_then(|token| {
                (!credential_is_safe(token))
                    .then_some("PLANETSCALE_AUTH_TOKEN contains unsupported characters")
            }),
        }
    }

    fn authorization_header_value(&self) -> Result<String> {
        if let Some(error) = self.configuration_error() {
            return Err(RivoraError::invalid_value("planetscale_auth", error));
        }
        if let (Some(id), Some(token)) = (&self.service_token_id, &self.service_token) {
            return Ok(format!("{id}:{token}"));
        }
        self.oauth_token
            .as_ref()
            .map(|token| format!("Bearer {token}"))
            .ok_or_else(|| {
                RivoraError::invalid_value(
                    "planetscale_auth",
                    "PlanetScale credentials are not configured",
                )
            })
    }

    #[must_use]
    pub fn redact(&self, value: &str) -> String {
        let mut value = value.to_string();
        for credential in [
            self.service_token_id.as_deref(),
            self.service_token.as_deref(),
            self.oauth_token.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            value = value.replace(credential, "[redacted]");
        }
        redact_token_like_values(&value)
    }
}

fn nonempty_env(name: &str) -> Option<String> {
    std::env::var(name).ok().and_then(optional_nonempty)
}

fn optional_nonempty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

fn credential_is_safe(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        })
}

fn redact_token_like_values(value: &str) -> String {
    let parts = value.split_whitespace().collect::<Vec<_>>();
    let mut output = Vec::with_capacity(parts.len());
    let mut index = 0;
    while index < parts.len() {
        let part = parts[index];
        let trimmed = trim_sensitive(part);
        if trimmed.eq_ignore_ascii_case("bearer") {
            output.push("[redacted]".to_string());
            index += usize::from(index + 1 < parts.len()) + 1;
            continue;
        }
        if is_token_like(trimmed) {
            output.push(part.replace(trimmed, "[redacted]"));
        } else {
            output.push(part.to_string());
        }
        index += 1;
    }
    output.join(" ")
}

fn is_token_like(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.starts_with("planetscale_service_token=")
        || lower.starts_with("planetscale_service_token_id=")
        || lower.starts_with("planetscale_auth_token=")
        || lower.starts_with("pscale_tkn_")
        || lower.starts_with("pscale_oauth_")
        || lower.starts_with("access_token=")
        || lower.starts_with("service_token=")
        || lower.starts_with("api_key=")
        || lower.starts_with("password=")
        || lower.starts_with("branch_password=")
        || lower.starts_with("connection_string=")
        || lower.starts_with("dsn=")
        || lower.starts_with("mysql://")
        || lower.starts_with("mysqls://")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanetScaleDatabaseRef {
    pub org: String,
    pub database: String,
}

impl PlanetScaleDatabaseRef {
    #[must_use]
    pub fn new(org: impl Into<String>, database: impl Into<String>) -> Self {
        Self {
            org: org.into(),
            database: database.into(),
        }
    }

    #[must_use]
    pub fn repository_label(&self) -> String {
        format!("{}/{}", self.org, self.database)
    }
}

pub trait PlanetScaleClient {
    fn fetch_branches(&self, database: &PlanetScaleDatabaseRef, limit: usize) -> Result<String>;
    fn fetch_deploy_requests(
        &self,
        database: &PlanetScaleDatabaseRef,
        limit: usize,
    ) -> Result<String>;
}

#[derive(Debug, Clone)]
pub struct HttpPlanetScaleClient {
    auth: PlanetScaleAuthConfig,
    base_url: String,
}

impl HttpPlanetScaleClient {
    #[must_use]
    pub fn new(auth: PlanetScaleAuthConfig) -> Self {
        Self {
            auth,
            base_url: PLANETSCALE_API_BASE.to_string(),
        }
    }

    pub(crate) fn request_config(&self, path: &str) -> Result<String> {
        let mut config = format!(
            "url = \"{}{}\"\nsilent\nshow-error\nfail\nrequest = \"GET\"\nheader = \"Accept: application/json\"\nheader = \"User-Agent: rivora-connectors\"\n",
            self.base_url, path
        );
        let authorization = self.auth.authorization_header_value()?;
        config.push_str(&format!("header = \"Authorization: {authorization}\"\n"));
        Ok(config)
    }

    fn database_path(
        &self,
        database: &PlanetScaleDatabaseRef,
        suffix: &str,
        limit: usize,
    ) -> String {
        format!(
            "/organizations/{}/databases/{}/{}?page=1&per_page={}",
            url_encode(&database.org),
            url_encode(&database.database),
            suffix,
            clamp_limit(limit)
        )
    }

    fn get(&self, path: &str) -> Result<String> {
        ensure_planetscale_read_only_method("GET")?;
        let config = self.request_config(path)?;
        let mut command = Command::new("curl");
        command.args(planetscale_curl_args());
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                RivoraError::provider(PLANETSCALE_CONNECTOR, format!("curl unavailable: {error}"))
            })?;
        child
            .stdin
            .take()
            .ok_or_else(|| {
                RivoraError::provider(PLANETSCALE_CONNECTOR, "could not open curl stdin pipe")
            })?
            .write_all(config.as_bytes())
            .map_err(|error| {
                RivoraError::provider(
                    PLANETSCALE_CONNECTOR,
                    format!("curl config write failed: {error}"),
                )
            })?;
        let output = child.wait_with_output().map_err(|error| {
            RivoraError::provider(
                PLANETSCALE_CONNECTOR,
                format!("curl did not finish: {error}"),
            )
        })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RivoraError::provider(
                PLANETSCALE_CONNECTOR,
                format!(
                    "PlanetScale API request failed for {}: {}",
                    self.auth.redact(path),
                    self.auth.redact(stderr.as_ref()).trim()
                ),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[must_use]
fn planetscale_curl_args() -> &'static [&'static str] {
    &["-q", "--proto", "=https", "--config", "-"]
}

impl PlanetScaleClient for HttpPlanetScaleClient {
    fn fetch_branches(&self, database: &PlanetScaleDatabaseRef, limit: usize) -> Result<String> {
        self.get(&self.database_path(database, "branches", limit))
    }

    fn fetch_deploy_requests(
        &self,
        database: &PlanetScaleDatabaseRef,
        limit: usize,
    ) -> Result<String> {
        self.get(&self.database_path(database, "deploy-requests", limit))
    }
}

#[derive(Debug, Clone, Default)]
pub struct FixturePlanetScaleClient {
    branches: Option<String>,
    deploy_requests: Option<String>,
}

impl FixturePlanetScaleClient {
    #[must_use]
    pub fn builder() -> FixturePlanetScaleClientBuilder {
        FixturePlanetScaleClientBuilder::default()
    }
}

impl PlanetScaleClient for FixturePlanetScaleClient {
    fn fetch_branches(&self, _database: &PlanetScaleDatabaseRef, _limit: usize) -> Result<String> {
        self.branches.clone().ok_or_else(|| {
            RivoraError::provider(PLANETSCALE_CONNECTOR, "no fixture loaded for branches")
        })
    }

    fn fetch_deploy_requests(
        &self,
        _database: &PlanetScaleDatabaseRef,
        _limit: usize,
    ) -> Result<String> {
        self.deploy_requests.clone().ok_or_else(|| {
            RivoraError::provider(
                PLANETSCALE_CONNECTOR,
                "no fixture loaded for deploy requests",
            )
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct FixturePlanetScaleClientBuilder {
    branches: Option<String>,
    deploy_requests: Option<String>,
}

impl FixturePlanetScaleClientBuilder {
    #[must_use]
    pub fn branches(mut self, fixture: impl Into<String>) -> Self {
        self.branches = Some(fixture.into());
        self
    }

    #[must_use]
    pub fn deploy_requests(mut self, fixture: impl Into<String>) -> Self {
        self.deploy_requests = Some(fixture.into());
        self
    }

    #[must_use]
    pub fn build(self) -> FixturePlanetScaleClient {
        FixturePlanetScaleClient {
            branches: self.branches,
            deploy_requests: self.deploy_requests,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanetScaleIngestRequest {
    pub database: PlanetScaleDatabaseRef,
    pub limit: usize,
    pub since: Option<String>,
    pub branch: Option<String>,
}

impl PlanetScaleIngestRequest {
    #[must_use]
    pub fn new(database: PlanetScaleDatabaseRef) -> Self {
        Self {
            database,
            limit: 20,
            since: None,
            branch: None,
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
    pub fn with_branch(mut self, branch: impl Into<String>) -> Self {
        self.branch = Some(branch.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlanetScaleIngestResult {
    pub repository: String,
    pub evidence: Vec<EvidenceItem>,
    pub branches: usize,
    pub deploy_requests: usize,
    pub topics: Vec<String>,
}

pub struct PlanetScaleConnector {
    client: Box<dyn PlanetScaleClient>,
}

impl std::fmt::Debug for PlanetScaleConnector {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PlanetScaleConnector")
            .finish_non_exhaustive()
    }
}

impl PlanetScaleConnector {
    #[must_use]
    pub fn new(client: impl PlanetScaleClient + 'static) -> Self {
        Self {
            client: Box::new(client),
        }
    }

    pub fn ingest(&self, request: PlanetScaleIngestRequest) -> Result<PlanetScaleIngestResult> {
        if request.limit == 0 {
            return Err(RivoraError::invalid_value(
                "limit",
                "PlanetScale limit must be positive",
            ));
        }
        let limit = clamp_limit(request.limit);
        validate_target(&request.database)?;
        let cutoff = request
            .since
            .as_deref()
            .map(parse_since_cutoff)
            .transpose()?;
        let repository = request.database.repository_label();
        let source = EvidenceSource::planetscale(repository.clone());
        let branches = parse_branch_list(&self.client.fetch_branches(&request.database, limit)?)?;
        let deploy_requests = parse_deploy_request_list(
            &self
                .client
                .fetch_deploy_requests(&request.database, limit)?,
        )?;

        let branch_filter = request.branch.as_deref();
        let mut evidence = branches
            .iter()
            .filter(|branch| branch_filter.is_none_or(|name| branch.name.as_deref() == Some(name)))
            .map(|branch| branch_item(&source, &request.database, branch))
            .chain(
                deploy_requests
                    .iter()
                    .filter(|deploy_request| {
                        branch_filter.is_none_or(|name| {
                            deploy_request.branch.as_deref() == Some(name)
                                || deploy_request.into_branch.as_deref() == Some(name)
                        })
                    })
                    .map(|deploy_request| {
                        deploy_request_item(&source, &request.database, deploy_request)
                    }),
            )
            .collect::<Result<Vec<_>>>()?;
        if let Some(cutoff) = cutoff {
            evidence.retain(|item| evidence_is_after_cutoff(item, cutoff));
        }
        let mut evidence_by_id = std::collections::BTreeMap::new();
        for item in evidence {
            match evidence_by_id.entry(item.id.clone()) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(item);
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    if evidence_winner_key(&item) > evidence_winner_key(entry.get()) {
                        entry.insert(item);
                    }
                }
            }
        }
        let mut evidence = evidence_by_id.into_values().collect::<Vec<_>>();
        evidence.sort_by(|left, right| {
            right
                .timestamp
                .cmp(&left.timestamp)
                .then_with(|| left.id.cmp(&right.id))
        });
        evidence.truncate(limit);

        let branches = evidence
            .iter()
            .filter(|item| item.kind == EvidenceKind::PlanetScaleDatabaseBranch)
            .count();
        let deploy_requests = evidence
            .iter()
            .filter(|item| item.kind == EvidenceKind::PlanetScaleDeployRequest)
            .count();
        let mut topics = std::collections::BTreeSet::new();
        for item in &evidence {
            if let Some(service) = &item.service {
                topics.insert(service.clone());
            }
            topics.extend(item.tags.iter().cloned());
        }

        Ok(PlanetScaleIngestResult {
            repository,
            evidence,
            branches,
            deploy_requests,
            topics: topics.into_iter().collect(),
        })
    }
}

fn evidence_winner_key(item: &EvidenceItem) -> (&Option<String>, &str, &str, &str) {
    (
        &item.timestamp,
        item.title.as_str(),
        item.summary.as_str(),
        item.body.as_str(),
    )
}

fn validate_target(database: &PlanetScaleDatabaseRef) -> Result<()> {
    if !is_safe_target_identifier(&database.org) || !is_safe_target_identifier(&database.database) {
        return Err(RivoraError::invalid_value(
            "planetscale_target",
            "PlanetScale organization and database must be non-empty slugs",
        ));
    }
    Ok(())
}

fn is_safe_target_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
        && !is_token_like(value)
}

#[derive(Debug, Clone, Deserialize)]
struct ListResponse<T> {
    data: Vec<T>,
}

#[derive(Debug, Clone, Deserialize)]
struct ActorResponse {
    display_name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct BranchResponse {
    name: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
    production: Option<bool>,
    parent_branch: Option<String>,
    actor: Option<ActorResponse>,
    html_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct DeploymentResponse {
    state: Option<String>,
    deployable: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct DeployRequestResponse {
    id: Option<String>,
    number: Option<u64>,
    actor: Option<ActorResponse>,
    branch: Option<String>,
    into_branch: Option<String>,
    approved: Option<bool>,
    state: Option<String>,
    deployment_state: Option<String>,
    deployment: Option<DeploymentResponse>,
    html_url: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
    closed_at: Option<String>,
    deployed_at: Option<String>,
}

fn parse_branch_list(raw: &str) -> Result<Vec<BranchResponse>> {
    serde_json::from_str::<ListResponse<BranchResponse>>(raw)
        .map(|response| response.data)
        .map_err(|error| {
            RivoraError::provider(
                PLANETSCALE_CONNECTOR,
                format!("invalid PlanetScale branch response: {error}"),
            )
        })
}

fn parse_deploy_request_list(raw: &str) -> Result<Vec<DeployRequestResponse>> {
    serde_json::from_str::<ListResponse<DeployRequestResponse>>(raw)
        .map(|response| response.data)
        .map_err(|error| {
            RivoraError::provider(
                PLANETSCALE_CONNECTOR,
                format!("invalid PlanetScale deploy-request response: {error}"),
            )
        })
}

const BRANCH_METADATA_KEYS: &[&str] = &[
    "org",
    "database",
    "branch",
    "branch_role",
    "is_production",
    "is_default",
    "base_branch",
    "created_at",
    "updated_at",
    "actor",
    "permalink",
];

const DEPLOY_REQUEST_METADATA_KEYS: &[&str] = &[
    "org",
    "database",
    "deploy_request_number",
    "deploy_request_id",
    "title",
    "state",
    "status",
    "branch",
    "base_branch",
    "created_at",
    "updated_at",
    "deployed_at",
    "closed_at",
    "actor",
    "review_state",
    "deployability_state",
    "permalink",
];

fn branch_item(
    source: &EvidenceSource,
    database: &PlanetScaleDatabaseRef,
    branch: &BranchResponse,
) -> Result<EvidenceItem> {
    let name = safe_identifier_value(branch.name.as_deref()).ok_or_else(|| {
        RivoraError::invalid_value(
            "planetscale_branch",
            "PlanetScale branch response did not include a safe name",
        )
    })?;
    let role = if branch.production.unwrap_or(false) {
        "production"
    } else {
        "development"
    };
    let created_at = safe_timestamp(branch.created_at.as_deref());
    let updated_at = safe_timestamp(branch.updated_at.as_deref());
    let actor = safe_actor(branch.actor.as_ref());
    let base_branch = safe_identifier_value(branch.parent_branch.as_deref()).unwrap_or_default();
    let permalink = safe_permalink(branch.html_url.as_deref());
    let body = render_metadata(
        BRANCH_METADATA_KEYS,
        &[
            ("org", database.org.clone()),
            ("database", database.database.clone()),
            ("branch", name.clone()),
            ("branch_role", role.to_string()),
            (
                "is_production",
                branch.production.unwrap_or(false).to_string(),
            ),
            ("base_branch", base_branch),
            ("created_at", created_at.clone()),
            ("updated_at", updated_at.clone()),
            ("actor", actor.clone()),
            ("permalink", permalink.clone()),
        ],
    );
    let mut tags = vec![
        "planetscale".to_string(),
        "database".to_string(),
        "schema".to_string(),
        "branch".to_string(),
        slug(&database.database),
        slug(role),
    ];
    tags.sort();
    tags.dedup();
    let mut refs = Vec::new();
    if !permalink.is_empty() {
        refs.push(permalink);
    }
    Ok(EvidenceItem {
        id: EvidenceId::new(format!(
            "planetscale:branch:{}:{}:{}",
            slug(&database.org),
            slug(&database.database),
            slug(&name)
        ))?,
        kind: EvidenceKind::PlanetScaleDatabaseBranch,
        source: source.clone(),
        title: format!("PlanetScale branch {name}"),
        summary: format!(
            "PlanetScale {role} branch {name} in {} was observed",
            database.database
        ),
        body,
        service: Some(slug(&database.database)),
        files_changed: Vec::new(),
        timestamp: nonempty(updated_at).or_else(|| nonempty(created_at)),
        author: nonempty(actor),
        tags,
        refs,
        confidence: 0.85,
    })
}

fn deploy_request_item(
    source: &EvidenceSource,
    database: &PlanetScaleDatabaseRef,
    deploy_request: &DeployRequestResponse,
) -> Result<EvidenceItem> {
    let number = deploy_request.number.ok_or_else(|| {
        RivoraError::invalid_value(
            "planetscale_deploy_request",
            "PlanetScale deploy request did not include a number",
        )
    })?;
    let id = safe_identifier_value(deploy_request.id.as_deref()).unwrap_or_default();
    let branch = safe_identifier_value(deploy_request.branch.as_deref()).unwrap_or_default();
    let base_branch =
        safe_identifier_value(deploy_request.into_branch.as_deref()).unwrap_or_default();
    let state = safe_identifier_value(deploy_request.state.as_deref()).unwrap_or_default();
    let status = safe_identifier_value(
        deploy_request
            .deployment_state
            .as_deref()
            .or_else(|| deploy_request.deployment.as_ref()?.state.as_deref()),
    )
    .unwrap_or_default();
    let deployability_state = deploy_request
        .deployment
        .as_ref()
        .and_then(|deployment| deployment.deployable)
        .map(|value| {
            if value {
                "deployable"
            } else {
                "not_deployable"
            }
        })
        .unwrap_or("unknown")
        .to_string();
    let review_state = deploy_request
        .approved
        .map(|approved| if approved { "approved" } else { "pending" })
        .unwrap_or("unknown")
        .to_string();
    let created_at = safe_timestamp(deploy_request.created_at.as_deref());
    let updated_at = safe_timestamp(deploy_request.updated_at.as_deref());
    let deployed_at = safe_timestamp(deploy_request.deployed_at.as_deref());
    let closed_at = safe_timestamp(deploy_request.closed_at.as_deref());
    let actor = safe_actor(deploy_request.actor.as_ref());
    let permalink = safe_permalink(deploy_request.html_url.as_deref());
    let body = render_metadata(
        DEPLOY_REQUEST_METADATA_KEYS,
        &[
            ("org", database.org.clone()),
            ("database", database.database.clone()),
            ("deploy_request_number", number.to_string()),
            ("deploy_request_id", id.clone()),
            ("state", state.clone()),
            ("status", status.clone()),
            ("branch", branch.clone()),
            ("base_branch", base_branch),
            ("created_at", created_at.clone()),
            ("updated_at", updated_at.clone()),
            ("deployed_at", deployed_at.clone()),
            ("closed_at", closed_at),
            ("actor", actor.clone()),
            ("review_state", review_state.clone()),
            ("deployability_state", deployability_state.clone()),
            ("permalink", permalink.clone()),
        ],
    );
    let display_state = if !status.is_empty() { &status } else { &state };
    let mut tags = vec![
        "planetscale".to_string(),
        "database".to_string(),
        "schema".to_string(),
        "deploy-request".to_string(),
        slug(&database.database),
        format!("state-{}", slug(display_state)),
        review_state,
        deployability_state,
    ];
    if matches!(
        display_state.to_ascii_lowercase().as_str(),
        "failed" | "cancelled"
    ) {
        tags.push("failed".to_string());
    }
    tags.sort();
    tags.dedup();
    let mut refs = vec![format!("deploy-request:{number}")];
    if !id.is_empty() {
        refs.push(format!("deploy-request-id:{id}"));
    }
    if !permalink.is_empty() {
        refs.push(permalink);
    }
    Ok(EvidenceItem {
        id: EvidenceId::new(format!(
            "planetscale:deploy-request:{}:{}:{}",
            slug(&database.org),
            slug(&database.database),
            number
        ))?,
        kind: EvidenceKind::PlanetScaleDeployRequest,
        source: source.clone(),
        title: format!("PlanetScale deploy request #{number}"),
        summary: format!(
            "PlanetScale deploy request #{number} for {} is {}",
            database.database,
            value_or_unknown(display_state)
        ),
        body,
        service: Some(slug(&database.database)),
        files_changed: Vec::new(),
        timestamp: nonempty(deployed_at)
            .or_else(|| nonempty(updated_at))
            .or_else(|| nonempty(created_at)),
        author: nonempty(actor),
        tags,
        refs,
        confidence: 0.9,
    })
}

fn render_metadata(allowlist: &[&str], metadata: &[(&str, String)]) -> String {
    metadata
        .iter()
        .filter(|(key, _)| allowlist.contains(key))
        .map(|(key, value)| format!("{}: {}", metadata_label(key), value_or_unknown(value)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn metadata_label(key: &str) -> &'static str {
    match key {
        "org" => "Org",
        "database" => "Database",
        "branch" => "Branch",
        "branch_role" => "Branch role",
        "is_production" => "Production",
        "is_default" => "Default",
        "base_branch" => "Base branch",
        "deploy_request_number" => "Deploy request",
        "deploy_request_id" => "Deploy request id",
        "title" => "Title",
        "state" => "State",
        "status" => "Status",
        "created_at" => "Created at",
        "updated_at" => "Updated at",
        "deployed_at" => "Deployed at",
        "closed_at" => "Closed at",
        "actor" => "Actor",
        "review_state" => "Review state",
        "deployability_state" => "Deployability",
        "permalink" => "Permalink",
        _ => "Unknown",
    }
}

fn safe_actor(actor: Option<&ActorResponse>) -> String {
    safe_text(actor.and_then(|actor| actor.display_name.as_deref()))
}

fn safe_text(value: Option<&str>) -> String {
    value
        .map(sanitize_metadata_value)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_default()
}

fn sanitize_metadata_value(value: &str) -> String {
    redact_token_like_values(value)
        .split_whitespace()
        .map(|part| {
            let trimmed = trim_sensitive(part);
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
        || lower.contains("://") && !lower.starts_with("https://")
        || is_ipv4(value)
        || is_token_like(value)
        || lower.starts_with("/users/")
        || lower.starts_with("/home/")
        || lower.contains("password")
        || lower.contains("connection_string")
        || lower.contains(".internal")
        || lower.contains(".local")
        || lower == "localhost"
        || (lower.contains(':')
            && lower
                .chars()
                .all(|character| character.is_ascii_hexdigit() || character == ':'))
}

fn trim_sensitive(value: &str) -> &str {
    value.trim_matches(|character: char| {
        !character.is_ascii_alphanumeric()
            && !matches!(character, '.' | '@' | '_' | '-' | '=' | '/' | ':' | '\\')
    })
}

fn safe_identifier_value(value: Option<&str>) -> Option<String> {
    value
        .filter(|value| {
            !value.is_empty()
                && value.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
                })
                && !is_token_like(value)
        })
        .map(str::to_string)
}

fn safe_permalink(value: Option<&str>) -> String {
    value
        .filter(|value| {
            value.starts_with("https://app.planetscale.com/")
                && !value.contains(['@', '?', '#', '\n', '\r'])
        })
        .map(sanitize_metadata_value)
        .unwrap_or_default()
}

fn safe_timestamp(value: Option<&str>) -> String {
    value
        .filter(|value| parse_timestamp(value).is_some())
        .map(str::to_string)
        .unwrap_or_default()
}

fn nonempty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

fn value_or_unknown(value: &str) -> &str {
    if value.is_empty() {
        "unknown"
    } else {
        value
    }
}

fn is_ipv4(value: &str) -> bool {
    let parts = value.split('.').collect::<Vec<_>>();
    parts.len() == 4
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.parse::<u8>().is_ok())
}

fn evidence_is_after_cutoff(item: &EvidenceItem, cutoff: i64) -> bool {
    item.timestamp
        .as_deref()
        .and_then(parse_timestamp)
        .is_some_and(|timestamp| timestamp >= cutoff)
}

fn parse_since_cutoff(value: &str) -> Result<i64> {
    let trimmed = value.trim();
    if let Some((amount, multiplier)) = relative_duration(trimmed) {
        let amount = amount.parse::<i64>().map_err(|_| since_error())?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| {
                RivoraError::invalid_value("planetscale_since", "system clock is before unix epoch")
            })?
            .as_secs() as i64;
        return Ok(now - amount.saturating_mul(multiplier));
    }
    parse_timestamp(trimmed).ok_or_else(since_error)
}

fn since_error() -> RivoraError {
    RivoraError::invalid_value(
        "planetscale_since",
        "use an ISO timestamp or duration like 24h or 7d",
    )
}

fn relative_duration(value: &str) -> Option<(&str, i64)> {
    value
        .strip_suffix('h')
        .map(|amount| (amount, 3_600))
        .or_else(|| value.strip_suffix('d').map(|amount| (amount, 86_400)))
}

fn parse_timestamp(value: &str) -> Option<i64> {
    let date_time = value.trim().strip_suffix('Z').unwrap_or(value.trim());
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

fn clamp_limit(limit: usize) -> usize {
    limit.clamp(1, 100)
}

fn url_encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const BRANCHES: &str = r#"{
        "type":"list",
        "data":[{
            "id":"branch-secret-id",
            "name":"main",
            "production":true,
            "parent_branch":"development",
            "created_at":"2026-07-01T10:00:00Z",
            "updated_at":"2026-07-01T11:00:00Z",
            "html_url":"https://app.planetscale.com/demo-org/checkout-db/main",
            "actor":{"display_name":"Database Engineer"},
            "mysql_address":"private.internal",
            "region":{"public_ip_addresses":["192.0.2.1"]},
            "password":"secret-password"
        }]
    }"#;

    const DEPLOY_REQUESTS: &str = r#"{
        "data":[{
            "id":"dr_42",
            "number":42,
            "branch":"checkout-schema-change",
            "into_branch":"main",
            "approved":true,
            "state":"closed",
            "deployment_state":"complete",
            "deployment":{"state":"complete","deployable":true,"deploy_operations":[{"ddl_statement":"ALTER TABLE users ADD COLUMN token varchar(255)"}]},
            "created_at":"2026-07-01T10:30:00Z",
            "updated_at":"2026-07-01T11:30:00Z",
            "deployed_at":"2026-07-01T11:25:00Z",
            "html_url":"https://app.planetscale.com/demo-org/checkout-db/deploy-requests/42",
            "actor":{"display_name":"Schema Reviewer"},
            "connection_string":"mysql://user:password@host/database",
            "branch_password":"secret-branch-password",
            "dsn":"mysql://user:password@host/database",
            "host":"private-host.internal",
            "username":"user@example.com",
            "email":"person@example.com",
            "ip_address":"192.0.2.1",
            "schema":"CREATE TABLE users (email varchar(255), password varchar(255))",
            "rows":[{"email":"customer@example.com","password":"secret"}],
            "query_result":[{"token":"secret-query-token"}],
            "metadata":{"api_key":"secret-api-key"},
            "schema_diff":"ALTER TABLE users ADD COLUMN ssn varchar(255)"
        }]
    }"#;

    fn connector() -> PlanetScaleConnector {
        PlanetScaleConnector::new(
            FixturePlanetScaleClient::builder()
                .branches(BRANCHES)
                .deploy_requests(DEPLOY_REQUESTS)
                .build(),
        )
    }

    fn request() -> PlanetScaleIngestRequest {
        PlanetScaleIngestRequest::new(PlanetScaleDatabaseRef::new("demo-org", "checkout-db"))
    }

    #[test]
    fn parses_branches_and_deploy_requests_with_stable_ids() {
        let result = connector().ingest(request()).unwrap();
        assert_eq!(result.branches, 1);
        assert_eq!(result.deploy_requests, 1);
        assert!(result
            .evidence
            .iter()
            .any(|item| { item.id.as_str() == "planetscale:branch:demo-org:checkout-db:main" }));
        assert!(result.evidence.iter().any(|item| {
            item.id.as_str() == "planetscale:deploy-request:demo-org:checkout-db:42"
        }));
    }

    #[test]
    fn sensitive_and_arbitrary_fields_are_not_persisted() {
        let result = connector().ingest(request()).unwrap();
        let serialized = serde_json::to_string(&result.evidence).unwrap();
        for forbidden in [
            "branch-secret-id",
            "private.internal",
            "192.0.2.1",
            "secret-password",
            "mysql://",
            "secret-branch-password",
            "private-host.internal",
            "user@example.com",
            "person@example.com",
            "customer@example.com",
            "ALTER TABLE",
            "CREATE TABLE",
            "secret-query-token",
            "secret-api-key",
            "schema_diff",
            "ddl_statement",
            "rows",
        ] {
            assert!(
                !serialized.contains(forbidden),
                "persisted {forbidden}: {serialized}"
            );
        }
    }

    #[test]
    fn metadata_allowlists_are_exact_and_narrow() {
        assert_eq!(
            BRANCH_METADATA_KEYS,
            &[
                "org",
                "database",
                "branch",
                "branch_role",
                "is_production",
                "is_default",
                "base_branch",
                "created_at",
                "updated_at",
                "actor",
                "permalink"
            ]
        );
        assert_eq!(
            DEPLOY_REQUEST_METADATA_KEYS,
            &[
                "org",
                "database",
                "deploy_request_number",
                "deploy_request_id",
                "title",
                "state",
                "status",
                "branch",
                "base_branch",
                "created_at",
                "updated_at",
                "deployed_at",
                "closed_at",
                "actor",
                "review_state",
                "deployability_state",
                "permalink"
            ]
        );
    }

    #[test]
    fn branch_and_since_filters_apply_locally() {
        assert!(connector()
            .ingest(request().with_branch("missing"))
            .unwrap()
            .evidence
            .is_empty());
        assert_eq!(
            connector()
                .ingest(request().with_branch("main"))
                .unwrap()
                .evidence
                .len(),
            2
        );
        assert!(connector()
            .ingest(request().with_since("2026-07-02T00:00:00Z"))
            .unwrap()
            .evidence
            .is_empty());
        assert!(connector()
            .ingest(request().with_since("recently"))
            .is_err());
    }

    #[test]
    fn get_only_guard_and_request_config_reject_mutations() {
        assert_eq!(planetscale_allowed_http_methods(), &["GET"]);
        for method in planetscale_forbidden_http_methods() {
            assert!(ensure_planetscale_read_only_method(method).is_err());
        }
        let client = HttpPlanetScaleClient::new(PlanetScaleAuthConfig::with_service_token(
            "token-id", "secret",
        ));
        let config = client
            .request_config("/organizations/org/databases/db/branches")
            .unwrap();
        assert!(config.contains("request = \"GET\""));
        assert!(config.contains("Authorization: token-id:secret"));
        assert!(!config.contains("Bearer secret"));
        assert!(!config.contains("\"POST\""));
        assert_eq!(
            planetscale_curl_args(),
            &["-q", "--proto", "=https", "--config", "-"]
        );
    }

    #[test]
    fn auth_is_redacted_and_prefers_complete_service_token_credentials() {
        let auth = PlanetScaleAuthConfig::with_service_token("token-id", "pscale_tkn_secret");
        assert!(!format!("{auth:?}").contains("pscale_tkn_secret"));
        assert!(!format!("{auth:?}").contains("token-id"));
        assert_eq!(auth.redact("Bearer pscale_tkn_secret"), "[redacted]");

        std::env::set_var("PLANETSCALE_SERVICE_TOKEN_ID", "primary-id");
        std::env::set_var("PLANETSCALE_AUTH_TOKEN", "fallback");
        std::env::set_var("PLANETSCALE_SERVICE_TOKEN", "primary");
        let auth = PlanetScaleAuthConfig::from_env();
        assert_eq!(
            auth.redact("primary-id primary fallback"),
            "[redacted] [redacted] [redacted]"
        );
        let config = HttpPlanetScaleClient::new(auth)
            .request_config("/organizations/org/databases/db/branches")
            .unwrap();
        assert!(config.contains("Authorization: primary-id:primary"));
        assert!(!config.contains("fallback"));
        std::env::remove_var("PLANETSCALE_SERVICE_TOKEN_ID");
        std::env::remove_var("PLANETSCALE_SERVICE_TOKEN");
        std::env::remove_var("PLANETSCALE_AUTH_TOKEN");
    }

    #[test]
    fn oauth_fallback_uses_bearer_and_incomplete_service_credentials_are_invalid() {
        let oauth = PlanetScaleAuthConfig::with_oauth_token("pscale_oauth_secret");
        let config = HttpPlanetScaleClient::new(oauth)
            .request_config("/organizations/org/databases/db/branches")
            .unwrap();
        assert!(config.contains("Authorization: Bearer pscale_oauth_secret"));

        assert!(PlanetScaleAuthConfig::with_service_token("", "secret")
            .configuration_error()
            .is_some());
        assert!(PlanetScaleAuthConfig::with_service_token("token-id", "")
            .configuration_error()
            .is_some());
    }

    #[test]
    fn credential_like_values_and_private_permalinks_are_redacted_or_omitted() {
        let auth = PlanetScaleAuthConfig::with_service_token("configured-id", "configured-token");
        let redacted = auth.redact(
            "configured-id configured-token pscale_oauth_secret access_token=secret service_token=secret api_key=secret password=secret branch_password=secret connection_string=mysql://user:password@private.internal/db",
        );
        for forbidden in [
            "configured-id",
            "configured-token",
            "pscale_oauth_secret",
            "access_token=secret",
            "service_token=secret",
            "api_key=secret",
            "password=secret",
            "branch_password=secret",
            "mysql://",
            "private.internal",
        ] {
            assert!(
                !redacted.contains(forbidden),
                "leaked {forbidden}: {redacted}"
            );
        }
        assert_eq!(
            safe_permalink(Some("https://private.internal/deploy/42")),
            ""
        );
        assert_eq!(
            safe_permalink(Some(
                "https://app.planetscale.com/org/db/deploy-requests/42?token=secret"
            )),
            ""
        );
    }

    #[test]
    fn malformed_list_responses_fail_closed() {
        let malformed = PlanetScaleConnector::new(
            FixturePlanetScaleClient::builder()
                .branches("{}")
                .deploy_requests(r#"{"data":[]}"#)
                .build(),
        );
        assert!(malformed.ingest(request()).is_err());
    }

    #[test]
    fn poisoned_allowlisted_fields_are_omitted_or_redacted() {
        let client = FixturePlanetScaleClient::builder()
            .branches(
                r#"{"data":[{"name":"safe-branch","parent_branch":"mysql://user:password@private.internal/db","actor":{"display_name":"api_key=actor-secret private.internal 2001:db8::1"},"html_url":"https://private.internal/branch?access_token=secret","updated_at":"2026-07-01T11:00:00Z"}]}"#,
            )
            .deploy_requests(
                r#"{"data":[{"id":"dr_99","number":99,"branch":"ALTER TABLE users","into_branch":"pscale_oauth_secret","state":"api_key=state-secret","deployment_state":"password=state-secret","actor":{"display_name":"person@example.com"},"html_url":"https://app.planetscale.com/org/db/deploy-requests/99?access_token=secret","updated_at":"2026-07-01T12:00:00Z"}]}"#,
            )
            .build();
        let result = PlanetScaleConnector::new(client).ingest(request()).unwrap();
        let serialized = serde_json::to_string(&result.evidence).unwrap();
        for forbidden in [
            "mysql://",
            "private.internal",
            "actor-secret",
            "2001:db8::1",
            "access_token",
            "ALTER TABLE",
            "pscale_oauth_secret",
            "state-secret",
            "person@example.com",
        ] {
            assert!(
                !serialized.contains(forbidden),
                "leaked {forbidden}: {serialized}"
            );
        }
    }

    #[test]
    fn duplicate_ids_choose_newest_evidence_independent_of_provider_order() {
        let branch_old = r#"{"name":"main","production":true,"updated_at":"2026-07-01T09:00:00Z"}"#;
        let branch_other =
            r#"{"name":"other","production":false,"updated_at":"2026-07-01T10:00:00Z"}"#;
        let branch_new =
            r#"{"name":"main","production":false,"updated_at":"2026-07-01T11:00:00Z"}"#;
        let ingest = |items: &str| {
            PlanetScaleConnector::new(
                FixturePlanetScaleClient::builder()
                    .branches(format!(r#"{{"data":[{items}]}}"#))
                    .deploy_requests(r#"{"data":[]}"#)
                    .build(),
            )
            .ingest(request())
            .unwrap()
        };
        let first = ingest(&format!("{branch_old},{branch_other},{branch_new}"));
        let second = ingest(&format!("{branch_new},{branch_other},{branch_old}"));
        assert_eq!(first.evidence, second.evidence);
        assert_eq!(first.evidence.len(), 2);
        let main = first
            .evidence
            .iter()
            .find(|item| item.id.as_str().ends_with(":main"))
            .unwrap();
        assert_eq!(main.timestamp.as_deref(), Some("2026-07-01T11:00:00Z"));
        assert!(main.body.contains("Branch role: development"));
    }

    #[test]
    fn empty_responses_and_invalid_limits_are_deterministic() {
        let empty = PlanetScaleConnector::new(
            FixturePlanetScaleClient::builder()
                .branches(r#"{"data":[]}"#)
                .deploy_requests(r#"{"data":[]}"#)
                .build(),
        );
        assert!(empty.ingest(request()).unwrap().evidence.is_empty());
        assert!(connector().ingest(request().with_limit(0)).is_err());
        assert_eq!(
            connector()
                .ingest(request().with_limit(250))
                .unwrap()
                .evidence
                .len(),
            2
        );
    }
}
