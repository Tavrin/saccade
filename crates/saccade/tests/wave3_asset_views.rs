//! Synthetic combined geometry/view report, including historical reader compatibility.
#![cfg(feature = "geometry")]
#![allow(clippy::unwrap_used, missing_docs)]
use saccade_core::asset_views::*;
use std::process::Command;
#[test]
fn geometry_packet_embeds_camera_views_with_exact_mesh_identity_and_coverage_exit() {
    let d = tempfile::tempdir().unwrap();
    let mesh = d.path().join("asset.obj");
    std::fs::write(&mesh, "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n").unwrap();
    let run = |extra: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_saccade"))
            .args([
                "experiment",
                "geometry",
                mesh.to_str().unwrap(),
                mesh.to_str().unwrap(),
                "--unit",
                "metres",
                "--samples",
                "4",
                "--json",
            ])
            .args(extra)
            .output()
            .unwrap()
    };
    let old = run(&[]);
    assert_eq!(old.status.code(), Some(0));
    let old: saccade_core::geometry::Document = serde_json::from_slice(&old.stdout).unwrap();
    assert!(old.views.is_none());
    let saccade_core::geometry::Operation::Geometry { analysis } = old.operation else {
        unreachable!()
    };
    let geometry = analysis.evidence.unwrap().identity.geometry_sha256;
    let doc = saccade_core::localized::digest(&std::fs::read(&mesh).unwrap());
    let h = "a".repeat(64);
    let identity = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    let camera = Camera {
        convention: "row_major_column_vector_right_handed_clip_z_zero_to_one".into(),
        model_to_world: identity,
        world_to_view: identity,
        projection: identity,
    };
    let context = Context {
        renderer: "synthetic pixels".into(),
        renderer_source_sha256: h.clone(),
        renderer_binary_sha256: h.clone(),
        settings_sha256: h.clone(),
        lighting_sha256: h.clone(),
        background_sha256: h.clone(),
        color_pipeline_sha256: h.clone(),
        material_mode: "asset_materials".into(),
        override_material_sha256: None,
        dimensions: [16, 16],
    };
    let image = image::RgbaImage::from_pixel(16, 16, image::Rgba([30, 40, 50, 255]));
    let path = d.path().join("image.png");
    image.save(&path).unwrap();
    let image = ImagePin {
        path: "image.png".into(),
        sha256: saccade_core::localized::digest(&std::fs::read(path).unwrap()),
    };
    let render = |side: usize| Render {
        image: image.clone(),
        silhouette: None,
        camera_sha256: hash(&camera).unwrap(),
        context_sha256: hash(&context).unwrap(),
        asset_document_sha256: doc.clone(),
        asset_geometry_sha256: geometry[side].clone(),
        materials_sha256: h.clone(),
        textures_sha256: vec![],
    };
    let view = View {
        id: "front".into(),
        camera: camera.clone(),
        reference: Some(render(0)),
        candidate: Some(render(1)),
    };
    let mut m = Manifest {
        schema: "saccade-asset-views.v1".into(),
        unit: "metres".into(),
        assets: [
            Asset {
                document_sha256: doc.clone(),
                geometry_sha256: geometry[0].clone(),
                lod: "source".into(),
            },
            Asset {
                document_sha256: doc,
                geometry_sha256: geometry[1].clone(),
                lod: "LOD0".into(),
            },
        ],
        context,
        views: vec![view],
        pixels_per_degree: 67.0,
        maximum_mean_flip: 0.01,
        maximum_silhouette_change: 0.0,
    };
    let manifest = d.path().join("manifest.json");
    let out = d.path().join("packet.json");
    std::fs::write(&manifest, serde_json::to_vec(&m).unwrap()).unwrap();
    let result = run(&[
        "--views",
        manifest.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    if !cfg!(feature = "graphics") {
        assert_eq!(result.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&result.stdout).contains("feature_unavailable"));
        return;
    }
    assert_eq!(
        result.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    let document: saccade_core::geometry::Document =
        serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
    assert!(document.views.unwrap().passed);
    m.views[0].candidate = None;
    std::fs::write(&manifest, serde_json::to_vec(&m).unwrap()).unwrap();
    let result = run(&["--views", manifest.to_str().unwrap()]);
    assert_eq!(result.status.code(), Some(1));
    let document: saccade_core::geometry::Document =
        serde_json::from_slice(&result.stdout).unwrap();
    let views = document.views.unwrap();
    assert_eq!(views.coverage.fraction, 0.0);
    assert!(views.worst_view.is_none());
    assert!(!views.passed);
    m.assets[0].document_sha256 = h;
    std::fs::write(&manifest, serde_json::to_vec(&m).unwrap()).unwrap();
    assert_eq!(
        run(&["--views", manifest.to_str().unwrap()]).status.code(),
        Some(2)
    );
}
