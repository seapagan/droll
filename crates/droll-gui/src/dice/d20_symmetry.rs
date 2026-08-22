use bevy::prelude::{Mat3, Quat};

use super::{D20Face, d20_geometry};

const ROTATION_EPSILON: f32 = 2.0e-5;

/// One orientation-preserving rotation of the project-owned d20.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct D20SolidSymmetry {
    pub id: u8,
    pub rotation: Quat,
}

impl D20SolidSymmetry {
    /// Returns the face reached by rotating `face` in body-local space.
    #[must_use]
    pub fn map_face(self, face: D20Face) -> D20Face {
        let mapped_normal = self.rotation * face.normal;
        d20_geometry()
            .faces
            .into_iter()
            .find(|candidate| candidate.normal.dot(mapped_normal) > 1.0 - ROTATION_EPSILON)
            .expect("a d20 symmetry maps every face normal")
    }
}

/// Generates the 60 proper rotations from the canonical d20 face frames.
#[must_use]
pub fn d20_solid_symmetries() -> Vec<D20SolidSymmetry> {
    let geometry = d20_geometry();
    let canonical = geometry.faces[0];
    let canonical_frame = oriented_face_frame(canonical, 0);
    let mut rotations = Vec::with_capacity(60);
    for face in geometry.faces {
        for edge_phase in 0..3 {
            let destination = oriented_face_frame(face, edge_phase);
            let rotation =
                Quat::from_mat3(&(destination * canonical_frame.transpose())).normalize();
            if preserves_vertices(rotation)
                && rotations.iter().all(|candidate: &D20SolidSymmetry| {
                    !same_rotation(candidate.rotation, rotation)
                })
            {
                rotations.push(D20SolidSymmetry {
                    id: u8::try_from(rotations.len()).expect("d20 group has 60 elements"),
                    rotation,
                });
            }
        }
    }
    rotations
}

/// Returns all three solid rotations mapping one face to another.
#[must_use]
pub fn d20_symmetry_mappings(target_face: u8, base_face: u8) -> Vec<D20SolidSymmetry> {
    let geometry = d20_geometry();
    let Some(target) = geometry.face(target_face) else {
        return Vec::new();
    };
    let Some(base) = geometry.face(base_face) else {
        return Vec::new();
    };
    d20_solid_symmetries()
        .into_iter()
        .filter(|symmetry| symmetry.map_face(target).value == base.value)
        .collect()
}

/// Selects one target-to-base mapping by cyclic oriented-edge phase.
#[must_use]
pub fn d20_symmetry_mapping(target_face: u8, base_face: u8, phase: u8) -> Option<D20SolidSymmetry> {
    if phase > 2 {
        return None;
    }
    let geometry = d20_geometry();
    let target = geometry.face(target_face)?;
    let base = geometry.face(base_face)?;
    let target_frame = oriented_face_frame(target, 0);
    let base_frame = oriented_face_frame(base, 0);
    let phase_frame = oriented_face_frame(geometry.faces[0], usize::from(phase));
    let canonical_frame = oriented_face_frame(geometry.faces[0], 0);
    let target_transform = target_frame * canonical_frame.transpose();
    let base_transform = base_frame * canonical_frame.transpose();
    let phase_rotation = phase_frame * canonical_frame.transpose();
    let rotation =
        Quat::from_mat3(&(base_transform * phase_rotation * target_transform.transpose()))
            .normalize();
    d20_solid_symmetries()
        .into_iter()
        .find(|symmetry| same_rotation(symmetry.rotation, rotation))
}

fn oriented_face_frame(face: D20Face, edge_phase: usize) -> Mat3 {
    let geometry = d20_geometry();
    let a = geometry.vertices[face.vertices[edge_phase]];
    let b = geometry.vertices[face.vertices[(edge_phase + 1) % 3]];
    let right = (b - a).normalize();
    let up = face.normal.cross(right).normalize();
    Mat3::from_cols(right, up, face.normal)
}

fn preserves_vertices(rotation: Quat) -> bool {
    let vertices = d20_geometry().vertices;
    vertices.iter().all(|vertex| {
        vertices
            .iter()
            .any(|candidate| candidate.distance(rotation * *vertex) < ROTATION_EPSILON)
    })
}

fn same_rotation(left: Quat, right: Quat) -> bool {
    left.dot(right).abs() > 1.0 - ROTATION_EPSILON
}
