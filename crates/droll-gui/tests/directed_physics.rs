use std::time::Duration;

use avian3d::prelude::{
    AngularVelocity, Collider, CollidingEntities, ComputedAngularInertia, ComputedMass,
    LinearVelocity, PhysicsPlugins, RigidBody, Rotation,
};
use bevy::{prelude::*, time::TimeUpdateStrategy};
use droll_gui::{
    dice::d6_geometry,
    physics::{
        DieLifecycle, DieMetrics, DieObservation, DieState, DirectedDie, DirectedPhysicsConfig,
        DirectedPhysicsPlugin, TransitionReason, TraySurface, advance_lifecycle,
        directed_d6_components,
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
    max_guidance_angular_acceleration: f32,
    max_guided_linear_speed: f32,
    max_guided_angular_speed: f32,
    guided_disturbances: usize,
    guidance_entry_angular_error: Option<f32>,
    guidance_entry_kinetic_energy: Option<f32>,
    recovery_entry_angular_error: Option<f32>,
    recovery_entry_kinetic_energy: Option<f32>,
    max_recovery_linear_impulse: f32,
    max_recovery_angular_impulse: f32,
    recovery_impulses_applied: u8,
    body_mass: f32,
    max_principal_inertia: f32,
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
            observation(0.2, 0.3, true, 1, 1, 0.005),
            0.1,
            &config
        ),
        Some(TransitionReason::TargetInsideCapture)
    );
}

#[test]
fn test_low_energy_target_outside_capture_enters_recovery() {
    let config = DirectedPhysicsConfig::default();
    let mut state = DieState {
        lifecycle: DieLifecycle::Bouncing,
        ..default()
    };
    assert_eq!(
        advance_lifecycle(
            &mut state,
            observation(0.1, 0.1, true, 2, 1, std::f32::consts::FRAC_PI_2),
            0.01,
            &config
        ),
        Some(TransitionReason::TargetOutsideCapture)
    );
    assert_eq!(state.lifecycle, DieLifecycle::Recovery);
    assert_eq!(state.recovery_count, 1);
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
        Some(TransitionReason::LeftCaptureRegion)
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
        DirectedPhysicsConfig::candidate("capture-narrow", 0.0125, 0.05, 2.6),
        DirectedPhysicsConfig::candidate("review-candidate-d", 0.025, 0.05, 2.6),
        DirectedPhysicsConfig::candidate("capture-candidate-e", 0.05, 0.2, 3.0),
    ];
    for config in candidates {
        let mut recovery_total = 0_u32;
        let mut recovery_max = 0_u8;
        let mut completion_max = 0.0_f32;
        let mut guidance_error_max = 0.0_f32;
        let mut guidance_energy_max = 0.0_f32;
        for case in SpikeScenario::D6Faces.cases().expect("d6 cases exist") {
            let result = run_case(&case, config.clone());
            recovery_total += u32::from(result.recoveries);
            recovery_max = recovery_max.max(result.recoveries);
            completion_max = completion_max.max(result.seconds);
            guidance_error_max =
                guidance_error_max.max(result.guidance_entry_angular_error.unwrap_or_default());
            guidance_energy_max =
                guidance_energy_max.max(result.guidance_entry_kinetic_energy.unwrap_or_default());
            print_measurement(&config, &case, &result);
            assert!(matches!(
                result.lifecycle,
                DieLifecycle::Revealed | DieLifecycle::Failed
            ));
            if result.lifecycle == DieLifecycle::Revealed {
                assert_eq!(result.upward, result.target);
            }
        }
        println!(
            "summary,{},{recovery_total},{recovery_max},{completion_max:.3},{guidance_error_max:.6},{guidance_energy_max:.6}",
            config.name
        );
    }
}

fn print_measurement(config: &DirectedPhysicsConfig, case: &SpikeCase, result: &RunResult) {
    println!(
        "{},{},{:?},{},{},{},{:.3},{:.4},{:.4},{:.4},{:.3},{:.4},{:.4},{:.4},{:.4},{:.6},{:.4},{:.6},{:.4},{:.6},{:.6},{:.3},{:.4},{:.4}",
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
        result.guidance_entry_angular_error.unwrap_or(-1.0),
        result.guidance_entry_kinetic_energy.unwrap_or(-1.0),
        result.max_guidance_torque,
        result.max_guidance_angular_acceleration,
        result.recovery_entry_angular_error.unwrap_or(-1.0),
        result.recovery_entry_kinetic_energy.unwrap_or(-1.0),
        result.max_recovery_linear_impulse,
        result.max_recovery_angular_impulse,
        result.first_contact_seconds.unwrap_or(-1.0),
        result.max_linear_speed,
        result.max_angular_speed
    );
}

