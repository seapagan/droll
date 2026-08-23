use std::{mem::size_of, time::Duration};

use bevy::prelude::{Quat, Vec3};

use super::validity::PhysicalValidityPolicy;

pub const PHYSICS_HZ: u32 = 60;
pub const RECORDING_HZ: u32 = 60;
pub const MAX_TOTAL_ATTEMPTS: u8 = 3;
pub const FIXED_STEP: Duration = Duration::from_nanos(16_666_667);

/// Physical shape requested from the hidden runner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DieKind {
    D6,
    D20,
}

impl DieKind {
    #[must_use]
    pub const fn circumradius(self) -> f32 {
        match self {
            Self::D6 => 0.866_025_4,
            Self::D20 => 0.629_204_3,
        }
    }
}

/// Target-independent physical tray dimensions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicalTray {
    pub width: f32,
    pub depth: f32,
    pub wall_height: f32,
}

impl Default for PhysicalTray {
    fn default() -> Self {
        Self {
            width: 8.0,
            depth: 6.0,
            wall_height: 0.8,
        }
    }
}

/// The complete input boundary available to launch generation and physics.
#[derive(Clone, Debug, PartialEq)]
pub struct PhysicalBatchRequest {
    dice: Vec<DieKind>,
    physical_presentation_seed: u64,
    tray: PhysicalTray,
    validity: PhysicalValidityPolicy,
}

impl PhysicalBatchRequest {
    #[must_use]
    pub fn new(dice: Vec<DieKind>, physical_presentation_seed: u64) -> Self {
        Self {
            dice,
            physical_presentation_seed,
            tray: PhysicalTray::default(),
            validity: PhysicalValidityPolicy::default(),
        }
    }

    #[must_use]
    pub fn with_validity_policy(mut self, validity: PhysicalValidityPolicy) -> Self {
        self.validity = validity;
        self
    }

    #[must_use]
    pub fn dice(&self) -> &[DieKind] {
        &self.dice
    }

    #[must_use]
    pub const fn physical_presentation_seed(&self) -> u64 {
        self.physical_presentation_seed
    }

    #[must_use]
    pub const fn tray(&self) -> PhysicalTray {
        self.tray
    }

    #[must_use]
    pub const fn validity(&self) -> PhysicalValidityPolicy {
        self.validity
    }
}

/// Dense in-memory 60 Hz transform sample. Its Phase 0 raw payload is 32 bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrajectorySample {
    pub fixed_step: u32,
    pub world_position: [f32; 3],
    pub unit_orientation: [f32; 4],
}

impl TrajectorySample {
    pub(super) fn new(fixed_step: u32, position: Vec3, orientation: Quat) -> Self {
        Self {
            fixed_step,
            world_position: position.to_array(),
            unit_orientation: orientation.to_array(),
        }
    }
}

/// Objective terminal support classification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SupportClassification {
    Tray,
    ReadableStack { supporting_ordinals: Vec<u16> },
}

/// Bounded aggregate contact evidence retained without a production event schema.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ContactDiagnostics {
    pub tray_contact_events: u32,
    pub dice_contact_events: u32,
    pub max_simultaneous_contacts: u16,
    pub first_contact_step: Option<u32>,
    pub last_contact_step: Option<u32>,
}

/// One genuine dice-dice contact-bearing fixed step from the shared world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiceContactSample {
    pub fixed_step: u32,
    pub first_ordinal: u16,
    pub second_ordinal: u16,
    pub world_point: [f32; 3],
    pub world_normal: [f32; 3],
    pub normal_impulse: f32,
    pub approach_speed: f32,
}

/// Bounded shared-interaction evidence for one accepted physical batch.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BatchContactDiagnostics {
    pub dice_contact_samples: Vec<DiceContactSample>,
    pub dice_contact_interactions: u32,
    pub strongest_dice_contact: Option<DiceContactSample>,
}

/// Target-independent launch state retained for identity and overlap proofs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InitialPhysicalState {
    pub world_position: [f32; 3],
    pub unit_orientation: [f32; 4],
    pub linear_velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
}

