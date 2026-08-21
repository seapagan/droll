//! Bounded, development-only directed-physics scenario harness.

mod scenario;

use avian3d::prelude::{Collider, Friction, RigidBody};
use bevy::{
    app::AppExit,
    color::palettes::css::{BLACK, DARK_SLATE_GRAY, GOLD},
    prelude::*,
    window::{PresentMode, WindowPlugin},
};

use crate::dice::{d6_geometry, d6_labels, d6_mesh};
use crate::physics::{
    DieLifecycle, DieMetrics, DieState, DirectedDie, DirectedPhysicsPlugin, directed_d6_components,
};

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
    active: Entity,
    terminal_hold_seconds: f32,
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
        DirectedPhysicsPlugin,
    ));
    app.insert_resource(options)
        .add_systems(Startup, setup_spike)
        .add_systems(
            Update,
            (advance_cases, update_window_status, escape_to_exit),
        );
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
    let active = spawn_scene(&mut commands, &mut meshes, &mut materials, &first);
    commands.insert_resource(CaseSequence {
        cases,
        index: 0,
        active,
        terminal_hold_seconds: 0.0,
    });
}

fn spawn_scene(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    case: &SpikeCase,
) -> Entity {
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(8.0, 0.2, 6.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: DARK_SLATE_GRAY.into(),
            perceptual_roughness: 0.9,
            ..default()
        })),
        Transform::from_xyz(0.0, -0.1, 0.0),
        RigidBody::Static,
        Collider::cuboid(8.0, 0.2, 6.0),
        Friction::new(0.72),
    ));
    let wall_material = materials.add(StandardMaterial {
        base_color: DARK_SLATE_GRAY.into(),
        perceptual_roughness: 0.9,
        ..default()
    });
    for (size, translation) in [
        (Vec3::new(8.0, 0.8, 0.2), Vec3::new(0.0, 0.4, -3.0)),
        (Vec3::new(8.0, 0.8, 0.2), Vec3::new(0.0, 0.4, 3.0)),
        (Vec3::new(0.2, 0.8, 6.0), Vec3::new(-4.0, 0.4, 0.0)),
        (Vec3::new(0.2, 0.8, 6.0), Vec3::new(4.0, 0.4, 0.0)),
    ] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(wall_material.clone()),
            Transform::from_translation(translation),
            RigidBody::Static,
            Collider::cuboid(size.x, size.y, size.z),
            Friction::new(0.72),
        ));
    }
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
    spawn_directed_die(commands, meshes, materials, case)
}

#[derive(Component)]
struct SpikeDie;

fn spawn_directed_die(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    case: &SpikeCase,
) -> Entity {
    let entity = commands
        .spawn((
            SpikeDie,
            Mesh3d(meshes.add(d6_mesh())),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: GOLD.into(),
                perceptual_roughness: 0.45,
                ..default()
            })),
            directed_d6_components(case),
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
        })
        .id();
    info!(
        scenario = case.scenario.as_str(),
        case = case.id,
        target = case.target_face,
        state = "spawned",
        "Stage 1 spike case"
    );
    entity
}

fn advance_cases(
    mut commands: Commands,
    time: Res<Time>,
    mut sequence: ResMut<CaseSequence>,
    dice: Query<(&DieState, &DieMetrics), With<SpikeDie>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok((state, metrics)) = dice.get(sequence.active) else {
        return;
    };
    if !matches!(
        state.lifecycle,
        DieLifecycle::Revealed | DieLifecycle::Failed
    ) {
        return;
    }
    sequence.terminal_hold_seconds += time.delta_secs();
    if sequence.terminal_hold_seconds < 1.5 {
        return;
    }
    info!(
        case = sequence.cases[sequence.index].id,
        terminal = ?state.lifecycle,
        recoveries = state.recovery_count,
        completion_seconds = metrics.terminal_seconds,
        "Stage 1 case summary"
    );
    if sequence.index + 1 == sequence.cases.len() {
        exit.write(AppExit::Success);
        return;
    }
    commands.entity(sequence.active).despawn();
    sequence.index += 1;
    sequence.terminal_hold_seconds = 0.0;
    let case = sequence.cases[sequence.index].clone();
    sequence.active = spawn_directed_die(&mut commands, &mut meshes, &mut materials, &case);
}

fn update_window_status(
    sequence: Res<CaseSequence>,
    dice: Query<(&DirectedDie, &DieState, &avian3d::prelude::Rotation), With<SpikeDie>>,
    mut windows: Query<&mut Window>,
) {
    let Ok((die, state, rotation)) = dice.get(sequence.active) else {
        return;
    };
    let current = d6_geometry().upward_face(rotation.0).value;
    for mut window in &mut windows {
        window.title = format!(
            "Droll Stage 1 - {} - target {} - current {} - {:?}",
            die.case_id, die.target_face, current, state.lifecycle
        );
    }
}

fn escape_to_exit(keys: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}
