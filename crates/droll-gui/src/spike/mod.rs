//! Bounded, development-only Stage 1 physics scenario harness.

mod recorded_replay;
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
    DieLifecycle, DieMetrics, DieState, DirectedDie, DirectedPhysicsPlugin, SymmetryDie,
    SymmetryDieState, SymmetryLifecycle, SymmetryMetrics, SymmetryPhysicsPlugin, TraySurface,
    directed_d6_components, symmetry_d6_components,
};
use recorded_replay::{
    advance_recorded_replay_cases, prepare_mixed50_diagnostic, report_mixed50_diagnostic,
    setup_recorded_replay, update_recorded_replay_window_status,
};

pub use scenario::{
    SpikeCase, SpikeMode, SpikeScenario, StartCase, nominal_nuisance, symmetry_d6_cases,
};

#[derive(Resource, Clone, Debug)]
pub struct SpikeOptions {
    pub scenario: SpikeScenario,
    pub case_id: Option<String>,
    pub mode: SpikeMode,
}

#[derive(Resource)]
struct CaseSequence {
    cases: Vec<SpikeCase>,
    index: usize,
    active: Entity,
    terminal_hold_seconds: f32,
    mode: SpikeMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SpikeDispatch {
    Graphical,
    HeadlessMixed50Diagnostic,
}

enum SpikeExecution<T> {
    Graphical(T),
    HeadlessMixed50Diagnostic(recorded_replay::Mixed50DiagnosticOutcome),
}

fn spike_dispatch(options: &SpikeOptions) -> SpikeDispatch {
    if options.mode == SpikeMode::RecordedReplay && options.scenario == SpikeScenario::Mixed50 {
        SpikeDispatch::HeadlessMixed50Diagnostic
    } else {
        SpikeDispatch::Graphical
    }
}

fn execute_spike_dispatch<T>(
    options: SpikeOptions,
    prepare_headless: impl FnOnce() -> recorded_replay::Mixed50DiagnosticOutcome,
    build_graphical: impl FnOnce(SpikeOptions) -> T,
) -> SpikeExecution<T> {
    match spike_dispatch(&options) {
        SpikeDispatch::HeadlessMixed50Diagnostic => {
            SpikeExecution::HeadlessMixed50Diagnostic(prepare_headless())
        }
        SpikeDispatch::Graphical => SpikeExecution::Graphical(build_graphical(options)),
    }
}

/// Runs the requested Stage 1 spike through its graphical or diagnostic path.
pub fn run_spike(options: SpikeOptions) {
    match execute_spike_dispatch(options, prepare_mixed50_diagnostic, build_spike_app) {
        SpikeExecution::HeadlessMixed50Diagnostic(outcome) => {
            report_mixed50_diagnostic(&outcome);
        }
        SpikeExecution::Graphical(mut app) => {
            app.run();
        }
    }
}

/// Builds the bounded real-window Stage 1 spike application.
pub fn build_spike_app(options: SpikeOptions) -> App {
    assert_ne!(
        spike_dispatch(&options),
        SpikeDispatch::HeadlessMixed50Diagnostic,
        "mixed50 recorded replay is a headless diagnostic"
    );
    let title = format!(
        "Droll Stage 1 - {} - {}",
        options.scenario.as_str(),
        options.mode.as_str()
    );
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title,
            present_mode: PresentMode::AutoVsync,
            ..default()
        }),
        ..default()
    }));
    match options.mode {
        SpikeMode::CandidateD => app.add_plugins((
            avian3d::prelude::PhysicsPlugins::default(),
            DirectedPhysicsPlugin,
        )),
        SpikeMode::RecordedReplay => app.add_plugins(crate::physics::RecordedPlaybackPlugin),
        SpikeMode::SymmetryLaunch => app.add_plugins((
            avian3d::prelude::PhysicsPlugins::default(),
            SymmetryPhysicsPlugin,
        )),
    };
    let mode = options.mode;
    app.insert_resource(options);
    if mode == SpikeMode::RecordedReplay {
        app.add_systems(Startup, setup_recorded_replay).add_systems(
            Update,
            (
                advance_recorded_replay_cases,
                update_recorded_replay_window_status,
                escape_to_exit,
            ),
        );
    } else {
        app.add_systems(Startup, setup_spike).add_systems(
            Update,
            (advance_cases, update_window_status, escape_to_exit),
        );
    }
    app
}

