use bevy::prelude::Vec3;

use super::geometry::d6_geometry;
use super::{D20Face, d20_geometry};

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

const D20_LABEL_OFFSET: f32 = 0.006;
const DIGIT_WIDTH: f32 = 0.075;
const DIGIT_HEIGHT: f32 = 0.13;
const STROKE_WIDTH: f32 = 0.014;
const DIGIT_GAP: f32 = 0.018;

/// One generated numeric d20 label made only from triangles.
#[derive(Clone, Debug, PartialEq)]
pub struct D20Label {
    pub value: u8,
    pub face_value: u8,
    pub normal: Vec3,
    pub vertices: Vec<Vec3>,
    pub indices: Vec<u32>,
    pub has_orientation_mark: bool,
}

/// Generates project-owned numeric labels from one shared digit description.
#[must_use]
pub fn d20_labels() -> Vec<D20Label> {
    d20_geometry()
        .faces
        .iter()
        .copied()
        .map(d20_label)
        .collect()
}

fn d20_label(face: D20Face) -> D20Label {
    let digits = if face.value >= 10 {
        vec![face.value / 10, face.value % 10]
    } else {
        vec![face.value]
    };
    let total_width =
        digits.len() as f32 * DIGIT_WIDTH + (digits.len().saturating_sub(1)) as f32 * DIGIT_GAP;
    let mut label = D20Label {
        value: face.value,
        face_value: face.value,
        normal: face.normal,
        vertices: Vec::new(),
        indices: Vec::new(),
        has_orientation_mark: matches!(face.value, 6 | 9),
    };
    for (index, digit) in digits.into_iter().enumerate() {
        let x = -total_width / 2.0 + DIGIT_WIDTH / 2.0 + index as f32 * (DIGIT_WIDTH + DIGIT_GAP);
        append_digit(&mut label, face, digit, x);
    }
    if label.has_orientation_mark {
        append_rect(
            &mut label,
            face,
            0.0,
            -DIGIT_HEIGHT * 0.68,
            DIGIT_WIDTH * 0.6,
            STROKE_WIDTH,
        );
    }
    label
}

fn append_digit(label: &mut D20Label, face: D20Face, digit: u8, x: f32) {
    const SEGMENTS: [[bool; 7]; 10] = [
        [true, true, true, false, true, true, true],
        [false, false, true, false, false, true, false],
        [true, false, true, true, true, false, true],
        [true, false, true, true, false, true, true],
        [false, true, true, true, false, true, false],
        [true, true, false, true, false, true, true],
        [true, true, false, true, true, true, true],
        [true, false, true, false, false, true, false],
        [true, true, true, true, true, true, true],
        [true, true, true, true, false, true, true],
    ];
    let h = DIGIT_HEIGHT / 2.0;
    let w = DIGIT_WIDTH / 2.0;
    let definitions = [
        (x, h, DIGIT_WIDTH, STROKE_WIDTH),
        (x - w, h / 2.0, STROKE_WIDTH, DIGIT_HEIGHT / 2.0),
        (x + w, h / 2.0, STROKE_WIDTH, DIGIT_HEIGHT / 2.0),
        (x, 0.0, DIGIT_WIDTH, STROKE_WIDTH),
        (x - w, -h / 2.0, STROKE_WIDTH, DIGIT_HEIGHT / 2.0),
        (x + w, -h / 2.0, STROKE_WIDTH, DIGIT_HEIGHT / 2.0),
        (x, -h, DIGIT_WIDTH, STROKE_WIDTH),
    ];
    for (enabled, (cx, cy, width, height)) in
        SEGMENTS[usize::from(digit)].into_iter().zip(definitions)
    {
        if enabled {
            append_rect(label, face, cx, cy, width, height);
        }
    }
}

fn append_rect(label: &mut D20Label, face: D20Face, x: f32, y: f32, width: f32, height: f32) {
    let base = u32::try_from(label.vertices.len()).expect("d20 label mesh is small");
    let origin = face.center + face.normal * D20_LABEL_OFFSET;
    for [dx, dy] in [
        [-width / 2.0, -height / 2.0],
        [width / 2.0, -height / 2.0],
        [width / 2.0, height / 2.0],
        [-width / 2.0, height / 2.0],
    ] {
        label
            .vertices
            .push(origin + face.label_right * (x + dx) + face.label_up * (y + dy));
    }
    label
        .indices
        .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
}
