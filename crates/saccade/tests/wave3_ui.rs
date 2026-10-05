//! Synthetic CLI/ingest contracts; optional process fixtures are not recognizer parity.
#![allow(clippy::unwrap_used, missing_docs)]
use serde_json::json;
use std::{path::Path, process::Command};
fn hash(bytes: &[u8]) -> String {
    saccade_core::localized::digest(bytes)
}
fn source(hash: &str) -> serde_json::Value {
    json!({"schema":"saccade-ui-source.v1","capture_sha256":hash,"dimensions":[16,16],"kind":"dom","producer":{"fixture":"synthetic"},"complete":true,"nodes":[{"id":"price","text":"€19.99","reading_order":0}]})
}
fn image(path: &Path) -> String {
    image::RgbImage::from_pixel(16, 16, image::Rgb([20, 20, 20]))
        .save(path)
        .unwrap();
    hash(&std::fs::read(path).unwrap())
}
#[test]
fn ingest_preserves_and_checks_both_source_documents() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let bh = image(&root.join("before.png"));
    let ah = image(&root.join("after.png"));
    let mut after = source(&ah);
    after["nodes"][0]["text"] = json!("€79.99");
    let manifest = json!({"schema":"saccade-playwright.v1","entries":[{"test_id":"synthetic","project":"fixture","browser":"fixture","viewport":[16,16],"expected":"before.png","actual":"after.png","diff":null,"ui_sources":[source(&bh),after]}]});
    std::fs::write(
        root.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let out = root.join("ingested");
    let result = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["ingest", "playwright"])
        .arg(root.join("manifest.json"))
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert!(result.status.success(), "{result:?}");
    assert!(out.join("ui-sources/0000-0.json").is_file());
    assert!(out.join("ui-sources/0000-1.json").is_file());
    let result = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["review", "ui"])
        .arg(out.join("baseline/0000.png"))
        .arg(out.join("capture/0000.png"))
        .arg("--reference-source")
        .arg(out.join("ui-sources/0000-0.json"))
        .arg("--candidate-source")
        .arg(out.join("ui-sources/0000-1.json"))
        .args([
            "--box",
            "2,2,4,4",
            "--ocr-contract",
            "missing-runtime.json",
            "--out",
        ])
        .arg(root.join("packet.json"))
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    let packet: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("packet.json")).unwrap()).unwrap();
    assert_eq!(packet["findings"][0]["evidence"]["before"], "€19.99");
    assert_eq!(packet["findings"][0]["evidence"]["after"], "€79.99");
    let mut stale = manifest;
    stale["entries"][0]["ui_sources"][1]["capture_sha256"] = json!(hash(b"stale"));
    std::fs::write(
        root.join("manifest.json"),
        serde_json::to_vec(&stale).unwrap(),
    )
    .unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["ingest", "playwright"])
        .arg(root.join("manifest.json"))
        .arg("--out")
        .arg(root.join("stale"))
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
}
#[cfg(all(feature = "ocr", unix))]
#[test]
fn pinned_optional_ocr_process_and_model_hashes_are_verified() {
    use saccade_core::evidence::canonical::Digest;
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    image(&root.join("a.png"));
    let version = b"tesseract 5.fixture\n";
    let executable=b"#!/bin/sh\nif [ \"$1\" = --version ]; then printf 'tesseract 5.fixture\\n'; exit 0; fi\nprintf 'level\\tpage_num\\tblock_num\\tpar_num\\tline_num\\tword_num\\tleft\\ttop\\twidth\\theight\\tconf\\ttext\\n5\\t1\\t1\\t1\\t1\\t1\\t0\\t0\\t8\\t8\\t100\\tEUR19.99\\n' > \"$2.tsv\"\n";
    std::fs::write(root.join("fixture-ocr"), executable).unwrap();
    std::fs::set_permissions(
        root.join("fixture-ocr"),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    std::fs::create_dir(root.join("models")).unwrap();
    std::fs::write(root.join("models/eng.traineddata"), b"synthetic model").unwrap();
    let mut contract = json!({"schema":"saccade-tesseract.v1","executable":"fixture-ocr","executable_sha256":Digest::of_bytes(executable),"version":"5.fixture","version_output_sha256":Digest::of_bytes(version),"tessdata":"models","models":{"eng":Digest::of_bytes(b"synthetic model")},"psm":6,"timeout_ms":1000});
    let run = |contract: &serde_json::Value, out: &str| {
        std::fs::write(root.join("ocr.json"), serde_json::to_vec(contract).unwrap()).unwrap();
        Command::new(env!("CARGO_BIN_EXE_saccade"))
            .args(["review", "ui"])
            .arg(root.join("a.png"))
            .arg(root.join("a.png"))
            .args(["--box", "2,2,4,4", "--ocr-contract"])
            .arg(root.join("ocr.json"))
            .arg("--out")
            .arg(root.join(out))
            .output()
            .unwrap()
    };
    let result = run(&contract, "ocr-packet.json");
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    let packet: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("ocr-packet.json")).unwrap()).unwrap();
    assert_eq!(packet["sources"][0]["kind"], "tesseract_tsv");
    assert_eq!(packet["sources"][0]["nodes"][0]["ocr_confidence"], 100.0);
    assert_eq!(packet["sources"][0]["complete"], false);
    contract["models"]["eng"] = json!(Digest::of_bytes(b"wrong"));
    assert_eq!(run(&contract, "bad.json").status.code(), Some(2));
}
