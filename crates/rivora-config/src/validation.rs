//! Configuration validation.
//!
//! Validation is structural and value-based only. It never contacts an
//! external service.

use crate::Config;
use rivora_errors::RivoraError;

/// Recognized tracing levels (lowercase). Anything else is a configuration
/// error.
const RECOGNIZED_LEVELS: &[&str] = &["trace", "debug", "info", "warn", "error", "off"];

/// Validates a [`Config`].
///
/// # Errors
/// Returns [`RivoraError::InvalidConfig`] if any value fails validation.
pub fn validate(cfg: &Config) -> Result<(), RivoraError> {
    let level = cfg.logging.level.trim().to_lowercase();
    if !RECOGNIZED_LEVELS.contains(&level.as_str()) {
        return Err(RivoraError::InvalidConfig {
            reason: format!(
                "logging.level {:?} is not recognized (expected one of: {})",
                cfg.logging.level,
                RECOGNIZED_LEVELS.join(", ")
            ),
        });
    }

    if let Some(backend) = &cfg.storage.backend {
        // Backend names are provider-scoped (future feature-gated crates); we
        // only require a non-empty name here, which NonEmptyString guarantees.
        let _ = backend;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OrganizationSection, StorageSection};
    use rivora_core::LoggingConfig;

    fn cfg_with_level(level: &str) -> Config {
        Config {
            organization: OrganizationSection::default(),
            storage: StorageSection::default(),
            logging: LoggingConfig {
                level: level.to_string(),
                ..Default::default()
            },
        }
    }

    #[test]
    fn accepts_recognized_levels() {
        for level in ["trace", "DEBUG", "Info", "warn", "error", "off"] {
            assert!(
                validate(&cfg_with_level(level)).is_ok(),
                "level {level} should be valid"
            );
        }
    }

    #[test]
    fn rejects_unknown_level() {
        let err = validate(&cfg_with_level("verbose")).unwrap_err();
        assert_eq!(err.kind(), rivora_errors::ErrorKind::InvalidConfig);
        assert!(err.to_string().contains("verbose"));
    }

    #[test]
    fn rejects_blank_level() {
        assert!(validate(&cfg_with_level("   ")).is_err());
    }
}
