//! Project-owned die geometry and face metadata.

mod geometry;
mod labels;
mod symmetry;

pub use geometry::{D6_MIN_FACE_BOUNDARY_ANGLE, D6Face, D6Geometry, d6_geometry, d6_mesh};
pub use labels::{FaceLabel, d6_labels};
pub use symmetry::{D6SolidSymmetry, d6_solid_symmetries, d6_symmetry_mapping};
