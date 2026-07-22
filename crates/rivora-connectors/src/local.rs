//! Local project connector — read-only observation of a filesystem project.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::Utc;
use rivora::domain::ObservationKind;
use serde_json::json;

use crate::{ConnectorError, ConnectorResult, NormalizedObservation};

/// Read-only local project connector.
#[derive(Debug, Clone)]
pub struct LocalConnector {
    /// Project root to observe.
    pub root: PathBuf,
}

impl LocalConnector {
    /// Create a connector for `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Observe the local project and produce normalized Observations.
    ///
    /// Sources:
    /// - repository metadata (path existence)
    /// - git status / branch
    /// - recent commits
    /// - changed files
    /// - optional test output file
    /// - structured local event files under `.rivora/events/`
    pub fn observe(&self) -> ConnectorResult<Vec<NormalizedObservation>> {
        let mut out = Vec::new();
        out.push(self.observe_repository()?);

        if self.root.join(".git").exists() {
            if let Ok(obs) = self.observe_git_status() {
                out.push(obs);
            }
            if let Ok(mut commits) = self.observe_recent_commits(5) {
                out.append(&mut commits);
            }
            if let Ok(obs) = self.observe_changed_files() {
                out.push(obs);
            }
        }

        if let Ok(obs) = self.observe_test_output() {
            out.push(obs);
        }

        if let Ok(mut events) = self.observe_event_files() {
            out.append(&mut events);
        }

        Ok(out)
    }

