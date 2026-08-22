//! Project-owned die geometry and face metadata.

mod d20;
mod d20_symmetry;
mod geometry;
mod labels;
mod symmetry;

pub use d20::{D20_FACE_TO_FACE, D20Face, D20Geometry, d20_geometry, d20_mesh};
pub use d20_symmetry::{
    D20SolidSymmetry, d20_solid_symmetries, d20_symmetry_mapping, d20_symmetry_mappings,
};
pub use geometry::{D6_MIN_FACE_BOUNDARY_ANGLE, D6Face, D6Geometry, d6_geometry, d6_mesh};
pub use labels::{D20Label, FaceLabel, d6_labels, d20_labels};
pub use symmetry::{D6SolidSymmetry, d6_solid_symmetries, d6_symmetry_mapping};
