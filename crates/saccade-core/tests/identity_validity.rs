#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

use image::{Rgb, RgbImage, Rgba};
use saccade_core::config::RunConfig;
use saccade_core::meta::{DeclaredChange, Validity};
use saccade_core::report::{Metric, Mode, Status};
use saccade_core::run::run;
use std::path::{Path, PathBuf};

fn setup(root: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let (base, cap, out) = (root.join("base"), root.join("cap"), root.join("out"));
    for dir in [&base, &cap] {
        std::fs::create_dir_all(dir).expect("mkdir");
        RgbImage::from_pixel(8, 8, Rgb([100, 80, 60]))
            .save(dir.join("a.png"))
            .expect("save");
    }
    (base, cap, out)
}

fn identity() -> RunConfig {
    RunConfig {
        mode: Mode::Identity,
        default_threshold: 0.0,
        default_metric: Metric::Max,
        ..Default::default()
    }
}

#[test]
fn native_depth_and_sample_type_decide_identity_even_at_zero_flip() {
    let tmp = tempfile::tempdir().expect("temp");
    let (b, c, out) = setup(tmp.path());
    for (dir, v) in [(&b, 30000u16), (&c, 30001u16)] {
        image::ImageBuffer::from_pixel(8, 8, Rgb([v, v, v]))
            .save(dir.join("a.png"))
            .expect("save deep");
    }
    let r = run(&b, &c, &out, &identity()).expect("run");
    assert_eq!(r.entries[0].metrics.as_ref().expect("metrics").max, 0.0);
    assert_eq!(r.sample_equality(), Some(false));
    assert_eq!(r.entries[0].status, Status::Fail);
    assert_eq!(r.capture_validity().status, Validity::Unknown);
    image::ImageBuffer::from_pixel(8, 8, Rgb([0u16, 0, 0]))
        .save(b.join("a.png"))
        .expect("16 bit");
    RgbImage::from_pixel(8, 8, Rgb([0, 0, 0]))
        .save(c.join("a.png"))
        .expect("8 bit");
    assert_eq!(
        run(&b, &c, &out, &identity())
            .expect("run")
            .sample_equality(),
        Some(false)
    );
}

#[test]
fn hidden_rgb_alpha_and_channel_interpretation_are_native_samples() {
    let tmp = tempfile::tempdir().expect("temp");
    let (b, c, out) = setup(tmp.path());
    for (bp, cp) in [
        ([10, 20, 30, 0], [11, 20, 30, 0]),
        ([10, 20, 30, 100], [10, 20, 30, 101]),
    ] {
        image::RgbaImage::from_pixel(8, 8, Rgba(bp))
            .save(b.join("a.png"))
            .expect("base");
        image::RgbaImage::from_pixel(8, 8, Rgba(cp))
            .save(c.join("a.png"))
            .expect("cap");
        let r = run(&b, &c, &out, &identity()).expect("run");
        assert_eq!(r.sample_equality(), Some(false));
        assert!(r.is_regression());
    }
    RgbImage::from_pixel(8, 8, Rgb([10, 20, 30]))
        .save(b.join("a.png"))
        .expect("rgb");
    image::RgbaImage::from_pixel(8, 8, Rgba([10, 20, 30, 255]))
        .save(c.join("a.png"))
        .expect("rgba");
    assert_eq!(
        run(&b, &c, &out, &identity())
            .expect("run")
            .sample_equality(),
        Some(false)
    );
}

#[test]
fn identity_rejects_project_tolerances_and_numerical_acceptance() {
    for text in [
        "threshold = 0.0",
        "metric = 'max'",
        "[[override]]\nglob = '**'\nthreshold = 0.0",
    ] {
        let mut cfg = RunConfig::from_toml_str(text).expect("parse");
        cfg.mode = Mode::Identity;
        cfg.default_threshold = 0.0;
        cfg.default_metric = Metric::Max;
        assert!(cfg.validate().is_err(), "{text}");
    }
    let mut cfg = identity();
    cfg.default_threshold = 0.01;
    assert!(cfg.validate().is_err());
}

#[test]
fn identity_rejects_masks_and_region_acceptance_before_creating_output() {
    let tmp = tempfile::tempdir().expect("temp");
    let (b, c, out) = setup(tmp.path());
    for text in [
        "[[mask]]\nrect = [0, 0, 1, 1]",
        "[[region]]\nname = 'roi'\nrect = [0, 0, 1, 1]\nthreshold = 0",
    ] {
        let mut cfg = RunConfig::from_toml_str(text).expect("parse");
        cfg.mode = Mode::Identity;
        cfg.default_threshold = 0.0;
        cfg.default_metric = Metric::Max;
        assert!(run(&b, &c, &out, &cfg).is_err());
        assert!(!out.exists());
    }
}

