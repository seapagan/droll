use std::time::Duration;

use avian3d::prelude::{
    AngularVelocity, Collider, LinearVelocity, PhysicsPlugins, RigidBody, Rotation,
};
use bevy::{prelude::*, time::TimeUpdateStrategy};
use droll_gui::{
    dice::d6_geometry,
    physics::{
        DieLifecycle, DieMetrics, DieObservation, DieState, DirectedDie, DirectedPhysicsConfig,
        DirectedPhysicsPlugin, TransitionReason, advance_lifecycle, directed_d6_components,
    },
    spike::{SpikeCase, SpikeScenario},
};

const TEST_WATCHDOG_STEPS: usize = 1_400;

#[derive(Debug)]
struct RunResult {
    lifecycle: DieLifecycle,
    target: u8,
    upward: u8,
    recoveries: u8,
    seconds: f32,
    linear_speed: f32,
    angular_speed: f32,
    angular_error: f32,
    first_guidance_seconds: Option<f32>,
    guidance_entry_linear_speed: Option<f32>,
    guidance_entry_angular_speed: Option<f32>,
    max_guidance_torque: f32,
    first_contact_seconds: Option<f32>,
    max_linear_speed: f32,
    max_angular_speed: f32,
}

#[test]
fn test_lifecycle_requires_low_energy_before_guidance() {
    let config = DirectedPhysicsConfig::default();
    let mut state = DieState {
        lifecycle: DieLifecycle::Bouncing,
        ..default()
    };
    assert_eq!(
        advance_lifecycle(
            &mut state,
            observation(2.0, 5.0, true, 1, 1, 0.0),
            0.1,
            &config
        ),
        None
    );
    assert_eq!(state.lifecycle, DieLifecycle::Bouncing);
    assert_eq!(
        advance_lifecycle(
            &mut state,
            observation(0.2, 0.4, true, 2, 1, 1.0),
            0.1,
            &config
        ),
        Some(TransitionReason::LowEnergyNearTray)
    );
}

#[test]
fn test_rest_candidate_reenters_bouncing_after_disturbance() {
    let config = DirectedPhysicsConfig::default();
    let mut state = DieState {
        lifecycle: DieLifecycle::RestCandidate,
        stable_seconds: 0.4,
        ..default()
    };
    assert_eq!(
        advance_lifecycle(
            &mut state,
            observation(1.6, 0.2, false, 1, 1, 0.03),
            0.1,
            &config
        ),
        Some(TransitionReason::RenewedMotion)
    );
    assert_eq!(state.lifecycle, DieLifecycle::Bouncing);
    assert_eq!(state.stable_seconds, 0.0);
}

#[test]
fn test_wrong_face_never_reveals() {
    let config = DirectedPhysicsConfig::default();
    let mut state = DieState {
        lifecycle: DieLifecycle::RestCandidate,
        stable_seconds: config.stable_window_seconds - 0.01,
        ..default()
    };
    assert_eq!(
        advance_lifecycle(
            &mut state,
            observation(0.01, 0.01, true, 2, 1, 0.01),
            0.02,
            &config
        ),
        Some(TransitionReason::CandidateInvalidated)
    );
    assert_ne!(state.lifecycle, DieLifecycle::Revealed);
}

#[test]
fn test_recovery_budget_terminates() {
    let config = DirectedPhysicsConfig::default();
    let mut state = DieState {
        lifecycle: DieLifecycle::GuidedSettling,
        state_seconds: config.guidance_stall_seconds,
        recovery_count: config.recovery_budget,
        ..default()
    };
    assert_eq!(
        advance_lifecycle(
            &mut state,
            observation(0.1, 0.1, true, 2, 1, 1.0),
            0.01,
            &config
        ),
        Some(TransitionReason::RecoveryBudgetExhausted)
    );
    assert_eq!(state.lifecycle, DieLifecycle::Failed);
}

