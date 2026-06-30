//! The [`Clock`] trait — abstract time source.
//!
//! A clock provides ISO-8601 timestamps for observations, snapshots, and
//! receipts. Abstracting time enables deterministic testing where timestamps
//! are fixed or sequenced.
//!
//! # Design principles
//!
//! - **Deterministic**: implementations control the time source, enabling
//!   reproducible tests.
//! - **Simple**: returns ISO-8601 strings, not `SystemTime` or `Instant`.
//! - **Portable**: no platform-specific time APIs.

/// An abstract time source that yields ISO-8601 timestamps.
///
/// # Examples
///
/// ```rust
/// use rivora_traits::clock::Clock;
///
/// struct FixedClock(String);
///
/// impl Clock for FixedClock {
///     fn now_iso(&self) -> String {
///         self.0.clone()
///     }
/// }
///
/// let clock = FixedClock("2026-06-26T12:00:00Z".into());
/// assert_eq!(clock.now_iso(), "2026-06-26T12:00:00Z");
/// ```
pub trait Clock: Send + Sync {
    /// Returns the current time as an ISO-8601 string.
    fn now_iso(&self) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SequenceClock {
        counter: std::sync::atomic::AtomicU64,
    }

    impl Clock for SequenceClock {
        fn now_iso(&self) -> String {
            let n = self
                .counter
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            format!("2026-01-01T00:00:{n:02}Z")
        }
    }

    #[test]
    fn fixed_clock_returns_constant() {
        let clock = FixedClock("2026-06-26T12:00:00Z".into());
        assert_eq!(clock.now_iso(), "2026-06-26T12:00:00Z");
        assert_eq!(clock.now_iso(), "2026-06-26T12:00:00Z");
    }

    #[test]
    fn sequence_clock_advances() {
        let clock = SequenceClock {
            counter: std::sync::atomic::AtomicU64::new(0),
        };
        let a = clock.now_iso();
        let b = clock.now_iso();
        assert_ne!(a, b);
        assert_eq!(a, "2026-01-01T00:00:00Z");
        assert_eq!(b, "2026-01-01T00:00:01Z");
    }

    struct FixedClock(String);

    impl Clock for FixedClock {
        fn now_iso(&self) -> String {
            self.0.clone()
        }
    }
}