fn setup_spike(
    mut commands: Commands,
    options: Res<SpikeOptions>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let cases = selected_cases(&options).unwrap_or_else(|message| panic!("{message}"));
    let first = cases[0].clone();
    let active = spawn_scene(
        &mut commands,
        &mut meshes,
        &mut materials,
        &first,
        options.mode,
    );
    commands.insert_resource(CaseSequence {
        cases,
        index: 0,
        active,
        terminal_hold_seconds: 0.0,
        mode: options.mode,
    });
}

fn selected_cases(options: &SpikeOptions) -> Result<Vec<SpikeCase>, String> {
    match options.mode {
        SpikeMode::CandidateD => options.scenario.selected_cases(options.case_id.as_deref()),
        SpikeMode::RecordedReplay => unreachable!("recorded replay has an isolated setup path"),
        SpikeMode::SymmetryLaunch if options.scenario == SpikeScenario::D6Faces => {
            let cases = symmetry_d6_cases();
            match options.case_id.as_deref() {
                None => Ok(cases),
                Some(id) => cases
                    .into_iter()
                    .find(|case| case.id == id)
                    .map(|case| vec![case])
                    .ok_or_else(|| format!("unknown symmetry-launch case `{id}`")),
            }
        }
        SpikeMode::SymmetryLaunch => Err(format!(
            "mode `symmetry-launch` supports only scenario `d6-faces`, not `{}`",
            options.scenario
        )),
    }
}

fn spawn_scene(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    case: &SpikeCase,
    mode: SpikeMode,
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
        TraySurface,
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
            TraySurface,
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
    spawn_die(commands, meshes, materials, case, mode)
}

#[derive(Component)]
struct SpikeDie;

fn spawn_die(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    case: &SpikeCase,
    mode: SpikeMode,
) -> Entity {
    let mut entity = commands.spawn((
        SpikeDie,
        Mesh3d(meshes.add(d6_mesh())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: GOLD.into(),
            perceptual_roughness: 0.45,
            ..default()
        })),
    ));
    match mode {
        SpikeMode::CandidateD => entity.insert(directed_d6_components(case)),
        SpikeMode::RecordedReplay => unreachable!("recorded replay uses a render-only hierarchy"),
        SpikeMode::SymmetryLaunch => {
            let family = case.start.symmetry_family().expect("H1 case has a family");
            entity.insert(symmetry_d6_components(
                case.id.clone(),
                family,
                nominal_nuisance(),
                case.target_face,
            ))
        }
    };
    let entity = entity
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
        mode = mode.as_str(),
        "Stage 1 spike case"
    );
    entity
}

fn advance_cases(
    mut commands: Commands,
    time: Res<Time>,
    mut sequence: ResMut<CaseSequence>,
    dice: Query<(&DieState, &DieMetrics), With<SpikeDie>>,
    symmetry_dice: Query<(&SymmetryDieState, &SymmetryMetrics), With<SpikeDie>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut exit: MessageWriter<AppExit>,
) {
    let (terminal, recoveries, completion_seconds) = match sequence.mode {
        SpikeMode::CandidateD => {
            let Ok((state, metrics)) = dice.get(sequence.active) else {
                return;
            };
            (
                matches!(
                    state.lifecycle,
                    DieLifecycle::Revealed | DieLifecycle::Failed
                ),
                state.recovery_count,
                metrics.terminal_seconds,
            )
        }
        SpikeMode::RecordedReplay => unreachable!("recorded replay uses isolated systems"),
        SpikeMode::SymmetryLaunch => {
            let Ok((state, metrics)) = symmetry_dice.get(sequence.active) else {
                return;
            };
            (
                matches!(
                    state.lifecycle,
                    SymmetryLifecycle::Revealed | SymmetryLifecycle::Failed
                ),
                metrics.recovery_count,
                metrics.terminal_seconds,
            )
        }
    };
    if !terminal {
        return;
    }
    sequence.terminal_hold_seconds += time.delta_secs();
    if sequence.terminal_hold_seconds < 1.5 {
        return;
    }
    info!(
        case = sequence.cases[sequence.index].id,
        recoveries, completion_seconds, "Stage 1 case summary"
    );
    if sequence.index + 1 == sequence.cases.len() {
        exit.write(AppExit::Success);
        return;
    }
    commands.entity(sequence.active).despawn();
    sequence.index += 1;
    sequence.terminal_hold_seconds = 0.0;
    let case = sequence.cases[sequence.index].clone();
    sequence.active = spawn_die(
        &mut commands,
        &mut meshes,
        &mut materials,
        &case,
        sequence.mode,
    );
}

