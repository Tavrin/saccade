//! Image-bound CLI evidence, unavailable OCR and invalid input handling.
#![allow(clippy::unwrap_used)]
use serde_json::{Value, json};
use std::{fs, process::Command};
fn run(cues: &std::path::Path, map: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["timed-text"])
        .arg(cues)
        .arg(map)
        .args(["--region", "0,0,80,32", "--json"])
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn imported_sources_are_bound_missing_is_unknown_and_stale_or_semantic_sources_error() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let map = d.join("map.json");
    let cues = d.join("cues.srt");
    let source = d.join("sources");
    fs::create_dir(&source).unwrap();
    image::RgbaImage::from_pixel(80, 32, image::Rgba([255, 255, 255, 255]))
        .save(d.join("f.png"))
        .unwrap();
    let hash = saccade_core::localized::digest(&fs::read(d.join("f.png")).unwrap());
    fs::write(&map,serde_json::to_vec(&json!({"schema":"saccade-frame-map.v1","frames":[{"index":0,"timestamp_s":0.0,"file":"f.png","sha256":hash},{"index":1,"timestamp_s":0.5,"file":"f.png","sha256":hash}]})).unwrap()).unwrap();
    fs::write(&cues, "1\n00:00:00,000 --> 00:00:01,000\nExpected\n").unwrap();
    let result = run(&cues, &map, &[]);
    assert_eq!(result.status.code(), Some(4));
    let value: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["state"], "insufficient_evidence");
    assert!(value["cues"][0]["findings"].as_array().unwrap().is_empty());
    let mut source_value = json!({"schema":"saccade-ui-source.v1","capture_sha256":hash,"dimensions":[80,32],"kind":"provider_ocr","producer":{"adapter":"generated truth"},"complete":false,"nodes":[]});
    for i in 0..2 {
        fs::write(
            source.join(format!("{i}.json")),
            serde_json::to_vec(&json!({"schema":"saccade-timed-text-source.v1","source":source_value,"declared_empty_region_px":[0,0,80,32]})).unwrap(),
        )
        .unwrap();
    }
    let args = ["--sources", source.to_str().unwrap()];
    let result = run(&cues, &map, &args);
    assert_eq!(result.status.code(), Some(1));
    let value: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["cues"][0]["findings"], json!(["missing"]));
    let schema: Value =
        serde_json::from_str(saccade_core::schema_catalog::get("saccade-timed-text.v1").unwrap())
            .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(validator.is_valid(&value));
    source_value["capture_sha256"] = json!("00".repeat(32));
    fs::write(
        source.join("0.json"),
        serde_json::to_vec(&json!({"schema":"saccade-timed-text-source.v1","source":source_value,"declared_empty_region_px":[0,0,80,32]})).unwrap(),
    )
    .unwrap();
    assert_eq!(run(&cues, &map, &args).status.code(), Some(2));
    source_value["capture_sha256"] = json!(hash);
    source_value["kind"] = json!("dom");
    fs::write(
        source.join("0.json"),
        serde_json::to_vec(&json!({"schema":"saccade-timed-text-source.v1","source":source_value,"declared_empty_region_px":[0,0,80,32]})).unwrap(),
    )
    .unwrap();
    assert_eq!(run(&cues, &map, &args).status.code(), Some(2));
    fs::write(&cues, "1\n00:00:00,000 --> 00:00:01,000\n<b>Expected</b>\n").unwrap();
    assert_eq!(run(&cues, &map, &[]).status.code(), Some(2));
}
#[test]
fn incomplete_empty_low_confidence_and_clipped_text_never_establish_missing() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    image::RgbaImage::from_pixel(80, 32, image::Rgba([255, 255, 255, 255]))
        .save(d.join("f.png"))
        .unwrap();
    let hash = saccade_core::localized::digest(&fs::read(d.join("f.png")).unwrap());
    let map = d.join("map.json");
    let cues = d.join("cues.vtt");
    let sources = d.join("sources");
    fs::create_dir(&sources).unwrap();
    fs::write(&map,json!({"schema":"saccade-frame-map.v1","frames":[{"index":0,"timestamp_s":0.0,"file":"f.png"},{"index":1,"timestamp_s":0.5,"file":"f.png"}]}).to_string()).unwrap();
    fs::write(&cues, "WEBVTT\n\n00:00.000 --> 00:01.000\nExpected\n").unwrap();
    for (complete, nodes) in [
        (false, json!([])),
        (
            false,
            json!([{"id":"x","text":"Expected","bounds":[0,0,80,32],"reading_order":null,"keyboard_order":null,"ocr_confidence":79}]),
        ),
        (
            false,
            json!([{"id":"x","text":"Expected","bounds":[-1,0,80,32],"reading_order":null,"keyboard_order":null,"ocr_confidence":100}]),
        ),
    ] {
        let source = json!({"schema":"saccade-ui-source.v1","capture_sha256":hash,"dimensions":[80,32],"kind":"provider_ocr","producer":{},"complete":complete,"nodes":nodes});
        for i in 0..2 {
            fs::write(sources.join(format!("{i}.json")), source.to_string()).unwrap();
        }
        let result = run(&cues, &map, &["--sources", sources.to_str().unwrap()]);
        assert_eq!(result.status.code(), Some(4));
        let value: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(value["cues"][0]["findings"], json!([]));
    }
}
