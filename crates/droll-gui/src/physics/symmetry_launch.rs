use avian3d::prelude::*;
use bevy::{app::FixedPostUpdate, prelude::*};

use crate::dice::{d6_geometry, d6_symmetry_mapping};

use super::{D6LaunchFamily, LaunchNuisance, TraySurface};

type SymmetryObservationQuery<'world, 'state> = Query<
    'world,
    'state,
    (
        &'static SymmetryDie,
        &'static Position,
        &'static Rotation,
        &'static LinearVelocity,
        &'static AngularVelocity,
        &'static CollidingEntities,
        &'static mut SymmetryDieState,
        &'static mut SymmetryMetrics,
    ),
>;

/// H1 lifecycle: no guidance or recovery states exist on this path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymmetryLifecycle {
    Spawned,
    FreeThrow,
    Bouncing,
    RestCandidate,
    Revealed,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymmetryTerminalReason {
    CorrectFace,
    WrongFace,
    Timeout,
}

#[derive(Component, Clone, Debug)]
pub struct SymmetryDie {
    pub case_id: String,
    pub family_id: &'static str,
    pub nuisance: LaunchNuisance,
    pub target_face: u8,
    pub base_face: u8,
    pub symmetry_id: u8,
    pub symmetry: Quat,
}

#[derive(Component, Clone, Debug)]
pub struct SymmetryDieState {
    pub lifecycle: SymmetryLifecycle,
    pub total_seconds: f32,
    pub stable_seconds: f32,
    pub terminal_reason: Option<SymmetryTerminalReason>,
}

