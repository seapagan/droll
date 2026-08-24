//! Candidate D and H1 rigid-body paths used by the bounded Stage 1 spike.

mod controller;
mod launch;
mod metrics;
mod recorded;
mod state;
mod symmetry_launch;

use avian3d::prelude::PhysicsSystems;
use bevy::{app::FixedPostUpdate, prelude::*};

pub use controller::{DirectedDie, DirectedPhysicsConfig, TraySurface, directed_d6_components};
pub use launch::{
    D6LaunchCandidate, D6LaunchFamily, D6LaunchState, LaunchKind, LaunchNuisance,
    d6_launch_search_candidates, d6_passing_launch_families,
};
pub use metrics::{DieMetrics, MetricSummary, TransitionSample};
pub use recorded::{
    AttemptDiagnostic, AttemptOutcome, BatchContactDiagnostics, CalibrationMetrics,
    ContactDiagnostics, D6PresentationMapping, D6PresentationPhase, D20PresentationMapping,
    D20PresentationPhase, DiceContactSample, DieKind, FixedD6Presentation, FixedD20Presentation,
    InitialPhysicalState, InvalidityReason, NaturalTerminalDiagnostics, NumberedVisual,
    PhysicalBatchRequest, PhysicalTray, PhysicalValidityPolicy, PlaybackError, PlaybackRoot,
    PreparationFailure, PresentationMapError, RecordedBatch, RecordedDie, RecordedPlaybackClock,
    RecordedPlaybackPlugin, RecordedTrajectoryPlayback, SampledPlaybackTransform,
    SemanticPresentationMap, SupportClassification, TrajectorySample, compose_visible_orientation,
    map_d6_presentation, map_d20_presentation, natural_record_identity, prepare_recorded_batch,
    sample_recorded_transform,
};
pub use state::{DieLifecycle, DieObservation, DieState, TransitionReason, advance_lifecycle};
pub use symmetry_launch::{
    ContactSample, SymmetryDie, SymmetryDieState, SymmetryLifecycle, SymmetryMetrics,
    SymmetryPhysicsConfig, SymmetryPhysicsPlugin, SymmetryTerminalReason, symmetry_d6_components,
    symmetry_d6_control_components,
};

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
