//! Native graphical application scaffold for Droll.

use avian3d::prelude::PhysicsPlugins;
use bevy::{DefaultPlugins, app::App};

/// Constructs the minimal Stage 0 graphical application.
#[must_use]
pub fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins((DefaultPlugins, PhysicsPlugins::default()));
    app
}

#[cfg(test)]
mod tests {
    use super::build_app;

    #[test]
    fn test_build_app_registers_the_scaffold_plugins() {
        let _app = build_app();
    }
}
