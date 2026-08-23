use std::{sync::Arc, time::Duration};

use bevy::{
    app::AppExit,
    asset::RenderAssetUsages,
    color::palettes::css::{BLACK, DARK_SLATE_GRAY, GOLD},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

use crate::{
    dice::{D20Label, d6_geometry, d6_labels, d6_mesh, d20_geometry, d20_labels, d20_mesh},
    physics::{
        DieKind, FixedD6Presentation, FixedD20Presentation, NumberedVisual, PhysicalBatchRequest,
        PlaybackRoot, RecordedBatch, RecordedPlaybackClock, RecordedTrajectoryPlayback,
        SemanticPresentationMap, compose_visible_orientation, natural_record_identity,
        prepare_recorded_batch, sample_recorded_transform,
    },
};

use super::{SpikeOptions, SpikeScenario};

pub(super) const RECORDED_D6_PHYSICAL_SEED: u64 = 0xD65A_1E00_0000_0001;
pub(super) const RECORDED_D20_PHYSICAL_SEED: u64 = 0xD20A_1E00_0000_0001;
pub(super) const RECORDED_4D6_PHYSICAL_SEED: u64 = 0x4D6A_1E00_0000_0003;
const PHASE3_TUPLES: [[u8; 4]; 4] = [[6, 6, 6, 6], [1, 2, 3, 4], [6, 2, 5, 3], [2, 5, 1, 6]];
const STRONGEST_CONTACT_CASE: &str = "strongest-contact";
const SLOW_MOTION_SPEED: f32 = 0.2;

#[derive(Resource)]
pub(super) struct RecordedReplaySequence {
    kind: DieKind,
    record: Arc<RecordedBatch>,
    mappings: Vec<SemanticPresentationMap>,
    index: usize,
    active: Vec<Entity>,
    playback_start: Duration,
    playback_end: Duration,
    playback_speed: f32,
    terminal_hold_seconds: f32,
}

pub(super) fn setup_recorded_replay(
    mut commands: Commands,
    options: Res<SpikeOptions>,
    mut playback_clock: ResMut<RecordedPlaybackClock>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (kind, dice, physical_seed) = match options.scenario {
        SpikeScenario::D6Faces => (DieKind::D6, vec![DieKind::D6], RECORDED_D6_PHYSICAL_SEED),
        SpikeScenario::D20Faces => (DieKind::D20, vec![DieKind::D20], RECORDED_D20_PHYSICAL_SEED),
        SpikeScenario::FourD6 => (
            DieKind::D6,
            vec![DieKind::D6; 4],
            RECORDED_4D6_PHYSICAL_SEED,
        ),
        scenario => panic!("recorded replay does not support scenario `{scenario}`"),
    };
    let record = Arc::new(
        prepare_recorded_batch(&PhysicalBatchRequest::new(dice, physical_seed))
            .expect("recorded-replay hidden preparation"),
    );
    let requested_tuples = selected_requested_tuples(&record, kind, options.case_id.as_deref());
    let mappings = requested_tuples
        .into_iter()
        .map(|requested| match (kind, requested.as_slice()) {
            (DieKind::D6, [face]) => SemanticPresentationMap::for_single_d6(&record, *face),
            (DieKind::D6, tuple) => SemanticPresentationMap::for_d6_tuple(&record, tuple),
            (DieKind::D20, [face]) => SemanticPresentationMap::for_single_d20(&record, *face),
            (DieKind::D20, _) => unreachable!("d20 checkpoint uses one die"),
        })
        .collect::<Result<Vec<_>, _>>()
        .expect("complete proper-symmetry maps");
    log_preparation(&record, &mappings, physical_seed, kind);
    let (playback_start, playback_end, playback_speed) = playback_profile(
        &record,
        options.case_id.as_deref() == Some(STRONGEST_CONTACT_CASE),
    );
    playback_clock.configure(playback_start, playback_end, playback_speed);
    spawn_recorded_environment(
        &mut commands,
        &mut meshes,
        &mut materials,
        kind,
        record.dice.len(),
    );
    let active = spawn_recorded_dice(
        &mut commands,
        &mut meshes,
        &mut materials,
        &record,
        &mappings[0],
        kind,
        playback_start,
    );
    commands.insert_resource(RecordedReplaySequence {
        kind,
        record,
        mappings,
        index: 0,
        active,
        playback_start,
        playback_end,
        playback_speed,
        terminal_hold_seconds: 0.0,
    });
}

fn selected_requested_tuples(
    record: &RecordedBatch,
    kind: DieKind,
    case_id: Option<&str>,
) -> Vec<Vec<u8>> {
    if record.dice.len() == 4 {
        return match case_id {
            None => PHASE3_TUPLES.into_iter().map(Vec::from).collect(),
            Some(STRONGEST_CONTACT_CASE) => vec![Vec::from(PHASE3_TUPLES[2])],
            Some(id) => panic!("unknown recorded-replay case `{id}`"),
        };
    }
    let (prefix, maximum) = match kind {
        DieKind::D6 => ("d6-recorded-target-", 6),
        DieKind::D20 => ("d20-recorded-target-", 20),
    };
    match case_id {
        None => (1..=maximum).map(|face| vec![face]).collect(),
        Some(id) => {
            let requested = id
                .strip_prefix(prefix)
                .and_then(|value| value.parse::<u8>().ok())
                .filter(|value| (1..=maximum).contains(value))
                .unwrap_or_else(|| panic!("unknown recorded-replay case `{id}`"));
            vec![vec![requested]]
        }
    }
}

fn playback_profile(record: &RecordedBatch, strongest_contact: bool) -> (Duration, Duration, f32) {
    let final_elapsed = record.fixed_step
        * u32::try_from(record.dice[0].samples.len().saturating_sub(1))
            .expect("bounded recorded trajectory");
    if !strongest_contact {
        return (Duration::ZERO, final_elapsed, 1.0);
    }
    let strongest = record
        .contacts
        .strongest_dice_contact
        .expect("4d6 checkpoint has a strongest dice contact");
    let contact_elapsed = record.fixed_step * strongest.fixed_step.saturating_sub(1);
    let context = record.fixed_step * 30;
    (
        contact_elapsed.saturating_sub(context),
        contact_elapsed.saturating_add(context).min(final_elapsed),
        SLOW_MOTION_SPEED,
    )
}

fn spawn_recorded_environment(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    kind: DieKind,
    die_count: usize,
) {
    let tray_material = materials.add(StandardMaterial {
        base_color: DARK_SLATE_GRAY.into(),
        perceptual_roughness: 0.9,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(8.0, 0.2, 6.0))),
        MeshMaterial3d(tray_material.clone()),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));
    spawn_recorded_walls(commands, meshes, tray_material);
    commands.spawn((
        PointLight {
            intensity: 2_500_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 7.0, 4.0),
    ));
    let camera = match (kind, die_count) {
        (DieKind::D6, 4) => Vec3::new(5.8, 4.8, 7.5),
        (DieKind::D6, _) => Vec3::new(4.5, 3.5, 6.5),
        (DieKind::D20, _) => Vec3::new(3.8, 3.0, 5.4),
    };
    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(camera).looking_at(Vec3::new(0.0, 0.6, 0.0), Vec3::Y),
    ));
}

