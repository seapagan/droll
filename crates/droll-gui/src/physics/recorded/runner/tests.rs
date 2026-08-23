use super::*;
use crate::physics::recorded::types::{PhysicalTray, SupportClassification};

fn resting_recorder() -> DieRecorder {
    let policy = super::super::validity::PhysicalValidityPolicy::default();
    DieRecorder {
        ordinal: 1,
        kind: DieKind::D6,
        initial: initial_state(Vec3::ZERO, Quat::IDENTITY, Vec3::ZERO, Vec3::ZERO),
        samples: Vec::new(),
        stable_steps: 0,
        stable_face: None,
        stable_support: None,
        support_invalidity: None,
        last_face: observe_face(DieKind::D6, Quat::IDENTITY, policy),
        linear_speed: 0.0,
        angular_speed: 0.0,
        active_contacts: Vec::new(),
        contact_diagnostics: ContactDiagnostics::default(),
    }
}

fn stack_support(
    supporting_dice: &[(u16, Vec3)],
) -> Result<SupportClassification, InvalidityReason> {
    classify_support(
        1,
        Vec3::new(0.40, 1.0, 0.0),
        false,
        supporting_dice,
        super::super::validity::PhysicalValidityPolicy::default(),
    )
}

fn sample(recorder: &mut DieRecorder, support: Result<SupportClassification, InvalidityReason>) {
    update_stability(
        recorder,
        support,
        super::super::validity::PhysicalValidityPolicy::default(),
    );
}

#[test]
fn test_unreadable_stack_cannot_accumulate_stability() {
    let mut recorder = resting_recorder();
    for _ in 0..36 {
        sample(
            &mut recorder,
            stack_support(&[(0, Vec3::new(0.30, 0.20, 0.0))]),
        );
    }
    assert_eq!(recorder.stable_steps, 0);
    assert_eq!(
        recorder.support_invalidity,
        Some(InvalidityReason::UnreadableOrPathologicalStack { ordinal: 1 })
    );
}

#[test]
fn test_newly_readable_stack_starts_a_fresh_window() {
    let mut recorder = resting_recorder();
    for _ in 0..35 {
        sample(
            &mut recorder,
            stack_support(&[(0, Vec3::new(0.30, 0.20, 0.0))]),
        );
    }
    sample(
        &mut recorder,
        stack_support(&[(0, Vec3::new(0.0, 0.20, 0.0))]),
    );
    assert_eq!(recorder.stable_steps, 1);
}

#[test]
fn test_losing_readable_support_resets_stability() {
    let mut recorder = resting_recorder();
    for _ in 0..20 {
        sample(
            &mut recorder,
            stack_support(&[(0, Vec3::new(0.0, 0.20, 0.0))]),
        );
    }
    sample(&mut recorder, stack_support(&[]));
    assert_eq!(recorder.stable_steps, 0);
    assert_eq!(
        recorder.support_invalidity,
        Some(InvalidityReason::UnsupportedDie { ordinal: 1 })
    );
}

#[test]
fn test_changing_support_relationship_starts_a_fresh_window() {
    let mut recorder = resting_recorder();
    for _ in 0..20 {
        sample(
            &mut recorder,
            stack_support(&[(0, Vec3::new(0.0, 0.20, 0.0))]),
        );
    }
    sample(
        &mut recorder,
        stack_support(&[(2, Vec3::new(0.0, 0.20, 0.0))]),
    );
    assert_eq!(recorder.stable_steps, 1);
    assert_eq!(
        recorder.stable_support,
        Some(SupportClassification::ReadableStack {
            supporting_ordinals: vec![2]
        })
    );
}

#[test]
fn test_continuously_readable_stack_completes_stable_window() {
    let mut recorder = resting_recorder();
    for _ in 0..36 {
        sample(
            &mut recorder,
            stack_support(&[(0, Vec3::new(0.0, 0.20, 0.0))]),
        );
    }
    assert_eq!(recorder.stable_steps, 36);
}

#[test]
fn test_tray_supported_die_still_completes_stable_window() {
    let mut recorder = resting_recorder();
    for _ in 0..36 {
        sample(&mut recorder, Ok(SupportClassification::Tray));
    }
    assert_eq!(recorder.stable_steps, 36);
}

#[test]
fn test_watchdog_preserves_stable_support_invalidity() {
    for (support, expected) in [
        (
            stack_support(&[]),
            InvalidityReason::UnsupportedDie { ordinal: 1 },
        ),
        (
            stack_support(&[(0, Vec3::new(0.30, 0.20, 0.0))]),
            InvalidityReason::UnreadableOrPathologicalStack { ordinal: 1 },
        ),
        (
            Err(InvalidityReason::EdgeOrCornerTraySupport { ordinal: 1 }),
            InvalidityReason::EdgeOrCornerTraySupport { ordinal: 1 },
        ),
    ] {
        let mut recorder = resting_recorder();
        sample(&mut recorder, support);
        assert_eq!(
            terminal_watchdog_reason(
                &[recorder],
                super::super::validity::PhysicalValidityPolicy::default()
            ),
            expected
        );
    }
}

