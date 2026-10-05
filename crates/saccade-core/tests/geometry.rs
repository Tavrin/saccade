//! Analytic, synthetic surface-distance and loader witnesses.
#![cfg(feature = "geometry")]
#![allow(clippy::unwrap_used, missing_docs)]
use parry3d_f64::na::Point3;
use saccade_core::geometry::{self, Mesh};
fn plane(z: f64, offset: f64) -> Mesh {
    Mesh {
        vertices: vec![
            Point3::new(offset, 0.0, z),
            Point3::new(offset + 1.0, 0.0, z),
            Point3::new(offset + 1.0, 1.0, z),
            Point3::new(offset, 1.0, z),
        ],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        resources: vec![],
        limitations: vec![],
    }
}
#[test]
fn parallel_planes_have_known_bidirectional_distances_and_preserve_large_coordinates() {
    for offset in [0.0, 1e9] {
        let a = plane(0.0, offset);
        let b = plane(0.125, offset);
        let e = geometry::compare(&a, &b, "m", 512)
            .unwrap()
            .evidence
            .unwrap();
        for v in [
            e.sampled_hausdorff,
            e.mean,
            e.rms,
            e.baseline_to_capture.p95,
            e.capture_to_baseline.p99,
        ] {
            assert!((v - 0.125).abs() < 1e-8, "{v}");
        }
        assert!(e.baseline_to_capture.normal_mean_degrees.unwrap() < 1e-6);
        assert!(!e.identity.ordered_geometry_identical);
    }
}
#[test]
fn retriangulation_preserves_surface_but_flipped_faces_keep_oriented_normal_error() {
    let a = plane(0.0, 0.0);
    let mut b = plane(0.0, 0.0);
    b.triangles = vec![[0, 1, 3], [1, 2, 3]];
    let e = geometry::compare(&a, &b, "m", 128)
        .unwrap()
        .evidence
        .unwrap();
    assert!(e.sampled_hausdorff < 1e-12);
    assert!(!e.identity.ordered_geometry_identical);
    b.triangles.iter_mut().for_each(|t| t.swap(0, 1));
    let e = geometry::compare(&a, &b, "m", 128)
        .unwrap()
        .evidence
        .unwrap();
    assert!(e.sampled_hausdorff < 1e-12);
    assert!((e.baseline_to_capture.normal_mean_degrees.unwrap() - 180.0).abs() < 1e-8);
}
#[test]
fn small_removed_component_is_seen_by_mandatory_defect_probes_and_empty_mesh_is_rejected() {
    let a = plane(0.0, 0.0);
    let mut b = plane(0.0, 0.0);
    b.vertices.extend([
        Point3::new(0.4, 0.4, 2.0),
        Point3::new(0.401, 0.4, 2.0),
        Point3::new(0.4, 0.401, 2.0),
    ]);
    b.triangles.push([4, 5, 6]);
    let e = geometry::compare(&a, &b, "m", 1).unwrap().evidence.unwrap();
    assert!((e.sampled_hausdorff - 2.0).abs() < 1e-12);
    assert!(e.mean < 0.001);
    assert_eq!(e.capture_to_baseline.area_samples, 3);
    b.triangles.clear();
    assert!(geometry::compare(&a, &b, "m", 16).is_err());
}
#[test]
fn obj_identity_is_content_bound_and_uv_changes_do_not_become_geometry_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let a = tmp.path().join("a.obj");
    let b = tmp.path().join("b.obj");
    let text = "v 1000000000 0 0\nv 1000000000.001 0 0\nv 1000000000 1 0\nvt 0 0\nvt 1 0\nvt 0 1\nf 1/1 2/2 3/3\n";
    std::fs::write(&a, text).unwrap();
    std::fs::write(&b, text.replace("vt 1 0", "vt 0.5 0")).unwrap();
    let (a, b) = (geometry::load(&a).unwrap(), geometry::load(&b).unwrap());
    assert!((a.vertices[1].x - a.vertices[0].x - 0.001).abs() < 1e-6);
    let proof = geometry::identity(&a, &b, "m").unwrap().evidence.unwrap();
    assert!(proof.ordered_geometry_identical);
    assert!(!proof.source_geometry_resources_identical);
    assert!(matches!(
        proof.appearance_attributes,
        saccade_core::evidence::analysis::Capability::Unsupported { .. }
    ));
}
#[test]
fn gltf_applies_f64_node_transforms_and_hashes_external_buffers() {
    let tmp = tempfile::tempdir().unwrap();
    let positions: Vec<u8> = [0f32, 0., 0., 1., 0., 0., 0., 1., 0.]
        .iter()
        .flat_map(|x| x.to_le_bytes())
        .collect();
    std::fs::write(tmp.path().join("mesh.bin"), &positions).unwrap();
    let mut json = serde_json::json!({"asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0]}],"nodes":[{"mesh":0,"translation":[1000000000.001,0,2]}],"meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}],"buffers":[{"uri":"mesh.bin","byteLength":36}],"bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":36}],"accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]}]});
    let path = tmp.path().join("mesh.gltf");
    std::fs::write(&path, json.to_string()).unwrap();
    let mesh = geometry::load(&path).unwrap();
    assert_eq!(mesh.vertices[0], Point3::new(1000000000.001, 0.0, 2.0));
    assert_eq!(mesh.resources.len(), 2);
    json["buffers"][0]["uri"] = "../mesh.bin".into();
    std::fs::write(&path, json.to_string()).unwrap();
    assert!(geometry::load(&path).is_err());
}

#[test]
fn nested_closed_surfaces_use_surface_projection_not_solid_containment() {
    fn cube(s: f64) -> Mesh {
        Mesh {
            vertices: vec![
                Point3::new(-s, -s, -s),
                Point3::new(s, -s, -s),
                Point3::new(s, s, -s),
                Point3::new(-s, s, -s),
                Point3::new(-s, -s, s),
                Point3::new(s, -s, s),
                Point3::new(s, s, s),
                Point3::new(-s, s, s),
            ],
            triangles: vec![
                [0, 2, 1],
                [0, 3, 2],
                [4, 5, 6],
                [4, 6, 7],
                [0, 1, 5],
                [0, 5, 4],
                [3, 7, 6],
                [3, 6, 2],
                [0, 4, 7],
                [0, 7, 3],
                [1, 2, 6],
                [1, 6, 5],
            ],
            resources: vec![],
            limitations: vec![],
        }
    }
    let e = geometry::compare(&cube(1.0), &cube(2.0), "m", 128)
        .unwrap()
        .evidence
        .unwrap();
    assert!((e.baseline_to_capture.mean - 1.0).abs() < 1e-12);
    assert!((e.sampled_hausdorff - 3.0f64.sqrt()).abs() < 1e-12);
}
