use std::{env, process::ExitCode};

const HELP: &str = "Droll command-line scaffold\n\nUsage: droll [OPTIONS]\n\nOptions:\n  -h, --help     Print help\n  -V, --version  Print version\n";

fn main() -> ExitCode {
    match env::args().nth(1).as_deref() {
        Some("-h" | "--help") => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Some("-V" | "--version") => {
            println!("droll {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!(
                "droll is currently a Stage 0 scaffold; rolling and GUI dispatch are not implemented"
            );
            ExitCode::from(2)
        }
    }
}
