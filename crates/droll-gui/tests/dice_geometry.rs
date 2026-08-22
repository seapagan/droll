use std::collections::{BTreeMap, BTreeSet};

use avian3d::prelude::{Collider, ComputeMassProperties3d};
use bevy::{
    math::{Mat3, Quat},
    mesh::{Indices, Mesh, VertexAttributeValues},
    prelude::Vec3,
};
use droll_gui::dice::{
    D20_FACE_TO_FACE, D20SolidSymmetry, d6_geometry, d6_labels, d6_mesh, d6_solid_symmetries,
    d6_symmetry_mapping, d20_geometry, d20_labels, d20_mesh, d20_solid_symmetries,
    d20_symmetry_mapping, d20_symmetry_mappings,
};

const EPSILON: f32 = 1.0e-5;
const D20_EPSILON: f32 = 2.0e-5;

#[test]
fn test_d20_topology_is_a_finite_regular_icosahedron() {
    let geometry = d20_geometry();
    assert_eq!(geometry.vertices.len(), 12);
    assert_eq!(geometry.faces.len(), 20);
    assert_eq!(geometry.undirected_edges().len(), 30);
    assert_eq!(12_i32 - 30 + 20, 2);
    for (index, vertex) in geometry.vertices.iter().enumerate() {
        assert!(
            geometry.vertices[index + 1..]
                .iter()
                .all(|candidate| vertex.distance(*candidate) > D20_EPSILON)
        );
    }
    assert!(geometry.vertices.iter().all(|vertex| vertex.is_finite()));

    let mut edge_counts = BTreeMap::new();
    let mut vertex_degrees: [BTreeSet<usize>; 12] = std::array::from_fn(|_| BTreeSet::new());
    for face in geometry.faces {
        let [a, b, c] = face.vertices;
        for (left, right) in [(a, b), (b, c), (c, a)] {
            let edge = (left.min(right), left.max(right));
            *edge_counts.entry(edge).or_insert(0) += 1;
            vertex_degrees[left].insert(right);
            vertex_degrees[right].insert(left);
        }
    }
    assert!(edge_counts.values().all(|count| *count == 2));
    assert!(
        vertex_degrees
            .iter()
            .all(|neighbours| neighbours.len() == 5)
    );

    let lengths = geometry
        .undirected_edges()
        .into_iter()
        .map(|(a, b)| geometry.vertices[a].distance(geometry.vertices[b]))
        .collect::<Vec<_>>();
    assert!(
        lengths
            .iter()
            .all(|length| (*length - lengths[0]).abs() < D20_EPSILON)
    );
}

#[test]
fn test_d20_faces_share_one_numbering_geometry_source() {
    let geometry = d20_geometry();
    assert_eq!(D20_FACE_TO_FACE, 1.0);
    assert_eq!(
        geometry
            .faces
            .iter()
            .map(|face| face.value)
            .collect::<BTreeSet<_>>(),
        (1..=20).collect()
    );
    for face in geometry.faces {
        let [a, b, c] = face.vertices.map(|index| geometry.vertices[index]);
        let geometric_normal = (b - a).cross(c - a).normalize();
        assert!(face.normal.is_finite());
        assert!((face.normal.length() - 1.0).abs() < D20_EPSILON);
        assert!(face.normal.dot(face.center) > 0.0);
        assert!(face.normal.dot(geometric_normal) > 1.0 - D20_EPSILON);
        assert!((face.center.length() - 0.5).abs() < D20_EPSILON);
        assert_eq!(face.value + face.opposite_value, 21);
        assert_eq!(
            face.adjacent_values
                .into_iter()
                .collect::<BTreeSet<_>>()
                .len(),
            3
        );
        assert!(
            face.adjacent_values
                .iter()
                .all(|value| *value != face.value)
        );
        assert!(face.label_right.dot(face.normal).abs() < D20_EPSILON);
        assert!(face.label_up.dot(face.normal).abs() < D20_EPSILON);
        assert!(face.label_right.cross(face.label_up).dot(face.normal) > 1.0 - D20_EPSILON);
    }
}

