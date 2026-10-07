//! Offline acceptance of generated producer receipts and the public CLI contract.
#![allow(clippy::unwrap_used)]
use serde_json::Value;
use std::{path::PathBuf, process::Command};

fn kit() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/capture-kit")
}
fn run(path: &std::path::Path) -> (i32, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["capture", "conform"])
        .arg(path)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    (
        output.status.code().unwrap(),
        serde_json::from_slice(&output.stdout).unwrap(),
    )
}
#[test]
fn both_producers_pass_valid_receipts_and_reject_every_adversarial_case() {
    let expectations: Vec<Value> =
        serde_json::from_slice(&std::fs::read(kit().join("expected.json")).unwrap()).unwrap();
    let schema: Value = serde_json::from_str(
        saccade_core::schema_catalog::get(saccade_core::capture::RESULT_SCHEMA).unwrap(),
    )
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    for expected in expectations {
        let path = kit().join(expected["file"].as_str().unwrap());
        let (code, result) = run(&path);
        assert_eq!(
            code as i64,
            expected["exit"].as_i64().unwrap(),
            "{}: {result}",
            path.display()
        );
        assert!(validator.is_valid(&result), "{}: {result}", path.display());
        assert_eq!(
            result["conformant"],
            expected["code"].is_null(),
            "{}",
            path.display()
        );
        let findings = result["findings"].as_array().unwrap();
        if expected["code"].is_null() {
            assert!(findings.is_empty());
        } else {
            assert_eq!(findings.len(), 1, "{}: {result}", path.display());
            assert_eq!(findings[0]["code"], expected["code"], "{}", path.display());
        }
    }
}
#[test]
fn malformed_missing_and_future_records_emit_typed_json() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("receipt.json");
    assert_eq!(run(&path).1["findings"][0]["code"], "record_unavailable");
    std::fs::write(&path, b"{ malformed").unwrap();
    let (exit, result) = run(&path);
    assert_eq!(exit, 2);
    assert_eq!(result["findings"][0]["code"], "invalid_record");
    let valid = std::fs::read_to_string(kit().join("renderer-valid.json")).unwrap();
    std::fs::write(
        &path,
        valid.replace(
            "\"status\": \"captured\"",
            "\"status\": \"failed\", \"status\": \"captured\"",
        ),
    )
    .unwrap();
    assert_eq!(run(&path).1["findings"][0]["code"], "invalid_record");
    std::fs::write(&path, br#"{"schema":"saccade-capture-record.v2"}"#).unwrap();
    assert_eq!(run(&path).1["findings"][0]["code"], "unsupported_schema");
}
#[test]
fn shipped_input_schema_describes_generated_records_and_stays_discoverable() {
    let schema: Value = serde_json::from_str(
        saccade_core::schema_catalog::get(saccade_core::capture::RECORD_SCHEMA).unwrap(),
    )
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    for producer in ["renderer", "browser"] {
        let value: Value = serde_json::from_slice(
            &std::fs::read(kit().join(format!("{producer}-valid.json"))).unwrap(),
        )
        .unwrap();
        assert!(validator.is_valid(&value));
        let output = Command::new(env!("CARGO_BIN_EXE_saccade"))
            .args(["schema", "get", saccade_core::capture::RECORD_SCHEMA])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout).unwrap(),
            schema
        );
    }
}
