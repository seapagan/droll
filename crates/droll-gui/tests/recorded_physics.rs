use std::{collections::BTreeSet, mem::size_of};

use bevy::prelude::Quat;
use droll_gui::physics::{
    AttemptOutcome, DieKind, InvalidityReason, PhysicalBatchRequest, PhysicalValidityPolicy,
    TrajectorySample, prepare_recorded_batch,
};

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
fn test_d6_and_d20_hidden_runs_are_unpaced_normalized_and_in_memory() {
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
        assert!(calibration.simulated_duration > calibration.wall_clock_duration);
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
