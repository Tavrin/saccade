#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]
use image::{Rgb, RgbImage};
use std::path::Path;
use std::process::Command;

#[cfg(windows)]
const CAPTURE: &str = r#"copy /Y "frame.png" "%SACCADE_CAPTURE_DIR%\frame.png""#;
#[cfg(not(windows))]
const CAPTURE: &str = r#"cp "frame.png" "$SACCADE_CAPTURE_DIR/frame.png""#;
#[cfg(windows)]
const DIRTY_CAPTURE: &str =
    r#"type nul > dirty-in-clone & copy /Y "frame.png" "%SACCADE_CAPTURE_DIR%\frame.png""#;
#[cfg(not(windows))]
const DIRTY_CAPTURE: &str =
    r#"touch dirty-in-clone; cp "frame.png" "$SACCADE_CAPTURE_DIR/frame.png""#;
#[cfg(windows)]
const FAILED_CAPTURE: &str = "exit /B 7";
#[cfg(not(windows))]
const FAILED_CAPTURE: &str = "exit 7";

fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap().trim().into()
}

#[test]
fn finds_first_bad_and_restores_original_head() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("project with spaces");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.name", "Test"]);
    git(&repo, &["config", "user.email", "test@example.invalid"]);
    let frame = repo.join("frame.png");
    let write = |shade| {
        RgbImage::from_pixel(32, 32, Rgb([shade; 3]))
            .save(&frame)
            .unwrap()
    };
    write(60);
    git(&repo, &["add", "frame.png"]);
    git(&repo, &["commit", "-qm", "good"]);
    let good = git(&repo, &["rev-parse", "HEAD"]);
    let baseline = tmp.path().join("baseline with spaces");
    std::fs::create_dir(&baseline).unwrap();
    std::fs::copy(&frame, baseline.join("frame.png")).unwrap();
    std::fs::write(repo.join("note.txt"), "middle").unwrap();
    git(&repo, &["add", "note.txt"]);
    git(&repo, &["commit", "-qm", "middle"]);
    write(230);
    git(&repo, &["add", "frame.png"]);
    git(&repo, &["commit", "-qm", "bad"]);
    let first_bad = git(&repo, &["rev-parse", "HEAD"]);
    std::fs::write(repo.join("note.txt"), "still bad").unwrap();
    git(&repo, &["add", "note.txt"]);
    git(&repo, &["commit", "-qm", "still bad"]);
    let bad = git(&repo, &["rev-parse", "HEAD"]);
    let branch = git(&repo, &["branch", "--show-current"]);
    let out = tmp.path().join("evidence with spaces");
    let result = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .current_dir(&repo)
        .arg("bisect")
        .args(["--good", &good, "--bad", &bad, "--capture", CAPTURE])
        .arg("--baseline")
        .arg(&baseline)
        .arg("--out")
        .arg(&out)
        .arg("--json")
        .output()
        .unwrap();
    let evidence = std::fs::read_to_string(out.join("saccade-git-bisect.v1.json")).unwrap();
    assert_eq!(
        result.status.code(),
        Some(1),
        "{result:?}\n{evidence}\n{}",
        std::fs::read_to_string(out.join("git-bisect.log")).unwrap()
    );
    let result_json: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(result_json["data"]["first_bad"], first_bad);
    let evidence: serde_json::Value = serde_json::from_str(&evidence).unwrap();
    assert_eq!(evidence["git_bisect_exit"], 0);
    let steps = evidence["steps"].as_array().unwrap();
    for verdict in ["good", "bad"] {
        assert!(steps.iter().any(|step| step["verdict"] == verdict));
    }
    assert!(steps.iter().all(|step| step["capture_exit"] == 0));
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), bad);
    assert_eq!(git(&repo, &["branch", "--show-current"]), branch);
    assert_eq!(git(&repo, &["status", "--porcelain"]), "");
    assert!(out.join("saccade-git-bisect.v1.json").is_file());
    std::fs::write(repo.join("scratch"), "dirty").unwrap();
    let refused = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .current_dir(&repo)
        .arg("bisect")
        .args(["--good", &good, "--bad", &bad, "--capture", CAPTURE])
        .arg("--baseline")
        .arg(&baseline)
        .arg("--out")
        .arg(tmp.path().join("unused"))
        .output()
        .unwrap();
    assert_eq!(refused.status.code(), Some(2));
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), bad);
    assert!(!tmp.path().join("unused").exists());
    std::fs::remove_file(repo.join("scratch")).unwrap();
    let dirty_capture = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .current_dir(&repo)
        .arg("bisect")
        .args(["--good", &good, "--bad", &bad, "--capture", DIRTY_CAPTURE])
        .arg("--baseline")
        .arg(&baseline)
        .arg("--out")
        .arg(tmp.path().join("dirty-evidence"))
        .arg("--json")
        .output()
        .unwrap();
    assert_eq!(dirty_capture.status.code(), Some(2), "{dirty_capture:?}");
    let dirty_evidence: serde_json::Value = serde_json::from_slice(
        &std::fs::read(tmp.path().join("dirty-evidence/saccade-git-bisect.v1.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(dirty_evidence["steps"][0]["verdict"], "aborted");
    assert_eq!(dirty_evidence["steps"][0]["capture_exit"], 0);
    assert_ne!(dirty_evidence["git_bisect_exit"], 0);
    let failed_out = tmp.path().join("failed-evidence");
    let failed_capture = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .current_dir(&repo)
        .arg("bisect")
        .args(["--good", &good, "--bad", &bad, "--capture", FAILED_CAPTURE])
        .arg("--baseline")
        .arg(&baseline)
        .arg("--out")
        .arg(&failed_out)
        .arg("--json")
        .output()
        .unwrap();
    assert_eq!(failed_capture.status.code(), Some(2), "{failed_capture:?}");
    let failed_evidence: serde_json::Value = serde_json::from_slice(
        &std::fs::read(failed_out.join("saccade-git-bisect.v1.json")).unwrap(),
    )
    .unwrap();
    assert!(failed_evidence["first_bad"].is_null());
    assert_ne!(failed_evidence["git_bisect_exit"], 0);
    let steps = failed_evidence["steps"].as_array().unwrap();
    assert!(!steps.is_empty());
    assert!(steps.iter().all(|step| {
        step["verdict"] == "skip"
            && step["reason"] == "capture command failed"
            && step["capture_exit"] == 7
    }));
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), bad);
    assert_eq!(git(&repo, &["status", "--porcelain"]), "");
}
