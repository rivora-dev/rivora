//! The [`Logger`] trait — abstract structured logging.
//!
//! A logger receives structured log events with a level, message, and
//! key-value fields. Future implementations may wrap `tracing`, but the
//! trait itself has no dependency on any logging framework.
//!
//! # Design principles
//!
//! - **Structured**: log events carry key-value fields, not formatted
//!   strings.
//! - **Portable**: no dependency on `tracing`, `log`, or any other logging
//!   crate.
//! - **Deterministic**: implementations control output, enabling test
//!   assertions on log content.

use serde::{Deserialize, Serialize};

/// The severity level of a log event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    /// Tracing-level detail.
    Trace,
    /// Debug-level information.
    Debug,
    /// Informational messages.
    Info,
    /// Warnings about potential issues.
    Warn,
    /// Errors that prevent normal operation.
    Error,
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trace => write!(f, "trace"),
            Self::Debug => write!(f, "debug"),
            Self::Info => write!(f, "info"),
            Self::Warn => write!(f, "warn"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// A structured log event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogEvent {
    /// The severity level.
    pub level: Level,
    /// The log message.
    pub message: String,
    /// Structured key-value fields.
    pub fields: std::collections::HashMap<String, String>,
}

/// An abstract structured logger.
///
/// # Examples
///
/// ```rust
/// use rivora_traits::logger::{Logger, Level, LogEvent};
/// use std::collections::HashMap;
///
/// struct VecLogger {
///     events: std::sync::Mutex<Vec<LogEvent>>,
/// }
///
/// impl VecLogger {
///     fn new() -> Self {
///         Self { events: std::sync::Mutex::new(vec![]) }
///     }
/// }
///
/// impl Logger for VecLogger {
///     fn log(&self, event: LogEvent) {
///         self.events.lock().unwrap().push(event);
///     }
/// }
///
/// let logger = VecLogger::new();
/// logger.log(LogEvent {
///     level: Level::Info,
///     message: "server started".into(),
///     fields: HashMap::new(),
/// });
/// let events = logger.events.lock().unwrap();
/// assert_eq!(events.len(), 1);
/// assert_eq!(events[0].message, "server started");
/// ```
pub trait Logger: Send + Sync {
    /// Logs a structured event.
    fn log(&self, event: LogEvent);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn level_display() {
        assert_eq!(Level::Trace.to_string(), "trace");
        assert_eq!(Level::Debug.to_string(), "debug");
        assert_eq!(Level::Info.to_string(), "info");
        assert_eq!(Level::Warn.to_string(), "warn");
        assert_eq!(Level::Error.to_string(), "error");
    }

    #[test]
    fn level_ordering() {
        assert!(Level::Trace < Level::Debug);
        assert!(Level::Debug < Level::Info);
        assert!(Level::Info < Level::Warn);
        assert!(Level::Warn < Level::Error);
    }

    #[test]
    fn level_round_trips_through_serde() {
        let json = serde_json::to_string(&Level::Warn).unwrap();
        assert_eq!(json, "\"warn\"");
        let back: Level = serde_json::from_str(&json).unwrap();
        assert_eq!(back, Level::Warn);
    }

    #[test]
    fn log_event_round_trips_through_serde() {
        let mut fields = HashMap::new();
        fields.insert("key".into(), "value".into());
        let event = LogEvent {
            level: Level::Info,
            message: "test message".into(),
            fields,
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: LogEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back, event);
    }
}
