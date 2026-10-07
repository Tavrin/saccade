#![allow(clippy::unwrap_used, missing_docs)]
use serde_json::{Value, json};
use std::{path::Path, process::Command};
fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(args)
        .output()
        .unwrap()
}
fn read(p: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()
}
#[test]
fn frame_pins_masks_calibration_and_external_scores() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let mut frames = Vec::new();
    for t in 0..8 {
        let file = format!("{t}.png");
        image::RgbaImage::from_fn(24, 32, |x, y| {
            let v = ((x * 23 + y * 41 + t * 7) % 256) as u8;
            image::Rgba([v, v, v, 255])
        })
        .save(dir.join(&file))
        .unwrap();
        frames.push(json!({"index":t,"timestamp_s":t as f64/30.0,"file":file}));
    }
    let map = dir.join("frames.json");
    let m = json!({"schema":"saccade-frame-map.v1","nominal_fps":30,"frames":frames});
    std::fs::write(&map, serde_json::to_vec(&m).unwrap()).unwrap();
    let mapstr = map.to_str().unwrap();
    let report = dir.join("report.json");
    let reportstr = report.to_str().unwrap();
    assert!(
        run(&[
            "experiment",
            "motion-stats",
            mapstr,
            mapstr,
            "--out",
            reportstr,
            "--json"
        ])
        .status
        .success()
    );
    let data = read(&report);
    assert_eq!(data["schema"], "saccade-motion-stats.v1");
    assert_eq!(data["distances"]["duplicate_fraction"], 0.0);
    let schema: Value =
        serde_json::from_str(saccade_core::schema_catalog::get("saccade-motion-stats.v1").unwrap())
            .unwrap();
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(&data)
        .unwrap();
    let bad = dir.join("bad.json");
    let mut wrong = m.clone();
    wrong["frames"][0]["sha256"] = "0".repeat(64).into();
    std::fs::write(&bad, serde_json::to_vec(&wrong).unwrap()).unwrap();
    assert!(
        !run(&[
            "experiment",
            "motion-stats",
            bad.to_str().unwrap(),
            mapstr,
            "--out",
            reportstr
        ])
        .status
        .success()
    );
    let mask = dir.join("mask.png");
    image::GrayImage::from_pixel(24, 32, image::Luma([0]))
        .save(&mask)
        .unwrap();
    let spec = format!("mask:{}", mask.display());
    assert!(
        !run(&[
            "experiment",
            "motion-stats",
            mapstr,
            mapstr,
            "--mask",
            &spec,
            "--out",
            reportstr
        ])
        .status
        .success()
    );
    let out = dir.join("calibration");
    let outstr = out.to_str().unwrap();
    assert!(
        run(&[
            "experiment",
            "calibrate-degradations",
            "--positive",
            mapstr,
            "--strengths",
            "1",
            "--out",
            outstr
        ])
        .status
        .success()
    );
    let manifest = read(&out.join("manifest.json"));
    let scores = read(&out.join("scores.json"));
    for score in scores.as_array().unwrap().iter().filter(|s| {
        s["scorer"] == "interval_cv" && (s["class"] == "speed_up" || s["class"] == "speed_down")
    }) {
        // Uniform time scaling leaves coefficient of variation invariant.
        assert_eq!(score["negative"], 0.0);
    }

    assert_eq!(manifest["entries"].as_array().unwrap().len(), 10);
    let report = read(&out.join("calibration.json"));
    assert!(
        report["scorers"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["trusted_for"].as_array().unwrap().is_empty())
    );
    for (id, file) in [
        ("saccade-motion-calibration.v1", "calibration.json"),
        ("saccade-motion-degradations.v1", "manifest.json"),
    ] {
        let schema: Value =
            serde_json::from_str(saccade_core::schema_catalog::get(id).unwrap()).unwrap();
        jsonschema::validator_for(&schema)
            .unwrap()
            .validate(&read(&out.join(file)))
            .unwrap();
    }
    assert!(
        !run(&[
            "experiment",
            "calibrate-degradations",
            "--positive",
            mapstr,
            "--strengths",
            "1",
            "--out",
            outstr
        ])
        .status
        .success()
    );
    let scores = dir.join("external.json");
    let entries: Vec<Value> = manifest["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| json!({"id":e["id"],"scorer":"example","positive":1,"negative":0}))
        .collect();
    let s = json!({"schema":"saccade-motion-scores.v1","manifest_sha256":report["manifest_sha256"],"scores":entries});
    std::fs::write(&scores, serde_json::to_vec(&s).unwrap()).unwrap();
    let again = dir.join("again");
    assert!(
        run(&[
            "experiment",
            "calibrate-degradations",
            "--positive",
            mapstr,
            "--strengths",
            "1",
            "--scores",
            scores.to_str().unwrap(),
            "--out",
            again.to_str().unwrap()
        ])
        .status
        .success()
    );
    let report = read(&again.join("calibration.json"));
    assert!(
        report["scorers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["scorer"] == "external:example")
    );
    let mut stale = s;
    stale["manifest_sha256"] = "0".repeat(64).into();
    std::fs::write(&scores, serde_json::to_vec(&stale).unwrap()).unwrap();
    let invalid = dir.join("invalid");
    assert!(
        !run(&[
            "experiment",
            "calibrate-degradations",
            "--positive",
            mapstr,
            "--strengths",
            "1",
            "--scores",
            scores.to_str().unwrap(),
            "--out",
            invalid.to_str().unwrap()
        ])
        .status
        .success()
    );
}
