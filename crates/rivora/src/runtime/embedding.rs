//! Pluggable, local-first embedding abstraction (RFC-016).
//!
//! Semantic recall is optional and replaceable. The default provider is
//! a deterministic local baseline — hashed term-frequency vectors with
//! cosine similarity — requiring no network, no model download, and no
//! external provider. Embeddings are computed at query time and never
//! persisted, so search remains a derived read over durable records.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// A replaceable text-embedding provider.
///
/// Implementations must be deterministic: equal input text must produce
/// equal vectors. Providers are local-first; none is required by the
/// Runtime contract beyond the deterministic baseline.
pub trait EmbeddingProvider: Send + Sync {
    /// Provider name, surfaced in explanations.
    fn name(&self) -> &str;
    /// Embed text into a vector. Equal inputs must give equal outputs.
    fn embed(&self, text: &str) -> Vec<f32>;
}

/// Deterministic local embedding baseline.
///
/// Tokens (lowercase alphanumeric, length >= 2) are hashed into a
/// fixed-dimension term-frequency vector, L2-normalized. Hashing uses
/// [`DefaultHasher::new`], which uses fixed keys and is therefore
/// deterministic within a build; vectors are computed at query time and
/// never persisted, so cross-build stability is not required.
#[derive(Debug, Clone, Copy)]
pub struct TokenHashEmbedding {
    dimension: usize,
}

impl TokenHashEmbedding {
    /// Default dimension (256 buckets).
    pub const DEFAULT_DIMENSION: usize = 256;

    /// Create the baseline provider with the default dimension.
    pub fn new() -> Self {
        Self::with_dimension(Self::DEFAULT_DIMENSION)
    }

    /// Create the baseline provider with a custom dimension.
    pub fn with_dimension(dimension: usize) -> Self {
        Self {
            dimension: dimension.max(1),
        }
    }
}

impl Default for TokenHashEmbedding {
    fn default() -> Self {
        Self::new()
    }
}

impl EmbeddingProvider for TokenHashEmbedding {
    fn name(&self) -> &str {
        "token-hash-v1"
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        let mut vector = vec![0.0f32; self.dimension];
        for token in text
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() >= 2)
        {
            let mut hasher = DefaultHasher::new();
            token.hash(&mut hasher);
            let bucket = (hasher.finish() % self.dimension as u64) as usize;
            vector[bucket] += 1.0;
        }
        let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 0.0 {
            for value in &mut vector {
                *value /= norm;
            }
        }
        vector
    }
}

/// Cosine similarity of two vectors, clamped to `[0.0, 1.0]`.
///
/// Zero-length or mismatched vectors yield `0.0`.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let dot: f64 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| f64::from(*x) * f64::from(*y))
        .sum();
    let norm_a: f64 = a
        .iter()
        .map(|x| f64::from(*x) * f64::from(*x))
        .sum::<f64>()
        .sqrt();
    let norm_b: f64 = b
        .iter()
        .map(|x| f64::from(*x) * f64::from(*x))
        .sum::<f64>()
        .sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    (dot / (norm_a * norm_b)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedding_is_deterministic() {
        let provider = TokenHashEmbedding::new();
        assert_eq!(
            provider.embed("ci build failure"),
            provider.embed("ci build failure")
        );
    }

    #[test]
    fn identical_text_has_cosine_one() {
        let provider = TokenHashEmbedding::new();
        let a = provider.embed("database timeout during deploy");
        let b = provider.embed("database timeout during deploy");
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn dissimilar_text_scores_below_one() {
        let provider = TokenHashEmbedding::new();
        let a = provider.embed("database timeout during deploy");
        let b = provider.embed("frontend stylesheet cache headers");
        let score = cosine_similarity(&a, &b);
        assert!(score < 1.0);
        assert!(score >= 0.0);
    }

    #[test]
    fn empty_text_yields_zero_cosine() {
        let provider = TokenHashEmbedding::new();
        let a = provider.embed("");
        let b = provider.embed("something");
        assert_eq!(cosine_similarity(&a, &b), 0.0);
        assert_eq!(cosine_similarity(&a, &a), 0.0);
    }

    #[test]
    fn custom_dimension_respected() {
        let provider = TokenHashEmbedding::with_dimension(16);
        assert_eq!(provider.embed("hello world").len(), 16);
        assert_eq!(provider.name(), "token-hash-v1");
    }
}
