//! Agent-facing surface: `explain`, `mcp` and `approve --json`.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

#[path = "support/approval.rs"]
mod approval_support;
use std::io::Write;
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
        .args(["summary", "--format", "text"])
        .arg(&json)
        .output()
        .expect("spawn");
    assert!(String::from_utf8_lossy(&table.stdout).contains("↳ 1 hotspot: "));

    let out = tmp.path().join("pack");
    let ok = Command::new(BIN)
        .arg("explain")
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
        .arg("explain")
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
fn mcp(root: &Path, requests: &[Value]) -> Vec<Value> {
    let mut child = Command::new(BIN)
        .arg("mcp")
        .arg("--root")
        .arg(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn");
    let mut stdin = child.stdin.take().expect("stdin");
    for r in requests {
        writeln!(stdin, "{r}").expect("write");
    }
    drop(stdin);
    let out = child.wait_with_output().expect("wait");
    String::from_utf8(out.stdout)
        .expect("utf8")
        .lines()
        .map(|l| serde_json::from_str(l).expect("each stdout line is JSON"))
        .collect()
}

fn call(id: u32, tool: &str, args: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
           "params": {"name": tool, "arguments": args}})
}

#[test]
fn mcp_round_trip_compare_returns_a_structured_verdict() {
    let tmp = tempfile::tempdir().unwrap();
    let (base, cap) = dirs(tmp.path());
    let out = tmp.path().join("out");
    let replies = mcp(
        tmp.path(),
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": {"name": "test", "version": "0"}}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            call(
                3,
                "saccade_compare",
                json!({"baseline_dir": base, "capture_dir": cap, "out_dir": out}),
            ),
            call(
                4,
                "saccade_summary",
                json!({"report_json": out.join("saccade-report.v1.json")}),
            ),
        ],
    );
    assert_eq!(replies.len(), 4, "the notification gets no reply");
    assert_eq!(replies[0]["result"]["protocolVersion"], "2025-06-18");
    let names: Vec<&str> = replies[1]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for tool in [
        "saccade_compare",
        "saccade_identity",
        "saccade_explain",
        "saccade_summary",
        "saccade_snapshot",
        "saccade_decision_request",
        "saccade_decide",
    ] {
        assert!(names.contains(&tool), "{tool} is listed in {names:?}");
    }
    let result = &replies[2]["result"];
    assert_eq!(result["isError"], false);
    let s = &result["structuredContent"];
    assert_eq!(s["verdict"], "regression");
    assert_eq!(s["totals"]["fail"], 1);
    assert_eq!(s["failing"][0]["name"], "scene.png");
    assert_eq!(s["failing"][0]["hotspots"][0]["position"], "bottom-center");
    assert!(
        tmp.path()
            .join(s["paths"]["index_html"].as_str().unwrap())
            .is_file()
    );
    assert!(
        tmp.path()
            .join(s["paths"]["explain_md"].as_str().unwrap())
            .is_file()
    );
    assert!(
        result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("regression")
    );
    assert_eq!(
        replies[3]["result"]["structuredContent"]["verdict"],
        "regression"
    );
}

#[test]
fn mcp_reports_stable_error_codes() {
    let tmp = tempfile::tempdir().unwrap();
    let (base, cap) = dirs(tmp.path());
    let replies = mcp(
        tmp.path(),
        &[
            call(
                1,
                "saccade_compare",
                json!({"baseline_dir": tmp.path().join("nope"), "capture_dir": cap, "out_dir": tmp.path().join("o")}),
            ),
            call(
                2,
                "saccade_compare",
                json!({"baseline_dir": base, "capture_dir": cap, "out_dir": base.join("report")}),
            ),
        ],
    );
    let first = &replies[0]["result"];
    assert_eq!(first["isError"], true);
    assert_eq!(first["structuredContent"]["code"], "io");
    let second = &replies[1]["result"];
    assert_eq!(second["isError"], true);
    assert_eq!(second["structuredContent"]["code"], "config");
}

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
    assert_eq!(v["schema"], "saccade-approve.v1");
    assert_eq!(v["copied"].as_array().unwrap().len(), 1);
    assert_eq!(v["copied"][0]["name"], "scene.png");
    assert_eq!(v["pruned"].as_array().unwrap().len(), 1);
    assert!(!base.join("gone.png").exists());
}
