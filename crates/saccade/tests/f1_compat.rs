//! Consumer compatibility at the executable boundary.
#![allow(clippy::unwrap_used, missing_docs)]

use serde_json::{Value, json};
use std::path::Path;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_saccade");

fn capture(root: &Path, name: &str, id: &str, provenance: bool) {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    image::RgbImage::from_pixel(4, 4, image::Rgb([40, 50, 60]))
        .save(dir.join("scene.png"))
        .unwrap();
    let mut card = json!({"capture.id":id});
    if provenance {
        card["binary.sha"] = json!(format!("binary-{name}"));
        card["source.head"] = json!(format!("head-{name}"));
    }
    std::fs::write(dir.join("cost-card.json"), card.to_string()).unwrap();
}

#[test]
fn deprecated_command_and_flag_name_the_replacement() {
    let alias = Command::new(BIN)
        .args(["config", "--json"])
        .output()
        .unwrap();
    assert!(alias.status.success());
    assert!(
        String::from_utf8_lossy(&alias.stderr)
            .contains("deprecated command; use saccade inspect config")
    );
    let flag = Command::new(BIN)
        .args(["identity", "--threshold", "0.1"])
        .output()
        .unwrap();
    assert_eq!(flag.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&flag.stderr)
            .contains("identity is exact; use compare --threshold")
    );
    let temp = tempfile::tempdir().unwrap();
    capture(temp.path(), "a", "one", true);
    capture(temp.path(), "b", "two", true);
    let watch = Command::new(BIN)
        .current_dir(temp.path())
        .args(["watch", "a", "b", "--once"])
        .output()
        .unwrap();
    assert!(watch.status.success());
    assert!(String::from_utf8_lossy(&watch.stderr).contains("use saccade compare BASE CAPTURE"));
    assert!(
        temp.path()
            .join("watch-report/saccade-report.v1.json")
            .is_file()
    );
    let report = temp.path().join("watch-report/saccade-report.v1.json");
    let snapshot = Command::new(BIN)
        .current_dir(temp.path())
        .arg("snapshot")
        .arg(&report)
        .args(["--entry", "scene.png"])
        .output()
        .unwrap();
    assert!(
        snapshot.status.success(),
        "{}",
        String::from_utf8_lossy(&snapshot.stderr)
    );
    assert!(temp.path().join("snapshot.png").is_file());
    let explain = Command::new(BIN)
        .current_dir(temp.path())
        .arg("explain")
        .arg(&report)
        .output()
        .unwrap();
    assert!(
        explain.status.success(),
        "{}",
        String::from_utf8_lossy(&explain.stderr)
    );
    assert!(
        temp.path()
            .join("watch-report/explain/explain.json")
            .is_file()
    );
}

#[test]
fn doctor_reports_supported_versions() {
    let out = Command::new(BIN)
        .args(["doctor", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["version"], env!("CARGO_PKG_VERSION"));
    assert!(
        value["schemas"]["performance"]
            .as_array()
            .unwrap()
            .contains(&json!("saccade-perf.v2"))
    );
    assert!(value["features"].is_array());
}

#[test]
fn provenance_is_recorded_and_missing_values_are_loud() {
    let temp = tempfile::tempdir().unwrap();
    capture(temp.path(), "a", "one", true);
    capture(temp.path(), "b", "two", false);
    let out = Command::new(BIN)
        .args(["compare"])
        .arg(temp.path().join("a"))
        .arg(temp.path().join("b"))
        .arg("--out")
        .arg(temp.path().join("report"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("provenance is absent"));
    let report: Value = serde_json::from_slice(
        &std::fs::read(temp.path().join("report/saccade-report.v1.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        report["entries"][0]["capture_provenance"]["baseline.binary_sha256"],
        "binary-a"
    );
    assert!(
        report["entries"][0]["capture_validity"]["reasons"]
            .to_string()
            .contains("source_head provenance is absent")
    );
}

#[test]
fn moss_cost_card_aliases_supply_provenance() {
    let temp = tempfile::tempdir().unwrap();
    capture(temp.path(), "a", "one", false);
    capture(temp.path(), "b", "two", false);
    for name in ["a", "b"] {
        std::fs::write(
            temp.path().join(name).join("cost-card.json"),
            json!({"binary.sha":format!("binary-{name}"),"build.commit":"head","capture.id":name})
                .to_string(),
        )
        .unwrap();
    }
    let out = Command::new(BIN)
        .args(["identity"])
        .arg(temp.path().join("a"))
        .arg(temp.path().join("b"))
        .args(["--meta-name", "cost-card.json", "--out"])
        .arg(temp.path().join("report"))
        .args(["--json"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let result: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        !result["validity_reasons"]
            .to_string()
            .contains("source_head provenance is absent")
    );
    assert!(
        result["validity_guidance"]["undeclared_keys"]
            .to_string()
            .contains("binary.sha")
    );
    assert!(
        result["validity_guidance"]["action"]
            .as_str()
            .unwrap()
            .contains("--declare")
    );
    let report: Value = serde_json::from_slice(
        &std::fs::read(temp.path().join("report/saccade-report.v1.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        report["entries"][0]["capture_provenance"]["baseline.source_head"],
        "head"
    );
    assert_eq!(
        report["entries"][0]["capture_provenance"]["capture.binary_sha256"],
        "binary-b"
    );
}

#[test]
fn identical_capture_ids_refuse_identity_and_noise() {
    let temp = tempfile::tempdir().unwrap();
    capture(temp.path(), "a", "same", true);
    capture(temp.path(), "b", "same", true);
    let compare = Command::new(BIN)
        .args(["compare"])
        .arg(temp.path().join("a"))
        .arg(temp.path().join("b"))
        .arg("--out")
        .arg(temp.path().join("compare"))
        .output()
        .unwrap();
    assert!(compare.status.success());
    assert!(String::from_utf8_lossy(&compare.stderr).contains("same capture, not a repeat"));
    let identity = Command::new(BIN)
        .args(["identity"])
        .arg(temp.path().join("a"))
        .arg(temp.path().join("b"))
        .arg("--out")
        .arg(temp.path().join("identity"))
        .output()
        .unwrap();
    assert_eq!(identity.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&identity.stderr).contains("same capture, not a repeat"));
    let noise = Command::new(BIN)
        .args(["noise"])
        .arg(temp.path().join("a"))
        .arg(temp.path().join("b"))
        .arg("--out")
        .arg(temp.path().join("noise.toml"))
        .output()
        .unwrap();
    assert_eq!(noise.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&noise.stderr).contains("same capture, not a repeat"));
}
