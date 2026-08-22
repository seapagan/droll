use bevy::prelude::{Quat, Vec3};

use crate::dice::{D6_MIN_FACE_BOUNDARY_ANGLE, d6_geometry, d20_geometry};

use super::types::{DieKind, InvalidityReason, PhysicalTray, SupportClassification};

/// Frozen Phase 0 physical-validity thresholds, registered before calibration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicalValidityPolicy {
    pub watchdog_steps: u32,
    pub stable_steps: u16,
    pub rest_linear_speed: f32,
    pub rest_angular_speed: f32,
    pub face_boundary_guard_radians: f32,
    pub stack_min_horizontal_exposure: f32,
    pub stack_min_vertical_ordering: f32,
}

impl Default for PhysicalValidityPolicy {
    fn default() -> Self {
        Self {
            // Existing target-independent H1 observation used 18 s and 0.60 s.
            watchdog_steps: 18 * 60,
            stable_steps: 36,
            // No looser than Avian's pinned 0.15/0.15 sleeping thresholds.
            rest_linear_speed: 0.10,
            rest_angular_speed: 0.15,
            // Preserve five degrees inside the die-specific geometric boundary.
            face_boundary_guard_radians: 5.0_f32.to_radians(),
            // Require 30% of the project one-unit face-to-face scale to remain
            // exposed in top projection; this is a preregistered Phase 0 rule.
            stack_min_horizontal_exposure: 0.30,
            // Require 20% of the same unit scale for lower-to-upper ordering.
            stack_min_vertical_ordering: 0.20,
        }
    }
}

impl PhysicalValidityPolicy {
    #[must_use]
    pub fn with_watchdog_steps(mut self, watchdog_steps: u32) -> Self {
        self.watchdog_steps = watchdog_steps;
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct FaceObservation {
    pub value: u8,
    pub upward_score: f32,
    pub runner_up_score: f32,
    pub support_boundary_margin_radians: f32,
    pub unambiguous: bool,
}

pub(super) fn observe_face(
    kind: DieKind,
    rotation: Quat,
    policy: PhysicalValidityPolicy,
) -> FaceObservation {
    let mut scores = match kind {
        DieKind::D6 => d6_geometry()
            .faces
            .into_iter()
            .map(|face| (face.value, (rotation * face.normal).dot(Vec3::Y)))
            .collect::<Vec<_>>(),
        DieKind::D20 => d20_geometry()
            .faces
            .into_iter()
            .map(|face| (face.value, (rotation * face.normal).dot(Vec3::Y)))
            .collect::<Vec<_>>(),
    };
    scores.sort_by(|left, right| right.1.total_cmp(&left.1));
    let boundary = minimum_face_boundary_angle(kind);
    let tilt = scores[0].1.clamp(-1.0, 1.0).acos();
    let margin = boundary - tilt;
    FaceObservation {
        value: scores[0].0,
        upward_score: scores[0].1,
        runner_up_score: scores[1].1,
        support_boundary_margin_radians: margin,
        unambiguous: scores[0].1 > scores[1].1 && margin >= policy.face_boundary_guard_radians,
    }
}

fn minimum_face_boundary_angle(kind: DieKind) -> f32 {
    match kind {
        DieKind::D6 => D6_MIN_FACE_BOUNDARY_ANGLE,
        DieKind::D20 => {
            let geometry = d20_geometry();
            geometry
                .faces
                .iter()
                .flat_map(|face| {
                    face.adjacent_values.map(|value| {
                        let adjacent = geometry.face(value).expect("adjacent d20 face");
                        0.5 * face.normal.dot(adjacent.normal).clamp(-1.0, 1.0).acos()
                    })
                })
                .min_by(f32::total_cmp)
                .expect("d20 has adjacent faces")
        }
    }
}

pub(super) fn is_contained(kind: DieKind, position: Vec3, tray: PhysicalTray) -> bool {
    let radius = kind.circumradius();
    position.y >= -radius
        && position.y <= tray.wall_height + tray.width
        && position.x.abs() + radius <= tray.width * 0.5
        && position.z.abs() + radius <= tray.depth * 0.5
}

pub(super) fn classify_support(
    ordinal: u16,
    position: Vec3,
    touches_floor: bool,
    supporting_dice: &[(u16, Vec3)],
    policy: PhysicalValidityPolicy,
) -> Result<SupportClassification, InvalidityReason> {
    if touches_floor {
        return Ok(SupportClassification::Tray);
    }
    if supporting_dice.is_empty() {
        return Err(InvalidityReason::UnsupportedDie { ordinal });
    }
    let mut supporting_ordinals = Vec::with_capacity(supporting_dice.len());
    for (supporting_ordinal, supporting_position) in supporting_dice {
        let vertical = position.y - supporting_position.y;
        let horizontal = Vec3::new(
            position.x - supporting_position.x,
            0.0,
            position.z - supporting_position.z,
        )
        .length();
        if vertical < policy.stack_min_vertical_ordering
            || horizontal < policy.stack_min_horizontal_exposure
        {
            return Err(InvalidityReason::UnreadableOrPathologicalStack { ordinal });
        }
        supporting_ordinals.push(*supporting_ordinal);
    }
    supporting_ordinals.sort_unstable();
    Ok(SupportClassification::ReadableStack {
        supporting_ordinals,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offset_stable_stack_is_readable() {
        let classification = classify_support(
            1,
            Vec3::new(0.40, 1.0, 0.0),
            false,
            &[(0, Vec3::new(0.0, 0.2, 0.0))],
            PhysicalValidityPolicy::default(),
        );
        assert_eq!(
            classification,
            Ok(SupportClassification::ReadableStack {
                supporting_ordinals: vec![0]
            })
        );
    }

    #[test]
    fn test_occluded_or_unordered_stack_is_invalid() {
        let policy = PhysicalValidityPolicy::default();
        for supporting_position in [Vec3::new(0.1, 0.2, 0.0), Vec3::new(0.5, 0.9, 0.0)] {
            assert_eq!(
                classify_support(
                    1,
                    Vec3::new(0.0, 1.0, 0.0),
                    false,
                    &[(0, supporting_position)],
                    policy
                ),
                Err(InvalidityReason::UnreadableOrPathologicalStack { ordinal: 1 })
            );
        }
    }
}
