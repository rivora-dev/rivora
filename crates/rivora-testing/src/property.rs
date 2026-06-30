//! Property-test utilities, re-exporting `proptest` with ready-made
//! strategies for the foundational primitives.

use proptest::prelude::*;
use rivora_core::SchemaVersion;
use rivora_types::{IdTag, NonEmptyString, TypedId, Version};

pub use proptest;

/// Strategy generating valid typed identifiers of kind `T`.
pub fn arb_id<T: IdTag>() -> impl Strategy<Value = TypedId<T>> {
    proptest::string::string_regex("[a-z0-9_]{1,20}")
        .expect("valid identifier regex")
        .prop_map(|s| TypedId::<T>::new_unchecked(s))
}

/// Strategy generating semantic versions.
pub fn arb_version() -> impl Strategy<Value = Version> {
    (0u16..50u16, 0u16..50u16, 0u16..50u16)
        .prop_map(|(a, b, c)| Version::new(u64::from(a), u64::from(b), u64::from(c)))
}

/// Strategy generating schema versions.
pub fn arb_schema_version() -> impl Strategy<Value = SchemaVersion> {
    (0u16..50u16, 0u16..50u16, 0u16..50u16)
        .prop_map(|(a, b, c)| SchemaVersion::new(u64::from(a), u64::from(b), u64::from(c)))
}

/// Strategy generating non-empty strings.
pub fn arb_non_empty_string() -> impl Strategy<Value = NonEmptyString> {
    proptest::string::string_regex("[a-z]{1,30}")
        .expect("valid non-empty regex")
        .prop_map(|s| NonEmptyString::new(s).expect("generated string is non-empty"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rivora_core::{SchemaVersion as SV, ServiceId};
    use rivora_types::Version as V;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn arb_ids_round_trip(id in arb_id::<rivora_core::Service>()) {
            prop_assert!(ServiceId::new(id.as_str()).is_ok());
        }

        #[test]
        fn arb_versions_parse(v in arb_version()) {
            prop_assert!(V::parse(v.to_string()).is_ok());
        }

        #[test]
        fn arb_schema_versions_parse(sv in arb_schema_version()) {
            prop_assert!(SV::parse(sv.to_string()).is_ok());
        }

        #[test]
        fn arb_non_empty_strings_are_non_empty(s in arb_non_empty_string()) {
            prop_assert!(!s.as_str().is_empty());
        }
    }
}
