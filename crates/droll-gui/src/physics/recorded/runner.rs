use std::time::{Duration, Instant};

use avian3d::prelude::*;
use bevy::{app::FixedPostUpdate, prelude::*, time::TimeUpdateStrategy};

use crate::dice::{d6_geometry, d20_geometry};

use super::{
    types::{
        AttemptDiagnostic, AttemptOutcome, CalibrationMetrics, ContactDiagnostics, DieKind,
        FIXED_STEP, InvalidityReason, MAX_TOTAL_ATTEMPTS, NaturalTerminalDiagnostics,
        PhysicalBatchRequest, PreparationFailure, RecordedBatch, RecordedDie, TrajectorySample,
    },
    validity::{FaceObservation, classify_support, is_contained, observe_face},
};

#[derive(Component)]
struct HiddenDie;

#[derive(Component)]
struct HiddenTrayFloor;

#[derive(Component)]
struct HiddenTrayBoundary;

#[derive(Resource, Default)]
struct FixedStepCounter(u32);

struct HiddenWorld {
    app: App,
    dice: Vec<Entity>,
    floor: Entity,
    boundaries: Vec<Entity>,
    construction_duration: Duration,
    entity_count: u32,
}

struct DieRecorder {
    ordinal: u16,
    kind: DieKind,
    samples: Vec<TrajectorySample>,
    stable_steps: u16,
    stable_face: Option<u8>,
    stable_support: Option<super::types::SupportClassification>,
    support_invalidity: Option<InvalidityReason>,
    last_face: FaceObservation,
    linear_speed: f32,
    angular_speed: f32,
    active_contacts: Vec<Entity>,
    contact_diagnostics: ContactDiagnostics,
}

struct AcceptedAttempt {
    dice: Vec<RecordedDie>,
    fixed_steps: u32,
    wall_duration: Duration,
    construction_duration: Duration,
    entity_count: u32,
}

pub fn prepare_recorded_batch(
    request: &PhysicalBatchRequest,
) -> Result<RecordedBatch, PreparationFailure> {
    if request.dice().is_empty() {
        return Err(single_failure(request, InvalidityReason::EmptyBatch));
    }
    if request.dice().len() > usize::from(u16::MAX) {
        return Err(single_failure(request, InvalidityReason::TooManyDice));
    }
    let mut diagnostics = Vec::with_capacity(usize::from(MAX_TOTAL_ATTEMPTS));
    for attempt in 1..=MAX_TOTAL_ATTEMPTS {
        let seed = attempt_seed(request.physical_presentation_seed(), attempt);
        match run_attempt(request, seed) {
            Ok(accepted) => {
                diagnostics.push(attempt_diagnostic(
                    request,
                    attempt,
                    seed,
                    &accepted,
                    AttemptOutcome::Valid,
                ));
                let calibration = CalibrationMetrics::from_record(
                    &accepted.dice,
                    accepted.dice.capacity(),
                    accepted.fixed_steps,
                    accepted.wall_duration,
                    accepted.construction_duration,
                    accepted.entity_count,
                );
                return Ok(RecordedBatch {
                    fixed_step: FIXED_STEP,
                    attempts: diagnostics,
                    dice: accepted.dice,
                    calibration,
                });
            }
            Err((reason, failed)) => diagnostics.push(attempt_diagnostic(
                request,
                attempt,
                seed,
                &failed,
                AttemptOutcome::Invalid(reason),
            )),
        }
    }
    Err(PreparationFailure {
        attempts: diagnostics,
    })
}

fn run_attempt(
    request: &PhysicalBatchRequest,
    seed: u64,
) -> Result<AcceptedAttempt, (InvalidityReason, AcceptedAttempt)> {
    let hidden = build_hidden_world(request, seed);
    let mut recorders = create_recorders(request);
    let simulation_started = Instant::now();
    let mut hidden = hidden;
    for fixed_step in 1..=request.validity().watchdog_steps {
        let before = hidden.app.world().resource::<FixedStepCounter>().0;
        hidden.app.update();
        let after = hidden.app.world().resource::<FixedStepCounter>().0;
        if after != before + 1 {
            return failed_attempt(
                hidden,
                recorders,
                fixed_step,
                simulation_started.elapsed(),
                InvalidityReason::FixedStepDidNotAdvanceExactlyOnce,
            );
        }
        if let Err(reason) = record_step(&hidden, &mut recorders, fixed_step, request) {
            return failed_attempt(
                hidden,
                recorders,
                fixed_step,
                simulation_started.elapsed(),
                reason,
            );
        }
        if recorders
            .iter()
            .all(|recorder| recorder.stable_steps >= request.validity().stable_steps)
        {
            return finish_attempt(
                hidden,
                recorders,
                fixed_step,
                simulation_started.elapsed(),
                request,
            );
        }
    }
    let reason = terminal_watchdog_reason(&recorders, request.validity());
    failed_attempt(
        hidden,
        recorders,
        request.validity().watchdog_steps,
        simulation_started.elapsed(),
        reason,
    )
}

