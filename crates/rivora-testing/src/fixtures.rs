//! Reusable fixtures for tests across the workspace.

use rivora_core::{OrganizationId, SchemaVersion, ServiceId};
use rivora_types::{IdTag, TypedId, Version};

/// A fresh, random typed identifier of the requested kind.
#[must_use]
pub fn sample_id<T: IdTag>() -> TypedId<T> {
    TypedId::<T>::new_random()
}

/// A simple `1.0.0` version.
#[must_use]
pub fn sample_version() -> Version {
    Version::new(1, 0, 0)
}

/// A simple `1.0.0` schema version.
#[must_use]
pub fn sample_schema_version() -> SchemaVersion {
    SchemaVersion::new(1, 0, 0)
}

/// A fresh, random organization identifier.
#[must_use]
pub fn sample_organization_id() -> OrganizationId {
    OrganizationId::new_random()
}

/// A fresh, random service identifier.
#[must_use]
pub fn sample_service_id() -> ServiceId {
    ServiceId::new_random()
}

/// A minimal, valid `rivora.toml` fixture.
#[must_use]
pub fn sample_config_toml() -> &'static str {
    r#"
[organization]
id = "org-test"
name = "Test Org"

[storage]
backend = "redb"
path = "./.rivora/store"

[logging]
level = "info"
format = "pretty"
"#
}

#[cfg(test)]
mod tests {
    use super::*;
    use rivora_config::Config;

    #[test]
    fn sample_ids_are_valid() {
        assert!(ServiceId::new(sample_service_id().as_str()).is_ok());
        assert!(OrganizationId::new(sample_organization_id().as_str()).is_ok());
    }

    #[test]
    fn sample_version_is_1_0_0() {
        assert_eq!(sample_version().to_string(), "1.0.0");
        assert_eq!(sample_schema_version().to_string(), "1.0.0");
    }

    #[test]
    fn sample_config_toml_parses() {
        assert!(Config::from_toml_str(sample_config_toml()).is_ok());
    }
}
