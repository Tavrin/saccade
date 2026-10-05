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

#[test]
fn reporter_inventories_passing_missing_skipped_and_retry_attempts() {
    let tmp = tempfile::tempdir().unwrap();
    let expected = tmp.path().join("expected.png");
    let actual = tmp.path().join("actual.png");
    let image = image::RgbImage::from_pixel(16, 16, image::Rgb([30, 50, 70]));
    image.save(&expected).unwrap();
    image.save(&actual).unwrap();
    let reporter = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../integrations/playwright/reporter.cjs");
    let script = r#"
const Reporter=require(process.argv[1]);
const root=process.argv[2];const path=require('node:path');
const reporter=new Reporter({outputFile:path.join(root,'manifest.json')});
const test=id=>({id,annotations:[],parent:{project:()=>({name:'chromium',use:{browserName:'chromium',viewport:{width:16,height:16}}})}});
const tests=['pass','missing','skip'].map(test);reporter.onBegin({}, {allTests:()=>tests});
const attachments=['expected','actual'].map(role=>({name:'card-'+role,contentType:'image/png',path:path.join(root,role+'.png')}));
reporter.onTestEnd(tests[0],{status:'passed',retry:0,attachments});
reporter.onTestEnd(tests[1],{status:'passed',retry:0,attachments:[]});
reporter.onTestEnd(tests[2],{status:'skipped',retry:0,attachments:[]});
reporter.onEnd();
"#;
    let node = Command::new("node")
        .args(["-e", script])
        .arg(reporter)
        .arg(tmp.path())
        .output()
        .unwrap();
    assert!(node.status.success(), "{node:?}");
    let output = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["ingest", "playwright"])
        .arg(tmp.path().join("manifest.json"))
        .arg("--out")
        .arg(tmp.path().join("out"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let inventory: serde_json::Value =
        serde_json::from_slice(&std::fs::read(tmp.path().join("out/inventory.json")).unwrap())
            .unwrap();
    assert_eq!(inventory["expected"], 3);
    assert_eq!(inventory["compared"], 1);
    assert_eq!(inventory["outcomes"]["missing"], 1);
    assert_eq!(inventory["outcomes"]["skipped"], 1);
    assert_eq!(inventory["coverage"], "incomplete");
    // Empty attachment suites still produce explicit missing coverage.
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(tmp.path().join("manifest.json")).unwrap()).unwrap();
    manifest["entries"] = serde_json::json!([]);
    manifest["inventory"]["supplied"][0]["state"] = "missing".into();
    manifest["inventory"]["supplied"][0]["entry"] = serde_json::Value::Null;
    std::fs::write(tmp.path().join("empty.json"), manifest.to_string()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["ingest", "playwright"])
        .arg(tmp.path().join("empty.json"))
        .arg("--out")
        .arg(tmp.path().join("empty-out"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(tmp.path().join("empty-out/inventory.json").is_file());
}
