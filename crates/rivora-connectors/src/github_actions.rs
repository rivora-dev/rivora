//! Read-only GitHub Actions / CI connector (RFC-012, v0.3).
//!
//! Observes workflow runs and jobs. Never mutates GitHub.

use chrono::{DateTime, Utc};
use rivora::domain::ObservationKind;
use serde_json::{json, Value};

use crate::{ConnectorError, ConnectorResult, NormalizedObservation};

/// Read-only GitHub Actions connector.
#[derive(Debug, Clone)]
pub struct GitHubActionsConnector {
    /// `owner/repo`
    pub repository: String,
    /// Optional token (`GITHUB_TOKEN`).
    pub token: Option<String>,
    /// API base URL.
    pub api_base: String,
    /// Max workflow runs to collect.
    pub limit: usize,
}

impl GitHubActionsConnector {
    /// Create a connector for `owner/repo`.
    pub fn new(repository: impl Into<String>) -> Self {
        Self {
            repository: repository.into(),
            token: std::env::var("GITHUB_TOKEN").ok().filter(|s| !s.is_empty()),
            api_base: "https://api.github.com".into(),
            limit: 10,
        }
    }

    /// Override token.
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Configuration / credential status (no secrets returned).
    pub fn status(&self) -> ConnectorStatusReport {
        ConnectorStatusReport {
            id: "github_actions".into(),
            category: "ci".into(),
            configured: self.token.is_some(),
            read_only: true,
            details: if self.token.is_some() {
                format!("repository={} token=present", self.repository)
            } else {
                format!(
                    "repository={} token=missing (set GITHUB_TOKEN for live observe)",
                    self.repository
                )
            },
        }
    }

    /// Test configuration without mutating anything.
    pub fn test_configuration(&self) -> ConnectorResult<String> {
        if self.repository.split('/').count() != 2 {
            return Err(ConnectorError::Config(
                "repository must be owner/repo".into(),
            ));
        }
        if self.token.is_none() {
            return Ok(
                "github_actions: repository valid; GITHUB_TOKEN missing (fixture mode available)"
                    .into(),
            );
        }
        Ok("github_actions: repository valid; token present (read-only)".into())
    }

    /// Observe live workflow runs (read-only).
    pub fn observe(&self) -> ConnectorResult<Vec<NormalizedObservation>> {
        let token = self.token.as_ref().ok_or_else(|| {
            ConnectorError::Config("GITHUB_TOKEN required for live GitHub Actions observe".into())
        })?;
        let client = reqwest::blocking::Client::builder()
            .user_agent("rivora-github-actions-connector")
            .build()
            .map_err(|e| ConnectorError::Api(e.to_string()))?;

        let url = format!(
            "{}/repos/{}/actions/runs?per_page={}",
            self.api_base.trim_end_matches('/'),
            self.repository,
            self.limit
        );
        let mut req = client
            .get(&url)
            .header("Accept", "application/vnd.github+json");
        req = req.bearer_auth(token);
        let response = req.send().map_err(|e| ConnectorError::Api(e.to_string()))?;
        if response.status().as_u16() == 403 {
            let body = response.text().unwrap_or_default();
            if body.to_lowercase().contains("rate limit") {
                return Err(ConnectorError::Api(
                    "rate limited by GitHub Actions API (no secrets leaked)".into(),
                ));
            }
            return Err(ConnectorError::Api(
                "GitHub Actions API forbidden (credentials or permissions)".into(),
            ));
        }
        if !response.status().is_success() {
            return Err(ConnectorError::Api(format!(
                "GitHub Actions API status {}",
                response.status()
            )));
        }
        let body: Value = response
            .json()
            .map_err(|e| ConnectorError::Normalize(e.to_string()))?;
        Self::normalize_runs(&self.repository, &body)
    }

    /// Observe from a fixture JSON document.
    pub fn observe_from_fixture(fixture: &Value) -> ConnectorResult<Vec<NormalizedObservation>> {
        let repo = fixture
            .get("repository")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown/repo");
        if let Some(runs) = fixture.get("workflow_runs") {
            let wrapper = json!({ "workflow_runs": runs });
            return Self::normalize_runs(repo, &wrapper);
        }
        // Accept a bare list.
        if fixture.is_array() {
            let wrapper = json!({ "workflow_runs": fixture });
            return Self::normalize_runs(repo, &wrapper);
        }
        Err(ConnectorError::Normalize(
            "fixture must include workflow_runs array or be an array of runs".into(),
        ))
    }

