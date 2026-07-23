//! Bounded external execution adapters (RFC-026).
//!
//! These are distinct from observation connectors. They implement
//! `rivora::ExecutionCapability` and are invoked only by the Runtime.
//! Observation modules remain read-only.

use std::collections::HashMap;
use std::io::Read;
use std::time::Duration;

use rivora::{
    CapabilityExecutionResult, CapabilityExecutionStatus, CapabilityInvocation,
    CapabilityRiskLevel, CapabilityStateObservation, CapabilityStateQuery, CapabilityTarget,
    CapabilityVerificationStatus, DryRunResult, ExecutionCapability, ExecutionCapabilityDescriptor,
    ExecutionPolicyDecision, ExecutionPolicyDecisionKind, RivoraError, RivoraResult,
    RollbackMetadata,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_BODY_BYTES: usize = 65_536;
const MAX_TITLE_BYTES: usize = 256;
const MAX_LABEL_BYTES: usize = 128;
const MAX_REF_BYTES: usize = 256;
const MAX_WORKFLOW_BYTES: usize = 256;

/// Register the standard v0.6 bounded GitHub execution adapters when a token is present.
///
/// Without a token, adapters are still registered but live execute fails preconditions
/// with a clear credential error (dry-run/plan validation still works).
pub fn register_github_execution_capabilities(
    registry: &rivora::ExecutionCapabilityRegistry,
    repository: impl Into<String>,
    token: Option<String>,
) -> RivoraResult<()> {
    let repository = repository.into();
    validate_repository(&repository)?;
    let api_base = "https://api.github.com".to_string();
    registry.register(std::sync::Arc::new(GitHubIssueCommentCapability {
        repository: repository.clone(),
        token: token.clone(),
        api_base: api_base.clone(),
    }))?;
    registry.register(std::sync::Arc::new(GitHubIssueLabelCapability {
        repository: repository.clone(),
        token: token.clone(),
        api_base: api_base.clone(),
    }))?;
    registry.register(std::sync::Arc::new(GitHubIssueCreateCapability {
        repository: repository.clone(),
        token: token.clone(),
        api_base: api_base.clone(),
    }))?;
    registry.register(std::sync::Arc::new(GitHubDraftPrCapability {
        repository: repository.clone(),
        token: token.clone(),
        api_base: api_base.clone(),
    }))?;
    registry.register(std::sync::Arc::new(GitHubWorkflowDispatchCapability {
        repository,
        token,
        api_base,
    }))?;
    Ok(())
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

fn validate_repository(repository: &str) -> RivoraResult<()> {
    let mut segments = repository.split('/');
    let owner = segments.next().unwrap_or_default();
    let repo = segments.next().unwrap_or_default();
    if owner.is_empty()
        || repo.is_empty()
        || segments.next().is_some()
        || !valid_github_name(owner)
        || !valid_github_name(repo)
    {
        return Err(RivoraError::validation(
            "GitHub repository must be an exact `owner/repository` target",
        ));
    }
    Ok(())
}

fn valid_github_name(value: &str) -> bool {
    value.len() <= 100
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        && value != "."
        && value != ".."
}

fn validate_invocation(
    request: &CapabilityInvocation,
    capability_id: &str,
    actions: &[&str],
) -> RivoraResult<()> {
    if request.capability_id != capability_id {
        return Err(RivoraError::validation(format!(
            "capability `{}` cannot execute request for `{}`",
            capability_id, request.capability_id
        )));
    }
    if !actions.contains(&request.action_name.as_str()) {
        return Err(RivoraError::validation(format!(
            "unsupported action `{}` for capability `{capability_id}`",
            request.action_name
        )));
    }
    if !request.inputs.is_object() {
        return Err(RivoraError::validation(
            "capability inputs must be an object",
        ));
    }
    validate_environment(&request.environment)
}

fn validate_query(
    query: &CapabilityStateQuery,
    capability_id: &str,
    actions: &[&str],
) -> RivoraResult<()> {
    if query.capability_id != capability_id {
        return Err(RivoraError::validation(format!(
            "capability `{}` cannot observe request for `{}`",
            capability_id, query.capability_id
        )));
    }
    if !actions.contains(&query.action_name.as_str()) {
        return Err(RivoraError::validation(format!(
            "unsupported action `{}` for capability `{capability_id}`",
            query.action_name
        )));
    }
    if !query.inputs.is_object() {
        return Err(RivoraError::validation(
            "capability inputs must be an object",
        ));
    }
    validate_environment(&query.environment)
}

fn validate_environment(environment: &str) -> RivoraResult<()> {
    let environment = environment.trim();
    if environment.is_empty()
        || environment.len() > 64
        || !environment
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err(RivoraError::validation(
            "environment must be a non-empty bounded identifier",
        ));
    }
    Ok(())
}

fn required_input(inputs: &serde_json::Value, key: &str, max_bytes: usize) -> RivoraResult<String> {
    let value = inputs
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| RivoraError::validation(format!("{key} required")))?;
    if value.len() > max_bytes || value.chars().any(char::is_control) {
        return Err(RivoraError::validation(format!(
            "{key} exceeds safe input bounds"
        )));
    }
    Ok(value.to_string())
}

fn optional_input(inputs: &serde_json::Value, key: &str, max_bytes: usize) -> RivoraResult<String> {
    match inputs.get(key) {
        None | Some(serde_json::Value::Null) => Ok(String::new()),
        Some(value) => {
            let value = value
                .as_str()
                .ok_or_else(|| RivoraError::validation(format!("{key} must be a string")))?;
            if value.len() > max_bytes {
                return Err(RivoraError::validation(format!(
                    "{key} exceeds safe input bounds"
                )));
            }
            Ok(value.to_string())
        }
    }
}

fn issue_number(inputs: &serde_json::Value) -> RivoraResult<String> {
    let raw = required_input(inputs, "issue_number", 20)?;
    let parsed = raw
        .parse::<u64>()
        .map_err(|_| RivoraError::validation("issue_number must be a positive integer"))?;
    if parsed == 0 {
        return Err(RivoraError::validation(
            "issue_number must be a positive integer",
        ));
    }
    Ok(parsed.to_string())
}

fn external_number(query: &CapabilityStateQuery, kind: &str) -> RivoraResult<String> {
    let raw = query
        .external_identifiers
        .first()
        .map(String::as_str)
        .ok_or_else(|| RivoraError::validation(format!("missing external {kind} identifier")))?;
    let parsed = raw.parse::<u64>().map_err(|_| {
        RivoraError::validation(format!("external {kind} identifier must be numeric"))
    })?;
    if parsed == 0 {
        return Err(RivoraError::validation(format!(
            "external {kind} identifier must be positive"
        )));
    }
    Ok(parsed.to_string())
}

fn client() -> RivoraResult<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .user_agent("rivora-execution/0.6")
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| RivoraError::validation(format!("http client: {e}")))
}

fn read_json(mut response: reqwest::blocking::Response) -> Result<serde_json::Value, String> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err("GitHub response exceeded safe size limit".into());
    }
    let mut body = Vec::new();
    response
        .by_ref()
        .take((MAX_RESPONSE_BYTES + 1) as u64)
        .read_to_end(&mut body)
        .map_err(|e| format!("failed reading GitHub response: {e}"))?;
    if body.len() > MAX_RESPONSE_BYTES {
        return Err("GitHub response exceeded safe size limit".into());
    }
    serde_json::from_slice(&body).map_err(|e| format!("invalid GitHub JSON response: {e}"))
}

fn current_label_state(
    client: &reqwest::blocking::Client,
    api_base: &str,
    repository: &str,
    issue: &str,
    label: &str,
    token: &str,
) -> RivoraResult<bool> {
    let url = format!(
        "{}/repos/{repository}/issues/{issue}/labels",
        api_base.trim_end_matches('/')
    );
    let response = client
        .get(url)
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|error| {
            RivoraError::precondition(format!(
                "cannot establish initial label state before mutation: {error}"
            ))
        })?;
    if !response.status().is_success() {
        return Err(RivoraError::precondition(format!(
            "cannot establish initial label state before mutation: HTTP {}",
            response.status()
        )));
    }
    let json = read_json(response).map_err(RivoraError::precondition)?;
    let labels = json.as_array().ok_or_else(|| {
        RivoraError::precondition("cannot establish initial label state: expected label list")
    })?;
    Ok(labels
        .iter()
        .any(|item| item.get("name").and_then(serde_json::Value::as_str) == Some(label)))
}

