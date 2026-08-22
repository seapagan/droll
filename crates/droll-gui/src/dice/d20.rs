use std::collections::BTreeSet;

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, Mesh, PrimitiveTopology},
    prelude::{Quat, Vec3},
};

const PHI: f32 = 1.618_034;
const RAW_INRADIUS: f32 = PHI * PHI / 1.732_050_8;
const SCALE: f32 = 0.5 / RAW_INRADIUS;

/// Face-to-opposite-face size of the generated d20.
pub const D20_FACE_TO_FACE: f32 = 1.0;

/// One triangular face of the project-owned d20.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct D20Face {
    pub value: u8,
    pub vertices: [usize; 3],
    pub normal: Vec3,
    pub center: Vec3,
    pub label_right: Vec3,
    pub label_up: Vec3,
    pub adjacent_values: [u8; 3],
    pub opposite_value: u8,
}

impl D20Face {
    /// Builds a target rotation with the selected face upward and varied yaw.
    #[must_use]
    pub fn target_rotation(self, yaw_radians: f32) -> Quat {
        Quat::from_axis_angle(Vec3::Y, yaw_radians) * Quat::from_rotation_arc(self.normal, Vec3::Y)
    }
}

/// Canonical vertices and derived face metadata for a regular d20.
#[derive(Clone, Debug)]
pub struct D20Geometry {
    pub vertices: [Vec3; 12],
    pub faces: [D20Face; 20],
}

impl D20Geometry {
    #[must_use]
    pub fn face(&self, value: u8) -> Option<D20Face> {
        self.faces.iter().copied().find(|face| face.value == value)
    }

    #[must_use]
    pub fn upward_face(&self, rotation: Quat) -> D20Face {
        self.faces
            .iter()
            .copied()
            .max_by(|left, right| {
                (rotation * left.normal)
                    .dot(Vec3::Y)
                    .total_cmp(&(rotation * right.normal).dot(Vec3::Y))
            })
            .expect("a d20 always has faces")
    }

    #[must_use]
    pub fn collider_vertices(&self) -> Vec<Vec3> {
        self.vertices.to_vec()
    }

    #[must_use]
    pub fn undirected_edges(&self) -> BTreeSet<(usize, usize)> {
        self.faces
            .iter()
            .flat_map(|face| face_edges(face.vertices))
            .collect()
    }
}

/// Explicit topology and numbering table. Opposite entries sum to 21.
const FACE_TABLE: [(u8, [usize; 3]); 20] = [
    (1, [0, 8, 2]),
    (2, [0, 2, 10]),
    (3, [0, 6, 4]),
    (4, [0, 4, 8]),
    (5, [0, 10, 6]),
    (19, [1, 3, 9]),
    (20, [1, 11, 3]),
    (6, [1, 4, 6]),
    (7, [1, 9, 4]),
    (8, [1, 6, 11]),
    (15, [2, 5, 7]),
    (13, [2, 8, 5]),
    (14, [2, 7, 10]),
    (18, [3, 7, 5]),
    (16, [3, 5, 9]),
    (17, [3, 11, 7]),
    (9, [4, 9, 8]),
    (10, [5, 8, 9]),
    (11, [6, 10, 11]),
    (12, [7, 11, 10]),
];

/// Returns the project-owned regular d20 at unit face-to-face scale.
#[must_use]
pub fn d20_geometry() -> D20Geometry {
    let vertices = raw_vertices().map(|vertex| vertex * SCALE);
    let mut faces = FACE_TABLE.map(|(value, indices)| make_face(value, indices, vertices));
    let snapshot = faces;
    for face in &mut faces {
        face.adjacent_values = adjacent_values(face.vertices, snapshot);
        face.opposite_value = opposite_value(face.normal, snapshot);
    }
    D20Geometry { vertices, faces }
}

fn raw_vertices() -> [Vec3; 12] {
    [
        Vec3::new(0.0, -1.0, -PHI),
        Vec3::new(0.0, -1.0, PHI),
        Vec3::new(0.0, 1.0, -PHI),
        Vec3::new(0.0, 1.0, PHI),
        Vec3::new(-1.0, -PHI, 0.0),
        Vec3::new(-1.0, PHI, 0.0),
        Vec3::new(1.0, -PHI, 0.0),
        Vec3::new(1.0, PHI, 0.0),
        Vec3::new(-PHI, 0.0, -1.0),
        Vec3::new(-PHI, 0.0, 1.0),
        Vec3::new(PHI, 0.0, -1.0),
        Vec3::new(PHI, 0.0, 1.0),
    ]
}

fn make_face(value: u8, indices: [usize; 3], vertices: [Vec3; 12]) -> D20Face {
    let [a, b, c] = indices.map(|index| vertices[index]);
    let normal = (b - a).cross(c - a).normalize();
    let label_right = (b - a).normalize();
    D20Face {
        value,
        vertices: indices,
        normal,
        center: (a + b + c) / 3.0,
        label_right,
        label_up: normal.cross(label_right).normalize(),
        adjacent_values: [0; 3],
        opposite_value: 0,
    }
}

fn adjacent_values(vertices: [usize; 3], faces: [D20Face; 20]) -> [u8; 3] {
    let edges = face_edges(vertices);
    edges.map(|edge| {
        faces
            .iter()
            .find(|candidate| {
                candidate.vertices != vertices && face_edges(candidate.vertices).contains(&edge)
            })
            .expect("every d20 edge has a neighbouring face")
            .value
    })
}

fn opposite_value(normal: Vec3, faces: [D20Face; 20]) -> u8 {
    faces
        .iter()
        .min_by(|left, right| left.normal.dot(normal).total_cmp(&right.normal.dot(normal)))
        .expect("a d20 always has opposite faces")
        .value
}

fn face_edges([a, b, c]: [usize; 3]) -> [(usize, usize); 3] {
    [
        (a.min(b), a.max(b)),
        (b.min(c), b.max(c)),
        (c.min(a), c.max(a)),
    ]
}

/// Builds a flat-shaded render mesh from the canonical d20 face table.
#[must_use]
pub fn d20_mesh() -> Mesh {
    let geometry = d20_geometry();
    let mut positions = Vec::with_capacity(60);
    let mut normals = Vec::with_capacity(60);
    let mut indices = Vec::with_capacity(60);
    for face in geometry.faces {
        let base = u32::try_from(positions.len()).expect("d20 mesh is small");
        positions.extend(
            face.vertices
                .map(|index| geometry.vertices[index].to_array()),
        );
        normals.extend([face.normal.to_array(); 3]);
        indices.extend([base, base + 1, base + 2]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(indices))
}
