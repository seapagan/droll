mod support;

use std::{cell::Cell, ffi::OsString};

use support::gui_scaffold_harness;

fn arguments(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

#[test]
fn test_normal_listing_reports_one_non_ignored_test_without_running_it() {
    let runs = Cell::new(0);
    let mut output = Vec::new();

    let result = gui_scaffold_harness::run(
        &arguments(&["--list", "--format", "terse"]),
        &mut output,
        || runs.set(runs.get() + 1),
    );

    assert_eq!(result, Ok(()));
    assert_eq!(output, b"gui_scaffold: test\n");
    assert_eq!(runs.get(), 0);
}

#[test]
fn test_ignored_listing_reports_no_tests_without_running_it() {
    let runs = Cell::new(0);
    let mut output = Vec::new();

    let result = gui_scaffold_harness::run(
        &arguments(&["--list", "--format", "terse", "--ignored"]),
        &mut output,
        || runs.set(runs.get() + 1),
    );

    assert_eq!(result, Ok(()));
    assert!(output.is_empty());
    assert_eq!(runs.get(), 0);
}

#[test]
fn test_exact_execution_runs_the_scaffold_once() {
    let runs = Cell::new(0);
    let mut output = Vec::new();

    let result = gui_scaffold_harness::run(
        &arguments(&["--exact", "gui_scaffold", "--nocapture"]),
        &mut output,
        || runs.set(runs.get() + 1),
    );

    assert_eq!(result, Ok(()));
    assert!(output.is_empty());
    assert_eq!(runs.get(), 1);
}

#[test]
fn test_unfiltered_cargo_execution_runs_the_scaffold_once() {
    let runs = Cell::new(0);
    let mut output = Vec::new();

    let result = gui_scaffold_harness::run(&[], &mut output, || runs.set(runs.get() + 1));

    assert_eq!(result, Ok(()));
    assert!(output.is_empty());
    assert_eq!(runs.get(), 1);
}

#[test]
fn test_unsupported_invocation_fails_without_running_the_scaffold() {
    let runs = Cell::new(0);
    let mut output = Vec::new();

    let result = gui_scaffold_harness::run(&arguments(&["unexpected"]), &mut output, || {
        runs.set(runs.get() + 1);
    });

    assert_eq!(result, Err("unsupported test harness invocation"));
    assert!(output.is_empty());
    assert_eq!(runs.get(), 0);
}
