#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::path::Path;
use std::process::Command;

use image::{Rgb, RgbImage};

fn save(dir: &Path, name: &str, v: u8) {
    std::fs::create_dir_all(dir).expect("mkdir");
    RgbImage::from_pixel(16, 16, Rgb([v, v, v]))
        .save(dir.join(name))
        .expect("save");
}

fn saccade(args: &[&Path]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(args)
        .output()
        .expect("spawn")
}

#[test]
fn exit_codes_and_approve() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (base, cap, out) = (
        tmp.path().join("base"),
        tmp.path().join("cap"),
        tmp.path().join("out"),
    );
    save(&base, "a.png", 100);
    save(&cap, "a.png", 100);
    let compare = |out: &Path| {
        let o = Command::new(env!("CARGO_BIN_EXE_saccade"))
            .args(["compare"])
            .args([&base, &cap])
            .arg("--out")
            .arg(out)
            .current_dir(tmp.path())
            .output()
            .expect("spawn");
        o.status.code()
    };
    assert_eq!(compare(&out), Some(0));

    // A regression exits 1 and the table names the failing image.
    save(&cap, "a.png", 200);
    assert_eq!(compare(&out), Some(1));

    // Approving it makes the next run clean again.
    let report = out.join("saccade-report.v1.json");
    let approve = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["approve"])
        .args([&cap, &base])
        .arg("--all-failing")
        .arg(&report)
        .output()
        .expect("spawn");
    assert_eq!(approve.status.code(), Some(0));
    assert_eq!(compare(&out), Some(0));

    // Usage and IO errors exit 2.
    assert_eq!(saccade(&[Path::new("compare")]).status.code(), Some(2));
    let missing = tmp.path().join("nope");
    assert_eq!(
        saccade(&[Path::new("compare"), &missing, &cap])
            .status
            .code(),
        Some(2)
    );
}
