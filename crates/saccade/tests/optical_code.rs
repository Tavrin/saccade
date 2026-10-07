//! Generated document/app optical assertions and emitted schema contracts.
#![allow(clippy::unwrap_used)]
use serde_json::Value;
use std::process::Command;
#[cfg(feature = "optical-code")]
#[path = "../../saccade-core/tests/support/optical.rs"]
mod fixtures;
fn run(path: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_saccade"))
        .arg("optical-code")
        .arg(path)
        .args(args)
        .arg("--json")
        .output()
        .unwrap()
}
#[test]
fn unavailable_and_missing_inputs_are_execution_errors() {
    let out = run(std::path::Path::new("missing-optical.png"), &[]);
    assert_eq!(out.status.code(), Some(2));
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["execution"], "error");
    if !cfg!(feature = "optical-code") {
        assert_eq!(value["errors"][0]["code"], "feature_unavailable");
    }
}
#[cfg(feature = "optical-code")]
#[test]
fn generated_app_truth_artifact_schema_and_pixel_identity_are_independent() {
    let tmp = tempfile::tempdir().unwrap();
    let schema: Value =
        serde_json::from_str(saccade_core::schema_catalog::get("saccade-optical-code.v1").unwrap())
            .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let mut damaged = fixtures::qr("https://example.org/item/7", 4, 0, 255);
    for y in 16 + 8 * 4..damaged.height() - 16 {
        for x in 16 + 8 * 4..damaged.width() - 16 {
            damaged.put_pixel(x, y, image::Rgba([255; 4]));
        }
    }
    for (name, code, expect, state, failure) in [
        (
            "good",
            fixtures::qr("https://example.org/item/7", 4, 0, 255),
            0,
            "decoded",
            None,
        ),
        (
            "wrong",
            fixtures::qr("https://example.org/item/8", 4, 0, 255),
            1,
            "decoded",
            Some("payload_mismatch"),
        ),
        (
            "small",
            fixtures::qr("https://example.org/item/7", 2, 0, 255),
            1,
            "decoded",
            Some("module_size_below_minimum"),
        ),
        (
            "low",
            fixtures::qr("https://example.org/item/7", 4, 20, 180),
            1,
            "decoded",
            Some("contrast_below_minimum"),
        ),
        (
            "damaged",
            damaged,
            1,
            "not_decodable",
            Some("not_decodable"),
        ),
    ] {
        let path = tmp.path().join(format!("{name}.png"));
        fixtures::scene(&code).save(&path).unwrap();
        let artifact = tmp.path().join(format!("report-{name}"));
        let out = run(
            &path,
            &[
                "--expect",
                "https://example.org/item/7",
                "--minimum-module-px",
                "3",
                "--minimum-contrast",
                "0.8",
                "--require-quiet-zone",
                "--out",
                artifact.to_str().unwrap(),
            ],
        );
        assert_eq!(
            out.status.code(),
            Some(expect),
            "{name}: {}",
            String::from_utf8_lossy(&out.stdout)
        );
        let value: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert!(validator.is_valid(&value), "{name}: {value}");
        assert_eq!(value["state"], state);
        if let Some(failure) = failure {
            assert!(
                value["failures"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v == failure)
            );
        }
        let stored: Value = serde_json::from_slice(
            &std::fs::read(artifact.join("saccade-optical-code.v1.json")).unwrap(),
        )
        .unwrap();
        assert!(validator.is_valid(&stored));
        assert_eq!(stored["payload"], value["payload"]);
        assert_eq!(stored["input_sha256"], value["input_sha256"]);
        if name == "damaged" {
            let identical = Command::new(env!("CARGO_BIN_EXE_saccade"))
                .arg("prove")
                .arg("identity")
                .arg(&path)
                .arg(&path)
                .arg("--out")
                .arg(tmp.path().join("identity"))
                .output()
                .unwrap();
            assert_eq!(
                identical.status.code(),
                Some(0),
                "{}",
                String::from_utf8_lossy(&identical.stderr)
            );
        }
    }
    let path = tmp.path().join("barcode.png");
    fixtures::barcode("Sample-123", rxing::BarcodeFormat::CODE_128)
        .save(&path)
        .unwrap();
    let out = run(&path, &["--symbology", "code128", "--expect", "Sample-123"]);
    assert_eq!(out.status.code(), Some(0));
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(validator.is_valid(&value));
    assert_eq!(value["symbology"], "code128");
}
#[cfg(all(feature = "optical-code", feature = "documents"))]
#[test]
fn final_pdf_page_render_requires_explicit_density_and_page() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("page.pdf");
    std::fs::write(
        &path,
        fixtures::pdf(&fixtures::qr("https://example.org/item/7", 4, 0, 255)),
    )
    .unwrap();
    assert_eq!(run(&path, &[]).status.code(), Some(2));
    assert_eq!(
        run(&path, &["--page", "0", "--dpi", "72"]).status.code(),
        Some(2)
    );
    let out = run(
        &path,
        &[
            "--page",
            "1",
            "--dpi",
            "72",
            "--expect",
            "https://example.org/item/7",
            "--minimum-module-px",
            "3",
            "--require-quiet-zone",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["page"], 1);
    assert_eq!(value["dpi"], 72.);
    assert_eq!(value["state"], "decoded");
    assert!(value["quality"]["module_size_px"].as_f64().unwrap() >= 3.9);
}
#[cfg(feature = "optical-code")]
#[test]
fn invalid_policy_and_no_overwrite_are_typed_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("code.png");
    fixtures::qr("Sample", 4, 0, 255).save(&path).unwrap();
    for args in [
        &["--expect-pattern", "["][..],
        &["--region", "4294967295,0,2,2"],
        &["--minimum-contrast", "2"],
        &["--minimum-module-px", "NaN"],
    ] {
        assert_eq!(run(&path, args).status.code(), Some(2));
    }
    let dir = tmp.path().join("existing");
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(dir.join("keep.txt"), "keep").unwrap();
    assert_eq!(
        run(&path, &["--out", dir.to_str().unwrap()]).status.code(),
        Some(2)
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("keep.txt")).unwrap(),
        "keep"
    );
}