#[test]
fn test_d20_render_mesh_is_outward_wound() {
    let mesh = d20_mesh();
    let positions = mesh_positions(&mesh, "d20");
    let normals = mesh_normals(&mesh, "d20");
    let indices = mesh_indices(&mesh, "d20");
    assert_eq!(positions.len(), 60);
    assert_eq!(indices.len(), 60);
    for triangle in indices.chunks_exact(3) {
        let a = Vec3::from_array(positions[triangle[0] as usize]);
        let b = Vec3::from_array(positions[triangle[1] as usize]);
        let c = Vec3::from_array(positions[triangle[2] as usize]);
        let declared = Vec3::from_array(normals[triangle[0] as usize]);
        assert!((b - a).cross(c - a).dot(declared) > 0.0);
    }
}

#[test]
fn test_d20_target_rotation_keeps_each_face_uniquely_up() {
    let geometry = d20_geometry();
    for face in geometry.faces {
        for yaw in [0.0, 0.37, 1.9, 5.2] {
            let rotation = face.target_rotation(yaw);
            assert!(((rotation * face.normal) - Vec3::Y).length() < D20_EPSILON);
            let scores = geometry
                .faces
                .map(|candidate| (rotation * candidate.normal).dot(Vec3::Y));
            assert_eq!(geometry.upward_face(rotation).value, face.value);
            assert_eq!(
                scores
                    .iter()
                    .filter(|score| **score > 1.0 - D20_EPSILON)
                    .count(),
                1
            );
        }
    }
}

#[test]
fn test_d20_labels_are_parallel_outside_and_fit_their_faces() {
    let geometry = d20_geometry();
    let labels = d20_labels();
    assert_eq!(labels.len(), 20);
    for label in labels {
        let face = geometry.face(label.value).expect("label face exists");
        assert_eq!(label.value, label.face_value);
        assert_eq!(label.normal, face.normal);
        assert!(!label.vertices.is_empty());
        assert_eq!(label.indices.len() % 3, 0);
        for vertex in label.vertices {
            let offset = vertex - face.center;
            assert!(offset.dot(face.normal) > 0.0);
            assert!((offset.dot(face.normal) - 0.006).abs() < D20_EPSILON);
            assert_point_inside_face(
                vertex - face.normal * 0.006,
                face.vertices,
                geometry.vertices,
            );
        }
        assert_eq!(label.has_orientation_mark, matches!(label.value, 6 | 9));
    }
}

#[test]
fn test_d20_collider_mass_uses_only_canonical_vertices() {
    let geometry = d20_geometry();
    assert_eq!(geometry.collider_vertices(), geometry.vertices);
    let collider = Collider::convex_hull(geometry.collider_vertices()).expect("valid d20 hull");
    let properties = collider.mass_properties(1.0);
    assert!(properties.mass.is_finite() && properties.mass > 0.0);
    assert!(properties.center_of_mass.length() < D20_EPSILON);
    let inertia = properties.principal_angular_inertia;
    assert!((inertia.x - inertia.y).abs() < D20_EPSILON);
    assert!((inertia.y - inertia.z).abs() < D20_EPSILON);
    assert!(
        d20_labels()
            .iter()
            .map(|label| label.vertices.len())
            .sum::<usize>()
            > 12
    );
}

#[test]
fn test_d20_symmetry_group_has_60_unique_proper_rotations() {
    let symmetries = d20_solid_symmetries();
    assert_eq!(symmetries.len(), 60);
    assert!(
        symmetries
            .iter()
            .any(|symmetry| same_rotation(symmetry.rotation, Quat::IDENTITY))
    );
    for (index, symmetry) in symmetries.iter().enumerate() {
        assert_eq!(usize::from(symmetry.id), index);
        assert!((symmetry.rotation.length() - 1.0).abs() < D20_EPSILON);
        assert!((Mat3::from_quat(symmetry.rotation).determinant() - 1.0).abs() < D20_EPSILON);
        assert!(
            symmetries[index + 1..]
                .iter()
                .all(|candidate| !same_rotation(symmetry.rotation, candidate.rotation))
        );
    }
}

