//! Sensitivity transport, generated proofs and fail-closed eligibility.
#![allow(clippy::unwrap_used)]
use serde_json::{Value, json};
use std::{path::Path, process::Command};
fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_saccade"))
}
fn validate(report: &Value) {
    let schema: Value = serde_json::from_str(
        saccade_core::schema_catalog::get(report["schema"].as_str().unwrap()).unwrap(),
    )
    .unwrap();
    assert!(jsonschema::validator_for(&schema).unwrap().is_valid(report));
}
#[test]
fn two_generated_sets_prove_six_classes_under_frozen_before_after_policies() {
    let temp = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let result = Command::new("python3")
        .arg(root.join("scripts/sensitivity/fixtures.py"))
        .arg("--out")
        .arg(temp.path().join("inputs"))
        .args(["--binary", env!("CARGO_BIN_EXE_saccade"), "--receipts"])
        .arg(temp.path().join("receipts"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    for set in ["set-a", "set-b"] {
        validate(
            &serde_json::from_slice::<Value>(
                &std::fs::read(
                    temp.path()
                        .join("receipts")
                        .join(set)
                        .join("saccade-sensitivity.v2.json"),
                )
                .unwrap(),
            )
            .unwrap(),
        );
    }
}
#[test]
fn eligibility_errors_noops_selection_and_collisions_never_become_detections() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let base = root.join("base");
    std::fs::create_dir(&base).unwrap();
    let image = image::RgbaImage::from_pixel(32, 32, image::Rgba([100, 100, 100, 255]));
    image.save(base.join("a.png")).unwrap();
    let source = std::fs::read(base.join("a.png")).unwrap();
    let index_attempt = binary()
        .arg("sensitivity")
        .arg(&base)
        .args([
            "--catalogue",
            "unused.json",
            "--config",
            "unused.toml",
            "--out",
        ])
        .arg(root.join("index-attempt"))
        .arg("--report-index")
        .arg(base.join("a.png"))
        .arg("--json")
        .output()
        .unwrap();
    assert_eq!(index_attempt.status.code(), Some(2));
    assert!(!root.join("index-attempt").exists());

    let catalogue = root.join("catalogue.json");
    let config = root.join("policy.toml");
    let run = |out: &Path| {
        binary()
            .arg("sensitivity")
            .arg(&base)
            .arg("--catalogue")
            .arg(&catalogue)
            .arg("--config")
            .arg(&config)
            .arg("--out")
            .arg(out)
            .arg("--json")
            .output()
            .unwrap()
    };
    let write = |defect: Value, policy: &str| {
        std::fs::write(&catalogue,serde_json::to_vec(&json!({"schema":"saccade-sensitivity-catalogue.v1","provenance":"procedural MIT","injections":[{"id":"one","defect":defect,"magnitudes":[1]}]})).unwrap()).unwrap();
        std::fs::write(&config, policy).unwrap();
    };
    write(json!({"class":"blur"}), "threshold=0.0\n");
    let result = run(&root.join("noop"));
    assert_eq!(result.status.code(), Some(4));
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    validate(&report);
    assert_eq!(report["summaries"][0]["counts"]["ineffective"], 1);
    assert!(report["summaries"][0]["miss_rate"].is_null());
    let local = json!({"class":"local_edit","rect":[0.2,0.2,0.2,0.2],"colour":[240,0,0]});
    write(local.clone(), "threshold=0.0\nignore=['a.png']\n");
    let result = run(&root.join("excluded"));
    assert_eq!(result.status.code(), Some(4));
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["summaries"][0]["counts"]["excluded"], 1);
    write(local.clone(), "threshold=-0.1\n");
    let result = run(&root.join("bad-control"));
    assert_eq!(result.status.code(), Some(4));
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["summaries"][0]["counts"]["unavailable"], 1);
    assert_eq!(report["summaries"][0]["counts"]["detected"], 0);
    write(local, "threshold=1.0\nhotspot_fail=0.4\n");
    let result = run(&root.join("guard"));
    assert_eq!(result.status.code(), Some(0));
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["summaries"][0]["counts"]["detected"], 1);
    assert_eq!(run(&base).status.code(), Some(2));
    assert_eq!(run(&root.join("guard")).status.code(), Some(2));
    std::fs::write(
        &catalogue,
        b"{\"schema\":\"future\",\"provenance\":\"MIT\",\"injections\":[]}",
    )
    .unwrap();
    assert_eq!(run(&root.join("invalid")).status.code(), Some(2));
    assert!(!root.join("invalid").exists());
    write(
        json!({"class":"glyph_edit","before":"base/a.png","after":"base/a.png","before_sha256":"0".repeat(64),"after_sha256":"1".repeat(64),"origin":[0.0,0.0]}),
        "threshold=0.0\n",
    );
    assert_eq!(run(&root.join("pin-mismatch")).status.code(), Some(2));
    assert!(!root.join("pin-mismatch").exists());
    #[cfg(unix)]
    {
        image.save(base.join("..\\escape.png")).unwrap();
        write(json!({"class":"blur"}), "threshold=0.0\n");
        assert_eq!(run(&root.join("unsafe-name")).status.code(), Some(2));
        assert!(!root.join("unsafe-name").exists());
    }
    #[cfg(unix)]
    std::fs::remove_file(base.join("..\\escape.png")).unwrap();
    std::fs::write(base.join("b.hdr"), b"invalid native input").unwrap();
    assert_eq!(run(&root.join("native-input")).status.code(), Some(2));
    assert!(!root.join("native-input").exists());
    assert_eq!(std::fs::read(base.join("a.png")).unwrap(), source);
}
#[test]
fn jpeg_names_keep_overrides_and_masks_without_reencoding_noise() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let base = root.join("base");
    std::fs::create_dir(&base).unwrap();
    image::RgbImage::from_pixel(32, 32, image::Rgb([100, 100, 100]))
        .save(base.join("sample.jpg"))
        .unwrap();
    let catalogue = root.join("catalogue.json");
    std::fs::write(&catalogue,serde_json::to_vec(&json!({"schema":"saccade-sensitivity-catalogue.v1","provenance":"procedural MIT","injections":[{"id":"local","defect":{"class":"local_edit","rect":[0.2,0.2,0.2,0.2],"colour":[250,0,0]},"magnitudes":[1]}]})).unwrap()).unwrap();
    let config = root.join("policy.toml");
    std::fs::write(
        &config,
        "threshold=1.0\n[[override]]\nglob='*.jpg'\nthreshold=0.0\nmetric='max'\n",
    )
    .unwrap();
    let run = |out: &str| {
        binary()
            .arg("sensitivity")
            .arg(&base)
            .arg("--catalogue")
            .arg(&catalogue)
            .arg("--config")
            .arg(&config)
            .arg("--out")
            .arg(root.join(out))
            .arg("--json")
            .output()
            .unwrap()
    };
    let result = run("override");
    assert_eq!(result.status.code(), Some(0));
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["trials"][0]["entry"], "sample.jpg");
    let compare: Value = serde_json::from_slice(
        &std::fs::read(
            root.join("override").join(
                report["trials"][0]["outcomes"][0]["reports"][1]
                    .as_str()
                    .unwrap(),
            ),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(compare["entries"][0]["threshold"], 0.0);
    assert_eq!(compare["entries"][0]["metric_used"], "max");
    std::fs::write(
        &config,
        "threshold=0.0\nmask_mode='neutralize'\n[[mask]]\nglob='*.jpg'\nrect=[0.1,0.1,0.4,0.4]\n",
    )
    .unwrap();
    let result = run("masked");
    assert_eq!(result.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["summaries"][0]["counts"]["missed"], 1);
}
