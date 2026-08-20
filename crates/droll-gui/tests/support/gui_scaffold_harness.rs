use std::{ffi::OsString, io::Write};

const TEST_NAME: &str = "gui_scaffold";

pub fn run<W, F>(
    arguments: &[OsString],
    output: &mut W,
    construct_scaffold: F,
) -> Result<(), &'static str>
where
    W: Write,
    F: FnOnce(),
{
    match arguments {
        [] => {
            construct_scaffold();
            Ok(())
        }
        [list, format, terse] if list == "--list" && format == "--format" && terse == "terse" => {
            writeln!(output, "{TEST_NAME}: test").map_err(|_| "failed to write test listing")
        }
        [list, format, terse, ignored]
            if list == "--list"
                && format == "--format"
                && terse == "terse"
                && ignored == "--ignored" =>
        {
            Ok(())
        }
        [first, second, third]
            if (first == "--exact" && second == TEST_NAME && third == "--nocapture")
                || (first == TEST_NAME && second == "--nocapture" && third == "--exact") =>
        {
            construct_scaffold();
            Ok(())
        }
        _ => Err("unsupported test harness invocation"),
    }
}
