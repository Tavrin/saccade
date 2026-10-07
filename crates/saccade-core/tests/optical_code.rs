//! Generated optical-code truth: decode and policy evidence stay separate.
#![allow(clippy::unwrap_used, missing_docs)]
use saccade_core::general::optical_code::{self as oc, Options};
#[cfg(feature = "optical-code")]
use saccade_core::general::optical_code::{Expected, State, Symbology};
#[cfg(feature = "optical-code")]
#[path = "support/optical.rs"]
mod fixtures;

#[cfg(not(feature = "optical-code"))]
#[test]
fn unavailable_is_not_decode_failure() {
    assert!(matches!(
        oc::verify(&image::RgbaImage::new(1, 1), Options::default()),
        Err(saccade_core::Error::FeatureUnavailable {
            feature: "optical-code"
        })
    ));
}
#[cfg(feature = "optical-code")]
#[test]
fn generated_qr_truth_and_pixel_margins() {
    let image = fixtures::qr("https://example.org/item/7", 4, 0, 255);
    let policy = Options {
        expected: Some(Expected::Exact("https://example.org/item/7".into())),
        minimum_module_px: Some(3.),
        minimum_contrast: Some(0.8),
        require_quiet_zone: true,
        ..Default::default()
    };
    let good = oc::verify(&image, policy.clone()).unwrap();
    assert_eq!(good.verdict, "pass");
    assert_eq!(good.state, State::Decoded);
    assert_eq!(good.payload_match, Some(true));
    assert!((good.quality.module_size_px.unwrap() - 4.).abs() < 0.1);
    assert!(good.quality.contrast.unwrap() > 0.99);
    assert_eq!(good.quality.quiet_zone_modules, Some([4; 4]));
    let b = good.bounding_box.unwrap();
    assert!((b[0] as i32 - 16).abs() <= 1);
    assert!(b[2] > 80);
    let mismatch = oc::verify(
        &fixtures::qr("https://example.org/item/8", 4, 0, 255),
        policy.clone(),
    )
    .unwrap();
    assert_eq!(mismatch.state, State::Decoded);
    assert_eq!(mismatch.payload_match, Some(false));
    assert!(mismatch.failures.contains(&"payload_mismatch".into()));
    let small = oc::verify(
        &fixtures::qr("https://example.org/item/7", 2, 0, 255),
        policy.clone(),
    )
    .unwrap();
    assert_eq!(small.state, State::Decoded);
    assert!(small.failures.contains(&"module_size_below_minimum".into()));
    let low = oc::verify(
        &fixtures::qr("https://example.org/item/7", 4, 20, 180),
        policy,
    )
    .unwrap();
    assert_eq!(low.state, State::Decoded);
    assert!(low.quality.contrast.unwrap() < 0.65);
    assert!(low.failures.contains(&"contrast_below_minimum".into()));
    let very_low = oc::verify(
        &fixtures::qr("https://example.org/item/7", 4, 180, 210),
        Options::default(),
    )
    .unwrap();
    assert_eq!(very_low.state, State::NotFound);
    assert!(very_low.quality.contrast.is_none());
}
#[cfg(feature = "optical-code")]
#[test]
fn damage_absence_transparency_and_cropped_quiet_zone() {
    let mut damaged = fixtures::qr("https://example.org/item/7", 4, 0, 255);
    // Preserve finder patterns while destroying data and ECC modules.
    for y in 16 + 8 * 4..damaged.height() - 16 {
        for x in 16 + 8 * 4..damaged.width() - 16 {
            damaged.put_pixel(x, y, image::Rgba([255; 4]));
        }
    }
    let result = oc::verify(&damaged, Options::default()).unwrap();
    assert!(result.found);
    assert_eq!(result.state, State::NotDecodable);
    assert!(result.payload.is_none());
    let blank = oc::verify(
        &image::RgbaImage::from_pixel(120, 120, image::Rgba([255; 4])),
        Options::default(),
    )
    .unwrap();
    assert_eq!(blank.state, State::NotFound);
    let mut invisible = fixtures::qr("Sample", 4, 0, 255);
    for p in invisible.pixels_mut() {
        p[3] = 0;
    }
    assert_eq!(
        oc::verify(&invisible, Options::default()).unwrap().state,
        State::NotFound
    );
    let code = fixtures::qr("Sample", 4, 0, 255);
    let cropped = oc::verify(
        &code,
        Options {
            region: Some([16, 16, code.width() - 32, code.height() - 32]),
            require_quiet_zone: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(cropped.state, State::Decoded);
    assert_ne!(cropped.quality.quiet_zone_clear, Some(true));
    assert_eq!(cropped.verdict, "fail");
}
#[cfg(feature = "optical-code")]
#[test]
fn full_payload_patterns_rotations_offsets_and_invalid_policy() {
    let code = fixtures::qr("prefix-Sample-suffix", 4, 0, 255);
    let result = oc::verify(
        &code,
        Options {
            expected: Some(Expected::Pattern("Sample".into())),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.payload_match, Some(false));
    let page = fixtures::scene(&image::imageops::rotate90(&code));
    let result = oc::verify(
        &page,
        Options {
            expected: Some(Expected::Pattern("prefix-.*-suffix".into())),
            region: Some([80, 120, code.width(), code.height()]),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.verdict, "pass");
    assert!(result.bounding_box.unwrap()[0] >= 80);
    for options in [
        Options {
            region: Some([u32::MAX, 0, 2, 2]),
            ..Default::default()
        },
        Options {
            minimum_module_px: Some(f64::NAN),
            ..Default::default()
        },
        Options {
            expected: Some(Expected::Pattern("[".into())),
            ..Default::default()
        },
    ] {
        assert!(oc::verify(&code, options).is_err());
    }
}
#[cfg(feature = "optical-code")]
#[test]
fn common_1d_and_2d_decoders_keep_unsupported_quality_unavailable() {
    for (format, symbology, payload) in [
        (
            rxing::BarcodeFormat::CODE_128,
            Symbology::Code128,
            "Sample-123",
        ),
        (
            rxing::BarcodeFormat::DATA_MATRIX,
            Symbology::DataMatrix,
            "Sample-123",
        ),
    ] {
        let code = fixtures::barcode(payload, format);
        let options = Options {
            symbology,
            expected: Some(Expected::Exact(payload.into())),
            ..Default::default()
        };
        let result = oc::verify(&code, options.clone()).unwrap();
        assert_eq!(result.verdict, "pass");
        assert_eq!(result.state, State::Decoded);
        assert!(result.bounding_box.is_some());
        let gate = oc::verify(
            &code,
            Options {
                minimum_module_px: Some(2.),
                ..options
            },
        )
        .unwrap();
        assert!(gate.failures.contains(&"module_size_unavailable".into()));
    }
}
