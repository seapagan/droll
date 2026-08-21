use std::{collections::BTreeMap, time::Duration};

use avian3d::prelude::{
    AngularDamping, AngularVelocity, Collider, CollidingEntities, Friction, LinearDamping,
    LinearVelocity, PhysicsPlugins, Restitution, RigidBody, Rotation,
};
use bevy::{prelude::*, time::TimeUpdateStrategy};
use droll_gui::{
    dice::d6_geometry,
    physics::{
        D6LaunchCandidate, D6LaunchState, LaunchKind, LaunchNuisance, d6_launch_search_candidates,
        d6_passing_launch_families,
    },
};

const STEP_SECONDS: f64 = 1.0 / 60.0;
const WATCHDOG_STEPS: usize = 1_080;
const STABLE_STEPS: usize = 36;

#[derive(Clone, Copy, Debug)]
struct NaturalRun {
    face: u8,
    completed_seconds: f32,
    first_contact_seconds: f32,
    contact_steps: usize,
    max_linear_speed: f32,
    max_angular_speed: f32,
}

#[test]
fn test_bounded_search_finds_four_robust_energetic_families() {
    let candidates = d6_launch_search_candidates();
    assert_eq!(candidates.len(), 64);
    let mut accepted = BTreeMap::new();
    let mut rejected = Vec::new();
    for candidate in candidates {
        if accepted.contains_key(&kind_key(candidate.kind)) {
            continue;
        }
        match screen(candidate) {
            Ok(runs) => {
                println!("accepted,{},base-face-{}", candidate.id(), runs[0].face);
                for (nuisance, run) in LaunchNuisance::ALL.into_iter().zip(runs) {
                    println!(
                        "screen,{},{},{},{:.3},{:.3},{},{:.4},{:.4}",
                        candidate.id(),
                        nuisance.as_str(),
                        run.face,
                        run.first_contact_seconds,
                        run.completed_seconds,
                        run.contact_steps,
                        run.max_linear_speed,
                        run.max_angular_speed
                    );
                }
                accepted.insert(kind_key(candidate.kind), candidate);
            }
            Err(reason) => {
                println!("rejected,{},{}", candidate.id(), reason);
                rejected.push((candidate.id(), reason));
            }
        }
    }
    println!(
        "search-summary,cap-{},considered-{},accepted-{},rejected-{}",
        64,
        accepted.len() + rejected.len(),
        accepted.len(),
        rejected.len()
    );
    assert_eq!(
        accepted.len(),
        4,
        "accepted={accepted:?}, rejected={rejected:?}"
    );
    let selected = d6_passing_launch_families();
    assert_eq!(
        selected.map(|family| family.candidate.search_id),
        [1, 16, 32, 48]
    );
    for family in selected {
        let accepted_candidate = accepted
            .get(&kind_key(family.candidate.kind))
            .expect("each selected kind passed");
        assert_eq!(*accepted_candidate, family.candidate);
    }
}

fn screen(candidate: D6LaunchCandidate) -> Result<Vec<NaturalRun>, String> {
    let mut runs: Vec<NaturalRun> = Vec::with_capacity(LaunchNuisance::ALL.len());
    for nuisance in LaunchNuisance::ALL {
        let run = run_natural(nuisance.apply(candidate.state))?;
        if let Some(nominal) = runs.first()
            && run.face != nominal.face
        {
            return Err(format!(
                "{} changed face {}->{}",
                nuisance.as_str(),
                nominal.face,
                run.face
            ));
        }
        runs.push(run);
    }
    Ok(runs)
}

fn run_natural(state: D6LaunchState) -> Result<NaturalRun, String> {
    let (mut app, die, floor) = build_app(state);
    let mut stable_steps = 0;
    let mut contact_steps = 0;
    let mut first_contact = None;
    let mut max_linear = 0.0_f32;
    let mut max_angular = 0.0_f32;
    for step in 1..=WATCHDOG_STEPS {
        app.update();
        let world = app.world();
        let linear = world.get::<LinearVelocity>(die).expect("linear velocity");
        let angular = world.get::<AngularVelocity>(die).expect("angular velocity");
        let collisions = world
            .get::<CollidingEntities>(die)
            .expect("collision state");
        let contact = collisions.contains(&floor);
        max_linear = max_linear.max(linear.length());
        max_angular = max_angular.max(angular.length());
        if contact {
            contact_steps += 1;
            first_contact.get_or_insert(step as f32 * STEP_SECONDS as f32);
        }
        if contact && linear.length() <= 0.10 && angular.length() <= 0.16 {
            stable_steps += 1;
        } else {
            stable_steps = 0;
        }
        if stable_steps >= STABLE_STEPS {
            if max_linear < 2.0 || max_angular < 5.0 || contact_steps == 0 {
                return Err("insufficient energetic throw/contact".to_owned());
            }
            let rotation = world.get::<Rotation>(die).expect("rotation").0;
            return Ok(NaturalRun {
                face: d6_geometry().upward_face(rotation).value,
                completed_seconds: step as f32 * STEP_SECONDS as f32,
                first_contact_seconds: first_contact.expect("stable rest followed contact"),
                contact_steps,
                max_linear_speed: max_linear,
                max_angular_speed: max_angular,
            });
        }
    }
    Err("watchdog timeout".to_owned())
}

fn build_app(state: D6LaunchState) -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, PhysicsPlugins::default()));
    app.finish();
    app.cleanup();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        STEP_SECONDS,
    )));
    let floor = app
        .world_mut()
        .spawn((
            RigidBody::Static,
            Collider::cuboid(8.0, 0.2, 6.0),
            Friction::new(0.72),
            Transform::from_xyz(0.0, -0.1, 0.0),
        ))
        .id();
    for (size, translation) in [
        (Vec3::new(8.0, 0.8, 0.2), Vec3::new(0.0, 0.4, -3.0)),
        (Vec3::new(8.0, 0.8, 0.2), Vec3::new(0.0, 0.4, 3.0)),
        (Vec3::new(0.2, 0.8, 6.0), Vec3::new(-4.0, 0.4, 0.0)),
        (Vec3::new(0.2, 0.8, 6.0), Vec3::new(4.0, 0.4, 0.0)),
    ] {
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(size.x, size.y, size.z),
            Friction::new(0.72),
            Transform::from_translation(translation),
        ));
    }
    let die = app
        .world_mut()
        .spawn((
            RigidBody::Dynamic,
            Collider::convex_hull(d6_geometry().collider_vertices()).expect("valid d6 hull"),
            CollidingEntities::default(),
            Friction::new(0.65),
            Restitution::new(0.32),
            LinearDamping(0.18),
            AngularDamping(0.28),
            LinearVelocity(state.linear_velocity),
            AngularVelocity(state.angular_velocity),
            Transform::from_translation(state.position).with_rotation(state.orientation),
        ))
        .id();
    (app, die, floor)
}

const fn kind_key(kind: LaunchKind) -> u8 {
    match kind {
        LaunchKind::HighTumble => 0,
        LaunchKind::SideSpin => 1,
        LaunchKind::OverheadTumble => 2,
        LaunchKind::DiagonalSpin => 3,
    }
}
