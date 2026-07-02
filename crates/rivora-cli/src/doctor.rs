//! `rivora doctor` — local-first diagnostic command.
//!
//! `rivora doctor` checks the local `.rivora/` store, whether it is
//! gitignored, and the provider tokens Rivora can see from the environment.
//! It never prints token values, never requires network access, and never
//! takes infrastructure actions. It is intentionally separate from
//! `rivora slack doctor`, which validates Slack-specific setup.

use rivora_errors::Result;
use serde::Serialize;

use crate::{
    display_path, store_from_env, DoctorOptions, LocalMemoryStore, StoreSnapshot, EVIDENCE_FILE,
    FEEDBACK_FILE, MEMORIES_FILE, RECEIPTS_FILE,
};

/// Provider tokens `rivora doctor` reports on. Values are never read or
/// printed; only `set` / `not set` is reported.
const PROVIDER_TOKEN_VARS: &[&str] = &[
    "GITHUB_TOKEN",
    "VERCEL_TOKEN",
    "CLOUDFLARE_API_TOKEN",
    "CF_API_TOKEN",
    "SENTRY_AUTH_TOKEN",
    "SENTRY_TOKEN",
    "SLACK_BOT_TOKEN",
    "SLACK_APP_TOKEN",
    "SLACK_SIGNING_SECRET",
];

/// Run `rivora doctor`.
pub fn run(default_store: &LocalMemoryStore, options: DoctorOptions) -> Result<String> {
    // Honor RIVORA_STORE_DIR so doctor reports on the same store Slack and
    // other commands would use.
    let store = store_from_env(&default_store.root);
    let report = DoctorReport::collect(&store);

    if options.json {
        Ok(serde_json::to_string_pretty(&report)?)
    } else {
        Ok(report.render())
    }
}

#[derive(Debug, Serialize)]
struct DoctorReport {
    working_directory: String,
    store_directory: String,
    store: DoctorStoreReport,
    providers: DoctorProviderReport,
    no_tokens_printed: bool,
    no_infrastructure_actions: bool,
    next: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
struct DoctorStoreReport {
    found: bool,
    gitignored: &'static str,
    evidence: StoreFileStatus,
    memories: StoreFileStatus,
    feedback: StoreFileStatus,
    receipts: StoreFileStatus,
}

#[derive(Debug, Serialize)]
struct DoctorProviderReport {
    tokens: Vec<DoctorTokenStatus>,
}

#[derive(Debug, Serialize)]
struct DoctorTokenStatus {
    name: &'static str,
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct StoreFileStatus {
    found: bool,
}

impl DoctorReport {
    fn collect(store: &LocalMemoryStore) -> Self {
        let store_dir = store.store_dir();
        let found = store_dir.exists();
        let snapshot = if found {
            store.load().unwrap_or_default()
        } else {
            StoreSnapshot::default()
        };

        let gitignored = gitignore_status_for_store(store);

        Self {
            working_directory: display_path(&store.root),
            store_directory: display_path(&store.store_dir()),
            store: DoctorStoreReport {
                found,
                gitignored,
                evidence: StoreFileStatus {
                    found: !snapshot.evidence.is_empty() || store.evidence_path().exists(),
                },
                memories: StoreFileStatus {
                    found: store.memories_path().exists(),
                },
                feedback: StoreFileStatus {
                    found: store.feedback_path().exists(),
                },
                receipts: StoreFileStatus {
                    found: store.receipts_path().exists(),
                },
            },
            providers: DoctorProviderReport {
                tokens: PROVIDER_TOKEN_VARS
                    .iter()
                    .map(|name| DoctorTokenStatus {
                        name,
                        status: token_status(name),
                    })
                    .collect(),
            },
            no_tokens_printed: true,
            no_infrastructure_actions: true,
            next: next_steps(found, &snapshot),
        }
    }

