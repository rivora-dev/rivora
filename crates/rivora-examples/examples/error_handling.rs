use rivora_errors::{ErrorKind, Result, RivoraError};
use std::path::Path;

fn print_error(label: &str, err: &RivoraError) {
    println!("{label}");
    println!("  display: {err}");
    println!("  kind: {}", err.kind().as_str());
    println!("  json: {}", serde_json::to_string(err).unwrap());
}

fn load_config_or_fail() -> Result<()> {
    let _ = rivora_config::Config::from_file(Path::new("/nonexistent/rivora.toml"))?;
    Ok(())
}

fn main() {
    let id_err = RivoraError::invalid_identifier("observation", "must not be empty");
    print_error("invalid identifier", &id_err);

    let version_err = RivoraError::invalid_version("not-semver", "unrecognized");
    print_error("invalid version", &version_err);

    let config_err =
        rivora_config::Config::from_file(Path::new("/nonexistent/rivora.toml")).unwrap_err();
    print_error("missing config file", &config_err);
    println!(
        "is config_not_found? {}",
        config_err.kind() == ErrorKind::ConfigNotFound
    );

    let boxed: Box<dyn std::error::Error> = Box::new(id_err);
    println!("boxed std::error::Error: {boxed}");

    if let Err(e) = load_config_or_fail() {
        println!("? operator propagated error: kind={}", e.kind().as_str());
    }
}
