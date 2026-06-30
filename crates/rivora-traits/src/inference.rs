//! The [`InferenceProvider`] trait — abstract inference backend.
//!
//! An inference provider represents any LLM or reasoning backend. Examples
//! include Claude, OpenAI, Gemini, Ollama, Workers AI, OpenRouter, and
//! custom endpoints.
//!
//! Providers receive a [`ReasoningRequest`] and return a [`ReasoningResponse`]
//! containing proposed reasoning, not final answers. The engine records the
//! provider's output as evidence and computes confidence separately.
//!
//! # Design principles
//!
//! - **Proposed reasoning**: the provider suggests; the engine decides.
//! - **Determinism contract**: providers accept a `deterministic` flag.
//!   Non-deterministic outputs are recorded in receipts.
//! - **Portable**: no SDK-specific types; the trait uses only standard Rust
//!   and serde types.

use serde::{Deserialize, Serialize};

use crate::HealthStatus;

/// A request to an inference provider.
///
/// Contains the prompt, context, and configuration for a reasoning call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReasoningRequest {
    /// The prompt or question to reason about.
    pub prompt: String,
    /// Optional context to condition the reasoning on.
    pub context: Vec<String>,
    /// Whether the caller requires deterministic output (same input → same
    /// output). Non-deterministic providers should return an error if this
    /// is `true` and they cannot comply.
    pub deterministic: bool,
    /// Optional temperature for providers that support it. Ignored by
    /// deterministic providers.
    pub temperature: Option<f64>,
}

/// The response from an inference provider.
///
/// Contains proposed reasoning, a confidence indicator, and metadata about
/// the provider's output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReasoningResponse {
    /// The provider's proposed reasoning or conclusion.
    pub reasoning: String,
    /// The provider's self-assessed confidence (0.0 – 1.0).
    pub confidence: f64,
    /// Tokens consumed by this request (if reported by the provider).
    pub tokens_used: Option<u64>,
    /// The model or engine that produced this response.
    pub model: String,
}

/// Metadata describing an inference provider's identity and capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InferenceMetadata {
    /// Unique identifier for the provider (e.g. `"openai"`, `"claude"`).
    pub id: String,
    /// The model identifier (e.g. `"gpt-4"`, `"claude-3-opus"`).
    pub model: String,
    /// Version of the model or provider.
    pub version: String,
    /// Whether this provider supports deterministic output.
    pub deterministic: bool,
}

/// An inference backend that proposes reasoning from context.
///
/// # Examples
///
/// ```rust
/// use rivora_traits::inference::{
///     InferenceProvider, InferenceMetadata, ReasoningRequest, ReasoningResponse,
/// };
/// use rivora_traits::HealthStatus;
///
/// struct EchoProvider;
///
/// impl InferenceProvider for EchoProvider {
///     fn metadata(&self) -> InferenceMetadata {
///         InferenceMetadata {
///             id: "echo".into(),
///             model: "echo-1".into(),
///             version: "0.1.0".into(),
///             deterministic: true,
///         }
///     }
///
///     fn health(&self) -> HealthStatus {
///         HealthStatus::Healthy
///     }
///
///     fn reason(&self, request: &ReasoningRequest) -> ReasoningResponse {
///         ReasoningResponse {
///             reasoning: format!("Echo: {}", request.prompt),
///             confidence: 1.0,
///             tokens_used: None,
///             model: "echo-1".into(),
///         }
///     }
/// }
///
/// let p = EchoProvider;
/// assert_eq!(p.metadata().model, "echo-1");
/// let resp = p.reason(&ReasoningRequest {
///     prompt: "hello".into(),
///     context: vec![],
///     deterministic: true,
///     temperature: None,
/// });
/// assert_eq!(resp.confidence, 1.0);
/// ```
pub trait InferenceProvider: Send + Sync {
    /// Returns metadata identifying this provider and its model.
    fn metadata(&self) -> InferenceMetadata;

    /// Returns the current health status of the provider.
    fn health(&self) -> HealthStatus;

    /// Sends a reasoning request and returns the provider's proposed response.
    ///
    /// The response contains proposed reasoning, not final answers. The
    /// engine records this as evidence and computes confidence separately.
    fn reason(&self, request: &ReasoningRequest) -> ReasoningResponse;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reasoning_request_round_trips_through_serde() {
        let req = ReasoningRequest {
            prompt: "analyze this".into(),
            context: vec!["ctx1".into()],
            deterministic: true,
            temperature: Some(0.7),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: ReasoningRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn reasoning_response_round_trips_through_serde() {
        let resp = ReasoningResponse {
            reasoning: "because of X".into(),
            confidence: 0.85,
            tokens_used: Some(128),
            model: "test-model".into(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: ReasoningResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back, resp);
    }

    #[test]
    fn inference_metadata_round_trips_through_serde() {
        let meta = InferenceMetadata {
            id: "openai".into(),
            model: "gpt-4".into(),
            version: "2024-01".into(),
            deterministic: false,
        };
        let json = serde_json::to_string(&meta).unwrap();
        let back: InferenceMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(back, meta);
    }
}
