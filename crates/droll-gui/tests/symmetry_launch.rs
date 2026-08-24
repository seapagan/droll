use std::time::Duration;

use avian3d::prelude::{
    AngularVelocity, Collider, CollidingEntities, LinearVelocity, PhysicsPlugins, Position,
    RigidBody, Rotation,
};
use bevy::{prelude::*, time::TimeUpdateStrategy};
use droll_gui::{
    dice::d6_geometry,
    physics::{
        D6LaunchFamily, LaunchNuisance, SymmetryDie, SymmetryDieState, SymmetryLifecycle,
        SymmetryMetrics, SymmetryPhysicsPlugin, SymmetryTerminalReason, TraySurface,
        d6_passing_launch_families, symmetry_d6_components, symmetry_d6_control_components,
    },
};

const STEP_SECONDS: f64 = 1.0 / 60.0;
const WATCHDOG_STEPS: usize = 1_080;
const POSITION_TOLERANCE: f32 = 0.01;
const VELOCITY_TOLERANCE: f32 = 0.02;
const ROTATION_TOLERANCE: f32 = 0.01;

#[derive(Clone, Copy, Debug)]
struct TrajectorySample {
    position: Vec3,
    linear_velocity: Vec3,
    angular_velocity: Vec3,
    canonical_orientation: Quat,
    contact_count: usize,
    lifecycle: SymmetryLifecycle,
}

#[derive(Clone, Copy, Debug, Default)]
struct TrajectoryDelta {
    position: f32,
    linear_velocity: f32,
    angular_velocity: f32,
    canonical_orientation: f32,
}

#[derive(Debug)]
struct H1Run {
    lifecycle: SymmetryLifecycle,
    reason: Option<SymmetryTerminalReason>,
    upward_face: u8,
    target_face: u8,
    seconds: f32,
    metrics: SymmetryMetrics,
    trace: Vec<TrajectorySample>,
}

#[test]
fn test_h1_all_face_nominal_and_nuisance_corpus_is_216_of_216() {
    let mut passed = 0;
    for family in d6_passing_launch_families() {
        for nuisance in LaunchNuisance::ALL {
            let mut group_passed = 0;
            let mut completion_max = 0.0_f32;
            let mut max_linear = 0.0_f32;
            let mut max_angular = 0.0_f32;
            for target_face in 1..=6 {
                let run = run_h1(family, nuisance, target_face, false);
                assert_h1_success(family, nuisance, target_face, &run);
                completion_max = completion_max.max(run.seconds);
                max_linear = max_linear.max(run.metrics.max_linear_speed);
                max_angular = max_angular.max(run.metrics.max_angular_speed);
                group_passed += 1;
                passed += 1;
            }
            println!(
                "h1-corpus,{},{},{},{},{group_passed}/6,{completion_max:.3},{max_linear:.4},{max_angular:.4},intervention-0,recovery-0",
                family.family_id,
                family.candidate.search_id,
                family.natural_face,
                nuisance.as_str()
            );
        }
    }
    assert_eq!(passed, 216);
}

#[test]
fn test_h1_paired_trajectories_match_before_contact_after_symmetry_canonicalization() {
    let mut paired = 0;
    for family in d6_passing_launch_families() {
        for nuisance in LaunchNuisance::ALL {
            let control = run_h1(family, nuisance, family.natural_face, true);
            assert_h1_success(family, nuisance, family.natural_face, &control);
            for target_face in 1..=6 {
                let target = run_h1(family, nuisance, target_face, false);
                assert_h1_success(family, nuisance, target_face, &target);
                assert_paired_trajectory_contract(family, nuisance, target_face, &control, &target);
                paired += 1;
            }
        }
    }
    assert_eq!(paired, 216);
}

#[test]
#[should_panic(expected = "pre-contact linear velocity")]
fn test_pre_contact_gate_rejects_target_dependent_launch_input() {
    let control = TrajectorySample {
        position: Vec3::ZERO,
        linear_velocity: Vec3::X,
        angular_velocity: Vec3::Y,
        canonical_orientation: Quat::IDENTITY,
        contact_count: 0,
        lifecycle: SymmetryLifecycle::FreeThrow,
    };
    let target = TrajectorySample {
        linear_velocity: Vec3::X * 1.1,
        ..control
    };
    assert_pre_contact_equivalence(
        "target-dependent launch regression",
        &[control],
        &[target],
        1,
    );
}

