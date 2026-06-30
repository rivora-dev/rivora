//! The [`IdGenerator`] trait — abstract identifier generation.
//!
//! An ID generator produces unique identifiers for observations, receipts,
//! and other domain entities. Abstracting ID generation enables deterministic
//! testing where IDs are predictable.
//!
//! # Design principles
//!
//! - **Deterministic**: implementations control ID generation, enabling
//!   reproducible tests.
//! - **Simple**: returns `String` IDs. Typed wrappers are applied by the
//!   caller.
//! - **Portable**: no platform-specific UUID APIs required.

/// An abstract generator of unique identifiers.
///
/// # Examples
///
/// ```rust
/// use rivora_traits::idgen::IdGenerator;
///
/// struct CountingIdGen {
///     counter: std::sync::atomic::AtomicU64,
/// }
///
/// impl IdGenerator for CountingIdGen {
///     fn generate(&self) -> String {
///         let n = self.counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
///         format!("id-{n}")
///     }
/// }
///
/// let gen = CountingIdGen {
///     counter: std::sync::atomic::AtomicU64::new(0),
/// };
/// assert_eq!(gen.generate(), "id-0");
/// assert_eq!(gen.generate(), "id-1");
/// ```
pub trait IdGenerator: Send + Sync {
    /// Generates a new unique identifier.
    fn generate(&self) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct CountingIdGen {
        counter: std::sync::atomic::AtomicU64,
    }

    impl IdGenerator for CountingIdGen {
        fn generate(&self) -> String {
            let n = self
                .counter
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            format!("id-{n}")
        }
    }

    #[test]
    fn counting_gen_produces_sequential_ids() {
        let gen = CountingIdGen {
            counter: std::sync::atomic::AtomicU64::new(0),
        };
        assert_eq!(gen.generate(), "id-0");
        assert_eq!(gen.generate(), "id-1");
        assert_eq!(gen.generate(), "id-2");
    }

    #[test]
    fn ids_are_unique() {
        let gen = CountingIdGen {
            counter: std::sync::atomic::AtomicU64::new(0),
        };
        let mut ids = std::collections::HashSet::new();
        for _ in 0..100 {
            ids.insert(gen.generate());
        }
        assert_eq!(ids.len(), 100);
    }
}
