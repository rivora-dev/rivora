use rivora_graph::fixtures;
use rivora_graph::validation::validate_graph;
use rivora_testing::snapshot::rivora_settings;

#[test]
fn golden_empty_graph_json() {
    let g = fixtures::empty_graph();
    let output = serde_json::to_string_pretty(&g).unwrap();
    let settings = rivora_settings();
    settings.bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_single_service_graph_json() {
    let g = fixtures::single_service_graph();
    let output = serde_json::to_string_pretty(&g).unwrap();
    let settings = rivora_settings();
    settings.bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_service_with_repo_graph_json() {
    let g = fixtures::service_with_repo_graph();
    let output = serde_json::to_string_pretty(&g).unwrap();
    let settings = rivora_settings();
    settings.bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_deployment_affecting_service_graph_json() {
    let g = fixtures::deployment_affecting_service_graph();
    let output = serde_json::to_string_pretty(&g).unwrap();
    let settings = rivora_settings();
    settings.bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_incident_explained_by_receipt_graph_json() {
    let g = fixtures::incident_explained_by_receipt_graph();
    let output = serde_json::to_string_pretty(&g).unwrap();
    let settings = rivora_settings();
    settings.bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_ability_generated_receipt_graph_json() {
    let g = fixtures::ability_generated_receipt_graph();
    let output = serde_json::to_string_pretty(&g).unwrap();
    let settings = rivora_settings();
    settings.bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_empty_graph_snapshot() {
    let g = fixtures::empty_graph();
    let snap = g.snapshot();
    let output = serde_json::to_string_pretty(&snap).unwrap();
    let settings = rivora_settings();
    settings.bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_single_service_snapshot() {
    let g = fixtures::single_service_graph();
    let snap = g.snapshot();
    let output = serde_json::to_string_pretty(&snap).unwrap();
    let settings = rivora_settings();
    settings.bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_service_with_repo_snapshot() {
    let g = fixtures::service_with_repo_graph();
    let snap = g.snapshot();
    let output = serde_json::to_string_pretty(&snap).unwrap();
    let settings = rivora_settings();
    settings.bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_deployment_affecting_service_snapshot() {
    let g = fixtures::deployment_affecting_service_graph();
    let snap = g.snapshot();
    let output = serde_json::to_string_pretty(&snap).unwrap();
    let settings = rivora_settings();
    settings.bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_incident_explained_by_receipt_snapshot() {
    let g = fixtures::incident_explained_by_receipt_graph();
    let snap = g.snapshot();
    let output = serde_json::to_string_pretty(&snap).unwrap();
    let settings = rivora_settings();
    settings.bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_ability_generated_receipt_snapshot() {
    let g = fixtures::ability_generated_receipt_graph();
    let snap = g.snapshot();
    let output = serde_json::to_string_pretty(&snap).unwrap();
    let settings = rivora_settings();
    settings.bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_invalid_dangling_edge_validation_error() {
    let g = fixtures::invalid_dangling_edge_graph();
    let err = validate_graph(&g).unwrap_err();
    let output = err.to_string();
    let settings = rivora_settings();
    settings.bind(|| {
        insta::assert_snapshot!(output);
    });
}
