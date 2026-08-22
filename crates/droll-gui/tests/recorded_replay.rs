use std::{collections::BTreeSet, sync::Arc, time::Duration};

use avian3d::prelude::{Collider, RigidBody};
use bevy::{app::App, math::Quat, prelude::*};
use droll_gui::{
    dice::{d6_geometry, d6_solid_symmetries},
    physics::{
        DieKind, FixedD6Presentation, NumberedVisual, PhysicalBatchRequest, PlaybackRoot,
        RecordedPlaybackPlugin, RecordedTrajectoryPlayback, SemanticPresentationMap,
        TrajectorySample, compose_visible_orientation, map_d6_presentation,
        natural_record_identity, prepare_recorded_batch, sample_recorded_transform,
    },
};

const EPSILON: f32 = 1.0e-5;
const PHYSICAL_SEED: u64 = 0xD65A_1E00_0000_0001;

#[test]
fn test_all_36_d6_pairs_use_proper_group_and_target_independent_phase() {
    let proper_ids = d6_solid_symmetries()
        .into_iter()
        .map(|symmetry| symmetry.id)
        .collect::<BTreeSet<_>>();
    for natural in 1..=6 {
        let mut phases = BTreeSet::new();
        for requested in 1..=6 {
            let mapping = map_d6_presentation(natural, requested, 7, PHYSICAL_SEED)
                .expect("all ordered d6 pairs map");
            assert!(proper_ids.contains(&mapping.symmetry_id));
            assert_eq!(
                mapping.phase.index,
                u8::try_from(mapping.phase.identifier % 4).expect("d6 phase")
            );
            let landed = d6_geometry()
                .face(natural)
                .expect("natural face")
                .target_rotation(0.37);
            assert_eq!(
                d6_geometry().upward_face(landed * mapping.symmetry).value,
                requested
            );
            phases.insert((mapping.phase.index, mapping.phase.identifier));
        }
        assert_eq!(phases.len(), 1, "requested value changed free phase");
    }
}

#[test]
fn test_one_immutable_natural_record_maps_all_six_d6_values() {
    let record =
        prepare_recorded_batch(&PhysicalBatchRequest::new(vec![DieKind::D6], PHYSICAL_SEED))
            .expect("one target-blind d6 record");
    let frozen = record.clone();
    let identity = natural_record_identity(&record);
    let natural_face = record.dice[0].natural_terminal_face;
    let mut symmetry_ids = BTreeSet::new();
    for requested in 1..=6 {
        let presentation =
            SemanticPresentationMap::for_single_d6(&record, requested).expect("presentation map");
        let mapping = presentation.d6[0];
        assert_eq!(presentation.natural_record_identity, identity);
        assert_eq!(mapping.natural_face, natural_face);
        assert_eq!(mapping.requested_face, requested);
        assert_eq!(
            mapping.accepted_physical_seed,
            record
                .attempts
                .last()
                .expect("accepted attempt")
                .physical_seed
        );
        assert_visible_transform_contract(&record.dice[0].samples, record.fixed_step, mapping);
        symmetry_ids.insert(mapping.symmetry_id);
        assert_eq!(record, frozen, "pure mapping mutated physical record");
    }
    assert_eq!(symmetry_ids.len(), 6);
    assert_eq!(record.dice[0].samples, frozen.dice[0].samples);
    assert_eq!(record.attempts, frozen.attempts);
    assert_eq!(
        record.dice[0].terminal.contacts,
        frozen.dice[0].terminal.contacts
    );
    eprintln!(
        "phase1-record id={identity:016x} natural_face={natural_face} base_seed={PHYSICAL_SEED:#018x} accepted_seed={:#018x} attempts={} steps={} samples={} simulated={:?} wall={:?}",
        record
            .attempts
            .last()
            .expect("accepted attempt")
            .physical_seed,
        record.attempts.len(),
        record.calibration.fixed_steps,
        record.dice[0].samples.len(),
        record.calibration.simulated_duration,
        record.calibration.wall_clock_duration,
    );
}

