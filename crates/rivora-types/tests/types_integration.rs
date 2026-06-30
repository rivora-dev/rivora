//! Integration: typed primitives behave the same across the public API boundary.

use rivora_errors::ErrorKind;
use rivora_types::{NonEmptyString, TypedId, Version};

enum Service {}
impl rivora_types::IdTag for Service {
    const KIND: &'static str = "service";
}
type ServiceId = TypedId<Service>;

#[test]
fn typed_id_validation_surfaces_error_kind() {
    let err = ServiceId::new("").unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidIdentifier);
}

#[test]
fn version_and_text_serialize_as_strings() {
    let v = Version::new(1, 0, 0);
    let n = NonEmptyString::new("payments").unwrap();
    assert_eq!(serde_json::to_string(&v).unwrap(), "\"1.0.0\"");
    assert_eq!(serde_json::to_string(&n).unwrap(), "\"payments\"");
}

#[test]
fn round_trip_through_json_value() {
    let id = ServiceId::new("svc-api").unwrap();
    let v = serde_json::to_value(&id).unwrap();
    let back: ServiceId = serde_json::from_value(v).unwrap();
    assert_eq!(back, id);
}