fn update_window_status(
    sequence: Res<CaseSequence>,
    dice: Query<(&DirectedDie, &DieState, &avian3d::prelude::Rotation), With<SpikeDie>>,
    symmetry_dice: Query<
        (&SymmetryDie, &SymmetryDieState, &avian3d::prelude::Rotation),
        With<SpikeDie>,
    >,
    mut windows: Query<&mut Window>,
) {
    let (case_id, target_face, current, lifecycle) = match sequence.mode {
        SpikeMode::CandidateD => {
            let Ok((die, state, rotation)) = dice.get(sequence.active) else {
                return;
            };
            (
                die.case_id.as_str(),
                die.target_face,
                d6_geometry().upward_face(rotation.0).value,
                format!("{:?}", state.lifecycle),
            )
        }
        SpikeMode::RecordedReplay => unreachable!("recorded replay uses isolated systems"),
        SpikeMode::SymmetryLaunch => {
            let Ok((die, state, rotation)) = symmetry_dice.get(sequence.active) else {
                return;
            };
            (
                die.case_id.as_str(),
                die.target_face,
                d6_geometry().upward_face(rotation.0).value,
                format!("{:?}", state.lifecycle),
            )
        }
    };
    for mut window in &mut windows {
        window.title = format!(
            "Droll Stage 1 - {} - target {} - current {} - {}",
            case_id, target_face, current, lifecycle
        );
    }
}

fn escape_to_exit(keys: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use crate::physics::PreparationFailure;

    use super::*;

    fn recorded_options(scenario: SpikeScenario) -> SpikeOptions {
        SpikeOptions {
            scenario,
            case_id: None,
            mode: SpikeMode::RecordedReplay,
        }
    }

    #[test]
    fn test_mixed50_recorded_replay_dispatches_headlessly_without_building_app() {
        let preparation_count = Cell::new(0);
        let graphical_build_count = Cell::new(0);
        let execution = execute_spike_dispatch(
            recorded_options(SpikeScenario::Mixed50),
            || {
                preparation_count.set(preparation_count.get() + 1);
                recorded_replay::Mixed50DiagnosticOutcome::BoundedExhaustion(PreparationFailure {
                    attempts: Vec::new(),
                })
            },
            |_| {
                graphical_build_count.set(graphical_build_count.get() + 1);
            },
        );

        assert!(matches!(
            execution,
            SpikeExecution::HeadlessMixed50Diagnostic(_)
        ));
        assert_eq!(preparation_count.get(), 1);
        assert_eq!(graphical_build_count.get(), 0);
    }

    #[test]
    fn test_mixed10_and_mixed20_recorded_replay_dispatch_graphically() {
        for scenario in [SpikeScenario::Mixed10, SpikeScenario::Mixed20] {
            let preparation_count = Cell::new(0);
            let graphical_build_count = Cell::new(0);
            let execution = execute_spike_dispatch(
                recorded_options(scenario),
                || {
                    preparation_count.set(preparation_count.get() + 1);
                    recorded_replay::Mixed50DiagnosticOutcome::BoundedExhaustion(
                        PreparationFailure {
                            attempts: Vec::new(),
                        },
                    )
                },
                |_| {
                    graphical_build_count.set(graphical_build_count.get() + 1);
                },
            );

            assert!(matches!(execution, SpikeExecution::Graphical(())));
            assert_eq!(preparation_count.get(), 0);
            assert_eq!(graphical_build_count.get(), 1);
        }
    }
}
