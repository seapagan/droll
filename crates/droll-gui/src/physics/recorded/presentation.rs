use std::{error::Error, fmt};

use bevy::prelude::Quat;

use crate::dice::{D6SolidSymmetry, d6_geometry, d6_solid_symmetries};

use super::types::{AttemptOutcome, DieKind, RecordedBatch};

const D6_STABILIZER_ORDER: usize = 4;

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

/// Semantic presentation data kept structurally separate from physical input.
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticPresentationMap {
    pub natural_record_identity: u64,
    pub d6: Vec<D6PresentationMapping>,
}

impl SemanticPresentationMap {
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
    for attempt in &record.attempts {
        hash.u8(attempt.attempt);
        hash.u64(attempt.physical_seed);
        hash.u64(attempt.die_count as u64);
        hash.u32(attempt.fixed_steps);
        hash.duration(attempt.simulated_duration);
        hash.bytes(format!("{:?}", attempt.outcome).as_bytes());
    }
    for die in &record.dice {
        hash.u16(die.ordinal);
        hash.u8(match die.kind {
            DieKind::D6 => 6,
            DieKind::D20 => 20,
        });
        hash.u8(die.natural_terminal_face);
        hash.bytes(format!("{:?}", die.terminal).as_bytes());
        for sample in &die.samples {
            hash.u32(sample.fixed_step);
            for value in sample.world_position {
                hash.u32(value.to_bits());
            }
            for value in sample.unit_orientation {
                hash.u32(value.to_bits());
            }
        }
    }
    hash.finish()
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
    ExpectedD6,
    MissingAcceptedAttempt,
    InvalidRequestedFace(u8),
    InvalidNaturalFace(u8),
    IncompleteProperGroup,
}

impl fmt::Display for PresentationMapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid d6 semantic presentation mapping: {self:?}"
        )
    }
}

impl Error for PresentationMapError {}
