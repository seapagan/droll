use std::{
    ffi::OsStr,
    process::{Command, Output},
};

const STAGE_0_DIAGNOSTIC: &str =
    "droll is currently a Stage 0 scaffold; rolling and GUI dispatch are not implemented\n";

fn run_droll(argument: impl AsRef<OsStr>) -> Output {
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

#[test]
fn test_unsupported_argument_uses_stage_0_fallback() {
    let output = run_droll("unsupported");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(String::from_utf8_lossy(&output.stderr), STAGE_0_DIAGNOSTIC);
}

#[cfg(unix)]
#[test]
fn test_non_unicode_argument_uses_stage_0_fallback() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    let output = run_droll(OsString::from_vec(vec![0xff]));

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(String::from_utf8_lossy(&output.stderr), STAGE_0_DIAGNOSTIC);
}
