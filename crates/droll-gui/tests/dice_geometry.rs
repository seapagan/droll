use std::collections::BTreeSet;

use avian3d::prelude::{Collider, ComputeMassProperties3d};
use bevy::{
    math::Quat,
    mesh::{Indices, Mesh, VertexAttributeValues},
    prelude::Vec3,
};
use droll_gui::dice::{d6_geometry, d6_labels, d6_mesh, d6_solid_symmetries, d6_symmetry_mapping};

const EPSILON: f32 = 1.0e-5;

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
