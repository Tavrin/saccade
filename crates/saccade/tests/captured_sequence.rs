//! Same capture command across constructed object-detail and progressive raster cases.
#![cfg(feature = "graphics")]
#![allow(clippy::unwrap_used, missing_docs)]
use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use std::{path::Path, process::Command};
fn run(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_saccade"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn specimen(domain: usize, detail: u8, frame: usize) -> RgbaImage {
    RgbaImage::from_fn(32, 32, |x, y| {
        let inside = if domain == 0 {
            (i64::from(x) - 16).pow(2) + (i64::from(y) - 16).pow(2) < 100
        } else {
            (8..24).contains(&x) && (8..24).contains(&y)
        };
        let base = 40 + frame as u8;
        let value = if inside && ((x + y) % 2 == 0) {
            detail
        } else {
            base
        };
        Rgba([value, value, value, 255])
    })
}
fn fixture(root: &Path, domain: usize, fade: bool) {
    let mut candidate = vec![];
    let mut reference = vec![];
    for i in 0..9 {
        let detail = if i < 3 {
            220
        } else if fade {
            220_u16.saturating_sub(((i - 2) * 50) as u16).max(20) as u8
        } else {
            20
        };
        let a = format!("candidate_{i}.png");
        let b = format!("reference_{i}.png");
        specimen(domain, detail, i).save(root.join(&a)).unwrap();
        specimen(domain, 20, i).save(root.join(&b)).unwrap();
        candidate.push(json!({"image":a,"timestamp_ms":i*100}));
        reference.push(json!({"image":b,"timestamp_ms":i*100}));
    }
    std::fs::write(root.join("plan.json"),serde_json::to_vec(&json!({"schema":"saccade-captured-sequence-plan.v1","candidate":candidate,"reference":reference,"masks":null,"id_buffers":null,"include_ids":[],"alignment":"none"})).unwrap()).unwrap();
}
fn transition(root: &Path) -> std::process::Output {
    run(
        root,
        &[
            "experiment",
            "transition",
            "plan.json",
            "--change-frame",
            "3",
            "--maximum-pop",
            "0.08",
            "--maximum-duration-ms",
            "400",
            "--maximum-steady-error",
            "0.001",
            "--settle-threshold",
            "0.001",
            "--tile-size",
            "8",
            "--out",
            "report",
            "--json",
        ],
    )
}
fn validate(value: &Value) {
    let id = value["schema"].as_str().unwrap();
    let schema: Value =
        serde_json::from_str(saccade_core::schema_catalog::get(id).unwrap()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(
        validator.is_valid(value),
        "{:?}",
        validator
            .iter_errors(value)
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
    );
}
#[test]
fn cross_domain_detail_pop_and_progressive_tile_fade_use_the_same_command() {
    for domain in 0..2 {
        for fade in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            fixture(dir.path(), domain, fade);
            let result = transition(dir.path());
            assert_eq!(
                result.status.code(),
                Some(if fade { 0 } else { 1 }),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let value: Value = serde_json::from_slice(&result.stdout).unwrap();
            validate(&value);
            assert_eq!(value["settle_frame"], if fade { 6 } else { 3 });
            assert_eq!(
                value["transition_duration_ms"],
                if fade { 300.0 } else { 0.0 }
            );
            if !fade {
                assert_eq!(value["pop_frame"], 3);
            }
            assert_eq!(value["steady_state_difference"], 0.0);
            assert!(value["sources"].as_object().unwrap().len() >= 19);
            let disk: Value = serde_json::from_slice(
                &std::fs::read(dir.path().join("report/saccade-transition.v1.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(disk, value);
        }
    }
}
#[test]
fn animation_glitch_and_correct_capture_preserve_first_divergence() {
    for glitch in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        fixture(dir.path(), 0, false);
        let mut plan: Value =
            serde_json::from_slice(&std::fs::read(dir.path().join("plan.json")).unwrap()).unwrap();
        plan["candidate"] = plan["reference"].clone();
        plan["alignment"] = "translation".into();
        if glitch {
            let mut image = specimen(0, 20, 4);
            for y in 10..18 {
                for x in 10..18 {
                    image.put_pixel(x, y, Rgba([240, 20, 20, 255]));
                }
            }
            image.save(dir.path().join("glitch.png")).unwrap();
            plan["candidate"][4]["image"] = "glitch.png".into();
        }
        std::fs::write(
            dir.path().join("plan.json"),
            serde_json::to_vec(&plan).unwrap(),
        )
        .unwrap();
        let result = run(
            dir.path(),
            &[
                "experiment",
                "animation",
                "plan.json",
                "--maximum-frame-error",
                "0.01",
                "--maximum-local-error",
                "0.08",
                "--maximum-flicker",
                "0.04",
                "--tile-size",
                "8",
                "--out",
                "report",
                "--json",
            ],
        );
        assert_eq!(
            result.status.code(),
            Some(i32::from(glitch)),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let value: Value = serde_json::from_slice(&result.stdout).unwrap();
        validate(&value);
        assert_eq!(
            value["first_divergence_frame"],
            if glitch { json!(4) } else { Value::Null }
        );
        if glitch {
            assert!(!value["frames"][4]["regions"].as_array().unwrap().is_empty());
        } else {
            assert_eq!(value["temporal_flicker"], 0.0);
        }
    }
}
#[test]
fn masks_and_id_buffers_freeze_nonempty_scope_and_missing_files_fail() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path(), 1, false);
    image::GrayImage::from_pixel(32, 32, image::Luma([255]))
        .save(dir.path().join("mask.png"))
        .unwrap();
    image::ImageBuffer::from_pixel(32, 32, image::Luma([513_u16]))
        .save(dir.path().join("ids.png"))
        .unwrap();
    let mut plan: Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("plan.json")).unwrap()).unwrap();
    // A legal image named plan must not overwrite the encoded plan identity.
    std::fs::rename(dir.path().join("candidate_0.png"), dir.path().join("plan")).unwrap();
    plan["candidate"][0]["image"] = "plan".into();
    plan["masks"] = json!(vec!["mask.png"; 9]);
    plan["id_buffers"] = json!(vec!["ids.png"; 9]);
    plan["include_ids"] = json!([513]);
    std::fs::write(
        dir.path().join("plan.json"),
        serde_json::to_vec(&plan).unwrap(),
    )
    .unwrap();
    let output = transition(dir.path());
    assert_eq!(output.status.code(), Some(1));
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value["sources"]["plan"],
        saccade_core::localized::digest(&std::fs::read(dir.path().join("plan.json")).unwrap())
    );
    assert_eq!(
        value["sources"]["input:plan"],
        saccade_core::localized::digest(&std::fs::read(dir.path().join("plan")).unwrap())
    );
    std::fs::remove_file(dir.path().join("candidate_4.png")).unwrap();
    let result = transition(dir.path());
    assert_eq!(result.status.code(), Some(2));
}
#[test]
fn declared_timestamp_mismatch_and_symlink_escape_are_usage_errors() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path(), 0, false);
    let mut plan: Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("plan.json")).unwrap()).unwrap();
    plan["reference"][3]["timestamp_ms"] = json!(301);
    std::fs::write(
        dir.path().join("plan.json"),
        serde_json::to_vec(&plan).unwrap(),
    )
    .unwrap();
    assert_eq!(transition(dir.path()).status.code(), Some(2));
    #[cfg(unix)]
    {
        fixture(dir.path(), 0, false);
        let outside = tempfile::tempdir().unwrap();
        specimen(0, 20, 0)
            .save(outside.path().join("outside.png"))
            .unwrap();
        std::fs::remove_file(dir.path().join("candidate_0.png")).unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("outside.png"),
            dir.path().join("candidate_0.png"),
        )
        .unwrap();
        assert_eq!(transition(dir.path()).status.code(), Some(2));
    }
}
