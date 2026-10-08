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
fn bitmap_detection_regression() {
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
                    scale_known: true,
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
                    if expected == Verdict::Fail {
                        assert!(
                            matches!(f.verdict, Verdict::Fail | Verdict::Unmeasurable),
                            "{f:?}"
                        );
                    } else {
                        assert_eq!(f.verdict, expected, "{f:?}");
                    }
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
        "BITMAP_REGRESSION text_precision={matched}/{detected} ({precision:.6}) text_recall={matched}/{truths} ({recall:.6}) contrast_accuracy={correct}/{truths} ({accuracy:.6}) false_PASS={false_pass}/{failing} finite_corpus_upper=0 binomial_one_sided_95_upper={upper:.6} IoU>=0.5; independent-case assumption unqualified"
    );
    assert_eq!(false_pass, 0);
    assert_eq!(matched, truths, "constructed text recall must not regress");
    assert_eq!(
        detected, matched,
        "constructed text precision must not regress"
    );
    assert!(
        correct >= truths - failing,
        "all positive and gradient controls remain qualified; thin negative abstentions count against accuracy"
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
            scale_known: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!f.assumed_large);
    assert_eq!(f.required_ratio, 4.5);
    assert_eq!(f.verdict, Verdict::Fail);
    // A detector's expanded box cannot promote a normal-size failing body to large text.
    let (normal, _) = scene(0, 3, 130, false);
    let mut expanded = auto::fallback(&normal)
        .unwrap()
        .into_iter()
        .find(|r| r.kind == "text")
        .unwrap();
    expanded.text_height_px = 32;
    let check = auto::measure(
        &normal,
        expanded,
        &Options {
            scale_known: true,
            ..Default::default()
        },
    )
    .unwrap();
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

