use avian3d::prelude::*;
use bevy::prelude::*;

use crate::{
    dice::{D6_MIN_FACE_BOUNDARY_ANGLE, d6_geometry},
    spike::{SpikeCase, StartCase},
};

use super::{
    DieLifecycle, DieMetrics, DieObservation, DieState, TransitionSample, advance_lifecycle,
};

type DirectedDieObservationQuery<'world, 'state> = Query<
    'world,
    'state,
    (
        &'static DirectedDie,
        &'static Position,
        &'static Rotation,
        &'static LinearVelocity,
        &'static AngularVelocity,
        &'static ComputedMass,
        &'static ComputedAngularInertia,
        &'static CollidingEntities,
        &'static mut DieState,
        &'static mut DieMetrics,
    ),
>;

/// Evidence-tuned controller and completion gates.
#[derive(Resource, Clone, Debug)]
pub struct DirectedPhysicsConfig {
    pub name: &'static str,
    pub guidance_enabled: bool,
    pub near_tray_height: f32,
    pub guidance_linear_speed: f32,
    pub guidance_angular_speed: f32,
    pub capture_angular_error: f32,
    pub disturbance_linear_speed: f32,
    pub disturbance_angular_speed: f32,
    pub proportional_gain: f32,
    pub damping_gain: f32,
    pub max_guidance_angular_acceleration: f32,
    pub ramp_seconds: f32,
    pub orientation_tolerance: f32,
    pub rest_linear_speed: f32,
    pub rest_angular_speed: f32,
    pub stable_window_seconds: f32,
    pub guidance_stall_seconds: f32,
    pub recovery_timeout_seconds: f32,
    pub recovery_upward_speed: f32,
    pub recovery_max_angular_speed: f32,
    pub recovery_rotation_fraction: f32,
    pub recovery_yaw_speed: f32,
    pub recovery_budget: u8,
    pub timeout_seconds: f32,
}

impl DirectedPhysicsConfig {
    #[must_use]
    pub fn unguided_baseline() -> Self {
        Self {
            name: "unguided-baseline",
            guidance_enabled: false,
            ..Self::candidate("unguided-baseline", 0.025, 0.1, 2.8)
        }
    }

    #[must_use]
    pub fn candidate(
        name: &'static str,
        capture_fraction: f32,
        max_angular_acceleration: f32,
        recovery_upward_speed: f32,
    ) -> Self {
        Self {
            name,
            guidance_enabled: true,
            near_tray_height: 0.9,
            guidance_linear_speed: 0.25,
            guidance_angular_speed: 0.35,
            capture_angular_error: D6_MIN_FACE_BOUNDARY_ANGLE * capture_fraction,
            disturbance_linear_speed: 0.65,
            disturbance_angular_speed: 1.0,
            proportional_gain: 4.0,
            damping_gain: 2.5,
            max_guidance_angular_acceleration: max_angular_acceleration,
            ramp_seconds: 0.35,
            orientation_tolerance: 0.10,
            rest_linear_speed: 0.10,
            rest_angular_speed: 0.16,
            stable_window_seconds: 0.60,
            guidance_stall_seconds: 1.5,
            recovery_timeout_seconds: 1.2,
            recovery_upward_speed,
            recovery_max_angular_speed: 6.0,
            recovery_rotation_fraction: 1.0,
            recovery_yaw_speed: 0.8,
            recovery_budget: 3,
            timeout_seconds: 18.0,
        }
    }
}

impl Default for DirectedPhysicsConfig {
    fn default() -> Self {
        Self::candidate("review-candidate-d", 0.025, 0.05, 2.6)
    }
}

/// Marks colliders that belong to the Stage 1 tray or its static walls.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct TraySurface;

#[derive(Component, Clone, Debug)]
pub struct DirectedDie {
    pub case_id: String,
    pub target_face: u8,
    pub target_rotation: Quat,
}

/// Returns the production physics components for one predetermined d6 case.
pub fn directed_d6_components(case: &SpikeCase) -> impl Bundle {
    let geometry = d6_geometry();
    let (translation, rotation, linear_velocity, angular_velocity) = initial_motion(case.start);
    (
        DirectedDie {
            case_id: case.id.clone(),
            target_face: case.target_face,
            target_rotation: geometry
                .face(case.target_face)
                .expect("scenario targets a d6 face")
                .target_rotation(case.yaw_radians),
        },
        DieState::default(),
        DieMetrics::default(),
        RigidBody::Dynamic,
        Collider::convex_hull(geometry.collider_vertices()).expect("d6 hull is convex"),
        CollisionEventsEnabled,
        CollidingEntities::default(),
        Friction::new(0.65),
        Restitution::new(0.32),
        LinearDamping(0.18),
        AngularDamping(0.28),
        LinearVelocity(linear_velocity),
        AngularVelocity(angular_velocity),
        Transform::from_translation(translation).with_rotation(rotation),
    )
}

