//! Read-only Kubernetes connector (RFC-012, v0.3).
//!
//! Fixture-first with optional kubeconfig-based live inspect via `kubectl`
//! when available. Never mutates cluster state.

use chrono::Utc;
use rivora::domain::ObservationKind;
use serde_json::{json, Value};

use crate::github_actions::ConnectorStatusReport;
use crate::{ConnectorError, ConnectorResult, NormalizedObservation};

/// Read-only Kubernetes connector.
#[derive(Debug, Clone)]
pub struct KubernetesConnector {
    /// Optional context name.
    pub context: Option<String>,
    /// Namespace filter (default: default).
    pub namespace: String,
    /// Path to kubeconfig; defaults to KUBECONFIG or ~/.kube/config presence check.
    pub kubeconfig: Option<String>,
}

impl KubernetesConnector {
    /// Create a connector for a namespace.
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            context: None,
            namespace: namespace.into(),
            kubeconfig: std::env::var("KUBECONFIG").ok().filter(|s| !s.is_empty()),
        }
    }

    /// Status report without secrets.
    pub fn status(&self) -> ConnectorStatusReport {
        let configured = self.kubeconfig.is_some()
            || std::path::Path::new(&shellexpand_home("~/.kube/config")).exists();
        ConnectorStatusReport {
            id: "kubernetes".into(),
            category: "infrastructure".into(),
            configured,
            read_only: true,
            details: format!(
                "namespace={} context={} kubeconfig={}",
                self.namespace,
                self.context.as_deref().unwrap_or("(default)"),
                if configured { "present" } else { "missing" }
            ),
        }
    }

    /// Test configuration.
    pub fn test_configuration(&self) -> ConnectorResult<String> {
        if self.namespace.trim().is_empty() {
            return Err(ConnectorError::Config("namespace must not be empty".into()));
        }
        Ok(format!(
            "kubernetes: namespace={} read-only; fixture mode available",
            self.namespace
        ))
    }

    /// Live observe via `kubectl get` (read-only). Fails clearly when unavailable.
    pub fn observe(&self) -> ConnectorResult<Vec<NormalizedObservation>> {
        let mut cmd = std::process::Command::new("kubectl");
        cmd.arg("get")
            .arg("pods")
            .arg("-n")
            .arg(&self.namespace)
            .arg("-o")
            .arg("json");
        if let Some(ctx) = &self.context {
            cmd.arg("--context").arg(ctx);
        }
        if let Some(cfg) = &self.kubeconfig {
            cmd.env("KUBECONFIG", cfg);
        }
        let output = cmd.output().map_err(|e| {
            ConnectorError::Config(format!(
                "kubectl unavailable for live observe ({e}); use fixture mode"
            ))
        })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let redacted = redact_secrets(&stderr);
            return Err(ConnectorError::Api(format!(
                "kubectl get pods failed: {redacted}"
            )));
        }
        let body: Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| ConnectorError::Normalize(e.to_string()))?;
        Self::normalize_pod_list(&self.namespace, &body)
    }

    /// Observe from fixture JSON.
    pub fn observe_from_fixture(fixture: &Value) -> ConnectorResult<Vec<NormalizedObservation>> {
        let namespace = fixture
            .get("namespace")
            .and_then(|v| v.as_str())
            .unwrap_or("default");
        if let Some(items) = fixture.get("items") {
            let wrapper = json!({ "items": items });
            return Self::normalize_pod_list(namespace, &wrapper);
        }
        if let Some(pods) = fixture.get("pods") {
            let wrapper = json!({ "items": pods });
            return Self::normalize_pod_list(namespace, &wrapper);
        }
        if fixture.get("kind").and_then(|v| v.as_str()) == Some("PodList") {
            return Self::normalize_pod_list(namespace, fixture);
        }
        // Single deployment/resource snapshot.
        if fixture.get("kind").is_some() {
            return Self::normalize_resource(namespace, fixture);
        }
        Err(ConnectorError::Normalize(
            "fixture must include items/pods or a Kubernetes resource object".into(),
        ))
    }

    fn normalize_pod_list(
        namespace: &str,
        body: &Value,
    ) -> ConnectorResult<Vec<NormalizedObservation>> {
        let items = body
            .get("items")
            .and_then(|v| v.as_array())
            .ok_or_else(|| ConnectorError::Normalize("missing items array".into()))?;
        let mut out = Vec::new();
        for item in items {
            out.extend(Self::normalize_resource(namespace, item)?);
        }
        if out.is_empty() {
            // Empty cluster is valid — emit a health snapshot.
            out.push(NormalizedObservation::new(
                ObservationKind::Infrastructure,
                format!("Kubernetes namespace `{namespace}` has no pods"),
                json!({"namespace": namespace, "pod_count": 0}),
                "kubernetes",
                Utc::now(),
                Some(format!("k8s-empty:{namespace}")),
                "kubernetes-connector",
            ));
        }
        Ok(out)
    }

    fn normalize_resource(
        namespace: &str,
        resource: &Value,
    ) -> ConnectorResult<Vec<NormalizedObservation>> {
        let kind = resource
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("Resource");
        let name = resource
            .pointer("/metadata/name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let phase = resource
            .pointer("/status/phase")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        let ready = resource
            .pointer("/status/containerStatuses")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter(|c| c.get("ready").and_then(|v| v.as_bool()).unwrap_or(false))
                    .count()
            });
        let mut payload = resource.clone();
        redact_object(&mut payload);

        let health = if phase == "Running" || phase == "Succeeded" {
            "healthy"
        } else if phase == "Failed" || phase == "Unknown" {
            "unhealthy"
        } else {
            "degraded"
        };

        Ok(vec![NormalizedObservation::new(
            ObservationKind::Infrastructure,
            format!(
                "Kubernetes {kind} `{name}` in `{namespace}` phase={phase} health={health}{}",
                ready
                    .map(|r| format!(" ready_containers={r}"))
                    .unwrap_or_default()
            ),
            payload,
            "kubernetes",
            Utc::now(),
            Some(format!("k8s:{namespace}:{kind}:{name}")),
            "kubernetes-connector",
        )])
    }
}

