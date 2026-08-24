use std::{collections::BTreeSet, sync::Arc, time::Duration};

use avian3d::prelude::{Collider, RigidBody};
use bevy::{app::App, math::Quat, prelude::*};
use droll_gui::{
    dice::{d6_geometry, d6_solid_symmetries, d20_geometry, d20_solid_symmetries},
    physics::{
        D20PresentationMapping, DieKind, FixedD6Presentation, FixedD20Presentation, NumberedVisual,
        PhysicalBatchRequest, PlaybackRoot, RecordedPlaybackClock, RecordedPlaybackPlugin,
        RecordedTrajectoryPlayback, SemanticPresentationMap, TrajectorySample,
        compose_visible_orientation, map_d6_presentation, map_d20_presentation,
        natural_record_identity, prepare_recorded_batch, sample_recorded_transform,
    },
};

#[path = "support/recorded_batch.rs"]
mod recorded_batch_fixture;

const EPSILON: f32 = 1.0e-5;
const PHYSICAL_SEED: u64 = 0xD65A_1E00_0000_0001;
const D20_PHYSICAL_SEED: u64 = 0xD20A_1E00_0000_0001;
const PHASE3_4D6_SEED: u64 = 0x4D6A_1E00_0000_0003;
const PHASE3_TUPLES: [[u8; 4]; 4] = [[6, 6, 6, 6], [1, 2, 3, 4], [6, 2, 5, 3], [2, 5, 1, 6]];
const PHASE4_MIXED10_SEED: u64 = 0x2D20_8D6A_0000_0004;
const PHASE4_MIXED20_SEED: u64 = 0x4D20_16D6_0000_0004;
const PHASE4_TUPLES: [[u8; 10]; 4] = [
    [20, 1, 6, 1, 6, 1, 6, 1, 6, 1],
    [3, 17, 1, 2, 3, 4, 5, 6, 2, 5],
    [19, 20, 6, 6, 6, 6, 6, 6, 6, 6],
    [8, 13, 2, 5, 1, 6, 3, 4, 2, 5],
];

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
fn test_all_400_d20_pairs_use_three_proper_mappings_and_target_blind_phase() {
    let proper_ids = d20_solid_symmetries()
        .into_iter()
        .map(|symmetry| symmetry.id)
        .collect::<BTreeSet<_>>();
    for natural in 1..=20 {
        let mut phases = BTreeSet::new();
        for requested in 1..=20 {
            let mapping = map_d20_presentation(natural, requested, 7, D20_PHYSICAL_SEED)
                .expect("all ordered d20 pairs map");
            assert!(proper_ids.contains(&mapping.symmetry_id));
            assert_eq!(mapping.phase.index, (mapping.phase.identifier % 3) as u8);
            let landed = d20_geometry()
                .face(natural)
                .expect("natural d20 face")
                .target_rotation(0.37);
            assert_eq!(
                d20_geometry().upward_face(landed * mapping.symmetry).value,
                requested
            );
            assert_d20_shape_unchanged(landed, mapping.symmetry);
            phases.insert((mapping.phase.index, mapping.phase.identifier));
        }
        assert_eq!(phases.len(), 1, "requested value changed free phase");
    }
}

#[test]
fn test_one_immutable_natural_record_maps_all_twenty_d20_values() {
    let record = prepare_recorded_batch(&PhysicalBatchRequest::new(
        vec![DieKind::D20],
        D20_PHYSICAL_SEED,
    ))
    .expect("one target-blind d20 record");
    let frozen = record.clone();
    let identity = natural_record_identity(&record);
    let accepted_seed = record
        .attempts
        .last()
        .expect("accepted attempt")
        .physical_seed;
    for requested in 1..=20 {
        let presentation = SemanticPresentationMap::for_single_d20(&record, requested)
            .expect("d20 presentation map");
        assert_eq!(presentation.natural_record_identity, identity);
        assert_d20_mapping_matches_record(presentation.d20[0], &record, requested);
        assert_eq!(record, frozen, "pure mapping mutated physical record");
    }
    assert_eq!(record.dice[0].samples, frozen.dice[0].samples);
    assert_eq!(record.dice[0].terminal, frozen.dice[0].terminal);
    assert_eq!(record.attempts, frozen.attempts);
    assert_eq!(
        record.dice[0].natural_terminal_face,
        frozen.dice[0].natural_terminal_face
    );
    assert_eq!(
        accepted_seed,
        frozen
            .attempts
            .last()
            .expect("accepted attempt")
            .physical_seed
    );
    eprintln!(
        "phase2-record id={identity:016x} natural_face={} base_seed={D20_PHYSICAL_SEED:#018x} accepted_seed={accepted_seed:#018x} attempts={} steps={} samples={} simulated={:?} wall={:?}",
        record.dice[0].natural_terminal_face,
        record.attempts.len(),
        record.calibration.fixed_steps,
        record.dice[0].samples.len(),
        record.calibration.simulated_duration,
        record.calibration.wall_clock_duration,
    );
}

