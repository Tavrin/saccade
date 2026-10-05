//! Explicitly opted-in runtime provisioning and version-error smoke.
#![cfg(feature = "local-models")]
#![allow(clippy::unwrap_used)]
use std::{path::PathBuf, process::Command};
#[test]
#[ignore = "heavy: models"]
fn runtime_pull_and_incompatible_library_are_typed() {
    let cache = PathBuf::from(std::env::var_os("WAVE7_MODEL_CACHE").unwrap());
    let proof = tempfile::tempdir_in(&cache).unwrap();
    let bin = env!("CARGO_BIN_EXE_saccade");
    let output = Command::new(bin)
        .args(["models", "pull", "runtime", "--cache"])
        .arg(proof.path())
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(receipt["runtime"]["pin"]["version"], "1.22.0");
    assert_eq!(receipt["runtime"]["status"], "cached_verified");
    assert!(PathBuf::from(receipt["ORT_DYLIB_PATH"].as_str().unwrap()).is_file());
    // Verify ordinary inference actually finds the provisioned library through the cache.
    let face = Command::new(bin)
        .args(["faces"])
        .arg(cache.join("fixtures/face.png"))
        .args(["--cache"])
        .arg(&cache)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        face.status.success(),
        "{}",
        String::from_utf8_lossy(&face.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&face.stdout).unwrap();
    assert_eq!(result["provenance"]["runtime"], "onnx-cpu");
    let runtime = receipt["ORT_DYLIB_PATH"].as_str().unwrap();
    let from_env = Command::new(bin)
        .arg("faces")
        .arg(cache.join("fixtures/face.png"))
        .arg("--cache")
        .arg(&cache)
        .arg("--json")
        .env("ORT_DYLIB_PATH", runtime)
        .output()
        .unwrap();
    assert!(
        from_env.status.success(),
        "{}",
        String::from_utf8_lossy(&from_env.stderr)
    );
    let watermark = Command::new(bin)
        .arg("watermark")
        .arg(cache.join("fixtures/bottle.png"))
        .arg("--trustmark")
        .arg("--cache")
        .arg(&cache)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        watermark.status.success(),
        "{}",
        String::from_utf8_lossy(&watermark.stderr)
    );
    let finding: serde_json::Value = serde_json::from_slice(&watermark.stdout).unwrap();
    assert_eq!(finding["findings"][0]["status"], "unavailable");
    assert!(finding["findings"][0]["payload_hex"].is_null());
    assert_eq!(finding["findings"][0]["provenance"]["runtime"], "onnx-cpu");
    if let Some(old) = std::env::var_os("WAVE7_INCOMPATIBLE_LIBRARY") {
        let output = Command::new(bin)
            .arg("faces")
            .arg(cache.join("fixtures/face.png"))
            .arg("--cache")
            .arg(&cache)
            .arg("--runtime-library")
            .arg(old)
            .env("ORT_DYLIB_PATH", runtime)
            .arg("--json")
            .output()
            .unwrap();
        assert!(!output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("runtime_incompatible"), "{text}");
        assert!(text.contains("1.22.x"), "{text}");
        assert!(text.contains("1.16.3"), "{text}");
    }
}
