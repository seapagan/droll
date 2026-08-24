use std::{env, process::ExitCode, str::FromStr};

use droll_gui::spike::{SpikeMode, SpikeOptions, SpikeScenario, run_spike};

fn main() -> ExitCode {
    match parse_options(env::args().skip(1).collect()) {
        Ok(options) => {
            run_spike(options);
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("error: {message}");
            eprintln!(
                "usage: directed_physics_spike --scenario <name> --mode <candidate-d|recorded-replay|symmetry-launch> [--case <case-id>]"
            );
            ExitCode::from(2)
        }
    }
}

fn parse_options(arguments: Vec<String>) -> Result<SpikeOptions, String> {
    let mut scenario = None;
    let mut case_id = None;
    let mut mode = None;
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
            "--mode" => {
                let value = arguments
                    .next()
                    .ok_or_else(|| "--mode requires a value".to_owned())?;
                mode = Some(SpikeMode::from_str(&value)?);
            }
            _ => return Err(format!("unsupported argument `{argument}`")),
        }
    }
    Ok(SpikeOptions {
        scenario: scenario.ok_or_else(|| "--scenario is required".to_owned())?,
        case_id,
        mode: mode.ok_or_else(|| "--mode is required".to_owned())?,
    })
}
