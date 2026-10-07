//! Public watermark contract, including cached-only unavailability and strict versions.
#![allow(clippy::unwrap_used)]
use std::{path::PathBuf, process::Command};

#[test]
fn missing_trustmark_is_explicit_unavailable_without_cache_creation() {
    let temp = tempfile::tempdir().unwrap();
    let image = temp.path().join("input.png");
    image::RgbImage::new(16, 16).save(&image).unwrap();
    let cache = temp.path().join("absent-cache");
    let registry =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../saccade-core/assets/wave7-models.json");
    let output = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args([
            "watermark",
            image.to_str().unwrap(),
            "--trustmark",
            "--json",
            "--cache",
            cache.to_str().unwrap(),
            "--registry",
            registry.to_str().unwrap(),
            "--runtime-library",
        ])
        .arg(temp.path().join("absent-runtime"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schema"], "saccade-watermark.v4");
    assert_eq!(value["findings"][0]["status"], "unavailable");
    assert!(value["findings"][0]["payload_hex"].is_null());
    assert!(value["findings"][0]["payload_bits"].is_null());
    let schema: serde_json::Value =
        serde_json::from_str(saccade_core::schema_catalog::get("saccade-watermark.v4").unwrap())
            .unwrap();
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(&value)
        .unwrap();
    assert!(!cache.exists());
    let rejected = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args([
            "watermark",
            image.to_str().unwrap(),
            "--trustmark",
            "--allow-download",
            "--json",
            "--cache",
            cache.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(rejected.status.code(), Some(2));
    let rejection: serde_json::Value = serde_json::from_slice(&rejected.stdout).unwrap();
    assert!(
        rejection["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("models pull")
    );
    assert!(!cache.exists());
}

#[test]
fn retained_watermark_schema_replays_without_new_payload_fields() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("input.png");
    image::RgbImage::new(16, 16).save(&path).unwrap();
    let image = saccade_core::wave7::vision::VisionImage::load(&path).unwrap();
    let mut report = saccade_core::wave7::watermark::inspect(&image, None, None).unwrap();
    report.schema = "saccade-watermark.v1".into();
    let observation = temp.path().join("observation.json");
    std::fs::write(&observation, serde_json::to_vec(&report).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args([
            "watermark",
            path.to_str().unwrap(),
            "--observations",
            observation.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schema"], "saccade-watermark.v2");
    assert!(value["findings"][0].get("payload_bits").is_none());
    let schema: serde_json::Value =
        serde_json::from_str(saccade_core::schema_catalog::get("saccade-watermark.v2").unwrap())
            .unwrap();
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(&value)
        .unwrap();
}