fn spawn_recorded_walls(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
) {
    for (size, translation) in [
        (Vec3::new(8.0, 0.8, 0.2), Vec3::new(0.0, 0.4, -3.0)),
        (Vec3::new(8.0, 0.8, 0.2), Vec3::new(0.0, 0.4, 3.0)),
        (Vec3::new(0.2, 0.8, 6.0), Vec3::new(-4.0, 0.4, 0.0)),
        (Vec3::new(0.2, 0.8, 6.0), Vec3::new(4.0, 0.4, 0.0)),
    ] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(translation),
        ));
    }
}

fn spawn_recorded_dice(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    record: &RecordedBatch,
    presentation: &SemanticPresentationMap,
    kind: DieKind,
    playback_start: Duration,
) -> Vec<Entity> {
    (0..record.dice.len())
        .map(|die_index| {
            spawn_recorded_die(
                commands,
                meshes,
                materials,
                record,
                presentation,
                kind,
                die_index,
                playback_start,
            )
        })
        .collect()
}

fn spawn_recorded_die(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    record: &RecordedBatch,
    presentation: &SemanticPresentationMap,
    kind: DieKind,
    die_index: usize,
    playback_start: Duration,
) -> Entity {
    let die = &record.dice[die_index];
    let first = sample_recorded_transform(&die.samples, record.fixed_step, playback_start)
        .expect("accepted record has a first sample");
    let die_material = materials.add(StandardMaterial {
        base_color: GOLD.into(),
        perceptual_roughness: 0.45,
        ..default()
    });
    let label_material = materials.add(StandardMaterial {
        base_color: BLACK.into(),
        ..default()
    });
    let d6_assets =
        (kind == DieKind::D6).then(|| (meshes.add(d6_mesh()), meshes.add(Sphere::new(0.055))));
    let d20_assets = (kind == DieKind::D20).then(|| {
        let labels = d20_labels()
            .iter()
            .map(|label| meshes.add(d20_label_mesh(label)))
            .collect::<Vec<_>>();
        (meshes.add(d20_mesh()), labels)
    });
    commands
        .spawn((
            PlaybackRoot,
            RecordedTrajectoryPlayback::new(die.samples.clone().into(), record.fixed_step),
            Transform::from_translation(first.world_position)
                .with_rotation(first.recorded_orientation),
            Visibility::Inherited,
        ))
        .with_children(|root| match kind {
            DieKind::D6 => spawn_d6_visual(
                root,
                presentation,
                die_index,
                d6_assets.expect("d6 render assets"),
                die_material,
                label_material,
            ),
            DieKind::D20 => spawn_d20_visual(
                root,
                presentation,
                d20_assets.expect("d20 render assets"),
                die_material,
                label_material,
            ),
        })
        .id()
}

