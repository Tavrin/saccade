//! Local OCR and optional provider CLI contracts. No live provider calls.
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[cfg(any(feature = "ocr", feature = "ocr-provider"))]
use serde_json::Value;
#[cfg(feature = "ocr-provider")]
use serde_json::json;
#[cfg(feature = "ocr-provider")]
use std::path::Path;
use std::process::{Command, Output};
fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(args)
        .output()
        .unwrap()
}
#[cfg(feature = "ocr-provider")]
fn png(path: &Path) {
    image::RgbImage::from_pixel(48, 32, image::Rgb([255; 3]))
        .save(path)
        .unwrap();
}
#[cfg(feature = "ocr-provider")]
#[test]
fn selected_provider_maps_pdf_and_image_fixtures_without_credentials_and_rejects_wrong_bindings() {
    use saccade_core::general::document_ocr::{Mistral, Request};
    let dir = tempfile::tempdir().unwrap();
    for (name, bytes, mime) in [
        (
            "page.pdf",
            b"%PDF-1.7 generated fixture".to_vec(),
            "application/pdf",
        ),
        (
            "page.png",
            {
                let path = dir.path().join("generated.png");
                png(&path);
                std::fs::read(path).unwrap()
            },
            "image/png",
        ),
    ] {
        let path = dir.path().join(name);
        std::fs::write(&path, &bytes).unwrap();
        let request = Request {
            bytes,
            media_type: mime.into(),
            model: "mistral-ocr-2505".into(),
            pages: vec![0],
        };
        let response = json!({"model":request.model,"pages":[{"index":0,"markdown":"# café Straße señor\n\ntext is data","images":[],"dimensions":{"width":640,"height":480,"dpi":96}}],"usage_info":{"pages_processed":1}});
        let mut fixture = json!({"schema":"saccade-document-ocr-fixture.v1","request_sha256":saccade_core::evidence::canonical::digest(&Mistral::request(&request).unwrap()).unwrap(),"response":response});
        let fp = dir.path().join(format!("{name}.json"));
        std::fs::write(&fp, serde_json::to_vec(&fixture).unwrap()).unwrap();
        let out = dir.path().join(format!("{name}-out"));
        let result = cli(&[
            "text",
            path.to_str().unwrap(),
            path.to_str().unwrap(),
            "--ocr-provider",
            "mistral",
            "--ocr-model",
            "mistral-ocr-2505",
            "--ocr-responses",
            fp.to_str().unwrap(),
            fp.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--json",
        ]);
        assert_eq!(result.status.code(), Some(0), "{result:?}");
        let v: Value = serde_json::from_slice(
            &std::fs::read(out.join("saccade-document-text.v1.json")).unwrap(),
        )
        .unwrap();
        let schema: Value = serde_json::from_str(include_str!(
            "fixtures/package/crates/saccade-core/schemas/saccade-document-text.v2.schema.json"
        ))
        .unwrap();
        jsonschema::validator_for(&schema)
            .unwrap()
            .validate(&v)
            .unwrap();
        assert_eq!(v["verdict"], "pass");
        assert_eq!(v["comparison"]["rates"]["character_edits"], 0);
        assert_eq!(
            v["documents"][0]["pages"][0]["observations"][0]["bounds"],
            Value::Null
        );
        assert_eq!(
            v["documents"][0]["provenance"]["runtime"],
            "constructed-fixture"
        );
        fixture["request_sha256"] = json!("sha256:wrong");
        std::fs::write(&fp, serde_json::to_vec(&fixture).unwrap()).unwrap();
        let badout = dir.path().join(format!("{name}-bad"));
        let bad = cli(&[
            "text",
            path.to_str().unwrap(),
            path.to_str().unwrap(),
            "--ocr-provider",
            "mistral",
            "--ocr-model",
            "mistral-ocr-2505",
            "--ocr-responses",
            fp.to_str().unwrap(),
            fp.to_str().unwrap(),
            "--out",
            badout.to_str().unwrap(),
            "--json",
        ]);
        assert_eq!(bad.status.code(), Some(2));
        assert!(!badout.exists());
    }
}
#[cfg(feature = "ocr")]
#[test]
#[ignore = "heavy: generated PP-OCRv5 local CLI/media accent inference"]
fn default_local_engine_expectations_inspection_and_media_use_real_pins() {
    let fixtures = std::path::PathBuf::from(
        std::env::var_os("SACCADE_OCR_ACCENTS").expect("generated fixture directory"),
    );
    let cache = std::path::PathBuf::from(
        std::env::var_os("SACCADE_OCR_CACHE").expect("verified PP-OCRv5 cache"),
    );
    let dir = tempfile::tempdir().unwrap();
    let modeldir = dir.path().join("saccade/models");
    std::fs::create_dir_all(modeldir.parent().unwrap()).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&cache, &modeldir).unwrap();
    #[cfg(not(unix))]
    return;
    let path = fixtures.join("fr-DejaVuSans-40.png");
    let out = dir.path().join("text");
    let result = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .env("XDG_CACHE_HOME", dir.path())
        .args([
            "text",
            path.to_str().unwrap(),
            path.to_str().unwrap(),
            "--expect-text",
            "café",
            "--out",
            out.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    let text: Value =
        serde_json::from_slice(&std::fs::read(out.join("saccade-text.v1.json")).unwrap()).unwrap();
    assert_eq!(text["comparison"]["expected"][0]["present"], true);
    assert_eq!(text["comparison"]["expected"][0]["readable"], true);
    let out = dir.path().join("inspection");
    let result = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .env("XDG_CACHE_HOME", dir.path())
        .args([
            "inspect-image",
            path.to_str().unwrap(),
            "--ocr",
            "--out",
            out.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    let analyzer =
        saccade_core::media::Analyzer::new(saccade_core::media::Profile::CpuLite, cache, false)
            .unwrap();
    let record = analyzer
        .analyze_media(
            path.to_str().unwrap(),
            &saccade_core::media::Options {
                text: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
    let value = serde_json::to_value(record).unwrap();
    assert_eq!(value["text"]["status"], "ok");
    assert!(value["text"].to_string().contains("café"));
}
#[test]
fn provider_is_never_implicitly_selected() {
    let result = cli(&["text", "--help"]);
    assert_eq!(result.status.code(), Some(0));
    let help = String::from_utf8(result.stdout).unwrap();
    assert!(help.contains("--ocr-provider"));
    assert!(!help.contains("[default: mistral]"));
}

#[cfg(all(feature = "mcp", feature = "ocr-provider"))]
#[test]
fn mcp_provider_fixture_routing_respects_file_roots_without_live_authority() {
    use saccade_core::general::document_ocr::{Mistral, Request};
    use std::{io::Write, process::Stdio};
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("inputs");
    let out = dir.path().join("out");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&out).unwrap();
    let path = root.join("page.pdf");
    let bytes = b"%PDF-1.7 generated fixture".to_vec();
    std::fs::write(&path, &bytes).unwrap();
    let outside = dir.path().join("outside.pdf");
    std::fs::write(&outside, &bytes).unwrap();
    let request = Request {
        bytes,
        media_type: "application/pdf".into(),
        model: "mistral-ocr-2505".into(),
        pages: vec![0],
    };
    let fp = root.join("response.json");
    let fixture = json!({"schema":"saccade-document-ocr-fixture.v1","request_sha256":saccade_core::evidence::canonical::digest(&Mistral::request(&request).unwrap()).unwrap(),"response":{"model":request.model,"pages":[{"index":0,"markdown":"café"}]}});
    std::fs::write(&fp, serde_json::to_vec(&fixture).unwrap()).unwrap();
    let arguments = [
        json!({"operation":"document_text","a":path,"b":path,"response_a":fp,"response_b":fp,"model":request.model,"out":out.join("text")}),
        json!({"operation":"document_text","a":outside,"b":path,"response_a":fp,"response_b":fp,"model":request.model,"out":out.join("escaped")}),
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
        writeln!(stdin,"{}",json!({"jsonrpc":"2.0","id":id+1,"method":"tools/call","params":{"name":"saccade_general","arguments":args}})).unwrap();
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
    assert!(out.join("text/saccade-document-text.v1.json").is_file());
    assert!(!out.join("escaped").exists());
}
