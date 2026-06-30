//! Configuration loading: layered defaults < file < environment.
//!
//! File format is TOML (`rivora.toml`), per docs/14-Infrastructure.md.
//! Environment overrides use the `RIVORA_` prefix with `__` as the nesting
//! separator, e.g. `RIVORA_LOGGING__LEVEL=debug` sets `logging.level`.

use std::path::{Path, PathBuf};

use crate::validation;
use crate::Config;
use rivora_errors::RivoraError;
use serde_json::Value;

impl Config {
    /// Parses a TOML string into a [`Config`]. Does not apply env overrides.
    ///
    /// # Errors
    /// Returns [`RivoraError::ConfigLoad`] if the TOML is invalid or fails to
    /// deserialize.
    pub fn from_toml_str(s: &str) -> Result<Config, RivoraError> {
        toml::from_str(s).map_err(|e| RivoraError::ConfigLoad {
            reason: e.to_string(),
        })
    }

    /// Reads and parses a TOML file. Does not apply env overrides.
    ///
    /// # Errors
    /// Returns [`RivoraError::ConfigNotFound`] if the file is missing, and
    /// [`RivoraError::Io`] / [`RivoraError::ConfigLoad`] for other failures.
    pub fn from_file(path: &Path) -> Result<Config, RivoraError> {
        let contents = std::fs::read_to_string(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                RivoraError::ConfigNotFound {
                    path: path.display().to_string(),
                }
            } else {
                RivoraError::Io(e)
            }
        })?;
        Self::from_toml_str(&contents)
    }

    /// Loads configuration from an explicit file path, then layers environment
    /// overrides on top, then validates.
    ///
    /// # Errors
    /// See [`Config::from_file`] and [`validation::validate`].
    pub fn load_from(path: &Path) -> Result<Config, RivoraError> {
        let cfg = Self::from_file(path)?;
        Self::finalize(cfg)
    }

    /// Loads configuration with automatic file discovery, then layers
    /// environment overrides, then validates.
    ///
    /// Discovery order:
    /// 1. `RIVORA_CONFIG` environment variable, if set.
    /// 2. `./rivora.toml`, if it exists.
    /// 3. No file — start from defaults.
    ///
    /// # Errors
    /// See [`Config::load_from`] and [`validation::validate`].
    pub fn load() -> Result<Config, RivoraError> {
        if let Some(path) = std::env::var_os("RIVORA_CONFIG").map(PathBuf::from) {
            return Self::load_from(&path);
        }
        let default_path = PathBuf::from("rivora.toml");
        if default_path.is_file() {
            Self::load_from(&default_path)
        } else {
            Self::from_env_only()
        }
    }

    /// Builds a configuration from defaults plus environment overrides only
    /// (no file), then validates.
    ///
    /// # Errors
    /// See [`validation::validate`].
    pub fn from_env_only() -> Result<Config, RivoraError> {
        let cfg = Config::default();
        Self::finalize(cfg)
    }

    /// Validates this configuration.
    ///
    /// # Errors
    /// See [`validation::validate`].
    pub fn validate(&self) -> Result<(), RivoraError> {
        validation::validate(self)
    }

    fn finalize(cfg: Config) -> Result<Config, RivoraError> {
        let mut value = serde_json::to_value(cfg)?;
        apply_env_overrides(&mut value);
        let cfg: Config = serde_json::from_value(value)?;
        cfg.validate()?;
        Ok(cfg)
    }
}

/// Applies `RIVORA_`-prefixed environment variables onto a JSON
/// representation of the configuration.
fn apply_env_overrides(value: &mut Value) {
    let mut vars: Vec<(String, String)> = std::env::vars()
        .filter(|(k, _)| k.starts_with("RIVORA_") && k != "RIVORA_CONFIG")
        .collect();
    vars.sort_by(|a, b| a.0.cmp(&b.0));

    for (key, val) in vars {
        let Some(rest) = key.strip_prefix("RIVORA_") else {
            continue;
        };
        let lower = rest.to_lowercase();
        let path: Vec<&str> = lower.split("__").filter(|s| !s.is_empty()).collect();
        if path.is_empty() {
            continue;
        }
        set_path(value, &path, Value::String(val));
    }
}

fn ensure_object(v: &mut Value) {
    if !v.is_object() {
        *v = Value::Object(serde_json::Map::new());
    }
}

