use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

use avian3d::collision::contact_types::PackedFeatureId;
use avian3d::prelude::*;
use bevy::{app::FixedPostUpdate, prelude::*, time::TimeUpdateStrategy};

use crate::dice::{d6_geometry, d20_geometry};

use super::{
    types::{
        AttemptDiagnostic, AttemptOutcome, BatchContactDiagnostics, CalibrationMetrics,
        ContactDiagnostics, DiceContactSample, DieKind, FIXED_STEP, InitialPhysicalState,
        InvalidityReason, MAX_TOTAL_ATTEMPTS, NaturalTerminalDiagnostics, PhysicalBatchRequest,
        PreparationFailure, RecordedBatch, RecordedDie, TrajectorySample,
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
    initial_states: Vec<InitialPhysicalState>,
    floor: Entity,
    boundaries: Vec<Entity>,
    construction_duration: Duration,
    entity_count: u32,
}

struct DieRecorder {
    ordinal: u16,
    kind: DieKind,
    initial: InitialPhysicalState,
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
    contacts: BatchContactDiagnostics,
    fixed_steps: u32,
    wall_duration: Duration,
    construction_duration: Duration,
    entity_count: u32,
}

type AttemptFailure = (InvalidityReason, Box<AcceptedAttempt>);

#[derive(Default)]
struct BatchContactRecorder {
    active_pairs: BTreeSet<(u16, u16)>,
    diagnostics: BatchContactDiagnostics,
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
                    contacts: accepted.contacts,
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
) -> Result<AcceptedAttempt, AttemptFailure> {
    let hidden = build_hidden_world(request, seed);
    let mut recorders = create_recorders(request, &hidden.initial_states);
    let mut batch_contacts = BatchContactRecorder::default();
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
                batch_contacts,
                fixed_step,
                simulation_started.elapsed(),
                InvalidityReason::FixedStepDidNotAdvanceExactlyOnce,
            );
        }
        record_batch_contacts(&hidden, fixed_step, &mut batch_contacts);
        if let Err(reason) = record_step(&hidden, &mut recorders, fixed_step, request) {
            return failed_attempt(
                hidden,
                recorders,
                batch_contacts,
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
                batch_contacts,
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
        batch_contacts,
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
    let (dice, initial_states) = spawn_dice(&mut app, request.dice(), seed);
    let entity_count = app.world().entities().len();
    HiddenWorld {
        app,
        dice,
        initial_states,
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

fn spawn_dice(
    app: &mut App,
    kinds: &[DieKind],
    seed: u64,
) -> (Vec<Entity>, Vec<InitialPhysicalState>) {
    let mut rng = PhysicalRng::new(seed);
    let phase3_4d6 = kinds.len() == 4 && kinds.iter().all(|kind| *kind == DieKind::D6);
    let launches = kinds
        .iter()
        .enumerate()
        .map(|(index, _)| {
            if phase3_4d6 {
                interacting_4d6_launch(index, &mut rng)
            } else {
                ordinary_launch(index, kinds.len(), &mut rng)
            }
        })
        .collect::<Vec<_>>();
    let entities = kinds
        .iter()
        .copied()
        .zip(launches.iter().copied())
        .map(|(kind, launch)| {
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
                    LinearVelocity(Vec3::from_array(launch.linear_velocity)),
                    AngularVelocity(Vec3::from_array(launch.angular_velocity)),
                    Transform::from_translation(Vec3::from_array(launch.world_position))
                        .with_rotation(Quat::from_array(launch.unit_orientation)),
                ))
                .id()
        })
        .collect();
    (entities, launches)
}

fn ordinary_launch(index: usize, count: usize, rng: &mut PhysicalRng) -> InitialPhysicalState {
    let centered = index as f32 - (count.saturating_sub(1)) as f32 * 0.5;
    let position = Vec3::new(
        centered * 1.15 + rng.range(-0.12, 0.12),
        2.6 + (index % 2) as f32 * 0.35,
        rng.range(-0.55, 0.55),
    );
    let axis = Vec3::new(rng.signed(), rng.signed(), rng.signed())
        .try_normalize()
        .unwrap_or(Vec3::Y);
    let orientation = Quat::from_axis_angle(axis, rng.range(0.2, 5.8));
    let linear_velocity = Vec3::new(rng.range(-1.5, 1.5), 0.2, rng.range(-1.2, 1.2));
    let angular_velocity = Vec3::new(
        rng.range(-8.0, 8.0),
        rng.range(-8.0, 8.0),
        rng.range(-8.0, 8.0),
    );
    initial_state(position, orientation, linear_velocity, angular_velocity)
}

fn interacting_4d6_launch(index: usize, rng: &mut PhysicalRng) -> InitialPhysicalState {
    let base = [
        Vec3::new(-1.05, 3.32, -1.05),
        Vec3::new(1.05, 3.12, -1.05),
        Vec3::new(1.05, 3.38, 1.05),
        Vec3::new(-1.05, 3.18, 1.05),
    ][index];
    let position = base + Vec3::new(rng.range(-0.04, 0.04), 0.0, rng.range(-0.04, 0.04));
    let axis = Vec3::new(rng.signed(), rng.signed(), rng.signed())
        .try_normalize()
        .unwrap_or(Vec3::Y);
    let orientation = Quat::from_axis_angle(axis, rng.range(0.2, 5.8));
    let inward = -Vec3::new(position.x, 0.0, position.z).normalize();
    let tangent = Vec3::new(-inward.z, 0.0, inward.x);
    let linear_velocity = inward * rng.range(2.35, 2.85)
        + tangent * rng.range(-0.35, 0.35)
        + Vec3::Y * rng.range(-0.15, 0.45);
    let angular_velocity = Vec3::new(
        rng.range(-9.0, 9.0),
        rng.range(-9.0, 9.0),
        rng.range(-9.0, 9.0),
    );
    initial_state(position, orientation, linear_velocity, angular_velocity)
}

fn initial_state(
    position: Vec3,
    orientation: Quat,
    linear_velocity: Vec3,
    angular_velocity: Vec3,
) -> InitialPhysicalState {
    InitialPhysicalState {
        world_position: position.to_array(),
        unit_orientation: orientation.to_array(),
        linear_velocity: linear_velocity.to_array(),
        angular_velocity: angular_velocity.to_array(),
    }
}

fn create_recorders(
    request: &PhysicalBatchRequest,
    initial_states: &[InitialPhysicalState],
) -> Vec<DieRecorder> {
    request
        .dice()
        .iter()
        .copied()
        .zip(initial_states.iter().copied())
        .enumerate()
        .map(|(ordinal, (kind, initial))| DieRecorder {
            ordinal: u16::try_from(ordinal).expect("batch size checked"),
            kind,
            initial,
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

fn record_batch_contacts(
    hidden: &HiddenWorld,
    fixed_step: u32,
    recorder: &mut BatchContactRecorder,
) {
    let graph = hidden.app.world().resource::<ContactGraph>();
    let mut current_pairs = BTreeSet::new();
    for first in 0..hidden.dice.len() {
        for second in (first + 1)..hidden.dice.len() {
            let Some((_, pair)) = graph.get(hidden.dice[first], hidden.dice[second]) else {
                continue;
            };
            if !pair.is_touching() {
                continue;
            }
            let ordinals = (
                u16::try_from(first).expect("batch size checked"),
                u16::try_from(second).expect("batch size checked"),
            );
            current_pairs.insert(ordinals);
            if let Some(sample) = strongest_pair_sample(pair, ordinals, fixed_step, hidden) {
                update_strongest_contact(&mut recorder.diagnostics, sample);
                recorder.diagnostics.dice_contact_samples.push(sample);
            }
        }
    }
    recorder.diagnostics.dice_contact_interactions +=
        u32::try_from(current_pairs.difference(&recorder.active_pairs).count()).unwrap_or(u32::MAX);
    recorder.active_pairs = current_pairs;
}

fn strongest_pair_sample(
    pair: &ContactPair,
    ordinals: (u16, u16),
    fixed_step: u32,
    hidden: &HiddenWorld,
) -> Option<DiceContactSample> {
    let (normal, point) = pair
        .manifolds
        .iter()
        .flat_map(|manifold| {
            manifold
                .points
                .iter()
                .map(move |point| (manifold.normal, point))
        })
        .max_by(|(_, left), (_, right)| {
            left.normal_impulse
                .abs()
                .total_cmp(&right.normal_impulse.abs())
                .then_with(|| (-left.normal_speed).total_cmp(&-right.normal_speed))
        })?;
    let world_normal = if pair.collider1 == hidden.dice[usize::from(ordinals.0)] {
        normal
    } else {
        -normal
    };
    Some(DiceContactSample {
        fixed_step,
        first_ordinal: ordinals.0,
        second_ordinal: ordinals.1,
        world_point: point.point.to_array(),
        world_normal: world_normal.to_array(),
        normal_impulse: point.normal_impulse.abs(),
        approach_speed: (-point.normal_speed).max(0.0),
    })
}

fn update_strongest_contact(diagnostics: &mut BatchContactDiagnostics, sample: DiceContactSample) {
    let replace = diagnostics.strongest_dice_contact.is_none_or(|strongest| {
        sample.normal_impulse > strongest.normal_impulse
            || (sample.normal_impulse == strongest.normal_impulse
                && sample.approach_speed > strongest.approach_speed)
    });
    if replace {
        diagnostics.strongest_dice_contact = Some(sample);
    }
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
        let support = current_support(
            hidden,
            &positions,
            index,
            entity,
            recorder.kind,
            request.validity(),
        );
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
    batch_contacts: BatchContactRecorder,
    fixed_steps: u32,
    wall_duration: Duration,
    request: &PhysicalBatchRequest,
) -> Result<AcceptedAttempt, AttemptFailure> {
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
                let attempt = accepted_attempt(
                    hidden,
                    recorded_dice,
                    batch_contacts.diagnostics,
                    fixed_steps,
                    wall_duration,
                );
                return Err((reason, Box::new(attempt)));
            }
        }
    }
    Ok(accepted_attempt(
        hidden,
        recorded_dice,
        batch_contacts.diagnostics,
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
    let support = current_support(hidden, positions, index, entity, recorder.kind, policy)?;
    Ok(RecordedDie {
        ordinal: recorder.ordinal,
        kind: recorder.kind,
        initial: recorder.initial,
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
    kind: DieKind,
    policy: super::validity::PhysicalValidityPolicy,
) -> Result<super::types::SupportClassification, InvalidityReason> {
    let contacts = hidden
        .app
        .world()
        .get::<CollidingEntities>(entity)
        .expect("collision tracking");
    let supporting_entities = hidden
        .dice
        .iter()
        .enumerate()
        .filter(|(_, candidate)| contacts.contains(*candidate))
        .filter(|(candidate_index, _)| positions[*candidate_index].y < positions[index].y)
        .map(|(candidate_index, candidate)| (candidate_index, *candidate))
        .collect::<Vec<_>>();
    let supporting = supporting_entities
        .iter()
        .map(|(candidate_index, _)| {
            (
                u16::try_from(*candidate_index).expect("batch size checked"),
                positions[*candidate_index],
            )
        })
        .collect::<Vec<_>>();
    let ordinal = u16::try_from(index).expect("batch size checked");
    let touches_floor = contacts.contains(&hidden.floor);
    if kind == DieKind::D6
        && touches_floor
        && !d6_contact_is_face_bearing(hidden, entity, hidden.floor)
    {
        let stack = classify_support(ordinal, positions[index], false, &supporting, policy);
        let face_bearing_stack = !supporting_entities.is_empty()
            && supporting_entities
                .iter()
                .all(|(_, support)| d6_contact_is_face_bearing(hidden, entity, *support));
        return match stack {
            Ok(classification) if face_bearing_stack => Ok(classification),
            _ => Err(InvalidityReason::EdgeOrCornerTraySupport { ordinal }),
        };
    }
    classify_support(
        ordinal,
        positions[index],
        touches_floor,
        &supporting,
        policy,
    )
}

fn d6_contact_is_face_bearing(hidden: &HiddenWorld, die: Entity, support: Entity) -> bool {
    let graph = hidden.app.world().resource::<ContactGraph>();
    let Some((_, pair)) = graph.get(die, support) else {
        return false;
    };
    if !pair.is_touching() {
        return false;
    }
    let features = pair.manifolds.iter().flat_map(|manifold| {
        manifold.points.iter().map(|point| {
            if pair.collider1 == die {
                point.feature_id1
            } else {
                point.feature_id2
            }
        })
    });
    d6_tray_features_are_face_bearing(features)
}

fn d6_tray_features_are_face_bearing(features: impl IntoIterator<Item = PackedFeatureId>) -> bool {
    let mut vertices = BTreeSet::new();
    for feature in features {
        if feature.is_face() {
            return true;
        }
        if feature.is_vertex() {
            vertices.insert(feature.0);
        }
    }
    // A cube face is two-dimensional: contact with a plane exposes either a
    // face feature or at least three distinct coplanar vertices. An edge can
    // expose at most two vertices and a corner exactly one, so this geometric
    // dimension test needs no outcome-tuned angular or distance threshold.
    vertices.len() >= 3
}

fn accepted_attempt(
    hidden: HiddenWorld,
    dice: Vec<RecordedDie>,
    contacts: BatchContactDiagnostics,
    fixed_steps: u32,
    wall_duration: Duration,
) -> AcceptedAttempt {
    AcceptedAttempt {
        dice,
        contacts,
        fixed_steps,
        wall_duration,
        construction_duration: hidden.construction_duration,
        entity_count: hidden.entity_count,
    }
}

fn failed_attempt(
    hidden: HiddenWorld,
    recorders: Vec<DieRecorder>,
    batch_contacts: BatchContactRecorder,
    fixed_steps: u32,
    wall_duration: Duration,
    reason: InvalidityReason,
) -> Result<AcceptedAttempt, AttemptFailure> {
    let dice = recorders
        .into_iter()
        .map(|recorder| RecordedDie {
            ordinal: recorder.ordinal,
            kind: recorder.kind,
            initial: recorder.initial,
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
        Box::new(accepted_attempt(
            hidden,
            dice,
            batch_contacts.diagnostics,
            fixed_steps,
            wall_duration,
        )),
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
