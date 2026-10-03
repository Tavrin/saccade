//! The replaced run overview is available through view reference comparisons.
#![allow(clippy::unwrap_used, missing_docs)]
use image::{Rgb, RgbImage};
use std::process::Command;
#[test]
fn view_compares_captures_against_an_explicit_reference() {
    let tmp = tempfile::tempdir().unwrap();
    for (name, shade) in [("base", 40), ("same", 40), ("edit", 200)] {
        let dir = tmp.path().join(name);
        std::fs::create_dir(&dir).unwrap();
        RgbImage::from_pixel(16, 16, Rgb([shade; 3]))
            .save(dir.join("scene.png"))
            .unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .current_dir(tmp.path())
        .args([
            "view",
            "base",
            "same",
            "edit",
            "--reference",
            "base",
            "--out",
            "view",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(tmp.path().join("view/index.html").is_file());
    assert!(tmp.path().join("view/saccade-view.v1.json").is_file());
}
