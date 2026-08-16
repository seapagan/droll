use avian3d::prelude::PhysicsPlugins;
use bevy::{MinimalPlugins, app::App};

fn main() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, PhysicsPlugins::default()));
}