    fn observe_repository(&self) -> ConnectorResult<NormalizedObservation> {
        let root = self
            .root
            .canonicalize()
            .unwrap_or_else(|_| self.root.clone());
        let name = root
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "project".into());
        Ok(NormalizedObservation::new(
            ObservationKind::Repository,
            format!("Local repository `{name}`"),
            json!({
                "path": root.display().to_string(),
                "name": name,
                "has_git": self.root.join(".git").exists(),
            }),
            "local",
            Utc::now(),
            Some(format!("local-repo:{}", root.display())),
            "local-connector",
        ))
    }

    fn observe_git_status(&self) -> ConnectorResult<NormalizedObservation> {
        let branch = git_output(&self.root, &["rev-parse", "--abbrev-ref", "HEAD"])?;
        let status = git_output(&self.root, &["status", "--porcelain"])?;
        let dirty = !status.trim().is_empty();
        let lines: Vec<&str> = status.lines().filter(|l| !l.is_empty()).collect();
        Ok(NormalizedObservation::new(
            ObservationKind::GitStatus,
            format!(
                "Git branch `{branch}` ({})",
                if dirty { "dirty" } else { "clean" }
            ),
            json!({
                "branch": branch,
                "dirty": dirty,
                "changed_entry_count": lines.len(),
                "status_porcelain": status,
            }),
            "local",
            Utc::now(),
            Some(format!("local-git-status:{branch}")),
            "local-connector",
        ))
    }

    fn observe_recent_commits(&self, limit: usize) -> ConnectorResult<Vec<NormalizedObservation>> {
        let log = git_output(
            &self.root,
            &[
                "log",
                &format!("-{limit}"),
                "--pretty=format:%H%x09%an%x09%aI%x09%s",
            ],
        )?;
        let mut out = Vec::new();
        for line in log.lines().filter(|l| !l.is_empty()) {
            let parts: Vec<&str> = line.splitn(4, '\t').collect();
            if parts.len() < 4 {
                continue;
            }
            let sha = parts[0];
            let author = parts[1];
            let date = parts[2];
            let subject = parts[3];
            let observed_at = chrono::DateTime::parse_from_rfc3339(date)
                .map(|d| d.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| Utc::now());
            out.push(NormalizedObservation::new(
                ObservationKind::Commit,
                format!("Commit {}: {subject}", &sha[..7.min(sha.len())]),
                json!({
                    "sha": sha,
                    "author": author,
                    "date": date,
                    "subject": subject,
                }),
                "local",
                observed_at,
                Some(format!("local-commit:{sha}")),
                "local-connector",
            ));
        }
        Ok(out)
    }

    fn observe_changed_files(&self) -> ConnectorResult<NormalizedObservation> {
        let names = git_output(&self.root, &["diff", "--name-only", "HEAD"])?;
        let mut files: Vec<String> = names
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect();
        // Also include untracked/staged from status
        if let Ok(status) = git_output(&self.root, &["status", "--porcelain"]) {
            for line in status.lines() {
                if line.len() > 3 {
                    let path = line[3..].trim().to_string();
                    if !path.is_empty() && !files.contains(&path) {
                        files.push(path);
                    }
                }
            }
        }
        Ok(NormalizedObservation::new(
            ObservationKind::ChangedFiles,
            format!("{} changed file(s)", files.len()),
            json!({ "files": files }),
            "local",
            Utc::now(),
            Some("local-changed-files".into()),
            "local-connector",
        ))
    }

    fn observe_test_output(&self) -> ConnectorResult<NormalizedObservation> {
        // Optional user-supplied test output paths
        let candidates = [
            self.root.join("test-output.txt"),
            self.root.join(".rivora/test-output.txt"),
            self.root.join("target/rivora-test-output.txt"),
        ];
        for path in candidates {
            if path.is_file() {
                let content =
                    fs::read_to_string(&path).map_err(|e| ConnectorError::Io(e.to_string()))?;
                let failed = content.to_lowercase().contains("fail")
                    || content.to_lowercase().contains("error");
                return Ok(NormalizedObservation::new(
                    ObservationKind::TestOutput,
                    if failed {
                        "Local test output indicates failures"
                    } else {
                        "Local test output captured"
                    },
                    json!({
                        "path": path.display().to_string(),
                        "content": content.chars().take(4000).collect::<String>(),
                        "indicates_failure": failed,
                    }),
                    "local",
                    Utc::now(),
                    Some(format!("local-test-output:{}", path.display())),
                    "local-connector",
                ));
            }
        }
        Err(ConnectorError::Io("no test output file found".into()))
    }

    fn observe_event_files(&self) -> ConnectorResult<Vec<NormalizedObservation>> {
        let dir = self.root.join(".rivora/events");
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in fs::read_dir(&dir).map_err(|e| ConnectorError::Io(e.to_string()))? {
            let entry = entry.map_err(|e| ConnectorError::Io(e.to_string()))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let raw = fs::read_to_string(&path).map_err(|e| ConnectorError::Io(e.to_string()))?;
            let value: serde_json::Value =
                serde_json::from_str(&raw).map_err(|e| ConnectorError::Normalize(e.to_string()))?;
            let summary = value
                .get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or("Local structured event")
                .to_string();
            let key = value
                .get("idempotency_key")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .or_else(|| Some(format!("local-event:{}", path.display())));
            out.push(NormalizedObservation::new(
                ObservationKind::LocalEvent,
                summary,
                value,
                "local",
                Utc::now(),
                key,
                "local-connector",
            ));
        }
        Ok(out)
    }
}

fn git_output(cwd: &Path, args: &[&str]) -> ConnectorResult<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|e| ConnectorError::Io(format!("failed to run git: {e}")))?;
    if !output.status.success() {
        return Err(ConnectorError::Io(format!(
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observes_non_git_directory() {
        let dir = tempfile::tempdir().unwrap();
        let connector = LocalConnector::new(dir.path());
        let obs = connector.observe().unwrap();
        assert!(!obs.is_empty());
        assert!(obs
            .iter()
            .any(|o| matches!(o.kind, ObservationKind::Repository)));
        // No reasoning fields — only observation data.
        for o in &obs {
            assert_eq!(o.source, "local");
            assert!(!o.summary.is_empty());
        }
    }

    #[test]
    fn observes_event_files() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join(".rivora/events");
        fs::create_dir_all(&events).unwrap();
        fs::write(
            events.join("e1.json"),
            r#"{"summary":"deploy started","status":"ok"}"#,
        )
        .unwrap();
        let connector = LocalConnector::new(dir.path());
        let obs = connector.observe().unwrap();
        assert!(obs.iter().any(|o| o.summary.contains("deploy started")));
    }
}