fn initial_motion(start: StartCase) -> (Vec3, Quat, Vec3, Vec3) {
    if let Some(family) = start.symmetry_family() {
        let state = family.candidate.state;
        return (
            state.position,
            state.orientation,
            state.linear_velocity,
            state.angular_velocity,
        );
    }
    match start {
        StartCase::HighTumble => (
            Vec3::new(-1.4, 3.4, -0.8),
            Quat::from_euler(EulerRot::XYZ, 0.6, 0.2, 1.1),
            Vec3::new(2.4, 0.4, 1.1),
            Vec3::new(8.0, 4.0, 6.0),
        ),
        StartCase::SideSpin => (
            Vec3::new(1.6, 2.6, -0.4),
            Quat::from_euler(EulerRot::XYZ, 1.3, 0.7, 0.2),
            Vec3::new(-2.0, 0.2, 1.5),
            Vec3::new(2.0, 9.0, 3.0),
        ),
        StartCase::AwkwardLowEnergy => (
            Vec3::new(0.4, 1.0, 0.5),
            Quat::from_euler(EulerRot::XYZ, 0.78, 0.1, 0.72),
            Vec3::new(0.25, -0.1, -0.2),
            Vec3::new(0.6, 0.4, 0.5),
        ),
        StartCase::RecoveryBadOrientation => (
            Vec3::new(0.0, 0.72, 0.0),
            Quat::from_euler(EulerRot::XYZ, 0.78, 0.0, 0.78),
            Vec3::ZERO,
            Vec3::splat(0.05),
        ),
        StartCase::RecoveryEdge => (
            Vec3::new(3.4, 0.75, 0.0),
            Quat::from_euler(EulerRot::XYZ, 0.0, 0.3, 0.78),
            Vec3::new(-0.2, 0.0, 0.0),
            Vec3::new(0.1, 0.2, 0.1),
        ),
        StartCase::SymmetryFamilyA
        | StartCase::SymmetryFamilyB
        | StartCase::SymmetryFamilyC
        | StartCase::SymmetryFamilyD => unreachable!("handled above"),
    }
}

pub(super) fn apply_directed_forces(
    config: Res<DirectedPhysicsConfig>,
    mut dice: Query<(
        &DirectedDie,
        &mut DieState,
        Forces,
        &ComputedMass,
        &ComputedAngularInertia,
        &mut DieMetrics,
    )>,
) {
    for (die, mut state, mut forces, mass, inertia, mut metrics) in &mut dice {
        match state.lifecycle {
            DieLifecycle::GuidedSettling if config.guidance_enabled => {
                apply_guidance(&config, die, &state, &mut forces, inertia, &mut metrics);
            }
            DieLifecycle::Recovery if !state.recovery_impulse_applied => {
                apply_recovery(&config, die, &mut forces, mass, inertia, &mut metrics);
                state.recovery_impulse_applied = true;
            }
            _ => {}
        }
    }
}

pub(super) fn observe_directed_dice(
    time: Res<Time<Fixed>>,
    config: Res<DirectedPhysicsConfig>,
    tray_surfaces: Query<(), With<TraySurface>>,
    mut dice: DirectedDieObservationQuery,
) {
    let delta_seconds = time.delta_secs();
    for (
        die,
        position,
        rotation,
        linear,
        angular,
        mass,
        inertia,
        collisions,
        mut state,
        mut metrics,
    ) in &mut dice
    {
        let geometry = d6_geometry();
        let upward_face = geometry.upward_face(rotation.0).value;
        let observation = DieObservation {
            tray_contact: collisions
                .iter()
                .any(|entity| tray_surfaces.contains(*entity)),
            position_y: position.y,
            linear_speed: linear.length(),
            angular_speed: angular.length(),
            angular_error: target_tilt_error(die.target_face, rotation.0).1,
            kinetic_energy: kinetic_energy(*linear, *angular, *mass, *inertia, rotation.0),
            upward_face,
            target_face: die.target_face,
        };
        metrics.observe(observation.linear_speed, observation.angular_speed);
        if state.lifecycle == DieLifecycle::GuidedSettling {
            metrics.observe_guided_motion(observation.linear_speed, observation.angular_speed);
        }
        let previous = state.lifecycle;
        if let Some(reason) = advance_lifecycle(&mut state, observation, delta_seconds, &config) {
            let sample = TransitionSample {
                at_seconds: state.total_seconds,
                from: previous,
                to: state.lifecycle,
                reason,
                linear_speed: observation.linear_speed,
                angular_speed: observation.angular_speed,
                angular_error: observation.angular_error,
                kinetic_energy: observation.kinetic_energy,
            };
            info!(
                case = die.case_id,
                target = die.target_face,
                current = observation.upward_face,
                linear_speed = observation.linear_speed,
                angular_speed = observation.angular_speed,
                angular_error = observation.angular_error,
                from = ?sample.from,
                to = ?sample.to,
                reason = ?sample.reason,
                "directed d6 transition"
            );
            metrics.record(sample);
        }
    }
}