/// Sets `new_val` at the dotted `path` inside `root`, creating intermediate
/// objects as needed. Non-object intermediate values are replaced with empty
/// objects (env wins).
fn set_path(root: &mut Value, path: &[&str], new_val: Value) {
    match path {
        [] => {}
        [key] => {
            ensure_object(root);
            root.as_object_mut()
                .unwrap()
                .insert((*key).to_string(), new_val);
        }
        [first, rest @ ..] => {
            ensure_object(root);
            let child = root
                .as_object_mut()
                .unwrap()
                .entry((*first).to_string())
                .or_insert(Value::Object(serde_json::Map::new()));
            if !child.is_object() {
                *child = Value::Object(serde_json::Map::new());
            }
            set_path(child, rest, new_val);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_toml_str_parses_full_config() {
        let toml = r#"
[organization]
id = "org-acme"
name = "Acme"

[storage]
backend = "redb"
path = "./.rivora/store"

[logging]
level = "debug"
format = "json"
"#;
        let cfg = Config::from_toml_str(toml).unwrap();
        assert_eq!(cfg.organization.id.as_ref().unwrap().as_str(), "org-acme");
        assert_eq!(cfg.organization.name.as_ref().unwrap().as_str(), "Acme");
        assert_eq!(cfg.storage.backend.as_ref().unwrap().as_str(), "redb");
        assert_eq!(
            cfg.storage.path.as_ref().unwrap().as_str(),
            "./.rivora/store"
        );
        assert_eq!(cfg.logging.level, "debug");
    }

    #[test]
    fn from_toml_str_uses_defaults_for_missing_sections() {
        let cfg = Config::from_toml_str("").unwrap();
        assert!(cfg.organization.id.is_none());
        assert_eq!(cfg.logging.level, "info");
    }

    #[test]
    fn from_toml_str_rejects_invalid_toml() {
        let err = Config::from_toml_str("not = valid = toml").unwrap_err();
        assert_eq!(err.kind(), rivora_errors::ErrorKind::ConfigLoad);
    }

    #[test]
    fn from_toml_str_rejects_invalid_value() {
        // NonEmptyString rejects empty on deserialize; surfaced as a config
        // load failure with an actionable message.
        let toml = r#"
[organization]
name = ""
"#;
        let err = Config::from_toml_str(toml).unwrap_err();
        assert_eq!(err.kind(), rivora_errors::ErrorKind::ConfigLoad);
        assert!(err.to_string().to_lowercase().contains("empty"));
    }

    #[test]
    fn from_file_missing_returns_config_not_found() {
        let err = Config::from_file(Path::new("/no/such/rivora.toml")).unwrap_err();
        assert_eq!(err.kind(), rivora_errors::ErrorKind::ConfigNotFound);
        assert!(err.to_string().contains("/no/such/rivora.toml"));
    }

    #[test]
    fn from_file_reads_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rivora.toml");
        std::fs::write(
            &path,
            r#"
[organization]
id = "org-x"
[storage]
backend = "sqlite"
"#,
        )
        .unwrap();
        let cfg = Config::from_file(&path).unwrap();
        assert_eq!(cfg.organization.id.as_ref().unwrap().as_str(), "org-x");
        assert_eq!(cfg.storage.backend.as_ref().unwrap().as_str(), "sqlite");
    }

    #[test]
    fn load_from_validates_level() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rivora.toml");
        std::fs::write(&path, "[logging]\nlevel = \"verbose\"\n").unwrap();
        let err = Config::load_from(&path).unwrap_err();
        assert_eq!(err.kind(), rivora_errors::ErrorKind::InvalidConfig);
    }

    #[test]
    fn set_path_creates_nested_objects() {
        let mut root = Value::Object(serde_json::Map::new());
        set_path(
            &mut root,
            &["logging", "level"],
            Value::String("debug".into()),
        );
        assert_eq!(root["logging"]["level"], Value::String("debug".into()));
    }

    #[test]
    fn set_path_replaces_scalar_with_object() {
        let mut root = Value::String("scalar".into());
        set_path(&mut root, &["a", "b"], Value::String("v".into()));
        assert_eq!(root["a"]["b"], Value::String("v".into()));
    }

    #[test]
    fn finalize_preserves_existing_when_no_env() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rivora.toml");
        std::fs::write(&path, "[organization]\nid = \"org-keep\"\n").unwrap();
        let loaded = Config::load_from(&path).unwrap();
        assert_eq!(
            loaded.organization.id.as_ref().unwrap().as_str(),
            "org-keep"
        );
    }
}
