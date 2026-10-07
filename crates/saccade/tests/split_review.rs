//! CLI split-review exit, schema and output containment contracts.
#![allow(clippy::unwrap_used, missing_docs)]
use serde_json::{Value, json};
use std::{path::Path, process::Command};
fn run(manifest: &Path, out: &Path, extra: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_saccade"))
        .arg("split-review")
        .arg(manifest)
        .arg("--out")
        .arg(out)
        .arg("--json")
        .args(extra)
        .output()
        .unwrap()
}
fn validate(value: &Value) {
    let id = value["schema"].as_str().unwrap();
    let schema: Value =
        serde_json::from_str(saccade_core::schema_catalog::get(id).unwrap()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(
        validator.is_valid(value),
        "{:?}",
        validator
            .iter_errors(value)
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
    );
}
#[test]
fn review_and_clean_results_validate_successor_schema_and_keep_limits() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("dataset");
    std::fs::create_dir(&data).unwrap();
    let im = image::RgbaImage::from_fn(48, 48, |x, y| {
        image::Rgba([(x * 3) as u8, (y * 5) as u8, 30, 255])
    });
    im.save(data.join("a.png")).unwrap();
    std::fs::copy(data.join("a.png"), data.join("b.png")).unwrap();
    let manifest = data.join("splits.json");
    let mut m = json!({"schema":"saccade-split-manifest.v1","entries":[{"path":"a.png","split":"train"},{"path":"b.png","split":"test"}],"injected_pairs":[{"a":"a.png","b":"b.png","transform":"exact"}]});
    validate(&m);
    std::fs::write(&manifest, m.to_string()).unwrap();
    let out = dir.path().join("review");
    let result = run(&manifest, &out, &[]);
    assert_eq!(
        result.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    validate(&report);
    assert_eq!(report["injected_recall"]["recall"], 1.);
    assert_eq!(
        report["cross_split_pairs"][0]["evidence"][0]["basis"],
        "encoded_sha256_equality"
    );
    let persisted: Value =
        serde_json::from_slice(&std::fs::read(out.join("saccade-split-review.v1.json")).unwrap())
            .unwrap();
    assert_eq!(persisted, report);
    assert!(out.join("index.html").is_file());
    assert!(out.join("pairs.csv").is_file());
    assert_eq!(run(&manifest, &out, &[]).status.code(), Some(2));
    assert_eq!(
        run(&manifest, &data.join("review"), &[]).status.code(),
        Some(2)
    );
    m["entries"][1]["split"] = json!("train");
    m["injected_pairs"] = json!([]);
    std::fs::write(&manifest, m.to_string()).unwrap();
    let result = run(&manifest, &dir.path().join("burst"), &[]);
    assert_eq!(result.status.code(), Some(0));
    let clean: Value = serde_json::from_slice(&result.stdout).unwrap();
    validate(&clean);
    assert_eq!(clean["verdict"], "no_candidates");
    assert!(
        clean["summary"]
            .as_str()
            .unwrap()
            .contains("not proof of no leakage")
    );
    assert_eq!(clean["groups"][0]["members"].as_array().unwrap().len(), 2);
}
#[test]
fn missing_input_is_a_typed_error() {
    let dir = tempfile::tempdir().unwrap();
    let result = run(
        &dir.path().join("missing.json"),
        &dir.path().join("out"),
        &[],
    );
    assert_eq!(result.status.code(), Some(2));
    let error: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        saccade_core::report_links::original_schema(error["schema"].as_str().unwrap()),
        "saccade-result.v2"
    );
}
#[cfg(not(feature = "embeddings"))]
#[test]
fn requested_unavailable_route_fails_without_clean_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    std::fs::create_dir(&data).unwrap();
    image::RgbaImage::from_pixel(8, 8, image::Rgba([80, 90, 100, 255]))
        .save(data.join("a.png"))
        .unwrap();
    let manifest = data.join("split.json");
    std::fs::write(
        &manifest,
        json!({"schema":"saccade-split-manifest.v1","entries":[{"path":"a.png","split":"train"}]})
            .to_string(),
    )
    .unwrap();
    let result = run(&manifest, &dir.path().join("out"), &["--embeddings"]);
    assert_eq!(result.status.code(), Some(2));
    let error: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(error["errors"][0]["code"], "feature_unavailable");
}
