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

fn triangle_fixture(root: &std::path::Path) -> (std::path::PathBuf, serde_json::Value) {
    let bytes: Vec<u8> = [0f32, 0., 0., 1., 0., 0., 0., 1., 0.]
        .iter()
        .flat_map(|x| x.to_le_bytes())
        .collect();
    std::fs::write(root.join("mesh.bin"), bytes).unwrap();
    (
        root.join("mesh.gltf"),
        serde_json::json!({"asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0]}],"nodes":[{"mesh":0}],"meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}],"buffers":[{"uri":"mesh.bin","byteLength":36}],"bufferViews":[{"buffer":0,"byteLength":36}],"accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]}]}),
    )
}

#[test]
fn failed_index_read_cannot_manufacture_identity() {
    let tmp = tempfile::tempdir().unwrap();
    let (path, mut doc) = triangle_fixture(tmp.path());
    std::fs::write(&path, doc.to_string()).unwrap();
    let valid = geometry::load(&path).unwrap();
    doc["accessors"].as_array_mut().unwrap().push(serde_json::json!({"bufferView":0,"byteOffset":36,"componentType":5123,"count":3,"type":"SCALAR"}));
    doc["meshes"][0]["primitives"][0]["indices"] = 1.into();
    std::fs::write(&path, doc.to_string()).unwrap();
    let loaded = geometry::load(&path);
    if let Ok(mesh) = &loaded {
        assert!(
            !geometry::identity(&valid, mesh, "m")
                .unwrap()
                .evidence
                .unwrap()
                .ordered_geometry_identical
        );
    }
    assert!(matches!(loaded, Err(saccade_core::Error::Config(_))));
}

#[test]
fn corrupt_meshes_return_errors_without_panicking() {
    let tmp = tempfile::tempdir().unwrap();
    let (path, mut doc) = triangle_fixture(tmp.path());
    doc["meshes"][0]["primitives"][0]["indices"] = 0.into();
    std::fs::write(&path, doc.to_string()).unwrap();
    let result = std::panic::catch_unwind(|| geometry::load(&path));
    assert!(result.is_ok(), "f32 index accessor panicked");
    assert!(matches!(
        result.unwrap(),
        Err(saccade_core::Error::Config(_))
    ));
    let (_, doc) = triangle_fixture(tmp.path());
    let gltf = doc.to_string().into_bytes();
    let obj = b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";
    for (extension, bytes) in [("gltf", gltf.as_slice()), ("obj", obj.as_slice())] {
        let path = tmp.path().join(format!("corrupt.{extension}"));
        for end in 0..bytes.len() {
            std::fs::write(&path, &bytes[..end]).unwrap();
            assert!(
                std::panic::catch_unwind(|| geometry::load(&path)).is_ok(),
                "{extension} truncated at {end}"
            );
        }
        for i in 0..bytes.len() {
            let mut corrupt = bytes.to_vec();
            corrupt[i] = 0xff;
            std::fs::write(&path, corrupt).unwrap();
            assert!(
                std::panic::catch_unwind(|| geometry::load(&path)).is_ok(),
                "{extension} corruption at {i}"
            );
        }
    }
}

#[test]
fn instanced_header_counts_are_rejected_before_expansion() {
    let tmp = tempfile::tempdir().unwrap();
    let (path, mut doc) = triangle_fixture(tmp.path());
    // The review's 1000 * 999999 expansion, with deliberately truncated data:
    // header admission must reject before even attempting a POSITION read.
    doc["accessors"][0]["count"] = 999_999.into();
    doc["nodes"] = serde_json::json!(vec![serde_json::json!({"mesh":0}); 1000]);
    doc["scenes"][0]["nodes"] = serde_json::json!((0..1000).collect::<Vec<_>>());
    std::fs::write(&path, doc.to_string()).unwrap();
    let err = geometry::load(&path).err().unwrap().to_string();
    assert!(err.contains("before expansion"), "{err}");
}
