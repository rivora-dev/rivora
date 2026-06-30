use rivora_config::Config;

fn main() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| {
        eprintln!("could not create temp dir: {e}");
        std::process::exit(1);
    });
    let cfg_path = dir.path().join("rivora.toml");
    let toml = r#"
[organization]
id = "org-demo"

[storage]
backend = "redb"
path = "./store"

[logging]
level = "info"
"#;
    std::fs::write(&cfg_path, toml).unwrap_or_else(|e| {
        eprintln!("could not write rivora.toml: {e}");
        std::process::exit(1);
    });

    let cfg = Config::load_from(&cfg_path).unwrap_or_else(|e| {
        eprintln!("could not load configuration: {e}");
        std::process::exit(1);
    });

    println!(
        "organization id: {}",
        cfg.organization.id.as_ref().unwrap().as_str()
    );
    println!(
        "storage backend: {}",
        cfg.storage.backend.as_ref().unwrap().as_str()
    );
    println!("logging level: {}", cfg.logging.level);
}
