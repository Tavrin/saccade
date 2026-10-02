#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::path::Path;
use std::process::{Command, Output};

use image::{Rgb, RgbImage};

fn gradient() -> RgbImage {
    RgbImage::from_fn(32, 32, |x, y| Rgb([(x * 6) as u8, (y * 6) as u8, 120]))
}

fn save(dir: &Path, name: &str, img: &RgbImage) {
    std::fs::create_dir_all(dir.join(name).parent().expect("parent")).expect("mkdir");
    img.save(dir.join(name)).expect("save");
}

/// Writes `a.png` (identical) and `sub/b.png` (identical or one pixel +1).
fn pair(tmp: &Path, nudge: bool) -> (std::path::PathBuf, std::path::PathBuf) {
    let (parent, cand) = (tmp.join("parent"), tmp.join("cand"));
    for dir in [&parent, &cand] {
        save(dir, "a.png", &gradient());
    }
    save(&parent, "sub/b.png", &gradient());
    let mut b = gradient();
    if nudge {
        let p = b.get_pixel_mut(10, 10);
        p.0[1] = p.0[1].saturating_add(1);
    }
    save(&cand, "sub/b.png", &b);
    (parent, cand)
}

fn identity(tmp: &Path, extra: &[&str]) -> Output {
    let (parent, cand) = (tmp.join("parent"), tmp.join("cand"));
    Command::new(env!("CARGO_BIN_EXE_saccade"))
        .arg("identity")
        .args([&parent, &cand])
        .arg("--out")
        .arg(tmp.join("out"))
        .args(extra)
        .output()
        .expect("spawn")
}

fn report(tmp: &Path) -> serde_json::Value {
    let text = std::fs::read_to_string(tmp.join("out/saccade-report.v1.json")).expect("report");
    serde_json::from_str(&text).expect("json")
}

#[test]
fn identical_pair_is_bit_identical_and_passes_at_zero() {
    let tmp = tempfile::tempdir().expect("tmp");
    pair(tmp.path(), false);
    let o = identity(tmp.path(), &[]);
    assert_eq!(o.status.code(), Some(0), "{o:?}");
    let r = report(tmp.path());
    assert_eq!(r["config"]["mode"], "identity");
    assert_eq!(r["config"]["default_threshold"], 0.0);
    for e in r["entries"].as_array().expect("entries") {
        assert_eq!(e["bit_identical"], true);
        assert_eq!(e["status"], "pass");
    }
}