#[test]
fn test_d20_symmetry_group_is_closed_and_has_inverses() {
    let symmetries = d20_solid_symmetries();
    for left in &symmetries {
        assert!(
            symmetries
                .iter()
                .any(|candidate| { same_rotation(candidate.rotation, left.rotation.inverse()) })
        );
        for right in &symmetries {
            assert!(symmetries.iter().any(|candidate| {
                same_rotation(candidate.rotation, left.rotation * right.rotation)
            }));
        }
    }
}

#[test]
fn test_d20_symmetries_preserve_geometry_topology_and_mass() {
    let geometry = d20_geometry();
    let baseline = Collider::convex_hull(geometry.collider_vertices())
        .expect("valid hull")
        .mass_properties(1.0);
    for symmetry in d20_solid_symmetries() {
        assert_same_d20_vertex_set(
            geometry.vertices,
            geometry.vertices.map(|vertex| symmetry.rotation * vertex),
        );
        let transformed = Collider::convex_hull(
            geometry
                .vertices
                .map(|vertex| symmetry.rotation * vertex)
                .to_vec(),
        )
        .expect("symmetry preserves hull")
        .mass_properties(1.0);
        assert!((transformed.mass - baseline.mass).abs() < D20_EPSILON);
        assert!((transformed.center_of_mass - baseline.center_of_mass).length() < D20_EPSILON);
        assert!(
            (transformed.principal_angular_inertia - baseline.principal_angular_inertia).length()
                < D20_EPSILON
        );
        assert_mapped_d20_relationships(symmetry);
    }
}

#[test]
fn test_d20_every_ordered_face_pair_has_three_phased_mappings() {
    let geometry = d20_geometry();
    let mut orbit = BTreeSet::new();
    for target in 1..=20 {
        for base in 1..=20 {
            let mappings = d20_symmetry_mappings(target, base);
            assert_eq!(mappings.len(), 3, "target={target} base={base}");
            for phase in 0..3 {
                let selected = d20_symmetry_mapping(target, base, phase).expect("phase mapping");
                assert!(mappings.iter().any(|candidate| candidate.id == selected.id));
                let target_face = geometry.face(target).expect("target exists");
                let base_face = geometry.face(base).expect("base exists");
                assert_eq!(selected.map_face(target_face).value, base);
                assert_face_triangle_maps(selected, target_face.vertices, base_face.vertices);
                let start = geometry.vertices[base_face.vertices[usize::from(phase)]];
                let end = geometry.vertices[base_face.vertices[(usize::from(phase) + 1) % 3]];
                let expected_right = (end - start).normalize();
                assert!(
                    (selected.rotation * target_face.label_right - expected_right).length()
                        < D20_EPSILON
                );
                assert!(
                    (selected.rotation * target_face.label_up
                        - base_face.normal.cross(expected_right).normalize())
                    .length()
                        < D20_EPSILON
                );
            }
        }
        orbit.extend(d20_solid_symmetries().into_iter().map(|symmetry| {
            symmetry
                .map_face(geometry.face(target).expect("target exists"))
                .value
        }));
    }
    assert_eq!(orbit, (1..=20).collect());
}

