use std::str::FromStr;

use droll_gui::spike::{SpikeMode, SpikeScenario};

#[test]
fn test_recorded_replay_mode_is_explicitly_named() {
    assert_eq!(
        SpikeMode::from_str("recorded-replay"),
        Ok(SpikeMode::RecordedReplay)
    );
    assert!(SpikeMode::from_str("recorded").is_err());
}

#[test]
fn test_phase3_scenario_is_explicitly_named_4d6() {
    assert_eq!(SpikeScenario::from_str("4d6"), Ok(SpikeScenario::FourD6));
    assert!(SpikeScenario::from_str("multi-d6").is_err());
}

#[test]
fn test_phase4_scenarios_are_explicitly_named() {
    assert_eq!(
        SpikeScenario::from_str("mixed10"),
        Ok(SpikeScenario::Mixed10)
    );
    assert_eq!(
        SpikeScenario::from_str("mixed20"),
        Ok(SpikeScenario::Mixed20)
    );
    assert_eq!(
        SpikeScenario::from_str("mixed50"),
        Ok(SpikeScenario::Mixed50)
    );
    assert!(SpikeScenario::from_str("2d20-8d6").is_err());
}

#[test]
fn test_recorded_replay_harness_prepares_once_without_visible_physics() {
    let harness = include_str!("../src/spike/recorded_replay.rs");
    let production = harness
        .split("#[cfg(test)]")
        .next()
        .expect("production harness source");
    for forbidden in ["PhysicsPlugins", "RigidBody", "Collider", "PhysicsSchedule"] {
        assert!(
            !harness.contains(forbidden),
            "recorded-replay harness contains {forbidden}"
        );
    }
    assert_eq!(
        production
            .matches("prepare_recorded_batch(request)")
            .count(),
        1,
        "full checkpoint must prepare physics exactly once"
    );
}
