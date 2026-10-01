//! End-to-end CLI/MCP contracts for sequences, ranking and numerical buffers.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use image::{Rgb, RgbImage};
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_saccade");

fn fixtures(root: &Path) -> (PathBuf, Vec<PathBuf>) {
    let reference = root.join("reference");
    let candidates: Vec<_> = ["same", "small", "large"]
        .iter()
        .map(|n| root.join(n))
        .collect();
    for (dir, shade) in
        std::iter::once((&reference, 64)).chain(candidates.iter().zip([64, 80, 180]))
    {
        std::fs::create_dir_all(dir).unwrap();
        for n in [2, 10] {
            RgbImage::from_pixel(32, 32, Rgb([shade, shade, shade]))
                .save(dir.join(format!("frame_{n}.png")))
                .unwrap();
        }
    }
    (reference, candidates)
}

fn validate(schema: &Value, v: &Value) {
    let check = jsonschema::validator_for(schema).unwrap();
    let errors: Vec<_> = check.iter_errors(v).map(|e| e.to_string()).collect();
    assert!(errors.is_empty(), "{errors:?}: {v}");
}

fn schema(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../schemas/saccade-{name}.v1.schema.json"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn cli_sequence_rank_and_buffer_outputs_validate_against_shipped_schemas() {
    let tmp = tempfile::tempdir().unwrap();
    let (reference, candidates) = fixtures(tmp.path());
    for mode in ["--json", "--json=full"] {
        for (command, args) in [
            ("sequence", vec![reference.clone(), candidates[2].clone()]),
            (
                "rank",
                std::iter::once(reference.clone())
                    .chain(candidates.clone())
                    .collect(),
            ),
        ] {
            let out = tmp.path().join(command);
            let result = Command::new(BIN)
                .arg(command)
                .args(args)
                .args(["--out"])
                .arg(&out)
                .arg(mode)
                .output()
                .unwrap();
            assert_eq!(
                result.status.code(),
                Some(1),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let v: Value = serde_json::from_slice(&result.stdout).unwrap();
            assert_eq!(v["schema"], format!("saccade-{command}.v1"));
            validate(&schema(command), &v);
            let disk: Value = serde_json::from_str(
                &std::fs::read_to_string(out.join(format!("saccade-{command}.v1.json"))).unwrap(),
            )
            .unwrap();
            validate(&schema(command), &disk);
            if mode == "--json" {
                assert!(v.get("frames").is_none() && v.get("images").is_none());
            }
        }
    }
    let config = tmp.path().join("buffer.toml");
    std::fs::write(
        &config,
        "[[buffer]]\nglob='*'\nkind='mask'\nthreshold=0.0\n",
    )
    .unwrap();
    let out = tmp.path().join("buffer");
    let result = Command::new(BIN)
        .arg("compare")
        .arg(&reference)
        .arg(&candidates[1])
        .arg("--out")
        .arg(out)
        .arg("--config")
        .arg(config)
        .arg("--json=full")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    let v: Value = serde_json::from_slice(&result.stdout).unwrap();
    validate(&schema("report"), &v);
    assert_eq!(v["entries"][0]["buffer"]["stats"]["changed_pixels"], 1024);
    assert!(v["entries"][0]["metrics"].is_null());
    let result = Command::new(BIN)
        .arg("compare")
        .arg(&reference)
        .arg(&candidates[1])
        .arg("--out")
        .arg(tmp.path().join("buffer"))
        .arg("--config")
        .arg(tmp.path().join("buffer.toml"))
        .arg("--json")
        .output()
        .unwrap();
    let lean: Value = serde_json::from_slice(&result.stdout).unwrap();
    validate(&schema("result"), &lean);
    assert_eq!(lean["failing"][0]["buffer"]["unit"], "changed_fraction");
}

#[test]
fn mcp_sequence_rank_are_lean_schema_valid_and_confined_to_the_root() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    let (reference, candidates) = fixtures(&root);
    let call = |id, name, args| json!({"jsonrpc":"2.0", "id":id, "method":"tools/call", "params":{"name":name,"arguments":args}});
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut requests = vec![
        json!({"jsonrpc":"2.0","id":0,"method":"tools/list"}),
        call(
            1,
            "saccade_sequence",
            json!({"baseline_dir":reference, "capture_dir":candidates[2], "out_dir":"seq"}),
        ),
        call(
            2,
            "saccade_rank",
            json!({"reference_dir":reference, "candidate_dirs":candidates, "out_dir":"rank"}),
        ),
        call(
            3,
            "saccade_sequence",
            json!({"baseline_dir":"../outside", "capture_dir":candidates[0], "out_dir":"out"}),
        ),
        call(
            4,
            "saccade_rank",
            json!({"reference_dir":reference, "candidate_dirs":[candidates[0]], "out_dir":"../outside"}),
        ),
        call(
            5,
            "saccade_rank",
            json!({"reference_dir":reference, "candidate_dirs":["../outside"], "out_dir":"out"}),
        ),
        call(
            6,
            "saccade_sequence",
            json!({"baseline_dir":reference, "capture_dir":candidates[0], "out_dir":"out", "config":"../outside.toml"}),
        ),
    ];
    #[cfg(unix)]
    {
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("escape")).unwrap();
        requests.push(call(7, "saccade_rank", json!({"reference_dir":reference, "candidate_dirs":[candidates[0]], "out_dir":"escape"})));
    }
    let mut child = Command::new(BIN)
        .arg("mcp")
        .arg("--root")
        .arg(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for request in requests {
        writeln!(stdin, "{request}").unwrap();
    }
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    let replies: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    for (i, name) in [(1, "sequence"), (2, "rank")] {
        let result = &replies[i]["result"];
        assert_eq!(result["isError"], false, "{result}");
        validate(&schema(name), &result["structuredContent"]);
        let tool = replies[0]["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == format!("saccade_{name}"))
            .unwrap();
        validate(&tool["outputSchema"], &result["structuredContent"]);
        assert_eq!(tool["annotations"]["destructiveHint"], false);
        assert_eq!(tool["annotations"]["idempotentHint"], true);
    }
    for reply in replies.iter().skip(3) {
        assert_eq!(reply["result"]["isError"], true);
        assert_eq!(reply["result"]["structuredContent"]["code"], "unsafe_path");
    }
}