fn uncertain_result(
    request_summary: String,
    unchanged_resources: Vec<String>,
    error: impl Into<String>,
) -> CapabilityExecutionResult {
    let error = error.into();
    CapabilityExecutionResult {
        status: CapabilityExecutionStatus::Uncertain,
        request_summary,
        response_summary: "external mutation outcome is unknown".into(),
        changed_resources: vec![],
        unchanged_resources,
        external_identifiers: vec![],
        warnings: vec!["do not retry until independent verification completes".into()],
        rollback: RollbackMetadata::default(),
        verification_requirements: vec!["independently determine whether mutation occurred".into()],
        evidence_refs: vec![],
        error: Some(error),
        duplicate_suppressed: false,
    }
}

fn failed_result(
    request_summary: String,
    unchanged_resources: Vec<String>,
    error: impl Into<String>,
) -> CapabilityExecutionResult {
    let error = error.into();
    CapabilityExecutionResult {
        status: CapabilityExecutionStatus::Failed,
        request_summary,
        response_summary: "external mutation was not attempted".into(),
        changed_resources: vec![],
        unchanged_resources,
        external_identifiers: vec![],
        warnings: vec![],
        rollback: RollbackMetadata::default(),
        verification_requirements: vec![],
        evidence_refs: vec![],
        error: Some(error),
        duplicate_suppressed: false,
    }
}

fn failed_observation(
    resource_key: String,
    error: impl Into<String>,
) -> CapabilityStateObservation {
    let error = error.into();
    CapabilityStateObservation {
        resource_key,
        fields: HashMap::new(),
        summary: error.clone(),
        observed: false,
        verification_status: CapabilityVerificationStatus::Inconclusive,
        error: Some(error),
    }
}

fn ambiguous_http_status(status: reqwest::StatusCode) -> bool {
    matches!(status.as_u16(), 408 | 500 | 502 | 503 | 504)
}