#[test]
fn test_d20_target_symmetry_is_right_composed_after_world_nuisance() {
    let geometry = d20_geometry();
    let base_face = 9;
    let base_rotation = Quat::from_euler(bevy::math::EulerRot::XYZ, 0.4, -0.2, 0.9);
    let nuisance =
        Quat::from_axis_angle(Vec3::new(1.0, 1.0, 0.0).normalize(), 0.5_f32.to_radians());
    let variant = nuisance * base_rotation;
    for phase in 0..3 {
        for target in 1..=20 {
            let symmetry = d20_symmetry_mapping(target, base_face, phase).expect("mapping exists");
            let mapped = variant * symmetry.rotation;
            assert_same_d20_vertex_set(
                geometry.vertices.map(|vertex| variant * vertex),
                geometry.vertices.map(|vertex| mapped * vertex),
            );
            let target_normal = geometry.face(target).expect("target exists").normal;
            let base_normal = geometry.face(base_face).expect("base exists").normal;
            assert!(((mapped * target_normal) - (variant * base_normal)).length() < D20_EPSILON);
        }
    }
}

#[test]
fn test_d6_topology_and_values_are_complete() {
    let geometry = d6_geometry();
    assert_eq!(geometry.vertices.len(), 8);
    assert_eq!(geometry.faces.len(), 6);
    assert_eq!(
        geometry
            .faces
            .iter()
            .map(|face| face.value)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([1, 2, 3, 4, 5, 6])
    );
    for face in geometry.faces {
        let [a, b, c, d] = face.vertices.map(|index| geometry.vertices[index]);
        assert!((b - a).cross(c - a).length() > EPSILON);
        assert!((c - a).cross(d - a).length() > EPSILON);
        assert!((face.normal.length() - 1.0).abs() < EPSILON);
        assert!(face.center.dot(face.normal) > 0.0);
    }
}

#[test]
fn test_d6_render_triangles_are_non_degenerate_and_outward_wound() {
    let mesh = d6_mesh();
    let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(positions)) => positions,
        _ => panic!("d6 mesh must have f32 positions"),
    };
    let normals = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
        Some(VertexAttributeValues::Float32x3(normals)) => normals,
        _ => panic!("d6 mesh must have f32 normals"),
    };
    let indices = match mesh.indices() {
        Some(Indices::U32(indices)) => indices,
        _ => panic!("d6 mesh must have u32 indices"),
    };

    assert_eq!(indices.len(), 6 * 2 * 3);
    for triangle in indices.chunks_exact(3) {
        let [a_index, b_index, c_index] = triangle else {
            unreachable!("chunks_exact returns three indices")
        };
        let a = Vec3::from_array(positions[*a_index as usize]);
        let b = Vec3::from_array(positions[*b_index as usize]);
        let c = Vec3::from_array(positions[*c_index as usize]);
        let geometric_normal = (b - a).cross(c - a);
        let declared_normal = Vec3::from_array(normals[*a_index as usize]);
        assert!(geometric_normal.length() > EPSILON);
        assert!(geometric_normal.dot(declared_normal) > 0.0);
    }
}

#[test]
fn test_d6_target_rotation_preserves_face_across_yaw() {
    let geometry = d6_geometry();
    for face in geometry.faces {
        for yaw in [0.0, 0.37, 1.9, 5.2] {
            let rotation = face.target_rotation(yaw);
            assert!(((rotation * face.normal) - Vec3::Y).length() < EPSILON);
            assert_eq!(geometry.upward_face(rotation).value, face.value);
        }
    }
}

#[test]
fn test_d6_collider_hull_matches_mesh_scale() {
    let geometry = d6_geometry();
    assert_eq!(geometry.collider_vertices(), geometry.vertices);
    for vertex in geometry.collider_vertices() {
        assert!((vertex.abs() - Vec3::splat(0.5)).length() < EPSILON);
    }
}

#[test]
fn test_d6_labels_follow_face_bases_without_z_fighting() {
    let geometry = d6_geometry();
    let labels = d6_labels();
    assert_eq!(labels.len(), 21);
    for label in labels {
        let face = geometry.face(label.value).expect("label value is a face");
        assert_eq!(label.normal, face.normal);
        assert!(label.center.dot(face.normal) > face.center.dot(face.normal));
        assert!(face.label_right.dot(face.normal).abs() < EPSILON);
        assert!(face.label_up.dot(face.normal).abs() < EPSILON);
    }
}

