//! Structured logging setup.
//!
//! Open Rivora uses [`tracing`] exclusively (never `println!`). This module
//! installs a structured [`tracing_subscriber`] with the organization's
//! configured level and format, and is the single place where the global
//! subscriber is wired up. Future receipts will consume the same structured
//! spans/fields; secrets are never logged (enforced by callers, not here).
//!
//! The subscriber consults `RUST_LOG` when present and otherwise falls back to
//! the configured level.

use rivora_errors::RivoraError;
use serde::{Deserialize, Serialize};
use tracing_subscriber::EnvFilter;

/// Output format for the structured log subscriber.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoggingFormat {
    /// Human-readable, multi-line (default for terminals).
    #[default]
    Pretty,
    /// Single-line, dense.
    Compact,
    /// Machine-readable JSON (for pipelines and future receipt ingestion).
    Json,
}

/// Logging configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level string (e.g. `info`, `debug`, `warn`). Honors `RUST_LOG` when set.
    #[serde(default = "default_level")]
    pub level: String,
    /// Output format.
    #[serde(default)]
    pub format: LoggingFormat,
}

fn default_level() -> String {
    "info".to_string()
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_level(),
            format: LoggingFormat::Pretty,
        }
    }
}

/// Installs the global tracing subscriber from `config`.
///
/// Calling [`init_logging`] more than once in a process returns an error for
/// the subsequent calls (the global default is already set).
///
/// # Errors
/// Returns [`RivoraError::Internal`] if the subscriber cannot be installed
/// (e.g. a global default already exists, or the level is invalid).
pub fn init_logging(config: &LoggingConfig) -> Result<(), RivoraError> {
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(&config.level))
        .map_err(|e| RivoraError::internal(format!("invalid log filter: {e}")))?;

    let result = match config.format {
        LoggingFormat::Pretty => tracing_subscriber::fmt()
            .pretty()
            .with_env_filter(filter)
            .try_init(),
        LoggingFormat::Compact => tracing_subscriber::fmt()
            .compact()
            .with_env_filter(filter)
            .try_init(),
        LoggingFormat::Json => tracing_subscriber::fmt()
            .json()
            .with_env_filter(filter)
            .try_init(),
    };
    result.map_err(|e| RivoraError::internal(format!("could not install tracing subscriber: {e}")))
}

/// Installs the global tracing subscriber with [`LoggingConfig::default`].
///
/// # Errors
/// See [`init_logging`].
pub fn init_logging_default() -> Result<(), RivoraError> {
    init_logging(&LoggingConfig::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_info_pretty() {
        let cfg = LoggingConfig::default();
        assert_eq!(cfg.level, "info");
        assert_eq!(cfg.format, LoggingFormat::Pretty);
    }

    #[test]
    fn logging_format_serde_is_snake_case() {
        assert_eq!(
            serde_json::to_string(&LoggingFormat::Json).unwrap(),
            "\"json\""
        );
        let back: LoggingFormat = serde_json::from_str("\"compact\"").unwrap();
        assert_eq!(back, LoggingFormat::Compact);
    }

    #[test]
    fn config_round_trips_through_json() {
        let cfg = LoggingConfig {
            level: "debug".to_string(),
            format: LoggingFormat::Json,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: LoggingConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cfg);
    }

    #[test]
    fn config_uses_defaults_when_fields_missing() {
        let back: LoggingConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(back.level, "info");
        assert_eq!(back.format, LoggingFormat::Pretty);
    }
}
