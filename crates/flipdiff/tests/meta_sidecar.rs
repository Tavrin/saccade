#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use image::{Rgb, RgbImage};

/// Two directories with `a.png` and `b.png` (identical pixels) and the given
/// directory-level sidecars.
fn setup(tmp: &Path, base_card: Option<&str>, cap_card: Option<&str>) -> (PathBuf, PathBuf) {
    let (base, cap) = (tmp.join("base"), tmp.join("cap"));
    let img = RgbImage::from_fn(16, 16, |x, y| Rgb([(x * 9) as u8, (y * 9) as u8, 80]));
    for (dir, card) in [(&base, base_card), (&cap, cap_card)] {
        std::fs::create_dir_all(dir).expect("mkdir");
        img.save(dir.join("a.png")).expect("save");
        img.save(dir.join("b.png")).expect("save");
        if let Some(text) = card {
            std::fs::write(dir.join("cost-card.json"), text).expect("card");
        }
    }
    (base, cap)
}

fn compare(tmp: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_flipdiff"))
        .arg("compare")
        .args([tmp.join("base"), tmp.join("cap")])
        .arg("--out")
        .arg(tmp.join("out"))
        .arg("--json")
        .args(extra)
        .output()
        .expect("spawn")
}

fn report(o: &Output) -> serde_json::Value {
    serde_json::from_slice(&o.stdout).expect("report json")
}

fn entry<'a>(r: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    r["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .find(|e| e["name"] == name)
        .expect("entry")
}

#[test]
fn identical_sidecars_have_no_diff() {
    let tmp = tempfile::tempdir().expect("tmp");
    let card = r#"{"receiver.mode":"fast","warmup_frames":8}"#;
    setup(tmp.path(), Some(card), Some(card));
    let o = compare(tmp.path(), &["--require-matching-meta"]);
    assert_eq!(o.status.code(), Some(0), "{o:?}");
    let r = report(&o);
    assert_eq!(r["config"]["meta"]["required"], true);
    assert_eq!(r["config"]["meta"]["name"], "cost-card.json");
    for e in r["entries"].as_array().expect("entries") {
        assert_eq!(e["meta_diff"], serde_json::json!([]));
    }
}

#[test]
fn undeclared_difference_is_an_error_declared_one_passes_and_is_shown() {
    let tmp = tempfile::tempdir().expect("tmp");
    setup(
        tmp.path(),
        Some(r#"{"receiver.mode":"fast","warmup_frames":8}"#),
        Some(r#"{"receiver.mode":"slow","warmup_frames":16}"#),
    );
    let o = compare(
        tmp.path(),
        &["--require-matching-meta", "--declare", "warmup_frames"],
    );
    assert_eq!(o.status.code(), Some(1), "{o:?}");
    let e = entry(&report(&o), "a.png").clone();
    assert_eq!(e["status"], "error");
    let msg = e["error"].as_str().expect("message");
    assert!(
        msg.contains("receiver.mode") && !msg.contains("warmup_frames"),
        "{msg}"
    );
    assert_eq!(e["meta_diff"].as_array().expect("diff").len(), 2);

    let o = compare(
        tmp.path(),
        &[
            "--require-matching-meta",
            "--declare",
            "warmup_frames,receiver.mode",
        ],
    );
    assert_eq!(o.status.code(), Some(0), "{o:?}");
    let e = entry(&report(&o), "a.png").clone();
    assert_eq!(e["status"], "pass");
    assert_eq!(
        e["meta_diff"][0],
        serde_json::json!({"key": "receiver.mode", "baseline": "fast", "capture": "slow"})
    );

    // Without --require-matching-meta the difference is shown, never enforced.
    let o = compare(tmp.path(), &[]);
    assert_eq!(o.status.code(), Some(0), "{o:?}");
    assert_eq!(
        entry(&report(&o), "a.png")["meta_diff"]
            .as_array()
            .expect("diff")
            .len(),
        2
    );
}

#[test]
fn per_image_sidecar_overrides_and_one_sided_sidecar_is_absent() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (base, cap) = setup(tmp.path(), Some(r#"{"mode":"x"}"#), Some(r#"{"mode":"x"}"#));
    // b.png alone gets a different mode on the capture side; a.png has a
    // per-image sidecar on the capture side only, adding a key.
    std::fs::write(cap.join("b.cost-card.json"), r#"{"mode":"y"}"#).expect("card");
    std::fs::write(cap.join("a.cost-card.json"), r#"{"extra":true}"#).expect("card");
    let o = compare(tmp.path(), &[]);
    let r = report(&o);
    assert_eq!(
        entry(&r, "b.png")["meta_diff"][0],
        serde_json::json!({"key": "mode", "baseline": "x", "capture": "y"})
    );
    assert_eq!(
        entry(&r, "a.png")["meta_diff"][0],
        serde_json::json!({"key": "extra", "baseline": "<absent>", "capture": "true"})
    );
    // A sidecar present on exactly one side: every key is a difference.
    std::fs::remove_file(base.join("cost-card.json")).expect("rm");
    let o = compare(tmp.path(), &[]);
    assert_eq!(
        entry(&report(&o), "a.png")["meta_diff"][1],
        serde_json::json!({"key": "mode", "baseline": "<absent>", "capture": "x"})
    );
}

#[test]
fn timing_keys_are_ignored_by_default_and_nested_values_are_errors() {
    let tmp = tempfile::tempdir().expect("tmp");
    setup(
        tmp.path(),
        Some(r#"{"cfg":1,"Frame_Time":3,"run.id":"a","gpu_ms":1.5,"elapsed.s":2}"#),
        Some(r#"{"cfg":1,"Frame_Time":9,"run.id":"b","gpu_ms":2.5,"elapsed.s":7}"#),
    );
    let o = compare(tmp.path(), &["--require-matching-meta"]);
    assert_eq!(o.status.code(), Some(0), "{o:?}");
    assert_eq!(
        entry(&report(&o), "a.png")["meta_diff"],
        serde_json::json!([])
    );

    std::fs::write(
        tmp.path().join("cap/cost-card.json"),
        r#"{"gpu":{"name":"x"}}"#,
    )
    .expect("card");
    let o = compare(tmp.path(), &[]);
    assert_eq!(o.status.code(), Some(1), "{o:?}");
    let e = entry(&report(&o), "a.png").clone();
    assert_eq!(e["status"], "error");
    assert!(
        e["error"].as_str().expect("message").contains("\"gpu\""),
        "{e}"
    );
}
