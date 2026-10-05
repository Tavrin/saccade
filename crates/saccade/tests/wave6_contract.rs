//! Coordinator-run CLI/MCP registration and generated-fixture evidence gates.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]
use serde_json::Value;
use std::process::Command;
fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(args)
        .output()
        .expect("CLI")
}
#[test]
#[ignore = "heavy: wave6-cli"]
fn registration_report_has_schema_and_visible_geometry() {
    let temp = tempfile::tempdir().unwrap();
    let a = temp.path().join("a.png");
    let b = temp.path().join("b.png");
    let out = temp.path().join("report");
    let image = image::RgbaImage::from_fn(40, 30, |x, y| {
        image::Rgba([(x * 3) as u8, (y * 4) as u8, 90, 255])
    });
    image.save(&a).unwrap();
    image.save(&b).unwrap();
    let result = cli(&[
        "compare",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        "--align",
        "none",
        "--out",
        out.to_str().unwrap(),
        "--json",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let summary: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(summary["schema"], "saccade-general-result.v1");
    let report: Value =
        serde_json::from_slice(&std::fs::read(out.join("saccade-registration.v1.json")).unwrap())
            .unwrap();
    for (name, value) in [
        ("saccade-general-result.v1", summary),
        ("saccade-registration.v1", report.clone()),
    ] {
        let schema: Value = serde_json::from_str(
            &std::fs::read(format!("../saccade-core/schemas/{name}.schema.json")).unwrap(),
        )
        .unwrap();
        jsonschema::validator_for(&schema)
            .unwrap()
            .validate(&value)
            .unwrap();
    }
    assert_eq!(
        report["entries"][0]["registration"]["excluded_by_geometry"],
        0
    );
    assert!(out.join("pair-000000/geometry-inclusion.png").is_file());
    assert!(
        std::fs::read_to_string(out.join("index.html"))
            .unwrap()
            .contains("geometry-inclusion.png")
    );
    let rejected = cli(&[
        "compare",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        "--align",
        "none",
        "--intent",
        "preserve pixels",
        "--out",
        temp.path().join("reject").to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(rejected.status.code(), Some(2));
}