fn spawn_d6_visual(
    root: &mut ChildSpawnerCommands,
    presentation: &SemanticPresentationMap,
    die_index: usize,
    (mesh, pip_mesh): (Handle<Mesh>, Handle<Mesh>),
    die_material: Handle<StandardMaterial>,
    label_material: Handle<StandardMaterial>,
) {
    let mapping = presentation.d6[die_index];
    root.spawn((
        NumberedVisual,
        FixedD6Presentation(mapping),
        Transform::from_rotation(mapping.symmetry),
        Visibility::Inherited,
    ))
    .with_children(|visual| {
        visual.spawn((Mesh3d(mesh), MeshMaterial3d(die_material)));
        for label in d6_labels() {
            visual.spawn((
                Mesh3d(pip_mesh.clone()),
                MeshMaterial3d(label_material.clone()),
                Transform::from_translation(label.center),
            ));
        }
    });
}

fn spawn_d20_visual(
    root: &mut ChildSpawnerCommands,
    presentation: &SemanticPresentationMap,
    (mesh, labels): (Handle<Mesh>, Vec<Handle<Mesh>>),
    die_material: Handle<StandardMaterial>,
    label_material: Handle<StandardMaterial>,
) {
    let mapping = presentation.d20[0];
    root.spawn((
        NumberedVisual,
        FixedD20Presentation(mapping),
        Transform::from_rotation(mapping.symmetry),
        Visibility::Inherited,
    ))
    .with_children(|visual| {
        visual.spawn((Mesh3d(mesh), MeshMaterial3d(die_material)));
        for label in labels {
            visual.spawn((Mesh3d(label), MeshMaterial3d(label_material.clone())));
        }
    });
}

fn d20_label_mesh(label: &D20Label) -> Mesh {
    let positions = label
        .vertices
        .iter()
        .map(|vertex| vertex.to_array())
        .collect::<Vec<_>>();
    let normals = vec![label.normal.to_array(); positions.len()];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(label.indices.clone()))
}

pub(super) fn advance_recorded_replay_cases(
    mut commands: Commands,
    time: Res<Time>,
    mut sequence: ResMut<RecordedReplaySequence>,
    mut playback_clock: ResMut<RecordedPlaybackClock>,
    playback: Query<&RecordedTrajectoryPlayback, With<PlaybackRoot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut exit: MessageWriter<AppExit>,
) {
    if !sequence.active.iter().all(|entity| {
        playback
            .get(*entity)
            .is_ok_and(|playback| playback.complete)
    }) {
        return;
    }
    sequence.terminal_hold_seconds += time.delta_secs();
    if sequence.terminal_hold_seconds < 1.5 {
        return;
    }
    log_case_summary(&sequence);
    if sequence.index + 1 == sequence.mappings.len() {
        exit.write(AppExit::Success);
        return;
    }
    for entity in sequence.active.drain(..) {
        commands.entity(entity).despawn();
    }
    sequence.index += 1;
    sequence.terminal_hold_seconds = 0.0;
    playback_clock.configure(
        sequence.playback_start,
        sequence.playback_end,
        sequence.playback_speed,
    );
    sequence.active = spawn_recorded_dice(
        &mut commands,
        &mut meshes,
        &mut materials,
        &sequence.record,
        &sequence.mappings[sequence.index],
        sequence.kind,
        sequence.playback_start,
    );
}

