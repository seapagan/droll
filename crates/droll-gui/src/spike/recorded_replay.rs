use std::{sync::Arc, time::Duration};

use bevy::{
    app::AppExit,
    color::palettes::css::{BLACK, DARK_SLATE_GRAY, GOLD},
    prelude::*,
};

use crate::{
    dice::{d6_geometry, d6_labels, d6_mesh},
    physics::{
        DieKind, FixedD6Presentation, NumberedVisual, PhysicalBatchRequest, PlaybackRoot,
        RecordedBatch, RecordedTrajectoryPlayback, SemanticPresentationMap,
        compose_visible_orientation, natural_record_identity, prepare_recorded_batch,
        sample_recorded_transform,
    },
};

use super::{SpikeOptions, SpikeScenario};

pub(super) const RECORDED_D6_PHYSICAL_SEED: u64 = 0xD65A_1E00_0000_0001;

#[derive(Resource)]
pub(super) struct RecordedReplaySequence {
    record: Arc<RecordedBatch>,
    mappings: Vec<SemanticPresentationMap>,
    index: usize,
    active: Entity,
    terminal_hold_seconds: f32,
}

pub(super) fn setup_recorded_replay(
    mut commands: Commands,
    options: Res<SpikeOptions>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    assert_eq!(options.scenario, SpikeScenario::D6Faces);
    let record = Arc::new(
        prepare_recorded_batch(&PhysicalBatchRequest::new(
            vec![DieKind::D6],
            RECORDED_D6_PHYSICAL_SEED,
        ))
        .expect("recorded-replay hidden d6 preparation"),
    );
    let requested_faces = selected_requested_faces(options.case_id.as_deref());
    let mappings = requested_faces
        .into_iter()
        .map(|requested| {
            SemanticPresentationMap::for_single_d6(&record, requested)
                .expect("complete d6 proper-symmetry map")
        })
        .collect::<Vec<_>>();
    log_preparation(&record, &mappings);
    spawn_recorded_environment(&mut commands, &mut meshes, &mut materials);
    let active = spawn_recorded_die(
        &mut commands,
        &mut meshes,
        &mut materials,
        &record,
        &mappings[0],
    );
    commands.insert_resource(RecordedReplaySequence {
        record,
        mappings,
        index: 0,
        active,
        terminal_hold_seconds: 0.0,
    });
}

fn selected_requested_faces(case_id: Option<&str>) -> Vec<u8> {
    match case_id {
        None => (1..=6).collect(),
        Some(id) => {
            let requested = id
                .strip_prefix("d6-recorded-target-")
                .and_then(|value| value.parse::<u8>().ok())
                .filter(|value| (1..=6).contains(value))
                .unwrap_or_else(|| panic!("unknown recorded-replay case `{id}`"));
            vec![requested]
        }
    }
}

fn spawn_recorded_environment(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
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
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(4.5, 3.5, 6.5).looking_at(Vec3::new(0.0, 0.6, 0.0), Vec3::Y),
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

fn spawn_recorded_die(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    record: &RecordedBatch,
    presentation: &SemanticPresentationMap,
) -> Entity {
    let die = &record.dice[0];
    let first = sample_recorded_transform(&die.samples, record.fixed_step, Duration::ZERO)
        .expect("accepted record has a first sample");
    let mapping = presentation.d6[0];
    let mesh = meshes.add(d6_mesh());
    let die_material = materials.add(StandardMaterial {
        base_color: GOLD.into(),
        perceptual_roughness: 0.45,
        ..default()
    });
    let pip_mesh = meshes.add(Sphere::new(0.055));
    let pip_material = materials.add(StandardMaterial {
        base_color: BLACK.into(),
        ..default()
    });
    commands
        .spawn((
            PlaybackRoot,
            RecordedTrajectoryPlayback::new(die.samples.clone().into(), record.fixed_step),
            Transform::from_translation(first.world_position)
                .with_rotation(first.recorded_orientation),
            Visibility::Inherited,
        ))
        .with_children(|root| {
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
                        MeshMaterial3d(pip_material.clone()),
                        Transform::from_translation(label.center),
                    ));
                }
            });
        })
        .id()
}

