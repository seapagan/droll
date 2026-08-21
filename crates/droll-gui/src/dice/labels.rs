use bevy::prelude::Vec3;

use super::geometry::d6_geometry;

const PIP_OFFSET: f32 = 0.012;
const PIP_SPACING: f32 = 0.22;

/// One project-owned d6 pip, positioned from face metadata.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceLabel {
    pub value: u8,
    pub center: Vec3,
    pub normal: Vec3,
    pub radius: f32,
}

/// Generates conventional d6 pip labels without a font or external asset.
#[must_use]
pub fn d6_labels() -> Vec<FaceLabel> {
    let geometry = d6_geometry();
    geometry
        .faces
        .iter()
        .flat_map(|face| {
            pip_pattern(face.value).into_iter().map(|[x, y]| FaceLabel {
                value: face.value,
                center: face.center
                    + face.normal * PIP_OFFSET
                    + face.label_right * (x * PIP_SPACING)
                    + face.label_up * (y * PIP_SPACING),
                normal: face.normal,
                radius: 0.055,
            })
        })
        .collect()
}

fn pip_pattern(value: u8) -> Vec<[f32; 2]> {
    let mut pips = Vec::with_capacity(usize::from(value));
    if value % 2 == 1 {
        pips.push([0.0, 0.0]);
    }
    if value >= 2 {
        pips.extend([[-1.0, 1.0], [1.0, -1.0]]);
    }
    if value >= 4 {
        pips.extend([[1.0, 1.0], [-1.0, -1.0]]);
    }
    if value == 6 {
        pips.extend([[-1.0, 0.0], [1.0, 0.0]]);
    }
    pips
}
