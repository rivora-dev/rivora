//! Golden snapshot support, re-exporting [`insta`] with Rivora-tuned settings.
//!
//! Callers use `rivora_testing::insta::assert_snapshot!` (file snapshots) or
//! `assert_inline_snapshot!` (inline snapshots). [`rivora_settings`] returns
//! a [`insta::Settings`] configured for deterministic output (sorted maps),
//! which callers can bind with `insta::with_settings!`.

pub use insta;

/// Returns [`insta::Settings`] tuned for deterministic Rivora snapshots.
#[must_use]
pub fn rivora_settings() -> insta::Settings {
    let mut settings = insta::Settings::new();
    settings.set_sort_maps(true);
    settings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_snapshot_passes_when_expected_matches() {
        let value = "Hello, Rivora!";
        // Inline snapshot (`@` form): no external `.snap` file is required.
        insta::assert_snapshot!(value, @"Hello, Rivora!");
    }

    #[test]
    fn rivora_settings_compiles_and_returns() {
        let _settings = rivora_settings();
    }
}
