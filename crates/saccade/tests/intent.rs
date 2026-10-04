#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]
use image::{Rgb, RgbImage};
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
}