fn shellexpand_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{home}/{rest}");
        }
    }
    path.to_string()
}

fn redact_secrets(text: &str) -> String {
    let mut out = text.to_string();
    for secretish in ["Bearer ", "token=", "password=", "secret="] {
        if let Some(idx) = out.find(secretish) {
            let end = (idx + secretish.len() + 12).min(out.len());
            out.replace_range(idx..end, &format!("{secretish}[redacted]"));
        }
    }
    out
}

fn redact_object(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                let lower = k.to_lowercase();
                if lower.contains("token")
                    || lower.contains("secret")
                    || lower.contains("password")
                    || lower == "data" && v.is_object()
                {
                    *v = json!("[redacted]");
                } else {
                    redact_object(v);
                }
            }
        }
        Value::Array(arr) => {
            for v in arr {
                redact_object(v);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fixture_normalizes_pods() {
        let fixture = json!({
            "namespace": "prod",
            "items": [{
                "kind": "Pod",
                "metadata": {"name": "api-1"},
                "status": {
                    "phase": "Failed",
                    "containerStatuses": [{"ready": false, "name": "api"}]
                },
                "spec": {"serviceAccountToken": "leak-me"}
            }]
        });
        let obs = KubernetesConnector::observe_from_fixture(&fixture).unwrap();
        assert_eq!(obs.len(), 1);
        assert!(matches!(obs[0].kind, ObservationKind::Infrastructure));
        assert!(obs[0].summary.contains("unhealthy") || obs[0].summary.contains("Failed"));
        assert_eq!(
            obs[0].payload["spec"]["serviceAccountToken"],
            json!("[redacted]")
        );
    }

    #[test]
    fn empty_items_emits_snapshot() {
        let fixture = json!({"namespace": "dev", "items": []});
        let obs = KubernetesConnector::observe_from_fixture(&fixture).unwrap();
        assert_eq!(obs.len(), 1);
        assert!(obs[0].summary.contains("no pods"));
    }

    #[test]
    fn malformed_fixture_errors() {
        let err = KubernetesConnector::observe_from_fixture(&json!({"x": 1})).unwrap_err();
        assert!(matches!(err, ConnectorError::Normalize(_)));
    }
}