pub(super) fn update_recorded_replay_window_status(
    sequence: Res<RecordedReplaySequence>,
    mut windows: Query<&mut Window>,
) {
    let mappings = mapping_summaries(&sequence.mappings[sequence.index], sequence.kind);
    let identity = sequence.mappings[sequence.index].natural_record_identity;
    let strongest = sequence.record.contacts.strongest_dice_contact;
    for mut window in &mut windows {
        window.title = format!(
            "Droll Stage 1 - recorded-replay - case {}/{} - requested {:?} - natural {:?} - symmetries {:?} - phases {:?} - speed {:.2}x - strongest {:?} - record {identity:016x}",
            sequence.index + 1,
            sequence.mappings.len(),
            mappings
                .iter()
                .map(|mapping| mapping.requested_face)
                .collect::<Vec<_>>(),
            mappings
                .iter()
                .map(|mapping| mapping.natural_face)
                .collect::<Vec<_>>(),
            mappings
                .iter()
                .map(|mapping| mapping.symmetry_id)
                .collect::<Vec<_>>(),
            mappings
                .iter()
                .map(|mapping| mapping.phase_index)
                .collect::<Vec<_>>(),
            sequence.playback_speed,
            strongest.map(|sample| (
                sample.first_ordinal,
                sample.second_ordinal,
                sample.fixed_step,
            )),
        );
    }
}

fn log_preparation(
    record: &RecordedBatch,
    mappings: &[SemanticPresentationMap],
    base_seed: u64,
    kind: DieKind,
) {
    let accepted = record.attempts.last().expect("accepted attempt");
    let strongest = record.contacts.strongest_dice_contact;
    info!(
        record_id = format_args!("{:016x}", natural_record_identity(record)),
        base_seed = format_args!("{base_seed:#018x}"),
        accepted_seed = format_args!("{:#018x}", accepted.physical_seed),
        attempts = record.attempts.len(),
        natural_faces = ?record.dice.iter().map(|die| die.natural_terminal_face).collect::<Vec<_>>(),
        steps = record.calibration.fixed_steps,
        samples_per_die = record.dice[0].samples.len(),
        simulated_seconds = record.calibration.simulated_duration.as_secs_f64(),
        wall_milliseconds = record.calibration.wall_clock_duration.as_secs_f64() * 1_000.0,
        requested_order = ?mappings.iter().map(|map| mapping_summaries(map, kind).iter().map(|mapping| mapping.requested_face).collect::<Vec<_>>()).collect::<Vec<_>>(),
        dice_contact_interactions = record.contacts.dice_contact_interactions,
        dice_contact_steps = record.contacts.dice_contact_samples.len(),
        strongest_pair = ?strongest.map(|sample| (sample.first_ordinal, sample.second_ordinal)),
        strongest_step = ?strongest.map(|sample| sample.fixed_step),
        strongest_seconds = ?strongest.map(|sample| (record.fixed_step * sample.fixed_step).as_secs_f64()),
        strongest_impulse = ?strongest.map(|sample| sample.normal_impulse),
        strongest_approach_speed = ?strongest.map(|sample| sample.approach_speed),
        "Target-blind recorded preparation"
    );
    for (tuple_index, presentation) in mappings.iter().enumerate() {
        for (ordinal, mapping) in mapping_summaries(presentation, kind).iter().enumerate() {
            info!(
                tuple = tuple_index + 1,
                ordinal,
                requested = mapping.requested_face,
                natural = mapping.natural_face,
                symmetry_id = mapping.symmetry_id,
                phase_index = mapping.phase_index,
                phase_id = format_args!("{:016x}", mapping.phase_identifier),
                "Pure semantic presentation mapping"
            );
        }
    }
}