#[test]
fn test_phase3_tuples_change_only_four_presentation_mappings() {
    let record = prepare_recorded_batch(&PhysicalBatchRequest::new(
        vec![DieKind::D6; 4],
        PHASE3_4D6_SEED,
    ))
    .expect("one target-blind interacting 4d6 record");
    let frozen = record.clone();
    let identity = natural_record_identity(&record);
    let accepted_seed = record
        .attempts
        .last()
        .expect("accepted attempt")
        .physical_seed;
    let expected_ordinals = [0_u16, 1, 2, 3];
    let mut physical_phases = None;
    let mut tuple_mappings = Vec::new();
    let contact_elapsed = record.fixed_step
        * record
            .contacts
            .strongest_dice_contact
            .expect("strongest contact")
            .fixed_step
            .saturating_sub(1);
    for requested in PHASE3_TUPLES {
        let presentation = SemanticPresentationMap::for_d6_tuple(&record, &requested)
            .expect("complete 4d6 presentation map");
        assert_eq!(presentation.natural_record_identity, identity);
        assert_eq!(presentation.d6.len(), 4);
        let phases = presentation
            .d6
            .iter()
            .map(|mapping| (mapping.phase.index, mapping.phase.identifier))
            .collect::<Vec<_>>();
        assert_eq!(
            physical_phases.get_or_insert_with(|| phases.clone()),
            &phases
        );
        for (index, mapping) in presentation.d6.iter().copied().enumerate() {
            let die = &record.dice[index];
            assert_eq!(mapping.ordinal, expected_ordinals[index]);
            assert_eq!(mapping.ordinal, die.ordinal);
            assert_eq!(mapping.natural_face, die.natural_terminal_face);
            assert_eq!(mapping.requested_face, requested[index]);
            assert_eq!(mapping.accepted_physical_seed, accepted_seed);
            assert_visible_transform_contract(&die.samples, record.fixed_step, mapping);
            let contact =
                sample_recorded_transform(&die.samples, record.fixed_step, contact_elapsed)
                    .expect("strongest-contact transform");
            assert_occupied_shape_unchanged(contact.recorded_orientation, mapping.symmetry);
        }
        tuple_mappings.push(presentation.d6);
        assert_eq!(record, frozen, "tuple mapping mutated the physical record");
        assert_eq!(natural_record_identity(&record), identity);
    }
    assert_eq!(record.attempts, frozen.attempts);
    assert_eq!(record.dice, frozen.dice);
    assert_eq!(record.contacts, frozen.contacts);
    assert_eq!(record.calibration, frozen.calibration);
    assert_eq!(tuple_mappings[0][0], tuple_mappings[2][0]);
    assert_eq!(tuple_mappings[1][1], tuple_mappings[2][1]);
}

