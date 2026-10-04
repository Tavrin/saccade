#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]
use image::{Rgb, RgbImage};
use saccade_core::evidence::{Artifact, Document};
use std::process::Command;

#[test]
fn visual_intent_is_checked_without_provider() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().join("base");
    let cap = tmp.path().join("cap");
    std::fs::create_dir_all(&base).unwrap();
    std::fs::create_dir_all(&cap).unwrap();
    let baseline = RgbImage::from_pixel(64, 64, Rgb([80, 80, 80]));
    let mut capture = baseline.clone();
    for y in 8..24 {
        for x in 8..24 {
            capture.put_pixel(x, y, Rgb([240, 240, 240]));
        }
    }
    baseline.save(base.join("a.png")).unwrap();
    capture.save(cap.join("a.png")).unwrap();
    let intent = tmp.path().join("intent.json");
    std::fs::write(&intent, r#"{"schema":"saccade-visual-intent.v1","objective":"patch","no_change_elsewhere":true,"changes":[{"entry":"a.png","kind":"structure","rect_frac":[0.5,0.5,0.5,0.5]}]}"#).unwrap();
    let out = tmp.path().join("out");
    let result = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .arg("compare")
        .arg(&base)
        .arg(&cap)
        .arg("--out")
        .arg(&out)
        .arg("--threshold")
        .arg("1")
        .arg("--intent-file")
        .arg(&intent)
        .arg("--json")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["intent_verification"]["status"], "mismatch");
    let full: serde_json::Value =
        serde_json::from_slice(&std::fs::read(out.join("intent-verification.v1.json")).unwrap())
            .unwrap();
    assert_eq!(full["missing"].as_array().unwrap().len(), 1);
    assert_eq!(full["unexpected"].as_array().unwrap().len(), 1);
    assert!(
        std::fs::read_to_string(out.join("index.html"))
            .unwrap()
            .contains("Intent verification")
    );
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(out.join("saccade-report.v1.json")).unwrap())
            .unwrap();
    assert!(
        !report["entries"][0]["changed_pixel_runs"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let mask = tmp.path().join("patch-mask.png");
    let mut mask_image = image::GrayImage::new(64, 64);
    for y in 8..24 {
        for x in 8..24 {
            mask_image.put_pixel(x, y, image::Luma([255]));
        }
    }
    mask_image.save(&mask).unwrap();
    std::fs::write(&intent, r#"{"schema":"saccade-visual-intent.v1","objective":"patch","no_change_elsewhere":true,"changes":[{"entry":"a.png","kind":"structure","mask":"patch-mask.png"}]}"#).unwrap();
    let mask_out = tmp.path().join("mask-out");
    let result = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .arg("compare")
        .arg(&base)
        .arg(&cap)
        .arg("--out")
        .arg(&mask_out)
        .arg("--intent-file")
        .arg(&intent)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    assert_eq!(
        std::fs::read(mask_out.join("assets/patch-mask.png")).unwrap(),
        std::fs::read(mask).unwrap()
    );
    let document_path = mask_out.join("evidence.json");
    let document = Document::read(&document_path).unwrap();
    let Artifact::Case(case) = document.artifact else {
        panic!("expected case");
    };
    let intent = case.intent.value().unwrap();
    assert_eq!(intent.mask_sources.len(), 1);
    intent.mask_sources[0].verify(&document_path).unwrap();
    std::fs::write(mask_out.join("assets/patch-mask.png"), b"tampered").unwrap();
    assert!(intent.mask_sources[0].verify(&document_path).is_err());
}