fn font_scene(fg: [u8; 3], bg: [u8; 3], texture: bool) -> (RgbImage, Vec<[u32; 4]>) {
    use ab_glyph::{Font, FontRef, ScaleFont};
    let font = FontRef::try_from_slice(include_bytes!("fonts/DejaVuSans.ttf")).unwrap();
    let font = font.as_scaled(24.);
    let mut im = RgbImage::from_fn(400, 100, |x, y| {
        Rgb(if texture {
            [((x * 11 + y * 7) % 160 + 60) as u8; 3]
        } else {
            bg
        })
    });
    let mut boxes = Vec::new();
    for (line, baseline) in [
        ("The quick brown fox", 35.),
        ("Settings  Profile  Help", 75.),
    ] {
        let mut x = 18.;
        let (mut left, mut top, mut right, mut bottom) = (400, 100, 0, 0);
        for c in line.chars() {
            let id = font.glyph_id(c);
            let glyph = id.with_scale_and_position(24., ab_glyph::point(x, baseline));
            if let Some(outline) = font.outline_glyph(glyph) {
                let b = outline.px_bounds();
                left = left.min(b.min.x as u32);
                top = top.min(b.min.y as u32);
                right = right.max(b.max.x as u32);
                bottom = bottom.max(b.max.y as u32);
                outline.draw(|xx, yy, coverage| {
                    let (xx, yy) = (xx + b.min.x as u32, yy + b.min.y as u32);
                    let backing = im.get_pixel(xx, yy).0;
                    im.put_pixel(
                        xx,
                        yy,
                        Rgb(std::array::from_fn(|i| {
                            (backing[i] as f32 * (1. - coverage) + fg[i] as f32 * coverage).round()
                                as u8
                        })),
                    );
                });
            }
            x += font.h_advance(id);
        }
        boxes.push([left - 3, top - 3, right - left + 6, bottom - top + 6]);
    }
    (im, boxes)
}
#[test]
fn antialiased_multiline_local_ink_offline_gate() {
    let (mut detected, mut matched, mut measured, mut false_pass, mut failures) = (0, 0, 0, 0, 0);
    let (mut correct, mut false_fail) = (0, 0);
    let mut max_error = 0f64;
    let mut truths = 0;
    // Actual quantized pairs on either side of 3, 4.5 and 7, light and dark.
    for bg in [[255; 3], [0; 3], [255, 176, 0]] {
        for threshold in [3., 4.5, 7.] {
            let cross = (0..=255u8)
                .min_by(|a, b| {
                    let r = |v| {
                        saccade_core::contrast::contrast_ratio(
                            saccade_core::color::rgb([v; 3]),
                            saccade_core::color::rgb(bg),
                        )
                    };
                    (r(*a) - threshold)
                        .abs()
                        .total_cmp(&(r(*b) - threshold).abs())
                })
                .unwrap();
            for fg in [cross.saturating_sub(1), cross.saturating_add(1)] {
                let true_ratio = saccade_core::contrast::contrast_ratio(
                    saccade_core::color::rgb([fg; 3]),
                    saccade_core::color::rgb(bg),
                );
                let options = Options {
                    level: if threshold == 7. {
                        Level::AAA
                    } else {
                        Level::AA
                    },
                    scale_known: threshold == 3.,
                    px_per_pt: if threshold == 3. { 0.5 } else { 96. / 72. },
                    ..Default::default()
                };
                let (im, boxes) = font_scene([fg; 3], bg, false);
                let regions = auto::fallback(&im).unwrap();
                let text: Vec<_> = regions.into_iter().filter(|r| r.kind == "text").collect();
                detected += text.len();
                truths += boxes.len();
                for box_ in boxes {
                    if let Some(r) = text.iter().find(|r| iou(r.rect_px, box_) >= 0.5) {
                        matched += 1;
                        let f = auto::measure(&im, r.clone(), &options).unwrap();
                        let expected = if true_ratio >= threshold {
                            Verdict::Pass
                        } else {
                            Verdict::Fail
                        };
                        correct += usize::from(f.verdict == expected);
                    }
                }
                for r in text {
                    let f = auto::measure(&im, r, &options).unwrap();
                    if true_ratio < threshold {
                        failures += 1;
                        if f.verdict == Verdict::Pass {
                            false_pass += 1;
                        }
                    } else if f.verdict == Verdict::Fail {
                        false_fail += 1;
                    }
                    assert!(
                        f.required_ratio >= threshold,
                        "size proxy must not relax below declared oracle scale"
                    );
                    if let Some(ratio) = f.ratio {
                        if f.verdict != Verdict::Unmeasurable {
                            measured += 1;
                        }
                        let error = (ratio / true_ratio - 1.).abs();
                        max_error = max_error.max(error);
                        assert!(
                            error <= 0.10
                                || (ratio <= true_ratio
                                    && f.reasons.iter().any(|r| r.contains("lower bound"))),
                            "true={true_ratio} {f:?}"
                        );
                    }
                    assert!(
                        !(true_ratio >= f.required_ratio && f.verdict == Verdict::Fail),
                        "false FAIL true={true_ratio}: {f:?}"
                    );
                }
            }
        }
    }
    {
        let (im, _) = font_scene([128; 3], [255; 3], true);
        for r in auto::fallback(&im)
            .unwrap()
            .into_iter()
            .filter(|r| r.kind == "text")
        {
            assert_ne!(
                auto::measure(&im, r, &Options::default()).unwrap().verdict,
                Verdict::Pass
            );
        }
    }
    println!(
        "AUTO_A11Y_EVAL real_font multiline text_precision={matched}/{detected} text_recall={matched}/{truths} contrast_accuracy={correct}/{truths} measured={measured}/{detected} false_PASS={false_pass}/{failures} false_FAIL_vs_oracle={false_fail} max_relative_ratio_error={max_error:.6}; finite corpus only"
    );
    assert_eq!(false_pass, 0);
    assert!(matched > 0);
}
#[test]
fn mixed_ink_backing_boundary_and_unknown_scale_never_pass() {
    let mut measured = 0;
    for bytes in [
        include_bytes!("fixtures/mixed-red.png").as_slice(),
        include_bytes!("fixtures/mixed-strokes.png").as_slice(),
        include_bytes!("fixtures/local-background-245.png").as_slice(),
        include_bytes!("fixtures/mostly-low-ui.png").as_slice(),
        include_bytes!("fixtures/real-font-dpr2.png").as_slice(),
    ] {
        let im = image::load_from_memory(bytes).unwrap().to_rgb8();
        for r in auto::fallback(&im).unwrap() {
            let f = auto::measure(&im, r, &Options::default()).unwrap();
            assert_ne!(f.verdict, Verdict::Pass, "{f:?}");
            if f.ratio.is_some() {
                measured += 1;
            }
        }
    }
    println!("AUTO_A11Y_REGRESSIONS false_PASS=0 measured_findings={measured}");
}

#[test]
fn thin_mixed_direction_ink_cannot_claim_a_lower_bound() {
    // A half-covered red stroke on green gives this brown sample. Its measured
    // 3.057 ratio exceeds the actual red/green 2.914; it cannot PASS at 3.
    for width in [1, 2] {
        let mut im = RgbImage::from_pixel(70, 40, Rgb([0, 255, 0]));
        for x in [10, 22, 34] {
            for y in 8..22 {
                for xx in x..x + width {
                    im.put_pixel(xx, y, Rgb([128, 128, 0]));
                }
            }
        }
        let options = Options {
            px_per_pt: 0.5,
            scale_known: true,
            ..Default::default()
        };
        let region = auto::fallback(&im)
            .unwrap()
            .into_iter()
            .find(|r| r.kind == "text")
            .unwrap();
        let f = auto::measure(&im, region, &options).unwrap();
        assert_eq!(f.verdict, Verdict::Unmeasurable, "{f:?}");
        assert_eq!(f.required_ratio, 3.);
        assert!(f.assumed_large);
        assert!(f.reasons.iter().any(|r| r.contains("mixed-direction")));
    }
}
