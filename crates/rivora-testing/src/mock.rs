//! Mock utilities, with an emphasis on determinism (docs/05-Adaptive-Engine.md
//! requires non-deterministic sources such as time to be injected and fixed in
//! tests).

use rivora_traits::clock::Clock;

/// A fake clock yielding deterministic ISO-8601-style timestamp strings.
///
/// Use as the injected time source in tests so receipts and observations are
/// reproducible.
pub struct FakeClock {
    mode: ClockMode,
}

enum ClockMode {
    Fixed(String),
    Sequence(u64),
}

impl FakeClock {
    /// Returns a clock that always yields the given timestamp.
    #[must_use]
    pub fn fixed(iso: impl Into<String>) -> Self {
        Self {
            mode: ClockMode::Fixed(iso.into()),
        }
    }

    /// Returns a clock that yields monotonically increasing timestamps
    /// starting at `2026-01-01T00:00:00Z`.
    #[must_use]
    pub fn sequence() -> Self {
        Self {
            mode: ClockMode::Sequence(0),
        }
    }

    /// Returns the current timestamp string and advances the clock for
    /// sequence mode.
    #[must_use]
    pub fn now_iso_internal(&mut self) -> String {
        match &mut self.mode {
            ClockMode::Fixed(s) => s.clone(),
            ClockMode::Sequence(n) => {
                let s = format!("2026-01-01T00:00:{:02}Z", *n % 60);
                *n += 1;
                s
            }
        }
    }
}

impl Default for FakeClock {
    fn default() -> Self {
        Self::sequence()
    }
}

impl Clock for FakeClock {
    fn now_iso(&self) -> String {
        match &self.mode {
            ClockMode::Fixed(s) => s.clone(),
            ClockMode::Sequence(n) => format!("2026-01-01T00:00:{:02}Z", *n % 60),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_clock_is_constant() {
        let mut clock = FakeClock::fixed("2026-06-26T12:00:00Z");
        assert_eq!(clock.now_iso_internal(), "2026-06-26T12:00:00Z");
        assert_eq!(clock.now_iso_internal(), "2026-06-26T12:00:00Z");
    }

    #[test]
    fn sequence_clock_monotonically_increases() {
        let mut clock = FakeClock::sequence();
        let a = clock.now_iso_internal();
        let b = clock.now_iso_internal();
        assert_ne!(a, b);
        assert_eq!(a, "2026-01-01T00:00:00Z");
        assert_eq!(b, "2026-01-01T00:00:01Z");
    }

    #[test]
    fn default_is_sequence() {
        let mut clock = FakeClock::default();
        assert!(clock.now_iso_internal().starts_with("2026-01-01T00:00:"));
    }

    #[test]
    fn fake_clock_implements_clock_trait() {
        let clock = FakeClock::fixed("2026-06-26T12:00:00Z");
        assert_eq!(Clock::now_iso(&clock), "2026-06-26T12:00:00Z");
    }

    #[test]
    fn sequence_clock_trait_does_not_advance() {
        // The trait method is &self, so it returns the same value for Sequence
        // unless the clock is advanced via now_iso_internal.
        let clock = FakeClock::sequence();
        let a = Clock::now_iso(&clock);
        let b = Clock::now_iso(&clock);
        // Both return the same because &self doesn't mutate
        assert_eq!(a, b);
    }
}