pub(super) fn advance_recorded_replay_cases(
    mut commands: Commands,
    time: Res<Time>,
    mut sequence: ResMut<RecordedReplaySequence>,
    playback: Query<&RecordedTrajectoryPlayback, With<PlaybackRoot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok(active) = playback.get(sequence.active) else {
        return;
    };
    if !active.complete {
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
    commands.entity(sequence.active).despawn();
    sequence.index += 1;
    sequence.terminal_hold_seconds = 0.0;
    sequence.active = spawn_recorded_die(
        &mut commands,
        &mut meshes,
        &mut materials,
        &sequence.record,
        &sequence.mappings[sequence.index],
    );
}

pub(super) fn update_recorded_replay_window_status(
    sequence: Res<RecordedReplaySequence>,
    mut windows: Query<&mut Window>,
) {
    let mapping = sequence.mappings[sequence.index].d6[0];
    let identity = sequence.mappings[sequence.index].natural_record_identity;
    for mut window in &mut windows {
        window.title = format!(
            "Droll Stage 1 - recorded-replay - case {}/{} - requested {} - natural {} - symmetry {} - phase {} - record {identity:016x}",
            sequence.index + 1,
            sequence.mappings.len(),
            mapping.requested_face,
            mapping.natural_face,
            mapping.symmetry_id,
            mapping.phase.index,
        );
    }
}

fn log_preparation(record: &RecordedBatch, mappings: &[SemanticPresentationMap]) {
    let accepted = record.attempts.last().expect("accepted attempt");
    info!(
        record_id = format_args!("{:016x}", natural_record_identity(record)),
        base_seed = format_args!("{RECORDED_D6_PHYSICAL_SEED:#018x}"),
        accepted_seed = format_args!("{:#018x}", accepted.physical_seed),
        attempts = record.attempts.len(),
        natural_face = record.dice[0].natural_terminal_face,
        steps = record.calibration.fixed_steps,
        samples = record.dice[0].samples.len(),
        simulated_seconds = record.calibration.simulated_duration.as_secs_f64(),
        wall_milliseconds = record.calibration.wall_clock_duration.as_secs_f64() * 1_000.0,
        requested_order = ?mappings.iter().map(|map| map.d6[0].requested_face).collect::<Vec<_>>(),
        "Phase 1 target-blind recorded preparation"
    );
    for mapping in mappings {
        let mapping = mapping.d6[0];
        info!(
            requested = mapping.requested_face,
            natural = mapping.natural_face,
            symmetry_id = mapping.symmetry_id,
            phase_index = mapping.phase.index,
            phase_id = format_args!("{:016x}", mapping.phase.identifier),
            "Phase 1 pure d6 presentation mapping"
        );
    }
}

fn log_case_summary(sequence: &RecordedReplaySequence) {
    let mapping = sequence.mappings[sequence.index].d6[0];
    let final_sample = sequence.record.dice[0]
        .samples
        .last()
        .expect("accepted trajectory has samples");
    let visible = compose_visible_orientation(
        Quat::from_array(final_sample.unit_orientation),
        mapping.symmetry,
        Quat::IDENTITY,
    );
    info!(
        case = sequence.index + 1,
        requested = mapping.requested_face,
        natural = mapping.natural_face,
        symmetry_id = mapping.symmetry_id,
        final_visible_face = d6_geometry().upward_face(visible).value,
        record_id = format_args!(
            "{:016x}",
            sequence.mappings[sequence.index].natural_record_identity
        ),
        "Phase 1 recorded-replay case summary"
    );
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
        record: RecordedBatch,
        presentation: SemanticPresentationMap,
        root: Option<Entity>,
    }

    fn spawn_replay_fixture(
        mut commands: Commands,
        mut meshes: ResMut<Assets<Mesh>>,
        mut materials: ResMut<Assets<StandardMaterial>>,
        mut fixture: ResMut<ReplaySpawnFixture>,
    ) {
        fixture.root = Some(spawn_recorded_die(
            &mut commands,
            &mut meshes,
            &mut materials,
            &fixture.record,
            &fixture.presentation,
        ));
    }

    fn replay_fixture_app() -> (App, Transform, FixedD6Presentation) {
        let request = PhysicalBatchRequest::new(vec![DieKind::D6], RECORDED_D6_PHYSICAL_SEED);
        let record = prepare_recorded_batch(&request).expect("recorded-replay fixture preparation");
        let presentation = SemanticPresentationMap::for_single_d6(&record, 4)
            .expect("recorded-replay fixture presentation");
        let first =
            sample_recorded_transform(&record.dice[0].samples, record.fixed_step, Duration::ZERO)
                .expect("fixture first sample");
        let expected_root = Transform::from_translation(first.world_position)
            .with_rotation(first.recorded_orientation);
        let expected_presentation = FixedD6Presentation(presentation.d6[0]);
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
            record,
            presentation,
            root: None,
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
        assert_eq!(selected_requested_faces(None), vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn test_single_recorded_replay_case_selects_only_requested_mapping() {
        assert_eq!(
            selected_requested_faces(Some("d6-recorded-target-4")),
            vec![4]
        );
    }

    #[test]
    fn test_recorded_replay_spawn_builds_complete_visible_transform_hierarchy() {
        let (mut app, expected_root, expected_presentation) = replay_fixture_app();
        app.update();
        let world = app.world();
        let root = world.resource::<ReplaySpawnFixture>().root.unwrap();
        let visual = assert_playback_root(world, root, &expected_root);
        let geometry = assert_numbered_visual(world, visual, expected_presentation);
        assert_visible_geometry(world, &geometry);
    }
}
