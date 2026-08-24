use std::{collections::BTreeSet, mem::size_of};

use bevy::prelude::{Quat, Vec3};
use droll_gui::physics::{
    AttemptOutcome, DieKind, InvalidityReason, PhysicalBatchRequest, PhysicalTray,
    PhysicalValidityPolicy, TrajectorySample, natural_record_identity, prepare_recorded_batch,
};

const PHASE3_4D6_SEED: u64 = 0x4D6A_1E00_0000_0003;
const PHASE4_MIXED10_SEED: u64 = 0x2D20_8D6A_0000_0004;
const PHASE4_MIXED20_SEED: u64 = 0x4D20_16D6_0000_0004;
const PHASE4_MIXED50_SEED: u64 = 0xAD20_28D6_0000_0004;

#[test]
fn test_phase_0_validity_policy_is_preregistered() {
    let policy = PhysicalValidityPolicy::default();
    assert_eq!(policy.watchdog_steps, 1_080);
    assert_eq!(policy.stable_steps, 36);
    assert_eq!(policy.rest_linear_speed, 0.10);
    assert_eq!(policy.rest_angular_speed, 0.15);
    assert_eq!(policy.face_boundary_guard_radians, 5.0_f32.to_radians());
    assert_eq!(policy.stack_min_horizontal_exposure, 0.30);
    assert_eq!(policy.stack_min_vertical_ordering, 0.20);
    assert_eq!(size_of::<TrajectorySample>(), 32);
}

#[test]
fn test_physical_request_source_excludes_semantic_authority() {
    let request_source = include_str!("../src/physics/recorded/types.rs");
    let policy_source = include_str!("../src/physics/recorded/validity.rs");
    let request = source_declaration(
        request_source,
        "pub struct PhysicalBatchRequest",
        "impl PhysicalBatchRequest",
    );
    let policy = source_declaration(
        policy_source,
        "pub struct PhysicalValidityPolicy",
        "impl Default for PhysicalValidityPolicy",
    );
    for forbidden in [
        "requested",
        "semantic",
        "target",
        "keep",
        "drop",
        "symmetry",
    ] {
        assert!(
            !request.contains(forbidden) && !policy.contains(forbidden),
            "physical request or validity policy contains forbidden authority `{forbidden}`"
        );
    }
    let runner_source = include_str!("../src/physics/recorded/runner.rs");
    for function in [
        "pub fn prepare_recorded_batch",
        "fn spawn_dice",
        "fn record_step",
        "fn update_stability",
        "fn terminal_watchdog_reason",
        "fn attempt_seed",
    ] {
        let body = source_function(runner_source, function);
        for forbidden in [
            "requested",
            "semantic",
            "target_face",
            "keep",
            "drop",
            "symmetry",
        ] {
            assert!(
                !body.contains(forbidden),
                "{function} contains `{forbidden}`"
            );
        }
    }

    let _: fn(
        &PhysicalBatchRequest,
    )
        -> Result<droll_gui::physics::RecordedBatch, droll_gui::physics::PreparationFailure> =
        prepare_recorded_batch;
}

fn source_declaration<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start = source.find(start).expect("source declaration");
    let rest = &source[start..];
    let end = rest.find(end).expect("source implementation");
    &rest[..end]
}

fn source_function<'a>(source: &'a str, function: &str) -> &'a str {
    let start = source.find(function).expect("runner function");
    let rest = &source[start..];
    let end = rest[1..]
        .find("\nfn ")
        .map_or(rest.len(), |offset| offset + 1);
    &rest[..end]
}