#[test]
fn test_recorded_sampler_preserves_endpoints_at_render_cadences() {
    let samples = representative_samples();
    let fixed_step = Duration::from_secs_f64(1.0 / 60.0);
    for render_hz in [30_u32, 60, 90, 120, 144] {
        let frame_step = Duration::from_secs_f64(1.0 / f64::from(render_hz));
        let final_time = fixed_step * u32::try_from(samples.len() - 1).expect("small sample set");
        let mut elapsed = Duration::ZERO;
        let mut previous = Duration::ZERO;
        let mut previous_sample_position = 0.0_f32;
        loop {
            let sampled = sample_recorded_transform(&samples, fixed_step, elapsed)
                .expect("finite trajectory");
            assert!(elapsed >= previous);
            let sample_position = sampled.lower_index as f32 + sampled.alpha;
            assert!(sample_position >= previous_sample_position);
            if elapsed.is_zero() {
                assert_sample_exact(sampled, &samples[0]);
            }
            if sampled.at_end {
                assert_sample_exact(sampled, samples.last().expect("final sample"));
                break;
            }
            previous = elapsed;
            previous_sample_position = sample_position;
            elapsed = elapsed.saturating_add(frame_step);
            assert!(elapsed <= final_time.saturating_add(frame_step));
        }
    }
}

#[test]
fn test_sampler_uses_shortest_arc_slerp_before_fixed_symmetry() {
    let mut samples = representative_samples();
    samples[1].unit_orientation = (-Quat::from_array(samples[1].unit_orientation)).to_array();
    let fixed_step = Duration::from_secs_f64(1.0 / 60.0);
    let sampled =
        sample_recorded_transform(&samples, fixed_step, fixed_step / 2).expect("midpoint sample");
    let first = Quat::from_array(samples[0].unit_orientation);
    let mut second = Quat::from_array(samples[1].unit_orientation);
    if first.dot(second) < 0.0 {
        second = -second;
    }
    assert_same_rotation(sampled.recorded_orientation, first.slerp(second, 0.5));

    let symmetry = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let base = Quat::from_rotation_x(0.31);
    let visible = compose_visible_orientation(sampled.recorded_orientation, symmetry, base);
    assert_same_rotation(visible, sampled.recorded_orientation * symmetry * base);
    assert!(!same_rotation(
        visible,
        symmetry * sampled.recorded_orientation * base
    ));
}

#[test]
fn test_playback_plugin_has_no_visible_physics_components() {
    let samples: Arc<[TrajectorySample]> = representative_samples().into();
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(RecordedPlaybackPlugin);
    let root = app
        .world_mut()
        .spawn((
            PlaybackRoot,
            RecordedTrajectoryPlayback::new(samples, Duration::from_secs_f64(1.0 / 60.0)),
            Transform::IDENTITY,
        ))
        .id();
    let mapping = map_d6_presentation(1, 6, 0, PHYSICAL_SEED).expect("mapping");
    let visual = app
        .world_mut()
        .spawn((
            NumberedVisual,
            FixedD6Presentation(mapping),
            Transform::from_rotation(mapping.symmetry),
        ))
        .id();
    app.world_mut().entity_mut(root).add_child(visual);
    let fixed_before = *app
        .world()
        .get::<FixedD6Presentation>(visual)
        .expect("fixed mapping before first update");
    let symmetry_before = app
        .world()
        .get::<Transform>(visual)
        .expect("fixed symmetry before first update")
        .rotation;
    app.update();
    app.update();

    for entity in [root, visual] {
        assert!(app.world().get::<RigidBody>(entity).is_none());
        assert!(app.world().get::<Collider>(entity).is_none());
    }
    assert_eq!(
        *app.world()
            .get::<FixedD6Presentation>(visual)
            .expect("fixed mapping after playback updates"),
        fixed_before
    );
    assert_eq!(
        app.world()
            .get::<Transform>(visual)
            .expect("fixed symmetry after playback updates")
            .rotation,
        symmetry_before
    );
}

