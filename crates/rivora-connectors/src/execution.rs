//! Bounded external execution adapters (RFC-026).
//!
//! These are distinct from observation connectors. They implement
//! `rivora::ExecutionCapability` and are invoked only by the Runtime.
//! Observation modules remain read-only.

use std::collections::HashMap;

use rivora::{
    CapabilityExecutionResult, CapabilityInvocation, CapabilityRiskLevel,
    CapabilityStateObservation, CapabilityStateQuery, DryRunResult, ExecutionCapability,
    ExecutionCapabilityDescriptor, ExecutionPolicyDecision, ExecutionPolicyDecisionKind,
    RivoraError, RivoraResult, RollbackMetadata,
};

/// Register the standard v0.6 bounded GitHub execution adapters when a token is present.
///
/// Without a token, adapters are still registered but live execute fails preconditions
/// with a clear credential error (dry-run/plan validation still works).
pub fn register_github_execution_capabilities(
    registry: &rivora::ExecutionCapabilityRegistry,
    repository: impl Into<String>,
    token: Option<String>,
) {
    let repository = repository.into();
    let api_base = "https://api.github.com".to_string();
    registry.register(std::sync::Arc::new(GitHubIssueCommentCapability {
        repository: repository.clone(),
        token: token.clone(),
        api_base: api_base.clone(),
    }));
    registry.register(std::sync::Arc::new(GitHubIssueLabelCapability {
        repository: repository.clone(),
        token: token.clone(),
        api_base: api_base.clone(),
    }));
    registry.register(std::sync::Arc::new(GitHubIssueCreateCapability {
        repository: repository.clone(),
        token: token.clone(),
        api_base: api_base.clone(),
    }));
    registry.register(std::sync::Arc::new(GitHubDraftPrCapability {
        repository: repository.clone(),
        token: token.clone(),
        api_base: api_base.clone(),
    }));
    registry.register(std::sync::Arc::new(GitHubWorkflowDispatchCapability {
        repository,
        token,
        api_base,
    }));
}

fn policy_low_risk(capability_id: &str) -> ExecutionPolicyDecision {
    ExecutionPolicyDecision {
        decision: ExecutionPolicyDecisionKind::AllowedWithApproval,
        reasons: vec![format!("{capability_id} is a bounded low-risk write")],
        risk_level: CapabilityRiskLevel::LowRiskWrite,
        dry_run_permitted: true,
        live_execution_permitted: true,
        evaluated_at: chrono::Utc::now(),
    }
}

fn policy_bounded(capability_id: &str) -> ExecutionPolicyDecision {
    ExecutionPolicyDecision {
        decision: ExecutionPolicyDecisionKind::AllowedWithApproval,
        reasons: vec![format!("{capability_id} is a bounded write")],
        risk_level: CapabilityRiskLevel::BoundedWrite,
        dry_run_permitted: true,
        live_execution_permitted: true,
        evaluated_at: chrono::Utc::now(),
    }
}

fn require_token(token: &Option<String>) -> RivoraResult<&str> {
    token
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .ok_or_else(|| {
            RivoraError::precondition(
                "GitHub token required for live execution (set GITHUB_TOKEN or register with token)",
            )
        })
}

fn input_str(inputs: &serde_json::Value, key: &str) -> Option<String> {
    inputs
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn client() -> RivoraResult<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .user_agent("rivora-execution/0.6")
        .build()
        .map_err(|e| RivoraError::validation(format!("http client: {e}")))
}

// ---------------------------------------------------------------------------
// github.issue.comment
// ---------------------------------------------------------------------------

/// Post a comment on a GitHub issue (LowRiskWrite).
pub struct GitHubIssueCommentCapability {
    /// owner/repo
    pub repository: String,
    /// Optional token
    pub token: Option<String>,
    /// API base
    pub api_base: String,
}

impl ExecutionCapability for GitHubIssueCommentCapability {
    fn descriptor(&self) -> ExecutionCapabilityDescriptor {
        ExecutionCapabilityDescriptor {
            capability_id: "github.issue.comment".into(),
            version: "1".into(),
            risk_level: CapabilityRiskLevel::LowRiskWrite,
            supported_actions: vec!["create_comment".into()],
            required_inputs: vec!["issue_number".into(), "body".into()],
            supports_dry_run: true,
            idempotency_behavior:
                "client idempotency key; duplicate comments possible if key differs".into(),
            reversibility: "comment may be deleted manually; no auto-rollback".into(),
            verification_method: "GET issue comments and match body".into(),
            credential_requirements: vec!["GITHUB_TOKEN".into()],
            target_restrictions: vec![self.repository.clone()],
            failure_semantics: "API errors fail the action; no partial comment".into(),
            description: "Post a comment on a GitHub issue".into(),
        }
    }

