//! Read-only evidence connectors for Open Rivora.
//!
//! Connectors feed evidence into the memory loop. They do not mutate
//! infrastructure, repositories, deployments, or production systems.
//!
//! Phase 10 added a read-only local Git connector (`git`). Phase 11 adds a
//! read-only GitHub connector (`github`) for pull requests, issues, workflow
//! runs, releases, and deployments. Phase 18A adds a read-only Vercel
//! connector (`vercel`) for deployment evidence. Phase 18B adds a read-only
//! Cloudflare connector (`cloudflare`) for Pages and Workers deployment
//! evidence. Phase 20A adds a read-only Sentry connector (`sentry`) for
//! observability issue/error evidence.

pub mod cloudflare;
pub mod git;
pub mod github;
pub mod sentry;
pub mod vercel;

pub use cloudflare::*;
pub use git::*;
pub use github::*;
pub use sentry::*;
pub use vercel::*;

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
/// REST API. Vercel kinds cover deployments ingested from the Vercel REST API.
/// Sentry kinds cover observability issues ingested from the Sentry REST API.
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
    VercelDeployment,
    CloudflarePagesDeployment,
    CloudflareWorkerDeployment,
    SentryIssue,
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
            Self::VercelDeployment => "vercel_deployment",
            Self::CloudflarePagesDeployment => "cloudflare_pages_deployment",
            Self::CloudflareWorkerDeployment => "cloudflare_worker_deployment",
            Self::SentryIssue => "sentry_issue",
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
            Self::VercelDeployment => "Vercel deployment",
            Self::CloudflarePagesDeployment => "Cloudflare Pages deployment",
            Self::CloudflareWorkerDeployment => "Cloudflare Worker deployment",
            Self::SentryIssue => "Sentry issue",
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

    #[must_use]
    pub fn is_vercel(self) -> bool {
        matches!(self, Self::VercelDeployment)
    }

    #[must_use]
    pub fn is_cloudflare(self) -> bool {
        matches!(
            self,
            Self::CloudflarePagesDeployment | Self::CloudflareWorkerDeployment
        )
    }

    #[must_use]
    pub fn is_cloudflare_pages(self) -> bool {
        matches!(self, Self::CloudflarePagesDeployment)
    }

    #[must_use]
    pub fn is_cloudflare_worker(self) -> bool {
        matches!(self, Self::CloudflareWorkerDeployment)
    }

    #[must_use]
    pub fn is_sentry(self) -> bool {
        matches!(self, Self::SentryIssue)
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

    #[must_use]
    pub fn vercel(repository: impl Into<String>) -> Self {
        Self {
            connector: vercel::VERCEL_CONNECTOR.to_string(),
            version: CONNECTOR_VERSION.to_string(),
            repository: Some(repository.into()),
            read_only: true,
        }
    }

    #[must_use]
    pub fn cloudflare(repository: impl Into<String>) -> Self {
        Self {
            connector: cloudflare::CLOUDFLARE_CONNECTOR.to_string(),
            version: CONNECTOR_VERSION.to_string(),
            repository: Some(repository.into()),
            read_only: true,
        }
    }

    #[must_use]
    pub fn sentry(repository: impl Into<String>) -> Self {
        Self {
            connector: sentry::SENTRY_CONNECTOR.to_string(),
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

    #[must_use]
    pub fn is_vercel(&self) -> bool {
        self.source.connector == vercel::VERCEL_CONNECTOR || self.kind.is_vercel()
    }

    #[must_use]
    pub fn is_cloudflare(&self) -> bool {
        self.source.connector == cloudflare::CLOUDFLARE_CONNECTOR || self.kind.is_cloudflare()
    }

    #[must_use]
    pub fn is_cloudflare_pages(&self) -> bool {
        self.kind.is_cloudflare_pages()
    }

    #[must_use]
    pub fn is_cloudflare_worker(&self) -> bool {
        self.kind.is_cloudflare_worker()
    }

    #[must_use]
    pub fn is_sentry(&self) -> bool {
        self.source.connector == sentry::SENTRY_CONNECTOR || self.kind.is_sentry()
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
        assert_eq!(EvidenceKind::VercelDeployment.as_str(), "vercel_deployment");
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
        assert_eq!(EvidenceKind::VercelDeployment.label(), "Vercel deployment");
    }

    #[test]
    fn evidence_kind_is_github_partition_is_correct() {
        assert!(EvidenceKind::GitHubPullRequest.is_github());
        assert!(EvidenceKind::GitHubWorkflowFailed.is_github());
        assert!(!EvidenceKind::GitCommit.is_github());
        assert!(!EvidenceKind::GitBranch.is_github());
        assert!(!EvidenceKind::VercelDeployment.is_github());
    }

    #[test]
    fn evidence_kind_is_vercel_partition_is_correct() {
        assert!(EvidenceKind::VercelDeployment.is_vercel());
        assert!(!EvidenceKind::GitCommit.is_vercel());
        assert!(!EvidenceKind::GitHubDeployment.is_vercel());
        assert!(!EvidenceKind::CloudflarePagesDeployment.is_vercel());
    }

    #[test]
    fn evidence_kind_is_cloudflare_partition_is_correct() {
        assert!(EvidenceKind::CloudflarePagesDeployment.is_cloudflare());
        assert!(EvidenceKind::CloudflareWorkerDeployment.is_cloudflare());
        assert!(EvidenceKind::CloudflarePagesDeployment.is_cloudflare_pages());
        assert!(EvidenceKind::CloudflareWorkerDeployment.is_cloudflare_worker());
        assert!(!EvidenceKind::CloudflarePagesDeployment.is_cloudflare_worker());
        assert!(!EvidenceKind::CloudflareWorkerDeployment.is_cloudflare_pages());
        assert!(!EvidenceKind::GitCommit.is_cloudflare());
        assert!(!EvidenceKind::VercelDeployment.is_cloudflare());
    }

    #[test]
    fn evidence_source_github_is_read_only() {
        let source = EvidenceSource::github("owner/name");
        assert_eq!(source.connector, "github");
        assert!(source.read_only);
        assert_eq!(source.repository.as_deref(), Some("owner/name"));
    }

    #[test]
    fn evidence_source_vercel_is_read_only() {
        let source = EvidenceSource::vercel("my-app");
        assert_eq!(source.connector, "vercel");
        assert!(source.read_only);
        assert_eq!(source.repository.as_deref(), Some("my-app"));
        assert_eq!(source.version, crate::CONNECTOR_VERSION);
    }

    #[test]
    fn evidence_source_cloudflare_is_read_only() {
        let source = EvidenceSource::cloudflare("my-app");
        assert_eq!(source.connector, "cloudflare");
        assert!(source.read_only);
        assert_eq!(source.repository.as_deref(), Some("my-app"));
        assert_eq!(source.version, crate::CONNECTOR_VERSION);
    }

    #[test]
    fn evidence_source_sentry_is_read_only() {
        let source = EvidenceSource::sentry("my-org/checkout-api");
        assert_eq!(source.connector, "sentry");
        assert!(source.read_only);
        assert_eq!(source.repository.as_deref(), Some("my-org/checkout-api"));
        assert_eq!(source.version, crate::CONNECTOR_VERSION);
    }

    #[test]
    fn evidence_kind_is_sentry_partition_is_correct() {
        assert!(EvidenceKind::SentryIssue.is_sentry());
        assert!(!EvidenceKind::GitCommit.is_sentry());
        assert!(!EvidenceKind::VercelDeployment.is_sentry());
        assert!(!EvidenceKind::CloudflarePagesDeployment.is_sentry());
    }

    #[test]
    fn slug_collapses_non_alphanumeric() {
        assert_eq!(slug("Owner/Name"), "owner-name");
        assert_eq!(slug("  "), "evidence");
    }
}
