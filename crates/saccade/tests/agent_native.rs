//! Agent-facing surface: `explain`, `mcp` and `approve --json`.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

#[path = "support/approval.rs"]
mod approval_support;
use std::path::Path;
use std::process::{Command, Stdio};

use image::{Rgb, RgbImage};
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_saccade");

/// A flat grey frame, with a bright 40x30 patch at (60, 80) when `patch`.
fn save(dir: &Path, name: &str, patch: bool) {
    std::fs::create_dir_all(dir).expect("mkdir");
    let mut img = RgbImage::from_pixel(160, 120, Rgb([90, 90, 90]));
    if patch {
        for y in 80..110 {
            for x in 60..100 {
                img.put_pixel(x, y, Rgb([230, 230, 230]));
            }
        }
    }
    img.save(dir.join(name)).expect("save");
}

fn dirs(tmp: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let (base, cap) = (tmp.join("base"), tmp.join("cap"));
    save(&base, "scene.png", false);
    save(&cap, "scene.png", true);
    save(&base, "same.png", false);
    save(&cap, "same.png", false);
    (base, cap)
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("json")
}

#[test]
fn explain_writes_strips_and_blind_hides_the_heatmap_and_the_key() {
    let tmp = tempfile::tempdir().unwrap();
    let (base, cap) = dirs(tmp.path());
    let report = tmp.path().join("report");
    let status = Command::new(BIN)
        .args(["compare"])
        .args([&base, &cap])
        .arg("--out")
        .arg(&report)
        .stdout(Stdio::null())
        .status()
        .expect("spawn");
    assert_eq!(status.code(), Some(1));
    let json = report.join("saccade-report.v1.json");

    // The text table carries the hotspot line.
    let table = Command::new(BIN)
        .arg("inspect")
        .arg(&json)
        .output()
        .expect("spawn");
    assert!(table.status.success());

    let out = tmp.path().join("pack");
    let ok = Command::new(BIN)
        .args(["inspect", "evidence"])
        .arg(&json)
        .arg("--out")
        .arg(&out)
        .stdout(Stdio::null())
        .status()
        .expect("spawn");
    assert!(ok.success());
    let pack = read_json(&out.join("explain.json"));
    assert_eq!(pack["schema"], "saccade-explain.v1");
    let hs = &pack["entries"][0]["hotspots"][0];
    assert_eq!(hs["panels"], json!(["baseline", "capture", "heatmap"]));
    let rect = hs["hotspot"]["rect_px"].as_array().unwrap();
    assert!(rect[0].as_i64().unwrap().abs_diff(60) <= 3, "{rect:?}");
    assert!(rect[1].as_i64().unwrap().abs_diff(80) <= 3, "{rect:?}");
    assert_eq!(hs["hotspot"]["position"], "bottom-center");
    let strip = image::open(out.join(hs["strip"].as_str().unwrap())).expect("strip decodes");
    assert!(strip.width() > 3 * 256, "three panels of at least 256 px");
    assert!(out.join("thumbs/scene.png").is_file());
    assert!(out.join("explain.md").is_file());
    assert!(!out.join("blind-key.json").exists());

    let out = tmp.path().join("blind");
    let key_out = tmp.path().join("keys/blind-key.json");
    let ok = Command::new(BIN)
        .args(["inspect", "evidence"])
        .arg(&json)
        .args(["--blind", "--seed", "7", "--out"])
        .arg(&out)
        .arg("--key-out")
        .arg(&key_out)
        .stdout(Stdio::null())
        .status()
        .expect("spawn");
    assert!(ok.success());
    let pack = read_json(&out.join("explain.json"));
    let hs = &pack["entries"][0]["hotspots"][0];
    assert_eq!(hs["panels"], json!(["A", "B"]));
    assert!(pack["labels"].is_null() && pack["report"].is_null());
    let blind_strip = image::open(out.join(hs["strip"].as_str().unwrap())).unwrap();
    assert!(blind_strip.width() < 700, "two panels, no heatmap");
    assert!(
        !out.join("blind-key.json").exists(),
        "the key stays out of the pack"
    );
    let key = read_json(&key_out);
    assert_eq!(key["seed"], 7);
    assert_eq!(
        key["items"].as_array().unwrap().len(),
        2,
        "thumbnail and one hotspot"
    );
    let md = std::fs::read_to_string(out.join("explain.md")).unwrap();
    assert!(!md.contains("baseline") && !md.contains("capture"));
}

/// Sends `requests` (one JSON message per line) to `saccade mcp --root root`,
/// returns the replies in order.
#[test]
fn approve_json_lists_copied_and_pruned_files() {
    let tmp = tempfile::tempdir().unwrap();
    let (base, cap) = dirs(tmp.path());
    save(&base, "gone.png", false);
    let report = tmp.path().join("report");
    Command::new(BIN)
        .arg("compare")
        .args([&base, &cap])
        .arg("--out")
        .arg(&report)
        .stdout(Stdio::null())
        .status()
        .expect("spawn");
    let decision = approval_support::draft(
        &report.join("saccade-report.v1.json"),
        &tmp.path().join("plan"),
        &[],
        true,
    );
    let out = Command::new(BIN)
        .arg("approve")
        .args([&cap, &base])
        .arg("--all-failing")
        .arg(report.join("saccade-report.v1.json"))
        .arg("--decisions")
        .arg(&decision)
        .args(["--prune-missing", "--json"])
        .output()
        .expect("spawn");
    assert!(out.status.success());
    let v: Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(
        v["schema"],
        saccade_core::report_links::linked_schema("saccade-result.v2")
    );
    let v = &v["data"];
    assert_eq!(v["copied"].as_array().unwrap().len(), 1);
    assert_eq!(v["copied"][0]["name"], "scene.png");
    assert_eq!(v["pruned"].as_array().unwrap().len(), 1);
    assert!(!base.join("gone.png").exists());
}