#[test]
fn test_retry_discards_whole_batch_and_stops_after_three_attempts() {
    let request = PhysicalBatchRequest::new(vec![DieKind::D6, DieKind::D20], 0xA11C_E55E)
        .with_validity_policy(PhysicalValidityPolicy::default().with_watchdog_steps(1));
    let failure = prepare_recorded_batch(&request).expect_err("one step cannot establish rest");
    assert_eq!(failure.attempts.len(), 3);
    assert_eq!(
        failure
            .attempts
            .iter()
            .map(|attempt| attempt.physical_seed)
            .collect::<BTreeSet<_>>()
            .len(),
        3
    );
    for (index, attempt) in failure.attempts.iter().enumerate() {
        assert_eq!(usize::from(attempt.attempt), index + 1);
        assert_eq!(attempt.die_count, 2);
        assert_eq!(attempt.fixed_steps, 1);
        assert!(matches!(
            attempt.outcome,
            AttemptOutcome::Invalid(
                InvalidityReason::WatchdogExpired | InvalidityReason::AmbiguousUpwardFace { .. }
            )
        ));
    }
}

#[test]
fn test_phase3_retries_only_complete_4d6_batches() {
    let request = PhysicalBatchRequest::new(vec![DieKind::D6; 4], PHASE3_4D6_SEED)
        .with_validity_policy(PhysicalValidityPolicy::default().with_watchdog_steps(1));
    let failure = prepare_recorded_batch(&request).expect_err("one step cannot settle 4d6");
    assert_eq!(failure.attempts.len(), 3);
    assert!(
        failure
            .attempts
            .iter()
            .all(|attempt| attempt.die_count == 4 && attempt.fixed_steps == 1)
    );
    assert_eq!(
        failure
            .attempts
            .iter()
            .map(|attempt| attempt.physical_seed)
            .collect::<BTreeSet<_>>()
            .len(),
        3
    );
}

#[test]
fn test_d6_and_d20_hidden_runs_are_unpaced_normalized_and_in_memory() {
    assert_hidden_runner_is_structurally_unpaced();
    for (kind, seed) in [(DieKind::D6, 0xD6), (DieKind::D20, 0xD20)] {
        let record = prepare_recorded_batch(&PhysicalBatchRequest::new(vec![kind], seed))
            .expect("Phase 0 single-die calibration should settle");
        assert_eq!(record.fixed_step.as_nanos(), 16_666_667);
        assert!(record.attempts.len() <= 3);
        assert_eq!(
            record.attempts.last().expect("accepted attempt").outcome,
            AttemptOutcome::Valid
        );
        assert_eq!(record.dice.len(), 1);
        let die = &record.dice[0];
        assert_eq!(die.kind, kind);
        assert!(!die.samples.is_empty());
        assert!(die.natural_terminal_face >= 1);
        assert!(die.natural_terminal_face <= if kind == DieKind::D6 { 6 } else { 20 });
        for (index, sample) in die.samples.iter().enumerate() {
            assert_eq!(sample.fixed_step as usize, index + 1);
            let orientation = Quat::from_array(sample.unit_orientation);
            assert!((orientation.length() - 1.0).abs() < 1.0e-5);
            if let Some(previous) = index.checked_sub(1).map(|prior| &die.samples[prior]) {
                assert!(Quat::from_array(previous.unit_orientation).dot(orientation) >= 0.0);
            }
        }
        assert_calibration_metrics(&record, kind);
    }
}

fn assert_hidden_runner_is_structurally_unpaced() {
    let runner_source = include_str!("../src/physics/recorded/runner.rs");
    let hidden_world = source_function(runner_source, "fn build_hidden_world");
    let hidden_run = source_function(runner_source, "fn run_attempt");
    assert!(hidden_world.contains("MinimalPlugins"));
    assert!(hidden_world.contains("TimeUpdateStrategy::ManualDuration(FIXED_STEP)"));
    assert!(hidden_run.contains("hidden.app.update();"));
    for pacing in [
        "sleep(",
        "thread::sleep",
        "ScheduleRunnerPlugin",
        "DefaultPlugins",
    ] {
        assert!(!hidden_world.contains(pacing) && !hidden_run.contains(pacing));
    }
}