#[test]
fn test_phase4_synthetic_mixed_tuples_reuse_one_immutable_record() {
    let record = recorded_batch_fixture::mixed_record(10, PHASE4_MIXED10_SEED);
    let frozen = record.clone();
    let identity = natural_record_identity(&record);
    let accepted_seed = record.attempts.last().unwrap().physical_seed;
    let strongest_mixed = record
        .contacts
        .dice_contact_samples
        .iter()
        .filter(|sample| {
            record.dice[usize::from(sample.first_ordinal)].kind
                != record.dice[usize::from(sample.second_ordinal)].kind
        })
        .max_by(|left, right| left.normal_impulse.total_cmp(&right.normal_impulse))
        .expect("accepted record has a d6/d20 contact");
    let contact_elapsed = record.fixed_step * strongest_mixed.fixed_step.saturating_sub(1);
    let mut stable_phases = None;
    let mut visible_tuples = Vec::new();
    for requested in PHASE4_TUPLES {
        let presentation = SemanticPresentationMap::for_mixed_tuple(&record, &requested)
            .expect("complete mixed proper-symmetry map");
        assert_eq!(presentation.natural_record_identity, identity);
        assert_eq!(presentation.d20.len(), 2);
        assert_eq!(presentation.d6.len(), 8);
        let phases = mixed_phase_ids(&presentation);
        assert_eq!(stable_phases.get_or_insert_with(|| phases.clone()), &phases);
        let visible = record
            .dice
            .iter()
            .map(|die| {
                assert_mixed_mapping(
                    die,
                    &presentation,
                    requested[usize::from(die.ordinal)],
                    accepted_seed,
                    record.fixed_step,
                    contact_elapsed,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(visible, requested);
        visible_tuples.push(visible);
        assert_eq!(record, frozen, "mixed mapping mutated physical record");
        assert_eq!(natural_record_identity(&record), identity);
    }
    assert_eq!(record.attempts, frozen.attempts);
    assert_eq!(record.dice, frozen.dice);
    assert_eq!(record.contacts, frozen.contacts);
    assert_eq!(record.calibration, frozen.calibration);
    eprintln!(
        "phase4-mixed-map id={identity:016x} accepted={accepted_seed:#018x} tuples={PHASE4_TUPLES:?} visible={visible_tuples:?} strongest_mixed={strongest_mixed:?}"
    );
}

#[test]
fn test_phase4_synthetic_mixed20_mapping_is_correct_and_transform_only() {
    let record = recorded_batch_fixture::mixed_record(20, PHASE4_MIXED20_SEED);
    let frozen = record.clone();
    let requested = record
        .dice
        .iter()
        .map(|die| 1 + (die.ordinal as u8 % die.kind.face_count()))
        .collect::<Vec<_>>();
    let presentation = SemanticPresentationMap::for_mixed_tuple(&record, &requested).unwrap();
    let accepted_seed = record.attempts.last().unwrap().physical_seed;
    let contact_elapsed = record.fixed_step
        * record
            .contacts
            .strongest_dice_contact
            .unwrap()
            .fixed_step
            .saturating_sub(1);
    let visible = record
        .dice
        .iter()
        .map(|die| {
            assert_mixed_mapping(
                die,
                &presentation,
                requested[usize::from(die.ordinal)],
                accepted_seed,
                record.fixed_step,
                contact_elapsed,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(visible, requested);
    assert_eq!(record, frozen);
}

fn mixed_phase_ids(presentation: &SemanticPresentationMap) -> Vec<(u16, u64)> {
    let mut phases = presentation
        .d6
        .iter()
        .map(|mapping| (mapping.ordinal, mapping.phase.identifier))
        .chain(
            presentation
                .d20
                .iter()
                .map(|mapping| (mapping.ordinal, mapping.phase.identifier)),
        )
        .collect::<Vec<_>>();
    phases.sort_unstable();
    phases
}

fn assert_mixed_mapping(
    die: &droll_gui::physics::RecordedDie,
    presentation: &SemanticPresentationMap,
    requested: u8,
    accepted_seed: u64,
    fixed_step: Duration,
    contact_elapsed: Duration,
) -> u8 {
    let (symmetry, natural, mapped, mapping_seed) = match die.kind {
        DieKind::D6 => {
            let mapping = presentation
                .d6
                .iter()
                .find(|map| map.ordinal == die.ordinal)
                .unwrap();
            (
                mapping.symmetry,
                mapping.natural_face,
                mapping.requested_face,
                mapping.accepted_physical_seed,
            )
        }
        DieKind::D20 => {
            let mapping = presentation
                .d20
                .iter()
                .find(|map| map.ordinal == die.ordinal)
                .unwrap();
            (
                mapping.symmetry,
                mapping.natural_face,
                mapping.requested_face,
                mapping.accepted_physical_seed,
            )
        }
    };
    assert_eq!(natural, die.natural_terminal_face);
    assert_eq!(mapped, requested);
    assert_eq!(mapping_seed, accepted_seed);
    let contact = sample_recorded_transform(&die.samples, fixed_step, contact_elapsed).unwrap();
    let final_orientation = Quat::from_array(die.samples.last().unwrap().unit_orientation);
    match die.kind {
        DieKind::D6 => {
            assert_occupied_shape_unchanged(contact.recorded_orientation, symmetry);
            d6_geometry()
                .upward_face(final_orientation * symmetry)
                .value
        }
        DieKind::D20 => {
            assert_d20_shape_unchanged(contact.recorded_orientation, symmetry);
            d20_geometry()
                .upward_face(final_orientation * symmetry)
                .value
        }
    }
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
fn test_d20_high_angular_speed_slerp_preserves_exact_first_and_final_transforms() {
    let fixed_step = Duration::from_secs_f64(1.0 / 60.0);
    let first_rotation = Quat::from_euler(EulerRot::XYZ, 0.3, -0.4, 0.2);
    let fast_delta =
        Quat::from_axis_angle(Vec3::new(1.0, 2.0, -1.0).normalize(), 175_f32.to_radians());
    let final_rotation = first_rotation * fast_delta;
    let samples = vec![
        TrajectorySample {
            fixed_step: 1,
            world_position: [-0.4, 1.7, 0.2],
            unit_orientation: first_rotation.to_array(),
        },
        TrajectorySample {
            fixed_step: 2,
            world_position: [0.5, 0.6, -0.3],
            unit_orientation: final_rotation.to_array(),
        },
    ];
    assert_sample_exact(
        sample_recorded_transform(&samples, fixed_step, Duration::ZERO).expect("first sample"),
        &samples[0],
    );
    let midpoint = sample_recorded_transform(&samples, fixed_step, fixed_step / 2)
        .expect("high-speed midpoint");
    assert_same_rotation(
        midpoint.recorded_orientation,
        first_rotation.slerp(final_rotation, 0.5),
    );
    assert_sample_exact(
        sample_recorded_transform(&samples, fixed_step, fixed_step).expect("final sample"),
        &samples[1],
    );
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
    let d20_mapping = map_d20_presentation(1, 20, 0, D20_PHYSICAL_SEED).expect("d20 mapping");
    let d20_visual = app
        .world_mut()
        .spawn((
            NumberedVisual,
            FixedD20Presentation(d20_mapping),
            Transform::from_rotation(d20_mapping.symmetry),
        ))
        .id();
    app.world_mut().entity_mut(root).add_child(d20_visual);
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

    for entity in [root, visual, d20_visual] {
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
fn test_four_playback_roots_advance_on_exactly_one_shared_clock() {
    let samples: Arc<[TrajectorySample]> = representative_samples().into();
    let fixed_step = Duration::from_secs_f64(1.0 / 60.0);
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(RecordedPlaybackPlugin);
    let roots = (0..4)
        .map(|_| {
            app.world_mut()
                .spawn((
                    PlaybackRoot,
                    RecordedTrajectoryPlayback::new(samples.clone(), fixed_step),
                    Transform::IDENTITY,
                ))
                .id()
        })
        .collect::<Vec<_>>();
    app.world_mut()
        .resource_mut::<RecordedPlaybackClock>()
        .configure(Duration::ZERO, fixed_step * 2, 0.2);
    app.update();
    app.update();

    let clock = app.world().resource::<RecordedPlaybackClock>();
    for root in roots {
        let entity = app.world().entity(root);
        let playback = entity.get::<RecordedTrajectoryPlayback>().unwrap();
        assert_eq!(playback.elapsed, clock.elapsed);
        assert_eq!(playback.fixed_step, fixed_step);
        assert!(entity.get::<RigidBody>().is_none());
        assert!(entity.get::<Collider>().is_none());
    }
    assert_eq!(clock.speed, 0.2);
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
        "D20PresentationMapping",
        "requested_face",
        "FixedD6Presentation",
        "FixedD20Presentation",
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

fn assert_d20_mapping_matches_record(
    mapping: D20PresentationMapping,
    record: &droll_gui::physics::RecordedBatch,
    requested: u8,
) {
    let die = &record.dice[0];
    assert_eq!(mapping.natural_face, die.natural_terminal_face);
    assert_eq!(mapping.requested_face, requested);
    assert_eq!(
        mapping.accepted_physical_seed,
        record
            .attempts
            .last()
            .expect("accepted attempt")
            .physical_seed
    );
    let final_sample = die.samples.last().expect("d20 trajectory samples");
    let visible = compose_visible_orientation(
        Quat::from_array(final_sample.unit_orientation),
        mapping.symmetry,
        Quat::IDENTITY,
    );
    assert_eq!(d20_geometry().upward_face(visible).value, requested);
    assert_d20_shape_unchanged(
        Quat::from_array(final_sample.unit_orientation),
        mapping.symmetry,
    );
}

fn assert_d20_shape_unchanged(recorded: Quat, symmetry: Quat) {
    let vertices = d20_geometry().vertices;
    let baseline = vertices.map(|vertex| recorded * vertex);
    let mapped = vertices.map(|vertex| recorded * symmetry * vertex);
    for vertex in baseline {
        assert!(
            mapped
                .iter()
                .any(|candidate| candidate.distance(vertex) < 2.0 * EPSILON)
        );
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