fn build_hidden_world(request: &PhysicalBatchRequest, seed: u64) -> HiddenWorld {
    let started = Instant::now();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, PhysicsPlugins::default()))
        .init_resource::<FixedStepCounter>()
        .add_systems(
            FixedPostUpdate,
            count_fixed_step.after(PhysicsSystems::Last),
        );
    app.finish();
    app.cleanup();
    app.insert_resource(Time::<Fixed>::from_duration(FIXED_STEP));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(FIXED_STEP));
    // Bevy's first update initializes its clocks with zero elapsed time. Warm
    // that frame before any physical entity exists so every recorded update is
    // exactly one 60 Hz fixed step.
    app.update();
    app.world_mut().resource_mut::<FixedStepCounter>().0 = 0;
    let tray = request.tray();
    let floor = spawn_floor(&mut app, tray);
    let boundaries = spawn_boundaries(&mut app, tray);
    let dice = spawn_dice(&mut app, request.dice(), seed);
    let entity_count = app.world().entities().len();
    HiddenWorld {
        app,
        dice,
        floor,
        boundaries,
        construction_duration: started.elapsed(),
        entity_count,
    }
}

fn spawn_floor(app: &mut App, tray: super::types::PhysicalTray) -> Entity {
    app.world_mut()
        .spawn((
            HiddenTrayFloor,
            RigidBody::Static,
            Collider::cuboid(tray.width, 0.2, tray.depth),
            Friction::new(0.72),
            Transform::from_xyz(0.0, -0.1, 0.0),
        ))
        .id()
}

fn spawn_boundaries(app: &mut App, tray: super::types::PhysicalTray) -> Vec<Entity> {
    let mut entities = Vec::with_capacity(4);
    for (size, translation) in [
        (
            Vec3::new(tray.width, tray.wall_height, 0.2),
            Vec3::new(0.0, tray.wall_height * 0.5, -tray.depth * 0.5),
        ),
        (
            Vec3::new(tray.width, tray.wall_height, 0.2),
            Vec3::new(0.0, tray.wall_height * 0.5, tray.depth * 0.5),
        ),
        (
            Vec3::new(0.2, tray.wall_height, tray.depth),
            Vec3::new(-tray.width * 0.5, tray.wall_height * 0.5, 0.0),
        ),
        (
            Vec3::new(0.2, tray.wall_height, tray.depth),
            Vec3::new(tray.width * 0.5, tray.wall_height * 0.5, 0.0),
        ),
    ] {
        entities.push(
            app.world_mut()
                .spawn((
                    HiddenTrayBoundary,
                    RigidBody::Static,
                    Collider::cuboid(size.x, size.y, size.z),
                    Friction::new(0.72),
                    Transform::from_translation(translation),
                ))
                .id(),
        );
    }
    entities
}

fn spawn_dice(app: &mut App, kinds: &[DieKind], seed: u64) -> Vec<Entity> {
    let mut rng = PhysicalRng::new(seed);
    kinds
        .iter()
        .copied()
        .enumerate()
        .map(|(index, kind)| {
            let transform = launch_transform(index, kinds.len(), &mut rng);
            let collider = match kind {
                DieKind::D6 => Collider::convex_hull(d6_geometry().collider_vertices()),
                DieKind::D20 => Collider::convex_hull(d20_geometry().collider_vertices()),
            }
            .expect("project die geometry is a valid convex hull");
            app.world_mut()
                .spawn((
                    HiddenDie,
                    RigidBody::Dynamic,
                    collider,
                    CollisionEventsEnabled,
                    CollidingEntities::default(),
                    Friction::new(0.65),
                    Restitution::new(0.32),
                    LinearDamping(0.18),
                    AngularDamping(0.28),
                    LinearVelocity(Vec3::new(rng.range(-1.5, 1.5), 0.2, rng.range(-1.2, 1.2))),
                    AngularVelocity(Vec3::new(
                        rng.range(-8.0, 8.0),
                        rng.range(-8.0, 8.0),
                        rng.range(-8.0, 8.0),
                    )),
                    transform,
                ))
                .id()
        })
        .collect()
}

