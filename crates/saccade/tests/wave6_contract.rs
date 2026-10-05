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

#[test]
#[ignore = "heavy: wave6-cli"]
fn hash_dedupe_and_imported_accent_text_contracts() {
    let temp = tempfile::tempdir().unwrap();
    let images = temp.path().join("images");
    std::fs::create_dir(&images).unwrap();
    let a = images.join("a.png");
    let b = images.join("b.png");
    let image = image::RgbaImage::from_fn(80, 40, |x, y| {
        image::Rgba([(x * 2) as u8, (y * 4) as u8, 90, 255])
    });
    image.save(&a).unwrap();
    image.save(&b).unwrap();
    let out = temp.path().join("dedupe");
    let result = cli(&[
        "dedupe",
        images.to_str().unwrap(),
        "--threshold",
        "0",
        "--out",
        out.to_str().unwrap(),
        "--json",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: Value =
        serde_json::from_slice(&std::fs::read(out.join("saccade-dedupe.v1.json")).unwrap())
            .unwrap();
    assert_eq!(report["clusters"][0].as_array().unwrap().len(), 2);
    let make_source = |image: &std::path::Path, text: &str, path: &std::path::Path| {
        let bytes = std::fs::read(image).unwrap();
        let source = serde_json::json!({"schema":"saccade-ui-source.v1","capture_sha256":saccade_core::localized::digest(&bytes),"dimensions":[80,40],"kind":"tesseract_tsv","producer":{"fixture":"generated; project licence"},"complete":false,"nodes":[{"id":"word","text":text,"bounds":[5.,5.,40.,10.],"reading_order":null,"keyboard_order":null,"ocr_confidence":95.}]});
        std::fs::write(path, serde_json::to_vec(&source).unwrap()).unwrap();
    };
    let sa = temp.path().join("a-source.json");
    let sb = temp.path().join("b-source.json");
    make_source(&a, "café", &sa);
    make_source(&b, "cafe", &sb);
    let out = temp.path().join("text");
    let result = cli(&[
        "text",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        "--a-source",
        sa.to_str().unwrap(),
        "--b-source",
        sb.to_str().unwrap(),
        "--expect-text",
        "café",
        "--out",
        out.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(
        result.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: Value =
        serde_json::from_slice(&std::fs::read(out.join("saccade-text.v1.json")).unwrap()).unwrap();
    assert_eq!(report["comparison"]["rates"]["cer"], 0.25);
    for (name, value) in [
        (
            "saccade-dedupe.v1",
            serde_json::from_slice::<Value>(
                &std::fs::read(temp.path().join("dedupe/saccade-dedupe.v1.json")).unwrap(),
            )
            .unwrap(),
        ),
        ("saccade-text.v1", report),
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../saccade-core/schemas/{name}.schema.json"));
        let schema: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        jsonschema::validator_for(&schema)
            .unwrap()
            .validate(&value)
            .unwrap();
    }
}
#[cfg(feature = "ocr")]
#[test]
#[ignore = "heavy: ocr-accents"]
fn pinned_ocr_reads_generated_accent_glyphs() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("accent.png");
    let glyphs = [
        [14u8, 17, 16, 16, 16, 17, 14],
        [14, 17, 17, 31, 17, 17, 17],
        [31, 16, 16, 30, 16, 16, 16],
        [31, 16, 16, 30, 16, 16, 31],
    ];
    let mut image = image::RgbaImage::from_pixel(320, 140, image::Rgba([255, 255, 255, 255]));
    for (k, rows) in glyphs.iter().enumerate() {
        for (y, &bits) in rows.iter().enumerate() {
            for x in 0..5 {
                if bits & (1 << (4 - x)) != 0 {
                    for dy in 0..8 {
                        for dx in 0..8 {
                            image.put_pixel(
                                30 + k as u32 * 56 + x * 8 + dx,
                                45 + y as u32 * 8 + dy,
                                image::Rgba([0, 0, 0, 255]),
                            );
                        }
                    }
                }
            }
        }
    }
    // Acute accent above the E, authored fixture under project licence.
    for n in 0..3 {
        for dy in 0..6 {
            for dx in 0..6 {
                image.put_pixel(
                    30 + 3 * 56 + 16 + n * 6 + dx,
                    31 - n * 6 + dy,
                    image::Rgba([0, 0, 0, 255]),
                );
            }
        }
    }
    image.save(&path).unwrap();
    let contract = std::env::var_os("SACCADE_W6_OCR_CONTRACT")
        .expect("coordinator supplies pinned OCR contract");
    let out = temp.path().join("text");
    let result = cli(&[
        "text",
        path.to_str().unwrap(),
        path.to_str().unwrap(),
        "--ocr-contract",
        std::path::Path::new(&contract).to_str().unwrap(),
        "--expect-text",
        "CAFÉ",
        "--out",
        out.to_str().unwrap(),
        "--json",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