fn log_case_summary(sequence: &RecordedReplaySequence) {
    let mappings = mapping_summaries(&sequence.mappings[sequence.index], sequence.kind);
    let visible_faces = sequence
        .record
        .dice
        .iter()
        .zip(&mappings)
        .map(|(die, mapping)| {
            let final_sample = die.samples.last().expect("accepted trajectory has samples");
            let visible = compose_visible_orientation(
                Quat::from_array(final_sample.unit_orientation),
                mapping.symmetry,
                Quat::IDENTITY,
            );
            match sequence.kind {
                DieKind::D6 => d6_geometry().upward_face(visible).value,
                DieKind::D20 => d20_geometry().upward_face(visible).value,
            }
        })
        .collect::<Vec<_>>();
    info!(
        case = sequence.index + 1,
        requested = ?mappings.iter().map(|mapping| mapping.requested_face).collect::<Vec<_>>(),
        natural = ?mappings.iter().map(|mapping| mapping.natural_face).collect::<Vec<_>>(),
        symmetry_ids = ?mappings.iter().map(|mapping| mapping.symmetry_id).collect::<Vec<_>>(),
        phases = ?mappings.iter().map(|mapping| mapping.phase_index).collect::<Vec<_>>(),
        final_visible = ?visible_faces,
        record_id = format_args!(
            "{:016x}",
            sequence.mappings[sequence.index].natural_record_identity
        ),
        "Recorded-replay case summary"
    );
}

#[derive(Clone, Copy)]
struct MappingSummary {
    natural_face: u8,
    requested_face: u8,
    symmetry_id: u8,
    symmetry: Quat,
    phase_index: u8,
    phase_identifier: u64,
}