#[test]
fn missing_new_and_decode_errors_leave_equality_unknown_and_validity_invalid() {
    let tmp = tempfile::tempdir().expect("temp");
    let (b, c, out) = setup(tmp.path());
    std::fs::copy(b.join("a.png"), b.join("missing.png")).expect("copy");
    std::fs::copy(c.join("a.png"), c.join("new.png")).expect("copy");
    std::fs::write(c.join("broken.png"), b"broken").expect("broken");
    let mut cfg = identity();
    cfg.fail_on_new = false;
    let r = run(&b, &c, &out, &cfg).expect("run");
    assert!(r.is_regression());
    assert_eq!(r.sample_equality(), None);
    assert_eq!(r.capture_validity().status, Validity::Invalid);
    assert_eq!((r.totals.new, r.totals.missing, r.totals.error), (1, 1, 1));
    assert!(RunConfig::default().fail_on_new);
    // Explicit onboarding permits new images only alongside real compared evidence.
    std::fs::remove_file(b.join("missing.png")).expect("remove");
    std::fs::remove_file(c.join("broken.png")).expect("remove");
    let cfg = RunConfig {
        fail_on_new: false,
        ..Default::default()
    };
    assert!(!run(&b, &c, &out, &cfg).expect("onboarding").is_regression());
    assert!(
        run(&b, &c, &out, &RunConfig::default())
            .expect("strict")
            .is_regression()
    );
}

#[test]
fn empty_selected_scope_is_never_evidence() {
    let tmp = tempfile::tempdir().expect("temp");
    let (b, c, out) = setup(tmp.path());
    let mut cfg = identity();
    cfg.entries = vec!["absent/**".into()];
    cfg.allow_empty = true;
    let r = run(&b, &c, &out, &cfg).expect("run");
    assert!(r.is_regression());
    assert_eq!(r.sample_equality(), None);
    assert_eq!(r.capture_validity().status, Validity::Unknown);
    assert_eq!(r.config.entries, cfg.entries);
    cfg.entries = vec!["a.png".into()];
    let r = run(&b, &c, &out, &cfg).expect("selected");
    assert_eq!(r.sample_equality(), Some(true));
    assert_eq!(r.config.entries, vec!["a.png"]);
    assert!(out.join(".saccade-run").is_file());
}

#[test]
fn equal_nonfinite_samples_fail_validity_on_either_side_including_alpha() {
    let tmp = tempfile::tempdir().expect("temp");
    let (b, c, out) = setup(tmp.path());
    for dir in [&b, &c] {
        std::fs::remove_file(dir.join("a.png")).expect("remove");
    }
    let mut cfg = identity();
    cfg.fail_on_nonfinite = false;
    for bad in [f32::NAN, f32::INFINITY] {
        let img = image::Rgba32FImage::from_pixel(8, 8, Rgba([0.5, 0.3, 0.2, bad]));
        img.save(b.join("a.exr")).expect("exr");
        std::fs::copy(b.join("a.exr"), c.join("a.exr")).expect("copy");
        let r = run(&b, &c, &out, &cfg).expect("run");
        assert_eq!(r.sample_equality(), Some(true));
        assert_eq!(r.entries[0].file_bytes_identical, Some(true));
        assert_eq!(r.capture_validity().status, Validity::Invalid);
        assert_eq!(r.entries[0].status, Status::Error);
        assert!(r.is_regression());
        image::Rgba32FImage::from_pixel(8, 8, Rgba([0.5, 0.3, 0.2, 1.0]))
            .save(c.join("a.exr"))
            .expect("finite");
        let r = run(&b, &c, &out, &cfg).expect("base invalid");
        assert_eq!(r.sample_equality(), Some(false));
        assert!(
            r.entries[0]
                .error
                .as_deref()
                .expect("error")
                .contains("baseline")
        );
    }
}

