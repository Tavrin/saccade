//! Synthetic acceptance and adversarial boundary fixtures (five tests).
#![allow(missing_docs, clippy::unwrap_used, clippy::expect_used)]

use image::{Rgb, RgbImage};
use saccade_a11y::a11y;
use saccade_core::safety::{self, Options, Standard};
use std::path::Path;

fn sequence(root: &Path, count: usize, pixel: impl Fn(usize, u32, u32) -> [u8; 3]) {
    std::fs::create_dir_all(root).unwrap();
    for i in 0..count {
        RgbImage::from_fn(160, 64, |x, y| Rgb(pixel(i, x, y)))
            .save(root.join(format!("frame_{i:04}.png")))
            .unwrap();
    }
}
fn run(root: &Path, out: &Path, standard: Standard) -> safety::SafetyReport {
    safety::run(
        root,
        out,
        &Options {
            fps: Some(32.0),
            standard,
            ..Default::default()
        },
    )
    .unwrap()
}

#[test]
fn six_synthetic_truths() {
    let tmp = tempfile::tempdir().unwrap();
    for (name, expected) in [
        ("2hz", "PASS"),
        ("4hz", "FAIL"),
        ("small", "PASS"),
        ("red", "FAIL"),
        ("stripes", "FAIL"),
        ("fade", "PASS"),
    ] {
        let frames = tmp.path().join(name);
        sequence(&frames, 64, |i, x, _| match name {
            "2hz" => {
                if i % 16 < 8 {
                    [255; 3]
                } else {
                    [0; 3]
                }
            }
            "4hz" => {
                if i % 8 < 4 {
                    [255; 3]
                } else {
                    [0; 3]
                }
            }
            "small" => {
                if x < 16 && i % 8 < 4 {
                    [255; 3]
                } else {
                    [0; 3]
                }
            }
            "red" => {
                if i % 8 < 4 {
                    [255, 0, 0]
                } else {
                    [0; 3]
                }
            }
            "stripes" => {
                if x % 8 < 4 {
                    [255; 3]
                } else {
                    [0; 3]
                }
            }
            _ => [i as u8; 3],
        });
        let report = run(
            &frames,
            &tmp.path().join(format!("out-{name}")),
            Standard::ItuBt1702,
        );
        assert_eq!(report.verdict, expected, "{name}: {}", report.text());
        if name == "red" {
            assert!(
                report
                    .segments
                    .iter()
                    .any(|s| s.kind == "red_flash" && s.verdict == "FAIL")
            );
        }
        if name == "4hz" {
            assert_eq!(report.segments[0].peak_flash_rate, 4.0);
            assert_eq!(report.segments[0].flashing_area_percent, 100.0);
        }
        assert!(report.disclaimer.contains("Not a certification"));
    }
}

#[test]
fn sliding_window_area_and_red_definition_boundaries() {
    let tmp = tempfile::tempdir().unwrap();
    let small = tmp.path().join("small");
    sequence(&small, 64, |i, x, _| {
        if x < 16 && i % 8 < 4 {
            [255; 3]
        } else {
            [0; 3]
        }
    });
    let wcag = safety::run(
        &small,
        &tmp.path().join("wcag"),
        &Options {
            fps: Some(32.0),
            standard: Standard::Wcag,
            display: safety::Display::parse("1920x1080@55,4.5").unwrap(),
        },
    )
    .unwrap();
    assert_eq!(wcag.verdict, "PASS");
    let close = tmp.path().join("close");
    sequence(&close, 64, |i, x, _| {
        if x < 36 && i % 8 < 4 {
            [255; 3]
        } else {
            [0; 3]
        }
    });
    assert_eq!(
        run(&close, &tmp.path().join("close-out"), Standard::ItuBt1702).verdict,
        "WARN"
    );
    let odd = tmp.path().join("odd");
    // Only one qualifying red excursion: the dark saturated red endpoint is
    // below the >20 redness-change threshold. Never a completed red flash.
    sequence(
        &odd,
        64,
        |i, _, _| if i % 8 < 4 { [30, 0, 0] } else { [0; 3] },
    );
    assert_eq!(
        run(&odd, &tmp.path().join("odd-out"), Standard::Wcag).verdict,
        "PASS"
    );
    let bright = tmp.path().join("bright");
    sequence(
        &bright,
        64,
        |i, _, _| if i % 8 < 4 { [255; 3] } else { [240; 3] },
    );
    assert_eq!(
        run(&bright, &tmp.path().join("bright-out"), Standard::Wcag).verdict,
        "PASS"
    );
    // BT.1702-3 Attachment 1: six pairs exceed five even if below the old
    // eight-pair rule; a stationary area below the 40% limit remains a PASS.
    for (name, rows, expected) in [("six-pairs", 64, "FAIL"), ("small-pattern", 16, "PASS")] {
        let frames = tmp.path().join(name);
        sequence(&frames, 2, |_, x, y| {
            if y < rows && x % 24 < 12 {
                [255; 3]
            } else {
                [0; 3]
            }
        });
        assert_eq!(
            run(
                &frames,
                &tmp.path().join(format!("out-{name}")),
                Standard::ItuBt1702
            )
            .verdict,
            expected
        );
    }
    let distant = safety::Display::parse("1920x1080@55,4.5").unwrap();
    let near = safety::Display::parse("1920x1080@55,0.5").unwrap();
    assert!(near.solid_angle() > distant.solid_angle());
    assert!(safety::Display::parse("0x1080@55,4.5").is_err());
}

