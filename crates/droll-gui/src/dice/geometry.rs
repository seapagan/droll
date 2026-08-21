use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, Mesh, PrimitiveTopology},
    prelude::{Quat, Vec3},
};

/// Half the edge length of the generated d6.
pub const D6_HALF_EXTENT: f32 = 0.5;

/// Reusable metadata for one d6 face.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct D6Face {
    pub value: u8,
    pub vertices: [usize; 4],
    pub normal: Vec3,
    pub center: Vec3,
    pub label_right: Vec3,
    pub label_up: Vec3,
}

impl D6Face {
    /// Builds a target rotation with the selected face upward and varied yaw.
    #[must_use]
    pub fn target_rotation(self, yaw_radians: f32) -> Quat {
        Quat::from_axis_angle(Vec3::Y, yaw_radians) * Quat::from_rotation_arc(self.normal, Vec3::Y)
    }
}

/// Single source of truth for the generated mesh, collider hull, and faces.
#[derive(Clone, Debug)]
pub struct D6Geometry {
    pub vertices: [Vec3; 8],
    pub faces: [D6Face; 6],
}

impl D6Geometry {
    #[must_use]
    pub fn face(&self, value: u8) -> Option<D6Face> {
        self.faces.iter().copied().find(|face| face.value == value)
    }

    #[must_use]
    pub fn upward_face(&self, rotation: Quat) -> D6Face {
        self.faces
            .iter()
            .copied()
            .max_by(|left, right| {
                (rotation * left.normal)
                    .dot(Vec3::Y)
                    .total_cmp(&(rotation * right.normal).dot(Vec3::Y))
            })
            .expect("a d6 always has faces")
    }

    #[must_use]
    pub fn collider_vertices(&self) -> Vec<Vec3> {
        self.vertices.to_vec()
    }
}

/// Returns the project-owned unit d6 topology and conventional opposite faces.
#[must_use]
pub fn d6_geometry() -> D6Geometry {
    let h = D6_HALF_EXTENT;
    let vertices = [
        Vec3::new(-h, -h, -h),
        Vec3::new(h, -h, -h),
        Vec3::new(h, h, -h),
        Vec3::new(-h, h, -h),
        Vec3::new(-h, -h, h),
        Vec3::new(h, -h, h),
        Vec3::new(h, h, h),
        Vec3::new(-h, h, h),
    ];
    let faces = [
        face(1, [3, 2, 6, 7], Vec3::Y, Vec3::X, Vec3::NEG_Z),
        face(6, [0, 4, 5, 1], Vec3::NEG_Y, Vec3::X, Vec3::Z),
        face(2, [4, 7, 6, 5], Vec3::Z, Vec3::X, Vec3::Y),
        face(5, [1, 2, 3, 0], Vec3::NEG_Z, Vec3::NEG_X, Vec3::Y),
        face(3, [5, 6, 2, 1], Vec3::X, Vec3::NEG_Z, Vec3::Y),
        face(4, [0, 3, 7, 4], Vec3::NEG_X, Vec3::Z, Vec3::Y),
    ];
    D6Geometry { vertices, faces }
}

fn face(
    value: u8,
    vertices: [usize; 4],
    normal: Vec3,
    label_right: Vec3,
    label_up: Vec3,
) -> D6Face {
    D6Face {
        value,
        vertices,
        normal,
        center: normal * D6_HALF_EXTENT,
        label_right,
        label_up,
    }
}

/// Builds a flat-shaded render mesh from the same face definition as the hull.
#[must_use]
pub fn d6_mesh() -> Mesh {
    let geometry = d6_geometry();
    let mut positions = Vec::with_capacity(24);
    let mut normals = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    for face in geometry.faces {
        let base = u32::try_from(positions.len()).expect("d6 mesh is small");
        positions.extend(
            face.vertices
                .map(|index| geometry.vertices[index].to_array()),
        );
        normals.extend([face.normal.to_array(); 4]);
        indices.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(indices))
}