#[test]
fn test_d6_measurement_matrix_is_finite_and_never_reveals_wrong_face() {
    let candidates = [
        DirectedPhysicsConfig::unguided_baseline(),
        DirectedPhysicsConfig::candidate("candidate-a", 4.0, 1.4, 0.35),
        DirectedPhysicsConfig::candidate("candidate-b", 7.0, 2.4, 0.55),
        DirectedPhysicsConfig::candidate("candidate-c", 10.0, 3.2, 0.80),
    ];
    for config in candidates {
        for case in SpikeScenario::D6Faces.cases().expect("d6 cases exist") {
            let result = run_case(&case, config.clone());
            println!(
                "{},{},{:?},{},{},{},{:.3},{:.4},{:.4},{:.4},{:.3},{:.4},{:.4},{:.4},{:.3},{:.4},{:.4}",
                config.name,
                case.id,
                result.lifecycle,
                result.target,
                result.upward,
                result.recoveries,
                result.seconds,
                result.linear_speed,
                result.angular_speed,
                result.angular_error,
                result.first_guidance_seconds.unwrap_or(-1.0),
                result.guidance_entry_linear_speed.unwrap_or(-1.0),
                result.guidance_entry_angular_speed.unwrap_or(-1.0),
                result.max_guidance_torque,
                result.first_contact_seconds.unwrap_or(-1.0),
                result.max_linear_speed,
                result.max_angular_speed
            );
            assert!(matches!(
                result.lifecycle,
                DieLifecycle::Revealed | DieLifecycle::Failed
            ));
            if result.lifecycle == DieLifecycle::Revealed {
                assert_eq!(result.upward, result.target);
            }
        }
    }
}

#[test]
fn test_review_candidate_d6_controller_settles_every_bounded_case() {
    let config = DirectedPhysicsConfig::default();
    for case in SpikeScenario::D6Faces.cases().expect("d6 cases exist") {
        let result = run_case(&case, config.clone());
        assert_eq!(
            result.lifecycle,
            DieLifecycle::Revealed,
            "{case:?}: {result:?}"
        );
        assert_eq!(result.upward, result.target, "{case:?}: {result:?}");
        assert!(result.recoveries <= config.recovery_budget);
    }
}

#[test]
fn test_recovery_scenario_cases_terminate_with_correct_face() {
    let config = DirectedPhysicsConfig::default();
    for case in SpikeScenario::Recovery
        .cases()
        .expect("recovery cases exist")
    {
        let result = run_case(&case, config.clone());
        println!("review-candidate-b,{},recovery,{result:?}", case.id);
        assert_eq!(
            result.lifecycle,
            DieLifecycle::Revealed,
            "{case:?}: {result:?}"
        );
        assert_eq!(result.upward, result.target, "{case:?}: {result:?}");
        assert!(result.recoveries <= config.recovery_budget);
    }
}

#[test]
fn test_physical_disturbance_reenters_bouncing() {
    let case = SpikeScenario::D6Faces.cases().expect("d6 cases exist")[0].clone();
    let (mut app, entity) = build_case_app(&case, DirectedPhysicsConfig::default());
    for _ in 0..TEST_WATCHDOG_STEPS {
        app.update();
        if app
            .world()
            .get::<DieState>(entity)
            .expect("state exists")
            .lifecycle
            == DieLifecycle::RestCandidate
        {
            break;
        }
    }
    assert_eq!(
        app.world()
            .get::<DieState>(entity)
            .expect("state exists")
            .lifecycle,
        DieLifecycle::RestCandidate
    );
    app.world_mut()
        .entity_mut(entity)
        .insert(LinearVelocity(Vec3::new(2.0, 1.0, 0.0)));
    app.update();
    assert_eq!(
        app.world()
            .get::<DieState>(entity)
            .expect("state exists")
            .lifecycle,
        DieLifecycle::Bouncing
    );
}

#[test]
fn test_guidance_does_not_snap_to_target_in_one_step() {
    let case = SpikeScenario::D6Faces.cases().expect("d6 cases exist")[10].clone();
    let (mut app, entity) = build_case_app(&case, DirectedPhysicsConfig::default());
    for _ in 0..TEST_WATCHDOG_STEPS {
        app.update();
        if app
            .world()
            .get::<DieState>(entity)
            .expect("state exists")
            .lifecycle
            == DieLifecycle::GuidedSettling
        {
            break;
        }
    }
    let before = target_tilt(&app, entity);
    app.update();
    let after = target_tilt(&app, entity);
    assert!(before > 0.1);
    assert!(after > 0.01);
    assert!((after - before).abs() < 0.5);
}

