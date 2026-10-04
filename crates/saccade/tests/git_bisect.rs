#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]
use image::{Rgb, RgbImage};
use std::path::Path;
use std::process::Command;

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
    let repo = tmp.path().join("project");
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
    let baseline = tmp.path().join("baseline");
    std::fs::create_dir(&baseline).unwrap();
    std::fs::copy(&frame, baseline.join("frame.png")).unwrap();
    std::fs::write(repo.join("note.txt"), "middle").unwrap();
    git(&repo, &["add", "note.txt"]);
    git(&repo, &["commit", "-qm", "middle"]);
    write(230);
    git(&repo, &["add", "frame.png"]);
    git(&repo, &["commit", "-qm", "bad"]);
    let bad = git(&repo, &["rev-parse", "HEAD"]);
    let branch = git(&repo, &["branch", "--show-current"]);
    let out = tmp.path().join("evidence");
    let result = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .current_dir(&repo)
        .arg("bisect")
        .args([
            "--good",
            &good,
            "--bad",
            &bad,
            "--capture",
            "cp frame.png \"$SACCADE_CAPTURE_DIR/frame.png\"",
        ])
        .arg("--baseline")
        .arg(&baseline)
        .arg("--out")
        .arg(&out)
        .arg("--json")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    let result_json: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(result_json["data"]["first_bad"], bad);
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), bad);
    assert_eq!(git(&repo, &["branch", "--show-current"]), branch);
    assert_eq!(git(&repo, &["status", "--porcelain"]), "");
    assert!(out.join("saccade-git-bisect.v1.json").is_file());
    std::fs::write(repo.join("scratch"), "dirty").unwrap();
    let refused = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .current_dir(&repo)
        .arg("bisect")
        .args(["--good", &good, "--bad", &bad, "--capture", "true"])
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
        .args([
            "--good",
            &good,
            "--bad",
            &bad,
            "--capture",
            "touch dirty-in-clone; cp frame.png \"$SACCADE_CAPTURE_DIR/frame.png\"",
        ])
        .arg("--baseline")
        .arg(&baseline)
        .arg("--out")
        .arg(tmp.path().join("dirty-evidence"))
        .arg("--json")
        .output()
        .unwrap();
    assert_eq!(dirty_capture.status.code(), Some(2), "{dirty_capture:?}");
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), bad);
    assert_eq!(git(&repo, &["status", "--porcelain"]), "");
}
