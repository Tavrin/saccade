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

#[test]
fn inventoried_decode_error_is_retained_beside_a_passing_capture() {
    let tmp = tempfile::tempdir().unwrap();
    let good = tmp.path().join("good.png");
    let bad = tmp.path().join("bad.png");
    RgbImage::from_pixel(16, 16, Rgb([50, 60, 70]))
        .save(&good)
        .unwrap();
    std::fs::write(&bad, b"not a decodable PNG").unwrap();
    let mut entries = vec![];
    let mut expected = vec![];
    let mut supplied = vec![];
    for (index, (id, path)) in [("pass", &good), ("bad", &bad)].into_iter().enumerate() {
        entries.push(serde_json::json!({"test_id":id,"case_id":id,"project":"chromium","browser":"chromium","viewport":[16,16],"expected":"good.png","actual":path.file_name().unwrap().to_str().unwrap(),"diff":null}));
        expected.push(
            serde_json::json!({"case_id":id,"entry":format!("{index:04}.png"),"required":true}),
        );
        supplied.push(serde_json::json!({"case_id":id,"entry":format!("{index:04}.png"),"state":"captured","capture_sha256":saccade_core::run::sha256_file(path).unwrap()}));
    }
    let manifest = tmp.path().join("manifest.json");
    std::fs::write(&manifest,serde_json::json!({"schema":"saccade-playwright.v1","entries":entries,"inventory":{"schema":"saccade-inventory.v1","expected":expected,"supplied":supplied}}).to_string()).unwrap();
    let out = tmp.path().join("out");
    let result = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["ingest", "playwright"])
        .arg(manifest)
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    let inventory: serde_json::Value =
        serde_json::from_slice(&std::fs::read(out.join("inventory.json")).unwrap()).unwrap();
    assert_eq!(inventory["expected"], 2);
    assert_eq!(inventory["compared"], 1);
    assert_eq!(inventory["outcomes"]["unusable"], 1);
    assert_eq!(inventory["coverage"], "incomplete");
}

#[test]
fn snapshot_names_and_duplicate_sources_cannot_create_complete_pairs() {
    let tmp = tempfile::tempdir().unwrap();
    for role in ["expected", "actual"] {
        RgbImage::from_pixel(16, 16, Rgb([30, 50, 70]))
            .save(tmp.path().join(format!("{role}.png")))
            .unwrap();
    }
    let script = r#"
const Reporter=require(process.argv[1]),path=require('node:path'),fs=require('node:fs');
const root=process.argv[2];
const test={id:'test',annotations:[],parent:{project:()=>({name:'chromium',use:{}})}};
for(const mode of ['mismatch','duplicate']) {
 const r=new Reporter({outputFile:path.join(root,mode+'.json')}); r.onBegin({}, {allTests:()=>[test]});
 const a=(name,role)=>({name,contentType:'image/png',path:path.join(root,role+'.png')});
 const attachments=mode==='mismatch'?[a('a-expected','expected'),a('b-actual','actual')]:[a('a-expected','expected'),a('a-actual','actual'),a('b-expected','expected'),a('b-actual','actual')];
 r.onTestEnd(test,{status:'passed',retry:0,attachments});r.onEnd();
 const m=JSON.parse(fs.readFileSync(path.join(root,mode+'.json')));
 if(mode==='mismatch' && m.inventory.supplied.some(s=>s.state==='captured')) throw Error('mismatched names paired');
 if(mode==='duplicate' && m.inventory.supplied.every(s=>s.state==='captured')) throw Error('duplicate sources concealed');
}
"#;
    let node = Command::new("node")
        .args(["-e", script])
        .arg(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../integrations/playwright/reporter.cjs"),
        )
        .arg(tmp.path())
        .output()
        .unwrap();
    assert!(node.status.success(), "{node:?}");
}

fn two_case_manifest(tmp: &std::path::Path) -> serde_json::Value {
    RgbImage::from_pixel(16, 16, Rgb([50, 60, 70]))
        .save(tmp.join("good.png"))
        .unwrap();
    let hash = saccade_core::run::sha256_file(&tmp.join("good.png")).unwrap();
    serde_json::json!({"schema":"saccade-playwright.v1","entries":(0..2).map(|i|serde_json::json!({"test_id":format!("test{i}"),"case_id":format!("case{i}"),"project":"chromium","browser":"chromium","viewport":null,"expected":"good.png","actual":"good.png","diff":null})).collect::<Vec<_>>(),"inventory":{"schema":"saccade-inventory.v1","expected":(0..2).map(|i|serde_json::json!({"case_id":format!("case{i}"),"entry":format!("{i:04}.png"),"required":true})).collect::<Vec<_>>(),"supplied":(0..2).map(|i|serde_json::json!({"case_id":format!("case{i}"),"entry":format!("{i:04}.png"),"state":"captured","capture_sha256":hash})).collect::<Vec<_>>()}})
}
#[test]
fn contradictory_snapshot_case_id_is_rejected_before_comparison() {
    let tmp = tempfile::tempdir().unwrap();
    let mut m = two_case_manifest(tmp.path());
    m["entries"][0]["case_id"] = "other-case".into();
    let path = tmp.path().join("manifest.json");
    std::fs::write(&path, m.to_string()).unwrap();
    let out = tmp.path().join("out");
    let r = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["ingest", "playwright"])
        .arg(path)
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(r.status.code(), Some(2), "{r:?}");
    assert!(!out.exists());
}
#[test]
fn missing_attachment_preserves_suite_accounting_and_usable_case() {
    let tmp = tempfile::tempdir().unwrap();
    let mut m = two_case_manifest(tmp.path());
    m["entries"][1]["actual"] = "missing.png".into();
    let path = tmp.path().join("manifest.json");
    std::fs::write(&path, m.to_string()).unwrap();
    let out = tmp.path().join("out");
    let r = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["ingest", "playwright"])
        .arg(path)
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(r.status.code(), Some(1), "{r:?}");
    let inv: serde_json::Value =
        serde_json::from_slice(&std::fs::read(out.join("inventory.json")).unwrap()).unwrap();
    assert_eq!(inv["compared"], 1);
    assert_eq!(inv["outcomes"]["missing"], 1);
}