    fn render(&self) -> String {
        let mut lines = vec!["Rivora Doctor".to_string(), String::new()];

        lines.push(format!("Working directory: {}", self.working_directory));
        lines.push(format!("Store directory: {}", self.store_directory));
        lines.push(String::new());

        lines.push("Local store:".to_string());
        lines.push(format!(
            "- .rivora/: {}",
            if self.store.found {
                "found"
            } else {
                "not found"
            }
        ));
        lines.push(format!(
            "- {EVIDENCE_FILE}: {}",
            file_label(self.store.evidence.found)
        ));
        lines.push(format!(
            "- {MEMORIES_FILE}: {}",
            file_label(self.store.memories.found)
        ));
        lines.push(format!(
            "- {FEEDBACK_FILE}: {}",
            file_label(self.store.feedback.found)
        ));
        lines.push(format!(
            "- {RECEIPTS_FILE}: {}",
            file_label(self.store.receipts.found)
        ));
        lines.push(format!("- .rivora/ gitignored: {}", self.store.gitignored));

        lines.push(String::new());
        lines.push("Provider tokens:".to_string());
        for token in &self.providers.tokens {
            lines.push(format!("- {}: {}", token.name, token.status));
        }

        lines.push(String::new());
        lines.push("No tokens were printed.".to_string());
        lines.push("No infrastructure actions were taken.".to_string());

        if !self.next.is_empty() {
            lines.push(String::new());
            lines.push("Next:".to_string());
            for step in &self.next {
                lines.push((*step).to_string());
            }
        }

        lines.join("\n")
    }
}

fn file_label(found: bool) -> &'static str {
    if found {
        "found"
    } else {
        "not found"
    }
}

fn token_status(name: &str) -> &'static str {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => "set",
        _ => "not set",
    }
}

/// Determine whether `.rivora/` is gitignored by inspecting the `.gitignore`
/// at the store root. Returns `"yes"`, `"no"`, or `"unknown"` (no
/// `.gitignore` present). Does not invoke git and requires no network.
fn gitignore_status_for_store(store: &LocalMemoryStore) -> &'static str {
    let gitignore = store.root.join(".gitignore");
    let Ok(contents) = std::fs::read_to_string(&gitignore) else {
        return "unknown";
    };
    let ignored = contents.lines().any(|line| {
        let trimmed = line.trim();
        trimmed == ".rivora/" || trimmed == ".rivora"
    });
    if ignored {
        "yes"
    } else {
        "no"
    }
}

