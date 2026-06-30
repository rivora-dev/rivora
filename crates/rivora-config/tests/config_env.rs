//! Environment-override behavior, isolated in its own test binary so that
//! `RIVORA_`-prefixed variables do not leak into other tests.

use rivora_config::{Config, SecretRef};
use rivora_types::NonEmptyString;

#[test]
fn env_overrides_and_secret_resolution() {
    // 1) Env overrides defaults (no file).
    std::env::set_var("RIVORA_ORGANIZATION__NAME", "env-team");
    let cfg = Config::from_env_only().expect("from_env_only with a name override");
    assert_eq!(cfg.organization.name.as_ref().unwrap().as_str(), "env-team");
    std::env::remove_var("RIVORA_ORGANIZATION__NAME");

    // 2) Env override of logging level is honored.
    std::env::set_var("RIVORA_LOGGING__LEVEL", "trace");
    let cfg = Config::from_env_only().expect("from_env_only with trace level");
    assert_eq!(cfg.logging.level, "trace");
    std::env::remove_var("RIVORA_LOGGING__LEVEL");

    // 3) Invalid env level is rejected by validation.
    std::env::set_var("RIVORA_LOGGING__LEVEL", "verbose");
    let res = Config::from_env_only();
    std::env::remove_var("RIVORA_LOGGING__LEVEL");
    assert!(res.is_err());

    // 4) Env override wins over file values.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rivora.toml");
    std::fs::write(&path, "[organization]\nid = \"from-file\"\n").unwrap();
    std::env::set_var("RIVORA_ORGANIZATION__ID", "from-env");
    let cfg = Config::load_from(&path).expect("load_from with env override");
    std::env::remove_var("RIVORA_ORGANIZATION__ID");
    assert_eq!(cfg.organization.id.as_ref().unwrap().as_str(), "from-env");

    // 5) SecretRef::Env resolves a real env var and redacts.
    std::env::set_var("RIVORA_EXAMPLE_SECRET_TOKEN", "tok_12345");
    let r = SecretRef::Env {
        var: NonEmptyString::new("RIVORA_EXAMPLE_SECRET_TOKEN").unwrap(),
    };
    let secret = r.resolve().expect("env secret resolves");
    assert_eq!(secret.expose(), "tok_12345");
    assert_eq!(format!("{secret:?}"), "***");
    std::env::remove_var("RIVORA_EXAMPLE_SECRET_TOKEN");
}
