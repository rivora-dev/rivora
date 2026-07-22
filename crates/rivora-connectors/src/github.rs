//! Narrowly scoped read-only GitHub connector (RFC-012).

use chrono::Utc;
use rivora::domain::ObservationKind;
use serde::Deserialize;
use serde_json::json;

use crate::{ConnectorError, ConnectorResult, NormalizedObservation};

/// Read-only GitHub connector configuration.
#[derive(Debug, Clone)]
pub struct GitHubConnector {
    /// `owner/repo`
    pub repository: String,
    /// Optional personal access token for private repos / higher rate limits.
    pub token: Option<String>,
    /// Optional pull request number to observe.
    pub pull_request: Option<u64>,
    /// API base URL (override for GHES).
    pub api_base: String,
}

impl GitHubConnector {
    /// Create a connector for `owner/repo`.
    pub fn new(repository: impl Into<String>) -> Self {
        Self {
            repository: repository.into(),
            token: std::env::var("GITHUB_TOKEN").ok().filter(|s| !s.is_empty()),
            pull_request: None,
            api_base: "https://api.github.com".into(),
        }
    }

    /// Observe a specific pull request.
    pub fn with_pull_request(mut self, number: u64) -> Self {
        self.pull_request = Some(number);
        self
    }

    /// Override token.
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Observe repository metadata, optional PR, commits, checks, and issues.
    pub fn observe(&self) -> ConnectorResult<Vec<NormalizedObservation>> {
        let client = self.client()?;
        let mut out = Vec::new();
        out.push(self.fetch_repository(&client)?);

        if let Some(pr) = self.pull_request {
            out.push(self.fetch_pull_request(&client, pr)?);
            if let Ok(mut commits) = self.fetch_pr_commits(&client, pr) {
                out.append(&mut commits);
            }
            if let Ok(mut checks) = self.fetch_pr_checks(&client, pr) {
                out.append(&mut checks);
            }
            if let Ok(mut issues) = self.fetch_linked_issue_refs(&client, pr) {
                out.append(&mut issues);
            }
        }

        Ok(out)
    }

    /// Observe using a provided JSON fixture (for offline tests / demos).
    pub fn observe_from_fixture(
        fixture: &serde_json::Value,
    ) -> ConnectorResult<Vec<NormalizedObservation>> {
        let mut out = Vec::new();
        if let Some(repo) = fixture.get("repository") {
            let full_name = repo
                .get("full_name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown/repo");
            out.push(NormalizedObservation::new(
                ObservationKind::Repository,
                format!("GitHub repository `{full_name}`"),
                repo.clone(),
                "github",
                Utc::now(),
                Some(format!("github-repo:{full_name}")),
                "github-connector",
            ));
        }
        if let Some(pr) = fixture.get("pull_request") {
            let number = pr.get("number").and_then(|v| v.as_u64()).unwrap_or(0);
            let title = pr
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("pull request");
            out.push(NormalizedObservation::new(
                ObservationKind::PullRequest,
                format!("Pull request #{number}: {title}"),
                pr.clone(),
                "github",
                Utc::now(),
                Some(format!("github-pr:{number}")),
                "github-connector",
            ));
        }
        if let Some(commits) = fixture.get("commits").and_then(|v| v.as_array()) {
            for commit in commits {
                let sha = commit
                    .get("sha")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let message = commit
                    .pointer("/commit/message")
                    .and_then(|v| v.as_str())
                    .or_else(|| commit.get("message").and_then(|v| v.as_str()))
                    .unwrap_or("")
                    .lines()
                    .next()
                    .unwrap_or("");
                out.push(NormalizedObservation::new(
                    ObservationKind::Commit,
                    format!("GitHub commit {}: {message}", &sha[..7.min(sha.len())]),
                    commit.clone(),
                    "github",
                    Utc::now(),
                    Some(format!("github-commit:{sha}")),
                    "github-connector",
                ));
            }
        }
        if let Some(checks) = fixture.get("checks").and_then(|v| v.as_array()) {
            for check in checks {
                let name = check
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("check");
                let conclusion = check
                    .get("conclusion")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                out.push(NormalizedObservation::new(
                    ObservationKind::CheckResult,
                    format!("GitHub check `{name}`: {conclusion}"),
                    check.clone(),
                    "github",
                    Utc::now(),
                    Some(format!("github-check:{name}:{conclusion}")),
                    "github-connector",
                ));
            }
        }
        if let Some(issues) = fixture.get("issues").and_then(|v| v.as_array()) {
            for issue in issues {
                let number = issue.get("number").and_then(|v| v.as_u64()).unwrap_or(0);
                let title = issue
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("issue");
                out.push(NormalizedObservation::new(
                    ObservationKind::Issue,
                    format!("Linked issue #{number}: {title}"),
                    issue.clone(),
                    "github",
                    Utc::now(),
                    Some(format!("github-issue:{number}")),
                    "github-connector",
                ));
            }
        }
        if out.is_empty() {
            return Err(ConnectorError::Normalize(
                "fixture contained no supported GitHub objects".into(),
            ));
        }
        Ok(out)
    }

