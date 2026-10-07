//! Focused deterministic pixel behaviours; generated multi-script proof is in the CLI crate.
#![cfg(feature = "text-quality")]
#![allow(clippy::unwrap_used)]
use image::{Rgba, RgbaImage};
use saccade_core::text_quality::{self as tq, State};
#[test]
fn triage_has_absolute_coordinates_and_never_certifies_clean() {
    let mut image = RgbaImage::from_pixel(100, 60, Rgba([255, 255, 255, 255]));
    for y in 12..36 {
        for x in 30..44 {
            if !(32..42).contains(&x) || !(14..34).contains(&y) {
                image.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
    }
    let report = tq::tofu(
        &image,
        None,
        "a".repeat(64),
        None,
        None,
        tq::unavailable_ocr("not requested"),
    )
    .unwrap();
    assert_eq!(report.state, State::Candidates);
    assert_eq!(report.regions[0].rect_px, [30, 12, 14, 24]);
    let empty = RgbaImage::from_pixel(100, 60, Rgba([255, 255, 255, 255]));
    assert_eq!(
        tq::tofu(
            &empty,
            None,
            "b".repeat(64),
            None,
            None,
            tq::unavailable_ocr("not requested")
        )
        .unwrap()
        .state,
        State::InsufficientEvidence
    );
}
#[test]
fn invalid_geometry_and_policy_are_errors() {
    assert!(tq::validate_rect([20, 20], [u32::MAX, 0, 2, 2]).is_err());
    let image = RgbaImage::from_pixel(20, 20, Rgba([255, 255, 255, 255]));
    assert!(
        tq::legibility(
            &image,
            [0, 0, 20, 20],
            0,
            tq::Policy {
                minimum_contrast: f64::NAN,
                ..Default::default()
            }
        )
        .is_err()
    );
    let mask = RgbaImage::new(2, 2);
    assert!(
        tq::tofu(
            &image,
            Some(&mask),
            "a".repeat(64),
            None,
            None,
            tq::unavailable_ocr("none")
        )
        .is_err()
    );
}
#[test]
fn a_small_body_cannot_hide_among_large_bodies() {
    let mut image = RgbaImage::from_pixel(100, 60, Rgba([255, 255, 255, 255]));
    for (x, height) in [(10, 20), (30, 20), (50, 20), (70, 4)] {
        for y in 10..10 + height {
            for xx in x..x + 3 {
                image.put_pixel(xx, y, Rgba([0, 0, 0, 255]));
            }
        }
    }
    let report = tq::legibility(&image, [0, 0, 100, 60], 0, Default::default()).unwrap();
    assert_eq!(report.state, State::Illegible);
    assert!(report.reasons.iter().any(|r| r == "too_small"));
}

#[test]
fn low_contrast_body_cannot_hide_among_dark_bodies() {
    let mut image = RgbaImage::from_pixel(100, 60, Rgba([255, 255, 255, 255]));
    for (x, color) in [(10, 0), (30, 0), (50, 0), (70, 230)] {
        for y in 10..30 {
            for xx in x..x + 3 {
                image.put_pixel(xx, y, Rgba([color, color, color, 255]));
            }
        }
    }
    let report = tq::legibility(&image, [0, 0, 100, 60], 0, Default::default()).unwrap();
    assert_eq!(report.state, State::Illegible);
    assert!(report.reasons.iter().any(|r| r == "low_contrast"));
}
