//! Exercise sidecar comparability through JSON, text and Markdown consumers.
#![allow(clippy::unwrap_used, missing_docs)]

use std::process::Command;

use image::{Rgb, RgbImage};
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_saccade");

#[test]
fn timing_names_pair_exactly_and_one_sided_keys_reach_all_outputs() {
    let tmp = tempfile::tempdir().unwrap();
    let (base, cap, out) = (
        tmp.path().join("base"),
        tmp.path().join("cap"),
        tmp.path().join("report"),
    );
    for dir in [&base, &cap] {
        std::fs::create_dir_all(dir).unwrap();
        RgbImage::from_pixel(8, 8, Rgb([40, 60, 80]))
            .save(dir.join("scene.png"))
            .unwrap();
    }
    let baseline = base.join("saccade-meta.json");
    let capture = cap.join("saccade-meta.json");
    std::fs::write(&baseline, r#"{"gpu_ms":4,"frame_ms":16,"driver":"a"}"#).unwrap();
    std::fs::write(&capture, r#"{"GPU_MS":2,"frame_ms":12,"driver":"b"}"#).unwrap();
    for (remove, expected, paired) in [
        (
            None,
            json!([{"key":"GPU_MS","side":"capture"},{"key":"gpu_ms","side":"baseline"}]),
            1,
        ),
        (
            Some(&baseline),
            json!([{"key":"GPU_MS","side":"capture"},{"key":"frame_ms","side":"capture"}]),
            0,
        ),
        (Some(&capture), json!([]), 0),
    ] {
        if let Some(path) = remove {
            std::fs::remove_file(path).unwrap();
        }
        let run = Command::new(BIN)
            .arg("compare")
            .arg(&base)
            .arg(&cap)
            .arg("--out")
            .arg(&out)
            .output()
            .unwrap();
        assert_eq!(run.status.code(), Some(0), "{:?}", run);
        let report = out.join("saccade-report.v1.json");
        let v: Value = serde_json::from_slice(&std::fs::read(&report).unwrap()).unwrap();
        let d = &v["entries"][0]["diagnostics"];
        assert_eq!(d["perf"].as_array().unwrap().len(), paired);
        if paired > 0 {
            assert_eq!(d["perf"][0]["key"], "frame_ms");
        }
        assert_eq!(
            d.get("perf_not_comparable").cloned().unwrap_or(json!([])),
            expected
        );
        let summary = Command::new(BIN)
            .arg("summary")
            .arg(&report)
            .args(["--format", "markdown"])
            .output()
            .unwrap();
        assert_eq!(summary.status.code(), Some(0));
        let keys = expected
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["key"].as_str().unwrap())
            .collect::<Vec<_>>();
        for output in [&run.stdout, &summary.stdout] {
            let text = String::from_utf8_lossy(output);
            if keys.is_empty() {
                assert!(!text.contains("not comparable:"));
            } else {
                assert!(
                    text.contains(&format!("not comparable: {}", keys.join(", "))),
                    "{text}"
                );
            }
        }
    }
}
