//! Golden snapshot tests for the `rivora-receipts` crate.
//!
//! Each fixture receipt is rendered to JSON (via [`JsonRenderer`]) and to
//! GitHub-Flavored Markdown (via [`MarkdownRenderer`]), and the rendered output
//! is captured as an [`insta`] snapshot. The invalid fixture's validation
//! error message is also snapshotted.
//!
//! Snapshots are auto-created on the first run. Run `cargo test` to generate
//! them, then `cargo insta review` (or set `INSTA_UPDATE=always`) to accept.
//!
//! Rivora-tuned [`insta`] settings (sorted maps for deterministic output) are
//! bound via [`rivora_testing::rivora_settings`] with [`insta::Settings::bind`].
//! `assert_snapshot!` is invoked directly inside each test (inside the `bind`
//! closure) so that the snapshot file name is derived from the test function.

use rivora_receipts::fixtures;
use rivora_receipts::renderers::{JsonRenderer, MarkdownRenderer};
use rivora_receipts::validation::validate_receipt;
use rivora_receipts::Receipt;
use rivora_testing::snapshot::rivora_settings;
use rivora_traits::receipt::{ReceiptRenderer, RenderFormat};

/// Serializes a receipt to a [`serde_json::Value`] for rendering.
fn to_value(receipt: &Receipt) -> serde_json::Value {
    serde_json::to_value(receipt).unwrap()
}

/// Renders a receipt to pretty-printed JSON via [`JsonRenderer`].
fn render_json(receipt: &Receipt) -> String {
    let value = to_value(receipt);
    JsonRenderer::new().render(&value, RenderFormat::Json)
}

/// Renders a receipt to GitHub-Flavored Markdown via [`MarkdownRenderer`].
fn render_markdown(receipt: &Receipt) -> String {
    let value = to_value(receipt);
    MarkdownRenderer::new().render(&value, RenderFormat::Markdown)
}

// ---------------------------------------------------------------------------
// JSON golden snapshots
// ---------------------------------------------------------------------------

#[test]
fn golden_observation_receipt_json() {
    let output = render_json(&fixtures::observation_receipt());
    rivora_settings().bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_incident_explanation_receipt_json() {
    let output = render_json(&fixtures::incident_explanation_receipt());
    rivora_settings().bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_deployment_review_receipt_json() {
    let output = render_json(&fixtures::deployment_review_receipt());
    rivora_settings().bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_recommendation_receipt_json() {
    let output = render_json(&fixtures::recommendation_receipt());
    rivora_settings().bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_ability_run_receipt_json() {
    let output = render_json(&fixtures::ability_run_receipt());
    rivora_settings().bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_memory_candidate_created_receipt_json() {
    let output = render_json(&fixtures::memory_candidate_created_receipt());
    rivora_settings().bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_memory_approved_receipt_json() {
    let output = render_json(&fixtures::memory_approved_receipt());
    rivora_settings().bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_recall_result_receipt_json() {
    let output = render_json(&fixtures::recall_result_receipt());
    rivora_settings().bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_human_feedback_recorded_receipt_json() {
    let output = render_json(&fixtures::human_feedback_recorded_receipt());
    rivora_settings().bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_invalid_receipt_json() {
    let output = render_json(&fixtures::invalid_receipt());
    rivora_settings().bind(|| {
        insta::assert_snapshot!(output);
    });
}

// ---------------------------------------------------------------------------
// Markdown golden snapshots
// ---------------------------------------------------------------------------

#[test]
fn golden_observation_receipt_markdown() {
    let output = render_markdown(&fixtures::observation_receipt());
    rivora_settings().bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_incident_explanation_receipt_markdown() {
    let output = render_markdown(&fixtures::incident_explanation_receipt());
    rivora_settings().bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_deployment_review_receipt_markdown() {
    let output = render_markdown(&fixtures::deployment_review_receipt());
    rivora_settings().bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_recommendation_receipt_markdown() {
    let output = render_markdown(&fixtures::recommendation_receipt());
    rivora_settings().bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_ability_run_receipt_markdown() {
    let output = render_markdown(&fixtures::ability_run_receipt());
    rivora_settings().bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_memory_candidate_created_receipt_markdown() {
    let output = render_markdown(&fixtures::memory_candidate_created_receipt());
    rivora_settings().bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_memory_approved_receipt_markdown() {
    let output = render_markdown(&fixtures::memory_approved_receipt());
    rivora_settings().bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_recall_result_receipt_markdown() {
    let output = render_markdown(&fixtures::recall_result_receipt());
    rivora_settings().bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_human_feedback_recorded_receipt_markdown() {
    let output = render_markdown(&fixtures::human_feedback_recorded_receipt());
    rivora_settings().bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_invalid_receipt_markdown() {
    let output = render_markdown(&fixtures::invalid_receipt());
    rivora_settings().bind(|| {
        insta::assert_snapshot!(output);
    });
}

// ---------------------------------------------------------------------------
// Validation error golden snapshot
// ---------------------------------------------------------------------------

#[test]
fn golden_invalid_receipt_validation_error() {
    let receipt = fixtures::invalid_receipt();
    let err = validate_receipt(&receipt).unwrap_err();
    rivora_settings().bind(|| {
        insta::assert_snapshot!(err.to_string());
    });
}
