//! Critical gates retain failures and unknowns independently of image averages.
#![allow(clippy::unwrap_used)]
use image::{Rgba, RgbaImage};
use saccade_core::{
    critical_text as ct, text_quality as tq,
    ui_review::{Node, Source},
};

fn image() -> RgbaImage {
    let mut image = RgbaImage::from_pixel(80, 40, Rgba([255, 255, 255, 255]));
    for y in 10..25 {
        for x in 10..14 {
            image.put_pixel(x, y, Rgba([0, 0, 0, 255]));
        }
    }
    image
}
fn policy() -> ct::Policy {
    ct::Policy {
        schema: ct::POLICY_SCHEMA.into(),
        dimensions: [80, 40],
        minimum_ocr_confidence: 80.,
        regions: vec![ct::Region {
            id: "amount".into(),
            rect_px: [0, 0, 80, 40],
            accepted_text: vec!["−1.5 €".into(), "-1.5\u{a0}€".into()],
            legibility: tq::Policy::default(),
        }],
    }
}
fn source(text: &str) -> Source {
    Source {
        schema: "saccade-ui-source.v1".into(),
        capture_sha256: "a".repeat(64),
        dimensions: [80, 40],
        kind: "dom".into(),
        producer: serde_json::json!({"adapter":"generated"}),
        complete: true,
        nodes: vec![Node {
            id: "amount".into(),
            text: text.into(),
            bounds: Some([8., 8., 30., 20.]),
            role: String::new(),
            reading_order: None,
            keyboard_order: None,
            disclosure: false,
            ocr_confidence: None,
        }],
    }
}
fn run(
    a: Option<Source>,
    b: Option<Source>,
    policy: ct::Policy,
) -> saccade_core::Result<ct::Report> {
    ct::evaluate(
        [&image(), &image()],
        ["a".repeat(64), "a".repeat(64)],
        [a, b],
        policy,
        "b".repeat(64),
        [None, None],
    )
}
#[test]
fn exact_symbols_and_only_explicit_typography_alternatives() {
    for text in ["1.5 €", "−15 €", "−1.5€", "−1.5 $", "prefix −1.5 €"] {
        let report = run(Some(source("−1.5 €")), Some(source(text)), policy()).unwrap();
        assert_eq!(report.state, ct::State::Fail, "{text}");
        assert!(
            report.regions[0]
                .reasons
                .iter()
                .any(|r| r == "critical_string_mismatch")
        );
        assert!(report.regions[0].rates.as_ref().unwrap().character_edits > 0);
    }
    assert_eq!(
        run(
            Some(source("−1.5 €")),
            Some(source("-1.5\u{a0}€")),
            policy()
        )
        .unwrap()
        .state,
        ct::State::Pass
    );
}
#[test]
fn invalid_baseline_and_missing_or_incomplete_evidence_never_pass() {
    assert_eq!(
        run(Some(source("wrong")), Some(source("−1.5 €")), policy())
            .unwrap()
            .state,
        ct::State::InsufficientEvidence
    );
    assert_eq!(
        run(None, None, policy()).unwrap().state,
        ct::State::InsufficientEvidence
    );
    let mut b = source("−1.5 €");
    b.complete = false;
    assert_eq!(
        run(Some(source("−1.5 €")), Some(b), policy())
            .unwrap()
            .state,
        ct::State::InsufficientEvidence
    );
    let mut b = source("−1.5 €");
    b.nodes.clear();
    assert_eq!(
        run(Some(source("−1.5 €")), Some(b.clone()), policy())
            .unwrap()
            .state,
        ct::State::Fail
    );
    b.kind = "paddle_ocr".into();
    b.complete = false;
    assert_eq!(
        run(Some(source("−1.5 €")), Some(b), policy())
            .unwrap()
            .state,
        ct::State::InsufficientEvidence
    );
}
#[test]
fn low_confidence_and_ambiguous_geometry_abstain_without_inventing_spaces() {
    let mut b = source("−1.5 €");
    b.kind = "paddle_ocr".into();
    b.complete = false;
    for confidence in [None, Some(79.)] {
        b.nodes[0].ocr_confidence = confidence;
        assert_eq!(
            run(Some(source("−1.5 €")), Some(b.clone()), policy())
                .unwrap()
                .state,
            ct::State::InsufficientEvidence
        );
    }
    b.nodes[0].ocr_confidence = Some(95.);
    assert_eq!(
        run(Some(source("−1.5 €")), Some(b), policy())
            .unwrap()
            .state,
        ct::State::Pass
    );
    for bounds in [None, Some([-1., 8., 30., 20.]), Some([70., 8., 30., 20.])] {
        let mut b = source("−1.5 €");
        b.nodes[0].bounds = bounds;
        assert_eq!(
            run(Some(source("−1.5 €")), Some(b), policy())
                .unwrap()
                .state,
            ct::State::InsufficientEvidence
        );
    }
    let mut b = source("−1.5");
    let mut n = b.nodes[0].clone();
    n.id = "currency".into();
    n.text = "€".into();
    b.nodes.push(n);
    let report = run(Some(source("−1.5 €")), Some(b), policy()).unwrap();
    assert_eq!(report.state, ct::State::InsufficientEvidence);
    assert!(report.regions[0].observations[1].text.is_none());
}
#[test]
fn stale_sources_and_invalid_policies_are_execution_errors() {
    assert!(!saccade_core::report_links::is_report_schema(
        ct::POLICY_SCHEMA
    ));
    let mut b = source("−1.5 €");
    b.capture_sha256 = "b".repeat(64);
    assert!(run(Some(source("−1.5 €")), Some(b), policy()).is_err());
    for mutation in 0..6 {
        let mut p = policy();
        match mutation {
            0 => p.regions.clear(),
            1 => p.dimensions = [40, 40],
            2 => p.regions[0].rect_px = [u32::MAX, 0, 2, 2],
            3 => p.regions.push(p.regions[0].clone()),
            4 => p.regions[0].accepted_text.clear(),
            _ => p.regions[0].legibility.minimum_contrast = f64::NAN,
        }
        assert!(run(Some(source("−1.5 €")), Some(source("−1.5 €")), p).is_err());
    }
}
#[test]
fn pixel_failure_dominates_unknown_text_and_all_regions_are_required() {
    let a = image();
    let mut b = a.clone();
    for pixel in b.pixels_mut() {
        if pixel[0] == 0 {
            *pixel = Rgba([210, 210, 210, 255]);
        }
    }
    let report = ct::evaluate(
        [&a, &b],
        ["a".repeat(64), "a".repeat(64)],
        [Some(source("−1.5 €")), None],
        policy(),
        "b".repeat(64),
        [None, None],
    )
    .unwrap();
    assert_eq!(report.state, ct::State::Fail);
    assert!(
        report.regions[0].pixels[1]
            .reasons
            .iter()
            .any(|r| r == "low_contrast")
    );
    let mut p = policy();
    let mut region = p.regions[0].clone();
    region.id = "other".into();
    region.rect_px = [40, 0, 40, 40];
    p.regions.push(region);
    let report = run(Some(source("−1.5 €")), Some(source("−1.5 €")), p).unwrap();
    assert_eq!(report.regions[0].state, ct::State::Pass);
    assert_eq!(report.regions[1].state, ct::State::Fail);
    assert_eq!(report.state, ct::State::Fail);
}