#[test]
fn metadata_absence_mismatch_and_moss_override_preserve_equality() {
    let tmp = tempfile::tempdir().expect("temp");
    let (b, c, out) = setup(tmp.path());
    let mut cfg = identity();
    cfg.meta.name = "cost-card.json".into();
    let r = run(&b, &c, &out, &cfg).expect("unknown");
    assert_eq!(r.sample_equality(), Some(true));
    assert_eq!(r.capture_validity().status, Validity::Unknown);
    assert!(!r.is_regression());
    cfg.meta.required = true;
    let r = run(&b, &c, &out, &cfg).expect("required");
    assert_eq!(r.sample_equality(), Some(true));
    assert_eq!(r.capture_validity().status, Validity::Invalid);
    assert!(r.is_regression());
    for dir in [&b, &c] {
        std::fs::write(dir.join("cost-card.json"), r#"{"mode":"fast"}"#).expect("card");
    }
    let r = run(&b, &c, &out, &cfg).expect("matching");
    assert_eq!(r.capture_validity().status, Validity::Unknown);
    assert!(
        r.capture_validity()
            .reasons
            .iter()
            .any(|reason| reason.contains("binary_sha256 provenance is absent"))
    );
    std::fs::write(c.join("a.cost-card.json"), r#"{"mode":"slow"}"#).expect("override");
    let r = run(&b, &c, &out, &cfg).expect("mismatch");
    assert_eq!(r.sample_equality(), Some(true));
    assert_eq!(r.capture_validity().status, Validity::Invalid);
    assert_eq!(r.entries[0].meta_diff[0].capture, "slow");
    cfg.meta.declared.push("mode".into());
    assert_eq!(
        run(&b, &c, &out, &cfg)
            .expect("declared")
            .capture_validity()
            .status,
        Validity::Unknown
    );
}

#[test]
fn proof_contract_uses_explicit_ignores_and_keeps_qualification_visible() {
    let tmp = tempfile::tempdir().expect("temp");
    let (b, c, out) = setup(tmp.path());
    let mut cfg = RunConfig::from_toml_str(
        "[capture]\nrequired_keys = ['source.revision', 'build.binary_hash']",
    )
    .expect("contract");
    cfg.mode = Mode::Identity;
    cfg.default_threshold = 0.0;
    cfg.default_metric = Metric::Max;
    for (dir, ms) in [(&b, 1), (&c, 2)] {
        std::fs::write(dir.join("saccade-meta.json"), format!(r#"{{"source.revision":"abc","build.binary_hash":"123","warmup_duration_ms":{ms},"gpu_ms":{ms},"qualification.elapsed_ms":{ms}}}"#)).expect("card");
    }
    let r = run(&b, &c, &out, &cfg).expect("proof");
    let e = &r.entries[0];
    assert_eq!(e.bit_identical, Some(true));
    assert_eq!(e.capture_validity.status, Validity::Invalid);
    assert!(e.meta_diff.iter().any(|d| d.key == "warmup_duration_ms"));
    assert!(
        e.meta_diff
            .iter()
            .any(|d| d.key == "qualification.elapsed_ms")
    );
    assert_eq!(e.meta_ignored_diff.len(), 1);
    cfg.meta.declared = vec![
        "warmup_duration_ms".into(),
        "qualification.elapsed_ms".into(),
    ];
    assert_eq!(
        run(&b, &c, &out, &cfg)
            .expect("declared")
            .capture_validity()
            .status,
        Validity::Unknown
    );
    for dir in [&b, &c] {
        std::fs::write(
            dir.join("saccade-meta.json"),
            r#"{"source.revision":"abc"}"#,
        )
        .expect("missing key");
    }
    assert_eq!(
        run(&b, &c, &out, &cfg)
            .expect("missing")
            .capture_validity()
            .status,
        Validity::Invalid
    );
    cfg.meta.ignore.push("*_ms".into());
    assert!(cfg.validate().is_err());
}

#[test]
fn declared_expected_values_are_checked_and_unchanged_interventions_reported() {
    let tmp = tempfile::tempdir().expect("temp");
    let (b, c, out) = setup(tmp.path());
    let mut cfg = identity();
    cfg.meta.required = true;
    cfg.meta.changes.push(DeclaredChange {
        key: "bloom".into(),
        reason: "disable bloom".into(),
        before: Some(true.into()),
        after: Some(false.into()),
    });
    for dir in [&b, &c] {
        std::fs::write(dir.join("saccade-meta.json"), r#"{"bloom":true}"#).expect("card");
    }
    let r = run(&b, &c, &out, &cfg).expect("unchanged");
    assert_eq!(r.sample_equality(), Some(true));
    assert_eq!(r.entries[0].meta_declared_unchanged, vec!["bloom"]);
    assert_eq!(r.capture_validity().status, Validity::Invalid);
    std::fs::write(c.join("saccade-meta.json"), r#"{"bloom":false}"#).expect("changed");
    let r = run(&b, &c, &out, &cfg).expect("changed");
    assert_eq!(r.capture_validity().status, Validity::Unknown);
    assert!(r.entries[0].meta_declared_unchanged.is_empty());
    let cfg = RunConfig::from_toml_str(
        "[[changes]]\nkey = 'bloom'\nreason = 'disable bloom'\nbefore = true\nafter = false",
    )
    .expect("parse changes");
    assert_eq!(cfg.meta.changes[0].key, "bloom");
}

#[test]
fn different_file_encodings_can_hold_identical_native_samples() {
    use image::ImageEncoder;
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    let tmp = tempfile::tempdir().expect("temp");
    let (b, c, out) = setup(tmp.path());
    let img = RgbImage::from_pixel(8, 8, Rgb([100, 80, 60]));
    for (dir, compression) in [(&b, CompressionType::Fast), (&c, CompressionType::Best)] {
        let file = std::fs::File::create(dir.join("a.png")).expect("file");
        PngEncoder::new_with_quality(file, compression, FilterType::NoFilter)
            .write_image(img.as_raw(), 8, 8, image::ExtendedColorType::Rgb8)
            .expect("encode");
    }
    let r = run(&b, &c, &out, &identity()).expect("run");
    assert_eq!(r.sample_equality(), Some(true));
    assert_eq!(r.entries[0].file_bytes_identical, Some(false));
    assert!(!r.is_regression());
}