    fn normalize_runs(repo: &str, body: &Value) -> ConnectorResult<Vec<NormalizedObservation>> {
        let runs = body
            .get("workflow_runs")
            .and_then(|v| v.as_array())
            .ok_or_else(|| ConnectorError::Normalize("missing workflow_runs array".into()))?;
        let mut out = Vec::new();
        for run in runs {
            let id = run
                .get("id")
                .and_then(|v| v.as_u64())
                .map(|n| n.to_string())
                .unwrap_or_else(|| "unknown".into());
            let name = run
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("workflow");
            let conclusion = run
                .get("conclusion")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let status = run
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let event = run
                .get("event")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let observed_at = run
                .get("updated_at")
                .or_else(|| run.get("created_at"))
                .and_then(|v| v.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(Utc::now);

            // Redact any accidental token-like fields.
            let mut payload = run.clone();
            if let Some(obj) = payload.as_object_mut() {
                for key in ["token", "secret", "authorization", "password"] {
                    if let Some(val) = obj.get_mut(key) {
                        *val = json!("[redacted]");
                    }
                }
            }

            out.push(NormalizedObservation::new(
                ObservationKind::WorkflowRun,
                format!(
                    "GitHub Actions workflow `{name}` on `{repo}` status={status} conclusion={conclusion} event={event}"
                ),
                payload,
                "github_actions",
                observed_at,
                Some(format!("github-actions-run:{repo}:{id}")),
                "github-actions-connector",
            ));

            if let Some(jobs) = run.get("jobs").and_then(|v| v.as_array()) {
                for job in jobs {
                    let job_id = job
                        .get("id")
                        .and_then(|v| v.as_u64())
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "unknown".into());
                    let job_name = job.get("name").and_then(|v| v.as_str()).unwrap_or("job");
                    let job_conclusion = job
                        .get("conclusion")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    out.push(NormalizedObservation::new(
                        ObservationKind::CheckResult,
                        format!(
                            "GitHub Actions job `{job_name}` conclusion={job_conclusion} (run {id})"
                        ),
                        job.clone(),
                        "github_actions",
                        observed_at,
                        Some(format!("github-actions-job:{repo}:{job_id}")),
                        "github-actions-connector",
                    ));
                }
            }
        }
        if out.is_empty() {
            return Err(ConnectorError::Normalize(
                "no workflow runs found to normalize".into(),
            ));
        }
        Ok(out)
    }
}

/// Lightweight connector status without secrets.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConnectorStatusReport {
    /// Connector id.
    pub id: String,
    /// Category: ci | infrastructure | observability | code | local.
    pub category: String,
    /// Whether credentials appear configured.
    pub configured: bool,
    /// Always true in v0.3.
    pub read_only: bool,
    /// Human-readable details (redacted).
    pub details: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fixture_normalizes_runs_and_jobs() {
        let fixture = json!({
            "repository": "acme/app",
            "workflow_runs": [{
                "id": 42,
                "name": "CI",
                "status": "completed",
                "conclusion": "failure",
                "event": "push",
                "updated_at": "2026-01-01T00:00:00Z",
                "token": "should-not-leak",
                "jobs": [{
                    "id": 7,
                    "name": "test",
                    "conclusion": "failure"
                }]
            }]
        });
        let obs = GitHubActionsConnector::observe_from_fixture(&fixture).unwrap();
        assert_eq!(obs.len(), 2);
        assert!(matches!(obs[0].kind, ObservationKind::WorkflowRun));
        assert!(obs[0].summary.contains("failure"));
        assert_eq!(obs[0].payload["token"], json!("[redacted]"));
        assert!(matches!(obs[1].kind, ObservationKind::CheckResult));
        assert_eq!(
            obs[0].idempotency_key.as_deref(),
            Some("github-actions-run:acme/app:42")
        );
    }

    #[test]
    fn missing_token_errors_on_live_observe() {
        let c = GitHubActionsConnector {
            repository: "a/b".into(),
            token: None,
            api_base: "https://api.github.com".into(),
            limit: 1,
        };
        let err = c.observe().unwrap_err();
        assert!(matches!(err, ConnectorError::Config(_)));
    }

    #[test]
    fn malformed_fixture_errors() {
        let err = GitHubActionsConnector::observe_from_fixture(&json!({"nope": true})).unwrap_err();
        assert!(matches!(err, ConnectorError::Normalize(_)));
    }

    #[test]
    fn status_never_includes_token_value() {
        let c = GitHubActionsConnector::new("a/b").with_token("super-secret");
        let s = c.status();
        assert!(s.configured);
        assert!(s.read_only);
        assert!(!s.details.contains("super-secret"));
    }
}