fn assert_calibration_metrics(record: &droll_gui::physics::RecordedBatch, kind: DieKind) {
    let die = &record.dice[0];
    let calibration = &record.calibration;
    assert_eq!(calibration.physics_hz, 60);
    assert_eq!(calibration.recording_hz, 60);
    assert_eq!(calibration.trajectory_sample_count, die.samples.len());
    assert_eq!(
        calibration.raw_trajectory_payload_bytes,
        die.samples.len() * 32
    );
    assert!(calibration.trajectory_capacity_bytes >= calibration.raw_trajectory_payload_bytes);
    assert!(calibration.record_container_bytes >= calibration.trajectory_capacity_bytes);
    assert!(calibration.world_entity_count >= 6);
    assert_eq!(
        calibration.simulated_duration,
        record.fixed_step * calibration.fixed_steps
    );
    assert_eq!(die.samples.len(), calibration.fixed_steps as usize);
    assert!(die.terminal.contacts.tray_contact_events >= 1);
    assert!(die.terminal.contacts.max_simultaneous_contacts >= 1);
    assert!(die.terminal.contacts.first_contact_step.is_some());
    assert!(die.terminal.contacts.last_contact_step.is_some());
    eprintln!(
        "phase0-calibration kind={kind:?} attempts={} outcome={:?} simulated={:?} wall={:?} steps={} samples={} raw_bytes={} capacity_bytes={} container_bytes={} construction={:?} entities={} app_bytes={}",
        record.attempts.len(),
        record.attempts.last().expect("accepted attempt").outcome,
        calibration.simulated_duration,
        calibration.wall_clock_duration,
        calibration.fixed_steps,
        calibration.trajectory_sample_count,
        calibration.raw_trajectory_payload_bytes,
        calibration.trajectory_capacity_bytes,
        calibration.record_container_bytes,
        calibration.world_construction_duration,
        calibration.world_entity_count,
        calibration.app_stack_bytes,
    );
}

#[test]
fn test_identical_physical_request_repeats_identical_natural_record() {
    let request = PhysicalBatchRequest::new(vec![DieKind::D6], 0x51_51_51);
    let first = prepare_recorded_batch(&request).expect("first physical record");
    let second = prepare_recorded_batch(&request).expect("second physical record");
    assert_eq!(first.dice, second.dice);
    assert_eq!(first.attempts.len(), second.attempts.len());
    for (left, right) in first.attempts.iter().zip(&second.attempts) {
        assert_eq!(left.physical_seed, right.physical_seed);
        assert_eq!(left.fixed_steps, right.fixed_steps);
        assert_eq!(left.outcome, right.outcome);
    }
}