fn assert_h1_success(
    family: D6LaunchFamily,
    nuisance: LaunchNuisance,
    target_face: u8,
    run: &H1Run,
) {
    assert_eq!(
        run.lifecycle,
        SymmetryLifecycle::Revealed,
        "{family:?} {nuisance:?} {run:?}"
    );
    assert_eq!(run.reason, Some(SymmetryTerminalReason::CorrectFace));
    assert_eq!(run.target_face, target_face);
    assert_eq!(run.upward_face, target_face);
    assert!(run.seconds <= 18.0);
    assert_eq!(run.metrics.post_spawn_target_force, 0.0);
    assert_eq!(run.metrics.post_spawn_target_torque, 0.0);
    assert_eq!(run.metrics.post_spawn_target_work, 0.0);
    assert_eq!(run.metrics.recovery_count, 0);
}

fn assert_paired_trajectory_contract(
    family: D6LaunchFamily,
    nuisance: LaunchNuisance,
    target_face: u8,
    control: &H1Run,
    target: &H1Run,
) {
    let context = format!("{family:?} {nuisance:?} target {target_face}");
    let first_contact_step = first_contact_step(control).min(first_contact_step(target));
    let pre_contact =
        assert_pre_contact_equivalence(&context, &control.trace, &target.trace, first_contact_step);
    report_post_contact_diagnostics(&context, control, target, first_contact_step, pre_contact);
}

fn first_contact_step(run: &H1Run) -> usize {
    run.trace
        .iter()
        .position(|sample| sample.contact_count > 0)
        .unwrap_or(run.trace.len())
}

fn assert_pre_contact_equivalence(
    context: &str,
    control: &[TrajectorySample],
    target: &[TrajectorySample],
    first_contact_step: usize,
) -> TrajectoryDelta {
    let mut maximum = TrajectoryDelta::default();
    for (step, (left, right)) in control
        .iter()
        .zip(target)
        .take(first_contact_step)
        .enumerate()
    {
        assert_eq!(left.contact_count, 0, "{context} pre-contact step {step}");
        assert_eq!(right.contact_count, 0, "{context} pre-contact step {step}");
        let delta = trajectory_delta(left, right);
        maximum.include(delta);
        assert!(
            delta.position <= POSITION_TOLERANCE,
            "{context} pre-contact position step {step}"
        );
        assert!(
            delta.linear_velocity <= VELOCITY_TOLERANCE,
            "{context} pre-contact linear velocity step {step}"
        );
        assert!(
            delta.angular_velocity <= VELOCITY_TOLERANCE,
            "{context} pre-contact angular velocity step {step}"
        );
        assert!(
            delta.canonical_orientation <= ROTATION_TOLERANCE,
            "{context} pre-contact canonical orientation step {step}"
        );
    }
    maximum
}

fn report_post_contact_diagnostics(
    context: &str,
    control: &H1Run,
    target: &H1Run,
    first_contact_step: usize,
    pre_contact: TrajectoryDelta,
) {
    let mut post_contact = TrajectoryDelta::default();
    let mut first_divergence = None;
    for (step, (left, right)) in control
        .trace
        .iter()
        .zip(&target.trace)
        .enumerate()
        .skip(first_contact_step)
    {
        let delta = trajectory_delta(left, right);
        post_contact.include(delta);
        if first_divergence.is_none()
            && (delta.exceeds_tolerance()
                || left.contact_count != right.contact_count
                || !lifecycle_equivalent(left.lifecycle, right.lifecycle))
        {
            first_divergence = Some((step, delta, left.contact_count, right.contact_count));
        }
    }
    println!(
        "paired,{context},first-contact-step-{first_contact_step},pre-{pre_contact:?},first-post-contact-divergence-{first_divergence:?},post-{post_contact:?},contact-type-tray,control-contacts-{:?},target-contacts-{:?},completion-delta-{:.6},final-faces-{}-{}",
        control.metrics.contact_samples,
        target.metrics.contact_samples,
        (control.seconds - target.seconds).abs(),
        control.upward_face,
        target.upward_face,
    );
}

fn trajectory_delta(left: &TrajectorySample, right: &TrajectorySample) -> TrajectoryDelta {
    TrajectoryDelta {
        position: (left.position - right.position).length(),
        linear_velocity: (left.linear_velocity - right.linear_velocity).length(),
        angular_velocity: (left.angular_velocity - right.angular_velocity).length(),
        canonical_orientation: rotation_distance(
            left.canonical_orientation,
            right.canonical_orientation,
        ),
    }
}

