//! `flipdiff_compare_runs`: the run overview as an MCP tool.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use image::{Rgb, RgbImage};
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_flipdiff");

fn save(dir: &Path, name: &str, grey: u8) {
    std::fs::create_dir_all(dir).expect("mkdir");
    RgbImage::from_pixel(64, 48, Rgb([grey, grey, grey]))
        .save(dir.join(name))
        .expect("save");
}

/// Sends `requests` to `flipdiff mcp --root root` and returns the replies.
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

#[test]
fn mcp_compare_runs_summarises_the_matrix_and_keeps_the_root_policy() {
    let tmp = tempfile::tempdir().unwrap();
    let (base, same, edit) = (
        tmp.path().join("base"),
        tmp.path().join("same"),
        tmp.path().join("edit"),
    );
    for (name, grey) in [("a.png", 40), ("b.png", 120)] {
        save(&base, name, grey);
        save(&same, name, grey);
    }
    save(&edit, "a.png", 40);
    save(&edit, "b.png", 220);
    let call = |id: u32, args: Value| {
        json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
               "params": {"name": "flipdiff_compare_runs", "arguments": args}})
    };
    let replies = mcp(
        tmp.path(),
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}),
            call(2, json!({"ref_dir": base, "run_dirs": [same, edit]})),
            call(3, json!({"ref_dir": "/", "run_dirs": [same]})),
        ],
    );
    let tool = replies[0]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "flipdiff_compare_runs")
        .expect("the tool is listed");
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    assert_eq!(
        tool["outputSchema"]["properties"]["schema"]["const"],
        "flipdiff-runs.v1"
    );

    let ok = &replies[1]["result"];
    assert_eq!(ok["isError"], false, "{ok}");
    let s = &ok["structuredContent"];
    assert_eq!(s["schema"], "flipdiff-runs.v1");
    assert_eq!(s["runs"][0]["label"], "same");
    assert_eq!(s["runs"][0]["no_visible_effect"], true);
    assert_eq!(s["runs"][0]["identical"], 2);
    assert_eq!(s["runs"][1]["changed"], 1);
    assert_eq!(s["runs"][1]["worst"]["name"], "b.png");
    assert_eq!(s["images"][1]["cells"][0]["status"], "identical");
    assert_eq!(s["images"][1]["cells"][1]["status"], "changed");
    assert!(
        ok["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("No visible effect")
    );

    let refused = &replies[2]["result"];
    assert_eq!(refused["isError"], true);
    assert_eq!(refused["structuredContent"]["code"], "unsafe_path");
}
