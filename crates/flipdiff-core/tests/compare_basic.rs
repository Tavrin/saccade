#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use flipdiff_core::compare::{CompareOptions, compare};
use flipdiff_core::{Error, properties};
use image::{Rgb, RgbImage};

fn stripes(w: u32, h: u32, shift: u32) -> RgbImage {
    RgbImage::from_fn(w, h, |x, _| {
        if ((x + shift) / 4) % 2 == 0 {
            Rgb([0, 0, 0])
        } else {
            Rgb([255, 255, 255])
        }
    })
}

fn gradient(w: u32, h: u32) -> RgbImage {
    RgbImage::from_fn(w, h, |x, y| Rgb([(x * 3) as u8, (y * 3) as u8, 128]))
}

#[test]
fn identical_images_have_zero_error() {
    let img = gradient(64, 64);
    let c = compare(&img, &img, &CompareOptions::default()).expect("compare");
    assert_eq!(c.metrics.mean, 0.0);
    assert_eq!(c.metrics.max, 0.0);
    assert_eq!(c.error_map.len(), 64 * 64);
    assert_eq!(c.heatmap_rgb().dimensions(), (64, 64));
}

#[test]
fn one_pixel_shift_of_high_contrast_pattern_fails_at_default_threshold() {
    let c = compare(
        &stripes(64, 64, 1),
        &stripes(64, 64, 0),
        &CompareOptions::default(),
    )
    .expect("compare");
    assert!(c.metrics.mean > 0.01, "mean = {}", c.metrics.mean);
}

#[test]
fn low_amplitude_noise_passes_at_0_05() {
    let base = gradient(64, 64);
    let mut state = 12345u32;
    let noisy = RgbImage::from_fn(64, 64, |x, y| {
        let mut p = *base.get_pixel(x, y);
        for ch in &mut p.0 {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            let d = ((state >> 24) % 3) as i16 - 1;
            *ch = (i16::from(*ch) + d).clamp(0, 255) as u8;
        }
        p
    });
    let c = compare(&noisy, &base, &CompareOptions::default()).expect("compare");
    assert!(c.metrics.mean <= 0.05, "mean = {}", c.metrics.mean);
}

#[test]
fn size_mismatch_and_empty_are_errors() {
    let a = gradient(8, 8);
    let b = gradient(8, 9);
    assert!(matches!(
        compare(&a, &b, &CompareOptions::default()),
        Err(Error::DimensionMismatch { .. })
    ));
    let empty = RgbImage::new(0, 0);
    assert!(matches!(
        compare(&empty, &empty, &CompareOptions::default()),
        Err(Error::EmptyImage)
    ));
}

#[test]
fn properties_flag_black_and_white_frames() {
    let black = properties::validate(&RgbImage::from_pixel(4, 4, Rgb([0, 0, 0])));
    assert!(black.is_all_black && !black.is_all_white);
    let white = properties::validate(&RgbImage::from_pixel(4, 4, Rgb([255, 255, 255])));
    assert!(white.is_all_white && (white.mean_luminance - 1.0).abs() < 1e-4);
}
