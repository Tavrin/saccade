//! Constructed exact text, semantic order, coverage and OCR uncertainty tests.
#![allow(clippy::expect_used)]
use saccade_core::{localized, ui_review::*};
use std::collections::BTreeMap;
fn source(hash: String) -> Source {
    Source {
        schema: "saccade-ui-source.v1".into(),
        capture_sha256: hash,
        dimensions: [16, 16],
        kind: "dom".into(),
        producer: serde_json::json!({"fixture":"synthetic DOM"}),
        nodes: vec![],
        complete: true,
    }
}
fn node(id: &str, text: &str, index: u32) -> Node {
    Node {
        id: id.into(),
        text: text.into(),
        role: "text".into(),
        bounds: Some([0., index as f64, 10., 1.]),
        reading_order: Some(index),
        keyboard_order: Some(index),
        disclosure: false,
        ocr_confidence: None,
    }
}
fn measurement() -> localized::Measurement {
    let image = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        16,
        16,
        image::Rgb([30, 30, 30]),
    ));
    let hash = localized::digest(b"synthetic reference");
    let region = localized::boxes(
        hash.clone(),
        [16, 16],
        &[[2, 2, 4, 4]],
        BTreeMap::from([("method".into(), "box".into())]),
    )
    .expect("region");
    localized::measure(
        &image,
        &image,
        &hash,
        localized::digest(b"synthetic candidate"),
        region,
        localized::Policy {
            ppd: 67.,
            exact_outside: true,
            maximum_outside_flip: 0.01,
        },
    )
    .expect("measurement")
}
#[test]
fn exact_identifiers_prices_punctuation_accents_and_disappeared_disclosure() {
    let m = measurement();
    let mut b = source(m.region.reference_sha256.clone());
    let mut a = source(m.candidate_sha256.clone());
    b.nodes = vec![
        node("price", "€19.99", 0),
        node("identifier", "AC-001/I", 1),
        node("title", "Échéance: 2026!", 2),
        node("legal", "Includes tax.", 3),
    ];
    b.nodes[3].disclosure = true;
    a.nodes = vec![
        node("price", "€79.99", 0),
        node("identifier", "AC-001/l", 1),
        node("title", "Echeance: 2026?", 2),
    ];
    let r = compare(&b, &a, m).expect("packet");
    assert_eq!(
        r.findings
            .iter()
            .filter(|f| f.rule == "text_changed")
            .count(),
        3
    );
    assert!(r.findings.iter().any(|f| f.rule == "missing_disclosure"));
    assert!(r.findings.iter().all(|f| f.assurance == "source_fact"));
}
#[test]
fn semantic_orders_change_with_identical_pixels_and_reflow_is_separate() {
    let m = measurement();
    assert_eq!(m.inside.changed_pixels, 0);
    let mut b = source(m.region.reference_sha256.clone());
    let mut a = source(m.candidate_sha256.clone());
    b.nodes = vec![node("left", "Left", 0), node("right", "Right", 1)];
    a.nodes = b.nodes.clone();
    a.nodes[0].reading_order = Some(1);
    a.nodes[1].reading_order = Some(0);
    a.nodes[0].keyboard_order = Some(1);
    a.nodes[1].keyboard_order = Some(0);
    a.nodes[0].bounds = Some([2., 2., 5., 2.]);
    let r = compare(&b, &a, m).expect("packet");
    assert!(r.findings.iter().any(|f| f.rule == "reading_order_changed"));
    assert!(
        r.findings
            .iter()
            .any(|f| f.rule == "keyboard_order_changed")
    );
    assert!(r.findings.iter().any(|f| f.rule == "layout_changed"));
    assert!(!r.findings.iter().any(|f| f.rule == "text_changed"));
}
#[test]
fn partial_sources_and_duplicate_or_stale_metadata_cannot_prove_removal() {
    let m = measurement();
    let mut b = source(m.region.reference_sha256.clone());
    let mut a = source(m.candidate_sha256.clone());
    b.nodes = vec![node("legal", "Terms", 0)];
    b.nodes[0].disclosure = true;
    a.complete = false;
    assert_eq!(
        compare(&b, &a, m.clone()).expect("partial").findings[0].rule,
        "node_not_observed"
    );
    a.nodes = vec![node("same", "A", 0), node("same", "B", 1)];
    assert!(compare(&b, &a, m.clone()).is_err());
    a.nodes.clear();
    a.capture_sha256 = localized::digest(b"stale");
    assert!(compare(&b, &a, m).is_err());
}
#[test]
fn tesseract_price_and_identifier_observations_remain_uncertain_at_100_confidence() {
    let header = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n";
    let m = measurement();
    let b=tesseract_tsv(&format!("{header}5\t1\t1\t1\t1\t1\t0\t0\t8\t8\t100\t€19.99\n5\t1\t1\t1\t1\t2\t8\t0\t8\t8\t15\tAC-001/I\n"),m.region.reference_sha256.clone(),[16,16],serde_json::json!({"models":"pinned synthetic observations"})).expect("TSV");
    let a=tesseract_tsv(&format!("{header}5\t1\t1\t1\t1\t1\t0\t0\t8\t8\t100\t€79.99\n5\t1\t1\t1\t1\t2\t8\t0\t8\t8\t15\tAC-001/l\n"),m.candidate_sha256.clone(),[16,16],serde_json::json!({})).expect("TSV");
    let r = compare(&b, &a, m).expect("packet");
    assert_eq!(r.findings.len(), 2);
    assert!(r.findings.iter().all(|f| f.assurance == "uncertain_ocr"));
    assert!(r.sources.iter().all(|s| !s.complete));
}