    fn dry_run(&self, request: &CapabilityInvocation) -> RivoraResult<DryRunResult> {
        let issue = input_str(&request.inputs, "issue_number").unwrap_or_default();
        let body = input_str(&request.inputs, "body").unwrap_or_default();
        Ok(DryRunResult {
            actions: vec!["create_comment".into()],
            target: format!("{}/issues/{issue}", self.repository),
            expected_mutations: vec![format!("create comment on issue {issue}")],
            required_permissions: vec!["issues:write".into()],
            current_state: None,
            predicted_state: Some(format!("comment body length {}", body.len())),
            risks: vec!["public comment visible to repository collaborators".into()],
            policy_decision: policy_low_risk("github.issue.comment"),
            missing_preconditions: if issue.is_empty() || body.is_empty() {
                vec!["issue_number and body required".into()]
            } else {
                vec![]
            },
            verification_steps: vec!["GET issue comments contains body".into()],
            rollback_options: vec!["manual comment deletion".into()],
            simulated: false,
        })
    }

    fn execute(&self, request: &CapabilityInvocation) -> RivoraResult<CapabilityExecutionResult> {
        let token = require_token(&self.token)?;
        let issue = input_str(&request.inputs, "issue_number")
            .ok_or_else(|| RivoraError::validation("issue_number required"))?;
        let body = input_str(&request.inputs, "body")
            .ok_or_else(|| RivoraError::validation("body required"))?;
        let url = format!(
            "{}/repos/{}/issues/{}/comments",
            self.api_base.trim_end_matches('/'),
            self.repository,
            issue
        );
        let client = client()?;
        let response = client
            .post(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .json(&serde_json::json!({ "body": body }))
            .send()
            .map_err(|e| RivoraError::validation(format!("github comment request failed: {e}")))?;
        let status = response.status();
        let json: serde_json::Value = response.json().unwrap_or(serde_json::json!({}));
        if !status.is_success() {
            return Ok(CapabilityExecutionResult {
                success: false,
                result_status: "failed".into(),
                request_summary: format!("POST comment issue {issue}"),
                response_summary: format!("HTTP {status}"),
                changed_resources: vec![],
                unchanged_resources: vec![format!("issue/{issue}")],
                external_identifiers: vec![],
                warnings: vec![],
                rollback: RollbackMetadata::default(),
                verification_requirements: vec![],
                evidence_refs: vec![],
                error: Some(format!("github API error: {status}")),
                duplicate_suppressed: false,
            });
        }
        let id = json
            .get("id")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unknown".into());
        Ok(CapabilityExecutionResult {
            success: true,
            result_status: "success".into(),
            request_summary: format!("POST comment issue {issue}"),
            response_summary: format!("created comment {id}"),
            changed_resources: vec![format!("issue/{issue}/comment/{id}")],
            unchanged_resources: vec![],
            external_identifiers: vec![id.clone()],
            warnings: vec![],
            rollback: RollbackMetadata {
                available: false,
                capability_id: None,
                inputs: None,
                risks: vec!["comment deletion not automated".into()],
                verification: Some("GET comment".into()),
                irreversible_effects: vec!["comment remains until manually deleted".into()],
            },
            verification_requirements: vec![format!("comment {id} exists")],
            evidence_refs: vec![format!("github-comment:{id}")],
            error: None,
            duplicate_suppressed: false,
        })
    }

    fn observe_state(
        &self,
        query: &CapabilityStateQuery,
    ) -> RivoraResult<CapabilityStateObservation> {
        let token = match require_token(&self.token) {
            Ok(t) => t,
            Err(e) => {
                return Ok(CapabilityStateObservation {
                    resource_key: self.repository.clone(),
                    fields: HashMap::new(),
                    summary: e.to_string(),
                    observed: false,
                    error: Some(e.to_string()),
                });
            }
        };
        let issue = input_str(&query.inputs, "issue_number").unwrap_or_default();
        let url = format!(
            "{}/repos/{}/issues/{}/comments",
            self.api_base.trim_end_matches('/'),
            self.repository,
            issue
        );
        let client = client()?;
        let response = client
            .get(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .send()
            .map_err(|e| RivoraError::validation(format!("github observe failed: {e}")))?;
        let ok = response.status().is_success();
        let body = response.text().unwrap_or_default();
        let mut fields = HashMap::new();
        fields.insert("comments_json_len".into(), body.len().to_string());
        if let Some(id) = query.external_identifiers.first() {
            fields.insert("contains_comment_id".into(), body.contains(id).to_string());
        }
        Ok(CapabilityStateObservation {
            resource_key: format!("issue/{issue}/comments"),
            fields,
            summary: if ok {
                "comments fetched".into()
            } else {
                "comment fetch failed".into()
            },
            observed: ok,
            error: if ok {
                None
            } else {
                Some("GET comments failed".into())
            },
        })
    }
}

// ---------------------------------------------------------------------------
// github.issue.label
// ---------------------------------------------------------------------------

/// Add or remove a label on a GitHub issue (LowRiskWrite).
pub struct GitHubIssueLabelCapability {
    /// owner/repo
    pub repository: String,
    /// Optional token
    pub token: Option<String>,
    /// API base
    pub api_base: String,
}

impl ExecutionCapability for GitHubIssueLabelCapability {
    fn descriptor(&self) -> ExecutionCapabilityDescriptor {
        ExecutionCapabilityDescriptor {
            capability_id: "github.issue.label".into(),
            version: "1".into(),
            risk_level: CapabilityRiskLevel::LowRiskWrite,
            supported_actions: vec!["add_label".into(), "remove_label".into()],
            required_inputs: vec!["issue_number".into(), "label".into()],
            supports_dry_run: true,
            idempotency_behavior: "add is naturally idempotent; remove is idempotent when absent"
                .into(),
            reversibility: "swap add/remove".into(),
            verification_method: "GET issue labels".into(),
            credential_requirements: vec!["GITHUB_TOKEN".into()],
            target_restrictions: vec![self.repository.clone()],
            failure_semantics: "failed label ops leave prior labels".into(),
            description: "Add or remove a label on a GitHub issue".into(),
        }
    }

    fn dry_run(&self, request: &CapabilityInvocation) -> RivoraResult<DryRunResult> {
        let issue = input_str(&request.inputs, "issue_number").unwrap_or_default();
        let label = input_str(&request.inputs, "label").unwrap_or_default();
        Ok(DryRunResult {
            actions: vec![request.action_name.clone()],
            target: format!("{}/issues/{issue}", self.repository),
            expected_mutations: vec![format!("{} label {label}", request.action_name)],
            required_permissions: vec!["issues:write".into()],
            current_state: None,
            predicted_state: Some(format!("label {label} {}", request.action_name)),
            risks: vec!["label changes visible to collaborators".into()],
            policy_decision: policy_low_risk("github.issue.label"),
            missing_preconditions: if issue.is_empty() || label.is_empty() {
                vec!["issue_number and label required".into()]
            } else {
                vec![]
            },
            verification_steps: vec!["GET issue labels".into()],
            rollback_options: vec!["inverse add/remove label".into()],
            simulated: false,
        })
    }

    fn execute(&self, request: &CapabilityInvocation) -> RivoraResult<CapabilityExecutionResult> {
        let token = require_token(&self.token)?;
        let issue = input_str(&request.inputs, "issue_number")
            .ok_or_else(|| RivoraError::validation("issue_number required"))?;
        let label = input_str(&request.inputs, "label")
            .ok_or_else(|| RivoraError::validation("label required"))?;
        let client = client()?;
        let (method_url, method) = match request.action_name.as_str() {
            "add_label" => (
                format!(
                    "{}/repos/{}/issues/{}/labels",
                    self.api_base.trim_end_matches('/'),
                    self.repository,
                    issue
                ),
                "POST",
            ),
            "remove_label" => (
                format!(
                    "{}/repos/{}/issues/{}/labels/{}",
                    self.api_base.trim_end_matches('/'),
                    self.repository,
                    issue,
                    urlencoding_lite(&label)
                ),
                "DELETE",
            ),
            other => {
                return Err(RivoraError::validation(format!(
                    "unsupported action {other}"
                )));
            }
        };
        let builder = if method == "POST" {
            client
                .post(&method_url)
                .json(&serde_json::json!({ "labels": [label] }))
        } else {
            client.delete(&method_url)
        };
        let response = builder
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .send()
            .map_err(|e| RivoraError::validation(format!("github label request failed: {e}")))?;
        let status = response.status();
        let success = status.is_success();
        Ok(CapabilityExecutionResult {
            success,
            result_status: if success {
                "success".into()
            } else {
                "failed".into()
            },
            request_summary: format!("{method} label {label} on issue {issue}"),
            response_summary: format!("HTTP {status}"),
            changed_resources: if success {
                vec![format!("issue/{issue}/label/{label}")]
            } else {
                vec![]
            },
            unchanged_resources: if success {
                vec![]
            } else {
                vec![format!("issue/{issue}")]
            },
            external_identifiers: vec![format!("issue:{issue}:label:{label}")],
            warnings: vec![],
            rollback: RollbackMetadata {
                available: true,
                capability_id: Some("github.issue.label".into()),
                inputs: Some(serde_json::json!({
                    "issue_number": issue,
                    "label": label,
                    "inverse": if request.action_name == "add_label" { "remove_label" } else { "add_label" }
                })),
                risks: vec![],
                verification: Some("GET labels".into()),
                irreversible_effects: vec![],
            },
            verification_requirements: vec![format!("label {label} state matches action")],
            evidence_refs: vec![format!("github-label:{issue}:{label}")],
            error: if success {
                None
            } else {
                Some(format!("github API error: {status}"))
            },
            duplicate_suppressed: false,
        })
    }

    fn observe_state(
        &self,
        query: &CapabilityStateQuery,
    ) -> RivoraResult<CapabilityStateObservation> {
        let token = match require_token(&self.token) {
            Ok(t) => t,
            Err(e) => {
                return Ok(CapabilityStateObservation {
                    resource_key: self.repository.clone(),
                    fields: HashMap::new(),
                    summary: e.to_string(),
                    observed: false,
                    error: Some(e.to_string()),
                });
            }
        };
        let issue = input_str(&query.inputs, "issue_number").unwrap_or_default();
        let label = input_str(&query.inputs, "label").unwrap_or_default();
        let url = format!(
            "{}/repos/{}/issues/{}/labels",
            self.api_base.trim_end_matches('/'),
            self.repository,
            issue
        );
        let client = client()?;
        let response = client
            .get(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .send()
            .map_err(|e| RivoraError::validation(format!("github observe labels failed: {e}")))?;
        let ok = response.status().is_success();
        let text = response.text().unwrap_or_default();
        let mut fields = HashMap::new();
        fields.insert("has_label".into(), text.contains(&label).to_string());
        Ok(CapabilityStateObservation {
            resource_key: format!("issue/{issue}/labels"),
            fields,
            summary: if ok {
                "labels fetched".into()
            } else {
                "labels fetch failed".into()
            },
            observed: ok,
            error: if ok {
                None
            } else {
                Some("GET labels failed".into())
            },
        })
    }
}

// ---------------------------------------------------------------------------
// github.issue.create
// ---------------------------------------------------------------------------

/// Create a GitHub issue (BoundedWrite).
pub struct GitHubIssueCreateCapability {
    /// owner/repo
    pub repository: String,
    /// Optional token
    pub token: Option<String>,
    /// API base
    pub api_base: String,
}

impl ExecutionCapability for GitHubIssueCreateCapability {
    fn descriptor(&self) -> ExecutionCapabilityDescriptor {
        ExecutionCapabilityDescriptor {
            capability_id: "github.issue.create".into(),
            version: "1".into(),
            risk_level: CapabilityRiskLevel::BoundedWrite,
            supported_actions: vec!["create_issue".into()],
            required_inputs: vec!["title".into()],
            supports_dry_run: true,
            idempotency_behavior: "client key; GitHub may create duplicates if key differs".into(),
            reversibility: "issue may be closed; not deleted automatically".into(),
            verification_method: "GET issue by number".into(),
            credential_requirements: vec!["GITHUB_TOKEN".into()],
            target_restrictions: vec![self.repository.clone()],
            failure_semantics: "failed create leaves no issue".into(),
            description: "Create a GitHub issue".into(),
        }
    }

    fn dry_run(&self, request: &CapabilityInvocation) -> RivoraResult<DryRunResult> {
        let title = input_str(&request.inputs, "title").unwrap_or_default();
        Ok(DryRunResult {
            actions: vec!["create_issue".into()],
            target: self.repository.clone(),
            expected_mutations: vec![format!("create issue titled {title}")],
            required_permissions: vec!["issues:write".into()],
            current_state: None,
            predicted_state: Some("new open issue".into()),
            risks: vec!["creates durable tracker item".into()],
            policy_decision: policy_bounded("github.issue.create"),
            missing_preconditions: if title.is_empty() {
                vec!["title required".into()]
            } else {
                vec![]
            },
            verification_steps: vec!["GET issue exists with title".into()],
            rollback_options: vec!["close issue manually".into()],
            simulated: false,
        })
    }

    fn execute(&self, request: &CapabilityInvocation) -> RivoraResult<CapabilityExecutionResult> {
        let token = require_token(&self.token)?;
        let title = input_str(&request.inputs, "title")
            .ok_or_else(|| RivoraError::validation("title required"))?;
        let body = input_str(&request.inputs, "body").unwrap_or_default();
        let url = format!(
            "{}/repos/{}/issues",
            self.api_base.trim_end_matches('/'),
            self.repository
        );
        let client = client()?;
        let response = client
            .post(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .json(&serde_json::json!({ "title": title, "body": body }))
            .send()
            .map_err(|e| RivoraError::validation(format!("github create issue failed: {e}")))?;
        let status = response.status();
        let json: serde_json::Value = response.json().unwrap_or(serde_json::json!({}));
        if !status.is_success() {
            return Ok(CapabilityExecutionResult {
                success: false,
                result_status: "failed".into(),
                request_summary: format!("POST issue {title}"),
                response_summary: format!("HTTP {status}"),
                changed_resources: vec![],
                unchanged_resources: vec![self.repository.clone()],
                external_identifiers: vec![],
                warnings: vec![],
                rollback: RollbackMetadata::default(),
                verification_requirements: vec![],
                evidence_refs: vec![],
                error: Some(format!("github API error: {status}")),
                duplicate_suppressed: false,
            });
        }
        let number = json
            .get("number")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "?".into());
        Ok(CapabilityExecutionResult {
            success: true,
            result_status: "success".into(),
            request_summary: format!("POST issue {title}"),
            response_summary: format!("created issue #{number}"),
            changed_resources: vec![format!("issue/{number}")],
            unchanged_resources: vec![],
            external_identifiers: vec![number.clone()],
            warnings: vec![],
            rollback: RollbackMetadata {
                available: false,
                capability_id: None,
                inputs: Some(serde_json::json!({"issue_number": number})),
                risks: vec!["close is not automated".into()],
                verification: Some("GET issue".into()),
                irreversible_effects: vec!["issue number retained".into()],
            },
            verification_requirements: vec![format!("issue {number} open")],
            evidence_refs: vec![format!("github-issue:{number}")],
            error: None,
            duplicate_suppressed: false,
        })
    }

    fn observe_state(
        &self,
        query: &CapabilityStateQuery,
    ) -> RivoraResult<CapabilityStateObservation> {
        let token = match require_token(&self.token) {
            Ok(t) => t,
            Err(e) => {
                return Ok(CapabilityStateObservation {
                    resource_key: self.repository.clone(),
                    fields: HashMap::new(),
                    summary: e.to_string(),
                    observed: false,
                    error: Some(e.to_string()),
                });
            }
        };
        let number = query
            .external_identifiers
            .first()
            .cloned()
            .unwrap_or_default();
        if number.is_empty() {
            return Ok(CapabilityStateObservation {
                resource_key: "issue/?".into(),
                fields: HashMap::new(),
                summary: "no issue number".into(),
                observed: false,
                error: Some("missing external id".into()),
            });
        }
        let url = format!(
            "{}/repos/{}/issues/{}",
            self.api_base.trim_end_matches('/'),
            self.repository,
            number
        );
        let client = client()?;
        let response = client
            .get(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .send()
            .map_err(|e| RivoraError::validation(format!("github get issue failed: {e}")))?;
        let ok = response.status().is_success();
        let json: serde_json::Value = response.json().unwrap_or(serde_json::json!({}));
        let mut fields = HashMap::new();
        if let Some(t) = json.get("title").and_then(|v| v.as_str()) {
            fields.insert("title".into(), t.into());
        }
        if let Some(s) = json.get("state").and_then(|v| v.as_str()) {
            fields.insert("state".into(), s.into());
        }
        Ok(CapabilityStateObservation {
            resource_key: format!("issue/{number}"),
            fields,
            summary: if ok {
                format!("issue {number} observed")
            } else {
                format!("issue {number} missing")
            },
            observed: ok,
            error: if ok {
                None
            } else {
                Some("GET issue failed".into())
            },
        })
    }
}

// ---------------------------------------------------------------------------
// github.pull_request.create_draft
// ---------------------------------------------------------------------------

/// Create a draft PR from an existing branch (BoundedWrite).
pub struct GitHubDraftPrCapability {
    /// owner/repo
    pub repository: String,
    /// Optional token
    pub token: Option<String>,
    /// API base
    pub api_base: String,
}

impl ExecutionCapability for GitHubDraftPrCapability {
    fn descriptor(&self) -> ExecutionCapabilityDescriptor {
        ExecutionCapabilityDescriptor {
            capability_id: "github.pull_request.create_draft".into(),
            version: "1".into(),
            risk_level: CapabilityRiskLevel::BoundedWrite,
            supported_actions: vec!["create_draft_pr".into()],
            required_inputs: vec!["title".into(), "head".into(), "base".into()],
            supports_dry_run: true,
            idempotency_behavior: "client key; natural head/base uniqueness may apply".into(),
            reversibility: "PR may be closed; no force operations".into(),
            verification_method: "GET pull request".into(),
            credential_requirements: vec!["GITHUB_TOKEN".into()],
            target_restrictions: vec![self.repository.clone()],
            failure_semantics: "failed create leaves no PR".into(),
            description: "Create a draft pull request from an existing branch".into(),
        }
    }

    fn dry_run(&self, request: &CapabilityInvocation) -> RivoraResult<DryRunResult> {
        let head = input_str(&request.inputs, "head").unwrap_or_default();
        let base = input_str(&request.inputs, "base").unwrap_or_default();
        Ok(DryRunResult {
            actions: vec!["create_draft_pr".into()],
            target: self.repository.clone(),
            expected_mutations: vec![format!("draft PR {head} -> {base}")],
            required_permissions: vec!["pull_requests:write".into()],
            current_state: None,
            predicted_state: Some("draft PR open".into()),
            risks: vec!["creates reviewable change proposal".into()],
            policy_decision: policy_bounded("github.pull_request.create_draft"),
            missing_preconditions: if head.is_empty() || base.is_empty() {
                vec!["head and base branches required".into()]
            } else {
                vec![]
            },
            verification_steps: vec!["GET PR draft=true".into()],
            rollback_options: vec!["close PR".into()],
            simulated: false,
        })
    }

    fn execute(&self, request: &CapabilityInvocation) -> RivoraResult<CapabilityExecutionResult> {
        let token = require_token(&self.token)?;
        let title = input_str(&request.inputs, "title")
            .ok_or_else(|| RivoraError::validation("title required"))?;
        let head = input_str(&request.inputs, "head")
            .ok_or_else(|| RivoraError::validation("head required"))?;
        let base = input_str(&request.inputs, "base")
            .ok_or_else(|| RivoraError::validation("base required"))?;
        let body = input_str(&request.inputs, "body").unwrap_or_default();
        let url = format!(
            "{}/repos/{}/pulls",
            self.api_base.trim_end_matches('/'),
            self.repository
        );
        let client = client()?;
        let response = client
            .post(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .json(&serde_json::json!({
                "title": title,
                "head": head,
                "base": base,
                "body": body,
                "draft": true
            }))
            .send()
            .map_err(|e| RivoraError::validation(format!("github create PR failed: {e}")))?;
        let status = response.status();
        let json: serde_json::Value = response.json().unwrap_or(serde_json::json!({}));
        if !status.is_success() {
            return Ok(CapabilityExecutionResult {
                success: false,
                result_status: "failed".into(),
                request_summary: format!("POST draft PR {head}->{base}"),
                response_summary: format!("HTTP {status}"),
                changed_resources: vec![],
                unchanged_resources: vec![self.repository.clone()],
                external_identifiers: vec![],
                warnings: vec![],
                rollback: RollbackMetadata::default(),
                verification_requirements: vec![],
                evidence_refs: vec![],
                error: Some(format!("github API error: {status}")),
                duplicate_suppressed: false,
            });
        }
        let number = json
            .get("number")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "?".into());
        Ok(CapabilityExecutionResult {
            success: true,
            result_status: "success".into(),
            request_summary: format!("POST draft PR {head}->{base}"),
            response_summary: format!("created draft PR #{number}"),
            changed_resources: vec![format!("pull/{number}")],
            unchanged_resources: vec![],
            external_identifiers: vec![number.clone()],
            warnings: vec![],
            rollback: RollbackMetadata {
                available: false,
                capability_id: None,
                inputs: Some(serde_json::json!({"pull_number": number})),
                risks: vec!["close not automated".into()],
                verification: Some("GET pull".into()),
                irreversible_effects: vec![],
            },
            verification_requirements: vec![format!("pr {number} is draft")],
            evidence_refs: vec![format!("github-pr:{number}")],
            error: None,
            duplicate_suppressed: false,
        })
    }

    fn observe_state(
        &self,
        query: &CapabilityStateQuery,
    ) -> RivoraResult<CapabilityStateObservation> {
        let token = match require_token(&self.token) {
            Ok(t) => t,
            Err(e) => {
                return Ok(CapabilityStateObservation {
                    resource_key: self.repository.clone(),
                    fields: HashMap::new(),
                    summary: e.to_string(),
                    observed: false,
                    error: Some(e.to_string()),
                });
            }
        };
        let number = query
            .external_identifiers
            .first()
            .cloned()
            .unwrap_or_default();
        let url = format!(
            "{}/repos/{}/pulls/{}",
            self.api_base.trim_end_matches('/'),
            self.repository,
            number
        );
        let client = client()?;
        let response = client
            .get(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .send()
            .map_err(|e| RivoraError::validation(format!("github get PR failed: {e}")))?;
        let ok = response.status().is_success();
        let json: serde_json::Value = response.json().unwrap_or(serde_json::json!({}));
        let mut fields = HashMap::new();
        if let Some(d) = json.get("draft") {
            fields.insert("draft".into(), d.to_string());
        }
        if let Some(s) = json.get("state").and_then(|v| v.as_str()) {
            fields.insert("state".into(), s.into());
        }
        Ok(CapabilityStateObservation {
            resource_key: format!("pull/{number}"),
            fields,
            summary: if ok {
                format!("pr {number} observed")
            } else {
                format!("pr {number} missing")
            },
            observed: ok,
            error: if ok {
                None
            } else {
                Some("GET pull failed".into())
            },
        })
    }
}

// ---------------------------------------------------------------------------
// github_actions.workflow_dispatch
// ---------------------------------------------------------------------------

/// Dispatch a named GitHub Actions workflow (BoundedWrite).
pub struct GitHubWorkflowDispatchCapability {
    /// owner/repo
    pub repository: String,
    /// Optional token
    pub token: Option<String>,
    /// API base
    pub api_base: String,
}

impl ExecutionCapability for GitHubWorkflowDispatchCapability {
    fn descriptor(&self) -> ExecutionCapabilityDescriptor {
        ExecutionCapabilityDescriptor {
            capability_id: "github_actions.workflow_dispatch".into(),
            version: "1".into(),
            risk_level: CapabilityRiskLevel::BoundedWrite,
            supported_actions: vec!["dispatch_workflow".into()],
            required_inputs: vec!["workflow_id".into(), "ref".into()],
            supports_dry_run: true,
            idempotency_behavior: "dispatch is not naturally idempotent; client key required"
                .into(),
            reversibility: "cancel run if policy allows; not automatic".into(),
            verification_method: "list workflow runs for workflow_id".into(),
            credential_requirements: vec!["GITHUB_TOKEN".into()],
            target_restrictions: vec![self.repository.clone()],
            failure_semantics: "failed dispatch starts no run".into(),
            description: "Trigger an explicitly named GitHub Actions workflow".into(),
        }
    }

    fn dry_run(&self, request: &CapabilityInvocation) -> RivoraResult<DryRunResult> {
        let workflow = input_str(&request.inputs, "workflow_id").unwrap_or_default();
        let git_ref = input_str(&request.inputs, "ref").unwrap_or_default();
        Ok(DryRunResult {
            actions: vec!["dispatch_workflow".into()],
            target: format!("{}/actions/{workflow}", self.repository),
            expected_mutations: vec![format!("dispatch {workflow} @ {git_ref}")],
            required_permissions: vec!["actions:write".into()],
            current_state: None,
            predicted_state: Some("new workflow_run queued".into()),
            risks: vec!["triggers CI compute".into()],
            policy_decision: policy_bounded("github_actions.workflow_dispatch"),
            missing_preconditions: if workflow.is_empty() || git_ref.is_empty() {
                vec!["workflow_id and ref required".into()]
            } else {
                vec![]
            },
            verification_steps: vec!["list recent runs for workflow".into()],
            rollback_options: vec!["cancel run if still in progress".into()],
            simulated: false,
        })
    }

    fn execute(&self, request: &CapabilityInvocation) -> RivoraResult<CapabilityExecutionResult> {
        let token = require_token(&self.token)?;
        let workflow = input_str(&request.inputs, "workflow_id")
            .ok_or_else(|| RivoraError::validation("workflow_id required"))?;
        let git_ref = input_str(&request.inputs, "ref")
            .ok_or_else(|| RivoraError::validation("ref required"))?;
        let url = format!(
            "{}/repos/{}/actions/workflows/{}/dispatches",
            self.api_base.trim_end_matches('/'),
            self.repository,
            workflow
        );
        let client = client()?;
        let response = client
            .post(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .json(&serde_json::json!({ "ref": git_ref }))
            .send()
            .map_err(|e| RivoraError::validation(format!("workflow dispatch failed: {e}")))?;
        let status = response.status();
        // GitHub returns 204 No Content on success.
        let success = status.as_u16() == 204 || status.is_success();
        Ok(CapabilityExecutionResult {
            success,
            result_status: if success {
                "success".into()
            } else {
                "failed".into()
            },
            request_summary: format!("dispatch {workflow} @ {git_ref}"),
            response_summary: format!("HTTP {status}"),
            changed_resources: if success {
                vec![format!("workflow/{workflow}")]
            } else {
                vec![]
            },
            unchanged_resources: if success {
                vec![]
            } else {
                vec![format!("workflow/{workflow}")]
            },
            external_identifiers: vec![format!("workflow:{workflow}:ref:{git_ref}")],
            warnings: vec!["run id not returned by dispatch API".into()],
            rollback: RollbackMetadata {
                available: false,
                capability_id: None,
                inputs: None,
                risks: vec!["cancel not automated".into()],
                verification: Some("list runs".into()),
                irreversible_effects: vec!["compute may have started".into()],
            },
            verification_requirements: vec![format!("recent run for {workflow}")],
            evidence_refs: vec![format!("github-workflow-dispatch:{workflow}")],
            error: if success {
                None
            } else {
                Some(format!("github API error: {status}"))
            },
            duplicate_suppressed: false,
        })
    }

    fn observe_state(
        &self,
        query: &CapabilityStateQuery,
    ) -> RivoraResult<CapabilityStateObservation> {
        let token = match require_token(&self.token) {
            Ok(t) => t,
            Err(e) => {
                return Ok(CapabilityStateObservation {
                    resource_key: self.repository.clone(),
                    fields: HashMap::new(),
                    summary: e.to_string(),
                    observed: false,
                    error: Some(e.to_string()),
                });
            }
        };
        let workflow = input_str(&query.inputs, "workflow_id").unwrap_or_default();
        let url = format!(
            "{}/repos/{}/actions/workflows/{}/runs?per_page=1",
            self.api_base.trim_end_matches('/'),
            self.repository,
            workflow
        );
        let client = client()?;
        let response = client
            .get(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .send()
            .map_err(|e| RivoraError::validation(format!("list runs failed: {e}")))?;
        let ok = response.status().is_success();
        let text = response.text().unwrap_or_default();
        let mut fields = HashMap::new();
        fields.insert("payload_len".into(), text.len().to_string());
        Ok(CapabilityStateObservation {
            resource_key: format!("workflow/{workflow}/runs"),
            fields,
            summary: if ok {
                "runs listed".into()
            } else {
                "list runs failed".into()
            },
            observed: ok,
            error: if ok {
                None
            } else {
                Some("GET runs failed".into())
            },
        })
    }
}

fn urlencoding_lite(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}
