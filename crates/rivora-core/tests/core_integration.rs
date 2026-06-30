//! Integration: domain IDs, version kinds, and logging behave across the API boundary.

use rivora_core::{
    init_logging, AbilityId, AbilityVersion, ContextId, DeploymentId, IncidentId, LoggingConfig,
    LoggingFormat, ObservationId, OrganizationId, ReceiptId, SchemaVersion, ServiceId,
};
use rivora_errors::ErrorKind;

#[test]
fn named_ids_are_typed_and_serializable() {
    let ids_json = serde_json::json!({
        "observation": ObservationId::new("obs_1").unwrap(),
        "ability": AbilityId::new("ability_1").unwrap(),
        "receipt": ReceiptId::new("receipt_1").unwrap(),
        "service": ServiceId::new("svc_1").unwrap(),
        "deployment": DeploymentId::new("deploy_1").unwrap(),
        "incident": IncidentId::new("inc_1").unwrap(),
        "context": ContextId::new("ctx_1").unwrap(),
        "organization": OrganizationId::new("org_1").unwrap(),
    });
    // Each identifier serializes as a plain string.
    assert_eq!(ids_json["observation"].as_str().unwrap(), "obs_1");
    assert_eq!(ids_json["organization"].as_str().unwrap(), "org_1");
}

#[test]
fn invalid_id_returns_invalid_identifier_kind() {
    let err = ObservationId::new("").unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidIdentifier);
}

#[test]
fn version_kinds_round_trip() {
    let schema = SchemaVersion::parse("1.0.0").unwrap();
    let ability = AbilityVersion::new(0, 2, 1);
    assert_eq!(serde_json::to_string(&schema).unwrap(), "\"1.0.0\"");
    let back: SchemaVersion = serde_json::from_str("\"1.0.0\"").unwrap();
    assert_eq!(back, schema);
    assert_ne!(schema.to_string(), ability.to_string());
}

#[test]
fn invalid_version_returns_invalid_version_kind() {
    let err = SchemaVersion::parse("bad").unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidVersion);
}

#[test]
fn logging_config_round_trips() {
    let cfg = LoggingConfig {
        level: "debug".to_string(),
        format: LoggingFormat::Json,
    };
    let json = serde_json::to_string(&cfg).unwrap();
    let back: LoggingConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(back, cfg);
}

#[test]
fn second_logging_init_is_an_error() {
    let cfg = LoggingConfig::default();
    let _ = init_logging(&cfg);
    let second = init_logging(&cfg);
    assert!(second.is_err(), "a second init must fail");
    assert_eq!(second.unwrap_err().kind(), ErrorKind::Internal);
}