fn mapping_summaries(presentation: &SemanticPresentationMap, kind: DieKind) -> Vec<MappingSummary> {
    match kind {
        DieKind::D6 => presentation
            .d6
            .iter()
            .map(|mapping| MappingSummary {
                natural_face: mapping.natural_face,
                requested_face: mapping.requested_face,
                symmetry_id: mapping.symmetry_id,
                symmetry: mapping.symmetry,
                phase_index: mapping.phase.index,
                phase_identifier: mapping.phase.identifier,
            })
            .collect(),
        DieKind::D20 => {
            let mapping = presentation.d20[0];
            vec![MappingSummary {
                natural_face: mapping.natural_face,
                requested_face: mapping.requested_face,
                symmetry_id: mapping.symmetry_id,
                symmetry: mapping.symmetry,
                phase_index: mapping.phase.index,
                phase_identifier: mapping.phase.identifier,
            }]
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        asset::AssetPlugin, camera::visibility::VisibilityPlugin,
        mesh::skinning::SkinnedMeshInverseBindposes, transform::TransformPlugin,
    };

    use super::*;

    #[derive(Resource)]
    struct ReplaySpawnFixture {
        kind: DieKind,
        record: RecordedBatch,
        presentation: SemanticPresentationMap,
        roots: Vec<Entity>,
    }

    fn spawn_replay_fixture(
        mut commands: Commands,
        mut meshes: ResMut<Assets<Mesh>>,
        mut materials: ResMut<Assets<StandardMaterial>>,
        mut fixture: ResMut<ReplaySpawnFixture>,
    ) {
        fixture.roots = spawn_recorded_dice(
            &mut commands,
            &mut meshes,
            &mut materials,
            &fixture.record,
            &fixture.presentation,
            fixture.kind,
            Duration::ZERO,
        );
    }

    fn replay_fixture_app(
        kind: DieKind,
        physical_seed: u64,
        requested_face: u8,
    ) -> (App, Transform, SemanticPresentationMap) {
        let request = PhysicalBatchRequest::new(vec![kind], physical_seed);
        let record = prepare_recorded_batch(&request).expect("recorded-replay fixture preparation");
        let presentation = match kind {
            DieKind::D6 => SemanticPresentationMap::for_single_d6(&record, requested_face),
            DieKind::D20 => SemanticPresentationMap::for_single_d20(&record, requested_face),
        }
        .expect("recorded-replay fixture presentation");
        let first =
            sample_recorded_transform(&record.dice[0].samples, record.fixed_step, Duration::ZERO)
                .expect("fixture first sample");
        let expected_root = Transform::from_translation(first.world_position)
            .with_rotation(first.recorded_orientation);
        let expected_presentation = presentation.clone();
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TransformPlugin,
            VisibilityPlugin,
        ))
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<SkinnedMeshInverseBindposes>>()
        .init_resource::<Assets<StandardMaterial>>()
        .insert_resource(ReplaySpawnFixture {
            kind,
            record,
            presentation,
            roots: Vec::new(),
        })
        .add_systems(Startup, spawn_replay_fixture);
        (app, expected_root, expected_presentation)
    }

    fn assert_playback_root(world: &World, root: Entity, expected: &Transform) -> Entity {
        let entity = world.entity(root);
        assert!(entity.contains::<PlaybackRoot>());
        assert!(entity.contains::<RecordedTrajectoryPlayback>());
        assert_eq!(entity.get::<Visibility>(), Some(&Visibility::Inherited));
        assert!(entity.get::<InheritedVisibility>().unwrap().get());
        assert!(entity.contains::<ViewVisibility>());
        assert_eq!(entity.get::<Transform>().unwrap(), expected);
        let children = entity.get::<Children>().unwrap();
        assert_eq!(children.len(), 1);
        children[0]
    }

    fn assert_numbered_visual(
        world: &World,
        visual: Entity,
        expected: FixedD6Presentation,
    ) -> Vec<Entity> {
        let entity = world.entity(visual);
        assert!(entity.contains::<NumberedVisual>());
        assert_eq!(entity.get::<Visibility>(), Some(&Visibility::Inherited));
        assert!(entity.get::<InheritedVisibility>().unwrap().get());
        assert!(entity.contains::<ViewVisibility>());
        assert_eq!(entity.get::<FixedD6Presentation>(), Some(&expected));
        assert_eq!(
            entity.get::<Transform>().unwrap(),
            &Transform::from_rotation(expected.0.symmetry)
        );
        entity.get::<Children>().unwrap().iter().collect()
    }

    fn assert_d20_numbered_visual(
        world: &World,
        visual: Entity,
        expected: FixedD20Presentation,
    ) -> Vec<Entity> {
        let entity = world.entity(visual);
        assert!(entity.contains::<NumberedVisual>());
        assert_eq!(entity.get::<Visibility>(), Some(&Visibility::Inherited));
        assert!(entity.get::<InheritedVisibility>().unwrap().get());
        assert!(entity.contains::<ViewVisibility>());
        assert_eq!(entity.get::<FixedD20Presentation>(), Some(&expected));
        assert_eq!(
            entity.get::<Transform>().unwrap(),
            &Transform::from_rotation(expected.0.symmetry)
        );
        entity.get::<Children>().unwrap().iter().collect()
    }

    fn assert_visible_geometry(world: &World, geometry: &[Entity]) {
        assert_eq!(geometry.len(), 1 + d6_labels().len());
        for &child in geometry {
            let entity = world.entity(child);
            assert!(entity.contains::<Mesh3d>());
            assert!(entity.get::<InheritedVisibility>().unwrap().get());
            assert!(entity.contains::<ViewVisibility>());
        }
    }

    #[test]
    fn test_full_checkpoint_uses_deterministic_one_through_six_order() {
        let record = prepare_recorded_batch(&PhysicalBatchRequest::new(
            vec![DieKind::D6],
            RECORDED_D6_PHYSICAL_SEED,
        ))
        .expect("d6 fixture record");
        assert_eq!(
            selected_requested_tuples(&record, DieKind::D6, None),
            (1..=6).map(|face| vec![face]).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_d20_checkpoint_uses_deterministic_one_through_twenty_order() {
        let record = prepare_recorded_batch(&PhysicalBatchRequest::new(
            vec![DieKind::D20],
            RECORDED_D20_PHYSICAL_SEED,
        ))
        .expect("d20 fixture record");
        assert_eq!(
            selected_requested_tuples(&record, DieKind::D20, None),
            (1..=20).map(|face| vec![face]).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_single_recorded_replay_case_selects_only_requested_mapping() {
        let d6 = prepare_recorded_batch(&PhysicalBatchRequest::new(
            vec![DieKind::D6],
            RECORDED_D6_PHYSICAL_SEED,
        ))
        .expect("d6 fixture record");
        let d20 = prepare_recorded_batch(&PhysicalBatchRequest::new(
            vec![DieKind::D20],
            RECORDED_D20_PHYSICAL_SEED,
        ))
        .expect("d20 fixture record");
        assert_eq!(
            selected_requested_tuples(&d6, DieKind::D6, Some("d6-recorded-target-4")),
            vec![vec![4]]
        );
        assert_eq!(
            selected_requested_tuples(&d20, DieKind::D20, Some("d20-recorded-target-19")),
            vec![vec![19]]
        );
    }

    #[test]
    fn test_4d6_checkpoint_and_slow_motion_use_one_deterministic_record() {
        let record = prepare_recorded_batch(&PhysicalBatchRequest::new(
            vec![DieKind::D6; 4],
            RECORDED_4D6_PHYSICAL_SEED,
        ))
        .expect("4d6 fixture record");
        assert_eq!(
            selected_requested_tuples(&record, DieKind::D6, None),
            PHASE3_TUPLES.map(Vec::from).to_vec()
        );
        assert_eq!(
            selected_requested_tuples(&record, DieKind::D6, Some(STRONGEST_CONTACT_CASE)),
            vec![Vec::from(PHASE3_TUPLES[2])]
        );
        let (start, end, speed) = playback_profile(&record, true);
        let strongest = record.contacts.strongest_dice_contact.unwrap();
        let contact_elapsed = record.fixed_step * strongest.fixed_step.saturating_sub(1);
        assert!(start <= contact_elapsed && contact_elapsed <= end);
        assert_eq!(speed, SLOW_MOTION_SPEED);
    }

    #[test]
    fn test_recorded_replay_spawn_builds_complete_visible_transform_hierarchy() {
        let (mut app, expected_root, presentation) =
            replay_fixture_app(DieKind::D6, RECORDED_D6_PHYSICAL_SEED, 4);
        app.update();
        let world = app.world();
        let root = world.resource::<ReplaySpawnFixture>().roots[0];
        let visual = assert_playback_root(world, root, &expected_root);
        let geometry =
            assert_numbered_visual(world, visual, FixedD6Presentation(presentation.d6[0]));
        assert_visible_geometry(world, &geometry);
    }

    #[test]
    fn test_d20_replay_spawn_builds_numbered_transform_only_hierarchy() {
        let (mut app, expected_root, presentation) =
            replay_fixture_app(DieKind::D20, RECORDED_D20_PHYSICAL_SEED, 19);
        app.update();
        let world = app.world();
        let root = world.resource::<ReplaySpawnFixture>().roots[0];
        let visual = assert_playback_root(world, root, &expected_root);
        let geometry =
            assert_d20_numbered_visual(world, visual, FixedD20Presentation(presentation.d20[0]));
        assert_eq!(geometry.len(), 1 + d20_labels().len());
    }

    #[test]
    fn test_4d6_replay_spawns_four_complete_concurrent_hierarchies() {
        let record = prepare_recorded_batch(&PhysicalBatchRequest::new(
            vec![DieKind::D6; 4],
            RECORDED_4D6_PHYSICAL_SEED,
        ))
        .expect("4d6 fixture record");
        let presentation = SemanticPresentationMap::for_d6_tuple(&record, &PHASE3_TUPLES[2])
            .expect("4d6 fixture mapping");
        let expected = record
            .dice
            .iter()
            .map(|die| {
                let first =
                    sample_recorded_transform(&die.samples, record.fixed_step, Duration::ZERO)
                        .expect("fixture first sample");
                Transform::from_translation(first.world_position)
                    .with_rotation(first.recorded_orientation)
            })
            .collect::<Vec<_>>();
        let mappings = presentation.d6.clone();
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TransformPlugin,
            VisibilityPlugin,
        ))
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<SkinnedMeshInverseBindposes>>()
        .init_resource::<Assets<StandardMaterial>>()
        .insert_resource(ReplaySpawnFixture {
            kind: DieKind::D6,
            record,
            presentation,
            roots: Vec::new(),
        })
        .add_systems(Startup, spawn_replay_fixture);
        app.update();
        let world = app.world();
        let roots = &world.resource::<ReplaySpawnFixture>().roots;
        assert_eq!(roots.len(), 4);
        for (index, root) in roots.iter().copied().enumerate() {
            let visual = assert_playback_root(world, root, &expected[index]);
            let geometry =
                assert_numbered_visual(world, visual, FixedD6Presentation(mappings[index]));
            assert_visible_geometry(world, &geometry);
        }
    }
}
