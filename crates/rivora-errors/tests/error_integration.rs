//! Integration test: errors serialize consistently and integrate with std::error::Error.

use rivora_errors::{ErrorKind, RivoraError};
use serde_json::json;

#[test]
fn error_implements_std_error() {
    let e: Box<dyn std::error::Error> =
        Box::new(RivoraError::invalid_identifier("receipt", "empty"));
    assert!(e.to_string().contains("receipt"));
}

#[test]
fn serialized_error_has_stable_shape() {
    let e = RivoraError::invalid_version("not-a-version", "unrecognized");
    let v = serde_json::to_value(&e).unwrap();
    assert_eq!(
        v,
        json!({
            "kind": "invalid_version",
            "message": e.to_string()
        })
    );
}

#[test]
fn kind_roundtrips_via_json_tag() {
    let e = RivoraError::invalid_value("organization.name", "must not be empty");
    let tag = serde_json::to_value(e.kind()).unwrap();
    let parsed: ErrorKind = serde_json::from_value(tag).unwrap();
    assert_eq!(parsed, e.kind());
}
