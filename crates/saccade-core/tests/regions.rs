#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::path::Path;

use image::{Luma, Rgb, RgbImage};
use saccade_core::config::RunConfig;
use saccade_core::render::{MarkdownOptions, render_markdown};
use saccade_core::report::{Entry, Report, Status};
use saccade_core::run::run;

const SIZE: u32 = 64;

fn save(dir: &Path, name: &str, img: &RgbImage) {
    std::fs::create_dir_all(dir).expect("mkdir");
    img.save(dir.join(name)).expect("save");
}

/// Compares a flat grey baseline with a capture that has one bright 8x8 patch
/// at (8, 8), and returns the report for `config`.
fn run_patch(tmp: &Path, config: &RunConfig) -> Report {
    let flat = RgbImage::from_pixel(SIZE, SIZE, Rgb([100, 100, 100]));
    let mut patched = flat.clone();
    for y in 8..16 {
        for x in 8..16 {
            patched.put_pixel(x, y, Rgb([240, 240, 240]));
        }
    }
    save(&tmp.join("base"), "a.png", &flat);
    save(&tmp.join("cap"), "a.png", &patched);
    run(
        &tmp.join("base"),
        &tmp.join("cap"),
        &tmp.join("out"),
        config,
    )
    .expect("run")
}

fn entry(r: &Report) -> &Entry {
    r.entries.iter().find(|e| e.name == "a.png").expect("entry")
}

/// The patch covers the top-left quarter's centre; this region wraps it.
const PATCH_REGION: &str =
    "[[region]]\nname = \"corner\"\nrect = [0.0, 0.0, 0.3, 0.3]\nthreshold = 0.05\n";

#[test]
fn region_fails_where_the_whole_image_mean_passes_and_reaches_markdown() {
    let tmp = tempfile::tempdir().expect("tmp");
    let cfg = RunConfig::from_toml_str(&format!("threshold = 0.05\n{PATCH_REGION}")).expect("cfg");
    let report = run_patch(tmp.path(), &cfg);
    let e = entry(&report);
    assert!(
        e.value.expect("value") <= 0.05,
        "whole-image mean {:?}",
        e.value
    );
    let region = &e.regions[0];
    assert_eq!(region.rect_px, [0, 0, 20, 20]);
    assert_eq!(region.status, Some(Status::Fail));
    assert!(region.value > 0.05, "region value {}", region.value);
    assert_eq!(e.status, Status::Fail);
    let md = render_markdown(&report, &MarkdownOptions::default());
    assert!(md.contains("`a.png › corner`"), "{md}");
}

#[test]
fn masked_change_is_ignored_and_fraction_reported() {
    let tmp = tempfile::tempdir().expect("tmp");
    let toml = "threshold = 0.001\n[[mask]]\nrect = [0.0, 0.0, 0.5, 0.5]\n\
         [[region]]\nname = \"far\"\nrect = [0.5, 0.5, 0.5, 0.5]\nthreshold = 0.05\n";
    let report = run_patch(tmp.path(), &RunConfig::from_toml_str(toml).expect("cfg"));
    let e = entry(&report);
    assert_eq!(e.masked_fraction, Some(0.25));
    assert_eq!(e.status, Status::Pass, "value {:?}", e.value);
    assert_eq!(e.regions[0].status, Some(Status::Pass));
    let m = e.metrics.expect("metrics");
    assert_eq!((m.width, m.height), (SIZE, SIZE));
}

#[test]
fn image_mask_excludes_white_and_paths_stay_inside_the_config_directory() {
    let tmp = tempfile::tempdir().expect("tmp");
    let cfg_dir = tmp.path().join("cfg");
    std::fs::create_dir_all(cfg_dir.join("masks")).expect("mkdir");
    // 16x16 mask image (resized to 64x64): white top-left 8x8 = top-left 32x32.
    let mask =
        image::GrayImage::from_fn(16, 16, |x, y| Luma([if x < 8 && y < 8 { 255 } else { 0 }]));
    mask.save(cfg_dir.join("masks/m.png")).expect("mask");
    std::fs::write(
        cfg_dir.join("saccade.toml"),
        "threshold = 0.001\n[[mask]]\nimage = \"masks/m.png\"\n",
    )
    .expect("cfg");
    let cfg = RunConfig::from_toml_file(&cfg_dir.join("saccade.toml")).expect("load");
    let report = run_patch(tmp.path(), &cfg);
    let e = entry(&report);
    assert_eq!(e.masked_fraction, Some(0.25));
    assert_eq!(e.status, Status::Pass, "value {:?}", e.value);

    for bad in ["../m.png", "/etc/passwd"] {
        let toml = format!("[[mask]]\nimage = {bad:?}\n");
        assert!(RunConfig::from_toml_str(&toml).is_err(), "{bad}");
    }
}

#[test]
fn fully_masked_region_is_informational_with_a_null_value() {
    let tmp = tempfile::tempdir().expect("tmp");
    let toml = "[[mask]]\nrect = [0.0, 0.0, 0.5, 0.5]\n\
                [[region]]\nname = \"sky\"\nrect = [0.0, 0.0, 0.25, 0.25]\nthreshold = 0.0\n";
    let report = run_patch(tmp.path(), &RunConfig::from_toml_str(toml).expect("cfg"));
    let r = &entry(&report).regions[0];
    assert_eq!(r.status, None);
    assert!(r.value.is_nan());
    let json = serde_json::to_value(&report).expect("json");
    assert!(json["entries"][0]["regions"][0]["value"].is_null());
    assert_eq!(entry(&report).status, Status::Pass);
}
