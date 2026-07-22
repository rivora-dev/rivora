//! Read-only Sentry connector (RFC-012, v0.3).
//!
//! Observes issues and error events. Never mutates Sentry (no resolve,
//! no assign, no comment).

use chrono::{DateTime, Utc};
use rivora::domain::ObservationKind;
use serde_json::{json, Value};

use crate::github_actions::ConnectorStatusReport;
use crate::{ConnectorError, ConnectorResult, NormalizedObservation};

/// Read-only Sentry connector.
#[derive(Debug, Clone)]
pub struct SentryConnector {
    /// Organization slug.
    pub organization: String,
    /// Project slug.
    pub project: String,
    /// Auth token from `SENTRY_AUTH_TOKEN` (never logged).
    pub token: Option<String>,
    /// API base.
    pub api_base: String,
    /// Max issues.
    pub limit: usize,
}

impl SentryConnector {
    /// Create a connector for org/project.
    pub fn new(organization: impl Into<String>, project: impl Into<String>) -> Self {
        Self {
            organization: organization.into(),
            project: project.into(),
            token: std::env::var("SENTRY_AUTH_TOKEN")
                .ok()
                .filter(|s| !s.is_empty()),
            api_base: "https://sentry.io/api/0".into(),
            limit: 20,
        }
    }

    /// Override token.
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Status without secrets.
    pub fn status(&self) -> ConnectorStatusReport {
        ConnectorStatusReport {
            id: "sentry".into(),
            category: "observability".into(),
            configured: self.token.is_some(),
            read_only: true,
            details: format!(
                "org={} project={} token={}",
                self.organization,
                self.project,
                if self.token.is_some() {
                    "present"
                } else {
                    "missing"
                }
            ),
        }
    }

    /// Test configuration.
    pub fn test_configuration(&self) -> ConnectorResult<String> {
        if self.organization.trim().is_empty() || self.project.trim().is_empty() {
            return Err(ConnectorError::Config(
                "organization and project are required".into(),
            ));
        }
        if self.token.is_none() {
            return Ok(
                "sentry: org/project valid; SENTRY_AUTH_TOKEN missing (fixture mode available)"
                    .into(),
            );
        }
        Ok("sentry: configured for read-only issue observation".into())
    }

    /// Live observe issues (read-only GET).
    pub fn observe(&self) -> ConnectorResult<Vec<NormalizedObservation>> {
        let token = self.token.as_ref().ok_or_else(|| {
            ConnectorError::Config("SENTRY_AUTH_TOKEN required for live Sentry observe".into())
        })?;
        let client = reqwest::blocking::Client::builder()
            .user_agent("rivora-sentry-connector")
            .build()
            .map_err(|e| ConnectorError::Api(e.to_string()))?;
        let url = format!(
            "{}/projects/{}/{}/issues/?limit={}",
            self.api_base.trim_end_matches('/'),
            self.organization,
            self.project,
            self.limit
        );
        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .map_err(|e| ConnectorError::Api(e.to_string()))?;
        if response.status().as_u16() == 429 {
            return Err(ConnectorError::Api(
                "rate limited by Sentry API (no secrets leaked)".into(),
            ));
        }
        if !response.status().is_success() {
            return Err(ConnectorError::Api(format!(
                "Sentry API status {}",
                response.status()
            )));
        }
        let body: Value = response
            .json()
            .map_err(|e| ConnectorError::Normalize(e.to_string()))?;
        Self::normalize_issues(&self.organization, &self.project, &body)
    }

    /// Observe from fixture JSON.
    pub fn observe_from_fixture(fixture: &Value) -> ConnectorResult<Vec<NormalizedObservation>> {
        let org = fixture
            .get("organization")
            .and_then(|v| v.as_str())
            .unwrap_or("org");
        let project = fixture
            .get("project")
            .and_then(|v| v.as_str())
            .unwrap_or("project");
        if let Some(issues) = fixture.get("issues") {
            return Self::normalize_issues(org, project, issues);
        }
        if fixture.is_array() {
            return Self::normalize_issues(org, project, fixture);
        }
        Err(ConnectorError::Normalize(
            "fixture must include issues array or be an array of issues".into(),
        ))
    }

