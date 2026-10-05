#![allow(clippy::unwrap_used, missing_docs)]
use saccade_core::localized::digest;
use saccade_core::renderdoc::{Action, Capture, Resource};
use std::process::Command;
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
