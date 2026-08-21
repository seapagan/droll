use std::collections::BTreeSet;

use bevy::{
    math::Quat,
    mesh::{Indices, Mesh, VertexAttributeValues},
    prelude::Vec3,
};
use droll_gui::dice::{d6_geometry, d6_labels, d6_mesh};

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
