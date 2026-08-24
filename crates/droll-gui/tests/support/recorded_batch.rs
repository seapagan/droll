use std::time::Duration;

use droll_gui::{
    dice::{d6_geometry, d20_geometry},
    physics::{
        AttemptDiagnostic, AttemptOutcome, BatchContactDiagnostics, CalibrationMetrics,
        ContactDiagnostics, DiceContactSample, DieKind, InitialPhysicalState,
        NaturalTerminalDiagnostics, PhysicalTray, RecordedBatch, RecordedDie,
        SupportClassification, TrajectorySample,
    },
};

pub fn mixed_record(count: usize, physical_seed: u64) -> RecordedBatch {
    let d20_count = count / 5;
    let kinds = std::iter::repeat_n(DieKind::D20, d20_count)
        .chain(std::iter::repeat_n(DieKind::D6, count - d20_count));
    let dice = kinds
        .enumerate()
        .map(|(ordinal, kind)| recorded_die(ordinal, kind))
        .collect::<Vec<_>>();
    let sample_count = dice.iter().map(|die| die.samples.len()).sum();
    let mixed_contact = DiceContactSample {
        fixed_step: 2,
        first_ordinal: 0,
        second_ordinal: u16::try_from(d20_count).expect("test fixture count"),
        world_point: [0.0, 0.7, 0.0],
        world_normal: [1.0, 0.0, 0.0],
        normal_impulse: 0.8,
        approach_speed: 1.2,
    };
    RecordedBatch {
        fixed_step: Duration::from_nanos(16_666_667),
        tray: PhysicalTray {
            width: 14.0,
            depth: 12.0,
            wall_height: 1.0,
        },
        attempts: vec![attempt_diagnostic(count, physical_seed, sample_count)],
        dice,
        contacts: BatchContactDiagnostics {
            dice_contact_samples: vec![mixed_contact],
            dice_contact_interactions: 1,
            max_simultaneous_dice_pairs: 1,
            strongest_dice_contact: Some(mixed_contact),
        },
        calibration: calibration(count, sample_count),
    }
}

fn attempt_diagnostic(count: usize, physical_seed: u64, sample_count: usize) -> AttemptDiagnostic {
    AttemptDiagnostic {
        attempt: 1,
        physical_seed,
        die_count: count,
        fixed_steps: 2,
        simulated_duration: Duration::from_nanos(33_333_334),
        wall_clock_duration: Duration::ZERO,
        world_construction_duration: Duration::ZERO,
        trajectory_sample_count: sample_count,
        raw_trajectory_payload_bytes: sample_count * std::mem::size_of::<TrajectorySample>(),
        trajectory_capacity_bytes: sample_count * std::mem::size_of::<TrajectorySample>(),
        record_container_bytes: std::mem::size_of::<RecordedBatch>(),
        dice_contact_sample_count: 1,
        dice_contact_interactions: 1,
        max_simultaneous_dice_pairs: 1,
        outcome: AttemptOutcome::Valid,
    }
}

fn calibration(count: usize, sample_count: usize) -> CalibrationMetrics {
    CalibrationMetrics {
        physics_hz: 60,
        recording_hz: 60,
        fixed_steps: 2,
        simulated_duration: Duration::from_nanos(33_333_334),
        wall_clock_duration: Duration::ZERO,
        trajectory_sample_count: sample_count,
        raw_trajectory_payload_bytes: sample_count * std::mem::size_of::<TrajectorySample>(),
        trajectory_capacity_bytes: sample_count * std::mem::size_of::<TrajectorySample>(),
        record_container_bytes: std::mem::size_of::<RecordedBatch>(),
        world_construction_duration: Duration::ZERO,
        world_entity_count: u32::try_from(count + 5).expect("test fixture count"),
        app_stack_bytes: 0,
    }
}

fn recorded_die(ordinal: usize, kind: DieKind) -> RecordedDie {
    let natural_face = 1 + u8::try_from(ordinal).expect("test fixture ordinal") % kind.face_count();
    let orientation = match kind {
        DieKind::D6 => d6_geometry()
            .face(natural_face)
            .expect("legal d6 fixture face")
            .target_rotation(0.17),
        DieKind::D20 => d20_geometry()
            .face(natural_face)
            .expect("legal d20 fixture face")
            .target_rotation(0.17),
    };
    let x = ordinal as f32 * 1.5;
    RecordedDie {
        ordinal: u16::try_from(ordinal).expect("test fixture ordinal"),
        kind,
        initial: InitialPhysicalState {
            world_position: [x, 1.5, 0.0],
            unit_orientation: orientation.to_array(),
            linear_velocity: [0.0, -1.0, 0.0],
            angular_velocity: [0.0, 1.0, 0.0],
        },
        samples: vec![
            TrajectorySample {
                fixed_step: 1,
                world_position: [x, 1.5, 0.0],
                unit_orientation: orientation.to_array(),
            },
            TrajectorySample {
                fixed_step: 2,
                world_position: [x, 0.5, 0.0],
                unit_orientation: orientation.to_array(),
            },
        ],
        natural_terminal_face: natural_face,
        terminal: NaturalTerminalDiagnostics {
            upward_score: 1.0,
            runner_up_score: 0.5,
            support_boundary_margin_radians: 0.25,
            linear_speed: 0.0,
            angular_speed: 0.0,
            stable_steps: 36,
            support: SupportClassification::Tray,
            contacts: ContactDiagnostics {
                tray_contact_events: 1,
                dice_contact_events: 1,
                max_simultaneous_contacts: 2,
                first_contact_step: Some(2),
                last_contact_step: Some(2),
            },
        },
    }
}