impl TrajectoryDelta {
    fn include(&mut self, other: Self) {
        self.position = self.position.max(other.position);
        self.linear_velocity = self.linear_velocity.max(other.linear_velocity);
        self.angular_velocity = self.angular_velocity.max(other.angular_velocity);
        self.canonical_orientation = self.canonical_orientation.max(other.canonical_orientation);
    }

    fn exceeds_tolerance(self) -> bool {
        self.position > POSITION_TOLERANCE
            || self.linear_velocity > VELOCITY_TOLERANCE
            || self.angular_velocity > VELOCITY_TOLERANCE
            || self.canonical_orientation > ROTATION_TOLERANCE
    }
}

fn lifecycle_equivalent(left: SymmetryLifecycle, right: SymmetryLifecycle) -> bool {
    left == right
        || matches!(
            (left, right),
            (
                SymmetryLifecycle::RestCandidate,
                SymmetryLifecycle::Revealed
            ) | (
                SymmetryLifecycle::Revealed,
                SymmetryLifecycle::RestCandidate
            ) | (
                SymmetryLifecycle::Bouncing,
                SymmetryLifecycle::RestCandidate
            ) | (
                SymmetryLifecycle::RestCandidate,
                SymmetryLifecycle::Bouncing
            )
        )
}

fn run_h1(
    family: D6LaunchFamily,
    nuisance: LaunchNuisance,
    target_face: u8,
    control: bool,
) -> H1Run {
    let mut app = build_app();
    let case_id = format!(
        "{}-{}-target-{target_face}",
        family.family_id,
        nuisance.as_str()
    );
    let entity = if control {
        app.world_mut()
            .spawn(symmetry_d6_control_components(case_id, family, nuisance))
            .id()
    } else {
        app.world_mut()
            .spawn(symmetry_d6_components(
                case_id,
                family,
                nuisance,
                target_face,
            ))
            .id()
    };
    let mut trace = Vec::new();
    for _ in 0..WATCHDOG_STEPS {
        app.update();
        trace.push(sample(&app, entity));
        if matches!(
            app.world()
                .get::<SymmetryDieState>(entity)
                .expect("state")
                .lifecycle,
            SymmetryLifecycle::Revealed | SymmetryLifecycle::Failed
        ) {
            break;
        }
    }
    result(&app, entity, trace)
}

fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        PhysicsPlugins::default(),
        SymmetryPhysicsPlugin,
    ));
    app.finish();
    app.cleanup();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        STEP_SECONDS,
    )));
    app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(8.0, 0.2, 6.0),
        TraySurface,
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
            TraySurface,
            Transform::from_translation(translation),
        ));
    }
    app
}

fn sample(app: &App, entity: Entity) -> TrajectorySample {
    let world = app.world();
    let die = world.get::<SymmetryDie>(entity).expect("die");
    let rotation = world.get::<Rotation>(entity).expect("rotation").0;
    TrajectorySample {
        position: world.get::<Position>(entity).expect("position").0,
        linear_velocity: world.get::<LinearVelocity>(entity).expect("linear").0,
        angular_velocity: world.get::<AngularVelocity>(entity).expect("angular").0,
        canonical_orientation: (rotation * die.symmetry.inverse()).normalize(),
        contact_count: world
            .get::<CollidingEntities>(entity)
            .expect("contacts")
            .len(),
        lifecycle: world
            .get::<SymmetryDieState>(entity)
            .expect("state")
            .lifecycle,
    }
}

fn result(app: &App, entity: Entity, trace: Vec<TrajectorySample>) -> H1Run {
    let world = app.world();
    let state = world.get::<SymmetryDieState>(entity).expect("state");
    let metrics = world.get::<SymmetryMetrics>(entity).expect("metrics");
    let die = world.get::<SymmetryDie>(entity).expect("die");
    let rotation = world.get::<Rotation>(entity).expect("rotation").0;
    H1Run {
        lifecycle: state.lifecycle,
        reason: state.terminal_reason,
        upward_face: d6_geometry().upward_face(rotation).value,
        target_face: die.target_face,
        seconds: state.total_seconds,
        metrics: metrics.clone(),
        trace,
    }
}

fn rotation_distance(left: Quat, right: Quat) -> f32 {
    2.0 * left.dot(right).abs().clamp(-1.0, 1.0).acos()
}
