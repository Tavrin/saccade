//! CLI, schema, MCP and optional-video production boundaries (two tests).
#![allow(missing_docs, clippy::unwrap_used, clippy::expect_used)]

use serde_json::{Value, json};
use std::io::Write;
use std::process::{Command, Stdio};

fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(args)
        .output()
        .unwrap()
}
fn schema(value: &Value, file: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas")
        .join(file);
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    jsonschema::validator_for(&doc)
        .unwrap()
        .validate(value)
        .unwrap();
}

#[test]
fn cli_artifacts_schemas_junit_and_root_confined_mcp() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let frames = root.join("frames");
    std::fs::create_dir(&frames).unwrap();
    for i in 0..64 {
        image::RgbImage::from_pixel(
            64,
            32,
            image::Rgb(if i % 8 < 4 { [255; 3] } else { [0; 3] }),
        )
        .save(frames.join(format!("frame_{i:03}.png")))
        .unwrap();
    }
    let out = root.join("safety");
    let xml = root.join("safety.xml");
    let result = cli(&[
        "safety",
        frames.to_str().unwrap(),
        "--fps",
        "32",
        "--out",
        out.to_str().unwrap(),
        "--json",
        "--junit",
        xml.to_str().unwrap(),
    ]);
    assert_eq!(
        result.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let value: Value = serde_json::from_slice(&result.stdout).unwrap();
    schema(&value, "saccade-safety.v1.schema.json");
    assert!(out.join("index.html").exists());
    assert!(out.join("report.txt").exists());
    assert!(std::fs::read_to_string(xml).unwrap().contains("<failure"));
    let picture = frames.join("frame_000.png");
    let aout = root.join("a11y");
    let a = cli(&[
        "a11y",
        picture.to_str().unwrap(),
        "--out",
        aout.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(a.status.code(), Some(0));
    schema(
        &serde_json::from_slice(&a.stdout).unwrap(),
        "saccade-a11y.v1.schema.json",
    );
    let bad = cli(&[
        "safety",
        frames.to_str().unwrap(),
        "--fps",
        "0",
        "--json",
        "--out",
        root.join("bad").to_str().unwrap(),
    ]);
    assert_eq!(bad.status.code(), Some(2));
    assert_eq!(
        serde_json::from_slice::<Value>(&bad.stdout).unwrap()["schema"],
        "saccade-error.v1"
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["mcp", "--root", root.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for msg in [
        json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"saccade_safety","arguments":{"input":"frames","fps":32,"out_dir":"mcp-safety"}}}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"saccade_a11y","arguments":{"input":"frames/frame_000.png","out_dir":"mcp-a11y"}}}),
        json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"saccade_safety","arguments":{"input":"../escape","out_dir":"mcp-unsafe"}}}),
    ] {
        writeln!(stdin, "{msg}").unwrap();
    }
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let replies: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert!(
        replies[0]["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["name"] == "saccade_a11y")
    );
    schema(
        &replies[1]["result"]["structuredContent"],
        "saccade-safety.v1.schema.json",
    );
    schema(
        &replies[2]["result"]["structuredContent"],
        "saccade-a11y.v1.schema.json",
    );
    assert_eq!(replies[3]["result"]["isError"], true);
    assert_eq!(
        replies[3]["result"]["structuredContent"]["code"],
        "unsafe_path"
    );
}

#[test]
fn video_metadata_and_missing_optional_ffmpeg() {
    let tmp = tempfile::tempdir().unwrap();
    let video = tmp.path().join("sample.mp4");
    // Exercise optional real decoding only on hosts with both tools installed.
    if Command::new("ffmpeg").arg("-version").output().is_ok()
        && Command::new("ffprobe").arg("-version").output().is_ok()
    {
        let generated = Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=64x32:r=24:d=0.5",
                "-c:v",
                "mpeg4",
            ])
            .arg(&video)
            .output()
            .unwrap();
        assert!(
            generated.status.success(),
            "{}",
            String::from_utf8_lossy(&generated.stderr)
        );
        let out = tmp.path().join("video-report");
        let result = cli(&[
            "safety",
            video.to_str().unwrap(),
            "--json",
            "--out",
            out.to_str().unwrap(),
        ]);
        assert_eq!(
            result.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let value: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(value["fps"], 24.0);
        schema(&value, "saccade-safety.v1.schema.json");
    } else {
        std::fs::write(&video, b"placeholder").unwrap();
    }
    let result = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .env("PATH", tmp.path())
        .args([
            "safety",
            video.to_str().unwrap(),
            "--fps",
            "24",
            "--json",
            "--out",
            tmp.path().join("missing").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&result.stdout).contains("ffmpeg on PATH"));
}
