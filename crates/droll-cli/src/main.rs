use std::{env, ffi::OsStr, process::ExitCode};

const HELP: &str = "Droll command-line scaffold\n\nUsage: droll [OPTIONS]\n\nOptions:\n  -h, --help     Print help\n  -V, --version  Print version\n";

fn main() -> ExitCode {
    match env::args_os().nth(1).as_deref() {
        Some(argument) if argument == OsStr::new("-h") || argument == OsStr::new("--help") => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Some(argument) if argument == OsStr::new("-V") || argument == OsStr::new("--version") => {
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