/// Target-independent terminal measurements for one die.
#[derive(Clone, Debug, PartialEq)]
pub struct NaturalTerminalDiagnostics {
    pub upward_score: f32,
    pub runner_up_score: f32,
    pub support_boundary_margin_radians: f32,
    pub linear_speed: f32,
    pub angular_speed: f32,
    pub stable_steps: u16,
    pub support: SupportClassification,
    pub contacts: ContactDiagnostics,
}

/// One naturally settled die and its immutable in-memory samples.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordedDie {
    pub ordinal: u16,
    pub kind: DieKind,
    pub initial: InitialPhysicalState,
    pub samples: Vec<TrajectorySample>,
    pub natural_terminal_face: u8,
    pub terminal: NaturalTerminalDiagnostics,
}

/// Physical invalidity is deliberately independent of semantic outcomes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidityReason {
    EmptyBatch,
    TooManyDice,
    LeftTray { ordinal: u16 },
    NonfinitePhysicsState { ordinal: u16 },
    AmbiguousUpwardFace { ordinal: u16 },
    UnsupportedDie { ordinal: u16 },
    UnreadableOrPathologicalStack { ordinal: u16 },
    WatchdogExpired,
    RecordOverflow,
    FixedStepDidNotAdvanceExactlyOnce,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttemptOutcome {
    Valid,
    Invalid(InvalidityReason),
}

/// Diagnostic retained for every whole-batch attempt.
#[derive(Clone, Debug, PartialEq)]
pub struct AttemptDiagnostic {
    pub attempt: u8,
    pub physical_seed: u64,
    pub die_count: usize,
    pub fixed_steps: u32,
    pub simulated_duration: Duration,
    pub wall_clock_duration: Duration,
    pub outcome: AttemptOutcome,
}

/// Bounded Phase 0 calibration signals, not product benchmark evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct CalibrationMetrics {
    pub physics_hz: u32,
    pub recording_hz: u32,
    pub fixed_steps: u32,
    pub simulated_duration: Duration,
    pub wall_clock_duration: Duration,
    pub trajectory_sample_count: usize,
    pub raw_trajectory_payload_bytes: usize,
    pub trajectory_capacity_bytes: usize,
    pub record_container_bytes: usize,
    pub world_construction_duration: Duration,
    pub world_entity_count: u32,
    pub app_stack_bytes: usize,
}

impl CalibrationMetrics {
    pub(super) fn from_record(
        dice: &[RecordedDie],
        dice_capacity: usize,
        fixed_steps: u32,
        wall_clock_duration: Duration,
        world_construction_duration: Duration,
        world_entity_count: u32,
    ) -> Self {
        let sample_count = dice.iter().map(|die| die.samples.len()).sum::<usize>();
        let capacity = dice.iter().map(|die| die.samples.capacity()).sum::<usize>();
        Self {
            physics_hz: PHYSICS_HZ,
            recording_hz: RECORDING_HZ,
            fixed_steps,
            simulated_duration: FIXED_STEP * fixed_steps,
            wall_clock_duration,
            trajectory_sample_count: sample_count,
            raw_trajectory_payload_bytes: sample_count * size_of::<TrajectorySample>(),
            trajectory_capacity_bytes: capacity * size_of::<TrajectorySample>(),
            record_container_bytes: size_of::<RecordedBatch>()
                + dice_capacity * size_of::<RecordedDie>()
                + capacity * size_of::<TrajectorySample>(),
            world_construction_duration,
            world_entity_count,
            app_stack_bytes: size_of::<bevy::app::App>(),
        }
    }
}

/// Accepted target-blind natural batch record.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordedBatch {
    pub fixed_step: Duration,
    pub attempts: Vec<AttemptDiagnostic>,
    pub dice: Vec<RecordedDie>,
    pub contacts: BatchContactDiagnostics,
    pub calibration: CalibrationMetrics,
}

/// Explicit failure after the bounded whole-batch attempt budget is exhausted.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparationFailure {
    pub attempts: Vec<AttemptDiagnostic>,
}
