#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::path::Path;
use std::process::{Command, Output};

use image::{Rgb, RgbImage};

fn gradient() -> RgbImage {
    RgbImage::from_fn(32, 32, |x, y| Rgb([(x * 6) as u8, (y * 6) as u8, 120]))
}

fn save(dir: &Path, name: &str, img: &RgbImage) {
    std::fs::create_dir_all(dir.join(name).parent().expect("parent")).expect("mkdir");
    img.save(dir.join(name)).expect("save");
}

/// Writes `a.png` (identical) and `sub/b.png` (identical or one pixel +1).
fn pair(tmp: &Path, nudge: bool) -> (std::path::PathBuf, std::path::PathBuf) {
    let (parent, cand) = (tmp.join("parent"), tmp.join("cand"));
    for dir in [&parent, &cand] {
        save(dir, "a.png", &gradient());
    }
    save(&parent, "sub/b.png", &gradient());
    let mut b = gradient();
    if nudge {
        let p = b.get_pixel_mut(10, 10);
        p.0[1] = p.0[1].saturating_add(1);
    }
    save(&cand, "sub/b.png", &b);
    (parent, cand)
}

fn identity(tmp: &Path, extra: &[&str]) -> Output {
    let (parent, cand) = (tmp.join("parent"), tmp.join("cand"));
    Command::new(env!("CARGO_BIN_EXE_flipdiff"))
        .arg("identity")
        .args([&parent, &cand])
        .arg("--out")
        .arg(tmp.join("out"))
        .args(extra)
        .output()
        .expect("spawn")
}

fn report(tmp: &Path) -> serde_json::Value {
    let text = std::fs::read_to_string(tmp.join("out/flipdiff-report.v1.json")).expect("report");
    serde_json::from_str(&text).expect("json")
}

#[test]
fn identical_pair_is_bit_identical_and_passes_at_zero() {
    let tmp = tempfile::tempdir().expect("tmp");
    pair(tmp.path(), false);
    let o = identity(tmp.path(), &[]);
    assert_eq!(o.status.code(), Some(0), "{o:?}");
    let r = report(tmp.path());
    assert_eq!(r["config"]["mode"], "identity");
    assert_eq!(r["config"]["default_threshold"], 0.0);
    for e in r["entries"].as_array().expect("entries") {
        assert_eq!(e["bit_identical"], true);
        assert_eq!(e["status"], "pass");
    }
}

#[test]
fn one_level_change_fails_by_default_and_passes_with_threshold() {
    let tmp = tempfile::tempdir().expect("tmp");
    pair(tmp.path(), true);
    let o = identity(tmp.path(), &[]);
    assert_eq!(o.status.code(), Some(1), "{o:?}");
    let r = report(tmp.path());
    let b = r["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .find(|e| e["name"] == "sub/b.png")
        .expect("entry");
    assert_eq!(b["bit_identical"], false);
    assert_eq!(b["status"], "fail");
    let o = identity(tmp.path(), &["--threshold", "0.01"]);
    assert_eq!(o.status.code(), Some(0), "{o:?}");
}

#[test]
fn labels_reach_json_and_html() {
    let tmp = tempfile::tempdir().expect("tmp");
    pair(tmp.path(), false);
    let o = identity(tmp.path(), &["--labels", "main@abc,pr-7"]);
    assert_eq!(o.status.code(), Some(0), "{o:?}");
    let r = report(tmp.path());
    assert_eq!(r["config"]["labels"]["baseline"], "main@abc");
    assert_eq!(r["config"]["labels"]["capture"], "pr-7");
    let html = std::fs::read_to_string(tmp.path().join("out/index.html")).expect("html");
    assert!(html.contains("main@abc") && html.contains("pr-7"));
}

#[test]
fn identity_markdown_headline() {
    let tmp = tempfile::tempdir().expect("tmp");
    pair(tmp.path(), false);
    identity(tmp.path(), &[]);
    let json = tmp.path().join("out/flipdiff-report.v1.json");
    let summary = |json: &Path| {
        let o = Command::new(env!("CARGO_BIN_EXE_flipdiff"))
            .arg("summary")
            .arg(json)
            .output()
            .expect("spawn");
        String::from_utf8(o.stdout).expect("utf8")
    };
    assert!(summary(&json).contains("identity: ✅ 2/2 bit-identical"));
    pair(tmp.path(), true);
    identity(tmp.path(), &[]);
    let md = summary(&json);
    assert!(
        md.contains("identity: ❌ 1 differ (max FLIP ") && md.contains(" on sub/b.png)"),
        "{md}"
    );
}
