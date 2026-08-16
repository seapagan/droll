use std::process::{Command, Output};

fn run_droll(argument: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_droll"))
        .arg(argument)
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .output()
        .expect("droll test binary should run")
}

#[test]
fn test_help_succeeds_without_a_display() {
    let output = run_droll("--help");

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Usage: droll"));
    assert!(output.stderr.is_empty());
}

#[test]
fn test_version_succeeds_without_a_display() {
    let output = run_droll("--version");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("droll {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert!(output.stderr.is_empty());
}
