//! Real stock-source cross-domain fixtures and typed execution failures.
#![allow(clippy::unwrap_used)]
use serde_json::Value;
use std::{path::Path, process::Command};
fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
#[test]
fn generated_pack_proves_mean_pass_critical_fail_and_typography_controls() {
    let out = tempfile::tempdir().unwrap();
    let result = Command::new("python3")
        .arg(root().join("scripts/critical-text/fixtures.py"))
        .args(["--binary", env!("CARGO_BIN_EXE_saccade"), "--receipts"])
        .arg(out.path())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    for file in std::fs::read_dir(out.path()).unwrap() {
        let path = file.unwrap().path();
        if path.extension().is_some_and(|s| s == "json")
            && path.file_name().unwrap() != "qualification.json"
        {
            let report: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            let schema: Value = serde_json::from_str(
                saccade_core::schema_catalog::get(report["schema"].as_str().unwrap()).unwrap(),
            )
            .unwrap();
            assert!(
                jsonschema::validator_for(&schema)
                    .unwrap()
                    .is_valid(&report),
                "{}",
                path.display()
            );
        }
    }
}
#[test]
fn missing_observations_stale_sources_and_output_collisions_fail_closed() {
    let pack = root().join("testdata/critical-text/app-decimal");
    let run = |extra: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_saccade"))
            .arg("critical-text")
            .arg(pack.join("baseline.png"))
            .arg(pack.join("same.png"))
            .arg("--policy")
            .arg(pack.join("policy.json"))
            .args(extra)
            .arg("--json")
            .output()
            .unwrap()
    };
    let no_source = run(&[]);
    assert_eq!(no_source.status.code(), Some(4));
    assert_eq!(
        serde_json::from_slice::<Value>(&no_source.stdout).unwrap()["state"],
        "insufficient_evidence"
    );
    let out = tempfile::tempdir().unwrap();
    let stale = out.path().join("stale.json");
    let mut source: Value =
        serde_json::from_slice(&std::fs::read(pack.join("same-source.json")).unwrap()).unwrap();
    source["capture_sha256"] = "0".repeat(64).into();
    std::fs::write(&stale, serde_json::to_vec(&source).unwrap()).unwrap();
    let a = pack.join("baseline-source.json");
    let result = run(&[
        "--a-source",
        a.to_str().unwrap(),
        "--b-source",
        stale.to_str().unwrap(),
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert_eq!(
        serde_json::from_slice::<Value>(&result.stdout).unwrap()["errors"][0]["code"],
        "config"
    );
    let b = pack.join("same-source.json");
    let result = run(&[
        "--a-source",
        a.to_str().unwrap(),
        "--b-source",
        b.to_str().unwrap(),
        "--out",
        pack.to_str().unwrap(),
    ]);
    assert_eq!(result.status.code(), Some(2));
    #[cfg(not(feature = "ocr"))]
    assert_eq!(run(&["--ocr"]).status.code(), Some(2));
}
