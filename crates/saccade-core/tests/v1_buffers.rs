//! Generated numerical-buffer fixtures exercise the normal report pipeline.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use std::path::Path;

use image::{ImageBuffer, Luma, Rgb, RgbImage, Rgba};
use saccade_core::config::RunConfig;
use saccade_core::report::Status;

fn config(kind: &str, encoding: &str, threshold: f64, scale: f64) -> RunConfig {
    RunConfig::from_toml_str(&format!(
        "[[buffer]]\nglob = '*'\nkind = '{kind}'\nencoding = '{encoding}'\nthreshold = {threshold}\nscale = {scale}\n"
    ))
    .unwrap()
}

fn dirs(root: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let (b, c) = (root.join("base"), root.join("cap"));
    std::fs::create_dir_all(&b).unwrap();
    std::fs::create_dir_all(&c).unwrap();
    (b, c)
}

#[test]
fn depth_preserves_16_bit_samples_and_reads_single_r_exr() {
    let tmp = tempfile::tempdir().unwrap();
    let (b, c) = dirs(tmp.path());
    ImageBuffer::from_pixel(2, 2, Luma([32768u16]))
        .save(b.join("depth.png"))
        .unwrap();
    ImageBuffer::from_pixel(2, 2, Luma([32769u16]))
        .save(c.join("depth.png"))
        .unwrap();
    for encoding in ["linear01", "reverse_z"] {
        let out = tmp.path().join(encoding);
        let r =
            saccade_core::run::run(&b, &c, &out, &config("depth", encoding, 0.00001, 1.0)).unwrap();
        let e = &r.entries[0];
        let buf = e.buffer.as_ref().unwrap();
        assert_eq!(e.status, Status::Fail);
        assert!(e.metrics.is_none() && e.hdr.is_none());
        assert!((buf.stats.mean - 1.0 / 65535.0).abs() < 1e-7);
        assert!(buf.stats.mean_relative.unwrap() > 0.0);
        assert!(out.join(e.paths.heatmap.as_ref().unwrap()).is_file());
    }
    std::fs::remove_file(b.join("depth.png")).unwrap();
    std::fs::remove_file(c.join("depth.png")).unwrap();
    use exr::prelude::{Image, SpecificChannels, WritableImage};
    for (dir, value) in [(&b, 2.0f32), (&c, 2.25f32)] {
        let channels = SpecificChannels::build()
            .with_channel("R")
            .with_pixel_fn(move |_| (value,));
        Image::from_channels((2, 2), channels)
            .write()
            .to_file(dir.join("depth.exr"))
            .unwrap();
    }
    let r = saccade_core::run::run(
        &b,
        &c,
        &tmp.path().join("exr"),
        &config("depth", "r32f", 0.1, 1.0),
    )
    .unwrap();
    let e = &r.entries[0];
    let stats = &e.buffer.as_ref().unwrap().stats;
    assert_eq!(e.status, Status::Fail);
    assert_eq!(stats.mean, 0.25);
    assert_eq!(stats.mean_relative, Some(0.125));
    assert_eq!(e.bit_identical, Some(false));
    assert!(e.paths.baseline.is_some() && e.paths.capture.is_some());
}

#[test]
fn normals_measure_angles_in_degrees_and_identity_is_zero() {
    let tmp = tempfile::tempdir().unwrap();
    let (b, c) = dirs(tmp.path());
    RgbImage::from_pixel(2, 2, Rgb([128, 128, 255]))
        .save(b.join("normal.png"))
        .unwrap();
    RgbImage::from_pixel(2, 2, Rgb([255, 128, 128]))
        .save(c.join("normal.png"))
        .unwrap();
    let cfg = config("normal", "rgb_snorm", 5.0, 1.0);
    let r = saccade_core::run::run(&b, &c, &tmp.path().join("report"), &cfg).unwrap();
    assert_eq!(r.entries[0].status, Status::Fail);
    assert!((r.entries[0].value.unwrap() - 90.0).abs() < 1.0);
    assert_eq!(r.entries[0].buffer.as_ref().unwrap().unit, "degrees");
    let r = saccade_core::run::run(
        &b,
        &b,
        &tmp.path().join("same"),
        &config("normal", "rgb_snorm", 0.0, 1.0),
    )
    .unwrap();
    assert_eq!(r.entries[0].value, Some(0.0));
    assert_eq!(r.entries[0].status, Status::Pass);
    assert!(
        RunConfig::from_toml_str("[[buffer]]\nglob='*'\nkind='normal'\nencoding='linear01'")
            .is_err()
    );
}

#[test]
fn motion_measures_pixel_end_point_error_with_configured_scale() {
    let tmp = tempfile::tempdir().unwrap();
    let (b, c) = dirs(tmp.path());
    RgbImage::from_pixel(2, 2, Rgb([128, 128, 0]))
        .save(b.join("motion.png"))
        .unwrap();
    RgbImage::from_pixel(2, 2, Rgb([131, 132, 255]))
        .save(c.join("motion.png"))
        .unwrap();
    let r = saccade_core::run::run(
        &b,
        &c,
        &tmp.path().join("report"),
        &config("motion", "rg_snorm", 0.1, 255.0),
    )
    .unwrap();
    let e = &r.entries[0];
    assert!((e.value.unwrap() - 10.0).abs() < 1e-4);
    assert_eq!(e.status, Status::Fail);
    assert_eq!(e.buffer.as_ref().unwrap().unit, "pixels");
    assert!(e.metrics.is_none());
}

#[test]
fn mask_and_id_compare_exact_native_pixels_including_alpha() {
    let tmp = tempfile::tempdir().unwrap();
    let (b, c) = dirs(tmp.path());
    let base = ImageBuffer::from_pixel(2, 2, Rgba([1000u16, 2000, 3000, 65535]));
    let mut cap = base.clone();
    cap.put_pixel(0, 0, Rgba([1000u16, 2000, 3000, 65534]));
    base.save(b.join("id.png")).unwrap();
    cap.save(c.join("id.png")).unwrap();
    for kind in ["mask", "id"] {
        let r = saccade_core::run::run(
            &b,
            &c,
            &tmp.path().join(kind),
            &config(kind, "exact", 0.0, 1.0),
        )
        .unwrap();
        let e = &r.entries[0];
        let stats = &e.buffer.as_ref().unwrap().stats;
        assert_eq!(stats.changed_pixels, Some(1));
        assert_eq!(stats.exact_match_fraction, Some(0.75));
        assert_eq!(e.value, Some(0.25));
        assert_eq!(e.status, Status::Fail);
        assert!(e.metrics.is_none());
    }
}
