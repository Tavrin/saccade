#![allow(missing_docs, clippy::unwrap_used, clippy::expect_used)]
mod support;
use saccade_geo::{RgbMapping, compare, input, mask_metrics};
use tiff::encoder::colortype;
fn validate(value: &serde_json::Value) {
    let schema: serde_json::Value = serde_json::from_str(
        saccade_core::schema_catalog::get(value["schema"].as_str().unwrap()).unwrap(),
    )
    .unwrap();
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(value)
        .unwrap();
}
#[test]
fn native_float_statistics_nodata_maps_and_explicit_preview() {
    let dir = tempfile::tempdir().unwrap();
    support::fixtures(dir.path());
    let p = dir.path();
    let result = compare(
        &p.join("float-a.tiff"),
        &p.join("float-b.tiff"),
        &p.join("out"),
        None,
    )
    .unwrap();
    validate(&result);
    assert!(result["perceptual"].is_null());
    assert_eq!(result["reference"]["grid"]["crs"], "EPSG:32631");
    assert_eq!(
        result["reference"]["grid"]["geotransform"],
        serde_json::json!([100., 2., 0., 200., 0., -2.])
    );
    assert_eq!(result["bands"][1]["changed_pixels"], 4);
    assert_eq!(result["bands"][1]["valid_pairs"], 196);
    assert_eq!(result["bands"][1]["nodata"]["both"], 60);
    assert_eq!(result["bands"][1]["statistics"]["max_delta"], 2.);
    assert!(
        (result["bands"][1]["statistics"]["mean_delta"]
            .as_f64()
            .unwrap()
            - 8. / 196.)
            .abs()
            < 1e-12
    );
    let change = image::open(p.join("out/band-2-change.png"))
        .unwrap()
        .to_luma8();
    assert_eq!(change[(7, 7)].0, [255]);
    assert_eq!(change[(0, 0)].0, [128]);
    let mut decoder =
        tiff::decoder::Decoder::new(std::fs::File::open(p.join("out/band-2-delta.tiff")).unwrap())
            .unwrap();
    let tiff::decoder::DecodingResult::F64(delta) = decoder.read_image().unwrap() else {
        panic!("not float64")
    };
    assert_eq!(delta[7 * 16 + 7], 2.);
    assert!(delta[0].is_nan());
    let mapping = RgbMapping {
        bands: [1, 2, 3],
        min: [0.; 3],
        max: [64.; 3],
    };
    let preview = compare(
        &p.join("float-a.tiff"),
        &p.join("float-b.tiff"),
        &p.join("rgb"),
        Some(&mapping),
    )
    .unwrap();
    validate(&preview);
    assert!(preview["perceptual"]["metrics"]["mean"].as_f64().unwrap() > 0.);
    assert_eq!(preview["perceptual"]["valid_pairs"], 196);
}
#[test]
fn grid_and_crs_refusals_include_both_descriptions_and_write_nothing() {
    let dir = tempfile::tempdir().unwrap();
    support::fixtures(dir.path());
    let p = dir.path();
    for name in ["other-grid.tiff", "other-crs.tiff"] {
        let error =
            compare(&p.join("float-a.tiff"), &p.join(name), &p.join("out"), None).unwrap_err();
        assert_eq!(error.code(), "different_crs_grid");
        let text = error.to_string();
        assert!(
            text.contains("reference:")
                && text.contains("candidate:")
                && text.contains("different CRS/grid")
        );
        assert!(!p.join("out").exists());
    }
}
#[test]
fn multichannel_u16_native_units_and_single_band_classes() {
    let dir = tempfile::tempdir().unwrap();
    support::fixtures(dir.path());
    let p = dir.path();
    let result = compare(
        &p.join("word-a.tiff"),
        &p.join("word-b.tiff"),
        &p.join("out"),
        None,
    )
    .unwrap();
    validate(&result);
    assert_eq!(result["reference"]["bands"], 4);
    assert_eq!(result["bands"][3]["changed_pixels"], 1);
    assert_eq!(result["bands"][3]["statistics"]["max_delta"], 1024.);
    let result = mask_metrics(
        &p.join("class-a.tiff"),
        &p.join("class-b.tiff"),
        &saccade_core::mask_metrics::Policy {
            classes: saccade_core::mask_metrics::ClassSelection::EachLabel,
            ..Default::default()
        },
    )
    .unwrap();
    validate(&result);
    assert_eq!(result["excluded_pixels"], 2);
    assert_eq!(result["metrics"]["evaluated_pixels"], 254);
    let class = result["metrics"]["classes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "id=1000")
        .unwrap();
    assert_eq!(class["iou"], 63. / 64.);
    let void_policy = saccade_core::mask_metrics::Policy {
        void: Some(saccade_core::mask_metrics::ClassSpec::parse("ignored=id=0").unwrap()),
        ..Default::default()
    };
    let void_result = mask_metrics(
        &p.join("class-a.tiff"),
        &p.join("class-b.tiff"),
        &void_policy,
    )
    .unwrap();
    validate(&void_result);
    assert_eq!(void_result["excluded_pixels"], 192);
    assert_eq!(void_result["declared_policy"]["void"]["name"], "ignored");
    assert_eq!(
        void_result["declared_policy"]["void"]["predicate"]["values"],
        serde_json::json!([0])
    );
    assert!(
        mask_metrics(
            &p.join("word-a.tiff"),
            &p.join("word-b.tiff"),
            &Default::default()
        )
        .is_err()
    );
}
#[test]
fn all_required_depths_are_lossless_and_nodata_is_not_silently_inferred() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    macro_rules! depth {
        ($c:ty,$v:expr) => {{
            let path = p.join("type.tiff");
            support::raster::<$c>(&path, 2, 1, &$v, None, None);
            let r = input::read(&path).unwrap();
            assert_eq!(r.values, $v.into_iter().map(f64::from).collect::<Vec<_>>());
        }};
    }
    depth!(colortype::Gray8, [0u8, 255]);
    depth!(colortype::Gray16, [0u16, 65535]);
    depth!(colortype::Gray32, [0u32, u32::MAX]);
    depth!(colortype::GrayI8, [i8::MIN, i8::MAX]);
    depth!(colortype::GrayI16, [i16::MIN, i16::MAX]);
    depth!(colortype::GrayI32, [i32::MIN, i32::MAX]);
    depth!(colortype::Gray32Float, [-0.5f32, 2.5]);
    support::raster::<support::Float<1>>(&p.join("nan.tiff"), 2, 1, &[f32::NAN, 1.], None, None);
    assert!(input::read(&p.join("nan.tiff")).is_err());
    support::raster::<support::Float<1>>(
        &p.join("nan.tiff"),
        2,
        1,
        &[f32::NAN, f32::NAN],
        None,
        Some("nan"),
    );
    let result = compare(
        &p.join("nan.tiff"),
        &p.join("nan.tiff"),
        &p.join("out"),
        None,
    )
    .unwrap();
    assert!(result["bands"][0]["statistics"].is_null());
}
#[test]
fn tile_coverage_changed_tile_and_identical_control() {
    let dir = tempfile::tempdir().unwrap();
    support::fixtures(dir.path());
    let p = dir.path();
    let result =
        saccade_geo::tiles::compare(&p.join("tiles-a"), &p.join("tiles-b"), &p.join("out"))
            .unwrap();
    validate(&result);
    assert_eq!(result["verdict"], "changed");
    assert_eq!(
        result["coverage"][0]["missing"],
        serde_json::json!(["2/1/1"])
    );
    assert_eq!(result["coverage"][0]["extra"], serde_json::json!(["2/1/3"]));
    assert_eq!(result["changed_tiles"], 1);
    assert_eq!(result["compared_tiles"], 2);
    assert!(result["tiles"][1]["metrics"]["mean"].as_f64().unwrap() > 0.);
    let control =
        saccade_geo::tiles::compare(&p.join("tiles-a"), &p.join("tiles-a"), &p.join("control"))
            .unwrap();
    assert_eq!(control["verdict"], "identical");
    std::fs::copy(p.join("tiles-a/2/1/0.png"), p.join("tiles-a/2/1/0.jpg")).unwrap();
    assert!(
        saccade_geo::tiles::compare(&p.join("tiles-a"), &p.join("tiles-b"), &p.join("bad"))
            .is_err()
    );
}

