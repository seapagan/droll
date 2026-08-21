//! Bounded, development-only directed-physics scenario harness.

mod scenario;

use std::time::Duration;

use bevy::{
    app::AppExit,
    color::palettes::css::{BLACK, DARK_SLATE_GRAY, GOLD},
    prelude::*,
    window::{PresentMode, WindowPlugin},
};

use crate::dice::{d6_geometry, d6_labels, d6_mesh};

pub use scenario::{SpikeCase, SpikeScenario, StartCase};

#[derive(Resource, Clone, Debug)]
pub struct SpikeOptions {
    pub scenario: SpikeScenario,
    pub case_id: Option<String>,
}

#[derive(Resource)]
struct CaseSequence {
    cases: Vec<SpikeCase>,
    index: usize,
    elapsed: Duration,
}

/// Builds the bounded real-window Stage 1 spike application.
pub fn build_spike_app(options: SpikeOptions) -> App {
    let title = format!("Droll Stage 1 - {}", options.scenario.as_str());
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title,
                present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }),
        avian3d::prelude::PhysicsPlugins::default(),
    ));
    app.insert_resource(options)
        .add_systems(Startup, setup_spike)
        .add_systems(Update, (advance_preview_cases, escape_to_exit));
    app
}

fn setup_spike(
    mut commands: Commands,
    options: Res<SpikeOptions>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let cases = options
        .scenario
        .selected_cases(options.case_id.as_deref())
        .unwrap_or_else(|message| panic!("{message}"));
    let first = cases[0].clone();
    commands.insert_resource(CaseSequence {
        cases,
        index: 0,
        elapsed: Duration::ZERO,
    });
    spawn_scene(&mut commands, &mut meshes, &mut materials, &first);
}

fn spawn_scene(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    case: &SpikeCase,
) {
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(8.0, 0.2, 6.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: DARK_SLATE_GRAY.into(),
            perceptual_roughness: 0.9,
            ..default()
        })),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));
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
    spawn_preview_die(commands, meshes, materials, case);
}

#[derive(Component)]
struct PreviewDie;

fn spawn_preview_die(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    case: &SpikeCase,
) {
    let geometry = d6_geometry();
    let rotation = geometry
        .face(case.target_face)
        .expect("scenario targets a d6 face")
        .target_rotation(case.yaw_radians);
    commands
        .spawn((
            PreviewDie,
            Mesh3d(meshes.add(d6_mesh())),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: GOLD.into(),
                perceptual_roughness: 0.45,
                ..default()
            })),
            Transform::from_xyz(0.0, 0.7, 0.0).with_rotation(rotation),
        ))
        .with_children(|parent| {
            let pip_mesh = meshes.add(Sphere::new(0.055));
            let pip_material = materials.add(StandardMaterial {
                base_color: BLACK.into(),
                ..default()
            });
            for label in d6_labels() {
                parent.spawn((
                    Mesh3d(pip_mesh.clone()),
                    MeshMaterial3d(pip_material.clone()),
                    Transform::from_translation(label.center),
                ));
            }
        });
    info!(
        scenario = case.scenario.as_str(),
        case = case.id,
        target = case.target_face,
        state = "geometry-preview",
        "Stage 1 spike case"
    );
}

fn advance_preview_cases(
    time: Res<Time>,
    mut sequence: ResMut<CaseSequence>,
    mut windows: Query<&mut Window>,
    mut dice: Query<&mut Transform, With<PreviewDie>>,
) {
    sequence.elapsed += time.delta();
    if sequence.elapsed < Duration::from_secs(3) {
        return;
    }
    sequence.elapsed = Duration::ZERO;
    sequence.index = (sequence.index + 1) % sequence.cases.len();
    let case = &sequence.cases[sequence.index];
    let rotation = d6_geometry()
        .face(case.target_face)
        .expect("scenario targets a d6 face")
        .target_rotation(case.yaw_radians);
    for mut transform in &mut dice {
        transform.rotation = rotation;
    }
    for mut window in &mut windows {
        window.title = format!(
            "Droll Stage 1 - {} - {} - target {} - geometry preview",
            case.scenario.as_str(),
            case.id,
            case.target_face
        );
    }
}

fn escape_to_exit(keys: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}
