#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::path::Path;

use flipdiff_core::config::{Override, RunConfig};
use flipdiff_core::report::{Metric, REPORT_FILE_NAME, Report, Status};
use flipdiff_core::run::run;
use image::{Rgb, RgbImage};

fn save(dir: &Path, name: &str, img: &RgbImage) {
    let path = dir.join(name);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    img.save(path).expect("save");
}

fn flat(w: u32, h: u32, v: u8) -> RgbImage {
    RgbImage::from_pixel(w, h, Rgb([v, v, v]))
}

fn status(report: &Report, name: &str) -> Status {
    report
        .entries
        .iter()
        .find(|e| e.name == name)
        .unwrap_or_else(|| panic!("no entry {name}"))
        .status
}

#[test]
fn statuses_overrides_and_report_json() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (base, cap, out) = (
        tmp.path().join("base"),
        tmp.path().join("cap"),
        tmp.path().join("out"),
    );
    // identical pair, new, missing, size mismatch, ignored, nested + override.
    save(&base, "same.png", &flat(16, 16, 100));
    save(&cap, "same.png", &flat(16, 16, 100));
    save(&cap, "fresh.png", &flat(16, 16, 10));
    save(&base, "gone.png", &flat(16, 16, 10));
    save(&base, "size.png", &flat(16, 16, 10));
    save(&cap, "size.png", &flat(16, 17, 10));
    save(&cap, "debug_x.png", &flat(16, 16, 10));
    save(&base, "terrain/hill.png", &flat(16, 16, 100));
    save(&cap, "terrain/hill.png", &flat(16, 16, 140));

    let mut cfg = RunConfig {
        ignore: vec!["debug_*.png".into()],
        ..RunConfig::default()
    };
    let strict = run(&base, &cap, &out, &cfg).expect("run");
    assert_eq!(status(&strict, "same.png"), Status::Pass);
    assert_eq!(status(&strict, "fresh.png"), Status::New);
    assert_eq!(status(&strict, "gone.png"), Status::Missing);
    assert_eq!(status(&strict, "size.png"), Status::Error);
    assert_eq!(status(&strict, "terrain/hill.png"), Status::Fail);
    assert!(strict.entries.iter().all(|e| e.name != "debug_x.png"));
    assert!(strict.is_regression());
    let size = strict
        .entries
        .iter()
        .find(|e| e.name == "size.png")
        .expect("size");
    assert!(size.error.is_some() && size.paths.capture.is_some());
    assert_eq!(strict.totals.total, 5);

    // A matching override flips the fail to a pass.
    cfg.overrides.push(Override {
        glob: "terrain/**".into(),
        threshold: Some(0.9),
        metric: Some(Metric::Max),
    });
    let lax = run(&base, &cap, &out, &cfg).expect("run");
    assert_eq!(status(&lax, "terrain/hill.png"), Status::Pass);

    let json = std::fs::read_to_string(out.join(REPORT_FILE_NAME)).expect("json");
    let parsed: Report = serde_json::from_str(&json).expect("parse");
    assert_eq!(parsed, lax);
    assert!(out.join("images/terrain/hill.png/heatmap.png").is_file());
}
