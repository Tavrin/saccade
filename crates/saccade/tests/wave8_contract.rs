//! Generated command records; no models, video decoder or external providers.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use serde_json::{Value, json};
use std::{path::Path, process::Command};
fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(args)
        .output()
        .expect("CLI fixture")
}
fn schema(id: &str, value: &Value) {
    let successor = saccade_core::report_links::linked_schema(id);
    let id = if value["schema"] == successor {
        successor
    } else {
        id
    };
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../saccade-core/schemas");
    let s: Value =
        serde_json::from_slice(&std::fs::read(dir.join(format!("{id}.schema.json"))).unwrap())
            .unwrap();
    jsonschema::validator_for(&s)
        .unwrap()
        .validate(value)
        .unwrap();
}
#[test]
fn media_record_keyframes_and_usage_schema_end_to_end() {
    let d = tempfile::tempdir().unwrap();
    let image = d.path().join("image.png");
    image::RgbImage::from_fn(80, 64, |x, y| {
        image::Rgb([
            ((x * 7 + y * 5) % 256) as u8,
            ((x * 11) % 256) as u8,
            ((y * 13) % 256) as u8,
        ])
    })
    .save(&image)
    .unwrap();
    let file = image.to_str().unwrap();
    let out = run(&["analyze-media", file, "--output-size", "400x300", "--json"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let r: Value = serde_json::from_slice(&out.stdout).unwrap();
    schema("saccade-media-record.v1", &r);
    assert_eq!(
        r["quality"]["data"]["fitness"][0]["adequate_resolution"],
        false
    );
    let record = d.path().join("record.json");
    std::fs::write(&record, &out.stdout).unwrap();
    let out = run(&["find-usage", record.to_str().unwrap(), file, "--json"]);
    assert!(out.status.success());
    let r: Value = serde_json::from_slice(&out.stdout).unwrap();
    schema("saccade-usage.v1", &r);
    assert_eq!(r["targets"][0]["basis"], "encoded-content-equality");
    let frames = d.path().join("frames");
    std::fs::create_dir(&frames).unwrap();
    for i in 0..3 {
        std::fs::copy(&image, frames.join(format!("{i:03}.png"))).unwrap();
    }
    let selected = d.path().join("selected");
    let out = run(&[
        "keyframes",
        frames.to_str().unwrap(),
        "--out",
        selected.to_str().unwrap(),
        "--json",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let r: Value = serde_json::from_slice(&out.stdout).unwrap();
    schema("saccade-keyframes.v1", &r);
    assert_eq!(r["keyframes"].as_array().unwrap().len(), 1);
    let out = run(&["analyze-media", frames.to_str().unwrap(), "--json"]);
    assert!(out.status.success());
    let r: Value = serde_json::from_slice(&out.stdout).unwrap();
    schema("saccade-media-record.v1", &r);
    assert_eq!(r["video"]["status"], "ok");
}
#[test]
fn strict_json_error_code_remains_visible() {
    let d = tempfile::tempdir().unwrap();
    let image = d.path().join("image.png");
    image::RgbImage::from_pixel(8, 8, image::Rgb([40, 40, 40]))
        .save(&image)
        .unwrap();
    let opts = d.path().join("options.json");
    std::fs::write(
        &opts,
        serde_json::to_vec(&json!({"description":true,"strict":true})).unwrap(),
    )
    .unwrap();
    let out = run(&[
        "analyze-media",
        image.to_str().unwrap(),
        "--options",
        opts.to_str().unwrap(),
        "--json",
    ]);
    assert!(!out.status.success());
    let r: Value = serde_json::from_slice(&out.stdout).unwrap();
    schema("saccade-result.v2", &r);
    assert_eq!(r["errors"][0]["code"], "media_section_failed");
}
#[cfg(feature = "mcp")]
#[test]
fn mcp_analyze_media_matches_core_and_refuses_root_escape() {
    use std::io::Write;
    let d = tempfile::tempdir().unwrap();
    let image = d.path().join("image.png");
    image::RgbImage::from_pixel(8, 8, image::Rgb([40, 50, 60]))
        .save(&image)
        .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["mcp", "--root", d.path().to_str().unwrap()])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"saccade_measure","arguments":{"operation":"analyze_media","image":image}}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"saccade_measure","arguments":{"operation":"analyze_media","image":"/outside/image.png"}}}),
    ];
    let mut stdin = child.stdin.take().unwrap();
    for request in requests {
        writeln!(stdin, "{request}").unwrap();
    }
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let values: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(values.len(), 2);
    let content = &values[0]["result"]["structuredContent"];
    schema("saccade-media-record.v1", content);
    assert_eq!(values[1]["result"]["isError"], true);
}

#[test]
fn compact_index_query_has_its_own_validated_schema() {
    let model = serde_json::json!({"schema":"saccade-embedding-model.v1","family":"dinov2-small","artifact":{"role":"embedding","version":"a".repeat(40),"format":"onnx","url":format!("https://example.org/{}/model.onnx","a".repeat(40)),"bytes":1,"sha256":"a".repeat(64),"license":"Apache-2.0"},"input":"pixels","output":"embedding","size":[8,8],"mean":[0.,0.,0.],"std":[1.,1.,1.],"dimensions":2,"calibration":null});
    let mut index = saccade_core::media::search::Index::new(model).unwrap();
    index
        .add_vector("generated.png", &"b".repeat(64), vec![1., 0.])
        .unwrap();
    let query = index.query_vector(&[1., 0.], 1, "vector").unwrap();
    schema("saccade-media-index-query.v1", &query);
    assert_eq!(query["hits"][0]["row_index"], 0);
    assert_ne!(
        query["schema"],
        saccade_core::general::embedding::QUERY_SCHEMA
    );
}
