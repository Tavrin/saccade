//! Constructed answers for budgeted captured sequences (procedural, MIT OR Apache-2.0).
#![allow(clippy::unwrap_used, missing_docs)]
use image::{Rgba, RgbaImage};
use saccade_core::captured_sequence::*;
fn image(value: u8) -> RgbaImage {
    let mut out = RgbaImage::from_pixel(32, 32, Rgba([20, 20, 20, 255]));
    for y in 8..24 {
        for x in 8..24 {
            out.put_pixel(x, y, Rgba([value, value, value, 255]));
        }
    }
    out
}
fn plan(n: usize, reference: bool) -> Plan {
    let captures = (0..n)
        .map(|i| Capture {
            image: format!("frame_{i}.png"),
            timestamp_ms: i as f64 * 100.0,
        })
        .collect::<Vec<_>>();
    Plan {
        schema: PLAN_SCHEMA.into(),
        candidate: captures.clone(),
        reference: reference.then_some(captures),
        masks: None,
        id_buffers: None,
        include_ids: vec![],
        alignment: Alignment::None,
    }
}
fn images(values: &[u8], reference: Option<&[u8]>) -> Images {
    Images {
        candidate: values.iter().copied().map(image).collect(),
        reference: reference.map(|r| r.iter().copied().map(image).collect()),
        scope: vec![vec![true; 1024]; values.len()],
    }
}
fn policy() -> TransitionPolicy {
    TransitionPolicy {
        change_frame: Some(3),
        maximum_pop: 0.1,
        maximum_duration_ms: 400.0,
        maximum_steady_error: 0.25,
        settle_threshold: 0.001,
        consecutive: 2,
        window: 2,
        tile_size: 8,
    }
}
fn animation_policy() -> AnimationPolicy {
    AnimationPolicy {
        maximum_frame_error: 0.01,
        maximum_local_error: 0.1,
        maximum_flicker: 0.05,
        tile_size: 8,
    }
}
#[test]
fn deliberate_object_pop_and_smooth_fade_have_known_switch_and_duration() {
    let pop = images(&[220, 220, 220, 20, 20, 20, 20, 20, 20], None);
    let r = transition(plan(9, false), &pop, policy()).unwrap();
    assert_eq!(r.verdict, "regression");
    assert_eq!(r.pop_frame, Some(3));
    assert_eq!(r.settle_frame, Some(3));
    assert_eq!(r.transition_duration_ms, Some(0.0));
    assert!((r.steady_state_difference - 50.0 / 255.0).abs() < 1e-9);
    assert_eq!(r.steady_state_basis, "pre_post_windows");
    let fade = images(&[220, 220, 220, 170, 120, 70, 20, 20, 20], None);
    let r = transition(plan(9, false), &fade, policy()).unwrap();
    assert_eq!(r.verdict, "pass");
    assert_eq!(r.settle_frame, Some(6));
    assert_eq!(r.transition_duration_ms, Some(300.0));
    let mut irregular = plan(9, false);
    irregular.candidate[6].timestamp_ms = 650.0;
    assert_eq!(
        transition(irregular, &fade, policy())
            .unwrap()
            .transition_duration_ms,
        Some(350.0)
    );
    let mut p = policy();
    p.maximum_duration_ms = 200.0;
    assert_eq!(
        transition(plan(9, false), &fade, p).unwrap().verdict,
        "regression"
    );
}
#[test]
fn irregular_timestamps_and_independent_target_preserve_steady_failures() {
    let captures = images(&[220, 220, 220, 60, 60, 60, 60, 60, 60], Some(&[20; 9]));
    let mut p = plan(9, true);
    p.candidate[5].timestamp_ms = 550.0;
    p.reference = p.reference.map(|_| p.candidate.clone());
    let mut budgets = policy();
    budgets.settle_threshold = 0.2;
    budgets.maximum_pop = 1.0;
    budgets.maximum_steady_error = 0.01;
    let r = transition(p, &captures, budgets).unwrap();
    assert!(r.steady_state_difference > 0.01);
    assert_eq!(r.verdict, "regression");
    let mut budgets = policy();
    budgets.change_frame = None;
    budgets.maximum_steady_error = 0.01;
    let r = transition(plan(9, true), &captures, budgets).unwrap();
    assert_eq!(r.pop_frame, None);
    assert_eq!(r.popping, None);
    assert_eq!(r.transition_duration_ms, None);
    assert_eq!(r.verdict, "regression");
}
#[test]
fn single_frame_deformation_and_correct_animation_have_known_answers() {
    let reference = [60, 70, 80, 90, 100, 110, 120, 130, 140];
    let mut candidate = reference;
    candidate[4] = 240;
    let r = animation(
        plan(9, true),
        &images(&candidate, Some(&reference)),
        animation_policy(),
    )
    .unwrap();
    assert_eq!(r.first_divergence_frame, Some(4));
    assert_eq!(r.first_flicker_frame, Some(4));
    assert_eq!(r.verdict, "regression");
    assert_eq!(r.frames[4].regions[0].rect_px, [8, 8, 8, 8]);
    let r = animation(
        plan(9, true),
        &images(&reference, Some(&reference)),
        animation_policy(),
    )
    .unwrap();
    assert_eq!(r.verdict, "pass");
    assert_eq!(r.temporal_flicker, Some(0.0));
    assert_eq!(r.first_divergence_frame, None);
}
#[test]
fn signed_flicker_does_not_cancel_equal_magnitude_errors() {
    let mut policy = animation_policy();
    policy.maximum_frame_error = 1.0;
    policy.maximum_local_error = 1.0;
    let r = animation(
        plan(5, true),
        &images(&[80, 120, 80, 120, 80], Some(&[100; 5])),
        policy,
    )
    .unwrap();
    assert_eq!(r.first_divergence_frame, None);
    assert_eq!(r.verdict, "regression");
    assert!(r.temporal_flicker.unwrap() > 0.05);
}
#[test]
fn bounded_input_and_scope_errors_are_never_passes() {
    let mut p = plan(5, true);
    p.reference.as_mut().unwrap()[2].timestamp_ms += 1.0;
    assert!(p.validate().is_err());
    p = plan(5, true);
    p.candidate[1].timestamp_ms = 0.0;
    assert!(p.validate().is_err());
    p = plan(5, true);
    p.candidate[0].image = "../escape.png".into();
    assert!(p.validate().is_err());
    let mut data = images(&[100; 5], Some(&[100; 5]));
    data.scope[2].fill(false);
    assert!(animation(plan(5, true), &data, animation_policy()).is_err());
    data.scope[2].fill(true);
    data.reference.as_mut().unwrap().pop();
    assert!(animation(plan(5, true), &data, animation_policy()).is_err());
    let mut policy = policy();
    policy.change_frame = Some(0);
    assert!(transition(plan(5, false), &images(&[100; 5], None), policy).is_err());
}
#[test]
fn low_confidence_alignment_is_explicit_and_raw_defects_survive() {
    let mut p = plan(5, true);
    p.alignment = Alignment::Translation;
    let flat = |v| RgbaImage::from_pixel(32, 32, Rgba([v, v, v, 255]));
    let data = Images {
        candidate: vec![flat(110); 5],
        reference: Some(vec![flat(100); 5]),
        scope: vec![vec![true; 1024]; 5],
    };
    let r = animation(p, &data, animation_policy()).unwrap();
    assert_eq!(r.verdict, "regression");
    assert_eq!(r.temporal_flicker, None);
    assert_eq!(r.frames[0].aligned_error, None);
}
#[test]
fn phase_motion_alignment_keeps_raw_error_and_reports_geometry_support() {
    let a = RgbaImage::from_fn(96, 96, |x, y| {
        let v = ((x * 17 + y * 31 + x * y * 7) % 170 + 40) as u8;
        Rgba([v, v, v, 255])
    });
    let b = RgbaImage::from_fn(96, 96, |x, y| *a.get_pixel(x.saturating_sub(2), y));
    let mut p = plan(4, true);
    p.alignment = Alignment::Translation;
    let mut s = vec![false; 96 * 96];
    for y in 8..88 {
        for x in 8..88 {
            s[y * 96 + x] = true;
        }
    }
    let data = Images {
        candidate: vec![b; 4],
        reference: Some(vec![a; 4]),
        scope: vec![s; 4],
    };
    let r = animation(p, &data, animation_policy()).unwrap();
    assert!(r.frames[0].raw_error > 0.01);
    assert!(r.frames[0].aligned_error.unwrap() < 0.005);
    assert!(r.frames[0].translation.unwrap()[0] > 1.5);
    assert_eq!(r.verdict, "regression");
}

#[test]
fn dense_correspondence_is_feature_gated_and_cannot_clear_raw_budgets() {
    let mut p = plan(5, true);
    p.alignment = Alignment::Dense;
    let data = images(&[200; 5], Some(&[100; 5]));
    #[cfg(feature = "dense-motion")]
    {
        let r = animation(p, &data, animation_policy()).unwrap();
        assert_eq!(r.verdict, "regression");
        assert_eq!(r.first_divergence_frame, Some(0));
        assert!(r.frames[0].aligned_pixels < r.frames[0].pixels);
    }
    #[cfg(not(feature = "dense-motion"))]
    assert!(matches!(
        animation(p, &data, animation_policy()),
        Err(saccade_core::Error::FeatureUnavailable {
            feature: "dense-motion"
        })
    ));
}