    fn client(&self) -> ConnectorResult<reqwest::blocking::Client> {
        reqwest::blocking::Client::builder()
            .user_agent("rivora-github-connector/0.1")
            .build()
            .map_err(|e| ConnectorError::Api(e.to_string()))
    }

    fn get_json(
        &self,
        client: &reqwest::blocking::Client,
        path: &str,
    ) -> ConnectorResult<serde_json::Value> {
        let url = format!("{}{}", self.api_base, path);
        let mut req = client
            .get(&url)
            .header("Accept", "application/vnd.github+json");
        if let Some(token) = &self.token {
            req = req.bearer_auth(token);
        }
        let response = req.send().map_err(|e| ConnectorError::Api(e.to_string()))?;
        if !response.status().is_success() {
            return Err(ConnectorError::Api(format!(
                "GET {path} failed: {}",
                response.status()
            )));
        }
        response
            .json()
            .map_err(|e| ConnectorError::Normalize(e.to_string()))
    }

    fn fetch_repository(
        &self,
        client: &reqwest::blocking::Client,
    ) -> ConnectorResult<NormalizedObservation> {
        let data = self.get_json(client, &format!("/repos/{}", self.repository))?;
        let full_name = data
            .get("full_name")
            .and_then(|v| v.as_str())
            .unwrap_or(&self.repository)
            .to_string();
        Ok(NormalizedObservation::new(
            ObservationKind::Repository,
            format!("GitHub repository `{full_name}`"),
            data,
            "github",
            Utc::now(),
            Some(format!("github-repo:{full_name}")),
            "github-connector",
        ))
    }

    fn fetch_pull_request(
        &self,
        client: &reqwest::blocking::Client,
        number: u64,
    ) -> ConnectorResult<NormalizedObservation> {
        let data = self.get_json(
            client,
            &format!("/repos/{}/pulls/{number}", self.repository),
        )?;
        let title = data
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("pull request");
        Ok(NormalizedObservation::new(
            ObservationKind::PullRequest,
            format!("Pull request #{number}: {title}"),
            data,
            "github",
            Utc::now(),
            Some(format!("github-pr:{}:{number}", self.repository)),
            "github-connector",
        ))
    }

    fn fetch_pr_commits(
        &self,
        client: &reqwest::blocking::Client,
        number: u64,
    ) -> ConnectorResult<Vec<NormalizedObservation>> {
        let data = self.get_json(
            client,
            &format!("/repos/{}/pulls/{number}/commits", self.repository),
        )?;
        let commits = data
            .as_array()
            .ok_or_else(|| ConnectorError::Normalize("expected commits array".into()))?;
        let mut out = Vec::new();
        for commit in commits.iter().take(20) {
            let sha = commit
                .get("sha")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let message = commit
                .pointer("/commit/message")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .lines()
                .next()
                .unwrap_or("");
            out.push(NormalizedObservation::new(
                ObservationKind::Commit,
                format!("GitHub commit {}: {message}", &sha[..7.min(sha.len())]),
                commit.clone(),
                "github",
                Utc::now(),
                Some(format!("github-commit:{sha}")),
                "github-connector",
            ));
        }
        Ok(out)
    }

