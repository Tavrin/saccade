//! Parser and sampled evidence regression checks.
#![allow(clippy::unwrap_used)]
use saccade_core::{
    text_quality::{self as tq, State},
    timed_text as tt,
};

fn cues(text: &str) -> Vec<tt::Cue> {
    tt::parse(text.as_bytes()).unwrap()
}
fn observed(index: u64, time: f64, text: Option<&str>) -> tt::Observation {
    tt::Observation {
        index,
        timestamp_s: time,
        image_sha256: "00".repeat(32),
        source_sha256: None,
        text: text.map(str::to_string),
        ocr_reason: "generated truth".into(),
        legibility: Some(tq::RegionResult {
            region: 0,
            rect_px: [0, 0, 80, 30],
            state: State::Legible,
            contrast: Some(10.),
            x_height_px: Some(15.),
            sharpness: Some(1.),
            stroke_px: Some(2.),
            ocr: tq::unavailable_ocr("independent"),
            reasons: Vec::new(),
        }),
    }
}
#[test]
fn srt_and_webvtt_preserve_unicode_multiline_and_identifiers() {
    let a = cues(
        "\u{feff}WEBVTT\r\n\r\nNOTE ignored\r\ncomment\r\n  \r\nid\r\n00:01.000 --> 00:02.500 align:center\r\nCafé\r\nsecond line\r\n",
    );
    let b = cues("1\n00:00:01,000 --> 00:00:02,500\nCafé\nsecond line\n");
    assert_eq!(a[0].text, b[0].text);
    assert_eq!(a[0].start_s, 1.);
    assert_eq!(a[0].end_s, 2.5);
    assert_eq!(a[0].id.as_deref(), Some("id"));
    let literal = cues("1\n00:00:00,000 --> 00:00:01,000\nA & B > 5 < 10\n");
    assert_eq!(literal[0].text, "A & B > 5 < 10");
    for bad in [
        "",
        "WEBVTT\nX-TIMESTAMP-MAP=LOCAL:00:00.000\n\n00:00.000 --> 00:01.000\nx",
        "1\n00:00:60,000 --> 00:01:01,000\nx",
        "1\n00:00:01,000 --> 00:00:01,000\nx",
        "WEBVTT\n\n00:00.000 --> 00:01.000\n<b>x</b>",
        "WEBVTT\n\nSTYLE\n::cue {}",
        "WEBVTT\n\n00:00.000 --> 00:01.000\nA &amp; B",
        "WEBVTT\n\n00:00.000 --> 00:01.000\n<00:00.500>Word",
        "1\n00:00:00,000 --> 00:00:01,000\nx\n\n2\n00:00:00,000 --> 00:00:02,000\nx\n\n3\n00:00:00,000 --> 00:00:01,000\n",
    ] {
        assert!(tt::parse(bad.as_bytes()).is_err(), "{bad}");
    }
    assert!(tt::parse(&[0xff]).is_err());
}
#[test]
fn aligned_shifted_missing_misspelt_extra_and_pixel_failure_are_distinct() {
    let expected = cues(
        "WEBVTT\n\n00:01.000 --> 00:02.000\nFirst\n\n00:02.000 --> 00:03.000\nShift\n\n00:04.000 --> 00:05.000\nMissing\n\n00:05.000 --> 00:06.000\nSpelling\n",
    );
    let mut observations: Vec<_> = (0..32)
        .map(|i| {
            let t = i as f64 / 4.;
            observed(
                i,
                t,
                Some(if (1.0..2.0).contains(&t) {
                    "First"
                } else if (2.5..3.5).contains(&t) {
                    "Shift"
                } else if (5.0..6.0).contains(&t) {
                    "Speling"
                } else if t == 7. {
                    "Extra"
                } else {
                    ""
                }),
            )
        })
        .collect();
    let (state, results, extra) =
        tt::check(&expected, &observations, tt::Policy::default()).unwrap();
    assert_eq!(state, "failed");
    assert_eq!(results[0].state, "aligned");
    assert_eq!(results[1].onset_offset_s, Some(0.5));
    assert!(results[1].findings.iter().any(|f| f == "late"));
    assert_eq!(results[2].findings, vec!["missing"]);
    assert_eq!(results[3].findings, vec!["text_mismatch"]);
    assert!(extra.iter().any(|e| e.text == "Extra"));
    observations[4].legibility.as_mut().unwrap().state = State::Illegible;
    let (_, results, _) = tt::check(&expected, &observations, tt::Policy::default()).unwrap();
    assert!(results[0].findings.iter().any(|f| f == "illegible"));
}
#[test]
fn sparse_unknown_and_identical_overlapping_cues_abstain_without_missing_or_late() {
    let expected = cues("WEBVTT\n\n00:01.000 --> 00:02.000\nSample\n");
    for observations in [
        vec![observed(0, 1.75, Some("Sample"))],
        vec![observed(0, 1., None), observed(1, 1.5, None)],
        vec![observed(0, 0., Some("")), observed(1, 3., Some(""))],
    ] {
        let (state, results, _) =
            tt::check(&expected, &observations, tt::Policy::default()).unwrap();
        assert_eq!(state, "insufficient_evidence");
        assert!(results[0].findings.is_empty());
    }
    let repeated =
        cues("WEBVTT\n\n00:01.000 --> 00:02.000\nSample\n\n00:01.000 --> 00:02.000\nSample\n");
    let (state, results, extra) = tt::check(
        &repeated,
        &[
            observed(0, 1., Some("Sample")),
            observed(1, 1.5, Some("Sample")),
        ],
        tt::Policy::default(),
    )
    .unwrap();
    assert_eq!(state, "insufficient_evidence");
    assert!(extra.is_empty());
    assert!(results.iter().all(|r| r.findings.is_empty()));
}
#[test]
fn early_repeated_cues_and_whitespace_normalization_do_not_use_fuzzy_spelling() {
    let expected = cues(
        "WEBVTT\n\n00:01.000 --> 00:02.000\nSame text\n\n00:04.000 --> 00:05.000\nSame text\n",
    );
    let observations: Vec<_> = (0..24)
        .map(|i| {
            let t = i as f64 / 4.;
            observed(
                i,
                t,
                Some(if (0.5..2.0).contains(&t) || (4.0..5.0).contains(&t) {
                    " Same\n text "
                } else {
                    ""
                }),
            )
        })
        .collect();
    let (_, results, _) = tt::check(&expected, &observations, tt::Policy::default()).unwrap();
    assert!(results[0].findings.iter().any(|f| f == "early"));
    assert_eq!(results[1].state, "aligned");
    assert_eq!(results[1].first_seen_s, Some(4.));
    let invalid = tt::Policy {
        maximum_gap_s: f64::NAN,
        ..tt::Policy::default()
    };
    assert!(tt::check(&expected, &observations, invalid).is_err());
    let invalid = tt::Policy {
        legibility: tq::Policy {
            minimum_contrast: 0.,
            ..tq::Policy::default()
        },
        ..tt::Policy::default()
    };
    assert!(tt::check(&expected, &observations, invalid).is_err());
}

