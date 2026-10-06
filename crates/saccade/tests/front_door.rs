//! The task front door: help footer, command-name compatibility and aliases.
#![allow(clippy::unwrap_used, missing_docs)]

use std::path::Path;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_saccade");

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
}

fn text(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr)
}

fn pair(dir: &Path, name: &str, value: u8) {
    std::fs::create_dir_all(dir.join(name)).unwrap();
    image::RgbImage::from_pixel(8, 8, image::Rgb([value; 3]))
        .save(dir.join(name).join("a.png"))
        .unwrap();
}

#[test]
fn help_footer_lists_every_exit_code() {
    let tmp = tempfile::tempdir().unwrap();
    let help = text(&run(tmp.path(), &["--help"]));
    for line in [
        "  0  success",
        "  1  regression found",
        "  2  the command could not run",
        "  3  strict producer check refused: an undeclared difference",
        "  4  strict producer check refused: a required key is missing",
    ] {
        assert!(help.contains(line), "footer lacks {line:?}");
    }
}

/// Command names are a compatibility surface: this list only grows.
#[test]
fn every_existing_top_level_command_still_parses() {
    let tmp = tempfile::tempdir().unwrap();
    for name in [
        "compare",
        "prove",
        "review",
        "identity",
        "noise",
        "view",
        "inspect",
        "experiment",
        "approve",
        "init",
        "demo",
        "serve",
        "mcp",
        "ingest",
        "bisect",
        "history",
        "doctor",
        "schema",
        "perf",
        "arms",
        "capabilities",
        "inspect-image",
        "assess",
        "text",
        "hash",
        "dedupe",
        "models",
        "locate",
        "quality-score",
        "watermark",
        "faces",
        "crop-check",
        "localized-check",
        "inventory",
        "quality-sweep",
        "explain-grounded",
        "analyze-media",
        "keyframes",
        "find-usage",
        "similar",
        "index",
        "render-evidence",
    ] {
        let out = run(tmp.path(), &[name, "--help"]);
        assert_eq!(out.status.code(), Some(0), "{name}: {}", text(&out));
    }
}

#[test]
fn old_and_canonical_spellings_give_the_same_result() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    pair(root, "base", 100);
    pair(root, "same", 100);
    pair(root, "other", 180);
    for candidate in ["same", "other"] {
        let old = run(
            root,
            &["identity", "base", candidate, "--out", "o1", "--json"],
        );
        let new = run(
            root,
            &[
                "prove", "identity", "base", candidate, "--out", "o2", "--json",
            ],
        );
        assert_eq!(old.status.code(), new.status.code(), "{candidate}");
        let verdict = |o: &Output| {
            serde_json::from_slice::<serde_json::Value>(&o.stdout).unwrap()["verdict"].clone()
        };
        assert_eq!(verdict(&old), verdict(&new), "{candidate}");
        std::fs::remove_dir_all(root.join("o1")).unwrap();
        std::fs::remove_dir_all(root.join("o2")).unwrap();
    }
    // `watch` stays a deprecated spelling of one `compare` and names its replacement.
    let watch = run(root, &["watch", "base", "other", "--out", "w"]);
    assert_eq!(watch.status.code(), Some(1));
    assert!(text(&watch).contains("deprecated command; use saccade compare"));
    // `score` is `quality-score`: the same parser, the same help.
    assert_eq!(
        text(&run(root, &["score", "--help"])),
        text(&run(root, &["quality-score", "--help"]))
    );
}

#[test]
fn doctor_reports_command_availability_with_fix_text() {
    let tmp = tempfile::tempdir().unwrap();
    let out = run(tmp.path(), &["doctor", "--json"]);
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let rows = value["command_availability"].as_array().unwrap();
    assert_eq!(rows[0]["feature"], "builtin");
    assert_eq!(rows[0]["status"], "available");
    for row in rows {
        assert!(matches!(
            row["status"].as_str(),
            Some("available" | "unavailable")
        ));
        assert!(!row["note"].as_str().unwrap().is_empty());
    }
}

#[test]
fn unavailable_model_route_points_at_doctor_not_at_an_argument() {
    if cfg!(feature = "local-models") {
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let out = run(tmp.path(), &["quality-score", "x.png"]);
    assert_eq!(out.status.code(), Some(2));
    let message = text(&out);
    assert!(message.contains("local-models"), "{message}");
    assert!(message.contains("saccade doctor"), "{message}");
    assert!(!message.contains("correct the named argument"), "{message}");
}
