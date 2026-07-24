//! Map internal typed errors to concise Workspace projections.

use serde::{Deserialize, Serialize};

/// How the user can retry after an error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryGuidance {
    SafeToRetry,
    NotSafeToRetry,
    FixInputThenRetry,
    OpenDoctor,
    ConfigureConnector,
}

/// User-facing error view — no secret leakage, no duplicate prefixes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceErrorView {
    pub title: String,
    pub summary: String,
    pub details: Option<String>,
    pub code: Option<String>,
    pub retry: RetryGuidance,
    pub actions: Vec<String>,
}

impl WorkspaceErrorView {
    pub fn display_lines(&self) -> Vec<String> {
        let mut lines = vec![self.title.clone(), self.summary.clone()];
        if let Some(d) = &self.details {
            lines.push(format!("Details: {d}"));
        }
        lines.push(format!("Next: {}", retry_label(&self.retry)));
        for a in &self.actions {
            lines.push(format!("• {a}"));
        }
        lines
    }
}

fn retry_label(r: &RetryGuidance) -> &'static str {
    match r {
        RetryGuidance::SafeToRetry => "You can retry this action",
        RetryGuidance::NotSafeToRetry => "Do not retry automatically",
        RetryGuidance::FixInputThenRetry => "Fix the input, then retry",
        RetryGuidance::OpenDoctor => "Run Doctor for recovery guidance",
        RetryGuidance::ConfigureConnector => "Configure the connector, then retry",
    }
}

/// Collapse noisy internal validation strings into product language.
pub fn map_error(err: &impl std::fmt::Display) -> WorkspaceErrorView {
    let raw = err.to_string();
    let cleaned = strip_duplicate_validation_prefix(&raw);

    if cleaned.to_lowercase().contains("investigation")
        && (cleaned.to_lowercase().contains("not found")
            || cleaned.to_lowercase().contains("invalid")
            || cleaned.to_lowercase().contains("length"))
    {
        return WorkspaceErrorView {
            title: "Investigation not found".into(),
            summary: "Choose an investigation from search or enter a complete Investigation ID."
                .into(),
            details: Some(cleaned),
            code: Some("investigation_not_found".into()),
            retry: RetryGuidance::FixInputThenRetry,
            actions: vec![
                "Search investigations".into(),
                "List recent investigations".into(),
            ],
        };
    }

    if cleaned.to_lowercase().contains("token")
        || cleaned.to_lowercase().contains("credential")
        || cleaned.to_lowercase().contains("unauthorized")
        || cleaned.to_lowercase().contains("authentication")
    {
        return WorkspaceErrorView {
            title: "Connector credentials missing".into(),
            summary: "Configure credentials outside the conversation. Secrets are never shown."
                .into(),
            details: Some(redact_secrets(&cleaned)),
            code: Some("connector_auth".into()),
            retry: RetryGuidance::ConfigureConnector,
            actions: vec![
                "Open Connectors".into(),
                "Use fixture mode if available".into(),
            ],
        };
    }

    if cleaned.to_lowercase().contains("corrupt")
        || cleaned.to_lowercase().contains("schema")
        || cleaned.to_lowercase().contains("lock")
    {
        return WorkspaceErrorView {
            title: "Local store issue".into(),
            summary: "Runtime data may need recovery. One corrupt record should not block others."
                .into(),
            details: Some(cleaned),
            code: Some("store_health".into()),
            retry: RetryGuidance::OpenDoctor,
            actions: vec!["Open Doctor".into()],
        };
    }

    WorkspaceErrorView {
        title: "Something went wrong".into(),
        summary: truncate(&cleaned, 160),
        details: if cleaned.len() > 160 {
            Some(cleaned)
        } else {
            None
        },
        code: None,
        retry: RetryGuidance::SafeToRetry,
        actions: vec!["Retry".into(), "Open Help".into()],
    }
}

fn strip_duplicate_validation_prefix(s: &str) -> String {
    let mut out = s.trim().to_string();
    // Collapse repeated "validation error:" prefixes.
    loop {
        let lower = out.to_lowercase();
        if let Some(rest) = lower.strip_prefix("validation error:") {
            let idx = out.len() - rest.len();
            out = out[idx..].trim().to_string();
            continue;
        }
        if let Some(rest) = lower.strip_prefix("error:") {
            let idx = out.len() - rest.len();
            out = out[idx..].trim().to_string();
            continue;
        }
        break;
    }
    out
}

fn redact_secrets(s: &str) -> String {
    let mut out = s.to_string();
    for key in ["ghp_", "gho_", "github_pat_", "Bearer ", "token="] {
        if let Some(idx) = out.find(key) {
            let end = (idx + key.len() + 8).min(out.len());
            out.replace_range(idx..end, &format!("{key}***"));
        }
    }
    out
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{t}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_duplicate_validation_prefix() {
        let v = map_error(
            &"validation error: validation error: invalid investigation id: invalid length",
        );
        assert_eq!(v.title, "Investigation not found");
        assert!(!v.summary.contains("validation error: validation error:"));
        assert!(v.details.is_some());
    }

    #[test]
    fn redacts_tokenish_details() {
        let v = map_error(&"unauthorized token=ghp_secrettokenvalue123");
        let details = v.details.unwrap_or_default();
        assert!(!details.contains("secrettokenvalue123"));
    }
}
