use std::str::FromStr;

use droll_gui::spike::SpikeMode;

#[test]
fn test_recorded_replay_mode_is_explicitly_named() {
    assert_eq!(
        SpikeMode::from_str("recorded-replay"),
        Ok(SpikeMode::RecordedReplay)
    );
    assert!(SpikeMode::from_str("recorded").is_err());
}

#[test]
fn test_recorded_replay_harness_prepares_once_without_visible_physics() {
    let harness = include_str!("../src/spike/recorded_replay.rs");
    for forbidden in ["PhysicsPlugins", "RigidBody", "Collider", "PhysicsSchedule"] {
        assert!(
            !harness.contains(forbidden),
            "recorded-replay harness contains {forbidden}"
        );
    }
    assert_eq!(
        harness
            .matches("prepare_recorded_batch(&PhysicalBatchRequest")
            .count(),
        1,
        "full checkpoint must prepare physics exactly once"
    );
}
