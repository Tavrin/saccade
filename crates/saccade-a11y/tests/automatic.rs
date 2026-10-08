//! Permanent constructed ground truth: no model, network, font downloads or manual labels.
#![allow(missing_docs, clippy::unwrap_used)]
use image::{Rgb, RgbImage};
use saccade_a11y::auto::{self, Level, Options, Verdict};

// Original five-by-seven bitmap glyphs. Layout bounds are known by construction.
const GLYPHS: [[u8; 7]; 4] = [
    [31, 4, 4, 4, 4, 4, 4],
    [31, 16, 16, 30, 16, 16, 31],
    [17, 17, 10, 4, 10, 17, 17],
    [31, 4, 4, 4, 4, 4, 4],
];
fn scene(domain: usize, scale: u32, fg: u8, gradient: bool) -> (RgbImage, [u32; 4]) {
    let mut im = RgbImage::from_fn(300, 160, |x, y| {
        Rgb(match domain {
            0 => [235; 3], // UI canvas
            1 => [255; 3], // document
            2 => {
                if y < 22 {
                    [225, 230, 235]
                } else {
                    [250; 3]
                }
            } // page header
            3 => [20, 30, 40], // HUD-like canvas
            _ => [
                (80 + x / 4) as u8,
                (80 + y / 2) as u8,
                (80 + (x + y) / 6) as u8,
            ], // synthetic photo
        })
    });
    let (x, y) = (28 + domain as u32 * 7, 45 + domain as u32 * 3);
    let rect = [x, y, 29 * scale, 7 * scale];
    // A panel/document backing is known to the generator, never supplied to detection.
    for yy in y - 8..y + 7 * scale + 8 {
        for xx in x - 8..x + 29 * scale + 8 {
            let bg = if gradient {
                170 + ((xx - (x - 8)) * 85 / (29 * scale + 15)) as u8
            } else {
                255
            };
            im.put_pixel(xx, yy, Rgb([bg; 3]));
        }
    }
    for (i, g) in GLYPHS.iter().enumerate() {
        for (row, bits) in g.iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) == 0 {
                    continue;
                }
                for yy in 0..scale {
                    for xx in 0..scale {
                        im.put_pixel(
                            x + i as u32 * 8 * scale + col * scale + xx,
                            y + row as u32 * scale + yy,
                            Rgb([fg; 3]),
                        );
                    }
                }
            }
        }
    }
    (im, rect)
}
fn ratio(byte: u8) -> f64 {
    saccade_core::contrast::contrast_ratio([1.; 3], saccade_core::color::rgb([byte; 3]))
}
fn iou(a: [u32; 4], b: [u32; 4]) -> f64 {
    let w = (a[0] + a[2])
        .min(b[0] + b[2])
        .saturating_sub(a[0].max(b[0]));
    let h = (a[1] + a[3])
        .min(b[1] + b[3])
        .saturating_sub(a[1].max(b[1]));
    let intersection = f64::from(w * h);
    intersection / (f64::from(a[2] * a[3] + b[2] * b[3]) - intersection)
}
#[test]
fn constructed_cross_domain_detection_and_false_pass_gate() {
    let (mut truths, mut detected, mut matched, mut correct, mut failing, mut false_pass) =
        (0, 0, 0, 0, 0, 0);
    for domain in 0..5 {
        for (level, scale, threshold) in [
            (Level::AA, 2, 4.5),
            (Level::AA, 4, 3.),
            (Level::AAA, 2, 7.),
            (Level::AAA, 4, 4.5),
        ] {
            // Actual quantized swatches straddle the exact threshold, without rounding oracle ratios.
            let fail = (0u8..=255).find(|v| ratio(*v) < threshold).unwrap();
            for (fg, expected) in [(fail - 1, Verdict::Pass), (fail, Verdict::Fail)] {
                let (im, truth) = scene(domain, scale, fg, false);
                let options = Options {
                    level,
                    ..Default::default()
                };
                truths += 1;
                if expected == Verdict::Fail {
                    failing += 1;
                }
                let regions = auto::fallback(&im).unwrap();
                let text: Vec<_> = regions.into_iter().filter(|r| r.kind == "text").collect();
                detected += text.len();
                let best = text.into_iter().find(|r| iou(r.rect_px, truth) >= 0.5);
                if let Some(r) = best {
                    matched += 1;
                    let f = auto::measure(&im, r, &options).unwrap();
                    assert_eq!(f.assumed_large, scale == 4);
                    assert_eq!(f.required_ratio, threshold);
                    if f.verdict == expected {
                        correct += 1;
                    }
                    if expected == Verdict::Fail && f.verdict == Verdict::Pass {
                        false_pass += 1;
                    }
                    assert_eq!(
                        f.verdict,
                        expected,
                        "domain={domain} level={level:?} scale={scale} fg={fg} oracle={} {f:?}",
                        ratio(fg)
                    );
                }
            }
        }
        let (im, truth) = scene(domain, 2, 0, true);
        truths += 1;
        let text: Vec<_> = auto::fallback(&im)
            .unwrap()
            .into_iter()
            .filter(|r| r.kind == "text")
            .collect();
        detected += text.len();
        if let Some(r) = text.into_iter().find(|r| iou(r.rect_px, truth) >= 0.5) {
            matched += 1;
            let f = auto::measure(&im, r, &Options::default()).unwrap();
            assert_eq!(
                f.verdict,
                Verdict::Unmeasurable,
                "gradient domain={domain}: {f:?}"
            );
            correct += 1;
        }
    }
    let precision = matched as f64 / detected as f64;
    let recall = matched as f64 / truths as f64;
    let accuracy = correct as f64 / truths as f64;
    let upper = 1. - 0.05_f64.powf(1. / failing as f64);
    println!(
        "AUTO_A11Y_EVAL text_precision={matched}/{detected} ({precision:.6}) text_recall={matched}/{truths} ({recall:.6}) contrast_accuracy={correct}/{truths} ({accuracy:.6}) false_PASS={false_pass}/{failing} finite_corpus_upper=0 binomial_one_sided_95_upper={upper:.6} IoU>=0.5; independent-case assumption unqualified"
    );
    assert_eq!(false_pass, 0);
    assert_eq!(matched, truths, "constructed text recall must not regress");
    assert_eq!(
        detected, matched,
        "constructed text precision must not regress"
    );
    assert_eq!(
        correct, truths,
        "misses/abstentions count against verdict accuracy"
    );
}
#[test]
fn controls_status_markers_and_no_text_are_not_implicit_passes() {
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("controls.png");
    let mut im = RgbImage::from_pixel(140, 70, Rgb([255; 3]));
    for (x, color) in [(20, [180; 3]), (80, [210; 3])] {
        for y in 20..44 {
            for xx in x..x + 24 {
                if y == 20 || y == 43 || xx == x || xx == x + 23 {
                    im.put_pixel(xx, y, Rgb(color));
                }
            }
        }
    }
    im.save(&input).unwrap();
    let report =
        saccade_a11y::run(&input, &tmp.path().join("report"), &Options::default()).unwrap();
    let image = &report.images[0];
    assert!(image.text_detection.starts_with("no text detected by"));
    assert!(image.detectors.iter().any(|d| d.state == "unavailable"));
    assert_eq!(
        image
            .automatic
            .iter()
            .filter(|f| f.region.kind == "ui")
            .count(),
        2
    );
    assert!(
        image
            .automatic
            .iter()
            .all(|f| f.verdict == Verdict::Fail && f.required_ratio == 3.)
    );
    assert_eq!(report.verdict, Verdict::Fail);
    let blank = tmp.path().join("blank.png");
    RgbImage::from_pixel(60, 40, Rgb([255; 3]))
        .save(&blank)
        .unwrap();
    let report = saccade_a11y::run(
        &blank,
        &tmp.path().join("blank-report"),
        &Options::default(),
    )
    .unwrap();
    assert_eq!(report.verdict, Verdict::Unmeasurable);
    let junit = tmp.path().join("blank.xml");
    report.junit(&junit, &[&blank]).unwrap();
    assert!(std::fs::read_to_string(junit).unwrap().contains("<skipped"));
    // Colour-only markers have no inferred semantics; the existing information-loss pass warns.
    let markers = tmp.path().join("status.png");
    RgbImage::from_fn(64, 32, |x, _| {
        Rgb(if x < 32 {
            [218, 108, 80]
        } else {
            [108, 166, 71]
        })
    })
    .save(&markers)
    .unwrap();
    let report = saccade_a11y::run(
        &markers,
        &tmp.path().join("status-report"),
        &Options::default(),
    )
    .unwrap();
    assert!(
        !report.images[0]
            .declared_and_colour_vision
            .findings
            .is_empty()
    );
    assert_ne!(report.verdict, Verdict::Pass);
}
#[test]
fn declarations_scale_glyphs_and_guards_preserve_honesty() {
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("text.png");
    let (im, _) = scene(0, 4, 130, false);
    im.save(&input).unwrap();
    let config = tmp.path().join("regions.toml");
    std::fs::write(
        &config,
        "[[region]]\nname='authoritative'\nkind='text'\nrect=[0.06666666666666667,0.23125,0.44,0.275]\nlarge=false\n",
    )
    .unwrap();
    let report = saccade_a11y::run(
        &input,
        &tmp.path().join("declared"),
        &Options {
            config: Some(config.clone()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        report.images[0].declared_and_colour_vision.contrast[0].verdict,
        "FAIL"
    );
    assert!(
        report.images[0]
            .automatic
            .iter()
            .filter(|f| f.region.kind == "text")
            .all(|f| f.region.overridden_by_declared && f.verdict == Verdict::Unmeasurable)
    );
    let r = auto::fallback(&im)
        .unwrap()
        .into_iter()
        .find(|r| r.kind == "text")
        .unwrap();
    let f = auto::measure(
        &im,
        r,
        &Options {
            px_per_pt: 2.,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!f.assumed_large);
    assert_eq!(f.required_ratio, 4.5);
    assert_eq!(f.verdict, Verdict::Fail);
    // A detector's expanded box cannot promote a normal-size failing body to large text.
    let (normal, _) = scene(0, 2, 130, false);
    let mut expanded = auto::fallback(&normal)
        .unwrap()
        .into_iter()
        .find(|r| r.kind == "text")
        .unwrap();
    expanded.text_height_px = 32;
    let check = auto::measure(&normal, expanded, &Options::default()).unwrap();
    assert!(!check.assumed_large);
    assert_eq!(check.verdict, Verdict::Fail);
    assert!(saccade_a11y::run(&input, &input, &Options::default()).is_err());
    assert!(
        saccade_a11y::run(
            &input,
            &tmp.path().join("invalid"),
            &Options {
                px_per_pt: f64::NAN,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(auto::fallback(&RgbImage::new(0, 2)).is_err());
    let mut glyph = RgbImage::from_pixel(90, 45, Rgb([255; 3]));
    for y in 10..34 {
        for x in 15..29 {
            if !(17..27).contains(&x) || !(12..32).contains(&y) {
                glyph.put_pixel(x, y, Rgb([0; 3]));
            }
        }
    }
    let r = auto::Region {
        kind: "text".into(),
        rect_px: [10, 5, 30, 35],
        text_height_px: 24,
        provenance: "auto_detected".into(),
        detector: auto::FALLBACK.into(),
        confidence: 0.7,
        overridden_by_declared: false,
    };
    let f = auto::measure(&glyph, r, &Options::default()).unwrap();
    assert_eq!(f.glyph_verdict, Verdict::Warn);
    assert_eq!(f.missing_glyphs[0].rect_px, [15, 10, 14, 24]);
}
