use std::{error::Error, fmt};

use bevy::prelude::Quat;

use crate::dice::{
    D6SolidSymmetry, d6_geometry, d6_solid_symmetries, d20_geometry, d20_solid_symmetries,
    d20_symmetry_mapping,
};

use super::types::{AttemptOutcome, DieKind, RecordedBatch};

const D6_STABILIZER_ORDER: usize = 4;
const D20_STABILIZER_ORDER: u64 = 3;

/// Target-independent choice among the four rotations for one d6 face pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct D6PresentationPhase {
    pub index: u8,
    pub identifier: u64,
}

impl D6PresentationPhase {
    /// Derives the free stabilizer phase only from accepted physics and identity.
    #[must_use]
    pub fn from_physical_seed(accepted_physical_seed: u64, ordinal: u16) -> Self {
        let input = accepted_physical_seed ^ u64::from(ordinal).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let identifier = mix64(input);
        Self {
            index: (identifier % D6_STABILIZER_ORDER as u64) as u8,
            identifier,
        }
    }
}

/// One post-physics d6 semantic-to-numbered-visual mapping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct D6PresentationMapping {
    pub ordinal: u16,
    pub natural_face: u8,
    pub requested_face: u8,
    pub accepted_physical_seed: u64,
    pub phase: D6PresentationPhase,
    pub symmetry_id: u8,
    pub symmetry: Quat,
}

/// Target-independent choice among the three rotations for one d20 face pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct D20PresentationPhase {
    pub index: u8,
    pub identifier: u64,
}

impl D20PresentationPhase {
    /// Derives the free stabilizer phase only from accepted physics and identity.
    #[must_use]
    pub fn from_physical_seed(accepted_physical_seed: u64, ordinal: u16) -> Self {
        let input = accepted_physical_seed ^ u64::from(ordinal).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let identifier = mix64(input);
        Self {
            index: (identifier % D20_STABILIZER_ORDER) as u8,
            identifier,
        }
    }
}

/// One post-physics d20 semantic-to-numbered-visual mapping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct D20PresentationMapping {
    pub ordinal: u16,
    pub natural_face: u8,
    pub requested_face: u8,
    pub accepted_physical_seed: u64,
    pub phase: D20PresentationPhase,
    pub symmetry_id: u8,
    pub symmetry: Quat,
}

/// Semantic presentation data kept structurally separate from physical input.
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticPresentationMap {
    pub natural_record_identity: u64,
    pub d6: Vec<D6PresentationMapping>,
    pub d20: Vec<D20PresentationMapping>,
}

impl SemanticPresentationMap {
    /// Maps a heterogeneous tuple over one already-complete immutable batch.
    pub fn for_mixed_tuple(
        record: &RecordedBatch,
        requested_faces: &[u8],
    ) -> Result<Self, PresentationMapError> {
        if record.dice.len() != requested_faces.len() {
            return Err(PresentationMapError::MismatchedDieCount);
        }
        let accepted_physical_seed = accepted_physical_seed(record)?;
        let mut d6 = Vec::new();
        let mut d20 = Vec::new();
        for (die, requested_face) in record.dice.iter().zip(requested_faces) {
            match die.kind {
                DieKind::D6 => d6.push(map_d6_presentation(
                    die.natural_terminal_face,
                    *requested_face,
                    die.ordinal,
                    accepted_physical_seed,
                )?),
                DieKind::D20 => d20.push(map_d20_presentation(
                    die.natural_terminal_face,
                    *requested_face,
                    die.ordinal,
                    accepted_physical_seed,
                )?),
            }
        }
        if d6.is_empty() || d20.is_empty() {
            return Err(PresentationMapError::ExpectedMixedDice);
        }
        Ok(Self {
            natural_record_identity: natural_record_identity(record),
            d6,
            d20,
        })
    }

