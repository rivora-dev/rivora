use rivora_memory::fixtures;
use rivora_memory::validation::validate_record;
use rivora_testing::snapshot::rivora_settings;

#[test]
fn golden_organization_fact_json() {
    let record = fixtures::organization_fact();
    let output = serde_json::to_string_pretty(&record).unwrap();
    let settings = rivora_settings();
    settings.bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_service_relationship_memory_json() {
    let record = fixtures::service_relationship_memory();
    let output = serde_json::to_string_pretty(&record).unwrap();
    let settings = rivora_settings();
    settings.bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_incident_learning_memory_json() {
    let record = fixtures::incident_learning_memory();
    let output = serde_json::to_string_pretty(&record).unwrap();
    let settings = rivora_settings();
    settings.bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_deployment_learning_memory_json() {
    let record = fixtures::deployment_learning_memory();
    let output = serde_json::to_string_pretty(&record).unwrap();
    let settings = rivora_settings();
    settings.bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_receipt_learning_memory_json() {
    let record = fixtures::receipt_learning_memory();
    let output = serde_json::to_string_pretty(&record).unwrap();
    let settings = rivora_settings();
    settings.bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_ability_learning_memory_json() {
    let record = fixtures::ability_learning_memory();
    let output = serde_json::to_string_pretty(&record).unwrap();
    let settings = rivora_settings();
    settings.bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_expired_memory_json() {
    let record = fixtures::expired_memory();
    let output = serde_json::to_string_pretty(&record).unwrap();
    let settings = rivora_settings();
    settings.bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_superseded_memory_json() {
    let record = fixtures::superseded_memory();
    let output = serde_json::to_string_pretty(&record).unwrap();
    let settings = rivora_settings();
    settings.bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_sample_index_json() {
    let index = fixtures::sample_index();
    let output = serde_json::to_string_pretty(&index).unwrap();
    let settings = rivora_settings();
    settings.bind(|| {
        insta::assert_snapshot!(output);
    });
}

#[test]
fn golden_invalid_memory_validation_error() {
    let record = fixtures::invalid_memory();
    let err = validate_record(&record).unwrap_err();
    let output = err.to_string();
    let settings = rivora_settings();
    settings.bind(|| {
        insta::assert_snapshot!(output);
    });
}
