//! Directed rigid-body settling used by the bounded Stage 1 spike.

mod controller;
mod metrics;
mod state;

use avian3d::prelude::PhysicsSystems;
use bevy::{app::FixedPostUpdate, prelude::*};

pub use controller::{DirectedDie, DirectedPhysicsConfig, directed_d6_components};
pub use metrics::{DieMetrics, MetricSummary, TransitionSample};
pub use state::{DieLifecycle, DieObservation, DieState, TransitionReason, advance_lifecycle};

use controller::{apply_directed_forces, observe_directed_dice};

/// Installs target-aware forces before Avian steps and classification after writeback.
pub struct DirectedPhysicsPlugin;

impl Plugin for DirectedPhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DirectedPhysicsConfig>().add_systems(
            FixedPostUpdate,
            (
                apply_directed_forces
                    .after(PhysicsSystems::Prepare)
                    .before(PhysicsSystems::StepSimulation),
                observe_directed_dice
                    .after(PhysicsSystems::Writeback)
                    .before(PhysicsSystems::Last),
            ),
        );
    }
}
