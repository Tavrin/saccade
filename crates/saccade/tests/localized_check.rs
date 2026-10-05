#![allow(clippy::unwrap_used, missing_docs)]
use saccade_core::localized::{DomMetadata, SelectorGeometry, digest};
use std::process::Command;
#[test]
fn box_and_selector_freeze_before_measurement_and_preserve_collateral() {
    let tmp = tempfile::tempdir().unwrap();
    let reference = tmp.path().join("reference.png");
    let candidate = tmp.path().join("candidate.png");
    let image = image::RgbImage::from_pixel(32, 32, image::Rgb([50, 70, 90]));
    image.save(&reference).unwrap();
    let mut changed = image.clone();
    changed.put_pixel(10, 10, image::Rgb([70, 70, 90]));
    changed.save(&candidate).unwrap();
    let out = tmp.path().join("box");
    let result = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .arg("localized-check")
        .arg(&reference)
        .arg(&candidate)
        .args(["--box", "8,8,8,8", "--out"])
        .arg(&out)
        .arg("--json")
        .output()
        .unwrap();
    assert!(result.status.success(), "{result:?}");
    assert!(out.join("frozen-region.json").is_file());
    let metadata = DomMetadata {
        schema: "saccade-dom-regions.v1".into(),
        reference_sha256: digest(&std::fs::read(&reference).unwrap()),
        dimensions: [32, 32],
        selectors: vec![SelectorGeometry {
            selector: "main .card".into(),
            boxes: vec![[8, 8, 8, 8]],
        }],
    };
    let file = tmp.path().join("dom.json");
    std::fs::write(&file, serde_json::to_vec(&metadata).unwrap()).unwrap();
    changed.put_pixel(0, 0, image::Rgb([51, 70, 90]));
    changed.save(&candidate).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .arg("localized-check")
        .arg(&reference)
        .arg(&candidate)
        .args(["--selector", "main .card", "--metadata"])
        .arg(&file)
        .arg("--out")
        .arg(tmp.path().join("selector"))
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(tmp.path().join("selector/localized.json")).unwrap())
            .unwrap();
    assert_eq!(report["outside"]["changed_pixels"], 1);
    assert_eq!(report["collateral"], "collateral_change");
}

#[test]
fn phrase_mask_import_freezes_reference_scope_without_model_inference() {
    let temp = tempfile::tempdir().unwrap();
    let reference = temp.path().join("reference.png");
    let mask = temp.path().join("mask.png");
    let region = temp.path().join("phrase.json");
    image::RgbImage::from_pixel(16, 16, image::Rgb([30, 40, 50]))
        .save(&reference)
        .unwrap();
    image::GrayImage::from_fn(16, 16, |x, y| {
        image::Luma([if (4..8).contains(&x) && (4..8).contains(&y) {
            255
        } else {
            0
        }])
    })
    .save(&mask)
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["regions", "import", "--reference"])
        .arg(&reference)
        .arg("--mask")
        .arg(&mask)
        .args(["--phrase", "the left sphere", "--out"])
        .arg(&region)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let frozen: saccade_core::localized::FrozenRegion =
        serde_json::from_slice(&std::fs::read(region).unwrap()).unwrap();
    assert_eq!(frozen.provenance["original_phrase"], "the left sphere");
    assert_eq!(frozen.provenance["model_inference"], "not_run");
    assert_eq!(frozen.inclusion.iter().filter(|&&v| v == 1).count(), 16);
    assert_eq!(
        frozen.reference_sha256,
        digest(&std::fs::read(reference).unwrap())
    );
}