    fn normalize_issues(
        org: &str,
        project: &str,
        body: &Value,
    ) -> ConnectorResult<Vec<NormalizedObservation>> {
        let issues = body
            .as_array()
            .ok_or_else(|| ConnectorError::Normalize("issues must be an array".into()))?;
        let mut out = Vec::new();
        for issue in issues {
            let id = issue
                .get("id")
                .and_then(|v| v.as_str())
                .or_else(|| issue.get("id").and_then(|v| v.as_u64()).map(|_| ""))
                .map(|s| {
                    if s.is_empty() {
                        issue
                            .get("id")
                            .and_then(|v| v.as_u64())
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "unknown".into())
                    } else {
                        s.to_string()
                    }
                })
                .unwrap_or_else(|| "unknown".into());
            let title = issue
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("error");
            let level = issue
                .get("level")
                .and_then(|v| v.as_str())
                .unwrap_or("error");
            let count = issue.get("count").cloned().unwrap_or(json!("unknown"));
            let culprit = issue.get("culprit").and_then(|v| v.as_str()).unwrap_or("");
            let observed_at = issue
                .get("lastSeen")
                .or_else(|| issue.get("firstSeen"))
                .and_then(|v| v.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(Utc::now);

            let mut payload = issue.clone();
            if let Some(obj) = payload.as_object_mut() {
                for key in ["token", "authToken", "dsn", "secret"] {
                    if obj.contains_key(key) {
                        obj.insert(key.into(), json!("[redacted]"));
                    }
                }
            }

            out.push(NormalizedObservation::new(
                ObservationKind::Observability,
                format!(
                    "Sentry issue in {org}/{project}: [{level}] {title} count={count} culprit={culprit}"
                ),
                payload,
                "sentry",
                observed_at,
                Some(format!("sentry-issue:{org}:{project}:{id}")),
                "sentry-connector",
            ));
        }
        if out.is_empty() {
            out.push(NormalizedObservation::new(
                ObservationKind::Observability,
                format!("Sentry project {org}/{project} has no open issues in sample"),
                json!({"organization": org, "project": project, "issue_count": 0}),
                "sentry",
                Utc::now(),
                Some(format!("sentry-empty:{org}:{project}")),
                "sentry-connector",
            ));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fixture_normalizes_issues() {
        let fixture = json!({
            "organization": "acme",
            "project": "api",
            "issues": [{
                "id": "123",
                "title": "NullPointerException",
                "level": "error",
                "count": "42",
                "culprit": "app.handlers",
                "lastSeen": "2026-01-02T00:00:00Z",
                "dsn": "https://secret@sentry.io/1"
            }]
        });
        let obs = SentryConnector::observe_from_fixture(&fixture).unwrap();
        assert_eq!(obs.len(), 1);
        assert!(matches!(obs[0].kind, ObservationKind::Observability));
        assert!(obs[0].summary.contains("NullPointerException"));
        assert_eq!(obs[0].payload["dsn"], json!("[redacted]"));
        assert_eq!(
            obs[0].idempotency_key.as_deref(),
            Some("sentry-issue:acme:api:123")
        );
    }

    #[test]
    fn missing_token_live_observe_errors() {
        let c = SentryConnector {
            organization: "o".into(),
            project: "p".into(),
            token: None,
            api_base: "https://sentry.io/api/0".into(),
            limit: 1,
        };
        assert!(matches!(
            c.observe().unwrap_err(),
            ConnectorError::Config(_)
        ));
    }

    #[test]
    fn status_redacts_token() {
        let c = SentryConnector::new("o", "p").with_token("secret-token-value");
        let s = c.status();
        assert!(s.configured);
        assert!(!s.details.contains("secret-token-value"));
    }
}
