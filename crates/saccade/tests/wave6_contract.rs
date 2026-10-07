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
    assert_eq!(
        summary["schema"],
        saccade_core::report_links::linked_schema("saccade-general-result.v1")
    );
    let report: Value =
        serde_json::from_slice(&std::fs::read(out.join("saccade-registration.v1.json")).unwrap())
            .unwrap();
    for (name, value) in [
        ("saccade-general-result.v1", summary),
        ("saccade-registration.v1", report.clone()),
    ] {
        let successor = saccade_core::report_links::linked_schema(name);
        let name = if value["schema"] == successor {
            successor
        } else {
            name
        };
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
    let routed_out = temp.path().join("routed-text");
    let routed = cli(&[
        "compare",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        "--question",
        "same-text",
        "--reference-source",
        sa.to_str().unwrap(),
        "--capture-source",
        sb.to_str().unwrap(),
        "--out",
        routed_out.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(routed.status.code(), Some(1), "{routed:?}");
    let routed_report: Value =
        serde_json::from_slice(&std::fs::read(routed_out.join("saccade-text.v1.json")).unwrap())
            .unwrap();
    validate_schema("saccade-text.v1", &routed_report);
    assert_eq!(routed_report["comparison"]["rates"]["cer"], 0.25);
    assert_eq!(routed_report["pipeline_choice"]["family"], "text");
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
        let successor = saccade_core::report_links::linked_schema(name);
        let name = if value["schema"] == successor {
            successor
        } else {
            name
        };
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
fn accent_fixture(path: &std::path::Path) {
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
    image.save(path).unwrap();
}
#[cfg(feature = "ocr")]
#[test]
#[ignore = "heavy: ocr-accents"]
fn pinned_ocr_reads_generated_accent_glyphs() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("accent.png");
    accent_fixture(&path);
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
                .join("../saccade-core/schemas/saccade-assess.v2.schema.json"),
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
#[ignore = "heavy: documents"]
fn generated_svg_pdf_inputs_require_real_rendering_and_page_summary() {
    let temp = tempfile::tempdir().unwrap();
    let svg = temp.path().join("vector.svg");
    std::fs::write(&svg,b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"100\" height=\"100\"><rect width=\"100\" height=\"100\" fill=\"white\"/><rect x=\"20\" y=\"20\" width=\"60\" height=\"60\" fill=\"black\"/></svg>").unwrap();
    // Generated public-domain-style primitive content; fixture code is project licensed.
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R >>",
        "<< /Length 17 >>\nstream\n20 20 60 60 re\nf\nendstream",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R >>",
    ];
    let mut bytes = b"%PDF-1.4\n".to_vec();
    let mut offsets = vec![0];
    for (i, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", i + 1, object).as_bytes());
    }
    let xref = bytes.len();
    bytes.extend_from_slice(b"xref\n0 6\n0000000000 65535 f \n");
    for offset in offsets.iter().skip(1) {
        bytes.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    bytes.extend_from_slice(
        format!("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    let pdf = temp.path().join("document.pdf");
    std::fs::write(&pdf, bytes).unwrap();
    let map = temp.path().join("identity-map.json");
    let pdf_hash = saccade_core::localized::digest(&std::fs::read(&pdf).unwrap());
    std::fs::write(&map, serde_json::to_vec(&serde_json::json!({"schema":"saccade-page-map.v1","reference_sha256":pdf_hash,"candidate_sha256":pdf_hash,"pairs":[{"reference":1,"candidate":1},{"reference":2,"candidate":2}]})).unwrap()).unwrap();
    for file in [&svg, &pdf] {
        let out = temp.path().join(file.extension().unwrap());
        let mut args = vec![
            "compare",
            file.to_str().unwrap(),
            file.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--json",
        ];
        if file == &pdf {
            args.extend(["--page-map", map.to_str().unwrap()]);
        }
        let result = cli(&args);
        assert!(
            result.status.success(),
            "real document rendering required: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let value: Value =
            serde_json::from_slice(&std::fs::read(out.join("saccade-documents.v3.json")).unwrap())
                .unwrap();
        validate_schema("saccade-documents.v3", &value);
        assert_eq!(value["counts"]["total"], if file == &pdf { 2 } else { 1 });
        assert_eq!(value["counts"]["failures"], 0);
        assert_eq!(value["rendering"]["dpi"], 96.);
        let image = image::open(out.join("page-0001/pair-000000/reference.png"))
            .unwrap()
            .to_rgba8();
        assert_eq!(
            image.get_pixel(image.width() / 2, image.height() / 2).0,
            [0, 0, 0, 255]
        );
    }
    let missing_map = temp.path().join("missing-map.json");
    let svg_hash = saccade_core::localized::digest(&std::fs::read(&svg).unwrap());
    std::fs::write(&missing_map,serde_json::to_vec(&serde_json::json!({"schema":"saccade-page-map.v1","reference_sha256":pdf_hash,"candidate_sha256":svg_hash,"pairs":[{"reference":1,"candidate":1},{"reference":2,"candidate":null}]})).unwrap()).unwrap();
    let out = temp.path().join("missing-page");
    let result = cli(&[
        "compare",
        pdf.to_str().unwrap(),
        svg.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--json",
        "--page-map",
        missing_map.to_str().unwrap(),
    ]);
    assert_eq!(result.status.code(), Some(1));
    let value: Value =
        serde_json::from_slice(&std::fs::read(out.join("saccade-documents.v3.json")).unwrap())
            .unwrap();
    assert_eq!(value["pages"][1]["status"], "missing");
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
                .join("../saccade-core/schemas/saccade-inspect-image.v2.schema.json"),
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

fn validate_schema(name: &str, value: &Value) {
    let successor = saccade_core::report_links::linked_schema(name);
    let name = if value["schema"] == successor {
        successor
    } else {
        name
    };
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../saccade-core/schemas/{name}.schema.json"));
    let schema: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(value)
        .unwrap();
}

#[test]
#[ignore = "heavy: wave6-cli"]
fn catalogue_and_questions_record_the_selected_family() {
    let result = cli(&["capabilities", "--json"]);
    assert!(result.status.success());
    let catalogue: Value = serde_json::from_slice(&result.stdout).unwrap();
    validate_schema("saccade-capabilities.v1", &catalogue);
    for name in [
        "hdr",
        "video_temporal",
        "embeddings",
        "documents",
        "ai_assist",
    ] {
        assert!(
            catalogue["families"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["family"] == name)
        );
    }
    let temp = tempfile::tempdir().unwrap();
    let a = temp.path().join("a.png");
    let b = temp.path().join("b.png");
    image::RgbaImage::from_fn(64, 64, |x, y| {
        image::Rgba([(x * 3) as u8, (y * 3) as u8, 80, 255])
    })
    .save(&a)
    .unwrap();
    std::fs::copy(&a, &b).unwrap();
    for (question, schema, verdict) in [
        ("near-duplicate", "saccade-near-duplicate.v1", "pass"),
        ("quality", "saccade-question-report.v1", "unknown"),
    ] {
        let out = temp.path().join(question);
        let result = cli(&[
            "compare",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
            "--question",
            question,
            "--out",
            out.to_str().unwrap(),
            "--json",
        ]);
        assert!(result.status.success(), "{result:?}");
        let evidence: Value =
            serde_json::from_slice(&std::fs::read(out.join(format!("{schema}.json"))).unwrap())
                .unwrap();
        validate_schema(schema, &evidence);
        assert_eq!(evidence["verdict"], verdict);
        assert_eq!(evidence["pipeline_choice"]["question"], question);
        assert_eq!(evidence["pipeline_choice"]["fallback"], "none");
    }
    let out = temp.path().join("render");
    let result = cli(&[
        "compare",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        "--question",
        "same-render",
        "--out",
        out.to_str().unwrap(),
        "--json",
    ]);
    assert!(result.status.success(), "{result:?}");
    let choice: Value = serde_json::from_slice(
        &std::fs::read(out.join("saccade-pipeline-choice.v1.json")).unwrap(),
    )
    .unwrap();
    validate_schema("saccade-pipeline-choice.v1", &choice);
    let bytes = std::fs::read(out.join("saccade-report.v1.json")).unwrap();
    assert_eq!(
        choice["measurement"]["sha256"],
        saccade_core::localized::digest(&bytes)
    );
    // An ordinary rerun removes the stale explicit-choice component.
    let result = cli(&[
        "compare",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--json",
    ]);
    assert!(result.status.success(), "{result:?}");
    assert!(!out.join("saccade-pipeline-choice.v1.json").exists());
}

#[test]
#[ignore = "heavy: wave6-cli"]
fn routed_failures_never_drop_pairs_or_change_questions() {
    let temp = tempfile::tempdir().unwrap();
    let a = temp.path().join("a");
    let b = temp.path().join("b");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    image::RgbImage::from_pixel(16, 16, image::Rgb([120; 3]))
        .save(a.join("missing.png"))
        .unwrap();
    std::fs::write(a.join("broken.png"), b"invalid").unwrap();
    std::fs::write(b.join("broken.png"), b"invalid").unwrap();
    let out = temp.path().join("out");
    let result = cli(&[
        "compare",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        "--question",
        "near-duplicate",
        "--out",
        out.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    let evidence: Value =
        serde_json::from_slice(&std::fs::read(out.join("saccade-near-duplicate.v1.json")).unwrap())
            .unwrap();
    assert_eq!(evidence["counts"]["total"], 2);
    assert_eq!(evidence["counts"]["failures"], 2);
    assert_eq!(evidence["counts"]["errors"], 1);
    for question in ["same-content", "same-text"] {
        let result = cli(&[
            "compare",
            a.join("missing.png").to_str().unwrap(),
            a.join("missing.png").to_str().unwrap(),
            "--question",
            question,
            "--out",
            temp.path().join(question).to_str().unwrap(),
            "--json",
        ]);
        assert_eq!(result.status.code(), Some(2), "{result:?}");
        assert!(
            !temp
                .path()
                .join(question)
                .join("saccade-report.v1.json")
                .exists()
        );
    }
    let result = cli(&[
        "compare",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        "--question",
        "quality",
        "--align",
        "none",
        "--json",
    ]);
    assert_eq!(result.status.code(), Some(2));
}

#[cfg(feature = "mcp")]
#[test]
#[ignore = "heavy: wave6-mcp"]
fn mcp_question_inputs_and_native_execution_keep_authority_boundaries() {
    use std::io::Write;
    use std::process::Stdio;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let out = temp.path().join("out");
    let config = temp.path().join("operator-config");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&out).unwrap();
    std::fs::create_dir(&config).unwrap();
    image::RgbImage::from_pixel(16, 16, image::Rgb([100; 3]))
        .save(root.join("image.png"))
        .unwrap();
    let outside = temp.path().join("outside.png");
    std::fs::copy(root.join("image.png"), &outside).unwrap();
    std::fs::write(root.join("runtime.so"), b"untrusted native library").unwrap();
    std::fs::write(root.join("model.json"), b"{}").unwrap();
    std::fs::create_dir(root.join("cache")).unwrap();
    let arguments = [
        serde_json::json!({"operation":"capabilities"}),
        serde_json::json!({"operation":"compare_question","question":"near-duplicate","reference":outside,"capture":root.join("image.png"),"out":out.join("escaped")}),
        serde_json::json!({"operation":"compare_question","question":"same-content","reference":root.join("image.png"),"capture":root.join("image.png"),"model":root.join("model.json"),"cache":root.join("cache"),"library":root.join("runtime.so"),"out":out.join("native")}),
    ];
    let mut child = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .arg("mcp")
        .arg("--root")
        .arg(&root)
        .arg("--out-root")
        .arg(&out)
        .env("XDG_CONFIG_HOME", &config)
        // The cache is operator configuration; the request below only restates it.
        .env("SACCADE_MODELS_DIR", root.join("cache"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for (id, args) in arguments.iter().enumerate() {
        writeln!(stdin,"{}",serde_json::json!({"jsonrpc":"2.0","id":id+1,"method":"tools/call","params":{"name":"saccade_general","arguments":args}})).unwrap();
    }
    drop(stdin);
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success(), "{result:?}");
    let values: Vec<Value> = String::from_utf8(result.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(values.len(), 3);
    assert_ne!(values[0]["result"]["isError"], true);
    assert_eq!(values[1]["result"]["isError"], true);
    assert_eq!(values[2]["result"]["isError"], true);
    assert!(
        values[2]
            .to_string()
            .contains("model_location_not_request_controlled")
    );
    assert!(!out.join("native").join("saccade-similar.v1.json").exists());
}

#[cfg(feature = "ocr")]
#[test]
#[ignore = "heavy: rust-ocr-accents"]
fn paddle_ocr_recognizes_generated_accents_with_observed_confidence() {
    let temp = tempfile::tempdir().unwrap();
    let root = std::path::PathBuf::from(
        std::env::var_os("SACCADE_OCR_ACCENTS").expect("generated OCR fixture directory"),
    );
    let path = root.join("fr-DejaVuSans-40.png");
    let contract =
        std::env::var_os("SACCADE_W6_RUST_OCR_CONTRACT").expect("pinned PP-OCRv5 contract/cache");
    let out = temp.path().join("paddle-text");
    let result = cli(&[
        "text",
        path.to_str().unwrap(),
        path.to_str().unwrap(),
        "--ocr-contract",
        std::path::Path::new(&contract).to_str().unwrap(),
        "--expect-text",
        "café",
        "--out",
        out.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(
        result.status.code(),
        Some(0),
        "real accent/readability contract: {result:?}"
    );
    let value: Value =
        serde_json::from_slice(&std::fs::read(out.join("saccade-text.v1.json")).unwrap()).unwrap();
    validate_schema("saccade-text.v1", &value);
    assert_eq!(value["comparison"]["expected"][0]["present"], true);
    assert_eq!(value["comparison"]["expected"][0]["readable"], true);
    assert_eq!(value["comparison"]["rates"]["character_edits"], 0);
    assert_eq!(value["producers"][0]["engine"], "PP-OCRv5-mobile/Latin");
}

#[cfg(feature = "mcp")]
#[test]
#[ignore = "heavy: documents-mcp"]
fn mcp_documents_preserve_roots_and_page_summary() {
    use std::io::Write;
    use std::process::Stdio;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("inputs");
    let out = temp.path().join("out");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&out).unwrap();
    let svg = root.join("vector.svg");
    std::fs::write(&svg, br#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="32" height="32" fill="red"/></svg>"#).unwrap();
    let outside = temp.path().join("outside.svg");
    std::fs::copy(&svg, &outside).unwrap();
    let arguments = [
        serde_json::json!({"operation":"documents_compare","reference":svg,"capture":svg,"dpi":96,"out":out.join("pages")}),
        serde_json::json!({"operation":"documents_compare","reference":outside,"capture":svg,"out":out.join("escaped")}),
    ];
    let mut child = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .arg("mcp")
        .arg("--root")
        .arg(&root)
        .arg("--out-root")
        .arg(&out)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for (id, args) in arguments.iter().enumerate() {
        writeln!(stdin,"{}",serde_json::json!({"jsonrpc":"2.0","id":id+1,"method":"tools/call","params":{"name":"saccade_general","arguments":args}})).unwrap();
    }
    drop(stdin);
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success(), "{result:?}");
    let values: Vec<Value> = String::from_utf8(result.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(values.len(), 2);
    assert_ne!(values[0]["result"]["isError"], true);
    assert_eq!(values[1]["result"]["isError"], true);
    let report: Value = serde_json::from_slice(
        &std::fs::read(out.join("pages/saccade-documents.v3.json")).unwrap(),
    )
    .unwrap();
    validate_schema("saccade-documents.v3", &report);
    assert_eq!(report["verdict"], "pass");
    assert!(!out.join("escaped/saccade-documents.v3.json").exists());
}
