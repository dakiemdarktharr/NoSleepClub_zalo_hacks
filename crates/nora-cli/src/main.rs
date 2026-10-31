//! Offline CLI entry point. It never opens a microphone or contacts a model API.

mod assets;
mod commands;

fn main() {
    if let Err(error) = commands::run(std::env::args().skip(1)) {
        eprintln!("nora: {error}");
        std::process::exit(2);
    }
}
