#![allow(clippy::unwrap_used, missing_docs)]
use saccade_core::localized::digest;
use saccade_core::renderdoc::{Action, Capture, Resource};
use std::process::{Command, Output};
#[test]
fn extraction_payload_hashes_are_checked_before_localization() {
    let temp = tempfile::tempdir().unwrap();
    let mut paths = vec![];
    for (side, value, event) in [("base", 42u8, 2), ("candidate", 43u8, 7)] {
        let root = temp.path().join(side);
        std::fs::create_dir(&root).unwrap();
        let data = vec![value; 16 * 16 * 4];
        std::fs::write(root.join("raw.bin"), &data).unwrap();
        let capture = Capture {
            schema: "saccade-renderdoc-extract.v1".into(),
            capture_sha256: digest(side.as_bytes()),
            worker_sha256: digest(b"synthetic-worker"),
            renderdoc_version: "1.34".into(),
            api: "Vulkan".into(),
            replay_mode: "conservative".into(),
            repeatability: "self_replay_byte_identical".into(),
            limits: vec!["Synthetic capture-shaped fixture; no replay executed".into()],
            actions: vec![Action {
                event_id: event,
                marker_path: vec!["lighting#0".into()],
                marker_unique: true,
                kind: "draw".into(),
                action_key: "sphere".into(),
                candidate_inputs: vec![format!("capture-local-input-{side}")],
                resources: vec![Resource {
                    role: "color0".into(),
                    resource_id: format!("capture-local-output-{side}"),
                    format: "RGBA8".into(),
                    dimensions: [16, 16, 1],
                    subresource: [0, 0, 0],
                    interpretation: "native_bytes".into(),
                    payload: Some("raw.bin".into()),
                    sha256: Some(digest(&data)),
                    bytes: Some(data.len() as u64),
                    error: None,
                }],
            }],
        };
        let path = root.join("extraction.json");
        std::fs::write(&path, serde_json::to_vec(&capture).unwrap()).unwrap();
        paths.push(path);
    }
    let out = temp.path().join("localized.json");
    let output = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .arg("renderdoc-localize")
        .args(&paths)
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let report: serde_json::Value = serde_json::from_slice(&std::fs::read(out).unwrap()).unwrap();
    assert_eq!(report["first_observed_divergence"]["baseline_event"], 2);
    assert_eq!(report["first_observed_divergence"]["candidate_event"], 7);
    assert_eq!(report["root_cause"], "unproven");
    std::fs::write(temp.path().join("candidate/raw.bin"), vec![44; 1024]).unwrap();
    let out = temp.path().join("rejected.json");
    let output = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .arg("renderdoc-localize")
        .args(&paths)
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(!out.exists());
}

use serde_json::{Value, json};
use std::path::Path;
fn cli(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_saccade"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}

fn extraction() -> Value {
    json!({"schema":"saccade-renderdoc-extract.v1","capture_sha256":"a".repeat(64),"worker_sha256":"b".repeat(64),"renderdoc_version":"1.34","api":"Vulkan","replay_mode":"conservative","repeatability":"self_replay_byte_identical","limits":[],"actions":[{"event_id":1,"marker_path":["marker"],"marker_unique":true,"kind":"draw","action_key":"draw","candidate_inputs":[],"resources":[{"role":"color0","resource_id":"resource","format":"RGBA8","dimensions":[1,1,1],"subresource":[0,0,0],"interpretation":"native_bytes","payload":"raw.bin","sha256":saccade_core::localized::digest(b"raw"),"bytes":3,"error":null}]}]})
}
#[test]
fn bare_filenames_work_for_quality_and_renderdoc() {
    let t = tempfile::tempdir().unwrap();
    std::fs::write(t.path().join("raw.bin"), b"raw").unwrap();
    std::fs::write(t.path().join("extraction.json"), extraction().to_string()).unwrap();
    let p = cli(
        t.path(),
        &[
            "renderdoc-localize",
            "extraction.json",
            "extraction.json",
            "--out",
            "local.json",
        ],
    );
    assert!(p.status.success(), "{p:?}");
    image::RgbImage::from_pixel(32, 32, image::Rgb([50, 60, 70]))
        .save(t.path().join("image.png"))
        .unwrap();
    let artifact = json!({"path":"image.png","sha256":saccade_core::localized::digest(&std::fs::read(t.path().join("image.png")).unwrap())});
    let m = json!({"schema":"saccade-quality-sweep.v1","original":artifact,"reference":artifact,"minimum_score":90.0,"maximum_bytes":10000,"viewing_conditions":"sRGB,100%,60cm,review pending","candidates":[{"id":"candidate","stages":[{"id":"delivery","encoder":"identity","quality":100.0,"subsampling":"4:4:4","pixel_policy":"normalized_srgb_opaque","dimensions":[32,32],"output":artifact}]}]});
    std::fs::write(t.path().join("sweep.json"), m.to_string()).unwrap();
    let p = cli(
        t.path(),
        &["quality-sweep", "sweep.json", "--out", "quality.json"],
    );
    assert!(p.status.success(), "{p:?}");
}
#[cfg(unix)]
#[test]
fn renderdoc_payload_fifo_is_rejected_without_blocking() {
    let t = tempfile::tempdir().unwrap();
    std::fs::write(t.path().join("extraction.json"), extraction().to_string()).unwrap();
    assert!(
        Command::new("mkfifo")
            .arg(t.path().join("raw.bin"))
            .status()
            .unwrap()
            .success()
    );
    let script = r#"import subprocess,sys
try:
 p=subprocess.run([sys.argv[1],'renderdoc-localize',sys.argv[2],sys.argv[2],'--out','out.json'],capture_output=True,timeout=2)
 assert p.returncode == 2 and b'regular file' in p.stderr, p
except subprocess.TimeoutExpired:
 raise AssertionError('payload FIFO blocked')
"#;
    let p = Command::new("python3")
        .args(["-c", script])
        .arg(env!("CARGO_BIN_EXE_saccade"))
        .arg(t.path().join("extraction.json"))
        .current_dir(t.path())
        .output()
        .unwrap();
    assert!(p.status.success(), "{p:?}");
}
