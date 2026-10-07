//! Procedural CC0 raster and source fixtures; no models, downloads or providers.
#![allow(clippy::unwrap_used)]
use serde_json::{Value, json};
use std::{path::Path, process::Command};
fn cli(root: &Path, args: &[&str], exit: i32) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .current_dir(root)
        .args(args)
        .arg("--json")
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(exit),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn put(root: &Path, name: &str, value: Value) {
    std::fs::write(root.join(name), serde_json::to_vec(&value).unwrap()).unwrap();
}
fn compare(root: &Path, changed: bool) {
    for name in ["a", "b"] {
        std::fs::create_dir(root.join(name)).unwrap();
        image::RgbaImage::from_pixel(
            32,
            32,
            image::Rgba([if changed && name == "b" { 0 } else { 255 }, 255, 255, 255]),
        )
        .save(root.join(name).join("image.png"))
        .unwrap();
    }
    put(
        root,
        "recipe.json",
        json!({"schema":"saccade-replay-recipe.v1","run":{"operation":"compare","a":"a","b":"b","threshold":0.01,"metric":"mean","ppd":67.}}),
    );
}
fn code(v: &Value) -> &str {
    v["errors"][0]["code"].as_str().unwrap()
}
#[test]
fn compare_pack_relocates_and_preserves_a_regression() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    compare(root, true);
    let receipt = cli(root, &["replay", "pack", "recipe.json", "--out", "pack"], 0);
    assert_eq!(receipt["measurement_exit"], 1);
    let pack: Value =
        serde_json::from_slice(&std::fs::read(root.join("pack/pack.json")).unwrap()).unwrap();
    let schema: Value =
        serde_json::from_str(saccade_core::schema_catalog::get("saccade-replay-pack.v1").unwrap())
            .unwrap();
    assert!(jsonschema::validator_for(&schema).unwrap().is_valid(&pack));
    std::fs::rename(root.join("pack"), root.join("moved")).unwrap();
    std::fs::remove_dir_all(root.join("a")).unwrap();
    std::fs::remove_dir_all(root.join("b")).unwrap();
    let replay = cli(root, &["replay", "verify", "moved"], 0);
    assert_eq!(replay["report_id"], receipt["report_id"]);
    assert_eq!(replay["measurement_exit"], 1);
}
#[test]
fn exact_input_config_report_tool_and_schema_drift_are_typed() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    compare(root, false);
    cli(root, &["replay", "pack", "recipe.json", "--out", "pack"], 0);
    for (path, expected) in [
        ("inputs/a/image.png", "replay_input_changed"),
        ("recipe.json", "replay_config_changed"),
        ("run/saccade-report.v1.json", "replay_report_changed"),
        ("tool/build.json", "replay_tool_changed"),
        (
            "schemas/saccade-replay-pack.v1.schema.json",
            "replay_schema_changed",
        ),
    ] {
        let p = root.join("pack").join(path);
        let bytes = std::fs::read(&p).unwrap();
        std::fs::write(&p, b"changed").unwrap();
        assert_eq!(code(&cli(root, &["replay", "verify", "pack"], 2)), expected);
        std::fs::write(p, bytes).unwrap();
    }
    std::fs::write(root.join("pack/inputs/a/extra.json"), b"{}").unwrap();
    assert_eq!(
        code(&cli(root, &["replay", "verify", "pack"], 2)),
        "replay_input_changed"
    );
}
#[test]
fn imported_text_pack_retains_exact_observations_and_settings() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    image::RgbaImage::from_pixel(40, 20, image::Rgba([255, 255, 255, 255]))
        .save(root.join("image.png"))
        .unwrap();
    let hash = saccade_core::localized::digest(&std::fs::read(root.join("image.png")).unwrap());
    for (name, text) in [("a.json", "Value -2"), ("b.json", "Value 2")] {
        put(
            root,
            name,
            json!({"schema":"saccade-ui-source.v1","kind":"paddle_ocr","capture_sha256":hash,"dimensions":[40,20],"producer":{"id":"generated-ocr-observation","license":"CC0-1.0"},"complete":false,"nodes":[{"id":"word-1","text":text,"bounds":[0.,0.,30.,10.],"reading_order":null,"keyboard_order":null,"disclosure":false,"ocr_confidence":99.}]}),
        );
    }
    put(
        root,
        "recipe.json",
        json!({"schema":"saccade-replay-recipe.v1","run":{"operation":"text","a":"image.png","b":"image.png","a_source":"a.json","b_source":"b.json","ocr":null,"expect_text":["Value 2"],"readable_confidence":80.,"moved_px":3.}}),
    );
    let original = cli(root, &["replay", "pack", "recipe.json", "--out", "pack"], 0);
    assert_eq!(original["measurement_exit"], 1);
    let replay = cli(root, &["replay", "verify", "pack"], 0);
    assert_eq!(replay["report_id"], original["report_id"]);
}
#[test]
fn supplied_report_must_match_fresh_execution() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    compare(root, false);
    cli(
        root,
        &[
            "compare",
            "a",
            "b",
            "--out",
            "existing",
            "--threshold",
            "0.01",
            "--ppd",
            "67",
        ],
        0,
    );
    cli(
        root,
        &[
            "replay",
            "pack",
            "recipe.json",
            "--report",
            "existing/saccade-report.v1.json",
            "--out",
            "pack",
        ],
        0,
    );
    let mut report: Value = serde_json::from_slice(
        &std::fs::read(root.join("existing/saccade-report.v1.json")).unwrap(),
    )
    .unwrap();
    report["config"]["threshold"] = json!(0.9);
    put(root, "bad-report.json", report);
    assert_eq!(
        code(&cli(
            root,
            &[
                "replay",
                "pack",
                "recipe.json",
                "--report",
                "bad-report.json",
                "--out",
                "bad-pack"
            ],
            2
        )),
        "replay_report_mismatch"
    );
    assert!(!root.join("bad-pack").exists());
}
#[test]
fn manifest_mutation_and_unknown_recipe_options_fail_closed() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    compare(root, false);
    cli(root, &["replay", "pack", "recipe.json", "--out", "pack"], 0);
    std::fs::write(root.join("pack/pack.json"), b"{}").unwrap();
    assert_eq!(
        code(&cli(root, &["replay", "verify", "pack"], 2)),
        "replay_pack_changed"
    );
    put(
        root,
        "recipe.json",
        json!({"schema":"saccade-replay-recipe.v1","run":{"operation":"shell","command":"true"}}),
    );
    cli(
        root,
        &["replay", "pack", "recipe.json", "--out", "invalid"],
        2,
    );
    assert!(!root.join("invalid").exists());
}
#[cfg(unix)]
#[test]
fn pack_and_verification_reject_input_symlinks() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    compare(root, false);
    cli(root, &["replay", "pack", "recipe.json", "--out", "pack"], 0);
    std::fs::remove_file(root.join("pack/inputs/a/image.png")).unwrap();
    std::os::unix::fs::symlink(
        root.join("a/image.png"),
        root.join("pack/inputs/a/image.png"),
    )
    .unwrap();
    assert_eq!(
        code(&cli(root, &["replay", "verify", "pack"], 2)),
        "unsafe_path"
    );
    std::os::unix::fs::symlink(root.join("a/image.png"), root.join("a/link.png")).unwrap();
    assert_eq!(
        code(&cli(
            root,
            &["replay", "pack", "recipe.json", "--out", "bad-pack"],
            2
        )),
        "unsafe_path"
    );
}

