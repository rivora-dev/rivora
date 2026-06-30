//! Receipt renderers that implement the [`ReceiptRenderer`] trait.
//!
//! The [`JsonRenderer`] renders receipts as pretty-printed JSON, while the
//! [`MarkdownRenderer`] renders receipts as GitHub-Flavored Markdown with
//! all sections: summary, subject, confidence, evidence, reasoning, risks,
//! suggested actions, and provenance.

use rivora_traits::receipt::{ReceiptRenderer, RenderFormat};

use crate::Receipt;

/// Renders a receipt as pretty-printed JSON.
#[derive(Debug, Clone, Default)]
pub struct JsonRenderer;

impl JsonRenderer {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl ReceiptRenderer for JsonRenderer {
    fn render(&self, receipt: &serde_json::Value, _format: RenderFormat) -> String {
        serde_json::to_string_pretty(receipt).unwrap_or_else(|_| "{}".to_string())
    }

    fn supported_formats(&self) -> Vec<RenderFormat> {
        vec![RenderFormat::Json]
    }
}

/// Renders a receipt as GitHub-Flavored Markdown.
#[derive(Debug, Clone, Default)]
pub struct MarkdownRenderer;

impl MarkdownRenderer {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl ReceiptRenderer for MarkdownRenderer {
    fn render(&self, receipt: &serde_json::Value, _format: RenderFormat) -> String {
        render_markdown_from_value(receipt)
    }

