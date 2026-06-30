//! Read-only local Git evidence connector.
//!
//! Reads recent commits, changed files, branches, tags, and diff summaries
//! from a repository on disk using read-only `git` subcommands only.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use rivora_errors::{Result, RivoraError};

use crate::{
    slug, EvidenceConnector, EvidenceId, EvidenceIngestRequest, EvidenceIngestResult, EvidenceItem,
    EvidenceKind, EvidenceSource,
};

pub const LOCAL_GIT_CONNECTOR: &str = "local-git";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalGitConnector {
    repo_path: std::path::PathBuf,
}

impl LocalGitConnector {
    #[must_use]
    pub fn new(repo_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            repo_path: repo_path.into(),
        }
    }

    pub fn ingest_recent(
        &self,
        since: Option<String>,
        limit: usize,
    ) -> Result<EvidenceIngestResult> {
        self.ingest(EvidenceIngestRequest {
            repo_path: self.repo_path.clone(),
            since,
            limit,
        })
    }
}

impl EvidenceConnector for LocalGitConnector {
    fn ingest(&self, request: EvidenceIngestRequest) -> Result<EvidenceIngestResult> {
        if request.limit == 0 {
            return Err(RivoraError::invalid_value(
                "limit",
                "limit must be positive",
            ));
        }

        let repository = canonical_repo_display(&request.repo_path);
        let mut log_args = vec![
            "log".to_string(),
            format!("--max-count={}", request.limit),
            "--pretty=format:%H%x1f%an%x1f%aI%x1f%s".to_string(),
        ];
        if let Some(since) = &request.since {
            log_args.push(format!("--since={since}"));
        }

        let log_output = run_git(&request.repo_path, &log_args)?;
        let commits = parse_git_log(&log_output);
        let branch_output = run_git(
            &request.repo_path,
            &["branch".to_string(), "--show-current".to_string()],
        )
        .unwrap_or_default();
        let tag_output = run_git(
            &request.repo_path,
            &["tag".to_string(), "--list".to_string()],
        )
        .unwrap_or_default();

        let mut evidence = Vec::new();
        let mut topics = BTreeSet::new();
        let source = EvidenceSource::local_git(repository.clone());
        let branch = branch_output.trim();
        if !branch.is_empty() {
            evidence.push(branch_item(&source, branch)?);
            topics.insert(branch.to_string());
        }
        for tag in tag_output
            .lines()
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
        {
            evidence.push(tag_item(&source, tag)?);
        }

        for commit in &commits {
            let files_output = run_git(
                &request.repo_path,
                &[
                    "show".to_string(),
                    "--name-only".to_string(),
                    "--pretty=format:".to_string(),
                    commit.sha.clone(),
                ],
            )?;
            let files = parse_name_only(&files_output);
            let inferred = infer_topics(&files);
            topics.extend(inferred.iter().cloned());
            evidence.push(commit_item(&source, commit, &files, &inferred)?);
            evidence.push(diff_summary_item(&source, commit, &files, &inferred)?);
            for file in &files {
                evidence.push(file_change_item(&source, commit, file, &inferred)?);
            }
        }

        evidence.sort_by(|a, b| a.id.cmp(&b.id));
        evidence.dedup_by(|a, b| a.id == b.id);
        let file_changes = evidence
            .iter()
            .filter(|item| item.kind == EvidenceKind::GitFileChange)
            .count();

        Ok(EvidenceIngestResult {
            repository,
            evidence,
            commits: commits.len(),
            file_changes,
            topics: topics.into_iter().collect(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCommit {
    pub sha: String,
    pub author: String,
    pub timestamp: String,
    pub summary: String,
}

#[must_use]
pub fn parse_git_log(output: &str) -> Vec<GitCommit> {
    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\x1f');
            Some(GitCommit {
                sha: parts.next()?.to_string(),
                author: parts.next()?.to_string(),
                timestamp: parts.next()?.to_string(),
                summary: parts.next()?.to_string(),
            })
        })
        .filter(|commit| !commit.sha.is_empty())
        .collect()
}

#[must_use]
pub fn parse_name_only(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect()
}

#[must_use]
pub fn infer_topics(files: &[String]) -> Vec<String> {
    let mut topics = BTreeSet::new();
    for file in files {
        let mut parts = file.split('/').filter(|part| !part.is_empty());
        if let Some(first) = parts.next() {
            let topic = match first {
                "crates" | "apps" | "services" => parts.next().unwrap_or(first),
                "docs" => "docs",
                ".github" => "github",
                other => other,
            };
            topics.insert(
                topic
                    .trim_start_matches("rivora-")
                    .trim_start_matches("svc-")
                    .to_string(),
            );
        }
    }
    topics.into_iter().collect()
}

#[must_use]
pub fn is_read_only_git_command(args: &[String]) -> bool {
    matches!(
        args.first().map(String::as_str),
        Some("log" | "show" | "diff" | "tag" | "branch" | "rev-parse")
    )
}

#[must_use]
pub fn forbidden_git_commands() -> &'static [&'static str] {
    &[
        "commit", "push", "pull", "reset", "checkout", "rebase", "merge", "clean",
    ]
}

fn run_git(repo_path: &Path, args: &[String]) -> Result<String> {
    if !is_read_only_git_command(args) {
        return Err(RivoraError::invalid_value(
            "git_command",
            "local git connector only allows read-only git commands",
        ));
    }
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()?;
    if !output.status.success() {
        return Err(RivoraError::provider(
            "git",
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn commit_item(
    source: &EvidenceSource,
    commit: &GitCommit,
    files: &[String],
    topics: &[String],
) -> Result<EvidenceItem> {
    let short = short_sha(&commit.sha);
    Ok(EvidenceItem {
        id: EvidenceId::new(format!("git:commit:{}", commit.sha))?,
        kind: EvidenceKind::GitCommit,
        source: source.clone(),
        title: format!("Git commit {short}"),
        summary: commit.summary.clone(),
        body: format!(
            "Commit {} by {} changed {} file(s): {}",
            commit.sha,
            commit.author,
            files.len(),
            files.join(", ")
        ),
        service: topics.first().cloned(),
        files_changed: files.to_vec(),
        timestamp: Some(commit.timestamp.clone()),
        author: Some(commit.author.clone()),
        tags: topics.to_vec(),
        refs: vec![commit.sha.clone()],
        confidence: 0.9,
    })
}

fn diff_summary_item(
    source: &EvidenceSource,
    commit: &GitCommit,
    files: &[String],
    topics: &[String],
) -> Result<EvidenceItem> {
    let short = short_sha(&commit.sha);
    Ok(EvidenceItem {
        id: EvidenceId::new(format!("git:diff:{}:summary", commit.sha))?,
        kind: EvidenceKind::GitDiffSummary,
        source: source.clone(),
        title: format!("Git diff summary {short}"),
        summary: format!("{} file(s) changed in commit {short}", files.len()),
        body: files.join("\n"),
        service: topics.first().cloned(),
        files_changed: files.to_vec(),
        timestamp: Some(commit.timestamp.clone()),
        author: Some(commit.author.clone()),
        tags: topics.to_vec(),
        refs: vec![commit.sha.clone()],
        confidence: 0.85,
    })
}

fn file_change_item(
    source: &EvidenceSource,
    commit: &GitCommit,
    file: &str,
    topics: &[String],
) -> Result<EvidenceItem> {
    let short = short_sha(&commit.sha);
    Ok(EvidenceItem {
        id: EvidenceId::new(format!("git:file:{}:{}", commit.sha, slug(file)))?,
        kind: EvidenceKind::GitFileChange,
        source: source.clone(),
        title: format!("{file} changed in {short}"),
        summary: format!("Git commit {short} changed {file}"),
        body: format!("File {file} changed in commit {}", commit.sha),
        service: topics.first().cloned(),
        files_changed: vec![file.to_string()],
        timestamp: Some(commit.timestamp.clone()),
        author: Some(commit.author.clone()),
        tags: topics.to_vec(),
        refs: vec![commit.sha.clone(), file.to_string()],
        confidence: 0.85,
    })
}

fn branch_item(source: &EvidenceSource, branch: &str) -> Result<EvidenceItem> {
    Ok(EvidenceItem {
        id: EvidenceId::new(format!("git:branch:{}", slug(branch)))?,
        kind: EvidenceKind::GitBranch,
        source: source.clone(),
        title: format!("Git branch {branch}"),
        summary: format!("Current local Git branch is {branch}"),
        body: "Branch evidence was read from the local repository.".to_string(),
        service: Some(branch.to_string()),
        files_changed: Vec::new(),
        timestamp: None,
        author: None,
        tags: vec![branch.to_string()],
        refs: vec![branch.to_string()],
        confidence: 0.75,
    })
}

fn tag_item(source: &EvidenceSource, tag: &str) -> Result<EvidenceItem> {
    Ok(EvidenceItem {
        id: EvidenceId::new(format!("git:tag:{}", slug(tag)))?,
        kind: EvidenceKind::GitTag,
        source: source.clone(),
        title: format!("Git tag {tag}"),
        summary: format!("Local Git tag {tag} exists"),
        body: "Tag evidence was read from the local repository.".to_string(),
        service: None,
        files_changed: Vec::new(),
        timestamp: None,
        author: None,
        tags: vec![tag.to_string()],
        refs: vec![tag.to_string()],
        confidence: 0.75,
    })
}

fn canonical_repo_display(path: &Path) -> String {
    if path.as_os_str().is_empty() {
        ".".to_string()
    } else {
        path.display().to_string()
    }
}

fn short_sha(sha: &str) -> String {
    sha.chars().take(7).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_commit_evidence_from_git_log_output() {
        let commits = parse_git_log(
            "abc123\x1fAda Lovelace\x1f2026-06-28T00:00:00Z\x1fAdd checkout memory\n",
        );

        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].sha, "abc123");
        assert_eq!(commits[0].author, "Ada Lovelace");
        assert_eq!(commits[0].summary, "Add checkout memory");
    }

    #[test]
    fn infers_topics_from_file_paths() {
        let topics = infer_topics(&[
            "crates/rivora-cli/src/lib.rs".to_string(),
            "services/checkout-api/src/main.rs".to_string(),
            "docs/README.md".to_string(),
        ]);

        assert_eq!(topics, vec!["checkout-api", "cli", "docs"]);
    }

    #[test]
    fn git_connector_allows_only_read_only_commands() {
        for command in ["log", "show", "diff", "tag", "branch", "rev-parse"] {
            assert!(is_read_only_git_command(&[command.to_string()]));
        }
    }

    #[test]
    fn mutation_git_commands_are_never_allowed() {
        for command in forbidden_git_commands() {
            assert!(!is_read_only_git_command(&[(*command).to_string()]));
        }
    }
}
