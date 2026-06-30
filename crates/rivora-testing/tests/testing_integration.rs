//! Integration: testing helpers compose into a realistic test scenario.

use rivora_config::Config;
use rivora_testing::{sample_config_toml, sample_service_id, FakeClock, TempWorkspace};

#[test]
fn helpers_compose() {
    // Typed identifier fixture is valid.
    let id = sample_service_id();
    assert!(rivora_core::ServiceId::new(id.as_str()).is_ok());

    // Temp workspace + config fixture + config loading compose.
    let ws = TempWorkspace::new();
    let path = ws.write_config(sample_config_toml());
    let cfg = Config::load_from(&path).expect("fixture config loads");
    assert_eq!(cfg.organization.id.as_ref().unwrap().as_str(), "org-test");

    // Deterministic clock yields reproducible timestamps.
    let mut clock = FakeClock::fixed("2026-06-26T12:00:00Z");
    assert_eq!(clock.now_iso_internal(), "2026-06-26T12:00:00Z");
    assert_eq!(clock.now_iso_internal(), "2026-06-26T12:00:00Z");
}
