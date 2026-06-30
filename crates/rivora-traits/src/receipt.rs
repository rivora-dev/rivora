//! The [`ReceiptRenderer`] trait — renders reliability receipts.
//!
//! A receipt renderer converts a serialized receipt into a human-readable
//! format (JSON, Markdown, or future HTML). It makes no assumptions about
//! the output format — each implementation decides how to present the data.
//!
//! # Design principles
//!
//! - **Format-agnostic**: the trait accepts a JSON receipt and returns a
//!   formatted string. The renderer decides the presentation.
//! - **Portable**: no formatting library dependencies; implementations may
//!   use any approach (markdown, HTML, plain text).
//! - **Extensible**: new formats are added by implementing the trait, not
//!   by modifying existing code.

use serde::{Deserialize, Serialize};

/// The output format of a rendered receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderFormat {
    /// JavaScript Object Notation — machine-readable.
    Json,
    /// GitHub-Flavored Markdown — human-readable in terminals and PRs.
    Markdown,
    /// HTML — for browser-based rendering.
    Html,
}

impl std::fmt::Display for RenderFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json => write!(f, "json"),
            Self::Markdown => write!(f, "markdown"),
            Self::Html => write!(f, "html"),
        }
    }
}

/// A renderer that converts serialized receipts into human-readable formats.
///
/// # Examples
///
/// ```rust
/// use rivora_traits::receipt::{ReceiptRenderer, RenderFormat};
///
/// struct JsonRenderer;
///
/// impl ReceiptRenderer for JsonRenderer {
///     fn render(&self, receipt: &serde_json::Value, _format: RenderFormat) -> String {
///         serde_json::to_string_pretty(receipt).unwrap()
///     }
///
///     fn supported_formats(&self) -> Vec<RenderFormat> {
///         vec![RenderFormat::Json]
///     }
/// }
///
/// let renderer = JsonRenderer;
/// let receipt = serde_json::json!({"id": "r1", "kind": "recommendation"});
/// let output = renderer.render(&receipt, RenderFormat::Json);
/// assert!(output.contains("r1"));
/// assert!(renderer.supported_formats().contains(&RenderFormat::Json));
/// ```
pub trait ReceiptRenderer: Send + Sync {
    /// Renders a serialized receipt into the given format.
    ///
    /// The `receipt` is a JSON value containing the receipt data. The
    /// renderer should produce a string representation appropriate for the
    /// requested format.
    ///
    /// # Errors
    ///
    /// Returns an error string if the format is not supported or rendering
    /// fails. Implementations should prefer returning an error over panicking.
    fn render(&self, receipt: &serde_json::Value, format: RenderFormat) -> String;

    /// Returns the list of formats this renderer supports.
    fn supported_formats(&self) -> Vec<RenderFormat>;

    /// Returns `true` if this renderer supports the given format.
    #[must_use]
    fn supports(&self, format: RenderFormat) -> bool {
        self.supported_formats().contains(&format)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_format_display() {
        assert_eq!(RenderFormat::Json.to_string(), "json");
        assert_eq!(RenderFormat::Markdown.to_string(), "markdown");
        assert_eq!(RenderFormat::Html.to_string(), "html");
    }

    #[test]
    fn render_format_round_trips_through_serde() {
        let json = serde_json::to_string(&RenderFormat::Markdown).unwrap();
        assert_eq!(json, "\"markdown\"");
        let back: RenderFormat = serde_json::from_str(&json).unwrap();
        assert_eq!(back, RenderFormat::Markdown);
    }

    #[test]
    fn default_supports_checks_list() {
        struct OnlyJson;
        impl ReceiptRenderer for OnlyJson {
            fn render(&self, receipt: &serde_json::Value, _format: RenderFormat) -> String {
                receipt.to_string()
            }
            fn supported_formats(&self) -> Vec<RenderFormat> {
                vec![RenderFormat::Json]
            }
        }

        let r = OnlyJson;
        assert!(r.supports(RenderFormat::Json));
        assert!(!r.supports(RenderFormat::Markdown));
        assert!(!r.supports(RenderFormat::Html));
    }
}