fn launch_transform(index: usize, count: usize, rng: &mut PhysicalRng) -> Transform {
    let centered = index as f32 - (count.saturating_sub(1)) as f32 * 0.5;
    let position = Vec3::new(
        centered * 1.15 + rng.range(-0.12, 0.12),
        2.6 + (index % 2) as f32 * 0.35,
        rng.range(-0.55, 0.55),
    );
    let axis = Vec3::new(rng.signed(), rng.signed(), rng.signed())
        .try_normalize()
        .unwrap_or(Vec3::Y);
    Transform::from_translation(position)
        .with_rotation(Quat::from_axis_angle(axis, rng.range(0.2, 5.8)))
}

fn create_recorders(request: &PhysicalBatchRequest) -> Vec<DieRecorder> {
    request
        .dice()
        .iter()
        .copied()
        .enumerate()
        .map(|(ordinal, kind)| DieRecorder {
            ordinal: u16::try_from(ordinal).expect("batch size checked"),
            kind,
            samples: Vec::with_capacity(request.validity().watchdog_steps as usize),
            stable_steps: 0,
            stable_face: None,
            stable_support: None,
            support_invalidity: None,
            last_face: observe_face(kind, Quat::IDENTITY, request.validity()),
            linear_speed: f32::INFINITY,
            angular_speed: f32::INFINITY,
            active_contacts: Vec::new(),
            contact_diagnostics: ContactDiagnostics::default(),
        })
        .collect()
}

fn record_step(
    hidden: &HiddenWorld,
    recorders: &mut [DieRecorder],
    fixed_step: u32,
    request: &PhysicalBatchRequest,
) -> Result<(), InvalidityReason> {
    let positions = die_positions(hidden);
    for (index, (entity, recorder)) in hidden
        .dice
        .iter()
        .copied()
        .zip(recorders.iter_mut())
        .enumerate()
    {
        let world = hidden.app.world();
        let position = world.get::<Position>(entity).expect("die position").0;
        let raw_rotation = world.get::<Rotation>(entity).expect("die rotation").0;
        let linear = world
            .get::<LinearVelocity>(entity)
            .expect("linear velocity")
            .0;
        let angular = world
            .get::<AngularVelocity>(entity)
            .expect("angular velocity")
            .0;
        if !position.is_finite()
            || !raw_rotation.is_finite()
            || !linear.is_finite()
            || !angular.is_finite()
        {
            return Err(InvalidityReason::NonfinitePhysicsState {
                ordinal: recorder.ordinal,
            });
        }
        if !is_contained(recorder.kind, position, request.tray()) {
            return Err(InvalidityReason::LeftTray {
                ordinal: recorder.ordinal,
            });
        }
        let orientation = continuous_orientation(recorder.samples.last(), raw_rotation);
        if recorder.samples.len() == request.validity().watchdog_steps as usize {
            return Err(InvalidityReason::RecordOverflow);
        }
        recorder
            .samples
            .push(TrajectorySample::new(fixed_step, position, orientation));
        recorder.last_face = observe_face(recorder.kind, orientation, request.validity());
        recorder.linear_speed = linear.length();
        recorder.angular_speed = angular.length();
        update_contact_diagnostics(hidden, entity, recorder, fixed_step);
        let support = current_support(hidden, &positions, index, entity, request.validity());
        update_stability(recorder, support, request.validity());
    }
    Ok(())
}

fn update_contact_diagnostics(
    hidden: &HiddenWorld,
    entity: Entity,
    recorder: &mut DieRecorder,
    fixed_step: u32,
) {
    let contacts = hidden
        .app
        .world()
        .get::<CollidingEntities>(entity)
        .expect("collision tracking");
    let current = contacts.iter().copied().collect::<Vec<_>>();
    for contact in current
        .iter()
        .filter(|contact| !recorder.active_contacts.contains(contact))
    {
        if hidden.dice.contains(contact) {
            recorder.contact_diagnostics.dice_contact_events += 1;
        } else if *contact == hidden.floor || hidden.boundaries.contains(contact) {
            recorder.contact_diagnostics.tray_contact_events += 1;
        }
    }
    let count = u16::try_from(current.len()).unwrap_or(u16::MAX);
    recorder.contact_diagnostics.max_simultaneous_contacts = recorder
        .contact_diagnostics
        .max_simultaneous_contacts
        .max(count);
    if !current.is_empty() {
        recorder
            .contact_diagnostics
            .first_contact_step
            .get_or_insert(fixed_step);
        recorder.contact_diagnostics.last_contact_step = Some(fixed_step);
    }
    recorder.active_contacts = current;
}

