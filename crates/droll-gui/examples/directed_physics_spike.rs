use std::{env, process::ExitCode, str::FromStr};

use droll_gui::spike::{SpikeOptions, SpikeScenario, build_spike_app};

fn main() -> ExitCode {
    match parse_options(env::args().skip(1).collect()) {
        Ok(options) => {
            build_spike_app(options).run();
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("error: {message}");
            eprintln!("usage: directed_physics_spike --scenario <name> [--case <case-id>]");
            ExitCode::from(2)
        }
    }
}

fn parse_options(arguments: Vec<String>) -> Result<SpikeOptions, String> {
    let mut scenario = None;
    let mut case_id = None;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--scenario" => {
                let value = arguments
                    .next()
                    .ok_or_else(|| "--scenario requires a value".to_owned())?;
                scenario = Some(SpikeScenario::from_str(&value)?);
            }
            "--case" => {
                case_id = Some(
                    arguments
                        .next()
                        .ok_or_else(|| "--case requires a value".to_owned())?,
                );
            }
            _ => return Err(format!("unsupported argument `{argument}`")),
        }
    }
    Ok(SpikeOptions {
        scenario: scenario.ok_or_else(|| "--scenario is required".to_owned())?,
        case_id,
    })
}
