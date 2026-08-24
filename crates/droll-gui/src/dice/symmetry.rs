use bevy::prelude::{Mat3, Quat, Vec3};

use super::{D6Face, d6_geometry};

const AXES: [Vec3; 6] = [
    Vec3::X,
    Vec3::NEG_X,
    Vec3::Y,
    Vec3::NEG_Y,
    Vec3::Z,
    Vec3::NEG_Z,
];

/// One orientation-preserving rotation of the project-owned d6 solid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct D6SolidSymmetry {
    pub id: u8,
    pub rotation: Quat,
}

impl D6SolidSymmetry {
    /// Returns the face reached by rotating `face` in body-local space.
    #[must_use]
    pub fn map_face(self, face: D6Face) -> D6Face {
        let mapped_normal = self.rotation * face.normal;
        d6_geometry()
            .faces
            .into_iter()
            .find(|candidate| candidate.normal.dot(mapped_normal) > 1.0 - 1.0e-5)
            .expect("a cube symmetry maps every face normal")
    }
}

/// Enumerates the 24 proper rotations of the cube.
#[must_use]
pub fn d6_solid_symmetries() -> Vec<D6SolidSymmetry> {
    let mut rotations = Vec::with_capacity(24);
    for x_axis in AXES {
        for y_axis in AXES {
            if x_axis.dot(y_axis).abs() > 1.0e-6 {
                continue;
            }
            let z_axis = x_axis.cross(y_axis);
            let rotation = Quat::from_mat3(&Mat3::from_cols(x_axis, y_axis, z_axis)).normalize();
            rotations.push(D6SolidSymmetry {
                id: u8::try_from(rotations.len()).expect("the cube group has 24 elements"),
                rotation,
            });
        }
    }
    rotations
}

/// Selects a proper body-local symmetry mapping `target_face` to `base_face`.
#[must_use]
pub fn d6_symmetry_mapping(target_face: u8, base_face: u8) -> Option<D6SolidSymmetry> {
    let geometry = d6_geometry();
    let target = geometry.face(target_face)?;
    let base = geometry.face(base_face)?;
    d6_solid_symmetries()
        .into_iter()
        .find(|symmetry| symmetry.map_face(target).value == base.value)
}