#[test]
fn test_upward_face_is_stable_for_identity_rotation() {
    assert_eq!(d6_geometry().upward_face(Quat::IDENTITY).value, 1);
}

#[test]
fn test_d6_symmetry_group_contains_24_proper_rotations() {
    let symmetries = d6_solid_symmetries();
    assert_eq!(symmetries.len(), 24);
    for symmetry in symmetries {
        let matrix = bevy::math::Mat3::from_quat(symmetry.rotation);
        assert!((matrix.determinant() - 1.0).abs() < EPSILON);
        assert!((symmetry.rotation.length() - 1.0).abs() < EPSILON);
    }
}

#[test]
fn test_d6_symmetries_preserve_vertices_faces_and_opposites() {
    let geometry = d6_geometry();
    for symmetry in d6_solid_symmetries() {
        assert_same_vertex_set(
            geometry.vertices,
            geometry.vertices.map(|vertex| symmetry.rotation * vertex),
        );
        let mapped = geometry
            .faces
            .map(|face| symmetry.map_face(face).value)
            .into_iter()
            .collect::<BTreeSet<_>>();
        assert_eq!(mapped, BTreeSet::from([1, 2, 3, 4, 5, 6]));
        for face in geometry.faces {
            let opposite = geometry
                .faces
                .into_iter()
                .find(|candidate| candidate.normal == -face.normal)
                .expect("every d6 face has an opposite");
            assert_eq!(
                symmetry.map_face(opposite).normal,
                -symmetry.map_face(face).normal
            );
        }
    }
}

#[test]
fn test_d6_symmetry_maps_every_ordered_face_pair() {
    for target in 1..=6 {
        for base in 1..=6 {
            let symmetry = d6_symmetry_mapping(target, base).expect("mapping exists");
            assert_eq!(
                symmetry
                    .map_face(d6_geometry().face(target).expect("target exists"))
                    .value,
                base
            );
        }
    }
}

#[test]
fn test_quaternion_composition_applies_right_operand_first() {
    let world_y = Quat::from_rotation_y(0.7);
    let local_x = Quat::from_rotation_x(0.4);
    let probe = Vec3::new(0.2, 0.6, -0.7).normalize();
    assert!(((world_y * local_x) * probe - world_y * (local_x * probe)).length() < EPSILON);
    assert!(((world_y * local_x) * probe - (local_x * world_y) * probe).length() > 0.1);
}

#[test]
fn test_target_symmetry_follows_target_independent_orientation_nuisance() {
    let geometry = d6_geometry();
    let base_face = 3;
    let base_rotation = Quat::from_euler(bevy::math::EulerRot::XYZ, 0.4, -0.2, 0.9);
    let nuisance_axis = Vec3::new(1.0, 1.0, 0.0).normalize();
    for angle in [0.0, 0.5_f32.to_radians(), (-0.5_f32).to_radians()] {
        let nuisance = Quat::from_axis_angle(nuisance_axis, angle);
        let variant = nuisance * base_rotation;
        for target in 1..=6 {
            let symmetry = d6_symmetry_mapping(target, base_face).expect("mapping exists");
            let mapped = variant * symmetry.rotation;
            assert_same_vertex_set(
                geometry.vertices.map(|vertex| variant * vertex),
                geometry.vertices.map(|vertex| mapped * vertex),
            );
            let target_normal = geometry.face(target).expect("target exists").normal;
            let base_normal = geometry.face(base_face).expect("base exists").normal;
            assert!(((mapped * target_normal) - (variant * base_normal)).length() < EPSILON);
        }
    }
}