    fn supported_formats(&self) -> Vec<RenderFormat> {
        vec![RenderFormat::Markdown]
    }
}

fn render_markdown_from_value(receipt: &serde_json::Value) -> String {
    let mut out = String::new();

    let kind = get_str(receipt, "kind", "unknown");
    let status = get_str(receipt, "status", "unknown");
    let id = get_str(receipt, "id", "unknown");

    let title = receipt
        .get("summary")
        .and_then(|s| s.get("title"))
        .and_then(|v| v.as_str())
        .unwrap_or("Untitled receipt");
    let description = receipt
        .get("summary")
        .and_then(|s| s.get("description"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    out.push_str(&format!("## {kind}: {title}\n\n"));
    out.push_str(&format!("- **ID:** `{id}`\n"));
    out.push_str(&format!("- **Status:** {status}\n"));

    if let Some(subject) = receipt.get("subject") {
        let s_kind = get_str(subject, "kind", "");
        let s_ref = get_str(subject, "reference", "");
        let s_name = get_str(subject, "display_name", "");
        out.push_str(&format!("- **Subject:** {s_name} ({s_kind}: {s_ref})\n"));
    }

    if !description.is_empty() {
        out.push_str(&format!("\n{description}\n"));
    }

    if let Some(confidence) = receipt.get("confidence") {
        out.push_str("\n### Confidence\n\n");
        let score = confidence
            .get("score")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let level = get_str(confidence, "level", "unknown");
        let method = get_str(confidence, "method", "unknown");
        let uncertainty = get_str(confidence, "uncertainty", "");
        out.push_str(&format!("- **Score:** {score:.2} ({level})\n"));
        out.push_str(&format!("- **Method:** {method}\n"));
        if !uncertainty.is_empty() {
            out.push_str(&format!("- **Uncertainty:** {uncertainty}\n"));
        }
        if let Some(factors) = confidence
            .get("contributing_factors")
            .and_then(|v| v.as_array())
        {
            if !factors.is_empty() {
                out.push_str("- **Contributing factors:**\n");
                for f in factors {
                    if let Some(s) = f.as_str() {
                        out.push_str(&format!("  - {s}\n"));
                    }
                }
            }
        }
        if let Some(factors) = confidence
            .get("limiting_factors")
            .and_then(|v| v.as_array())
        {
            if !factors.is_empty() {
                out.push_str("- **Limiting factors:**\n");
                for f in factors {
                    if let Some(s) = f.as_str() {
                        out.push_str(&format!("  - {s}\n"));
                    }
                }
            }
        }
    }

    if let Some(evidence) = receipt.get("evidence").and_then(|v| v.as_array()) {
        if !evidence.is_empty() {
            out.push_str("\n### Evidence\n\n");
            for (i, ev) in evidence.iter().enumerate() {
                let title = get_str(ev, "title", "Untitled");
                let kind = get_str(ev, "kind", "unknown");
                let observed_at = get_str(ev, "observed_at", "");
                let desc = get_str(ev, "description", "");
                out.push_str(&format!("{}. **{title}** ({kind})\n", i + 1));
                if !observed_at.is_empty() {
                    out.push_str(&format!("   - Observed: {observed_at}\n"));
                }
                if !desc.is_empty() {
                    out.push_str(&format!("   - {desc}\n"));
                }
            }
        }
    }

    if let Some(reasoning) = receipt.get("reasoning").and_then(|v| v.as_array()) {
        if !reasoning.is_empty() {
            out.push_str("\n### Reasoning\n\n");
            for step in reasoning {
                let num = step.get("step").and_then(|v| v.as_u64()).unwrap_or(0);
                let title = get_str(step, "title", "");
                let explanation = get_str(step, "explanation", "");
                let conclusion = get_str(step, "output_conclusion", "");
                out.push_str(&format!("{num}. **{title}**\n"));
                if !explanation.is_empty() {
                    out.push_str(&format!("   - {explanation}\n"));
                }
                if !conclusion.is_empty() {
                    out.push_str(&format!("   - Conclusion: {conclusion}\n"));
                }
            }
        }
    }

    if let Some(risk) = receipt.get("risk") {
        out.push_str("\n### Risk\n\n");
        let level = get_str(risk, "level", "unknown");
        let desc = get_str(risk, "description", "");
        out.push_str(&format!("- **Level:** {level}\n"));
        if !desc.is_empty() {
            out.push_str(&format!("- **Description:** {desc}\n"));
        }
        if let Some(services) = risk.get("affected_services").and_then(|v| v.as_array()) {
            if !services.is_empty() {
                out.push_str("- **Affected services:**\n");
                for s in services {
                    if let Some(s) = s.as_str() {
                        out.push_str(&format!("  - {s}\n"));
                    }
                }
            }
        }
        if let Some(impacts) = risk.get("possible_impacts").and_then(|v| v.as_array()) {
            if !impacts.is_empty() {
                out.push_str("- **Possible impacts:**\n");
                for s in impacts {
                    if let Some(s) = s.as_str() {
                        out.push_str(&format!("  - {s}\n"));
                    }
                }
            }
        }
        if let Some(mitigations) = risk.get("mitigations").and_then(|v| v.as_array()) {
            if !mitigations.is_empty() {
                out.push_str("- **Mitigations:**\n");
                for s in mitigations {
                    if let Some(s) = s.as_str() {
                        out.push_str(&format!("  - {s}\n"));
                    }
                }
            }
        }
    }

    if let Some(actions) = receipt.get("suggested_actions").and_then(|v| v.as_array()) {
        if !actions.is_empty() {
            out.push_str("\n### Suggested Actions\n\n");
            for (i, action) in actions.iter().enumerate() {
                let title = get_str(action, "title", "Untitled");
                let kind = get_str(action, "kind", "unknown");
                let approval = get_str(action, "approval", "unknown");
                let read_only = action
                    .get("read_only")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                let mutates = action
                    .get("mutates_infrastructure")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let outcome = get_str(action, "expected_outcome", "");
                let rollback = get_str(action, "rollback_strategy", "");

                out.push_str(&format!("{}. **{title}** ({kind})\n", i + 1));
                if !outcome.is_empty() {
                    out.push_str(&format!("   - Expected outcome: {outcome}\n"));
                }
                out.push_str(&format!("   - Approval: {approval}\n"));
                out.push_str(&format!("   - Read-only: {read_only}\n"));
                out.push_str(&format!("   - Mutates infrastructure: {mutates}\n"));
                if !rollback.is_empty() {
                    out.push_str(&format!("   - Rollback strategy: {rollback}\n"));
                }
            }
        }
    }

    if let Some(provenance) = receipt.get("provenance") {
        out.push_str("\n### Provenance\n\n");
        let source = get_str(provenance, "source", "unknown");
        let source_version = get_str(provenance, "source_version", "");
        out.push_str(&format!("- **Source:** {source} {source_version}\n"));
        if let Some(inf) = provenance.get("inference").filter(|v| !v.is_null()) {
            let provider = get_str(inf, "provider", "");
            let model = get_str(inf, "model", "");
            let temp = inf
                .get("temperature")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            if !provider.is_empty() {
                out.push_str(&format!(
                    "- **Inference:** {provider} / {model} (temp={temp})\n"
                ));
            }
        }
        if let Some(ability) = provenance.get("ability").filter(|v| !v.is_null()) {
            let id = get_str(ability, "id", "");
            let version = get_str(ability, "version", "");
            let status = get_str(ability, "status", "");
            if !id.is_empty() {
                out.push_str(&format!("- **Ability:** {id} v{version} ({status})\n"));
            }
        }
    }

    if let Some(timestamps) = receipt.get("timestamps") {
        let created = get_str(timestamps, "created_at", "");
        out.push_str(&format!("\n---\n\n*Created: {created}*\n"));
    }
    if let Some(version) = receipt.get("version") {
        let schema = version
            .get("schema")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        out.push_str(&format!("*Schema version: {schema}*\n"));
    }

    out
}

fn get_str(value: &serde_json::Value, field: &str, default: &str) -> String {
    value
        .get(field)
        .and_then(|v| v.as_str())
        .unwrap_or(default)
        .to_string()
}

impl Receipt {
    /// Serializes the receipt to pretty-printed JSON.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Renders the receipt to Markdown directly from the typed struct.
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let value = serde_json::to_value(self).unwrap_or(serde_json::Value::Null);
        render_markdown_from_value(&value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    #[test]
    fn json_renderer_supports_json() {
        let r = JsonRenderer::new();
        assert!(r.supports(RenderFormat::Json));
        assert!(!r.supports(RenderFormat::Markdown));
    }

    #[test]
    fn json_renderer_outputs_pretty_json() {
        let receipt = serde_json::json!({"id": "r1", "kind": "observation"});
        let output = JsonRenderer::new().render(&receipt, RenderFormat::Json);
        assert!(output.contains("\"id\""));
        assert!(output.contains("r1"));
    }

    #[test]
    fn markdown_renderer_supports_markdown() {
        let r = MarkdownRenderer::new();
        assert!(r.supports(RenderFormat::Markdown));
        assert!(!r.supports(RenderFormat::Json));
    }

    #[test]
    fn markdown_renders_all_sections() {
        let receipt = fixtures::recommendation_receipt();
        let value = serde_json::to_value(&receipt).unwrap();
        let output = MarkdownRenderer::new().render(&value, RenderFormat::Markdown);

        assert!(
            output.contains("### Confidence"),
            "missing confidence section"
        );
        assert!(output.contains("### Evidence"), "missing evidence section");
        assert!(
            output.contains("### Reasoning"),
            "missing reasoning section"
        );
        assert!(output.contains("### Risk"), "missing risk section");
        assert!(
            output.contains("### Suggested Actions"),
            "missing suggested actions section"
        );
        assert!(
            output.contains("### Provenance"),
            "missing provenance section"
        );
    }

    #[test]
    fn receipt_to_json_produces_valid_json() {
        let receipt = fixtures::observation_receipt();
        let json = receipt.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["kind"], "observation");
    }

    #[test]
    fn receipt_to_markdown_contains_title() {
        let receipt = fixtures::incident_explanation_receipt();
        let md = receipt.to_markdown();
        assert!(md.contains("### Confidence"));
        assert!(md.contains("### Evidence"));
    }

    #[test]
    fn markdown_shows_approval_for_mutating_actions() {
        let receipt = fixtures::recommendation_receipt();
        let value = serde_json::to_value(&receipt).unwrap();
        let output = MarkdownRenderer::new().render(&value, RenderFormat::Markdown);
        assert!(output.contains("required"));
        assert!(output.contains("Mutates infrastructure: true"));
    }

    #[test]
    fn markdown_omits_inference_line_when_none() {
        let receipt = fixtures::observation_receipt();
        let md = receipt.to_markdown();
        assert!(
            !md.contains("**Inference:**"),
            "observation receipt has no inference ref, should not render inference line"
        );
    }

    #[test]
    fn markdown_omits_ability_line_when_none() {
        let receipt = fixtures::observation_receipt();
        let md = receipt.to_markdown();
        assert!(
            !md.contains("**Ability:**"),
            "observation receipt has no ability ref, should not render ability line"
        );
    }

    #[test]
    fn markdown_shows_inference_line_when_present() {
        let receipt = fixtures::recommendation_receipt();
        let md = receipt.to_markdown();
        assert!(
            md.contains("**Inference:**"),
            "recommendation receipt has inference ref, should render inference line"
        );
    }

    #[test]
    fn markdown_shows_ability_line_when_present() {
        let receipt = fixtures::deployment_review_receipt();
        let md = receipt.to_markdown();
        assert!(
            md.contains("**Ability:**"),
            "deployment review receipt has ability ref, should render ability line"
        );
    }

    #[test]
    fn markdown_renders_memory_candidate_created_receipt() {
        let receipt = fixtures::memory_candidate_created_receipt();
        let md = receipt.to_markdown();
        assert!(md.contains("memory_candidate_created"));
        assert!(md.contains("### Confidence"));
        assert!(md.contains("### Evidence"));
    }
}
