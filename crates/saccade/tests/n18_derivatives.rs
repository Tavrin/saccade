//! Generated geometry/text proofs; no native model quality claim.
#![allow(clippy::unwrap_used)]
use image::{Rgba, RgbaImage};
use saccade_core::wave7::{
    faces::{FACE_LIMIT, FACES_SCHEMA},
    vision::Provenance,
};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

fn setup(root: &Path) -> Value {
    let inputs = root.join("inputs");
    fs::create_dir(&inputs).unwrap();
    let mut image = RgbaImage::from_pixel(320, 240, Rgba([255; 4]));
    for n in 0..8 {
        for y in 100..116 {
            for x in 100 + n * 16..110 + n * 16 {
                if x < 103 + n * 16 || y < 103 || (107..110).contains(&y) || y >= 113 {
                    image.put_pixel(x, y, Rgba([0, 0, 0, 255]));
                }
            }
        }
    }
    image.save(inputs.join("source.png")).unwrap();
    json!({"schema":"saccade-derivatives.v1","subjects":[{"label":"protected product","rect_px":[10,20,30,40]}],"text_regions":[{"label":"small label","rect_px":[80,80,176,56]}],"derivatives":[{"id":"full","display_size":[320,240]}]})
}
fn run(root: &Path, declaration: &Value, out: &str, faces: Option<&str>) -> (i32, Value) {
    let file = root.join("inputs/declaration.json");
    fs::write(&file, serde_json::to_vec(declaration).unwrap()).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_saccade"));
    command
        .args(["derivative-sheet", "--json", "--out"])
        .arg(root.join(out))
        .arg(root.join("inputs/source.png"))
        .arg(file);
    // Force a deterministic missing-model environment without using the host cache.
    command.env("SACCADE_MODELS_DIR", root.join("absent-models"));
    if let Some(faces) = faces {
        command.arg("--faces-report").arg(root.join(faces));
    }
    let result = command.output().unwrap();
    (
        result.status.code().unwrap(),
        serde_json::from_slice(&result.stdout).unwrap(),
    )
}
fn schema(value: &Value) {
    let schema: Value = serde_json::from_str(
        saccade_core::schema_catalog::get("saccade-derivative-sheet.v1").unwrap(),
    )
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<_> = validator
        .iter_errors(value)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
}
fn receipt(root: &Path, faces: Value, hash: Option<&str>) {
    let bytes = fs::read(root.join("inputs/source.png")).unwrap();
    let r = json!({"schema":FACES_SCHEMA,"image_sha256":hash.map(str::to_string).unwrap_or_else(||saccade_core::localized::digest(&bytes)),"image_size":[320,240],"faces":faces,"provenance":Provenance::fixture("generated-face-box"),"limitations":FACE_LIMIT});
    fs::write(
        root.join("inputs/faces.json"),
        serde_json::to_vec(&r).unwrap(),
    )
    .unwrap();
}
#[test]
fn generated_rows_measure_display_text_and_manifest_binds_assets() {
    let root = tempfile::tempdir().unwrap();
    let mut d = setup(root.path());
    d["derivatives"] = json!([{"id":"full <label>","display_size":[320,240]},{"id":"tiny","display_size":[80,60]},{"id":"cut","display_size":[200,240],"crop_px":[20,0,200,240]},{"id":"excluded","display_size":[200,240],"crop_px":[80,0,200,240]}]);
    let (exit, report) = run(root.path(), &d, "review", None);
    assert_eq!(exit, 1);
    schema(&report);
    assert_eq!(report["rows"][0]["verdict"], "pass");
    assert_eq!(report["rows"][1]["text_legibility"]["state"], "illegible");
    assert_eq!(
        report["rows"][2]["subject_preservation"]["subjects"][0]["state"],
        "cut"
    );
    assert_eq!(
        report["rows"][3]["subject_preservation"]["subjects"][0]["state"],
        "excluded"
    );
    assert_eq!(report["faces"]["state"], "unavailable");
    let out = root.path().join("review");
    assert!(saccade_core::manifest::verify(&out).unwrap().is_empty());
    let manifest: Value =
        serde_json::from_slice(&fs::read(out.join("saccade-manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["reports"][0]["report_id"], report["report_id"]);
    assert_eq!(manifest["reports"][0]["verdict_class"], "fail");
    let page = fs::read_to_string(out.join("index.html")).unwrap();
    assert!(page.contains("full &lt;label&gt;"));
    assert!(!page.contains("full <label>"));
    assert!(page.contains("source-text-000.png"));
    assert!(page.contains("Declared subject projections"));
    let png = image::open(out.join("derivative-001.png")).unwrap();
    assert_eq!([png.width(), png.height()], [80, 60]);
    fs::write(out.join("contact-sheet.png"), b"changed").unwrap();
    assert!(!saccade_core::manifest::verify(&out).unwrap().is_empty());
}
#[test]
fn empty_face_detection_abstains_and_known_faces_protect_crops() {
    let root = tempfile::tempdir().unwrap();
    let mut d = setup(root.path());
    d["subjects"] = json!([]);
    receipt(root.path(), json!([]), None);
    let (exit, report) = run(root.path(), &d, "empty", Some("inputs/faces.json"));
    assert_eq!(exit, 4);
    schema(&report);
    assert_eq!(report["faces"]["state"], "not_established");
    assert_eq!(report["rows"][0]["crop_safety"]["state"], "not_established");
    receipt(
        root.path(),
        json!([{"bbox":{"x":10.,"y":20.,"width":30.,"height":40.},"score":0.9,"landmarks":[]}]),
        None,
    );
    d["derivatives"] = json!([{"id":"face cut","display_size":[200,240],"crop_px":[20,0,200,240]}]);
    let (exit, report) = run(root.path(), &d, "face-cut", Some("inputs/faces.json"));
    assert_eq!(exit, 1);
    schema(&report);
    assert_eq!(report["faces"]["report"]["provenance"]["runtime"], "replay");
    assert_eq!(
        report["rows"][0]["subject_preservation"]["subjects"][0]["origin"],
        "detected_face"
    );
    assert_eq!(report["rows"][0]["crop_safety"]["state"], "unsafe");
    receipt(root.path(), json!([]), Some(&"0".repeat(64)));
    let (exit, _) = run(root.path(), &d, "stale", Some("inputs/faces.json"));
    assert_eq!(exit, 2);
    assert!(!root.path().join("stale").exists());
}
#[test]
fn delivered_missing_renditions_are_retained_and_content_claims_abstain() {
    let root = tempfile::tempdir().unwrap();
    let mut d = setup(root.path());
    d["derivatives"] = json!([{"id":"delivered","display_size":[320,240],"path":"source.png"},{"id":"missing","display_size":[80,60],"path":"absent.png"}]);
    let (exit, report) = run(root.path(), &d, "review", None);
    assert_eq!(exit, 1);
    schema(&report);
    assert_eq!(report["rows"].as_array().unwrap().len(), 2);
    assert_eq!(
        report["rows"][0]["subject_preservation"]["state"],
        "not_established"
    );
    assert_eq!(report["rows"][0]["text_legibility"]["state"], "legible");
    assert_eq!(report["rows"][0]["rendition_size"], json!([320, 240]));
    assert!(report["rows"][0]["rendition_sha256"].is_string());
    assert_eq!(report["rows"][1]["verdict"], "fail");
    assert!(report["rows"][1]["preview"].is_null());
}
#[test]
fn malformed_bounds_and_output_reuse_refuse_before_writes() {
    let root = tempfile::tempdir().unwrap();
    let mut d = setup(root.path());
    d["derivatives"][0]["crop_px"] = json!([u32::MAX, 0, 30, 40]);
    let (exit, _) = run(root.path(), &d, "invalid", None);
    assert_eq!(exit, 2);
    assert!(!root.path().join("invalid").exists());
    d["derivatives"][0]["crop_px"] = Value::Null;
    d["derivatives"][0]["display_size"] = json!([4097, 1]);
    assert_eq!(run(root.path(), &d, "oversized", None).0, 2);
    d["derivatives"][0]["display_size"] = json!([320, 240]);
    assert_eq!(run(root.path(), &d, "review", None).0, 0);
    let report = fs::read(root.path().join("review/saccade-derivative-sheet.v1.json")).unwrap();
    assert_eq!(run(root.path(), &d, "review", None).0, 2);
    assert_eq!(
        fs::read(root.path().join("review/saccade-derivative-sheet.v1.json")).unwrap(),
        report
    );
}
