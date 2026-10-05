#![allow(clippy::unwrap_used, clippy::panic, missing_docs)]
use saccade_core::dense_motion::*;
fn sidecar(dimensions: [u32; 2]) -> Sidecar {
    Sidecar {
        schema: "saccade-motion-vectors.v1".into(),
        dimensions,
        reference_sha256: "a".repeat(64),
        candidate_sha256: "b".repeat(64),
        vectors_sha256: "c".repeat(64),
        units: Units::Pixels,
        origin: Origin::TopLeft,
        direction: Direction::ReferenceToCandidate,
        reference_jitter_px: [0.0; 2],
        candidate_jitter_px: [0.0; 2],
        includes_jitter: false,
        frame_interval_ms: 16.0,
        producer: serde_json::json!({"fixture":"analytic translation"}),
    }
}
#[test]
fn explicit_units_direction_jitter_and_byte_identity_are_validated_without_producer() {
    for direction in [
        Direction::ReferenceToCandidate,
        Direction::CandidateToReference,
    ] {
        for units in [Units::Pixels, Units::Uv, Units::Ndc] {
            for origin in [Origin::TopLeft, Origin::BottomLeft] {
                let mut s = sidecar([32, 16]);
                s.direction = direction;
                s.units = units;
                s.origin = origin;
                s.includes_jitter = true;
                s.candidate_jitter_px = [0.5, -0.25];
                let sign = if direction == Direction::ReferenceToCandidate {
                    1.0
                } else {
                    -1.0
                };
                let scale = match units {
                    Units::Pixels => [1.0, 1.0],
                    Units::Uv => [32.0, 16.0],
                    Units::Ndc => [16.0, 8.0],
                };
                let v = [
                    sign * 2.5 / scale[0],
                    sign * 0.75 / scale[1]
                        * if origin == Origin::BottomLeft {
                            -1.0
                        } else {
                            1.0
                        },
                ];
                let b = Buffer {
                    schema: "saccade-vector-buffer.v1".into(),
                    vectors: vec![v; 512],
                    valid: vec![true; 512],
                };
                assert_eq!(
                    s.normalize(&b, &["a".repeat(64), "b".repeat(64), "c".repeat(64)])
                        .unwrap()[0],
                    [sign * 2.0, sign]
                );
                let mut bad = b.clone();
                bad.vectors[0][0] = f32::NAN;
                assert!(
                    s.normalize(&bad, &["a".repeat(64), "b".repeat(64), "c".repeat(64)])
                        .is_err()
                );
                assert!(
                    s.normalize(&b, &["a".repeat(64), "b".repeat(64), "d".repeat(64)])
                        .is_err()
                );
                bad = b.clone();
                bad.valid.pop();
                assert!(
                    s.normalize(&bad, &["a".repeat(64), "b".repeat(64), "c".repeat(64)])
                        .is_err()
                );
            }
        }
    }
}
#[cfg(not(feature = "dense-motion"))]
#[test]
fn reports_remain_readable_but_producer_is_explicitly_unavailable() {
    let a = image::RgbaImage::new(16, 16);
    assert!(matches!(
        review(&a, &a, ["a".repeat(64), "b".repeat(64)], None, 67.0, 0.01),
        Err(saccade_core::Error::FeatureUnavailable {
            feature: "dense-motion"
        })
    ));
    let state: State = serde_json::from_str("\"possible_occlusion_or_mismatch\"").unwrap();
    assert_eq!(state, State::PossibleOcclusionOrMismatch);
}
#[cfg(feature = "dense-motion")]
mod compute {
    use super::*;
    fn texture(x: u32, y: u32, salt: u32) -> u8 {
        let n = (x.wrapping_mul(73856093) ^ y.wrapping_mul(19349663) ^ salt.wrapping_mul(83492791))
            .wrapping_mul(2654435761);
        (40 + ((n >> 24) % 176)) as u8
    }
    fn image() -> image::RgbaImage {
        image::RgbaImage::from_fn(64, 64, |x, y| {
            let v = texture(x, y, 1);
            image::Rgba([v, v, v, 255])
        })
    }
    fn translated(a: &image::RgbaImage, dx: i32, dy: i32) -> image::RgbaImage {
        image::RgbaImage::from_fn(a.width(), a.height(), |x, y| {
            *a.get_pixel(
                (x as i32 - dx).clamp(0, a.width() as i32 - 1) as u32,
                (y as i32 - dy).clamp(0, a.height() as i32 - 1) as u32,
            )
        })
    }
    fn report(a: &image::RgbaImage, b: &image::RgbaImage) -> Report {
        review(a, b, ["a".repeat(64), "b".repeat(64)], None, 67.0, 0.01).unwrap()
    }
    fn interior(field: &Field, rect: [usize; 4], width: usize, expected: [f32; 2]) -> (usize, f32) {
        let mut count = 0;
        let mut error = 0.0;
        for y in rect[1]..rect[1] + rect[3] {
            for x in rect[0]..rect[0] + rect[2] {
                let i = y * width + x;
                if field.states[i] == State::Qualified {
                    count += 1;
                    error += (field.vectors[i][0] - expected[0])
                        .hypot(field.vectors[i][1] - expected[1]);
                }
            }
        }
        (count, error / count.max(1) as f32)
    }
    #[test]
    fn analytic_plane_translation_and_reverse_grid_have_correct_sign_and_scale() {
        let a = image();
        let b = translated(&a, 2, -1);
        let r = report(&a, &b);
        for (f, expected) in r.fields.iter().zip([[2.0, -1.0], [-2.0, 1.0]]) {
            let (n, e) = interior(f, [16, 16, 32, 32], 64, expected);
            assert!(n > 700, "support {n}");
            assert!(e < 0.15, "endpoint {e}");
        }
        assert!(r.fields[0].states.contains(&State::Boundary));
        assert!(r.raw_regression);
    }
    #[test]
    fn independent_object_and_stationary_background_do_not_reduce_to_global_translation() {
        let mut a = image();
        let mut b = a.clone();
        for y in 16..48 {
            for x in 24..56 {
                let v = texture(x, y, 7);
                a.put_pixel(x, y, image::Rgba([v, v, v, 255]));
            }
        }
        for y in 17..49 {
            for x in 26..58 {
                b.put_pixel(x, y, *a.get_pixel(x - 2, y - 1));
            }
        }
        let r = report(&a, &b);
        let (n, e) = interior(&r.fields[0], [34, 26, 12, 12], 64, [2.0, 1.0]);
        assert!(n > 95, "object support {n}");
        assert!(e < 0.3, "object {e}");
        let (n, e) = interior(&r.fields[0], [8, 20, 8, 20], 64, [0.0, 0.0]);
        assert!(n > 100, "background support {n}");
        assert!(e < 0.3, "background {e}");
        assert!(r.fields[0].states.iter().any(|s| matches!(
            s,
            State::MotionBoundary | State::PossibleOcclusionOrMismatch | State::AmbiguousTexture
        )));
    }
    #[test]
    fn occlusion_appearance_and_flat_texture_are_uncertain_and_raw_flip_is_preserved() {
        let a = image();
        let mut b = translated(&a, 2, -1);
        for y in 20..36 {
            for x in 20..36 {
                b.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
            }
        }
        let r = report(&a, &b);
        let raw = saccade_core::compare::compare_rgba(&b, &a, &Default::default()).unwrap();
        assert_eq!(r.raw_flip.mean, raw.metrics.mean);
        assert!(r.raw_regression);
        let uncertain = (20..36)
            .flat_map(|y| (20..36).map(move |x| y * 64 + x))
            .filter(|&i| r.fields[1].states[i] != State::Qualified)
            .count();
        assert!(uncertain > 200, "occluded support {uncertain}");
        let flat = image::RgbaImage::from_pixel(32, 32, image::Rgba([128, 128, 128, 255]));
        assert_eq!(report(&flat, &flat).fields[0].qualified_pixels, 0);
        let repeated = image::RgbaImage::from_fn(64, 64, |x, y| {
            let v = texture(x % 8, y % 8, 1);
            image::Rgba([v, v, v, 255])
        });
        assert!(report(&repeated, &repeated).fields[0].qualified_pixels < 64 * 64 / 4);
        let mut alpha = a.clone();
        alpha.put_pixel(0, 0, image::Rgba([0, 0, 0, 0]));
        assert!(
            review(
                &alpha,
                &b,
                ["a".repeat(64), "b".repeat(64)],
                None,
                67.0,
                0.01
            )
            .is_err()
        );
    }
    #[test]
    fn w3_f06_six_pixel_alias_is_ambiguous() {
        let repeated = image::RgbaImage::from_fn(64, 64, |x, y| {
            let v = texture(x % 6, y % 6, 1);
            image::Rgba([v, v, v, 255])
        });
        let r = report(&repeated, &repeated);
        assert_eq!(r.raw_flip.mean, 0.);
        for f in &r.fields {
            assert!(
                f.qualified_pixels < 64 * 64 / 4,
                "{} aliased pixels qualified",
                f.qualified_pixels
            );
        }
    }
    #[test]
    fn renderer_sign_scale_and_jitter_errors_are_measured_only_on_supported_pixels() {
        let a = image();
        let b = translated(&a, 2, 1);
        for direction in [
            Direction::ReferenceToCandidate,
            Direction::CandidateToReference,
        ] {
            let mut s = sidecar([64, 64]);
            s.direction = direction;
            s.origin = Origin::BottomLeft;
            s.units = Units::Uv;
            s.candidate_jitter_px = [1.0, 1.0];
            s.includes_jitter = true;
            let sign = if direction == Direction::ReferenceToCandidate {
                1.0
            } else {
                -1.0
            };
            let buffer = Buffer {
                schema: "saccade-vector-buffer.v1".into(),
                vectors: vec![[sign * 2.0 / 64.0, -sign / 64.0]; 4096],
                valid: vec![true; 4096],
            };
            let good = review(
                &a,
                &b,
                ["a".repeat(64), "b".repeat(64)],
                Some((&s, &buffer, &"c".repeat(64))),
                67.0,
                0.01,
            )
            .unwrap();
            let v = good.renderer.unwrap();
            assert!(v.compared_pixels > 1800);
            assert!(v.mean_endpoint_px.unwrap() < 0.15);
            assert!(v.coverage < 1.0);
            for (wrong, minimum) in [
                ([-sign * 2.0 / 64.0, sign / 64.0], 3.5),
                ([sign * 3.0 / 64.0, -sign / 64.0], 0.9),
            ] {
                let mut bad = buffer.clone();
                bad.vectors.fill(wrong);
                let report = review(
                    &a,
                    &b,
                    ["a".repeat(64), "b".repeat(64)],
                    Some((&s, &bad, &"c".repeat(64))),
                    67.0,
                    0.01,
                )
                .unwrap();
                assert!(report.renderer.unwrap().mean_endpoint_px.unwrap() > minimum);
                assert!(report.raw_regression);
            }
            let mut omitted = s.clone();
            omitted.includes_jitter = false;
            let r = review(
                &a,
                &b,
                ["a".repeat(64), "b".repeat(64)],
                Some((&omitted, &buffer, &"c".repeat(64))),
                67.0,
                0.01,
            )
            .unwrap();
            assert!(r.renderer.unwrap().mean_endpoint_px.unwrap() > 1.3);
        }
    }
}