fn next_steps(store_found: bool, snapshot: &StoreSnapshot) -> Vec<&'static str> {
    if !store_found {
        return vec!["rivora init", "rivora ingest git --repo . --limit 20"];
    }
    if snapshot.evidence.is_empty() {
        return vec![
            "rivora ingest git --repo . --limit 20",
            "rivora demo --scenario multi-source-release",
        ];
    }
    vec!["rivora ask \"what changed?\""]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LocalMemoryStore;
    use rivora_connectors::{EvidenceId, EvidenceItem, EvidenceKind, EvidenceSource};
    use std::fs;
    use tempfile::TempDir;

    fn temp_store() -> (TempDir, LocalMemoryStore) {
        let temp = TempDir::new().unwrap();
        let store = LocalMemoryStore::new(temp.path());
        (temp, store)
    }

    fn doctor_evidence() -> EvidenceItem {
        EvidenceItem {
            id: EvidenceId::new("git:commit:doctor-1").unwrap(),
            kind: EvidenceKind::GitCommit,
            source: EvidenceSource::local_git("."),
            title: "doctor commit".to_string(),
            summary: "doctor summary".to_string(),
            body: String::new(),
            service: Some("checkout".to_string()),
            files_changed: Vec::new(),
            timestamp: Some("2026-06-28T00:00:00Z".to_string()),
            author: None,
            tags: Vec::new(),
            refs: Vec::new(),
            confidence: 0.5,
        }
    }

    #[test]
    fn doctor_reports_present_store_and_files() {
        let (_temp, store) = temp_store();
        store.init().unwrap();
        store.append_evidence([doctor_evidence()]).unwrap();
        let output = run(&store, DoctorOptions::default()).unwrap();
        assert!(output.contains(".rivora/: found"));
        assert!(output.contains("evidence.json: found"));
        assert!(output.contains("memories.json: found"));
        assert!(output.contains("feedback.json: found"));
        assert!(output.contains("receipts.json: found"));
        assert!(output.contains("Next:"));
        assert!(output.contains("rivora ask \"what changed?\""));
    }

    #[test]
    fn doctor_reports_missing_store() {
        let (_temp, store) = temp_store();
        let output = run(&store, DoctorOptions::default()).unwrap();
        assert!(output.contains("Rivora Doctor"));
        assert!(output.contains(".rivora/: not found"));
        assert!(output.contains("gitignored: unknown"));
        assert!(output.contains("No tokens were printed."));
        assert!(output.contains("No infrastructure actions were taken."));
        assert!(output.contains("rivora init"));
    }

    #[test]
    fn doctor_suggests_ingest_when_store_has_no_evidence() {
        let (_temp, store) = temp_store();
        store.init().unwrap();
        let output = run(&store, DoctorOptions::default()).unwrap();
        assert!(output.contains("Next:"));
        assert!(output.contains("rivora ingest git"));
        assert!(!output.contains("rivora ask \"what changed?\""));
    }

    #[test]
    fn doctor_honors_rivora_store_dir() {
        let temp = TempDir::new().unwrap();
        let custom = temp.path().join("custom-store");
        fs::create_dir_all(&custom).unwrap();
        fs::write(custom.join("evidence.json"), "[]\n").unwrap();
        std::env::set_var("RIVORA_STORE_DIR", "custom-store");
        let default_store = LocalMemoryStore::new(temp.path());
        let output = run(&default_store, DoctorOptions::default()).unwrap();
        std::env::remove_var("RIVORA_STORE_DIR");

        assert!(output.contains("custom-store"));
        assert!(output.contains(".rivora/: found"));
        assert!(output.contains("evidence.json: found"));
    }

    #[test]
    fn doctor_redacts_provider_tokens() {
        let (_temp, store) = temp_store();
        std::env::set_var("GITHUB_TOKEN", "ghp_secret_doctor_token");
        let output = run(&store, DoctorOptions::default()).unwrap();
        std::env::remove_var("GITHUB_TOKEN");

        assert!(output.contains("GITHUB_TOKEN: set"));
        assert!(!output.contains("ghp_secret_doctor_token"));
    }

    #[test]
    fn doctor_json_is_stable_and_safe() {
        let (_temp, store) = temp_store();
        std::env::set_var("VERCEL_TOKEN", "vercel_secret_value");
        let output = run(&store, DoctorOptions { json: true }).unwrap();
        std::env::remove_var("VERCEL_TOKEN");

        let json: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(json["no_tokens_printed"], true);
        assert_eq!(json["no_infrastructure_actions"], true);
        assert_eq!(json["store"]["found"], false);
        let tokens = json["providers"]["tokens"].as_array().unwrap();
        assert!(tokens.iter().any(|t| t["name"] == "GITHUB_TOKEN"));
        assert!(tokens.iter().any(|t| t["name"] == "VERCEL_TOKEN"));
        assert!(tokens.iter().any(|t| t["name"] == "CLOUDFLARE_API_TOKEN"));
        assert!(tokens.iter().any(|t| t["name"] == "CF_API_TOKEN"));
        assert!(tokens.iter().any(|t| t["name"] == "SLACK_BOT_TOKEN"));
        assert!(tokens.iter().any(|t| t["name"] == "SLACK_APP_TOKEN"));
        assert!(tokens.iter().any(|t| t["name"] == "SLACK_SIGNING_SECRET"));
        assert!(!output.contains("vercel_secret_value"));
    }

    #[test]
    fn doctor_detects_gitignored_store() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join(".gitignore"), "/target/\n.rivora/\n").unwrap();
        let store = LocalMemoryStore::new(temp.path());
        let output = run(&store, DoctorOptions::default()).unwrap();
        assert!(output.contains(".rivora/ gitignored: yes"));
    }

    #[test]
    fn doctor_reports_no_when_gitignore_lacks_rivora() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join(".gitignore"), "/target/\n").unwrap();
        let store = LocalMemoryStore::new(temp.path());
        let output = run(&store, DoctorOptions::default()).unwrap();
        assert!(output.contains(".rivora/ gitignored: no"));
    }

    #[test]
    fn doctor_never_takes_infrastructure_actions() {
        let (_temp, store) = temp_store();
        let output = run(&store, DoctorOptions::default()).unwrap();
        assert!(!crate::output_contains_infrastructure_action(&output));
    }
}