#[test]
fn test_d6_symmetry_keeps_collider_mass_properties_and_pips_render_only() {
    let geometry = d6_geometry();
    let collider = Collider::convex_hull(geometry.collider_vertices()).expect("valid hull");
    let baseline = collider.mass_properties(1.0);
    assert_eq!(d6_labels().len(), 21);
    for symmetry in d6_solid_symmetries() {
        let transformed = Collider::convex_hull(
            geometry
                .vertices
                .map(|vertex| symmetry.rotation * vertex)
                .to_vec(),
        )
        .expect("symmetry preserves hull");
        let properties = transformed.mass_properties(1.0);
        assert!((properties.mass - baseline.mass).abs() < EPSILON);
        assert!((properties.center_of_mass - baseline.center_of_mass).length() < EPSILON);
        assert!(
            (properties.principal_angular_inertia - baseline.principal_angular_inertia).length()
                < EPSILON
        );
    }
}

fn assert_same_vertex_set(left: [Vec3; 8], right: [Vec3; 8]) {
    for vertex in left {
        assert!(
            right
                .iter()
                .any(|candidate| (*candidate - vertex).length() < EPSILON),
            "missing vertex {vertex:?} in {right:?}"
        );
    }
}

fn mesh_positions<'a>(mesh: &'a Mesh, die: &str) -> &'a [[f32; 3]] {
    match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(positions)) => positions,
        _ => panic!("{die} mesh must have f32 positions"),
    }
}

fn mesh_normals<'a>(mesh: &'a Mesh, die: &str) -> &'a [[f32; 3]] {
    match mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
        Some(VertexAttributeValues::Float32x3(normals)) => normals,
        _ => panic!("{die} mesh must have f32 normals"),
    }
}

fn mesh_indices<'a>(mesh: &'a Mesh, die: &str) -> &'a [u32] {
    match mesh.indices() {
        Some(Indices::U32(indices)) => indices,
        _ => panic!("{die} mesh must have u32 indices"),
    }
}

fn assert_point_inside_face(point: Vec3, indices: [usize; 3], vertices: [Vec3; 12]) {
    let [a, b, c] = indices.map(|index| vertices[index]);
    let normal = (b - a).cross(c - a).normalize();
    for (start, end) in [(a, b), (b, c), (c, a)] {
        assert!((end - start).cross(point - start).dot(normal) >= -D20_EPSILON);
    }
}

fn same_rotation(left: Quat, right: Quat) -> bool {
    left.dot(right).abs() > 1.0 - D20_EPSILON
}

fn assert_same_d20_vertex_set(left: [Vec3; 12], right: [Vec3; 12]) {
    for vertex in left {
        assert!(
            right
                .iter()
                .any(|candidate| candidate.distance(vertex) < D20_EPSILON),
            "missing vertex {vertex:?}"
        );
    }
}

fn assert_mapped_d20_relationships(symmetry: D20SolidSymmetry) {
    let geometry = d20_geometry();
    let mapped_values = geometry
        .faces
        .map(|face| symmetry.map_face(face).value)
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(mapped_values, (1..=20).collect());
    for face in geometry.faces {
        let mapped = symmetry.map_face(face);
        let mapped_adjacency = face
            .adjacent_values
            .map(|value| {
                symmetry
                    .map_face(geometry.face(value).expect("adjacent face exists"))
                    .value
            })
            .into_iter()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            mapped_adjacency,
            mapped.adjacent_values.into_iter().collect()
        );
        assert_eq!(
            symmetry
                .map_face(geometry.face(face.opposite_value).expect("opposite exists"))
                .value,
            mapped.opposite_value
        );
    }
}

fn assert_face_triangle_maps(symmetry: D20SolidSymmetry, target: [usize; 3], base: [usize; 3]) {
    let vertices = d20_geometry().vertices;
    let mapped = target.map(|index| symmetry.rotation * vertices[index]);
    for vertex in base.map(|index| vertices[index]) {
        assert!(
            mapped
                .iter()
                .any(|candidate| candidate.distance(vertex) < D20_EPSILON)
        );
    }
}
