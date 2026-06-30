use rivora_core::{init_logging, LoggingConfig, LoggingFormat};

fn main() {
    let cfg = LoggingConfig {
        level: "info".to_string(),
        format: LoggingFormat::Pretty,
    };
    let _ = init_logging(&cfg);

    tracing::info!(
        organization = "org-acme",
        event = "observe",
        "starting observation pass"
    );
    tracing::warn!(component = "config", "example warning with fields");
    tracing::debug!(detail = "hidden at info level", "this won't show at info");

    println!("logging initialized");
}