#[test]
fn test_playback_source_has_no_visible_physics() {
    let source = include_str!("../src/physics/recorded/playback.rs");
    for forbidden in ["PhysicsPlugins", "RigidBody", "Collider", "PhysicsSchedule"] {
        assert!(
            !source.contains(forbidden),
            "playback source contains {forbidden}"
        );
    }
}

#[test]
fn test_semantics_cannot_reach_physical_runner_or_consume_physical_rng() {
    let runner = include_str!("../src/physics/recorded/runner.rs");
    for forbidden in [
        "SemanticPresentationMap",
        "D6PresentationMapping",
        "requested_face",
        "FixedD6Presentation",
    ] {
        assert!(!runner.contains(forbidden), "runner contains {forbidden}");
    }
    let presentation = include_str!("../src/physics/recorded/presentation.rs");
    for forbidden in [
        "prepare_recorded_batch",
        "PhysicalBatchRequest",
        "PhysicalRng",
        "run_attempt",
        "retry",
    ] {
        assert!(
            !presentation.contains(forbidden),
            "presentation mapping contains {forbidden}"
        );
    }
}

fn assert_visible_transform_contract(
    samples: &[TrajectorySample],
    fixed_step: Duration,
    mapping: droll_gui::physics::D6PresentationMapping,
) {
    let final_time = fixed_step * u32::try_from(samples.len() - 1).expect("bounded record");
    for elapsed in [
        Duration::ZERO,
        fixed_step / 2,
        final_time / 2,
        final_time,
        final_time.saturating_add(fixed_step * 3),
    ] {
        let sampled = sample_recorded_transform(samples, fixed_step, elapsed).expect("sample");
        let visible = compose_visible_orientation(
            sampled.recorded_orientation,
            mapping.symmetry,
            Quat::IDENTITY,
        );
        assert_same_rotation(visible, sampled.recorded_orientation * mapping.symmetry);
        assert_occupied_shape_unchanged(sampled.recorded_orientation, mapping.symmetry);
        if elapsed >= final_time {
            assert_eq!(
                d6_geometry().upward_face(visible).value,
                mapping.requested_face
            );
        }
    }
}

fn assert_occupied_shape_unchanged(recorded: Quat, symmetry: Quat) {
    let vertices = d6_geometry().vertices;
    let baseline = vertices.map(|vertex| recorded * vertex);
    let mapped = vertices.map(|vertex| recorded * symmetry * vertex);
    for vertex in baseline {
        assert!(
            mapped
                .iter()
                .any(|candidate| candidate.distance(vertex) < EPSILON)
        );
    }
}

fn representative_samples() -> Vec<TrajectorySample> {
    [
        (Vec3::new(-1.0, 2.0, 0.5), Quat::from_rotation_x(0.2)),
        (
            Vec3::new(-0.2, 1.3, 0.1),
            Quat::from_euler(EulerRot::XYZ, 0.7, -0.4, 0.3),
        ),
        (
            Vec3::new(0.4, 0.7, -0.2),
            Quat::from_euler(EulerRot::XYZ, 1.1, 0.2, -0.6),
        ),
        (Vec3::new(0.8, 0.5, -0.3), Quat::from_rotation_y(1.2)),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (position, rotation))| TrajectorySample {
        fixed_step: u32::try_from(index + 1).expect("small sample set"),
        world_position: position.to_array(),
        unit_orientation: rotation.to_array(),
    })
    .collect()
}

fn assert_sample_exact(
    sampled: droll_gui::physics::SampledPlaybackTransform,
    expected: &TrajectorySample,
) {
    assert_eq!(sampled.world_position.to_array(), expected.world_position);
    assert_same_rotation(
        sampled.recorded_orientation,
        Quat::from_array(expected.unit_orientation),
    );
}

fn assert_same_rotation(left: Quat, right: Quat) {
    assert!(same_rotation(left, right));
}

fn same_rotation(left: Quat, right: Quat) -> bool {
    left.dot(right).abs() > 1.0 - EPSILON
}
