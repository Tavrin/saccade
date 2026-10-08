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
    let expected = file.trim_end_matches(".schema.json");
    let successor = saccade_core::report_links::linked_schema(expected);
    let file = if value["schema"] == successor {
        format!("{successor}.schema.json")
    } else {
        file.into()
    };
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../saccade-core/schemas")
        .join(&file);
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
        "experiment",
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
    let value: Value =
        serde_json::from_slice(&std::fs::read(out.join("saccade-safety.v1.json")).unwrap())
            .unwrap();
    schema(&value, "saccade-safety.v1.schema.json");
    assert!(out.join("index.html").exists());
    assert!(out.join("report.txt").exists());
    assert!(std::fs::read_to_string(xml).unwrap().contains("<failure"));
    let picture = frames.join("frame_000.png");
    let aout = root.join("a11y");
    let a = cli(&[
        "experiment",
        "a11y",
        picture.to_str().unwrap(),
        "--out",
        aout.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(a.status.code(), Some(0));
    schema(
        &serde_json::from_slice(&std::fs::read(aout.join("saccade-a11y.v1.json")).unwrap())
            .unwrap(),
        "saccade-a11y.v1.schema.json",
    );
    let bad = cli(&[
        "experiment",
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
        saccade_core::report_links::linked_schema("saccade-result.v2")
    );
    let outputs = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["mcp", "--root", root.to_str().unwrap()])
        .arg("--out-root")
        .arg(outputs.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for msg in [
        json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"saccade_measure","arguments":{"operation":"safety","input":"frames","fps":32,"out":"mcp-safety"}}}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"saccade_measure","arguments":{"operation":"a11y","input":"frames/frame_000.png","out":"mcp-a11y"}}}),
        json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"saccade_measure","arguments":{"operation":"safety","input":"../escape","out":"mcp-unsafe"}}}),
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
            .any(|t| t["name"] == "saccade_measure")
    );
    schema(
        &replies[1]["result"]["structuredContent"],
        "saccade-result.v2.schema.json",
    );
    schema(
        &replies[2]["result"]["structuredContent"],
        "saccade-result.v2.schema.json",
    );
    assert_eq!(replies[3]["result"]["isError"], true);
    assert_eq!(
        replies[3]["result"]["structuredContent"]["errors"][0]["code"],
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
            "experiment",
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
        let value: Value =
            serde_json::from_slice(&std::fs::read(out.join("saccade-safety.v1.json")).unwrap())
                .unwrap();
        assert_eq!(value["fps"], 24.0);
        schema(&value, "saccade-safety.v1.schema.json");
    } else {
        std::fs::write(&video, b"placeholder").unwrap();
    }
    let result = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .env("PATH", tmp.path())
        .args([
            "experiment",
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

#[test]
fn automatic_cli_directory_schema_junit_and_mcp_mirror() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let input = root.join("captures");
    std::fs::create_dir(&input).unwrap();
    image::RgbImage::from_pixel(80, 50, image::Rgb([255; 3]))
        .save(input.join("blank.png"))
        .unwrap();
    image::RgbImage::from_fn(60, 50, |x, y| {
        image::Rgb(
            if (10..40).contains(&x)
                && (10..40).contains(&y)
                && (x == 10 || x == 39 || y == 10 || y == 39)
            {
                [100; 3]
            } else {
                [255; 3]
            },
        )
    })
    .save(input.join("outline.png"))
    .unwrap();
    let out = root.join("auto-report");
    let junit = root.join("auto.xml");
    let result = cli(&[
        "a11y",
        "auto",
        input.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--json",
        "--junit",
        junit.to_str().unwrap(),
        "--level",
        "AAA",
    ]);
    assert_eq!(
        result.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let value: Value =
        serde_json::from_slice(&std::fs::read(out.join("saccade-auto-a11y.v1.json")).unwrap())
            .unwrap();
    schema(&value, "saccade-auto-a11y.v1.schema.json");
    assert_eq!(value["verdict"], "UNMEASURABLE");
    assert_eq!(value["level"], "AAA");
    assert!(
        value["images"][0]["text_detection"]
            .as_str()
            .unwrap()
            .starts_with("no text detected")
    );
    assert!(std::fs::read_to_string(junit).unwrap().contains("<skipped"));
    let bad = cli(&[
        "a11y",
        "auto",
        input.to_str().unwrap(),
        "--out",
        root.join("bad-auto").to_str().unwrap(),
        "--px-per-pt",
        "0",
    ]);
    assert_eq!(bad.status.code(), Some(2));
    let outputs = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["mcp", "--root", root.to_str().unwrap()])
        .arg("--out-root")
        .arg(outputs.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for msg in [
        json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"saccade_measure","arguments":{"operation":"a11y_auto","input":"captures","out":"automatic","level":"AAA"}}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"saccade_measure","arguments":{"operation":"a11y_auto","input":"captures","out":"../escape"}}}),
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
    assert_ne!(replies[0]["result"]["isError"], true, "{}", replies[0]);
    assert_eq!(replies[1]["result"]["isError"], true);
    let artifact: Value = serde_json::from_slice(
        &std::fs::read(outputs.path().join("automatic/saccade-auto-a11y.v1.json")).unwrap(),
    )
    .unwrap();
    schema(&artifact, "saccade-auto-a11y.v1.schema.json");
    assert_eq!(artifact, value);
    let doc: Value =
        serde_json::from_str(saccade_core::schema_catalog::get("saccade-auto-a11y.v1").unwrap())
            .unwrap();
    let validator = jsonschema::validator_for(&doc).unwrap();
    let mut impossible = value.clone();
    impossible["verdict"] = json!("PASS");
    impossible["images"] = json!([]);
    assert!(!validator.is_valid(&impossible));
    impossible = value.clone();
    impossible["images"][0]["verdict"] = json!("PASS");
    assert!(!validator.is_valid(&impossible));
    impossible = value.clone();
    impossible["images"][1]["automatic"][0]["verdict"] = json!("PASS");
    impossible["images"][1]["automatic"][0]["ratio"] = Value::Null;
    assert!(!validator.is_valid(&impossible));
    impossible = value.clone();
    impossible["images"][1]["automatic"][0]["region"]["rect_px"][2] = json!(0);
    assert!(!validator.is_valid(&impossible));
}
