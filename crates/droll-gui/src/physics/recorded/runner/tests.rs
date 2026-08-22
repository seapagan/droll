use super::*;
use crate::physics::recorded::types::SupportClassification;

fn resting_recorder() -> DieRecorder {
    let policy = super::super::validity::PhysicalValidityPolicy::default();
    DieRecorder {
        ordinal: 1,
        kind: DieKind::D6,
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