fn apply_guidance(
    config: &DirectedPhysicsConfig,
    die: &DirectedDie,
    state: &DieState,
    forces: &mut impl WriteRigidBodyForces,
    inertia: &ComputedAngularInertia,
    metrics: &mut DieMetrics,
) {
    let rotation = forces.rotation().0;
    let Some(angular_acceleration) = guidance_angular_acceleration(
        config,
        die.target_face,
        rotation,
        forces.angular_velocity(),
        state.state_seconds,
    ) else {
        return;
    };
    let torque = inertia.rotated(rotation).tensor() * angular_acceleration;
    metrics.max_guidance_angular_acceleration = metrics
        .max_guidance_angular_acceleration
        .max(angular_acceleration.length());
    metrics.max_guidance_torque = metrics.max_guidance_torque.max(torque.length());
    forces.apply_torque(torque);
}

fn guidance_angular_acceleration(
    config: &DirectedPhysicsConfig,
    target_face: u8,
    rotation: Quat,
    angular_velocity: Vec3,
    state_seconds: f32,
) -> Option<Vec3> {
    let (axis, angle) = target_tilt_error(target_face, rotation);
    if d6_geometry().upward_face(rotation).value != target_face
        || angle > config.capture_angular_error
    {
        return None;
    }
    let tilt_velocity = angular_velocity - Vec3::Y * angular_velocity.dot(Vec3::Y);
    let ramp = (state_seconds / config.ramp_seconds).clamp(0.0, 1.0);
    Some(
        ((axis * angle * config.proportional_gain - tilt_velocity * config.damping_gain) * ramp)
            .clamp_length_max(config.max_guidance_angular_acceleration),
    )
}

fn apply_recovery(
    config: &DirectedPhysicsConfig,
    die: &DirectedDie,
    forces: &mut impl WriteRigidBodyForces,
    mass: &ComputedMass,
    inertia: &ComputedAngularInertia,
    metrics: &mut DieMetrics,
) {
    let rotation = forces.rotation().0;
    let (axis, angle) = target_tilt_error(die.target_face, rotation);
    let flight_seconds = 2.0 * config.recovery_upward_speed / 9.81;
    let target_speed = (angle * config.recovery_rotation_fraction / flight_seconds)
        .min(config.recovery_max_angular_speed);
    let desired_angular_velocity = (axis * target_speed + Vec3::Y * config.recovery_yaw_speed)
        .clamp_length_max(config.recovery_max_angular_speed);
    let delta_angular_velocity = desired_angular_velocity - forces.angular_velocity();
    let angular_impulse = inertia.rotated(rotation).tensor() * delta_angular_velocity;
    let upward_delta = (config.recovery_upward_speed - forces.linear_velocity().y).max(0.0);
    let linear_impulse = Vec3::Y * mass.value() * upward_delta;
    metrics.max_recovery_linear_impulse = metrics
        .max_recovery_linear_impulse
        .max(linear_impulse.length());
    metrics.max_recovery_angular_impulse = metrics
        .max_recovery_angular_impulse
        .max(angular_impulse.length());
    metrics.recovery_impulses_applied += 1;
    forces.apply_linear_impulse(linear_impulse);
    forces.apply_angular_impulse(angular_impulse);
}

fn kinetic_energy(
    linear: LinearVelocity,
    angular: AngularVelocity,
    mass: ComputedMass,
    inertia: ComputedAngularInertia,
    rotation: Quat,
) -> f32 {
    let linear_energy = 0.5 * mass.value() * linear.length_squared();
    let angular_momentum = inertia.rotated(rotation).tensor() * angular.0;
    linear_energy + 0.5 * angular.0.dot(angular_momentum)
}

fn target_tilt_error(target_face: u8, current: Quat) -> (Vec3, f32) {
    let local_normal = d6_geometry()
        .face(target_face)
        .expect("directed d6 target is valid")
        .normal;
    Quat::from_rotation_arc(current * local_normal, Vec3::Y).to_axis_angle()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guidance_rejects_substantially_wrong_target_orientation() {
        let acceleration = guidance_angular_acceleration(
            &DirectedPhysicsConfig::default(),
            1,
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            Vec3::ZERO,
            1.0,
        );
        assert_eq!(acceleration, None);
    }

    #[test]
    fn test_guidance_accepts_only_small_target_upward_nudge() {
        let config = DirectedPhysicsConfig::default();
        let acceleration = guidance_angular_acceleration(
            &config,
            1,
            Quat::from_rotation_z(config.capture_angular_error * 0.5),
            Vec3::ZERO,
            config.ramp_seconds,
        )
        .expect("small target-upward tilt is capturable");
        assert!(acceleration.length() <= config.max_guidance_angular_acceleration);
    }
}
