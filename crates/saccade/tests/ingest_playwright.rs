#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]
use image::{Rgb, RgbImage};
use std::process::Command;

#[test]
fn manifest_maps_one_playwright_failure_to_comparison_and_sidecars() {
    let tmp = tempfile::tempdir().unwrap();
    let expected = tmp.path().join("expected.png");
    let actual = tmp.path().join("actual.png");
    RgbImage::from_pixel(16, 16, Rgb([30, 30, 30]))
        .save(&expected)
        .unwrap();
    RgbImage::from_pixel(16, 16, Rgb([240, 240, 240]))
        .save(&actual)
        .unwrap();
    let manifest = tmp.path().join("playwright.json");
    std::fs::write(&manifest, r#"{"schema":"saccade-playwright.v1","entries":[{"test_id":"card:0","project":"chromium","browser":"chromium","viewport":[800,600],"expected":"expected.png","actual":"actual.png","diff":null}]}"#).unwrap();
    let out = tmp.path().join("ingested");
    let result = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["ingest", "playwright"])
        .arg(&manifest)
        .arg("--out")
        .arg(&out)
        .arg("--json")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["totals"]["fail"], 1, "{value}");
    let mapping: serde_json::Value =
        serde_json::from_slice(&std::fs::read(out.join("playwright-mapping.json")).unwrap())
            .unwrap();
    assert_eq!(mapping["entries"][0]["project"], "chromium");
    let sidecar: serde_json::Value =
        serde_json::from_slice(&std::fs::read(out.join("capture/0000.saccade-meta.json")).unwrap())
            .unwrap();
    assert_eq!(sidecar["playwright_viewport_width"], 800);
    assert_eq!(sidecar["playwright_viewport_height"], 600);
    assert!(out.join("report/saccade-report.v1.json").is_file());
}

#[test]
fn playwright_manifest_rejects_escape_before_copy() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("results");
    std::fs::create_dir(&root).unwrap();
    let outside = tmp.path().join("outside.png");
    RgbImage::from_pixel(16, 16, Rgb([30, 30, 30]))
        .save(&outside)
        .unwrap();
    let manifest = root.join("manifest.json");
    for (index, escape) in [
        "../outside.png".to_string(),
        outside.to_string_lossy().into_owned(),
    ]
    .iter()
    .enumerate()
    {
        std::fs::write(&manifest, serde_json::json!({"schema":"saccade-playwright.v1","entries":[{"test_id":"case","project":"chromium","browser":"chromium","viewport":[800,600],"expected":escape,"actual":escape,"diff":null}]}).to_string()).unwrap();
        let out = tmp.path().join(format!("out-{index}"));
        let result = Command::new(env!("CARGO_BIN_EXE_saccade"))
            .args(["ingest", "playwright"])
            .arg(&manifest)
            .arg("--out")
            .arg(&out)
            .output()
            .unwrap();
        assert_ne!(result.status.code(), Some(0), "{result:?}");
        assert!(!out.exists());
    }
}