#[test]
fn raster_file_sidecars_are_retained_and_bound() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    compare(root, false);
    put(
        root,
        "a/saccade-meta.json",
        json!({"build.commit":"generated-reference","exposure":1}),
    );
    put(
        root,
        "b/saccade-meta.json",
        json!({"build.commit":"generated-candidate","exposure":2}),
    );
    put(
        root,
        "recipe.json",
        json!({"schema":"saccade-replay-recipe.v1","run":{"operation":"compare","a":"a/image.png","b":"b/image.png","threshold":0.01,"metric":"mean","ppd":67.}}),
    );
    cli(
        root,
        &[
            "compare",
            "a/image.png",
            "b/image.png",
            "--out",
            "existing",
            "--threshold",
            "0.01",
            "--ppd",
            "67",
        ],
        0,
    );
    cli(
        root,
        &[
            "replay",
            "pack",
            "recipe.json",
            "--report",
            "existing/saccade-report.v1.json",
            "--out",
            "pack",
        ],
        0,
    );
    assert!(root.join("pack/inputs/a/saccade-meta.json").is_file());
    cli(root, &["replay", "verify", "pack"], 0);
    std::fs::write(root.join("pack/inputs/a/saccade-meta.json"), b"{}").unwrap();
    assert_eq!(
        code(&cli(root, &["replay", "verify", "pack"], 2)),
        "replay_input_changed"
    );
}
