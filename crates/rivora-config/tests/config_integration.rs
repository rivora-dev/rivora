//! Integration: file-based configuration loading and validation (no env).

use rivora_config::Config;
use rivora_errors::ErrorKind;
use std::path::Path;

#[test]
fn from_file_round_trips_a_written_config() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rivora.toml");
    std::fs::write(
        &path,
        r#"
[organization]
id = "org-acme"
name = "Acme"

[storage]
backend = "redb"
path = "./.rivora/store"

[logging]
level = "info"
format = "pretty"
"#,
    )
    .unwrap();

    let cfg = Config::load_from(&path).unwrap();
    assert_eq!(cfg.organization.id.as_ref().unwrap().as_str(), "org-acme");
    assert_eq!(cfg.storage.backend.as_ref().unwrap().as_str(), "redb");
    assert_eq!(cfg.logging.level, "info");
}

#[test]
fn missing_file_is_config_not_found() {
    let err = Config::from_file(Path::new("/nonexistent/path/to/rivora.toml")).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::ConfigNotFound);
}

#[test]
fn empty_toml_yields_defaults_and_validates() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rivora.toml");
    std::fs::write(&path, "").unwrap();
    let cfg = Config::load_from(&path).unwrap();
    assert!(cfg.organization.id.is_none());
    assert_eq!(cfg.logging.level, "info");
}