fn run_case(case: &SpikeCase, config: DirectedPhysicsConfig) -> RunResult {
    let (mut app, entity) = build_case_app(case, config);
    for _ in 0..TEST_WATCHDOG_STEPS {
        app.update();
        let lifecycle = app
            .world()
            .get::<DieState>(entity)
            .expect("state exists")
            .lifecycle;
        if matches!(lifecycle, DieLifecycle::Revealed | DieLifecycle::Failed) {
            break;
        }
    }
    result(&app, entity)
}

fn build_case_app(case: &SpikeCase, config: DirectedPhysicsConfig) -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        PhysicsPlugins::default(),
        DirectedPhysicsPlugin,
    ));
    app.finish();
    app.cleanup();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / 60.0,
    )));
    app.insert_resource(config);
    app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(8.0, 0.2, 6.0),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));
    for (size, translation) in [
        (Vec3::new(8.0, 0.8, 0.2), Vec3::new(0.0, 0.4, -3.0)),
        (Vec3::new(8.0, 0.8, 0.2), Vec3::new(0.0, 0.4, 3.0)),
        (Vec3::new(0.2, 0.8, 6.0), Vec3::new(-4.0, 0.4, 0.0)),
        (Vec3::new(0.2, 0.8, 6.0), Vec3::new(4.0, 0.4, 0.0)),
    ] {
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(size.x, size.y, size.z),
            Transform::from_translation(translation),
        ));
    }
    let entity = app.world_mut().spawn(directed_d6_components(case)).id();
    (app, entity)
}

fn result(app: &App, entity: Entity) -> RunResult {
    let world = app.world();
    let state = world.get::<DieState>(entity).expect("state exists");
    let die = world.get::<DirectedDie>(entity).expect("die exists");
    let rotation = world.get::<Rotation>(entity).expect("rotation exists").0;
    let linear = world
        .get::<LinearVelocity>(entity)
        .expect("linear velocity exists");
    let angular = world
        .get::<AngularVelocity>(entity)
        .expect("angular velocity exists");
    let metrics = world.get::<DieMetrics>(entity).expect("metrics exist");
    RunResult {
        lifecycle: state.lifecycle,
        target: die.target_face,
        upward: d6_geometry().upward_face(rotation).value,
        recoveries: state.recovery_count,
        seconds: metrics.terminal_seconds.unwrap_or(state.total_seconds),
        linear_speed: linear.length(),
        angular_speed: angular.length(),
        angular_error: (rotation
            * d6_geometry()
                .face(die.target_face)
                .expect("target face exists")
                .normal)
            .angle_between(Vec3::Y),
        first_guidance_seconds: metrics.first_guidance_seconds,
        guidance_entry_linear_speed: metrics.guidance_entry_linear_speed,
        guidance_entry_angular_speed: metrics.guidance_entry_angular_speed,
        max_guidance_torque: metrics.max_guidance_torque,
        first_contact_seconds: metrics.first_contact_seconds,
        max_linear_speed: metrics.max_linear_speed,
        max_angular_speed: metrics.max_angular_speed,
    }
}

fn target_tilt(app: &App, entity: Entity) -> f32 {
    let die = app.world().get::<DirectedDie>(entity).expect("die exists");
    let rotation = app
        .world()
        .get::<Rotation>(entity)
        .expect("rotation exists")
        .0;
    (rotation
        * d6_geometry()
            .face(die.target_face)
            .expect("target face exists")
            .normal)
        .angle_between(Vec3::Y)
}

fn observation(
    linear_speed: f32,
    angular_speed: f32,
    tray_contact: bool,
    upward_face: u8,
    target_face: u8,
    angular_error: f32,
) -> DieObservation {
    DieObservation {
        tray_contact,
        position_y: 0.5,
        linear_speed,
        angular_speed,
        angular_error,
        upward_face,
        target_face,
    }
}
