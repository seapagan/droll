use std::collections::BTreeSet;

use bevy::{math::Quat, prelude::Vec3};
use droll_gui::dice::{d6_geometry, d6_labels};

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