    fn fetch_pr_checks(
        &self,
        client: &reqwest::blocking::Client,
        number: u64,
    ) -> ConnectorResult<Vec<NormalizedObservation>> {
        // Resolve head SHA from PR first.
        let pr = self.get_json(
            client,
            &format!("/repos/{}/pulls/{number}", self.repository),
        )?;
        let sha = pr
            .pointer("/head/sha")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ConnectorError::Normalize("PR missing head.sha".into()))?;
        let data = self.get_json(
            client,
            &format!("/repos/{}/commits/{sha}/check-runs", self.repository),
        )?;
        let runs = data
            .get("check_runs")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let mut out = Vec::new();
        for run in runs {
            let name = run
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("check")
                .to_string();
            let conclusion = run
                .get("conclusion")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            out.push(NormalizedObservation::new(
                ObservationKind::CheckResult,
                format!("GitHub check `{name}`: {conclusion}"),
                run,
                "github",
                Utc::now(),
                Some(format!("github-check:{sha}:{name}")),
                "github-connector",
            ));
        }
        Ok(out)
    }

    fn fetch_linked_issue_refs(
        &self,
        client: &reqwest::blocking::Client,
        number: u64,
    ) -> ConnectorResult<Vec<NormalizedObservation>> {
        let pr = self.get_json(
            client,
            &format!("/repos/{}/pulls/{number}", self.repository),
        )?;
        let body = pr.get("body").and_then(|v| v.as_str()).unwrap_or("");
        let mut out = Vec::new();
        for issue_number in extract_issue_numbers(body) {
            match self.get_json(
                client,
                &format!("/repos/{}/issues/{issue_number}", self.repository),
            ) {
                Ok(issue) => {
                    let title = issue
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("issue");
                    out.push(NormalizedObservation::new(
                        ObservationKind::Issue,
                        format!("Linked issue #{issue_number}: {title}"),
                        issue,
                        "github",
                        Utc::now(),
                        Some(format!("github-issue:{}:{issue_number}", self.repository)),
                        "github-connector",
                    ));
                }
                Err(_) => {
                    // Non-fatal: keep a lightweight reference observation.
                    out.push(NormalizedObservation::new(
                        ObservationKind::Issue,
                        format!("Referenced issue #{issue_number}"),
                        json!({"number": issue_number, "referenced_from_pr": number}),
                        "github",
                        Utc::now(),
                        Some(format!("github-issue-ref:{issue_number}")),
                        "github-connector",
                    ));
                }
            }
        }
        Ok(out)
    }
}

fn extract_issue_numbers(body: &str) -> Vec<u64> {
    let mut numbers = Vec::new();
    for token in body.split_whitespace() {
        let cleaned = token.trim_matches(|c: char| !c.is_ascii_digit() && c != '#');
        if let Some(stripped) = cleaned.strip_prefix('#') {
            if let Ok(n) = stripped.parse::<u64>() {
                if !numbers.contains(&n) {
                    numbers.push(n);
                }
            }
        }
    }
    numbers
}

/// Minimal types kept for documentation / potential typed parsing.
#[derive(Debug, Deserialize)]
struct _RepoMeta {
    full_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_normalization() {
        let fixture = json!({
            "repository": {"full_name": "acme/widgets", "default_branch": "main"},
            "pull_request": {"number": 42, "title": "Fix flaky test", "body": "Closes #7"},
            "commits": [{"sha": "abcdef123456", "message": "fix tests"}],
            "checks": [{"name": "ci", "conclusion": "failure"}],
            "issues": [{"number": 7, "title": "flaky suite"}]
        });
        let obs = GitHubConnector::observe_from_fixture(&fixture).unwrap();
        assert!(obs
            .iter()
            .any(|o| matches!(o.kind, ObservationKind::Repository)));
        assert!(obs
            .iter()
            .any(|o| matches!(o.kind, ObservationKind::PullRequest)));
        assert!(obs
            .iter()
            .any(|o| matches!(o.kind, ObservationKind::Commit)));
        assert!(obs
            .iter()
            .any(|o| matches!(o.kind, ObservationKind::CheckResult)));
        assert!(obs.iter().any(|o| matches!(o.kind, ObservationKind::Issue)));
        for o in &obs {
            assert_eq!(o.source, "github");
        }
    }

    #[test]
    fn extract_issues_from_body() {
        let nums = extract_issue_numbers("Fixes #12 and relates to #3, #12");
        assert_eq!(nums, vec![12, 3]);
    }
}
