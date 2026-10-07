//! Generated fixtures exercise both commands without downloads or OCR prerequisites.
#![cfg(feature = "text-quality")]
#![allow(clippy::unwrap_used)]
use serde_json::Value;
use std::process::Command;
#[test]
fn generated_text_quality_truth_and_schemas() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = tempfile::tempdir().unwrap();
    let result = Command::new("python3")
        .arg(root.join("scripts/text-quality/fixtures.py"))
        .arg("--out")
        .arg(out.path())
        .arg("--binary")
        .arg(env!("CARGO_BIN_EXE_saccade"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    for file in std::fs::read_dir(out.path()).unwrap() {
        let file = file.unwrap().path();
        if !file
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("report-")
        {
            continue;
        }
        let report: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        let schema: Value = serde_json::from_str(
            saccade_core::schema_catalog::get(report["schema"].as_str().unwrap()).unwrap(),
        )
        .unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        assert!(validator.is_valid(&report), "{}", file.display());
    }
}
#[test]
fn invalid_regions_are_typed_errors() {
    let out = tempfile::tempdir().unwrap();
    let image = out.path().join("image.png");
    image::RgbaImage::new(20, 20).save(&image).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["text-legibility"])
        .arg(&image)
        .arg(&image)
        .args(["--region", "4294967295,0,2,2", "--json"])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["execution"], "error");
    assert_eq!(report["errors"][0]["code"], "config");
}

#[test]
fn imported_ocr_disagreement_and_stale_sources_fail_closed() {
    let out = tempfile::tempdir().unwrap();
    let image = out.path().join("image.png");
    let mut pixels = image::RgbaImage::from_pixel(80, 40, image::Rgba([255, 255, 255, 255]));
    for y in 10..25 {
        for x in 10..14 {
            pixels.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
        }
    }
    pixels.save(&image).unwrap();
    let hash = saccade_core::localized::digest(&std::fs::read(&image).unwrap());
    let source = |text: &str| serde_json::json!({"schema":"saccade-ui-source.v1","capture_sha256":hash,"dimensions":[80,40],"kind":"paddle_ocr","producer":{"adapter":"fixture"},"complete":false,"nodes":[{"id":"0","text":text,"bounds":[8.,8.,30.,20.],"ocr_confidence":95.}]});
    let a = out.path().join("a.json");
    let b = out.path().join("b.json");
    std::fs::write(&a, serde_json::to_vec(&source("Sample")).unwrap()).unwrap();
    std::fs::write(&b, serde_json::to_vec(&source("Changed")).unwrap()).unwrap();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_saccade"))
            .arg("text-legibility")
            .arg(&image)
            .arg(&image)
            .args(["--region", "0,0,80,40", "--baseline-source"])
            .arg(&a)
            .arg("--variant-source")
            .arg(&b)
            .arg("--json")
            .output()
            .unwrap()
    };
    let result = run();
    assert_eq!(
        result.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        report["variants"][0]["regions"][0]["ocr"]["agrees_with_baseline"],
        false
    );
    let mut stale = source("Sample");
    stale["capture_sha256"] = "0".repeat(64).into();
    std::fs::write(&b, serde_json::to_vec(&stale).unwrap()).unwrap();
    assert_eq!(run().status.code(), Some(2));
}
