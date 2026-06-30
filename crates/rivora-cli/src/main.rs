fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match rivora_cli::run(
        args,
        std::env::current_dir()
            .as_deref()
            .unwrap_or_else(|_| std::path::Path::new(".")),
    ) {
        Ok(output) => {
            println!("{output}");
        }
        Err(error) => {
            eprintln!("Rivora could not complete the command: {error}");
            std::process::exit(1);
        }
    }
}
