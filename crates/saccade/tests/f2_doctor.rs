//! Build identity and behavior capability contract at the executable boundary.
#![allow(clippy::unwrap_used, missing_docs)]

use serde_json::Value;
use std::path::Path;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_saccade");

#[test]
fn doctor_exposes_build_identity_and_named_capabilities() {
    let output = Command::new(BIN)
        .args(["doctor", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let doctor: Value = serde_json::from_slice(&output.stdout).unwrap();
    let capabilities = doctor["capabilities"].as_array().unwrap();
    for name in [
        "compat-aliases",
        "removed-flag-errors",
        "version-skew-errors",
        "provenance-warnings",
        "repeat-detection",
        "perf-v2",
        "identity-json-v1",
    ] {
        assert!(
            capabilities.iter().any(|value| value == name),
            "missing {name}"
        );
    }
    for (name, enabled) in [
        ("prechecks", cfg!(feature = "prechecks")),
        ("mcp", cfg!(feature = "mcp")),
        ("review", cfg!(feature = "ai")),
    ] {
        assert_eq!(capabilities.iter().any(|value| value == name), enabled);
    }
    assert!(doctor["build"]["profile"].as_str().is_some());
    assert!(
        doctor["build"]["rustc_version"]
            .as_str()
            .unwrap()
            .starts_with("rustc ")
    );

    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let version = Command::new(BIN).arg("--version").output().unwrap();
    assert!(version.status.success());
    let version = String::from_utf8(version.stdout).unwrap();
    if root.join(".git").exists() {
        let full = doctor["build"]["git_commit"].as_str().unwrap();
        let short = doctor["build"]["git_commit_short"].as_str().unwrap();
        assert_eq!(full.len(), 40);
        assert!(full.starts_with(short));
        assert!(doctor["build"]["git_dirty"].is_boolean());
        assert!(version.contains(&format!("+g{short}")));
    } else {
        assert!(doctor["build"]["git_commit"].is_null());
        assert!(doctor["build"]["git_commit_short"].is_null());
        assert!(doctor["build"]["git_dirty"].is_null());
    }
}