    /// Maps a complete d6 tuple over one already-complete immutable batch.
    pub fn for_d6_tuple(
        record: &RecordedBatch,
        requested_faces: &[u8],
    ) -> Result<Self, PresentationMapError> {
        if record.dice.len() != requested_faces.len() {
            return Err(PresentationMapError::MismatchedDieCount);
        }
        if record.dice.iter().any(|die| die.kind != DieKind::D6) {
            return Err(PresentationMapError::ExpectedD6);
        }
        let accepted_physical_seed = accepted_physical_seed(record)?;
        let d6 = record
            .dice
            .iter()
            .zip(requested_faces)
            .map(|(die, requested_face)| {
                map_d6_presentation(
                    die.natural_terminal_face,
                    *requested_face,
                    die.ordinal,
                    accepted_physical_seed,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            natural_record_identity: natural_record_identity(record),
            d6,
            d20: Vec::new(),
        })
    }

    /// Maps one die from an already-complete immutable natural record.
    pub fn for_single_d6(
        record: &RecordedBatch,
        requested_face: u8,
    ) -> Result<Self, PresentationMapError> {
        if record.dice.len() != 1 {
            return Err(PresentationMapError::ExpectedSingleDie);
        }
        let die = &record.dice[0];
        if die.kind != DieKind::D6 {
            return Err(PresentationMapError::ExpectedD6);
        }
        let accepted_physical_seed = accepted_physical_seed(record)?;
        let mapping = map_d6_presentation(
            die.natural_terminal_face,
            requested_face,
            die.ordinal,
            accepted_physical_seed,
        )?;
        Ok(Self {
            natural_record_identity: natural_record_identity(record),
            d6: vec![mapping],
            d20: Vec::new(),
        })
    }

    /// Maps one d20 from an already-complete immutable natural record.
    pub fn for_single_d20(
        record: &RecordedBatch,
        requested_face: u8,
    ) -> Result<Self, PresentationMapError> {
        if record.dice.len() != 1 {
            return Err(PresentationMapError::ExpectedSingleDie);
        }
        let die = &record.dice[0];
        if die.kind != DieKind::D20 {
            return Err(PresentationMapError::ExpectedD20);
        }
        let accepted_physical_seed = accepted_physical_seed(record)?;
        let mapping = map_d20_presentation(
            die.natural_terminal_face,
            requested_face,
            die.ordinal,
            accepted_physical_seed,
        )?;
        Ok(Self {
            natural_record_identity: natural_record_identity(record),
            d6: Vec::new(),
            d20: vec![mapping],
        })
    }
}

/// Pure d6 mapping from finalized natural physics to semantic presentation.
pub fn map_d6_presentation(
    natural_face: u8,
    requested_face: u8,
    ordinal: u16,
    accepted_physical_seed: u64,
) -> Result<D6PresentationMapping, PresentationMapError> {
    let phase = D6PresentationPhase::from_physical_seed(accepted_physical_seed, ordinal);
    let candidates = d6_pair_symmetries(requested_face, natural_face)?;
    let symmetry = candidates[usize::from(phase.index)];
    Ok(D6PresentationMapping {
        ordinal,
        natural_face,
        requested_face,
        accepted_physical_seed,
        phase,
        symmetry_id: symmetry.id,
        symmetry: symmetry.rotation,
    })
}

/// Pure d20 mapping from finalized natural physics to semantic presentation.
pub fn map_d20_presentation(
    natural_face: u8,
    requested_face: u8,
    ordinal: u16,
    accepted_physical_seed: u64,
) -> Result<D20PresentationMapping, PresentationMapError> {
    if d20_geometry().face(requested_face).is_none() {
        return Err(PresentationMapError::InvalidRequestedFace(requested_face));
    }
    if d20_geometry().face(natural_face).is_none() {
        return Err(PresentationMapError::InvalidNaturalFace(natural_face));
    }
    let phase = D20PresentationPhase::from_physical_seed(accepted_physical_seed, ordinal);
    let symmetry = d20_symmetry_mapping(requested_face, natural_face, phase.index)
        .ok_or(PresentationMapError::IncompleteProperGroup)?;
    if d20_solid_symmetries()
        .into_iter()
        .filter(|candidate| {
            let requested = d20_geometry()
                .face(requested_face)
                .expect("validated requested d20 face");
            candidate.map_face(requested).value == natural_face
        })
        .count()
        != D20_STABILIZER_ORDER as usize
    {
        return Err(PresentationMapError::IncompleteProperGroup);
    }
    Ok(D20PresentationMapping {
        ordinal,
        natural_face,
        requested_face,
        accepted_physical_seed,
        phase,
        symmetry_id: symmetry.id,
        symmetry: symmetry.rotation,
    })
}

fn d6_pair_symmetries(
    requested_face: u8,
    natural_face: u8,
) -> Result<Vec<D6SolidSymmetry>, PresentationMapError> {
    let geometry = d6_geometry();
    let requested = geometry
        .face(requested_face)
        .ok_or(PresentationMapError::InvalidRequestedFace(requested_face))?;
    let natural = geometry
        .face(natural_face)
        .ok_or(PresentationMapError::InvalidNaturalFace(natural_face))?;
    let candidates = d6_solid_symmetries()
        .into_iter()
        .filter(|symmetry| symmetry.map_face(requested).value == natural.value)
        .collect::<Vec<_>>();
    if candidates.len() != D6_STABILIZER_ORDER {
        return Err(PresentationMapError::IncompleteProperGroup);
    }
    Ok(candidates)
}

fn accepted_physical_seed(record: &RecordedBatch) -> Result<u64, PresentationMapError> {
    record
        .attempts
        .iter()
        .rev()
        .find(|attempt| attempt.outcome == AttemptOutcome::Valid)
        .map(|attempt| attempt.physical_seed)
        .ok_or(PresentationMapError::MissingAcceptedAttempt)
}

/// Content identity for diagnostics proving reuse of one immutable record.
#[must_use]
pub fn natural_record_identity(record: &RecordedBatch) -> u64 {
    let mut hash = Fnv64::new();
    hash.duration(record.fixed_step);
    hash.f32(record.tray.width);
    hash.f32(record.tray.depth);
    hash.f32(record.tray.wall_height);
    for attempt in &record.attempts {
        hash.u8(attempt.attempt);
        hash.u64(attempt.physical_seed);
        hash.u64(attempt.die_count as u64);
        hash.u32(attempt.fixed_steps);
        hash.duration(attempt.simulated_duration);
        hash.bytes(format!("{:?}", attempt.outcome).as_bytes());
    }
    for die in &record.dice {
        hash_recorded_die(&mut hash, die);
    }
    hash_batch_contacts(&mut hash, record);
    hash_calibration(&mut hash, record);
    hash.finish()
}

fn hash_recorded_die(hash: &mut Fnv64, die: &super::types::RecordedDie) {
    hash.u16(die.ordinal);
    hash.u8(match die.kind {
        DieKind::D6 => 6,
        DieKind::D20 => 20,
    });
    for values in [
        die.initial.world_position.as_slice(),
        die.initial.unit_orientation.as_slice(),
        die.initial.linear_velocity.as_slice(),
        die.initial.angular_velocity.as_slice(),
    ] {
        for value in values {
            hash.f32(*value);
        }
    }
    hash.u8(die.natural_terminal_face);
    hash.bytes(format!("{:?}", die.terminal).as_bytes());
    for sample in &die.samples {
        hash.u32(sample.fixed_step);
        for value in sample
            .world_position
            .into_iter()
            .chain(sample.unit_orientation)
        {
            hash.f32(value);
        }
    }
}

fn hash_batch_contacts(hash: &mut Fnv64, record: &RecordedBatch) {
    for sample in &record.contacts.dice_contact_samples {
        hash.u32(sample.fixed_step);
        hash.u16(sample.first_ordinal);
        hash.u16(sample.second_ordinal);
        for value in sample.world_point {
            hash.f32(value);
        }
        for value in sample.world_normal {
            hash.f32(value);
        }
        hash.f32(sample.normal_impulse);
        hash.f32(sample.approach_speed);
    }
    hash.u32(record.contacts.dice_contact_interactions);
    hash.u16(record.contacts.max_simultaneous_dice_pairs);
}

fn hash_calibration(hash: &mut Fnv64, record: &RecordedBatch) {
    hash.u32(record.calibration.physics_hz);
    hash.u32(record.calibration.recording_hz);
    hash.u32(record.calibration.fixed_steps);
    hash.duration(record.calibration.simulated_duration);
    hash.u64(record.calibration.trajectory_sample_count as u64);
    hash.u64(record.calibration.raw_trajectory_payload_bytes as u64);
}

struct Fnv64(u64);

impl Fnv64 {
    const fn new() -> Self {
        Self(0xCBF2_9CE4_8422_2325)
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }

    fn u8(&mut self, value: u8) {
        self.bytes(&[value]);
    }

    fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn f32(&mut self, value: f32) {
        self.u32(value.to_bits());
    }

    fn duration(&mut self, value: std::time::Duration) {
        self.u64(value.as_secs());
        self.u32(value.subsec_nanos());
    }

    const fn finish(self) -> u64 {
        self.0
    }
}

fn mix64(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentationMapError {
    ExpectedSingleDie,
    MismatchedDieCount,
    ExpectedD6,
    ExpectedMixedDice,
    ExpectedD20,
    MissingAcceptedAttempt,
    InvalidRequestedFace(u8),
    InvalidNaturalFace(u8),
    IncompleteProperGroup,
}

impl fmt::Display for PresentationMapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid semantic presentation mapping: {self:?}")
    }
}

impl Error for PresentationMapError {}
