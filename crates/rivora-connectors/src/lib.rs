//! Read-only evidence connectors for Open Rivora.
//!
//! Connectors feed evidence into the memory loop. They do not mutate
//! infrastructure, repositories, deployments, or production systems.
//!
//! Phase 10 added a read-only local Git connector (`git`). Phase 11 adds a
//! read-only GitHub connector (`github`) for pull requests, issues, workflow
//! runs, releases, and deployments.

pub mod git;
pub mod github;

pub use git::*;
pub use github::*;

use rivora_errors::{Result, RivoraError};
use serde::{Deserialize, Serialize};

/// Read-only evidence connector contract.
///
/// Implementations ingest evidence from a source and return normalized
/// [`EvidenceItem`] values. Connectors must never mutate the source system.
pub trait EvidenceConnector {
    fn ingest(&self, request: EvidenceIngestRequest) -> Result<EvidenceIngestResult>;
}

/// Stable, unique identifier for an [`EvidenceItem`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EvidenceId(pub String);

impl EvidenceId {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(RivoraError::invalid_value(
                "evidence_id",
                "evidence id cannot be empty",
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Kind of evidence captured by a connector.
///
/// Git kinds cover local repository history. GitHub kinds cover pull requests,
/// issues, workflow runs, releases, and deployments ingested from the GitHub
/// REST API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    GitCommit,
    GitFileChange,
    GitTag,
    GitBranch,
    GitDiffSummary,
    GitHubPullRequest,
    GitHubPullRequestMerged,
    GitHubIssue,
    GitHubWorkflowRun,
    GitHubWorkflowFailed,
    GitHubWorkflowSucceeded,
    GitHubDeployment,
    GitHubRelease,
}

impl EvidenceKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GitCommit => "git_commit",
            Self::GitFileChange => "git_file_change",
            Self::GitTag => "git_tag",
            Self::GitBranch => "git_branch",
            Self::GitDiffSummary => "git_diff_summary",
            Self::GitHubPullRequest => "github_pull_request",
            Self::GitHubPullRequestMerged => "github_pull_request_merged",
            Self::GitHubIssue => "github_issue",
            Self::GitHubWorkflowRun => "github_workflow_run",
            Self::GitHubWorkflowFailed => "github_workflow_failed",
            Self::GitHubWorkflowSucceeded => "github_workflow_succeeded",
            Self::GitHubDeployment => "github_deployment",
            Self::GitHubRelease => "github_release",
        }
    }

    /// Human-friendly label used in CLI output.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::GitCommit => "Git commit",
            Self::GitFileChange => "Git file change",
            Self::GitTag => "Git tag",
            Self::GitBranch => "Git branch",
            Self::GitDiffSummary => "Git diff summary",
            Self::GitHubPullRequest => "GitHub pull request",
            Self::GitHubPullRequestMerged => "GitHub PR merged",
            Self::GitHubIssue => "GitHub issue",
            Self::GitHubWorkflowRun => "GitHub workflow run",
            Self::GitHubWorkflowFailed => "GitHub workflow failed",
            Self::GitHubWorkflowSucceeded => "GitHub workflow succeeded",
            Self::GitHubDeployment => "GitHub deployment",
            Self::GitHubRelease => "GitHub release",
        }
    }

    #[must_use]
    pub fn is_github(self) -> bool {
        matches!(
            self,
            Self::GitHubPullRequest
                | Self::GitHubPullRequestMerged
                | Self::GitHubIssue
                | Self::GitHubWorkflowRun
                | Self::GitHubWorkflowFailed
                | Self::GitHubWorkflowSucceeded
                | Self::GitHubDeployment
                | Self::GitHubRelease
        )
    }
}

/// Provenance for an [`EvidenceItem`]: which connector produced it and from
/// where.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceSource {
    pub connector: String,
    pub version: String,
    pub repository: Option<String>,
    pub read_only: bool,
}