#[test]
fn test_d6_face_bearing_tray_contact_passes() {
    assert!(d6_tray_features_are_face_bearing([
        PackedFeatureId::vertex(0),
        PackedFeatureId::vertex(1),
        PackedFeatureId::vertex(2),
    ]));
    assert!(d6_tray_features_are_face_bearing([PackedFeatureId::face(
        0
    )]));
}

#[test]
fn test_d6_edge_bearing_tray_contact_fails() {
    assert!(!d6_tray_features_are_face_bearing([
        PackedFeatureId::vertex(0),
        PackedFeatureId::vertex(1),
    ]));
}

#[test]
fn test_d6_corner_bearing_tray_contact_fails() {
    assert!(!d6_tray_features_are_face_bearing([
        PackedFeatureId::vertex(0)
    ]));
}

#[test]
fn test_edge_or_corner_tray_support_cannot_accumulate_stability() {
    let mut recorder = resting_recorder();
    let invalid = InvalidityReason::EdgeOrCornerTraySupport { ordinal: 1 };
    for _ in 0..36 {
        sample(&mut recorder, Err(invalid));
    }
    assert_eq!(recorder.stable_steps, 0);
    assert_eq!(recorder.support_invalidity, Some(invalid));
}

#[test]
fn test_becoming_face_supported_restarts_the_full_stable_window() {
    let mut recorder = resting_recorder();
    for _ in 0..20 {
        sample(&mut recorder, Ok(SupportClassification::Tray));
    }
    assert_eq!(recorder.stable_steps, 20);
    for _ in 0..35 {
        sample(
            &mut recorder,
            Err(InvalidityReason::EdgeOrCornerTraySupport { ordinal: 1 }),
        );
    }
    sample(&mut recorder, Ok(SupportClassification::Tray));
    assert_eq!(recorder.stable_steps, 1);
}

#[test]
fn test_rejected_phase3_attempt_remains_host_local_diagnostic_evidence() {
    const BASE_SEED: u64 = 0x4D6A_1E00_0000_0003;
    let request = PhysicalBatchRequest::new(vec![DieKind::D6; 4], BASE_SEED);
    let seed = attempt_seed(BASE_SEED, 1);
    assert_eq!(seed, 0x8D03_F82B_7AFD_ABA8);
    match run_attempt(&request, seed) {
        Err((InvalidityReason::EdgeOrCornerTraySupport { ordinal }, attempt)) => {
            let rejected = attempt
                .dice
                .get(usize::from(ordinal))
                .expect("invalidity must identify a die in the attempted batch");
            assert_eq!(rejected.ordinal, ordinal);
            assert!(!rejected.samples.is_empty());
        }
        Ok(attempt) => eprintln!(
            "historical Phase 3 attempt is valid on this host: steps={}",
            attempt.fixed_steps
        ),
        Err((reason, attempt)) => eprintln!(
            "historical Phase 3 attempt has a different host-local outcome: reason={reason:?} steps={}",
            attempt.fixed_steps
        ),
    }
}

#[test]
fn test_phase4_mixed_launches_are_seeded_contained_and_nonoverlapping() {
    for (count, tray) in [
        (
            10,
            PhysicalTray {
                width: 10.0,
                depth: 8.0,
                wall_height: 1.0,
            },
        ),
        (
            20,
            PhysicalTray {
                width: 14.0,
                depth: 12.0,
                wall_height: 1.0,
            },
        ),
        (
            50,
            PhysicalTray {
                width: 18.0,
                depth: 14.0,
                wall_height: 1.0,
            },
        ),
    ] {
        let kinds = std::iter::repeat_n(DieKind::D20, count / 5)
            .chain(std::iter::repeat_n(DieKind::D6, count - count / 5))
            .collect::<Vec<_>>();
        let mut first_rng = PhysicalRng::new(0xA11C_E55E);
        let first = mixed_handful_launches(&kinds, tray, &mut first_rng);
        let mut second_rng = PhysicalRng::new(0xA11C_E55E);
        let second = mixed_handful_launches(&kinds, tray, &mut second_rng);
        assert_eq!(first, second);
        for (index, launch) in first.iter().enumerate() {
            let position = Vec3::from_array(launch.world_position);
            assert!(is_contained(kinds[index], position, tray));
            assert!(Quat::from_array(launch.unit_orientation).is_finite());
            assert!(Vec3::from_array(launch.linear_velocity).is_finite());
            assert!(Vec3::from_array(launch.angular_velocity).is_finite());
            for prior in 0..index {
                let prior_position = Vec3::from_array(first[prior].world_position);
                assert!(
                    position.distance(prior_position)
                        > kinds[index].circumradius() + kinds[prior].circumradius(),
                    "bounding spheres overlap for count {count}, ordinals {prior}/{index}"
                );
            }
        }
    }
}