#[test]
fn one_level_change_fails_and_tolerance_flags_are_rejected() {
    let tmp = tempfile::tempdir().expect("tmp");
    pair(tmp.path(), true);
    let o = identity(tmp.path(), &[]);
    assert_eq!(o.status.code(), Some(1), "{o:?}");
    let r = report(tmp.path());
    let b = r["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .find(|e| e["name"] == "sub/b.png")
        .expect("entry");
    assert_eq!(b["bit_identical"], false);
    assert_eq!(b["status"], "fail");
    for args in [
        ["--threshold", "0.01"],
        ["--threshold", "0"],
        ["--metric", "max"],
    ] {
        let o = identity(tmp.path(), &args);
        assert_eq!(o.status.code(), Some(2), "{o:?}");
        assert!(String::from_utf8_lossy(&o.stderr).contains("identity rejects"));
    }
}

#[test]
fn labels_reach_json_and_html() {
    let tmp = tempfile::tempdir().expect("tmp");
    pair(tmp.path(), false);
    let o = identity(tmp.path(), &["--labels", "main@abc,pr-7"]);
    assert_eq!(o.status.code(), Some(0), "{o:?}");
    let r = report(tmp.path());
    assert_eq!(r["config"]["labels"]["baseline"], "main@abc");
    assert_eq!(r["config"]["labels"]["capture"], "pr-7");
    let html = std::fs::read_to_string(tmp.path().join("out/index.html")).expect("html");
    assert!(html.contains("main@abc") && html.contains("pr-7"));
}

#[test]
fn identity_markdown_headline() {
    let tmp = tempfile::tempdir().expect("tmp");
    pair(tmp.path(), false);
    identity(tmp.path(), &[]);
    let json = tmp.path().join("out/saccade-report.v1.json");
    let summary = |json: &Path| {
        let o = Command::new(env!("CARGO_BIN_EXE_saccade"))
            .arg("summary")
            .arg(json)
            .output()
            .expect("spawn");
        String::from_utf8(o.stdout).expect("utf8")
    };
    assert!(summary(&json).contains("identity: ✅ 2/2 bit-identical"));
    pair(tmp.path(), true);
    identity(tmp.path(), &[]);
    let md = summary(&json);
    assert!(
        md.contains("identity: ❌ 1 differ (max FLIP ") && md.contains(" on `sub/b.png`)"),
        "{md}"
    );
}

#[test]
fn bit_identity_compares_native_samples_not_the_cleaned_decode() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (parent, cand) = (tmp.path().join("parent"), tmp.path().join("cand"));
    std::fs::create_dir_all(&parent).expect("mkdir");
    std::fs::create_dir_all(&cand).expect("mkdir");
    // 16-bit samples that differ only in the low byte.
    let deep = |v: u16| image::ImageBuffer::from_pixel(8, 8, image::Rgb([v, v, v]));
    deep(30000).save(parent.join("deep.png")).expect("save");
    deep(30001).save(cand.join("deep.png")).expect("save");
    // A NaN sample against 0: the HDR decode cleans NaN to 0.
    let exr = |v: f32| image::Rgb32FImage::from_pixel(8, 8, image::Rgb([v, 0.0, 0.0]));
    exr(0.0).save(parent.join("nan.exr")).expect("save");
    exr(f32::NAN).save(cand.join("nan.exr")).expect("save");
    identity(tmp.path(), &[]);
    let r = report(tmp.path());
    for e in r["entries"].as_array().expect("entries") {
        assert_eq!(e["bit_identical"], false, "{e}");
    }
}

#[test]
fn auto_loaded_config_overrides_do_not_relax_identity() {
    let tmp = tempfile::tempdir().expect("tmp");
    pair(tmp.path(), true);
    let toml = "[[override]]\nglob = \"**\"\nthreshold = 1.0\n";
    std::fs::write(tmp.path().join("saccade.toml"), toml).expect("toml");
    let run = |extra: &[&str]| {
        let (parent, cand) = (tmp.path().join("parent"), tmp.path().join("cand"));
        Command::new(env!("CARGO_BIN_EXE_saccade"))
            .current_dir(tmp.path())
            .arg("identity")
            .args([&parent, &cand])
            .arg("--out")
            .arg(tmp.path().join("out"))
            .args(extra)
            .output()
            .expect("spawn")
    };
    let o = run(&[]);
    assert_eq!(o.status.code(), Some(2), "{o:?}");
    assert!(String::from_utf8_lossy(&o.stderr).contains("[[override]]"));
    let o = run(&["--config", "saccade.toml"]);
    assert_eq!(o.status.code(), Some(2), "{o:?}");
}

#[test]
fn moss_json_retains_its_contract_and_separates_equality_from_validity() {
    let tmp = tempfile::tempdir().expect("temp");
    pair(tmp.path(), false);
    let o = identity(
        tmp.path(),
        &["--json", "--require-matching-meta", "--entries", "a.png"],
    );
    assert_eq!(o.status.code(), Some(1), "{o:?}");
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).expect("json");
    assert_eq!(v["schema"], "saccade-result.v1");
    assert_eq!(v["mode"], "identity");
    assert_eq!(v["verdict"], "regression");
    for key in ["pass", "fail", "error", "missing", "new", "total"] {
        assert!(v["totals"][key].is_number(), "{key}");
    }
    assert!(
        v["failing"][0]["error"]
            .as_str()
            .expect("error")
            .contains("metadata")
    );
    assert_eq!(v["sample_equality"], true);
    assert_eq!(v["capture_validity"]["status"], "invalid");
    assert_eq!(v["scope"]["entries"], serde_json::json!(["a.png"]));
    let r = report(tmp.path());
    assert_eq!(r["entries"][0]["bit_identical"], true);
    assert_eq!(r["entries"][0]["capture_validity"]["status"], "invalid");
}
