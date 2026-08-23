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
    let mut label = D20Label {
        value: face.value,
        face_value: face.value,
        normal: face.normal,
        vertices: Vec::new(),
        indices: Vec::new(),
        has_orientation_mark: matches!(face.value, 6 | 9),
    };
    if digits.len() == 1 {
        append_digit(&mut label, face, digits[0], 0.0);
    } else {
        append_digits(&mut label, face, &digits);
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

fn append_digits(label: &mut D20Label, face: D20Face, digits: &[u8]) {
    let bounds = digits
        .iter()
        .map(|digit| digit_visual_bounds(*digit))
        .collect::<Vec<_>>();
    let total_width = bounds.iter().map(|(left, right)| right - left).sum::<f32>()
        + (digits.len() - 1) as f32 * DIGIT_GAP;
    let mut cursor = -total_width / 2.0;
    for (digit, (left, right)) in digits.iter().zip(bounds) {
        append_digit(label, face, *digit, cursor - left);
        cursor += right - left + DIGIT_GAP;
    }
}

fn append_digit(label: &mut D20Label, face: D20Face, digit: u8, x: f32) {
    for (enabled, (cx, cy, width, height)) in SEGMENTS[usize::from(digit)]
        .into_iter()
        .zip(digit_segments())
    {
        if enabled {
            append_rect(label, face, x + cx, cy, width, height);
        }
    }
}

fn digit_visual_bounds(digit: u8) -> (f32, f32) {
    SEGMENTS[usize::from(digit)]
        .into_iter()
        .zip(digit_segments())
        .filter(|(enabled, _)| *enabled)
        .fold(
            (f32::INFINITY, f32::NEG_INFINITY),
            |bounds, (_, segment)| {
                let (x, _, width, _) = segment;
                (bounds.0.min(x - width / 2.0), bounds.1.max(x + width / 2.0))
            },
        )
}

fn digit_segments() -> [(f32, f32, f32, f32); 7] {
    let h = DIGIT_HEIGHT / 2.0;
    let w = DIGIT_WIDTH / 2.0;
    [
        (0.0, h, DIGIT_WIDTH, STROKE_WIDTH),
        (-w, h / 2.0, STROKE_WIDTH, DIGIT_HEIGHT / 2.0),
        (w, h / 2.0, STROKE_WIDTH, DIGIT_HEIGHT / 2.0),
        (0.0, 0.0, DIGIT_WIDTH, STROKE_WIDTH),
        (-w, -h / 2.0, STROKE_WIDTH, DIGIT_HEIGHT / 2.0),
        (w, -h / 2.0, STROKE_WIDTH, DIGIT_HEIGHT / 2.0),
        (0.0, -h, DIGIT_WIDTH, STROKE_WIDTH),
    ]
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