#[test]
fn planar_and_seven_band_chunky_preserve_every_sample() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    let planes: Vec<Vec<u16>> = (0..7)
        .map(|b| (0..6).map(|i| b * 100 + i).collect())
        .collect();
    support::planar(&p.join("planar.tiff"), 3, 2, &planes);
    let interleaved: Vec<u16> = (0..6)
        .flat_map(|i| planes.iter().map(move |p| p[i]))
        .collect();
    support::raster::<support::Word<7>>(&p.join("chunky.tiff"), 3, 2, &interleaved, None, None);
    let a = input::read(&p.join("planar.tiff")).unwrap();
    let b = input::read(&p.join("chunky.tiff")).unwrap();
    assert_eq!(a.bands, 7);
    assert_eq!(a.values, b.values);
    assert_eq!(
        a.values,
        interleaved.into_iter().map(f64::from).collect::<Vec<_>>()
    );
}
#[test]
fn float_nodata_uses_native_precision_and_one_sided_changes_are_visible() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    support::raster::<support::Float<1>>(&p.join("a.tiff"), 2, 1, &[-3.4, 1.], None, Some("-3.4"));
    support::raster::<support::Float<1>>(&p.join("b.tiff"), 2, 1, &[2., -3.4], None, Some("-3.4"));
    let result = compare(&p.join("a.tiff"), &p.join("b.tiff"), &p.join("out"), None).unwrap();
    assert_eq!(result["bands"][0]["valid_pairs"], 0);
    assert_eq!(result["bands"][0]["nodata"]["reference_only"], 1);
    assert_eq!(result["bands"][0]["nodata"]["candidate_only"], 1);
}

#[test]
fn oversized_dimensions_are_refused_without_integer_overflow() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("oversized.tiff");
    support::raster::<support::Word<7>>(&path, 3, 2, &[0u16; 42], None, None);
    let mut bytes = std::fs::read(&path).unwrap();
    let offset = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let count = u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap()) as usize;
    for entry in bytes[offset + 2..offset + 2 + count * 12]
        .as_chunks_mut::<12>()
        .0
        .iter_mut()
    {
        let tag = u16::from_le_bytes(entry[..2].try_into().unwrap());
        if matches!(tag, 256 | 257 | 278) {
            entry[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        }
    }
    std::fs::write(&path, bytes).unwrap();
    assert!(input::read(&path).is_err());
}