#[test]
fn test_capture_candidate_d6_controller_settles_every_bounded_case() {
    let config = DirectedPhysicsConfig::default();
    for case in SpikeScenario::D6Faces.cases().expect("d6 cases exist") {
        let result = run_case(&case, config.clone());
        println!("{},{}:{result:?}", config.name, case.id);
        assert_eq!(
            result.lifecycle,
            DieLifecycle::Revealed,
            "{case:?}: {result:?}"
        );
        assert_eq!(result.upward, result.target, "{case:?}: {result:?}");
        assert!(result.recoveries <= config.recovery_budget);
        assert_eq!(result.recovery_impulses_applied, result.recoveries);
        assert!(
            result
                .guidance_entry_angular_error
                .is_none_or(|error| error <= config.capture_angular_error)
        );
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
        println!("review-candidate-d,{},recovery,{result:?}", case.id);
        assert_eq!(
            result.lifecycle,
            DieLifecycle::Revealed,
            "{case:?}: {result:?}"
        );
        assert_eq!(result.upward, result.target, "{case:?}: {result:?}");
        assert!(result.recoveries <= config.recovery_budget);
        assert_eq!(result.recovery_impulses_applied, result.recoveries);
        match case.start {
            droll_gui::spike::StartCase::RecoveryBadOrientation => {
                assert!(result.recoveries > 0, "{case:?}: {result:?}");
            }
            droll_gui::spike::StartCase::RecoveryEdge => {
                assert_eq!(result.recoveries, 0, "{case:?}: {result:?}");
            }
            _ => unreachable!("recovery scenario has only recovery starts"),
        }
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
fn test_guidance_nudge_stays_inside_capture_region_without_snap() {
    let case = SpikeScenario::D6Faces.cases().expect("d6 cases exist")[15].clone();
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
    let config = DirectedPhysicsConfig::default();
    assert!(before <= config.capture_angular_error);
    assert!(after <= config.capture_angular_error);
    assert!((after - before).abs() < config.capture_angular_error);
}

#[test]
fn test_guidance_output_stays_within_physical_acceleration_bound() {
    let config = DirectedPhysicsConfig::default();
    for case in SpikeScenario::D6Faces.cases().expect("d6 cases exist") {
        let result = run_case(&case, config.clone());
        assert!(
            result.max_guidance_angular_acceleration
                <= config.max_guidance_angular_acceleration + 1.0e-5,
            "{case:?}: {result:?}"
        );
        assert!(result.max_guidance_torque.is_finite());
        assert!(
            result.max_guidance_torque
                <= result.max_principal_inertia * config.max_guidance_angular_acceleration + 1.0e-5,
            "{case:?}: {result:?}"
        );
        assert!(result.max_recovery_linear_impulse.is_finite());
        assert!(result.max_recovery_angular_impulse.is_finite());
        assert!(
            result.max_recovery_linear_impulse
                <= result.body_mass * (config.recovery_upward_speed + config.guidance_linear_speed)
                    + 1.0e-5,
            "{case:?}: {result:?}"
        );
        assert!(
            result.max_recovery_angular_impulse
                <= result.max_principal_inertia
                    * (config.recovery_max_angular_speed + config.guidance_angular_speed)
                    + 1.0e-5,
            "{case:?}: {result:?}"
        );
        assert!(result.max_guided_linear_speed < config.disturbance_linear_speed);
        assert!(result.max_guided_angular_speed < config.disturbance_angular_speed);
        assert_eq!(result.guided_disturbances, 0, "{case:?}: {result:?}");
    }
}

#[test]
fn test_recovery_rotation_fraction_sweep_is_finite() {
    for fraction in [0.8, 1.0, 1.2] {
        let config = DirectedPhysicsConfig {
            recovery_rotation_fraction: fraction,
            ..default()
        };
        let mut recovery_total = 0_u32;
        let mut recovery_max = 0_u8;
        let mut completion_max = 0.0_f32;
        for case in SpikeScenario::D6Faces.cases().expect("d6 cases exist") {
            let result = run_case(&case, config.clone());
            assert_eq!(result.lifecycle, DieLifecycle::Revealed, "{case:?}");
            assert_eq!(result.upward, result.target, "{case:?}");
            recovery_total += u32::from(result.recoveries);
            recovery_max = recovery_max.max(result.recoveries);
            completion_max = completion_max.max(result.seconds);
        }
        println!(
            "recovery-sweep,{fraction:.1},{recovery_total},{recovery_max},{completion_max:.3}"
        );
    }
}

#[test]
fn test_unrelated_collider_contact_does_not_enable_guidance() {
    let case = SpikeScenario::D6Faces.cases().expect("d6 cases exist")[2].clone();
    let (mut app, entity, unrelated_collider) =
        build_case_app_with_surface(&case, DirectedPhysicsConfig::default(), false);
    for _ in 0..200 {
        app.update();
        if !app
            .world()
            .get::<CollidingEntities>(entity)
            .expect("collisions exist")
            .is_empty()
        {
            break;
        }
    }
    assert!(
        !app.world()
            .get::<CollidingEntities>(entity)
            .expect("collisions exist")
            .is_empty()
    );
    assert_eq!(
        app.world()
            .get::<DieState>(entity)
            .expect("state exists")
            .lifecycle,
        DieLifecycle::FreeThrow
    );

    app.world_mut()
        .entity_mut(unrelated_collider)
        .insert(TraySurface);
    app.update();
    assert_ne!(
        app.world()
            .get::<DieState>(entity)
            .expect("state exists")
            .lifecycle,
        DieLifecycle::FreeThrow
    );
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
    let (app, entity, _) = build_case_app_with_surface(case, config, true);
    (app, entity)
}

fn build_case_app_with_surface(
    case: &SpikeCase,
    config: DirectedPhysicsConfig,
    mark_floor_as_tray: bool,
) -> (App, Entity, Entity) {
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
    let floor = app
        .world_mut()
        .spawn((
            RigidBody::Static,
            Collider::cuboid(8.0, 0.2, 6.0),
            Transform::from_xyz(0.0, -0.1, 0.0),
        ))
        .id();
    if mark_floor_as_tray {
        app.world_mut().entity_mut(floor).insert(TraySurface);
    }
    for (size, translation) in [
        (Vec3::new(8.0, 0.8, 0.2), Vec3::new(0.0, 0.4, -3.0)),
        (Vec3::new(8.0, 0.8, 0.2), Vec3::new(0.0, 0.4, 3.0)),
        (Vec3::new(0.2, 0.8, 6.0), Vec3::new(-4.0, 0.4, 0.0)),
        (Vec3::new(0.2, 0.8, 6.0), Vec3::new(4.0, 0.4, 0.0)),
    ] {
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(size.x, size.y, size.z),
            TraySurface,
            Transform::from_translation(translation),
        ));
    }
    let entity = app.world_mut().spawn(directed_d6_components(case)).id();
    (app, entity, floor)
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
    let mass = world.get::<ComputedMass>(entity).expect("mass exists");
    let inertia = world
        .get::<ComputedAngularInertia>(entity)
        .expect("inertia exists");
    let (principal_inertia, _) = inertia.principal_angular_inertia_with_local_frame();
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
        max_guidance_angular_acceleration: metrics.max_guidance_angular_acceleration,
        max_guided_linear_speed: metrics.max_guided_linear_speed,
        max_guided_angular_speed: metrics.max_guided_angular_speed,
        guided_disturbances: metrics
            .transitions
            .iter()
            .filter(|sample| {
                sample.from == DieLifecycle::GuidedSettling
                    && sample.reason == TransitionReason::RenewedMotion
            })
            .count(),
        guidance_entry_angular_error: metrics.guidance_entry_angular_error,
        guidance_entry_kinetic_energy: metrics.guidance_entry_kinetic_energy,
        recovery_entry_angular_error: metrics.recovery_entry_angular_error,
        recovery_entry_kinetic_energy: metrics.recovery_entry_kinetic_energy,
        max_recovery_linear_impulse: metrics.max_recovery_linear_impulse,
        max_recovery_angular_impulse: metrics.max_recovery_angular_impulse,
        recovery_impulses_applied: metrics.recovery_impulses_applied,
        body_mass: mass.value(),
        max_principal_inertia: principal_inertia.max_element(),
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
        kinetic_energy: 0.0,
        upward_face,
        target_face,
    }
}