#[test]
fn test_phase3_4d6_batch_is_nonoverlapping_shared_and_genuinely_interacting() {
    let record = prepare_recorded_batch(&PhysicalBatchRequest::new(
        vec![DieKind::D6; 4],
        PHASE3_4D6_SEED,
    ))
    .expect("interacting 4d6 checkpoint record");
    assert_phase3_attempts_are_complete_and_locally_valid(&record);
    assert_eq!(record.dice.len(), 4);
    assert_eq!(
        record
            .dice
            .iter()
            .map(|die| die.ordinal)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
    let closest_spawn_distance = assert_nonoverlapping_initial_states(&record);
    assert!(record.contacts.dice_contact_interactions >= 2);
    assert!(!record.contacts.dice_contact_samples.is_empty());
    assert_contact_samples_are_ordered(&record);
    assert!(meaningful_contact_pairs(&record).len() >= 2);
    let strongest = record
        .contacts
        .strongest_dice_contact
        .expect("strongest genuine dice contact");
    assert!(strongest.normal_impulse > 0.0);
    assert!(strongest.approach_speed > 0.0);
    assert!(record.dice.iter().all(|die| {
        die.samples.len()
            == usize::try_from(record.calibration.fixed_steps).expect("bounded fixed steps")
    }));
    report_phase3_checkpoint(&record, closest_spawn_distance);
}

#[test]
fn test_phase4_mixed10_is_nonoverlapping_valid_and_genuinely_mixed() {
    let request = PhysicalBatchRequest::new(phase4_composition(10), PHASE4_MIXED10_SEED)
        .with_tray(phase4_diagnostic_tray(10));
    let outcome = repeatable_native_outcome(&request);
    match outcome {
        Ok(record) => {
            assert_valid_batch(&record, 10);
            assert_nonoverlapping_initial_states(&record);
            assert_contact_samples_are_ordered(&record);
            let kinds = meaningful_pair_kinds(&record);
            assert!(kinds.contains(&(DieKind::D6, DieKind::D6)));
            assert!(kinds.contains(&(DieKind::D6, DieKind::D20)));
            assert!(record.contacts.max_simultaneous_dice_pairs >= 2);
            report_phase4_checkpoint("mixed10", PHASE4_MIXED10_SEED, &record);
        }
        Err(failure) => {
            assert_bounded_native_exhaustion("mixed10", PHASE4_MIXED10_SEED, 10, &failure)
        }
    }
}

#[test]
fn test_phase4_mixed20_is_bounded_valid_and_interacting() {
    let request = PhysicalBatchRequest::new(phase4_composition(20), PHASE4_MIXED20_SEED)
        .with_tray(phase4_diagnostic_tray(20));
    let outcome = repeatable_native_outcome(&request);
    match outcome {
        Ok(record) => {
            assert_valid_batch(&record, 20);
            assert_nonoverlapping_initial_states(&record);
            assert_contact_samples_are_ordered(&record);
            assert!(meaningful_pair_kinds(&record).contains(&(DieKind::D6, DieKind::D20)));
            report_phase4_checkpoint("mixed20", PHASE4_MIXED20_SEED, &record);
        }
        Err(failure) => {
            assert_bounded_native_exhaustion("mixed20", PHASE4_MIXED20_SEED, 20, &failure)
        }
    }
}

#[test]
fn test_phase4_mixed50_diagnostic_is_bounded() {
    let request = PhysicalBatchRequest::new(phase4_composition(50), PHASE4_MIXED50_SEED)
        .with_tray(phase4_diagnostic_tray(50));
    match prepare_recorded_batch(&request) {
        Ok(record) => {
            assert_valid_batch(&record, 50);
            assert_nonoverlapping_initial_states(&record);
            report_phase4_checkpoint("mixed50", PHASE4_MIXED50_SEED, &record);
        }
        Err(failure) => {
            assert_eq!(failure.attempts.len(), 3);
            assert!(failure.attempts.iter().all(|attempt| {
                attempt.die_count == 50 && matches!(attempt.outcome, AttemptOutcome::Invalid(_))
            }));
            eprintln!("phase4-mixed50 bounded_failure={:?}", failure.attempts);
        }
    }
}

fn phase4_composition(count: usize) -> Vec<DieKind> {
    let d20_count = count / 5;
    std::iter::repeat_n(DieKind::D20, d20_count)
        .chain(std::iter::repeat_n(DieKind::D6, count - d20_count))
        .collect()
}

fn phase4_diagnostic_tray(count: usize) -> PhysicalTray {
    if count <= 10 {
        PhysicalTray {
            width: 10.0,
            depth: 8.0,
            wall_height: 1.0,
        }
    } else if count <= 20 {
        PhysicalTray {
            width: 14.0,
            depth: 12.0,
            wall_height: 1.0,
        }
    } else {
        PhysicalTray {
            width: 18.0,
            depth: 14.0,
            wall_height: 1.0,
        }
    }
}

fn repeatable_native_outcome(
    request: &PhysicalBatchRequest,
) -> Result<droll_gui::physics::RecordedBatch, droll_gui::physics::PreparationFailure> {
    let first = prepare_recorded_batch(request);
    let second = prepare_recorded_batch(request);
    assert_same_native_physical_outcome(&first, &second);
    first
}

fn assert_same_native_physical_outcome(
    first: &Result<droll_gui::physics::RecordedBatch, droll_gui::physics::PreparationFailure>,
    second: &Result<droll_gui::physics::RecordedBatch, droll_gui::physics::PreparationFailure>,
) {
    let (first_attempts, second_attempts) = match (first, second) {
        (Ok(first), Ok(second)) => {
            assert_eq!(first.fixed_step, second.fixed_step);
            assert_eq!(first.tray, second.tray);
            assert_eq!(first.dice, second.dice);
            assert_eq!(first.contacts, second.contacts);
            assert_eq!(
                first.calibration.fixed_steps,
                second.calibration.fixed_steps
            );
            (first.attempts.as_slice(), second.attempts.as_slice())
        }
        (Err(first), Err(second)) => (first.attempts.as_slice(), second.attempts.as_slice()),
        _ => panic!("identical physical requests changed accepted/exhausted outcome"),
    };
    assert_eq!(first_attempts.len(), second_attempts.len());
    for (first, second) in first_attempts.iter().zip(second_attempts) {
        assert_eq!(first.attempt, second.attempt);
        assert_eq!(first.physical_seed, second.physical_seed);
        assert_eq!(first.die_count, second.die_count);
        assert_eq!(first.fixed_steps, second.fixed_steps);
        assert_eq!(first.simulated_duration, second.simulated_duration);
        assert_eq!(
            first.trajectory_sample_count,
            second.trajectory_sample_count
        );
        assert_eq!(
            first.raw_trajectory_payload_bytes,
            second.raw_trajectory_payload_bytes
        );
        assert_eq!(
            first.trajectory_capacity_bytes,
            second.trajectory_capacity_bytes
        );
        assert_eq!(first.record_container_bytes, second.record_container_bytes);
        assert_eq!(
            first.dice_contact_sample_count,
            second.dice_contact_sample_count
        );
        assert_eq!(
            first.dice_contact_interactions,
            second.dice_contact_interactions
        );
        assert_eq!(
            first.max_simultaneous_dice_pairs,
            second.max_simultaneous_dice_pairs
        );
        assert_eq!(first.outcome, second.outcome);
    }
}

fn assert_bounded_native_exhaustion(
    name: &str,
    base_seed: u64,
    die_count: usize,
    failure: &droll_gui::physics::PreparationFailure,
) {
    assert_eq!(failure.attempts.len(), 3);
    for (index, attempt) in failure.attempts.iter().enumerate() {
        let attempt_number = u8::try_from(index + 1).expect("three attempts");
        assert_eq!(attempt.attempt, attempt_number);
        assert_eq!(attempt.die_count, die_count);
        assert_eq!(
            attempt.physical_seed,
            expected_attempt_seed(base_seed, attempt_number)
        );
        assert!(
            (1..=PhysicalValidityPolicy::default().watchdog_steps).contains(&attempt.fixed_steps)
        );
        assert_eq!(
            attempt.simulated_duration,
            record_step_duration() * attempt.fixed_steps
        );
        assert!(matches!(
            attempt.outcome,
            AttemptOutcome::Invalid(
                InvalidityReason::LeftTray { .. }
                    | InvalidityReason::NonfinitePhysicsState { .. }
                    | InvalidityReason::AmbiguousUpwardFace { .. }
                    | InvalidityReason::UnsupportedDie { .. }
                    | InvalidityReason::EdgeOrCornerTraySupport { .. }
                    | InvalidityReason::UnreadableOrPathologicalStack { .. }
                    | InvalidityReason::WatchdogExpired
                    | InvalidityReason::RecordOverflow
            )
        ));
    }
    // Fixture exhaustion is portable native evidence only. Stage D still requires
    // exactly 40 requested preparations per normal class, where every exhausted
    // <=20-die preparation remains independently blocking unless the owner revises it.
    eprintln!("phase4-{name} bounded_failure={:?}", failure.attempts);
}

fn record_step_duration() -> std::time::Duration {
    std::time::Duration::from_nanos(16_666_667)
}

fn expected_attempt_seed(base: u64, attempt: u8) -> u64 {
    mix64(base.wrapping_add(u64::from(attempt).wrapping_mul(0x9E37_79B9_7F4A_7C15)))
}

fn mix64(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn assert_valid_batch(record: &droll_gui::physics::RecordedBatch, count: usize) {
    assert!(!record.attempts.is_empty() && record.attempts.len() <= 3);
    assert_eq!(record.dice.len(), count);
    assert_eq!(
        record.attempts.last().unwrap().outcome,
        AttemptOutcome::Valid
    );
    assert!(
        record
            .attempts
            .iter()
            .all(|attempt| attempt.die_count == count)
    );
    assert_eq!(
        record
            .dice
            .iter()
            .map(|die| die.ordinal)
            .collect::<Vec<_>>(),
        (0..u16::try_from(count).unwrap()).collect::<Vec<_>>()
    );
    assert_eq!(
        record.dice.iter().map(|die| die.kind).collect::<Vec<_>>(),
        phase4_composition(count)
    );
    assert!(record.dice.iter().all(|die| {
        (1..=die.kind.face_count()).contains(&die.natural_terminal_face)
            && die.terminal.stable_steps >= 36
    }));
}

fn meaningful_pair_kinds(
    record: &droll_gui::physics::RecordedBatch,
) -> BTreeSet<(DieKind, DieKind)> {
    meaningful_contact_pairs(record)
        .into_iter()
        .map(|(first, second)| {
            let first_kind = record.dice[usize::from(first)].kind;
            let second_kind = record.dice[usize::from(second)].kind;
            if first_kind == DieKind::D20 && second_kind == DieKind::D6 {
                (second_kind, first_kind)
            } else {
                (first_kind, second_kind)
            }
        })
        .collect()
}

fn all_pair_kinds(record: &droll_gui::physics::RecordedBatch) -> BTreeSet<(DieKind, DieKind)> {
    record
        .contacts
        .dice_contact_samples
        .iter()
        .map(|sample| {
            let mut kinds = [
                record.dice[usize::from(sample.first_ordinal)].kind,
                record.dice[usize::from(sample.second_ordinal)].kind,
            ];
            kinds.sort_unstable();
            (kinds[0], kinds[1])
        })
        .collect()
}

fn report_phase4_checkpoint(
    name: &str,
    base_seed: u64,
    record: &droll_gui::physics::RecordedBatch,
) {
    eprintln!(
        "phase4-{name} id={:016x} base={base_seed:#018x} attempts={:?} faces={:?} duration={:?} steps={} samples={} raw_bytes={} capacity_bytes={} container_bytes={} wall={:?} interactions={} contact_samples={} max_pairs={} pair_kinds={:?} all_pair_kinds={:?} strongest={:?}",
        natural_record_identity(record),
        record.attempts,
        record
            .dice
            .iter()
            .map(|die| die.natural_terminal_face)
            .collect::<Vec<_>>(),
        record.calibration.simulated_duration,
        record.calibration.fixed_steps,
        record.calibration.trajectory_sample_count,
        record.calibration.raw_trajectory_payload_bytes,
        record.calibration.trajectory_capacity_bytes,
        record.calibration.record_container_bytes,
        record.calibration.wall_clock_duration,
        record.contacts.dice_contact_interactions,
        record.contacts.dice_contact_samples.len(),
        record.contacts.max_simultaneous_dice_pairs,
        meaningful_pair_kinds(record),
        all_pair_kinds(record),
        record.contacts.strongest_dice_contact,
    );
}

fn assert_phase3_attempts_are_complete_and_locally_valid(
    record: &droll_gui::physics::RecordedBatch,
) {
    assert!(!record.attempts.is_empty());
    assert!(record.attempts.len() <= 3);
    for (index, attempt) in record.attempts.iter().enumerate() {
        assert_eq!(usize::from(attempt.attempt), index + 1);
        assert_eq!(attempt.die_count, 4);
        assert!(attempt.fixed_steps > 0);
    }
    assert!(
        record.attempts[..record.attempts.len() - 1]
            .iter()
            .all(|attempt| matches!(attempt.outcome, AttemptOutcome::Invalid(_)))
    );
    assert_eq!(
        record.attempts.last().expect("accepted attempt").outcome,
        AttemptOutcome::Valid
    );
}

fn report_phase3_checkpoint(record: &droll_gui::physics::RecordedBatch, closest: f32) {
    let strongest = record
        .contacts
        .strongest_dice_contact
        .expect("strongest genuine dice contact");
    eprintln!(
        "phase3-4d6 id={:016x} seed={PHASE3_4D6_SEED:#018x} accepted={:#018x} attempts={:?} faces={:?} steps={} samples_per_die={} simulated={:?} closest_spawn={closest:.6} interactions={} contact_steps={} meaningful_pairs={:?} strongest_time={:?} strongest={strongest:?} terminals={:?} initials={:?}",
        natural_record_identity(record),
        record
            .attempts
            .last()
            .expect("accepted attempt")
            .physical_seed,
        record.attempts,
        record
            .dice
            .iter()
            .map(|die| die.natural_terminal_face)
            .collect::<Vec<_>>(),
        record.calibration.fixed_steps,
        record.dice[0].samples.len(),
        record.calibration.simulated_duration,
        record.contacts.dice_contact_interactions,
        record.contacts.dice_contact_samples.len(),
        meaningful_contact_pairs(record),
        record.fixed_step * strongest.fixed_step,
        record
            .dice
            .iter()
            .map(|die| &die.terminal)
            .collect::<Vec<_>>(),
        record
            .dice
            .iter()
            .map(|die| die.initial)
            .collect::<Vec<_>>(),
    );
}

fn meaningful_contact_pairs(record: &droll_gui::physics::RecordedBatch) -> BTreeSet<(u16, u16)> {
    record
        .contacts
        .dice_contact_samples
        .iter()
        .filter(|sample| sample.normal_impulse > 0.05 && sample.approach_speed > 0.10)
        .map(|sample| (sample.first_ordinal, sample.second_ordinal))
        .collect()
}

fn assert_nonoverlapping_initial_states(record: &droll_gui::physics::RecordedBatch) -> f32 {
    let mut closest = f32::INFINITY;
    for first in 0..record.dice.len() {
        for second in (first + 1)..record.dice.len() {
            let first_position = Vec3::from_array(record.dice[first].initial.world_position);
            let second_position = Vec3::from_array(record.dice[second].initial.world_position);
            let distance = first_position.distance(second_position);
            closest = closest.min(distance);
            let minimum_separation =
                record.dice[first].kind.circumradius() + record.dice[second].kind.circumradius();
            assert!(
                distance > minimum_separation,
                "initial bounding spheres overlap for ordinals {first}/{second}"
            );
        }
    }
    closest
}

fn assert_contact_samples_are_ordered(record: &droll_gui::physics::RecordedBatch) {
    for sample in &record.contacts.dice_contact_samples {
        assert!(sample.first_ordinal < sample.second_ordinal);
        assert!(usize::from(sample.second_ordinal) < record.dice.len());
        assert!(sample.fixed_step <= record.calibration.fixed_steps);
        assert!(Vec3::from_array(sample.world_point).is_finite());
        assert!((Vec3::from_array(sample.world_normal).length() - 1.0).abs() < 1.0e-4);
        assert!(sample.normal_impulse >= 0.0);
        assert!(sample.approach_speed >= 0.0);
    }
    assert!(
        record
            .contacts
            .dice_contact_samples
            .windows(2)
            .all(|samples| {
                (
                    samples[0].fixed_step,
                    samples[0].first_ordinal,
                    samples[0].second_ordinal,
                ) <= (
                    samples[1].fixed_step,
                    samples[1].first_ordinal,
                    samples[1].second_ordinal,
                )
            })
    );
}

#[test]
fn test_empty_batch_fails_without_starting_physics() {
    let failure = prepare_recorded_batch(&PhysicalBatchRequest::new(Vec::new(), 7))
        .expect_err("an empty physical batch is invalid");
    assert_eq!(failure.attempts.len(), 1);
    assert_eq!(failure.attempts[0].attempt, 0);
    assert_eq!(
        failure.attempts[0].outcome,
        AttemptOutcome::Invalid(InvalidityReason::EmptyBatch)
    );
}