fn github_target(
    repository: &str,
    branch_or_ref: Option<String>,
) -> RivoraResult<CapabilityTarget> {
    validate_repository(repository)?;
    let (owner, repository) = repository
        .split_once('/')
        .ok_or_else(|| RivoraError::validation("invalid GitHub repository target"))?;
    Ok(CapabilityTarget {
        provider: "github".into(),
        owner: Some(owner.to_string()),
        repository: Some(repository.to_string()),
        branch_or_ref,
    })
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

    fn target(
        &self,
        environment: &str,
        _inputs: &serde_json::Value,
    ) -> RivoraResult<CapabilityTarget> {
        validate_environment(environment)?;
        github_target(&self.repository, None)
    }

    fn validate_preconditions(&self, request: &CapabilityInvocation) -> RivoraResult<()> {
        validate_repository(&self.repository)?;
        validate_invocation(request, "github.issue.comment", &["create_comment"])?;
        issue_number(&request.inputs)?;
        required_input(&request.inputs, "body", MAX_BODY_BYTES)?;
        require_token(&self.token)?;
        Ok(())
    }

    fn dry_run(&self, request: &CapabilityInvocation) -> RivoraResult<DryRunResult> {
        validate_repository(&self.repository)?;
        validate_invocation(request, "github.issue.comment", &["create_comment"])?;
        let issue = issue_number(&request.inputs)?;
        let body = required_input(&request.inputs, "body", MAX_BODY_BYTES)?;
        Ok(DryRunResult {
            actions: vec!["create_comment".into()],
            target: format!("{}/issues/{issue}", self.repository),
            expected_mutations: vec![format!("create comment on issue {issue}")],
            required_permissions: vec!["issues:write".into()],
            current_state: None,
            predicted_state: Some(format!("comment body length {}", body.len())),
            risks: vec!["public comment visible to repository collaborators".into()],
            policy_decision: policy_low_risk("github.issue.comment"),
            missing_preconditions: if self.token.as_deref().map_or(true, str::is_empty) {
                vec!["GitHub credential unavailable for live execution".into()]
            } else {
                vec![]
            },
            verification_steps: vec!["GET exact comment id and compare exact body".into()],
            rollback_options: vec!["manual comment deletion".into()],
            simulated: false,
        })
    }

    fn execute(&self, request: &CapabilityInvocation) -> RivoraResult<CapabilityExecutionResult> {
        validate_repository(&self.repository)?;
        validate_invocation(request, "github.issue.comment", &["create_comment"])?;
        let token = require_token(&self.token)?;
        let issue = issue_number(&request.inputs)?;
        let body = required_input(&request.inputs, "body", MAX_BODY_BYTES)?;
        let url = format!(
            "{}/repos/{}/issues/{}/comments",
            self.api_base.trim_end_matches('/'),
            self.repository,
            issue
        );
        let client = client()?;
        let response = match client
            .post(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .json(&serde_json::json!({ "body": body }))
            .send()
        {
            Ok(response) => response,
            Err(error) => {
                return Ok(uncertain_result(
                    format!("POST comment issue {issue}"),
                    vec![format!("issue/{issue}")],
                    format!("github comment request outcome unknown: {error}"),
                ));
            }
        };
        let status = response.status();
        if ambiguous_http_status(status) {
            return Ok(uncertain_result(
                format!("POST comment issue {issue}"),
                vec![format!("issue/{issue}")],
                format!("GitHub returned ambiguous HTTP status {status}"),
            ));
        }
        if !status.is_success() {
            return Ok(CapabilityExecutionResult {
                status: CapabilityExecutionStatus::Failed,
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
        let json = match read_json(response) {
            Ok(json) => json,
            Err(error) => {
                return Ok(uncertain_result(
                    format!("POST comment issue {issue}"),
                    vec![format!("issue/{issue}")],
                    error,
                ));
            }
        };
        let id = json
            .get("id")
            .and_then(serde_json::Value::as_u64)
            .filter(|id| *id > 0)
            .map(|id| id.to_string())
            .ok_or_else(|| RivoraError::validation("GitHub comment response missing numeric id"));
        let id = match id {
            Ok(id) => id,
            Err(error) => {
                return Ok(uncertain_result(
                    format!("POST comment issue {issue}"),
                    vec![format!("issue/{issue}")],
                    error.to_string(),
                ));
            }
        };
        Ok(CapabilityExecutionResult {
            status: CapabilityExecutionStatus::Success,
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
                inverse_action_name: None,
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
        validate_repository(&self.repository)?;
        validate_query(query, "github.issue.comment", &["create_comment"])?;
        let token = match require_token(&self.token) {
            Ok(t) => t,
            Err(e) => {
                return Ok(CapabilityStateObservation {
                    resource_key: self.repository.clone(),
                    fields: HashMap::new(),
                    summary: e.to_string(),
                    observed: false,
                    verification_status: CapabilityVerificationStatus::Inconclusive,
                    error: Some(e.to_string()),
                });
            }
        };
        let issue = issue_number(&query.inputs)?;
        let expected_body = required_input(&query.inputs, "body", MAX_BODY_BYTES)?;
        let comment_id = external_number(query, "comment")?;
        let url = format!(
            "{}/repos/{}/issues/comments/{}",
            self.api_base.trim_end_matches('/'),
            self.repository,
            comment_id
        );
        let client = client()?;
        let response = match client
            .get(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .send()
        {
            Ok(response) => response,
            Err(error) => {
                return Ok(failed_observation(
                    format!("{}/issue/{issue}/comment/{comment_id}", self.repository),
                    format!("github comment observation failed: {error}"),
                ));
            }
        };
        let ok = response.status().is_success();
        if !ok {
            return Ok(failed_observation(
                format!("{}/issue/{issue}/comment/{comment_id}", self.repository),
                format!("GET exact comment returned {}", response.status()),
            ));
        }
        let json = match read_json(response) {
            Ok(json) => json,
            Err(error) => {
                return Ok(failed_observation(
                    format!("{}/issue/{issue}/comment/{comment_id}", self.repository),
                    error,
                ));
            }
        };
        let observed_body = json
            .get("body")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let exact_match = comment_matches(&json, &comment_id, &issue, &expected_body);
        let mut fields = HashMap::new();
        fields.insert("comment_id".into(), comment_id.clone());
        fields.insert("body".into(), observed_body.to_string());
        fields.insert("content_matches".into(), exact_match.to_string());
        fields.insert("verified".into(), exact_match.to_string());
        Ok(CapabilityStateObservation {
            resource_key: format!("{}/issue/{issue}/comment/{comment_id}", self.repository),
            fields,
            summary: if exact_match {
                format!("exact comment {comment_id} content observed")
            } else {
                format!("comment {comment_id} did not match the approved effect")
            },
            observed: exact_match,
            verification_status: if exact_match {
                CapabilityVerificationStatus::Passed
            } else {
                CapabilityVerificationStatus::Failed
            },
            error: if exact_match {
                None
            } else {
                Some("exact comment identifier/content mismatch".into())
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

    fn target(
        &self,
        environment: &str,
        _inputs: &serde_json::Value,
    ) -> RivoraResult<CapabilityTarget> {
        validate_environment(environment)?;
        github_target(&self.repository, None)
    }

    fn validate_preconditions(&self, request: &CapabilityInvocation) -> RivoraResult<()> {
        validate_repository(&self.repository)?;
        validate_invocation(
            request,
            "github.issue.label",
            &["add_label", "remove_label"],
        )?;
        issue_number(&request.inputs)?;
        required_input(&request.inputs, "label", MAX_LABEL_BYTES)?;
        require_token(&self.token)?;
        Ok(())
    }

    fn dry_run(&self, request: &CapabilityInvocation) -> RivoraResult<DryRunResult> {
        validate_repository(&self.repository)?;
        validate_invocation(
            request,
            "github.issue.label",
            &["add_label", "remove_label"],
        )?;
        let issue = issue_number(&request.inputs)?;
        let label = required_input(&request.inputs, "label", MAX_LABEL_BYTES)?;
        Ok(DryRunResult {
            actions: vec![request.action_name.clone()],
            target: format!("{}/issues/{issue}", self.repository),
            expected_mutations: vec![format!("{} label {label}", request.action_name)],
            required_permissions: vec!["issues:write".into()],
            current_state: None,
            predicted_state: Some(format!("label {label} {}", request.action_name)),
            risks: vec!["label changes visible to collaborators".into()],
            policy_decision: policy_low_risk("github.issue.label"),
            missing_preconditions: if self.token.as_deref().map_or(true, str::is_empty) {
                vec!["GitHub credential unavailable for live execution".into()]
            } else {
                vec![]
            },
            verification_steps: vec!["GET issue labels and compare exact final state".into()],
            rollback_options: vec!["inverse add/remove label".into()],
            simulated: false,
        })
    }

    fn execute(&self, request: &CapabilityInvocation) -> RivoraResult<CapabilityExecutionResult> {
        validate_repository(&self.repository)?;
        validate_invocation(
            request,
            "github.issue.label",
            &["add_label", "remove_label"],
        )?;
        let token = require_token(&self.token)?;
        let issue = issue_number(&request.inputs)?;
        let label = required_input(&request.inputs, "label", MAX_LABEL_BYTES)?;
        let client = client()?;
        let initially_present = current_label_state(
            &client,
            &self.api_base,
            &self.repository,
            &issue,
            &label,
            token,
        )?;
        let desired_present = request.action_name == "add_label";
        if initially_present == desired_present {
            return Ok(CapabilityExecutionResult {
                status: CapabilityExecutionStatus::DuplicateSuppressed,
                request_summary: format!("{} label {label} on issue {issue}", request.action_name),
                response_summary: "requested final label state already existed".into(),
                changed_resources: vec![],
                unchanged_resources: vec![format!("issue/{issue}/label/{label}")],
                external_identifiers: vec![format!("issue:{issue}:label:{label}")],
                warnings: vec![],
                rollback: RollbackMetadata::default(),
                verification_requirements: vec![format!(
                    "label {label} final state matches {}",
                    request.action_name
                )],
                evidence_refs: vec![format!("github-label:{issue}:{label}")],
                error: None,
                duplicate_suppressed: true,
            });
        }
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
        let response = match builder
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .send()
        {
            Ok(response) => response,
            Err(error) => {
                return Ok(uncertain_result(
                    format!("{method} label {label} on issue {issue}"),
                    vec![format!("issue/{issue}")],
                    format!("github label request outcome unknown: {error}"),
                ));
            }
        };
        let status = response.status();
        if ambiguous_http_status(status) {
            return Ok(uncertain_result(
                format!("{method} label {label} on issue {issue}"),
                vec![format!("issue/{issue}")],
                format!("GitHub returned ambiguous HTTP status {status}"),
            ));
        }
        let success = status.is_success();
        Ok(CapabilityExecutionResult {
            status: if success {
                CapabilityExecutionStatus::Success
            } else {
                CapabilityExecutionStatus::Failed
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
            rollback: label_rollback(&request.action_name, &issue, &label),
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
        validate_repository(&self.repository)?;
        validate_query(query, "github.issue.label", &["add_label", "remove_label"])?;
        let token = match require_token(&self.token) {
            Ok(t) => t,
            Err(e) => {
                return Ok(CapabilityStateObservation {
                    resource_key: self.repository.clone(),
                    fields: HashMap::new(),
                    summary: e.to_string(),
                    observed: false,
                    verification_status: CapabilityVerificationStatus::Inconclusive,
                    error: Some(e.to_string()),
                });
            }
        };
        let issue = issue_number(&query.inputs)?;
        let label = required_input(&query.inputs, "label", MAX_LABEL_BYTES)?;
        let url = format!(
            "{}/repos/{}/issues/{}/labels",
            self.api_base.trim_end_matches('/'),
            self.repository,
            issue
        );
        let client = client()?;
        let response = match client
            .get(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .send()
        {
            Ok(response) => response,
            Err(error) => {
                return Ok(failed_observation(
                    format!("{}/issue/{issue}/labels", self.repository),
                    format!("github label observation failed: {error}"),
                ));
            }
        };
        let ok = response.status().is_success();
        if !ok {
            return Ok(failed_observation(
                format!("{}/issue/{issue}/labels", self.repository),
                format!("GET issue labels returned {}", response.status()),
            ));
        }
        let json = match read_json(response) {
            Ok(json) => json,
            Err(error) => {
                return Ok(failed_observation(
                    format!("{}/issue/{issue}/labels", self.repository),
                    error,
                ));
            }
        };
        let has_label = labels_contain(&json, &label);
        let final_state_matches = match query.action_name.as_str() {
            "add_label" => has_label,
            "remove_label" => !has_label,
            _ => false,
        };
        let mut fields = HashMap::new();
        fields.insert("has_label".into(), has_label.to_string());
        fields.insert(
            "final_state_matches".into(),
            final_state_matches.to_string(),
        );
        fields.insert("verified".into(), final_state_matches.to_string());
        Ok(CapabilityStateObservation {
            resource_key: format!("{}/issue/{issue}/label/{label}", self.repository),
            fields,
            summary: if final_state_matches {
                format!("label {label} final state matches {}", query.action_name)
            } else {
                format!(
                    "label {label} final state contradicts {}",
                    query.action_name
                )
            },
            observed: final_state_matches,
            verification_status: if final_state_matches {
                CapabilityVerificationStatus::Passed
            } else {
                CapabilityVerificationStatus::Failed
            },
            error: if final_state_matches {
                None
            } else {
                Some("label final state mismatch".into())
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

    fn target(
        &self,
        environment: &str,
        _inputs: &serde_json::Value,
    ) -> RivoraResult<CapabilityTarget> {
        validate_environment(environment)?;
        github_target(&self.repository, None)
    }

    fn validate_preconditions(&self, request: &CapabilityInvocation) -> RivoraResult<()> {
        validate_repository(&self.repository)?;
        validate_invocation(request, "github.issue.create", &["create_issue"])?;
        required_input(&request.inputs, "title", MAX_TITLE_BYTES)?;
        optional_input(&request.inputs, "body", MAX_BODY_BYTES)?;
        require_token(&self.token)?;
        Ok(())
    }

    fn dry_run(&self, request: &CapabilityInvocation) -> RivoraResult<DryRunResult> {
        validate_repository(&self.repository)?;
        validate_invocation(request, "github.issue.create", &["create_issue"])?;
        let title = required_input(&request.inputs, "title", MAX_TITLE_BYTES)?;
        optional_input(&request.inputs, "body", MAX_BODY_BYTES)?;
        Ok(DryRunResult {
            actions: vec!["create_issue".into()],
            target: self.repository.clone(),
            expected_mutations: vec![format!("create issue titled {title}")],
            required_permissions: vec!["issues:write".into()],
            current_state: None,
            predicted_state: Some("new open issue".into()),
            risks: vec!["creates durable tracker item".into()],
            policy_decision: policy_bounded("github.issue.create"),
            missing_preconditions: if self.token.as_deref().map_or(true, str::is_empty) {
                vec!["GitHub credential unavailable for live execution".into()]
            } else {
                vec![]
            },
            verification_steps: vec!["GET exact issue id and compare title/body/type".into()],
            rollback_options: vec!["close issue manually".into()],
            simulated: false,
        })
    }

    fn execute(&self, request: &CapabilityInvocation) -> RivoraResult<CapabilityExecutionResult> {
        validate_repository(&self.repository)?;
        validate_invocation(request, "github.issue.create", &["create_issue"])?;
        let token = require_token(&self.token)?;
        let title = required_input(&request.inputs, "title", MAX_TITLE_BYTES)?;
        let body = optional_input(&request.inputs, "body", MAX_BODY_BYTES)?;
        let url = format!(
            "{}/repos/{}/issues",
            self.api_base.trim_end_matches('/'),
            self.repository
        );
        let client = client()?;
        let response = match client
            .post(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .json(&serde_json::json!({ "title": title, "body": body }))
            .send()
        {
            Ok(response) => response,
            Err(error) => {
                return Ok(uncertain_result(
                    format!("POST issue {title}"),
                    vec![self.repository.clone()],
                    format!("github create issue outcome unknown: {error}"),
                ));
            }
        };
        let status = response.status();
        if ambiguous_http_status(status) {
            return Ok(uncertain_result(
                format!("POST issue {title}"),
                vec![self.repository.clone()],
                format!("GitHub returned ambiguous HTTP status {status}"),
            ));
        }
        if !status.is_success() {
            return Ok(CapabilityExecutionResult {
                status: CapabilityExecutionStatus::Failed,
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
        let json = match read_json(response) {
            Ok(json) => json,
            Err(error) => {
                return Ok(uncertain_result(
                    format!("POST issue {title}"),
                    vec![self.repository.clone()],
                    error,
                ));
            }
        };
        let number = json
            .get("number")
            .and_then(serde_json::Value::as_u64)
            .filter(|number| *number > 0)
            .map(|number| number.to_string());
        let Some(number) = number else {
            return Ok(uncertain_result(
                format!("POST issue {title}"),
                vec![self.repository.clone()],
                "GitHub issue response missing numeric issue number",
            ));
        };
        Ok(CapabilityExecutionResult {
            status: CapabilityExecutionStatus::Success,
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
                inverse_action_name: None,
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
        validate_repository(&self.repository)?;
        validate_query(query, "github.issue.create", &["create_issue"])?;
        let token = match require_token(&self.token) {
            Ok(t) => t,
            Err(e) => {
                return Ok(CapabilityStateObservation {
                    resource_key: self.repository.clone(),
                    fields: HashMap::new(),
                    summary: e.to_string(),
                    observed: false,
                    verification_status: CapabilityVerificationStatus::Inconclusive,
                    error: Some(e.to_string()),
                });
            }
        };
        let number = external_number(query, "issue")?;
        let expected_title = required_input(&query.inputs, "title", MAX_TITLE_BYTES)?;
        let expected_body = optional_input(&query.inputs, "body", MAX_BODY_BYTES)?;
        let url = format!(
            "{}/repos/{}/issues/{}",
            self.api_base.trim_end_matches('/'),
            self.repository,
            number
        );
        let client = client()?;
        let response = match client
            .get(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .send()
        {
            Ok(response) => response,
            Err(error) => {
                return Ok(failed_observation(
                    format!("{}/issue/{number}", self.repository),
                    format!("github issue observation failed: {error}"),
                ));
            }
        };
        let ok = response.status().is_success();
        if !ok {
            return Ok(failed_observation(
                format!("{}/issue/{number}", self.repository),
                format!("GET exact issue returned {}", response.status()),
            ));
        }
        let json = match read_json(response) {
            Ok(json) => json,
            Err(error) => {
                return Ok(failed_observation(
                    format!("{}/issue/{number}", self.repository),
                    error,
                ));
            }
        };
        let number_matches =
            json.get("number").and_then(serde_json::Value::as_u64) == number.parse::<u64>().ok();
        let title_matches =
            json.get("title").and_then(serde_json::Value::as_str) == Some(expected_title.as_str());
        let body_matches = json
            .get("body")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            == expected_body;
        let is_issue = json.get("pull_request").is_none();
        let exact_match = issue_matches(&json, &number, &expected_title, &expected_body);
        let mut fields = HashMap::new();
        if let Some(t) = json.get("title").and_then(|v| v.as_str()) {
            fields.insert("title".into(), t.into());
        }
        if let Some(s) = json.get("state").and_then(|v| v.as_str()) {
            fields.insert("state".into(), s.into());
        }
        fields.insert("number_matches".into(), number_matches.to_string());
        fields.insert("title_matches".into(), title_matches.to_string());
        fields.insert("body_matches".into(), body_matches.to_string());
        fields.insert("is_issue".into(), is_issue.to_string());
        fields.insert("verified".into(), exact_match.to_string());
        Ok(CapabilityStateObservation {
            resource_key: format!("{}/issue/{number}", self.repository),
            fields,
            summary: if exact_match {
                format!("exact issue {number} observed")
            } else {
                format!("issue {number} does not match the approved creation")
            },
            observed: exact_match,
            verification_status: if exact_match {
                CapabilityVerificationStatus::Passed
            } else {
                CapabilityVerificationStatus::Failed
            },
            error: if exact_match {
                None
            } else {
                Some("exact issue verification mismatch".into())
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

    fn target(
        &self,
        environment: &str,
        inputs: &serde_json::Value,
    ) -> RivoraResult<CapabilityTarget> {
        validate_environment(environment)?;
        let head = required_input(inputs, "head", MAX_REF_BYTES)?;
        let base = required_input(inputs, "base", MAX_REF_BYTES)?;
        validate_git_ref(&head, "head")?;
        validate_git_ref(&base, "base")?;
        github_target(&self.repository, Some(format!("{head}->{base}")))
    }

    fn validate_preconditions(&self, request: &CapabilityInvocation) -> RivoraResult<()> {
        validate_repository(&self.repository)?;
        validate_invocation(
            request,
            "github.pull_request.create_draft",
            &["create_draft_pr"],
        )?;
        required_input(&request.inputs, "title", MAX_TITLE_BYTES)?;
        optional_input(&request.inputs, "body", MAX_BODY_BYTES)?;
        let head = required_input(&request.inputs, "head", MAX_REF_BYTES)?;
        let base = required_input(&request.inputs, "base", MAX_REF_BYTES)?;
        validate_git_ref(&head, "head")?;
        validate_git_ref(&base, "base")?;
        require_token(&self.token)?;
        Ok(())
    }

    fn dry_run(&self, request: &CapabilityInvocation) -> RivoraResult<DryRunResult> {
        validate_repository(&self.repository)?;
        validate_invocation(
            request,
            "github.pull_request.create_draft",
            &["create_draft_pr"],
        )?;
        required_input(&request.inputs, "title", MAX_TITLE_BYTES)?;
        optional_input(&request.inputs, "body", MAX_BODY_BYTES)?;
        let head = required_input(&request.inputs, "head", MAX_REF_BYTES)?;
        let base = required_input(&request.inputs, "base", MAX_REF_BYTES)?;
        validate_git_ref(&head, "head")?;
        validate_git_ref(&base, "base")?;
        Ok(DryRunResult {
            actions: vec!["create_draft_pr".into()],
            target: self.repository.clone(),
            expected_mutations: vec![format!("draft PR {head} -> {base}")],
            required_permissions: vec!["pull_requests:write".into()],
            current_state: None,
            predicted_state: Some("draft PR open".into()),
            risks: vec!["creates reviewable change proposal".into()],
            policy_decision: policy_bounded("github.pull_request.create_draft"),
            missing_preconditions: if self.token.as_deref().map_or(true, str::is_empty) {
                vec!["GitHub credential unavailable for live execution".into()]
            } else {
                vec![]
            },
            verification_steps: vec![
                "GET exact PR id and compare draft=true, title, head, and base".into(),
            ],
            rollback_options: vec!["close PR".into()],
            simulated: false,
        })
    }

    fn execute(&self, request: &CapabilityInvocation) -> RivoraResult<CapabilityExecutionResult> {
        validate_repository(&self.repository)?;
        validate_invocation(
            request,
            "github.pull_request.create_draft",
            &["create_draft_pr"],
        )?;
        let token = require_token(&self.token)?;
        let title = required_input(&request.inputs, "title", MAX_TITLE_BYTES)?;
        let head = required_input(&request.inputs, "head", MAX_REF_BYTES)?;
        let base = required_input(&request.inputs, "base", MAX_REF_BYTES)?;
        validate_git_ref(&head, "head")?;
        validate_git_ref(&base, "base")?;
        let body = optional_input(&request.inputs, "body", MAX_BODY_BYTES)?;
        let url = format!(
            "{}/repos/{}/pulls",
            self.api_base.trim_end_matches('/'),
            self.repository
        );
        let client = client()?;
        let response = match client
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
        {
            Ok(response) => response,
            Err(error) => {
                return Ok(uncertain_result(
                    format!("POST draft PR {head}->{base}"),
                    vec![self.repository.clone()],
                    format!("github create PR outcome unknown: {error}"),
                ));
            }
        };
        let status = response.status();
        if ambiguous_http_status(status) {
            return Ok(uncertain_result(
                format!("POST draft PR {head}->{base}"),
                vec![self.repository.clone()],
                format!("GitHub returned ambiguous HTTP status {status}"),
            ));
        }
        if !status.is_success() {
            return Ok(CapabilityExecutionResult {
                status: CapabilityExecutionStatus::Failed,
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
        let json = match read_json(response) {
            Ok(json) => json,
            Err(error) => {
                return Ok(uncertain_result(
                    format!("POST draft PR {head}->{base}"),
                    vec![self.repository.clone()],
                    error,
                ));
            }
        };
        let number = json
            .get("number")
            .and_then(serde_json::Value::as_u64)
            .filter(|number| *number > 0)
            .map(|number| number.to_string());
        let Some(number) = number else {
            return Ok(uncertain_result(
                format!("POST draft PR {head}->{base}"),
                vec![self.repository.clone()],
                "GitHub pull request response missing numeric PR number",
            ));
        };
        Ok(CapabilityExecutionResult {
            status: CapabilityExecutionStatus::Success,
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
                inverse_action_name: None,
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
        validate_repository(&self.repository)?;
        validate_query(
            query,
            "github.pull_request.create_draft",
            &["create_draft_pr"],
        )?;
        let token = match require_token(&self.token) {
            Ok(t) => t,
            Err(e) => {
                return Ok(CapabilityStateObservation {
                    resource_key: self.repository.clone(),
                    fields: HashMap::new(),
                    summary: e.to_string(),
                    observed: false,
                    verification_status: CapabilityVerificationStatus::Inconclusive,
                    error: Some(e.to_string()),
                });
            }
        };
        let number = external_number(query, "pull request")?;
        let expected_title = required_input(&query.inputs, "title", MAX_TITLE_BYTES)?;
        let expected_head = required_input(&query.inputs, "head", MAX_REF_BYTES)?;
        let expected_base = required_input(&query.inputs, "base", MAX_REF_BYTES)?;
        validate_git_ref(&expected_head, "head")?;
        validate_git_ref(&expected_base, "base")?;
        let url = format!(
            "{}/repos/{}/pulls/{}",
            self.api_base.trim_end_matches('/'),
            self.repository,
            number
        );
        let client = client()?;
        let response = match client
            .get(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .send()
        {
            Ok(response) => response,
            Err(error) => {
                return Ok(failed_observation(
                    format!("{}/pull/{number}", self.repository),
                    format!("github pull request observation failed: {error}"),
                ));
            }
        };
        let ok = response.status().is_success();
        if !ok {
            return Ok(failed_observation(
                format!("{}/pull/{number}", self.repository),
                format!("GET exact pull request returned {}", response.status()),
            ));
        }
        let json = match read_json(response) {
            Ok(json) => json,
            Err(error) => {
                return Ok(failed_observation(
                    format!("{}/pull/{number}", self.repository),
                    error,
                ));
            }
        };
        let number_matches =
            json.get("number").and_then(serde_json::Value::as_u64) == number.parse::<u64>().ok();
        let title_matches =
            json.get("title").and_then(serde_json::Value::as_str) == Some(expected_title.as_str());
        let draft = json
            .get("draft")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let head_matches = json
            .get("head")
            .and_then(|head| head.get("ref"))
            .and_then(serde_json::Value::as_str)
            == Some(expected_head.as_str());
        let base_matches = json
            .get("base")
            .and_then(|base| base.get("ref"))
            .and_then(serde_json::Value::as_str)
            == Some(expected_base.as_str());
        let exact_match = draft_pr_matches(
            &json,
            &number,
            &expected_title,
            &expected_head,
            &expected_base,
        );
        let mut fields = HashMap::new();
        fields.insert("draft".into(), draft.to_string());
        if let Some(s) = json.get("state").and_then(|v| v.as_str()) {
            fields.insert("state".into(), s.into());
        }
        fields.insert("number_matches".into(), number_matches.to_string());
        fields.insert("title_matches".into(), title_matches.to_string());
        fields.insert("head_matches".into(), head_matches.to_string());
        fields.insert("base_matches".into(), base_matches.to_string());
        fields.insert("verified".into(), exact_match.to_string());
        Ok(CapabilityStateObservation {
            resource_key: format!("{}/pull/{number}", self.repository),
            fields,
            summary: if exact_match {
                format!("exact draft PR {number} observed")
            } else {
                format!("PR {number} does not match approved draft creation")
            },
            observed: exact_match,
            verification_status: if exact_match {
                CapabilityVerificationStatus::Passed
            } else {
                CapabilityVerificationStatus::Failed
            },
            error: if exact_match {
                None
            } else {
                Some("exact draft PR verification mismatch".into())
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

    fn target(
        &self,
        environment: &str,
        inputs: &serde_json::Value,
    ) -> RivoraResult<CapabilityTarget> {
        validate_environment(environment)?;
        let git_ref = required_input(inputs, "ref", MAX_REF_BYTES)?;
        validate_git_ref(&git_ref, "ref")?;
        github_target(&self.repository, Some(git_ref))
    }

    fn validate_preconditions(&self, request: &CapabilityInvocation) -> RivoraResult<()> {
        validate_repository(&self.repository)?;
        validate_invocation(
            request,
            "github_actions.workflow_dispatch",
            &["dispatch_workflow"],
        )?;
        let workflow = required_input(&request.inputs, "workflow_id", MAX_WORKFLOW_BYTES)?;
        validate_workflow_id(&workflow)?;
        let git_ref = required_input(&request.inputs, "ref", MAX_REF_BYTES)?;
        validate_git_ref(&git_ref, "ref")?;
        workflow_inputs(&request.inputs)?;
        require_token(&self.token)?;
        Ok(())
    }

    fn dry_run(&self, request: &CapabilityInvocation) -> RivoraResult<DryRunResult> {
        validate_repository(&self.repository)?;
        validate_invocation(
            request,
            "github_actions.workflow_dispatch",
            &["dispatch_workflow"],
        )?;
        let workflow = required_input(&request.inputs, "workflow_id", MAX_WORKFLOW_BYTES)?;
        validate_workflow_id(&workflow)?;
        let git_ref = required_input(&request.inputs, "ref", MAX_REF_BYTES)?;
        validate_git_ref(&git_ref, "ref")?;
        workflow_inputs(&request.inputs)?;
        Ok(DryRunResult {
            actions: vec!["dispatch_workflow".into()],
            target: format!("{}/actions/{workflow}", self.repository),
            expected_mutations: vec![format!("dispatch {workflow} @ {git_ref}")],
            required_permissions: vec!["actions:write".into()],
            current_state: None,
            predicted_state: Some("new workflow_run queued".into()),
            risks: vec!["triggers CI compute".into()],
            policy_decision: policy_bounded("github_actions.workflow_dispatch"),
            missing_preconditions: if self.token.as_deref().map_or(true, str::is_empty) {
                vec!["GitHub credential unavailable for live execution".into()]
            } else {
                vec![]
            },
            verification_steps: vec![
                "find a workflow_dispatch run for the exact workflow/ref created after dispatch"
                    .into(),
            ],
            rollback_options: vec!["cancel run if still in progress".into()],
            simulated: false,
        })
    }

    fn execute(&self, request: &CapabilityInvocation) -> RivoraResult<CapabilityExecutionResult> {
        validate_repository(&self.repository)?;
        validate_invocation(
            request,
            "github_actions.workflow_dispatch",
            &["dispatch_workflow"],
        )?;
        let token = require_token(&self.token)?;
        let workflow = required_input(&request.inputs, "workflow_id", MAX_WORKFLOW_BYTES)?;
        validate_workflow_id(&workflow)?;
        let git_ref = required_input(&request.inputs, "ref", MAX_REF_BYTES)?;
        validate_git_ref(&git_ref, "ref")?;
        let inputs = workflow_inputs(&request.inputs)?;
        let url = format!(
            "{}/repos/{}/actions/workflows/{}/dispatches",
            self.api_base.trim_end_matches('/'),
            self.repository,
            urlencoding_lite(&workflow)
        );
        let client = client()?;
        let baseline_run_id = match workflow_baseline_run_id(
            &client,
            &self.api_base,
            &self.repository,
            &workflow,
            token,
        ) {
            Ok(run_id) => run_id,
            Err(error) => {
                return Ok(failed_result(
                    format!("dispatch {workflow} @ {git_ref}"),
                    vec![format!("workflow/{workflow}")],
                    error.to_string(),
                ));
            }
        };
        let dispatched_after = chrono::Utc::now();
        let response = match client
            .post(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .json(&serde_json::json!({ "ref": git_ref, "inputs": inputs }))
            .send()
        {
            Ok(response) => response,
            Err(error) => {
                return Ok(uncertain_result(
                    format!("dispatch {workflow} @ {git_ref}"),
                    vec![format!("workflow/{workflow}")],
                    format!("workflow dispatch outcome unknown: {error}"),
                ));
            }
        };
        let status = response.status();
        if ambiguous_http_status(status) {
            return Ok(uncertain_result(
                format!("dispatch {workflow} @ {git_ref}"),
                vec![format!("workflow/{workflow}")],
                format!("GitHub returned ambiguous HTTP status {status}"),
            ));
        }
        // GitHub returns 204 No Content on success.
        let success = status.as_u16() == 204;
        Ok(CapabilityExecutionResult {
            status: if success {
                CapabilityExecutionStatus::Success
            } else {
                CapabilityExecutionStatus::Failed
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
            external_identifiers: if success {
                vec![
                    format!("dispatch_after:{}", dispatched_after.to_rfc3339()),
                    format!("workflow_baseline_run_id:{baseline_run_id}"),
                ]
            } else {
                vec![]
            },
            warnings: vec!["run id not returned by dispatch API".into()],
            rollback: RollbackMetadata {
                available: false,
                capability_id: None,
                inputs: None,
                inverse_action_name: None,
                risks: vec!["cancel not automated".into()],
                verification: Some("list runs".into()),
                irreversible_effects: vec!["compute may have started".into()],
            },
            verification_requirements: vec![format!(
                "correlated workflow_dispatch run for {workflow} @ {git_ref}"
            )],
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
        validate_repository(&self.repository)?;
        validate_query(
            query,
            "github_actions.workflow_dispatch",
            &["dispatch_workflow"],
        )?;
        let token = match require_token(&self.token) {
            Ok(t) => t,
            Err(e) => {
                return Ok(CapabilityStateObservation {
                    resource_key: self.repository.clone(),
                    fields: HashMap::new(),
                    summary: e.to_string(),
                    observed: false,
                    verification_status: CapabilityVerificationStatus::Inconclusive,
                    error: Some(e.to_string()),
                });
            }
        };
        let workflow = required_input(&query.inputs, "workflow_id", MAX_WORKFLOW_BYTES)?;
        validate_workflow_id(&workflow)?;
        let git_ref = required_input(&query.inputs, "ref", MAX_REF_BYTES)?;
        validate_git_ref(&git_ref, "ref")?;
        let dispatched_after = dispatch_timestamp(query)?;
        let baseline_run_id = workflow_baseline_id(query)?;
        let url = format!(
            "{}/repos/{}/actions/workflows/{}/runs?event=workflow_dispatch&per_page=30",
            self.api_base.trim_end_matches('/'),
            self.repository,
            urlencoding_lite(&workflow)
        );
        let client = client()?;
        let response = match client
            .get(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .send()
        {
            Ok(response) => response,
            Err(error) => {
                return Ok(failed_observation(
                    format!("{}/workflow/{workflow}/ref/{git_ref}", self.repository),
                    format!("workflow run observation failed: {error}"),
                ));
            }
        };
        let ok = response.status().is_success();
        if !ok {
            return Ok(failed_observation(
                format!("{}/workflow/{workflow}/ref/{git_ref}", self.repository),
                format!("GET workflow runs returned {}", response.status()),
            ));
        }
        let json = match read_json(response) {
            Ok(json) => json,
            Err(error) => {
                return Ok(failed_observation(
                    format!("{}/workflow/{workflow}/ref/{git_ref}", self.repository),
                    error,
                ));
            }
        };
        let expected_branch = git_ref
            .strip_prefix("refs/heads/")
            .unwrap_or(git_ref.as_str());
        let correlated = json
            .get("workflow_runs")
            .and_then(serde_json::Value::as_array)
            .and_then(|runs| {
                runs.iter().find(|run| {
                    workflow_run_matches(
                        run,
                        &workflow,
                        expected_branch,
                        dispatched_after,
                        baseline_run_id,
                    )
                })
            });
        let mut fields = HashMap::new();
        fields.insert("workflow".into(), workflow.clone());
        fields.insert("ref".into(), git_ref.clone());
        fields.insert("correlated_run".into(), correlated.is_some().to_string());
        fields.insert("verified".into(), correlated.is_some().to_string());
        if let Some(run_id) = correlated
            .and_then(|run| run.get("id"))
            .and_then(serde_json::Value::as_u64)
        {
            fields.insert("run_id".into(), run_id.to_string());
        }
        Ok(CapabilityStateObservation {
            resource_key: format!("{}/workflow/{workflow}/ref/{git_ref}", self.repository),
            fields,
            summary: if correlated.is_some() {
                format!("correlated workflow run observed for {workflow} @ {git_ref}")
            } else {
                format!("no correlated workflow run observed for {workflow} @ {git_ref}")
            },
            observed: correlated.is_some(),
            verification_status: if correlated.is_some() {
                CapabilityVerificationStatus::Passed
            } else {
                CapabilityVerificationStatus::Failed
            },
            error: if correlated.is_some() {
                None
            } else {
                Some("workflow/ref/resulting-run correlation failed".into())
            },
        })
    }
}

fn validate_git_ref(git_ref: &str, key: &str) -> RivoraResult<()> {
    if git_ref.starts_with('/')
        || git_ref.ends_with('/')
        || git_ref.ends_with('.')
        || git_ref.contains("..")
        || git_ref.contains("//")
        || git_ref.contains("@{")
        || git_ref.contains('\\')
        || git_ref
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || matches!(c, '~' | '^' | '?' | '*'))
    {
        return Err(RivoraError::validation(format!(
            "{key} is not a safe Git ref"
        )));
    }
    Ok(())
}

fn validate_workflow_id(workflow: &str) -> RivoraResult<()> {
    if let Ok(id) = workflow.parse::<u64>() {
        return if id > 0 {
            Ok(())
        } else {
            Err(RivoraError::validation(
                "workflow_id must be a positive numeric id",
            ))
        };
    }
    if workflow.starts_with('.')
        || workflow.ends_with('.')
        || workflow.contains("..")
        || !workflow
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err(RivoraError::validation(
            "workflow_id must be a numeric id or bounded workflow file name",
        ));
    }
    Ok(())
}

fn workflow_inputs(inputs: &serde_json::Value) -> RivoraResult<serde_json::Value> {
    let value = inputs
        .get("workflow_inputs")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    if !value.is_object() {
        return Err(RivoraError::validation("workflow_inputs must be an object"));
    }
    let encoded = serde_json::to_vec(&value)
        .map_err(|e| RivoraError::validation(format!("invalid workflow_inputs: {e}")))?;
    if encoded.len() > MAX_BODY_BYTES {
        return Err(RivoraError::validation(
            "workflow_inputs exceeds safe input bounds",
        ));
    }
    Ok(value)
}

fn dispatch_timestamp(query: &CapabilityStateQuery) -> RivoraResult<chrono::DateTime<chrono::Utc>> {
    let raw = query
        .external_identifiers
        .iter()
        .find_map(|identifier| identifier.strip_prefix("dispatch_after:"))
        .ok_or_else(|| RivoraError::validation("missing workflow dispatch correlation time"))?;
    chrono::DateTime::parse_from_rfc3339(raw)
        .map(|timestamp| timestamp.with_timezone(&chrono::Utc))
        .map_err(|_| RivoraError::validation("invalid workflow dispatch correlation time"))
}

fn workflow_baseline_id(query: &CapabilityStateQuery) -> RivoraResult<u64> {
    query
        .external_identifiers
        .iter()
        .find_map(|identifier| identifier.strip_prefix("workflow_baseline_run_id:"))
        .ok_or_else(|| RivoraError::validation("missing workflow baseline run identifier"))?
        .parse::<u64>()
        .map_err(|_| RivoraError::validation("invalid workflow baseline run identifier"))
}

fn workflow_baseline_run_id(
    client: &reqwest::blocking::Client,
    api_base: &str,
    repository: &str,
    workflow: &str,
    token: &str,
) -> RivoraResult<u64> {
    let url = format!(
        "{}/repos/{repository}/actions/workflows/{}/runs?event=workflow_dispatch&per_page=30",
        api_base.trim_end_matches('/'),
        urlencoding_lite(workflow)
    );
    let response = client
        .get(url)
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|error| {
            RivoraError::precondition(format!(
                "cannot establish workflow run baseline before dispatch: {error}"
            ))
        })?;
    if !response.status().is_success() {
        return Err(RivoraError::precondition(format!(
            "cannot establish workflow run baseline before dispatch: HTTP {}",
            response.status()
        )));
    }
    let json = read_json(response).map_err(RivoraError::precondition)?;
    let runs = json
        .get("workflow_runs")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            RivoraError::precondition(
                "cannot establish workflow run baseline: expected workflow_runs",
            )
        })?;
    Ok(runs
        .iter()
        .filter_map(|run| run.get("id").and_then(serde_json::Value::as_u64))
        .max()
        .unwrap_or(0))
}

fn comment_matches(
    json: &serde_json::Value,
    comment_id: &str,
    issue: &str,
    expected_body: &str,
) -> bool {
    let observed_id = json.get("id").and_then(serde_json::Value::as_u64);
    let observed_body = json
        .get("body")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let observed_issue = json
        .get("issue_url")
        .and_then(serde_json::Value::as_str)
        .and_then(|url| url.rsplit('/').next());
    observed_id == comment_id.parse::<u64>().ok()
        && observed_body == expected_body
        && observed_issue == Some(issue)
}

fn labels_contain(json: &serde_json::Value, label: &str) -> bool {
    json.as_array().is_some_and(|labels| {
        labels
            .iter()
            .any(|item| item.get("name").and_then(serde_json::Value::as_str) == Some(label))
    })
}

fn issue_matches(
    json: &serde_json::Value,
    number: &str,
    expected_title: &str,
    expected_body: &str,
) -> bool {
    json.get("number").and_then(serde_json::Value::as_u64) == number.parse::<u64>().ok()
        && json.get("title").and_then(serde_json::Value::as_str) == Some(expected_title)
        && json
            .get("body")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            == expected_body
        && json.get("state").and_then(serde_json::Value::as_str) == Some("open")
        && json.get("pull_request").is_none()
}

fn draft_pr_matches(
    json: &serde_json::Value,
    number: &str,
    expected_title: &str,
    expected_head: &str,
    expected_base: &str,
) -> bool {
    json.get("number").and_then(serde_json::Value::as_u64) == number.parse::<u64>().ok()
        && json.get("title").and_then(serde_json::Value::as_str) == Some(expected_title)
        && json.get("draft").and_then(serde_json::Value::as_bool) == Some(true)
        && json.get("state").and_then(serde_json::Value::as_str) == Some("open")
        && json
            .get("head")
            .and_then(|head| head.get("ref"))
            .and_then(serde_json::Value::as_str)
            == Some(expected_head)
        && json
            .get("base")
            .and_then(|base| base.get("ref"))
            .and_then(serde_json::Value::as_str)
            == Some(expected_base)
}

fn label_rollback(action_name: &str, issue: &str, label: &str) -> RollbackMetadata {
    RollbackMetadata {
        available: true,
        capability_id: Some("github.issue.label".into()),
        inputs: Some(serde_json::json!({
            "issue_number": issue,
            "label": label
        })),
        inverse_action_name: Some(
            if action_name == "add_label" {
                "remove_label"
            } else {
                "add_label"
            }
            .into(),
        ),
        risks: vec!["label state may have changed again before rollback".into()],
        verification: Some("GET issue labels and compare exact inverse state".into()),
        irreversible_effects: vec![],
    }
}

fn workflow_run_matches(
    run: &serde_json::Value,
    workflow: &str,
    expected_branch: &str,
    dispatched_after: chrono::DateTime<chrono::Utc>,
    baseline_run_id: u64,
) -> bool {
    let event_matches =
        run.get("event").and_then(serde_json::Value::as_str) == Some("workflow_dispatch");
    let ref_matches =
        run.get("head_branch").and_then(serde_json::Value::as_str) == Some(expected_branch);
    let created_after = run
        .get("created_at")
        .and_then(serde_json::Value::as_str)
        .and_then(|created| chrono::DateTime::parse_from_rfc3339(created).ok())
        .is_some_and(|created| created.with_timezone(&chrono::Utc) >= dispatched_after);
    let is_new_run = run
        .get("id")
        .and_then(serde_json::Value::as_u64)
        .is_some_and(|run_id| run_id > baseline_run_id);
    let workflow_matches = workflow.parse::<u64>().ok().is_some_and(|expected| {
        run.get("workflow_id").and_then(serde_json::Value::as_u64) == Some(expected)
    }) || run
        .get("path")
        .and_then(serde_json::Value::as_str)
        .and_then(|path| path.split('@').next())
        .and_then(|path| path.rsplit('/').next())
        == Some(workflow);
    event_matches && ref_matches && created_after && is_new_run && workflow_matches
}

fn urlencoding_lite(s: &str) -> String {
    let mut encoded = String::with_capacity(s.len());
    for byte in s.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invocation(
        capability_id: &str,
        action_name: &str,
        inputs: serde_json::Value,
    ) -> CapabilityInvocation {
        CapabilityInvocation {
            capability_id: capability_id.into(),
            action_name: action_name.into(),
            action_id: "action-1".into(),
            inputs,
            environment: "production".into(),
            idempotency_key: "attempt-1:action-1".into(),
            investigation_id: "investigation-1".into(),
            plan_id: "plan-1".into(),
        }
    }

    #[test]
    fn targets_bind_exact_repository_and_refs() {
        let capability = GitHubDraftPrCapability {
            repository: "rivora-dev/rivora".into(),
            token: Some("token".into()),
            api_base: "https://api.github.test".into(),
        };
        let target = capability
            .target(
                "production",
                &serde_json::json!({"head": "feature/a", "base": "main"}),
            )
            .expect("valid target");
        assert_eq!(target.provider, "github");
        assert_eq!(target.owner.as_deref(), Some("rivora-dev"));
        assert_eq!(target.repository.as_deref(), Some("rivora"));
        assert_eq!(target.branch_or_ref.as_deref(), Some("feature/a->main"));

        let invalid = GitHubIssueCreateCapability {
            repository: "rivora-dev/rivora/other".into(),
            token: Some("token".into()),
            api_base: "https://api.github.test".into(),
        };
        assert!(invalid
            .target("production", &serde_json::json!({}))
            .is_err());
    }

    #[test]
    fn preflight_rejects_missing_credentials_and_invalid_contracts() {
        let capability = GitHubIssueCommentCapability {
            repository: "rivora-dev/rivora".into(),
            token: None,
            api_base: "https://api.github.test".into(),
        };
        let valid = invocation(
            "github.issue.comment",
            "create_comment",
            serde_json::json!({"issue_number": "7", "body": "exact"}),
        );
        assert!(capability.validate_preconditions(&valid).is_err());

        let wrong_action = invocation(
            "github.issue.comment",
            "delete_comment",
            serde_json::json!({"issue_number": "7", "body": "exact"}),
        );
        assert!(capability.dry_run(&wrong_action).is_err());

        let unsafe_number = invocation(
            "github.issue.comment",
            "create_comment",
            serde_json::json!({"issue_number": "../7", "body": "exact"}),
        );
        assert!(capability.dry_run(&unsafe_number).is_err());
    }

    #[test]
    fn comment_verification_requires_exact_id_issue_and_content() {
        let json = serde_json::json!({
            "id": 42,
            "body": "approved body",
            "issue_url": "https://api.github.com/repos/rivora-dev/rivora/issues/7"
        });
        assert!(comment_matches(&json, "42", "7", "approved body"));
    }

    #[test]
    fn comment_verification_rejects_same_id_with_different_content() {
        let json = serde_json::json!({
            "id": 42,
            "body": "different",
            "issue_url": "https://api.github.com/repos/rivora-dev/rivora/issues/7"
        });
        assert!(!comment_matches(&json, "42", "7", "approved body"));
        assert!(!comment_matches(&json, "42", "8", "different"));
    }

    #[test]
    fn label_verification_parses_exact_final_state() {
        let labels = serde_json::json!([{"name": "not-bug"}, {"name": "documentation"}]);
        assert!(!labels_contain(&labels, "bug"));
        assert!(labels_contain(&labels, "not-bug"));
    }

    #[test]
    fn label_receipt_declares_explicit_inverse_action() {
        let rollback = label_rollback("add_label", "7", "bug");
        assert_eq!(
            rollback.inverse_action_name.as_deref(),
            Some("remove_label")
        );
        assert_eq!(
            label_rollback("remove_label", "7", "bug")
                .inverse_action_name
                .as_deref(),
            Some("add_label")
        );
    }

    #[test]
    fn issue_verification_rejects_a_pull_request_or_content_mismatch() {
        let pull = serde_json::json!({
            "number": 9,
            "title": "approved",
            "body": "body",
            "state": "open",
            "pull_request": {}
        });
        assert!(!issue_matches(&pull, "9", "approved", "body"));
        let issue =
            serde_json::json!({"number": 9, "title": "approved", "body": "body", "state": "open"});
        assert!(issue_matches(&issue, "9", "approved", "body"));
        assert!(!issue_matches(&issue, "9", "different", "body"));
    }

    #[test]
    fn draft_pr_verification_requires_draft_and_exact_refs() {
        let ready = serde_json::json!({
            "number": 11,
            "title": "approved",
            "draft": false,
            "head": {"ref": "feature/a"},
            "base": {"ref": "main"}
        });
        assert!(!draft_pr_matches(
            &ready,
            "11",
            "approved",
            "feature/a",
            "main"
        ));
        let draft = serde_json::json!({
            "number": 11,
            "title": "approved",
            "draft": true,
            "state": "open",
            "head": {"ref": "feature/a"},
            "base": {"ref": "main"}
        });
        assert!(draft_pr_matches(
            &draft,
            "11",
            "approved",
            "feature/a",
            "main"
        ));
    }

    #[test]
    fn workflow_verification_correlates_workflow_ref_event_and_time() {
        let after = chrono::DateTime::parse_from_rfc3339("2026-07-23T12:00:00Z")
            .expect("timestamp")
            .with_timezone(&chrono::Utc);
        let run = serde_json::json!({
            "id": 77,
            "workflow_id": 123,
            "event": "workflow_dispatch",
            "head_branch": "main",
            "created_at": "2026-07-23T12:00:01Z",
            "path": ".github/workflows/release.yml@refs/heads/main"
        });
        assert!(workflow_run_matches(&run, "release.yml", "main", after, 76));
        assert!(!workflow_run_matches(
            &run,
            "release.yml",
            "main",
            after,
            77
        ));
        assert!(!workflow_run_matches(&run, "other.yml", "main", after, 76));
        assert!(!workflow_run_matches(
            &run,
            "release.yml",
            "other",
            after,
            76
        ));
    }

    #[test]
    fn malformed_success_response_is_uncertain_not_success() {
        let result = uncertain_result(
            "POST issue approved".into(),
            vec!["rivora-dev/rivora".into()],
            "GitHub issue response missing numeric issue number",
        );
        assert_eq!(result.status, CapabilityExecutionStatus::Uncertain);
        assert!(!result.verification_requirements.is_empty());
    }

    #[test]
    fn transport_error_is_uncertain_not_definite_failure() {
        let result = uncertain_result(
            "POST comment issue 7".into(),
            vec!["issue/7".into()],
            "request timed out",
        );
        assert_eq!(result.status, CapabilityExecutionStatus::Uncertain);
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.contains("do not retry")));
        assert!(ambiguous_http_status(reqwest::StatusCode::REQUEST_TIMEOUT));
        assert!(ambiguous_http_status(reqwest::StatusCode::GATEWAY_TIMEOUT));
        assert!(!ambiguous_http_status(reqwest::StatusCode::BAD_REQUEST));
    }

    #[test]
    fn workflow_and_ref_bounds_reject_path_or_ref_injection() {
        let capability = GitHubWorkflowDispatchCapability {
            repository: "rivora-dev/rivora".into(),
            token: Some("token".into()),
            api_base: "https://api.github.test".into(),
        };
        let unsafe_workflow = invocation(
            "github_actions.workflow_dispatch",
            "dispatch_workflow",
            serde_json::json!({"workflow_id": "../release.yml", "ref": "main"}),
        );
        assert!(capability.dry_run(&unsafe_workflow).is_err());

        let unsafe_ref = invocation(
            "github_actions.workflow_dispatch",
            "dispatch_workflow",
            serde_json::json!({"workflow_id": "release.yml", "ref": "main..evil"}),
        );
        assert!(capability.dry_run(&unsafe_ref).is_err());
    }
}
