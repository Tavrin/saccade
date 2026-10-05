#![allow(clippy::unwrap_used, missing_docs)]
use saccade_core::asset_views::*;
fn digest(c: char) -> String {
    c.to_string().repeat(64)
}
fn camera(index: usize) -> Camera {
    let identity = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    let mut c = Camera {
        convention: "row_major_column_vector_right_handed_clip_z_zero_to_one".into(),
        model_to_world: identity,
        world_to_view: identity,
        projection: identity,
    };
    c.world_to_view[3] = index as f64;
    if index % 2 == 1 {
        c.world_to_view[0] = -1.0;
        c.world_to_view[10] = -1.0;
        c.world_to_view[11] = 2.0;
    }
    c
}
fn manifest() -> Manifest {
    Manifest {
        schema: "saccade-asset-views.v1".into(),
        unit: "metres".into(),
        assets: [
            Asset {
                document_sha256: digest('a'),
                geometry_sha256: digest('c'),
                lod: "source".into(),
            },
            Asset {
                document_sha256: digest('b'),
                geometry_sha256: digest('d'),
                lod: "LOD1 selected cut".into(),
            },
        ],
        context: Context {
            renderer: "synthetic supplied pixels; no renderer execution".into(),
            renderer_source_sha256: digest('1'),
            renderer_binary_sha256: digest('2'),
            settings_sha256: digest('3'),
            lighting_sha256: digest('4'),
            background_sha256: digest('5'),
            color_pipeline_sha256: digest('6'),
            material_mode: "asset_materials".into(),
            override_material_sha256: None,
            dimensions: [32, 32],
        },
        views: vec![
            View {
                id: "front".into(),
                camera: camera(0),
                reference: None,
                candidate: None,
            },
            View {
                id: "rear".into(),
                camera: camera(1),
                reference: None,
                candidate: None,
            },
        ],
        pixels_per_degree: 67.0,
        maximum_mean_flip: 0.001,
        maximum_silhouette_change: 0.0,
    }
}
fn validate(m: &Manifest) -> saccade_core::Result<()> {
    m.validate(
        &[digest('a'), digest('b')],
        &[digest('c'), digest('d')],
        "metres",
    )
}
#[test]
fn manifest_binds_meshes_units_unique_nonsingular_cameras_and_context() {
    let m = manifest();
    validate(&m).unwrap();
    let mut bad = m.clone();
    bad.assets[1].geometry_sha256 = digest('e');
    assert!(validate(&bad).is_err());
    bad = m.clone();
    bad.unit = "centimetres".into();
    assert!(validate(&bad).is_err());
    bad = m.clone();
    bad.views[1].camera = bad.views[0].camera.clone();
    assert!(validate(&bad).is_err());
    bad = m.clone();
    bad.views[0].camera.projection = [0.0; 16];
    assert!(validate(&bad).is_err());
    bad = m.clone();
    bad.views[0].camera.world_to_view[3] = f64::INFINITY;
    assert!(validate(&bad).is_err());
    bad = m.clone();
    bad.context.override_material_sha256 = Some(digest('f'));
    assert!(validate(&bad).is_err());
    bad = m.clone();
    bad.views[1].id = bad.views[0].id.clone();
    assert!(validate(&bad).is_err());
}
#[cfg(not(feature = "graphics"))]
#[test]
fn reduced_build_can_read_a_manifest_but_cannot_produce_view_scores() {
    let m = manifest();
    let bytes = serde_json::to_vec(&m).unwrap();
    let m: Manifest = serde_json::from_slice(&bytes).unwrap();
    let d = tempfile::tempdir().unwrap();
    assert!(matches!(
        measure(
            m,
            d.path(),
            &[digest('a'), digest('b')],
            &[digest('c'), digest('d')],
            "metres"
        ),
        Err(saccade_core::Error::FeatureUnavailable {
            feature: "graphics"
        })
    ));
}
#[cfg(feature = "graphics")]
mod images {
    use super::*;
    fn pin(root: &std::path::Path, name: &str, image: &image::RgbaImage) -> ImagePin {
        let p = root.join(name);
        image.save(&p).unwrap();
        ImagePin {
            path: name.into(),
            sha256: saccade_core::localized::digest(&std::fs::read(p).unwrap()),
        }
    }
    fn receipts(
        m: &mut Manifest,
        root: &std::path::Path,
        images: &[(image::RgbaImage, image::RgbaImage)],
    ) {
        let context_sha256 = hash(&m.context).unwrap();
        for (index, (view, (a, b))) in m.views.iter_mut().zip(images).enumerate() {
            let camera_sha256 = hash(&view.camera).unwrap();
            for (side, image) in [a, b].into_iter().enumerate() {
                let r = Render {
                    image: pin(root, &format!("view-{index}-{side}.png"), image),
                    silhouette: None,
                    camera_sha256: camera_sha256.clone(),
                    context_sha256: context_sha256.clone(),
                    asset_document_sha256: m.assets[side].document_sha256.clone(),
                    asset_geometry_sha256: m.assets[side].geometry_sha256.clone(),
                    materials_sha256: digest(if side == 0 { '7' } else { '8' }),
                    textures_sha256: vec![digest(if side == 0 { '9' } else { '0' })],
                };
                if side == 0 {
                    view.reference = Some(r);
                } else {
                    view.candidate = Some(r);
                }
            }
        }
    }
    fn run(m: Manifest, root: &std::path::Path) -> Report {
        measure(
            m,
            root,
            &[digest('a'), digest('b')],
            &[digest('c'), digest('d')],
            "metres",
        )
        .unwrap()
    }
    fn base() -> image::RgbaImage {
        image::RgbaImage::from_pixel(32, 32, image::Rgba([60, 70, 80, 255]))
    }
    #[test]
    fn front_only_misses_constructed_rear_defect_and_full_set_preserves_worst_and_local_errors() {
        let d = tempfile::tempdir().unwrap();
        let mut m = manifest();
        let a = base();
        let mut rear = a.clone();
        for y in 12..16 {
            for x in 12..16 {
                rear.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
            }
        }
        receipts(&mut m, d.path(), &[(a.clone(), a.clone()), (a, rear)]);
        let mut front = m.clone();
        front.views.pop();
        let front = run(front, d.path());
        assert!(front.passed);
        assert_eq!(front.coverage.measured_views, 1);
        let report = run(m, d.path());
        assert!(!report.passed);
        assert_eq!(report.coverage.fraction, 1.0);
        assert_eq!(report.worst_view.unwrap().id, "rear");
        assert_eq!(report.views[0].flip.unwrap().mean, 0.0);
        assert!(report.views[1].flip.unwrap().mean > 0.001);
        assert!(report.views[1].high_error_bounds.is_some());
        assert_eq!(report.views[1].error_map.len(), 1024);
    }
    #[test]
    fn material_and_normal_map_appearance_and_fractional_silhouette_changes_remain_separate() {
        let d = tempfile::tempdir().unwrap();
        let mut m = manifest();
        let a = base();
        let material = image::RgbaImage::from_pixel(32, 32, image::Rgba([100, 50, 20, 255]));
        let normals = image::RgbaImage::from_fn(32, 32, |x, y| {
            let v = if (x + y) % 4 < 2 { 30 } else { 150 };
            image::Rgba([v, v, v, 255])
        });
        receipts(&mut m, d.path(), &[(a.clone(), material), (a, normals)]);
        let am = image::GrayImage::from_pixel(32, 32, image::Luma([255]));
        let mut bm = am.clone();
        for x in 0..8 {
            bm.put_pixel(x, 8, image::Luma([128]));
        }
        let p = d.path().join("a-mask.png");
        am.save(&p).unwrap();
        let ap = ImagePin {
            path: "a-mask.png".into(),
            sha256: saccade_core::localized::digest(&std::fs::read(p).unwrap()),
        };
        let p = d.path().join("b-mask.png");
        bm.save(&p).unwrap();
        let bp = ImagePin {
            path: "b-mask.png".into(),
            sha256: saccade_core::localized::digest(&std::fs::read(p).unwrap()),
        };
        m.views[1].reference.as_mut().unwrap().silhouette = Some(ap);
        m.views[1].candidate.as_mut().unwrap().silhouette = Some(bp);
        let r = run(m, d.path());
        assert!(!r.passed);
        assert!(r.views.iter().all(|v| v.flip.unwrap().mean > 0.001));
        let s = r.views[1].silhouette.as_ref().unwrap();
        assert_eq!(s.reference_coverage, 1.0);
        assert!((s.changed_fraction - 8.0 * 127.0 / 255.0 / 1024.0).abs() < 1e-12);
        assert_eq!(s.change_bounds, Some([0, 8, 8, 1]));
    }
    #[test]
    fn missing_rejected_and_stale_evidence_cannot_shrink_coverage_into_a_pass() {
        let d = tempfile::tempdir().unwrap();
        let mut m = manifest();
        let a = base();
        receipts(&mut m, d.path(), &[(a.clone(), a.clone()), (a.clone(), a)]);
        let mut missing = m.clone();
        missing.views[1].candidate = None;
        let r = run(missing, d.path());
        assert!(!r.passed);
        assert_eq!(r.coverage.fraction, 0.5);
        assert_eq!(r.coverage.missing_views, 1);
        assert!(r.views[1].flip.is_none());
        let mut bad = m.clone();
        bad.views[1].candidate.as_mut().unwrap().camera_sha256 = digest('e');
        let r = run(bad, d.path());
        assert!(!r.passed);
        assert_eq!(r.coverage.rejected_views, 1);
        assert!(r.views[1].issues[0].contains("identity"));
        bad = m.clone();
        bad.views[1].candidate.as_mut().unwrap().context_sha256 = digest('e');
        assert!(!run(bad, d.path()).passed);
        bad = m.clone();
        bad.views[1].candidate.as_mut().unwrap().image.sha256 = digest('e');
        let r = run(bad, d.path());
        assert!(r.views[1].issues[0].contains("hash"));
        assert!(!r.passed);
        let mut absent = m;
        absent.views[1].candidate.as_mut().unwrap().image.path = "not-supplied.png".into();
        let r = run(absent, d.path());
        assert_eq!(r.coverage.missing_views, 1);
        assert!(!r.passed);
    }
    #[cfg(unix)]
    #[test]
    fn images_and_missing_children_cannot_escape_through_paths_or_symlinks() {
        let d = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let mut m = manifest();
        let a = base();
        receipts(
            &mut m,
            d.path(),
            &[(a.clone(), a.clone()), (a.clone(), a.clone())],
        );
        let pin = pin(outside.path(), "outside.png", &a);
        std::os::unix::fs::symlink(outside.path(), d.path().join("escape")).unwrap();
        for path in ["../outside.png", "escape/outside.png", "escape/missing.png"] {
            let mut bad = m.clone();
            bad.views[0].reference.as_mut().unwrap().image = ImagePin {
                path: path.into(),
                sha256: pin.sha256.clone(),
            };
            assert!(
                measure(
                    bad,
                    d.path(),
                    &[digest('a'), digest('b')],
                    &[digest('c'), digest('d')],
                    "metres"
                )
                .is_err()
            );
        }
    }
}