fn continuous_orientation(previous: Option<&TrajectorySample>, raw: Quat) -> Quat {
    let mut normalized = raw.normalize();
    if let Some(previous) = previous {
        let prior = Quat::from_array(previous.unit_orientation);
        if prior.dot(normalized) < 0.0 {
            normalized = -normalized;
        }
    }
    normalized
}

fn update_stability(
    recorder: &mut DieRecorder,
    support: Result<super::types::SupportClassification, InvalidityReason>,
    policy: super::validity::PhysicalValidityPolicy,
) {
    let resting = recorder.linear_speed <= policy.rest_linear_speed
        && recorder.angular_speed <= policy.rest_angular_speed
        && recorder.last_face.unambiguous;
    recorder.support_invalidity = support.as_ref().err().copied();
    match support {
        Ok(current_support) if resting => {
            let same_window = recorder.stable_face == Some(recorder.last_face.value)
                && recorder.stable_support.as_ref() == Some(&current_support);
            recorder.stable_face = Some(recorder.last_face.value);
            recorder.stable_support = Some(current_support);
            recorder.stable_steps = if same_window {
                recorder.stable_steps.saturating_add(1)
            } else {
                1
            };
        }
        _ => reset_stability(recorder),
    }
}

fn reset_stability(recorder: &mut DieRecorder) {
    recorder.stable_face = None;
    recorder.stable_support = None;
    recorder.stable_steps = 0;
}

fn finish_attempt(
    hidden: HiddenWorld,
    recorders: Vec<DieRecorder>,
    fixed_steps: u32,
    wall_duration: Duration,
    request: &PhysicalBatchRequest,
) -> Result<AcceptedAttempt, (InvalidityReason, AcceptedAttempt)> {
    let positions = die_positions(&hidden);
    let mut recorded_dice = Vec::with_capacity(recorders.len());
    for (index, (entity, recorder)) in hidden.dice.iter().copied().zip(recorders).enumerate() {
        match finish_die(
            &hidden,
            &positions,
            index,
            entity,
            recorder,
            request.validity(),
        ) {
            Ok(die) => recorded_dice.push(die),
            Err(reason) => {
                let attempt = accepted_attempt(hidden, recorded_dice, fixed_steps, wall_duration);
                return Err((reason, attempt));
            }
        }
    }
    Ok(accepted_attempt(
        hidden,
        recorded_dice,
        fixed_steps,
        wall_duration,
    ))
}

fn finish_die(
    hidden: &HiddenWorld,
    positions: &[Vec3],
    index: usize,
    entity: Entity,
    recorder: DieRecorder,
    policy: super::validity::PhysicalValidityPolicy,
) -> Result<RecordedDie, InvalidityReason> {
    let support = current_support(hidden, positions, index, entity, policy)?;
    Ok(RecordedDie {
        ordinal: recorder.ordinal,
        kind: recorder.kind,
        samples: recorder.samples,
        natural_terminal_face: recorder.last_face.value,
        terminal: NaturalTerminalDiagnostics {
            upward_score: recorder.last_face.upward_score,
            runner_up_score: recorder.last_face.runner_up_score,
            support_boundary_margin_radians: recorder.last_face.support_boundary_margin_radians,
            linear_speed: recorder.linear_speed,
            angular_speed: recorder.angular_speed,
            stable_steps: recorder.stable_steps,
            support,
            contacts: recorder.contact_diagnostics,
        },
    })
}

fn die_positions(hidden: &HiddenWorld) -> Vec<Vec3> {
    hidden
        .dice
        .iter()
        .map(|entity| {
            hidden
                .app
                .world()
                .get::<Position>(*entity)
                .expect("position")
                .0
        })
        .collect()
}

fn current_support(
    hidden: &HiddenWorld,
    positions: &[Vec3],
    index: usize,
    entity: Entity,
    policy: super::validity::PhysicalValidityPolicy,
) -> Result<super::types::SupportClassification, InvalidityReason> {
    let contacts = hidden
        .app
        .world()
        .get::<CollidingEntities>(entity)
        .expect("collision tracking");
    let supporting = hidden
        .dice
        .iter()
        .enumerate()
        .filter(|(_, candidate)| contacts.contains(*candidate))
        .filter(|(candidate_index, _)| positions[*candidate_index].y < positions[index].y)
        .map(|(candidate_index, _)| {
            (
                u16::try_from(candidate_index).expect("batch size checked"),
                positions[candidate_index],
            )
        })
        .collect::<Vec<_>>();
    classify_support(
        u16::try_from(index).expect("batch size checked"),
        positions[index],
        contacts.contains(&hidden.floor),
        &supporting,
        policy,
    )
}