#[test]
fn timing_and_gap_boundaries_ignore_roundoff_but_reject_actual_excess() {
    let expected = cues("1\n00:00:00,100 --> 00:00:01,100\nSample\n");
    let mut samples = vec![
        observed(0, 0.4, Some("Sample")),
        observed(1, 0.7, Some("Sample")),
        observed(2, 1.0, Some("Sample")),
    ];
    let policy = tt::Policy {
        maximum_gap_s: 0.3,
        ..tt::Policy::default()
    };
    assert_eq!(tt::check(&expected, &samples, policy).unwrap().0, "aligned");
    let adjacent =
        cues("WEBVTT\n\n00:00.000 --> 00:01.000\nSample\n\n00:01.000 --> 00:02.000\nSample\n");
    let adjacent_samples: Vec<_> = (0..4)
        .map(|i| observed(i, i as f64 * 0.5, Some("Sample")))
        .collect();
    let (state, results, _) =
        tt::check(&adjacent, &adjacent_samples, tt::Policy::default()).unwrap();
    assert_eq!(state, "aligned");
    assert_eq!(results[1].first_seen_s, Some(1.0));
    let equidistant =
        cues("WEBVTT\n\n00:00.000 --> 00:00.200\nSample\n\n00:01.000 --> 00:01.200\nSample\n");
    let (state, results, extra) = tt::check(
        &equidistant,
        &[observed(0, 0.6, Some("Sample"))],
        tt::Policy::default(),
    )
    .unwrap();
    assert_eq!(state, "insufficient_evidence");
    assert!(results.iter().all(|r| r.matching_frames.is_empty()));
    assert!(extra.is_empty());
    let horizon = cues("WEBVTT\n\n00:02.100 --> 00:03.100\nSample\n");
    let (_, results, extra) = tt::check(
        &horizon,
        &[observed(0, 0.1, Some("Sample"))],
        tt::Policy::default(),
    )
    .unwrap();
    assert!(extra.is_empty());
    assert!(results[0].findings.iter().any(|f| f == "early"));
    let mut hours = expected.clone();
    hours[0].start_s += 36_000.0;
    hours[0].end_s += 36_000.0;
    let mut hour_samples = samples.clone();
    for sample in &mut hour_samples {
        sample.timestamp_s += 36_000.0;
    }
    assert_eq!(
        tt::check(&hours, &hour_samples, policy).unwrap().0,
        "aligned"
    );
    let mut coarse = expected.clone();
    coarse[0].start_s += 1e15;
    coarse[0].end_s += 1e15;
    let coarse_samples: Vec<_> = (0..4)
        .map(|i| observed(i, coarse[0].start_s + i as f64 * 0.25, Some("Sample")))
        .collect();
    let (state, results, _) = tt::check(&coarse, &coarse_samples, policy).unwrap();
    assert_eq!(state, "insufficient_evidence");
    assert!(
        results[0]
            .reasons
            .iter()
            .any(|r| r.contains("clock precision"))
    );
    samples[0].timestamp_s = 0.401;
    let (_, results, _) = tt::check(&expected, &samples, tt::Policy::default()).unwrap();
    assert!(results[0].findings.iter().any(|f| f == "late"));
    samples[0].timestamp_s = 0.4;
    samples[1].timestamp_s = 0.701;
    let (_, results, _) = tt::check(&expected, &samples, policy).unwrap();
    assert!(!results[0].window_covered);
    samples[0].timestamp_s = expected[0].start_s + f64::EPSILON;
    let strict = tt::Policy {
        timing_tolerance_s: 0.0,
        ..tt::Policy::default()
    };
    assert!(
        tt::check(&expected, &samples, strict).unwrap().1[0]
            .findings
            .iter()
            .any(|f| f == "late")
    );
}