#[test]
fn published_ciede2000_vectors_and_deutan_information_loss() {
    use safety::color::{delta_e, lab, rgb, simulate};
    // Sharma et al. supplementary data, reference pair #1.
    assert!((delta_e([50.0, 2.6772, -79.7751], [50.0, 0.0, -82.7485]) - 2.0425).abs() < 0.0001);
    assert!(delta_e(lab(rgb([218, 108, 80])), lab(rgb([108, 166, 71]))) > 5.0);
    assert!(
        delta_e(
            lab(simulate(rgb([218, 108, 80]), 1)),
            lab(simulate(rgb([108, 166, 71]), 1))
        ) < 2.0
    );
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("markers.png");
    RgbImage::from_fn(64, 32, |x, _| {
        if x < 32 {
            Rgb([218, 108, 80])
        } else {
            Rgb([108, 166, 71])
        }
    })
    .save(&input)
    .unwrap();
    let r = a11y::run(
        &input,
        &tmp.path().join("report"),
        &a11y::Options::default(),
    )
    .unwrap();
    assert_eq!(r.verdict, "WARN");
    assert!(
        r.images[0]
            .findings
            .iter()
            .any(|f| f.simulation == "deuteranopia" && f.rect[2] > 0.0)
    );
    assert_eq!(
        r.images[0].simulations["deuteranomaly"],
        r.images[0].simulations["deuteranopia"]
    );
}

#[test]
fn normal_large_aaa_and_ui_contrast() {
    assert!(
        (saccade_core::contrast::contrast_ratio([1.0; 3], [(1.05 / 4.5) - 0.05; 3]) - 4.5).abs()
            < 1e-12
    );
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("text.png");
    // 118 on white is 4.542:1; 149 on white is 2.995:1.
    RgbImage::from_fn(100, 80, |x, y| {
        Rgb(if x % 20 < 8 {
            if y < 40 { [118; 3] } else { [149; 3] }
        } else {
            [255; 3]
        })
    })
    .save(&input)
    .unwrap();
    let config = tmp.path().join("saccade.toml");
    std::fs::write(&config,"[[region]]\nname='aa'\nkind='text'\nrect=[0.0,0.0,1.0,0.5]\n[[region]]\nname='low'\nkind='text'\nrect=[0.0,0.5,1.0,0.5]\n[[region]]\nname='aaa'\nkind='text'\nlevel='AAA'\nrect=[0.0,0.0,1.0,0.5]\n[[region]]\nname='large'\nkind='text'\nlarge=true\nrect=[0.0,0.0,1.0,0.5]\n[[region]]\nname='ui'\nkind='ui'\nrect=[0.0,0.0,1.0,0.5]\n").unwrap();
    let r = a11y::run(
        &input,
        &tmp.path().join("report"),
        &a11y::Options {
            config: Some(config),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.verdict, "FAIL");
    let checks = &r.images[0].contrast;
    assert_eq!(
        checks
            .iter()
            .map(|c| c.verdict.as_str())
            .collect::<Vec<_>>(),
        ["PASS", "FAIL", "FAIL", "PASS", "PASS"]
    );
    assert!(checks[0].ratio.unwrap() >= 4.5);
    assert!(checks[1].ratio.unwrap() < 3.01);
}

#[test]
fn input_failures_and_output_guards_do_not_destroy_sources() {
    let tmp = tempfile::tempdir().unwrap();
    let frames = tmp.path().join("frames");
    sequence(&frames, 2, |_, _, _| [0; 3]);
    assert!(safety::run(&frames, &frames, &Options::default()).is_err());
    std::fs::rename(frames.join("frame_0001.png"), frames.join("frame_0003.png")).unwrap();
    assert!(safety::run(&frames, &tmp.path().join("report"), &Options::default()).is_err());
    let transparent = tmp.path().join("alpha.png");
    image::RgbaImage::from_pixel(8, 8, image::Rgba([255, 255, 255, 0]))
        .save(&transparent)
        .unwrap();
    assert!(
        a11y::run(
            &transparent,
            &tmp.path().join("a11y"),
            &a11y::Options::default()
        )
        .is_err()
    );
    let config = tmp.path().join("invalid.toml");
    std::fs::write(
        &config,
        "[[region]]\nname='oops'\nkind='text'\nrect=[0.9,0.0,0.5,1.0]",
    )
    .unwrap();
    assert!(
        a11y::run(
            &frames,
            &tmp.path().join("bad-config"),
            &a11y::Options {
                config: Some(config),
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(frames.join("frame_0000.png").exists());
    std::fs::rename(frames.join("frame_0003.png"), frames.join("frame_0001.png")).unwrap();
    std::fs::write(frames.join("saccade-meta.json"), r#"{"fps":"32"}"#).unwrap();
    assert!(safety::run(&frames, &tmp.path().join("bad-fps"), &Options::default()).is_err());
    std::fs::remove_file(frames.join("saccade-meta.json")).unwrap();
    let out = tmp.path().join("protected-config");
    std::fs::create_dir(&out).unwrap();
    std::fs::write(out.join(a11y::FILE), "{}").unwrap();
    let config = out.join("report.txt");
    std::fs::write(&config, "[a11y]\nscale=4\n").unwrap();
    assert!(
        a11y::run(
            &frames.join("frame_0000.png"),
            &out,
            &a11y::Options {
                config: Some(config.clone()),
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(
        config.is_file(),
        "output cleanup must preserve input configuration"
    );
}