fn accepted_attempt(
    hidden: HiddenWorld,
    dice: Vec<RecordedDie>,
    fixed_steps: u32,
    wall_duration: Duration,
) -> AcceptedAttempt {
    AcceptedAttempt {
        dice,
        fixed_steps,
        wall_duration,
        construction_duration: hidden.construction_duration,
        entity_count: hidden.entity_count,
    }
}

fn failed_attempt(
    hidden: HiddenWorld,
    recorders: Vec<DieRecorder>,
    fixed_steps: u32,
    wall_duration: Duration,
    reason: InvalidityReason,
) -> Result<AcceptedAttempt, (InvalidityReason, AcceptedAttempt)> {
    let dice = recorders
        .into_iter()
        .map(|recorder| RecordedDie {
            ordinal: recorder.ordinal,
            kind: recorder.kind,
            samples: recorder.samples,
            natural_terminal_face: recorder.last_face.value,
            terminal: NaturalTerminalDiagnostics {
                upward_score: recorder.last_face.upward_score,
                runner_up_score: recorder.last_face.runner_up_score,
                support_boundary_margin_radians: recorder.last_face.support_boundary_margin_radians,
                linear_speed: recorder.linear_speed,
                angular_speed: recorder.angular_speed,
                stable_steps: recorder.stable_steps,
                support: super::types::SupportClassification::Tray,
                contacts: recorder.contact_diagnostics,
            },
        })
        .collect();
    Err((
        reason,
        accepted_attempt(hidden, dice, fixed_steps, wall_duration),
    ))
}

fn terminal_watchdog_reason(
    recorders: &[DieRecorder],
    policy: super::validity::PhysicalValidityPolicy,
) -> InvalidityReason {
    if let Some(recorder) = recorders
        .iter()
        .find(|recorder| !recorder.last_face.unambiguous)
    {
        InvalidityReason::AmbiguousUpwardFace {
            ordinal: recorder.ordinal,
        }
    } else if let Some(reason) = recorders
        .iter()
        .filter(|recorder| {
            recorder.linear_speed <= policy.rest_linear_speed
                && recorder.angular_speed <= policy.rest_angular_speed
        })
        .find_map(|recorder| recorder.support_invalidity)
    {
        reason
    } else {
        InvalidityReason::WatchdogExpired
    }
}

#[cfg(test)]
mod tests;

fn attempt_diagnostic(
    request: &PhysicalBatchRequest,
    attempt: u8,
    seed: u64,
    result: &AcceptedAttempt,
    outcome: AttemptOutcome,
) -> AttemptDiagnostic {
    AttemptDiagnostic {
        attempt,
        physical_seed: seed,
        die_count: request.dice().len(),
        fixed_steps: result.fixed_steps,
        simulated_duration: FIXED_STEP * result.fixed_steps,
        wall_clock_duration: result.wall_duration,
        outcome,
    }
}

fn single_failure(request: &PhysicalBatchRequest, reason: InvalidityReason) -> PreparationFailure {
    PreparationFailure {
        attempts: vec![AttemptDiagnostic {
            attempt: 0,
            physical_seed: request.physical_presentation_seed(),
            die_count: request.dice().len(),
            fixed_steps: 0,
            simulated_duration: Duration::ZERO,
            wall_clock_duration: Duration::ZERO,
            outcome: AttemptOutcome::Invalid(reason),
        }],
    }
}

fn count_fixed_step(mut counter: ResMut<FixedStepCounter>) {
    counter.0 += 1;
}

fn attempt_seed(base: u64, attempt: u8) -> u64 {
    mix64(base.wrapping_add(u64::from(attempt).wrapping_mul(0x9E37_79B9_7F4A_7C15)))
}

struct PhysicalRng(u64);

impl PhysicalRng {
    fn new(seed: u64) -> Self {
        Self(mix64(seed))
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mix64(self.0)
    }

    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1_u32 << 24) as f32
    }

    fn signed(&mut self) -> f32 {
        self.range(-1.0, 1.0)
    }

    fn range(&mut self, start: f32, end: f32) -> f32 {
        start + (end - start) * self.unit()
    }
}

fn mix64(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}
