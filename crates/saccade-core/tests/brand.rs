//! Constructed brand workflow and published numerical references.
#![allow(clippy::expect_used)]
use saccade_core::{brand::*, color, evidence::canonical::Digest};
use std::{collections::BTreeMap, path::Path};
fn rgb(v: [f64; 3], profile: &str) -> Colour {
    Colour {
        rgb: v,
        profile: profile.into(),
    }
}
fn source() -> Evidence {
    Evidence {
        schema: "saccade-brand-source.v1".into(),
        source_artifact: "dom.json".into(),
        source_sha256: Digest::of_bytes(b"synthetic"),
        rendering_intent: "relative_colorimetric".into(),
        chromatic_adaptation: "bradford_icc_pcs".into(),
        samples: vec![],
        text: vec![],
    }
}
#[test]
fn sharma_all_34_published_pairs() {
    for line in include_str!("fixtures/wave3/sharma.txt")
        .lines()
        .filter(|line| !line.trim().is_empty())
    {
        let v: Vec<f64> = line
            .split_whitespace()
            .map(|v| v.parse().expect("number"))
            .collect();
        let a = [v[0], v[1], v[2]];
        let b = [v[3], v[4], v[5]];
        assert!(
            (color::delta_e(a, b) - v[6]).abs() < 0.00005,
            "{line}: {}",
            color::delta_e(a, b)
        );
        assert!((color::delta_e(b, a) - v[6]).abs() < 0.00005);
    }
}
#[test]
fn managed_srgb_p3_and_deliberate_drift() {
    let policy = Policy {
        swatches: vec![Swatch {
            name: "logo".into(),
            colour: rgb([1., 0., 0.], "srgb"),
            delta_e2000: Some(0.02),
            delta_e_itp: None,
        }],
        ..Default::default()
    };
    let mut e = source();
    e.samples.push(Sample {
        name: "logo".into(),
        colour: rgb([0.9174875573, 0.2002868077, 0.1385605912], "display_p3"),
        source: "pixel:logo".into(),
    });
    let report = review(&policy, &e, Path::new(".")).expect("review");
    assert_eq!(report.findings[0].status, "pass");
    assert_eq!(report.profile_identities.len(), 2);
    e.samples[0].colour = rgb([1., 0., 0.], "display_p3");
    assert_eq!(
        review(&policy, &e, Path::new("."))
            .expect("review")
            .findings[0]
            .status,
        "fail"
    );
}
#[test]
fn external_icc_pin_and_interpretation_are_checked() {
    let temp = tempfile::tempdir().expect("dir");
    let bytes = moxcms::ColorProfile::new_srgb().encode().expect("profile");
    std::fs::write(temp.path().join("rgb.icc"), &bytes).expect("write");
    let mut policy = Policy {
        profiles: BTreeMap::from([(
            "producer".into(),
            Profile {
                path: "rgb.icc".into(),
                sha256: Digest::of_bytes(&bytes),
            },
        )]),
        swatches: vec![Swatch {
            name: "logo".into(),
            colour: rgb([0.5; 3], "srgb"),
            delta_e2000: Some(0.01),
            delta_e_itp: None,
        }],
        ..Default::default()
    };
    let mut e = source();
    e.samples.push(Sample {
        name: "logo".into(),
        colour: rgb([0.5; 3], "producer"),
        source: "logo".into(),
    });
    assert_eq!(
        review(&policy, &e, temp.path()).expect("review").findings[0].status,
        "pass"
    );
    policy.profiles.get_mut("producer").expect("profile").sha256 = Digest::of_bytes(b"wrong");
    assert!(review(&policy, &e, temp.path()).is_err());
    e.rendering_intent = "absolute_colorimetric".into();
    assert!(review(&Policy::default(), &e, temp.path()).is_err());
}
#[test]
fn itp_reference_chroma_scaling_and_pq_neutrals() {
    assert!((delta_e_itp([0.; 3], [0., 0.1, 0.]) - 36.).abs() < 1e-12);
    assert!((delta_e_itp([0.; 3], [0.1, 0., 0.]) - 72.).abs() < 1e-12);
    for p in [0., 0.1, 0.5, 1.] {
        let c = pq_ictcp([p; 3]).expect("PQ");
        assert!((c[0] - p).abs() < 1e-6);
        assert!(c[1].abs() < 1e-12 && c[2].abs() < 1e-12);
    }
    let mut e = source();
    e.samples.push(Sample {
        name: "hdr".into(),
        colour: rgb([0.501; 3], "bt2020_pq"),
        source: "HDR pixel".into(),
    });
    let p = Policy {
        swatches: vec![Swatch {
            name: "hdr".into(),
            colour: rgb([0.5; 3], "bt2020_pq"),
            delta_e_itp: Some(0.7),
            delta_e2000: None,
        }],
        ..Default::default()
    };
    let r = review(&p, &e, Path::new(".")).expect("review");
    assert_eq!(r.findings[0].status, "fail");
    assert!((r.findings[0].evidence["measured"].as_f64().expect("number") - 0.72).abs() < 1e-8);
}
#[test]
fn machado_severity_zero_midpoint_and_endpoint() {
    let c = [0.8, 0.2, 0.1];
    for kind in 0..3 {
        assert_eq!(color::simulate_severity(c, kind, 0.).expect("zero"), c);
        assert_eq!(
            color::simulate_severity(c, kind, 1.).expect("end"),
            color::simulate(c, kind)
        );
        assert_ne!(color::simulate_severity(c, kind, 0.5).expect("mid"), c);
    }
    let c = color::simulate_severity([1., 0., 0.], 1, 0.5).expect("deutan");
    assert_eq!(c, [0.547494, 0.181692, 0.]);
    assert!(color::simulate_severity(c, 1, 1.1).is_err());
}
fn text() -> Text {
    Text {
        id: "price".into(),
        source: "dom:#price".into(),
        foreground: [0., 0., 0., 1.],
        backgrounds: vec![[1.; 3]],
        font_size_px: 16.,
        font_weight: 400,
        font: Some("Test".into()),
        expected_font: None,
        ink_box: None,
        clip_box: None,
        missing_glyphs: None,
    }
}
#[test]
fn wcag_declared_size_alpha_gradient_and_unrounded_boundary() {
    let mut e = source();
    let mut t = text();
    let boundary = 1.055_f64 * ((1.05 / 4.5 - 0.05_f64).powf(1. / 2.4)) - 0.055;
    t.foreground = [
        boundary + 0.000001,
        boundary + 0.000001,
        boundary + 0.000001,
        1.,
    ];
    e.text.push(t);
    let r = review(&Policy::default(), &e, Path::new(".")).expect("review");
    assert_eq!(r.findings[0].status, "fail");
    e.text[0].font_size_px = 24.;
    assert_eq!(
        review(&Policy::default(), &e, Path::new("."))
            .expect("review")
            .findings[0]
            .status,
        "pass"
    );
    e.text[0].foreground = [0., 0., 0., 0.2];
    assert_eq!(
        review(&Policy::default(), &e, Path::new("."))
            .expect("review")
            .findings[0]
            .status,
        "fail"
    );
    e.text[0].foreground = [0., 0., 0., 1.];
    e.text[0].backgrounds.push([0.1; 3]);
    assert_eq!(
        review(&Policy::default(), &e, Path::new("."))
            .expect("review")
            .findings[0]
            .status,
        "fail"
    );
}
#[test]
fn typography_clipping_and_missing_facts_are_distinct() {
    let mut e = source();
    let mut t = text();
    t.ink_box = Some([0., 0., 30., 20.]);
    t.clip_box = Some([0., 0., 30., 18.]);
    t.expected_font = Some("Required".into());
    t.missing_glyphs = Some(1);
    e.text.push(t);
    let r = review(&Policy::default(), &e, Path::new(".")).expect("review");
    assert_eq!(r.findings[1].rule, "source_ink_clipping");
    assert_eq!(r.findings[1].status, "fail");
    assert_eq!(r.findings[2].status, "fail");
    assert_eq!(r.findings[3].rule, "apca_wcag_3_draft");
    assert_eq!(r.findings[3].status, "unavailable");
}
