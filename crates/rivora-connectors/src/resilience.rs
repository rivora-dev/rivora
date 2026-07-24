//! Shared Connector resilience helpers (v0.9).
//!
//! Timeouts, payload bounds, rate-limit handling, and secret redaction
//! for observation connectors. Connectors remain reasoning-free.

use std::time::Duration;

use rivora::domain::{
    CONNECTOR_CONNECT_TIMEOUT_SECS, CONNECTOR_REQUEST_TIMEOUT_SECS, MAX_CONNECTOR_RESPONSE_BYTES,
    MAX_EVENT_BATCH_SIZE, MAX_PAYLOAD_BYTES,
};
use serde_json::Value;

use crate::{ConnectorError, ConnectorResult};

/// HTTP connect timeout for observation connectors.
pub fn connect_timeout() -> Duration {
    Duration::from_secs(CONNECTOR_CONNECT_TIMEOUT_SECS)
}

/// HTTP request timeout for observation connectors.
pub fn request_timeout() -> Duration {
    Duration::from_secs(CONNECTOR_REQUEST_TIMEOUT_SECS)
}

/// Maximum accepted HTTP response body size (bytes).
pub fn max_response_bytes() -> usize {
    MAX_CONNECTOR_RESPONSE_BYTES
}

/// Maximum Observation payload size (bytes).
pub fn max_payload_bytes() -> usize {
    MAX_PAYLOAD_BYTES
}

/// Maximum events per observe batch.
pub fn max_event_batch_size() -> usize {
    MAX_EVENT_BATCH_SIZE
}

/// Build a blocking HTTP client with production timeouts.
pub fn http_client(user_agent: &str) -> ConnectorResult<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .user_agent(user_agent)
        .connect_timeout(connect_timeout())
        .timeout(request_timeout())
        .build()
        .map_err(|e| ConnectorError::Api(sanitize_error(&e.to_string())))
}

/// Read a response body with a hard size limit.
pub fn read_response_limited(
    response: reqwest::blocking::Response,
) -> ConnectorResult<(reqwest::StatusCode, Vec<u8>)> {
    let status = response.status();
    let bytes = response
        .bytes()
        .map_err(|e| ConnectorError::Api(sanitize_error(&e.to_string())))?;
    if bytes.len() > max_response_bytes() {
        return Err(ConnectorError::PayloadTooLarge(format!(
            "response is {} bytes; max is {}",
            bytes.len(),
            max_response_bytes()
        )));
    }
    Ok((status, bytes.to_vec()))
}

/// Map HTTP status codes to structured Connector errors (no secrets).
pub fn map_http_status(status: reqwest::StatusCode, body_preview: &str) -> Option<ConnectorError> {
    let lower = body_preview.to_ascii_lowercase();
    match status.as_u16() {
        401 | 403 if lower.contains("rate limit") || lower.contains("secondary rate") => Some(
            ConnectorError::RateLimited("provider rate limited the request".into()),
        ),
        429 => Some(ConnectorError::RateLimited(
            "provider returned HTTP 429 Too Many Requests".into(),
        )),
        401 => Some(ConnectorError::Auth(
            "provider authentication failed".into(),
        )),
        403 => Some(ConnectorError::Auth(
            "provider forbidden the request (credentials or permissions)".into(),
        )),
        408 | 504 => Some(ConnectorError::Timeout("provider request timed out".into())),
        code if !(200..300).contains(&code) => {
            Some(ConnectorError::Api(format!("provider HTTP status {code}")))
        }
        _ => None,
    }
}

/// Truncate an observation batch to the supported envelope.
pub fn bound_batch<T>(items: Vec<T>) -> Vec<T> {
    let max = max_event_batch_size();
    if items.len() <= max {
        items
    } else {
        items.into_iter().take(max).collect()
    }
}

/// Ensure a JSON payload is within the envelope size limit.
pub fn ensure_payload_size(payload: &Value) -> ConnectorResult<()> {
    let bytes = serde_json::to_vec(payload)
        .map_err(|e| ConnectorError::Normalize(sanitize_error(&e.to_string())))?;
    if bytes.len() > max_payload_bytes() {
        return Err(ConnectorError::PayloadTooLarge(format!(
            "payload is {} bytes; max is {}",
            bytes.len(),
            max_payload_bytes()
        )));
    }
    Ok(())
}

/// Redact common secret field names in a JSON value (in place).
pub fn redact_json(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let keys: Vec<String> = map.keys().cloned().collect();
            for key in keys {
                if is_sensitive_key(&key) {
                    map.insert(key, Value::String("[redacted]".into()));
                } else if let Some(child) = map.get_mut(&key) {
                    redact_json(child);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_json(item);
            }
        }
        _ => {}
    }
}

/// Redact secret-like substrings from free-form error text.
pub fn sanitize_error(text: &str) -> String {
    let mut out = text.to_string();
    for marker in [
        "Bearer ",
        "token=",
        "token:",
        "password=",
        "secret=",
        "authorization:",
        "api_key=",
        "apikey=",
    ] {
        let marker_l = marker.to_ascii_lowercase();
        let mut cursor = 0usize;
        while cursor < out.len() {
            let lower = out[cursor..].to_ascii_lowercase();
            let Some(rel) = lower.find(&marker_l) else {
                break;
            };
            let idx = cursor + rel;
            let start = idx + marker.len();
            if start > out.len() {
                break;
            }
            // Skip values already redacted to avoid infinite replacement loops.
            if out[start..].starts_with("[redacted]") {
                cursor = start + "[redacted]".len();
                continue;
            }
            let end = out[start..]
                .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ',')
                .map(|i| start + i)
                .unwrap_or(out.len());
            if end > start {
                out.replace_range(start..end, "[redacted]");
                cursor = start + "[redacted]".len();
            } else {
                cursor = start.max(idx + 1);
            }
        }
    }
    // Bound error message length.
    if out.len() > 512 {
        out.truncate(512);
        out.push('…');
    }
    out
}

fn is_sensitive_key(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    matches!(
        k.as_str(),
        "token"
            | "access_token"
            | "refresh_token"
            | "authorization"
            | "password"
            | "secret"
            | "client_secret"
            | "api_key"
            | "apikey"
            | "auth"
            | "authtoken"
            | "dsn"
            | "private_key"
            | "cookie"
    ) || k.contains("token")
        || k.contains("secret")
        || k.contains("password")
        || k.contains("authorization")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redacts_nested_secrets() {
        let mut v = json!({
            "name": "ok",
            "token": "super-secret",
            "nested": { "password": "x", "value": 1 }
        });
        redact_json(&mut v);
        assert_eq!(v["token"], "[redacted]");
        assert_eq!(v["nested"]["password"], "[redacted]");
        assert_eq!(v["nested"]["value"], 1);
        assert_eq!(v["name"], "ok");
    }

    #[test]
    fn sanitizes_bearer_tokens() {
        let s = sanitize_error("Authorization: Bearer abcdef123456 failed");
        assert!(!s.contains("abcdef123456"));
        assert!(s.contains("[redacted]"));
    }

    #[test]
    fn bounds_batch_size() {
        let items: Vec<u32> = (0..max_event_batch_size() + 10).map(|i| i as u32).collect();
        let bounded = bound_batch(items);
        assert_eq!(bounded.len(), max_event_batch_size());
    }
}
