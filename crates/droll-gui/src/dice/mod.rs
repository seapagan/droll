//! Project-owned die geometry and face metadata.

mod geometry;
mod labels;

pub use geometry::{D6_MIN_FACE_BOUNDARY_ANGLE, D6Face, D6Geometry, d6_geometry, d6_mesh};
pub use labels::{FaceLabel, d6_labels};
