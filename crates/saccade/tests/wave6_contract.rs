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
        let schema: Value = serde_json::from_slice(
            &std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("../saccade-core/schemas/{name}.schema.json")),
            )
            .unwrap(),
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

#[test]
#[ignore = "heavy: wave6-cli"]
fn assessment_reports_paired_deltas_without_quality_verdict() {
    let temp = tempfile::tempdir().unwrap();
    let a = temp.path().join("before.png");
    let b = temp.path().join("after.png");
    let sharp = image::RgbaImage::from_fn(64, 64, |x, _| {
        image::Rgba(if x < 32 { [0, 0, 0, 255] } else { [255; 4] })
    });
    sharp.save(&a).unwrap();
    image::imageops::blur(&sharp, 2.).save(&b).unwrap();
    let out = temp.path().join("quality");
    let result = cli(&[
        "assess",
        b.to_str().unwrap(),
        "--compare-to",
        a.to_str().unwrap(),
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
        serde_json::from_slice(&std::fs::read(out.join("saccade-assess.v1.json")).unwrap())
            .unwrap();
    assert_eq!(report["verdict"], "unknown");
    assert!(report["deltas"]["laplacian_variance"].as_f64().unwrap() < 0.);
    let schema: Value = serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../saccade-core/schemas/saccade-assess.v1.schema.json"),
        )
        .unwrap(),
    )
    .unwrap();
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(&report)
        .unwrap();
}

#[test]
#[ignore = "heavy: documents-deferred"]
fn generated_svg_pdf_inputs_require_real_rendering_and_page_summary() {
    let temp = tempfile::tempdir().unwrap();
    let svg = temp.path().join("vector.svg");
    std::fs::write(&svg,b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"100\" height=\"100\"><rect width=\"100\" height=\"100\" fill=\"white\"/><rect x=\"20\" y=\"20\" width=\"60\" height=\"60\" fill=\"black\"/></svg>").unwrap();
    // Generated public-domain-style primitive content; fixture code is project licensed.
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R >>",
        "<< /Length 17 >>\nstream\n20 20 60 60 re\nf\nendstream",
    ];
    let mut bytes = b"%PDF-1.4\n".to_vec();
    let mut offsets = vec![0];
    for (i, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", i + 1, object).as_bytes());
    }
    let xref = bytes.len();
    bytes.extend_from_slice(b"xref\n0 5\n0000000000 65535 f \n");
    for offset in offsets.iter().skip(1) {
        bytes.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    bytes.extend_from_slice(
        format!("trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    let pdf = temp.path().join("document.pdf");
    std::fs::write(&pdf, bytes).unwrap();
    for file in [&svg, &pdf] {
        let out = temp.path().join(file.extension().unwrap());
        let result = cli(&[
            "compare",
            file.to_str().unwrap(),
            file.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--json",
        ]);
        assert!(
            result.status.success(),
            "deferred: real document adapter/routing required: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[test]
#[ignore = "heavy: wave6-cli"]
fn inspection_never_infers_generation_and_has_weak_visual_layer() {
    let temp = tempfile::tempdir().unwrap();
    let image = temp.path().join("input.jpg");
    image::RgbImage::from_fn(64, 64, |x, y| {
        image::Rgb([(x * 3) as u8, (y * 3) as u8, 100])
    })
    .save(&image)
    .unwrap();
    let out = temp.path().join("inspection");
    let result = cli(&[
        "inspect-image",
        image.to_str().unwrap(),
        "--output-size",
        "128x128",
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
        serde_json::from_slice(&std::fs::read(out.join("saccade-inspect-image.v1.json")).unwrap())
            .unwrap();
    assert_eq!(report["ai_generation"], "unknown");
    assert_eq!(report["verdict"], "unknown");
    assert_eq!(
        report["publication"]["outputs"][0]["requires_upsampling"],
        true
    );
    assert_eq!(
        report["error_level_analysis"]["assurance"],
        "weak visual layer only"
    );
    assert!(out.join("error-level-analysis.png").is_file());
    assert!(
        std::fs::read_to_string(out.join("index.html"))
            .unwrap()
            .contains("weak visual evidence")
    );
    let schema: Value = serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../saccade-core/schemas/saccade-inspect-image.v1.schema.json"),
        )
        .unwrap(),
    )
    .unwrap();
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(&report)
        .unwrap();
}
#[cfg(feature = "embeddings")]
#[test]
#[ignore = "heavy: embeddings-index"]
fn supplied_embedding_index_build_and_query_preserve_pins() {
    let temp = tempfile::tempdir().unwrap();
    let images = temp.path().join("images");
    std::fs::create_dir(&images).unwrap();
    let a = images.join("a.png");
    let b = images.join("b.png");
    let image = image::RgbaImage::from_fn(64, 64, |x, y| {
        image::Rgba([(x * 3) as u8, (y * 3) as u8, 100, 255])
    });
    image.save(&a).unwrap();
    image::imageops::flip_horizontal(&image).save(&b).unwrap();
    let model = std::env::var("SACCADE_W6_EMBEDDING_MODEL").expect("model");
    let cache = std::env::var("SACCADE_W6_MODEL_CACHE").expect("cache");
    let library = std::env::var("SACCADE_W6_ORT_LIBRARY").expect("runtime");
    let index = temp.path().join("index");
    let result = cli(&[
        "index",
        "build",
        images.to_str().unwrap(),
        "--model",
        &model,
        "--cache",
        &cache,
        "--library",
        &library,
        "--out",
        index.to_str().unwrap(),
        "--json",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let out = temp.path().join("matches");
    let result = cli(&[
        "index",
        "query",
        index.to_str().unwrap(),
        a.to_str().unwrap(),
        "--model",
        &model,
        "--cache",
        &cache,
        "--library",
        &library,
        "--out",
        out.to_str().unwrap(),
        "--json",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: Value = serde_json::from_slice(
        &std::fs::read(out.join("saccade-embedding-query.v1.json")).unwrap(),
    )
    .unwrap();
    assert!(report["results"][0]["cosine"].as_f64().unwrap() > 0.9999);
    std::fs::write(index.join("vectors.bin"), b"corrupt").unwrap();
    let rejected = cli(&[
        "index",
        "query",
        index.to_str().unwrap(),
        a.to_str().unwrap(),
        "--model",
        &model,
        "--cache",
        &cache,
        "--library",
        &library,
        "--out",
        temp.path().join("rejected").to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(rejected.status.code(), Some(2));
}