impl EvidenceSource {
    #[must_use]
    pub fn local_git(repository: impl Into<String>) -> Self {
        Self {
            connector: git::LOCAL_GIT_CONNECTOR.to_string(),
            version: CONNECTOR_VERSION.to_string(),
            repository: Some(repository.into()),
            read_only: true,
        }
    }

    #[must_use]
    pub fn github(repository: impl Into<String>) -> Self {
        Self {
            connector: github::GITHUB_CONNECTOR.to_string(),
            version: CONNECTOR_VERSION.to_string(),
            repository: Some(repository.into()),
            read_only: true,
        }
    }
}

/// Normalized, serializable evidence captured by a connector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceItem {
    pub id: EvidenceId,
    pub kind: EvidenceKind,
    pub source: EvidenceSource,
    pub title: String,
    pub summary: String,
    pub body: String,
    pub service: Option<String>,
    pub files_changed: Vec<String>,
    pub timestamp: Option<String>,
    pub author: Option<String>,
    pub tags: Vec<String>,
    pub refs: Vec<String>,
    pub confidence: f64,
}

impl EvidenceItem {
    #[must_use]
    pub fn is_github(&self) -> bool {
        self.source.connector == github::GITHUB_CONNECTOR || self.kind.is_github()
    }
}

/// Request for the local Git connector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceIngestRequest {
    pub repo_path: std::path::PathBuf,
    pub since: Option<String>,
    pub limit: usize,
}

impl EvidenceIngestRequest {
    #[must_use]
    pub fn new(repo_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            repo_path: repo_path.into(),
            since: None,
            limit: 20,
        }
    }

    #[must_use]
    pub fn with_since(mut self, since: impl Into<String>) -> Self {
        self.since = Some(since.into());
        self
    }

    #[must_use]
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}

/// Result of a local Git evidence ingestion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceIngestResult {
    pub repository: String,
    pub evidence: Vec<EvidenceItem>,
    pub commits: usize,
    pub file_changes: usize,
    pub topics: Vec<String>,
}

/// Connector version shared by all in-process connectors.
pub const CONNECTOR_VERSION: &str = "0.1.0";

/// Slugify a value for use in identifiers. Lowercase, alphanumeric and hyphen
/// only, empty parts collapsed. Falls back to `"evidence"` when nothing
/// meaningful remains.
pub(crate) fn slug(value: &str) -> String {
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
        "evidence".to_string()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_kind_as_str_covers_git_and_github() {
        assert_eq!(EvidenceKind::GitCommit.as_str(), "git_commit");
        assert_eq!(
            EvidenceKind::GitHubPullRequestMerged.as_str(),
            "github_pull_request_merged"
        );
        assert_eq!(
            EvidenceKind::GitHubWorkflowFailed.as_str(),
            "github_workflow_failed"
        );
        assert_eq!(EvidenceKind::GitHubRelease.as_str(), "github_release");
    }

    #[test]
    fn evidence_kind_label_is_human_friendly() {
        assert_eq!(EvidenceKind::GitCommit.label(), "Git commit");
        assert_eq!(
            EvidenceKind::GitHubPullRequestMerged.label(),
            "GitHub PR merged"
        );
        assert_eq!(
            EvidenceKind::GitHubWorkflowFailed.label(),
            "GitHub workflow failed"
        );
    }

    #[test]
    fn evidence_kind_is_github_partition_is_correct() {
        assert!(EvidenceKind::GitHubPullRequest.is_github());
        assert!(EvidenceKind::GitHubWorkflowFailed.is_github());
        assert!(!EvidenceKind::GitCommit.is_github());
        assert!(!EvidenceKind::GitBranch.is_github());
    }

    #[test]
    fn evidence_source_github_is_read_only() {
        let source = EvidenceSource::github("owner/name");
        assert_eq!(source.connector, "github");
        assert!(source.read_only);
        assert_eq!(source.repository.as_deref(), Some("owner/name"));
    }

    #[test]
    fn slug_collapses_non_alphanumeric() {
        assert_eq!(slug("Owner/Name"), "owner-name");
        assert_eq!(slug("  "), "evidence");
    }
}
