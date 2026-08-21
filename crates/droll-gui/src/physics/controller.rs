use avian3d::prelude::*;
use bevy::prelude::*;

use crate::{
    dice::d6_geometry,
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
    pub disturbance_linear_speed: f32,
    pub disturbance_angular_speed: f32,
    pub proportional_gain: f32,
    pub damping_gain: f32,
    pub ramp_seconds: f32,
    pub orientation_tolerance: f32,
    pub rest_linear_speed: f32,
    pub rest_angular_speed: f32,
    pub stable_window_seconds: f32,
    pub guidance_stall_seconds: f32,
    pub recovery_seconds: f32,
    pub recovery_impulse: f32,
    pub recovery_angular_impulse: f32,
    pub recovery_budget: u8,
    pub timeout_seconds: f32,
}

impl DirectedPhysicsConfig {
    #[must_use]
    pub fn unguided_baseline() -> Self {
        Self {
            name: "unguided-baseline",
            guidance_enabled: false,
            ..Self::candidate("unguided-baseline", 0.0, 0.0, 1.0)
        }
    }

    #[must_use]
    pub fn candidate(name: &'static str, proportional: f32, damping: f32, ramp: f32) -> Self {
        Self {
            name,
            guidance_enabled: true,
            near_tray_height: 0.9,
            guidance_linear_speed: 0.9,
            guidance_angular_speed: 2.4,
            disturbance_linear_speed: 1.4,
            disturbance_angular_speed: 3.2,
            proportional_gain: proportional,
            damping_gain: damping,
            ramp_seconds: ramp,
            orientation_tolerance: 0.10,
            rest_linear_speed: 0.10,
            rest_angular_speed: 0.16,
            stable_window_seconds: 0.60,
            guidance_stall_seconds: 3.5,
            recovery_seconds: 0.55,
            recovery_impulse: 1.8,
            recovery_angular_impulse: 0.7,
            recovery_budget: 3,
            timeout_seconds: 18.0,
        }
    }
}

impl Default for DirectedPhysicsConfig {
    fn default() -> Self {
        Self::candidate("review-candidate-b", 7.0, 2.4, 0.55)
    }
}

#[derive(Component, Clone, Debug)]
pub struct DirectedDie {
    pub case_id: String,
    pub target_face: u8,
    pub target_rotation: Quat,
    pub require_recovery: bool,
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
            require_recovery: matches!(
                case.start,
                StartCase::RecoveryBadOrientation | StartCase::RecoveryEdge
            ),
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
    }
}

pub(super) fn apply_directed_forces(
    config: Res<DirectedPhysicsConfig>,
    mut dice: Query<(&DirectedDie, &DieState, Forces, &mut DieMetrics)>,
) {
    for (die, state, mut forces, mut metrics) in &mut dice {
        match state.lifecycle {
            DieLifecycle::GuidedSettling
                if config.guidance_enabled
                    && (!die.require_recovery || state.recovery_count > 0) =>
            {
                let current = *forces.rotation();
                let (axis, angle) = target_tilt_error(die.target_face, current.0);
                let ramp = (state.state_seconds / config.ramp_seconds).clamp(0.0, 1.0);
                let torque = (axis * angle * config.proportional_gain
                    - forces.angular_velocity() * config.damping_gain)
                    * ramp;
                metrics.max_guidance_torque = metrics.max_guidance_torque.max(torque.length());
                forces.apply_torque(torque);
            }
            DieLifecycle::Recovery if state.state_seconds <= 0.02 => {
                forces.apply_linear_impulse(Vec3::Y * config.recovery_impulse);
                forces.apply_angular_impulse(
                    Vec3::new(0.7, 0.4, -0.6) * config.recovery_angular_impulse,
                );
            }
            _ => {}
        }
    }
}

pub(super) fn observe_directed_dice(
    time: Res<Time<Fixed>>,
    config: Res<DirectedPhysicsConfig>,
    mut dice: DirectedDieObservationQuery,
) {
    let delta_seconds = time.delta_secs();
    for (die, position, rotation, linear, angular, collisions, mut state, mut metrics) in &mut dice
    {
        let geometry = d6_geometry();
        let upward_face = geometry.upward_face(rotation.0).value;
        let observation = DieObservation {
            tray_contact: !collisions.is_empty(),
            position_y: position.y,
            linear_speed: linear.length(),
            angular_speed: angular.length(),
            angular_error: target_tilt_error(die.target_face, rotation.0).1,
            upward_face,
            target_face: die.target_face,
        };
        metrics.observe(observation.linear_speed, observation.angular_speed);
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

fn target_tilt_error(target_face: u8, current: Quat) -> (Vec3, f32) {
    let local_normal = d6_geometry()
        .face(target_face)
        .expect("directed d6 target is valid")
        .normal;
    Quat::from_rotation_arc(current * local_normal, Vec3::Y).to_axis_angle()
}
