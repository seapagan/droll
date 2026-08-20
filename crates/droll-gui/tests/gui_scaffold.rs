mod support;

use std::{env, io, process};

use avian3d::prelude::PhysicsPlugins;
use bevy::{MinimalPlugins, app::App};
use support::gui_scaffold_harness;

fn main() {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    let mut output = io::stdout().lock();

    if let Err(error) = gui_scaffold_harness::run(&arguments, &mut output, construct_scaffold) {
        eprintln!("gui_scaffold test harness: {error}");
        process::exit(2);
    }
}

fn construct_scaffold() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, PhysicsPlugins::default()));
}
