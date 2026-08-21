//! Native graphical application scaffold and bounded Stage 1 spike support.

pub mod dice;
pub mod spike;

use avian3d::prelude::PhysicsPlugins;
use bevy::{DefaultPlugins, app::App};

/// Constructs the minimal Stage 0 graphical application.
pub fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins((DefaultPlugins, PhysicsPlugins::default()));
    app
}