impl Default for SymmetryDieState {
    fn default() -> Self {
        Self {
            lifecycle: SymmetryLifecycle::Spawned,
            total_seconds: 0.0,
            stable_seconds: 0.0,
            terminal_reason: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactSample {
    pub at_seconds: f32,
    pub active: bool,
    pub contact_count: usize,
}

#[derive(Component, Clone, Debug, Default)]
pub struct SymmetryMetrics {
    pub first_contact_seconds: Option<f32>,
    pub terminal_seconds: Option<f32>,
    pub max_linear_speed: f32,
    pub max_angular_speed: f32,
    pub contact_samples: Vec<ContactSample>,
    pub post_spawn_target_force: f32,
    pub post_spawn_target_torque: f32,
    pub post_spawn_target_work: f32,
    pub recovery_count: u8,
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct SymmetryPhysicsConfig {
    pub rest_linear_speed: f32,
    pub rest_angular_speed: f32,
    pub stable_window_seconds: f32,
    pub timeout_seconds: f32,
}

impl Default for SymmetryPhysicsConfig {
    fn default() -> Self {
        Self {
            rest_linear_speed: 0.10,
            rest_angular_speed: 0.16,
            stable_window_seconds: 0.60,
            timeout_seconds: 18.0,
        }
    }
}

/// Observes an otherwise ordinary dynamic Avian body; it never applies forces.
pub struct SymmetryPhysicsPlugin;

#[derive(Clone, Copy)]
struct MotionObservation {
    rotation: Quat,
    linear_speed: f32,
    angular_speed: f32,
    contact: bool,
}

struct SymmetrySpawn {
    case_id: String,
    family: D6LaunchFamily,
    nuisance: LaunchNuisance,
    target_face: u8,
    symmetry_id: u8,
    symmetry: Quat,
    orientation: Quat,
    linear_velocity: Vec3,
    angular_velocity: Vec3,
    position: Vec3,
}

impl Plugin for SymmetryPhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SymmetryPhysicsConfig>().add_systems(
            FixedPostUpdate,
            observe_symmetry_dice
                .after(PhysicsSystems::Writeback)
                .before(PhysicsSystems::Last),
        );
    }
}

/// Builds a target-mapped H1 die from a complete target-independent variant.
#[must_use]
pub fn symmetry_d6_components(
    case_id: String,
    family: D6LaunchFamily,
    nuisance: LaunchNuisance,
    target_face: u8,
) -> impl Bundle {
    let variant = nuisance.apply(family.candidate.state);
    let symmetry = d6_symmetry_mapping(target_face, family.natural_face)
        .expect("every d6 target maps to the family base face");
    let target_orientation = variant.orientation * symmetry.rotation;
    symmetry_components(SymmetrySpawn {
        case_id,
        family,
        nuisance,
        target_face,
        symmetry_id: symmetry.id,
        symmetry: symmetry.rotation,
        orientation: target_orientation,
        linear_velocity: variant.linear_velocity,
        angular_velocity: variant.angular_velocity,
        position: variant.position,
    })
}

/// Builds the target-independent geometric control for paired H1 comparison.
#[must_use]
pub fn symmetry_d6_control_components(
    case_id: String,
    family: D6LaunchFamily,
    nuisance: LaunchNuisance,
) -> impl Bundle {
    let variant = nuisance.apply(family.candidate.state);
    symmetry_components(SymmetrySpawn {
        case_id,
        family,
        nuisance,
        target_face: family.natural_face,
        symmetry_id: u8::MAX,
        symmetry: Quat::IDENTITY,
        orientation: variant.orientation,
        linear_velocity: variant.linear_velocity,
        angular_velocity: variant.angular_velocity,
        position: variant.position,
    })
}

fn symmetry_components(spawn: SymmetrySpawn) -> impl Bundle {
    let geometry = d6_geometry();
    (
        SymmetryDie {
            case_id: spawn.case_id,
            family_id: spawn.family.family_id,
            nuisance: spawn.nuisance,
            target_face: spawn.target_face,
            base_face: spawn.family.natural_face,
            symmetry_id: spawn.symmetry_id,
            symmetry: spawn.symmetry,
        },
        SymmetryDieState::default(),
        SymmetryMetrics::default(),
        RigidBody::Dynamic,
        Collider::convex_hull(geometry.collider_vertices()).expect("d6 hull is convex"),
        CollisionEventsEnabled,
        CollidingEntities::default(),
        Friction::new(0.65),
        Restitution::new(0.32),
        LinearDamping(0.18),
        AngularDamping(0.28),
        LinearVelocity(spawn.linear_velocity),
        AngularVelocity(spawn.angular_velocity),
        Transform::from_translation(spawn.position).with_rotation(spawn.orientation),
    )
}

fn observe_symmetry_dice(
    time: Res<Time<Fixed>>,
    config: Res<SymmetryPhysicsConfig>,
    tray_surfaces: Query<(), With<TraySurface>>,
    mut dice: SymmetryObservationQuery,
) {
    let delta_seconds = time.delta_secs();
    for (die, _position, rotation, linear, angular, collisions, mut state, mut metrics) in &mut dice
    {
        let tray_contacts = collisions
            .iter()
            .filter(|entity| tray_surfaces.contains(**entity))
            .count();
        let contact = tray_contacts > 0;
        observe_contact(&mut metrics, state.total_seconds, contact, tray_contacts);
        metrics.max_linear_speed = metrics.max_linear_speed.max(linear.length());
        metrics.max_angular_speed = metrics.max_angular_speed.max(angular.length());
        advance_symmetry_state(
            die,
            &mut state,
            &mut metrics,
            MotionObservation {
                rotation: rotation.0,
                linear_speed: linear.length(),
                angular_speed: angular.length(),
                contact,
            },
            delta_seconds,
            &config,
        );
    }
}

fn observe_contact(
    metrics: &mut SymmetryMetrics,
    at_seconds: f32,
    active: bool,
    contact_count: usize,
) {
    let changed = metrics
        .contact_samples
        .last()
        .is_none_or(|sample| sample.active != active || sample.contact_count != contact_count);
    if changed {
        metrics.contact_samples.push(ContactSample {
            at_seconds,
            active,
            contact_count,
        });
    }
    if active {
        metrics.first_contact_seconds.get_or_insert(at_seconds);
    }
}

fn advance_symmetry_state(
    die: &SymmetryDie,
    state: &mut SymmetryDieState,
    metrics: &mut SymmetryMetrics,
    observation: MotionObservation,
    delta_seconds: f32,
    config: &SymmetryPhysicsConfig,
) {
    if matches!(
        state.lifecycle,
        SymmetryLifecycle::Revealed | SymmetryLifecycle::Failed
    ) {
        return;
    }
    state.total_seconds += delta_seconds;
    if state.total_seconds >= config.timeout_seconds {
        finish(state, metrics, SymmetryTerminalReason::Timeout);
        return;
    }
    match state.lifecycle {
        SymmetryLifecycle::Spawned => state.lifecycle = SymmetryLifecycle::FreeThrow,
        SymmetryLifecycle::FreeThrow if observation.contact => {
            state.lifecycle = SymmetryLifecycle::Bouncing;
        }
        SymmetryLifecycle::Bouncing | SymmetryLifecycle::RestCandidate => {
            update_rest_candidate(die, state, metrics, observation, delta_seconds, config);
        }
        _ => {}
    }
}

fn update_rest_candidate(
    die: &SymmetryDie,
    state: &mut SymmetryDieState,
    metrics: &mut SymmetryMetrics,
    observation: MotionObservation,
    delta_seconds: f32,
    config: &SymmetryPhysicsConfig,
) {
    let resting = observation.contact
        && observation.linear_speed <= config.rest_linear_speed
        && observation.angular_speed <= config.rest_angular_speed;
    if !resting {
        state.lifecycle = SymmetryLifecycle::Bouncing;
        state.stable_seconds = 0.0;
        return;
    }
    state.lifecycle = SymmetryLifecycle::RestCandidate;
    state.stable_seconds += delta_seconds;
    if state.stable_seconds < config.stable_window_seconds {
        return;
    }
    let upward = d6_geometry().upward_face(observation.rotation).value;
    let reason = if upward == die.target_face {
        SymmetryTerminalReason::CorrectFace
    } else {
        SymmetryTerminalReason::WrongFace
    };
    finish(state, metrics, reason);
}

fn finish(
    state: &mut SymmetryDieState,
    metrics: &mut SymmetryMetrics,
    reason: SymmetryTerminalReason,
) {
    state.lifecycle = if reason == SymmetryTerminalReason::CorrectFace {
        SymmetryLifecycle::Revealed
    } else {
        SymmetryLifecycle::Failed
    };
    state.terminal_reason = Some(reason);
    metrics.terminal_seconds = Some(state.total_seconds);
}
