//! Coordinator-only tests; fixture roots and cached reviewed models must be explicit.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use super::{
    faces::{self, FaceReport},
    models::{Registry, read_bounded},
    quality::{self, LearnedMetric, QualityReport},
    runtime::OnnxModel,
    vision::VisionImage,
};
use std::path::PathBuf;
fn root() -> PathBuf {
    PathBuf::from(
        std::env::var_os("WAVE7_HEAVY_FIXTURES")
            .expect("set WAVE7_HEAVY_FIXTURES to the reviewed, frozen model parity bundle"),
    )
}
fn registry() -> Registry {
    Registry::load(&root().join("models.json")).expect("reviewed model registry")
}
fn library() -> PathBuf {
    PathBuf::from(
        std::env::var_os("WAVE7_RUNTIME_LIBRARY").expect("set explicit ONNX Runtime 1.22 library"),
    )
}
fn json<T: serde::de::DeserializeOwned>(name: &str) -> T {
    serde_json::from_slice(&read_bounded(&root().join(name), 8 * 1024 * 1024).unwrap()).unwrap()
}
#[test]
#[ignore = "heavy: models"]
fn real_face_exports_match_frozen_source_receipts() {
    let r = registry();
    let image = VisionImage::load(&root().join("faces.png")).unwrap();
    for id in ["yunet-2026may", "ultraface-rfb"] {
        let m = r.model(id).unwrap();
        assert!(m.parity_sha256.is_some(), "source parity artifact required");
        let expected: FaceReport = json(&format!("{id}.faces.json"));
        expected.validate(&image).unwrap();
        assert_eq!(expected.provenance.model_id, id);
        assert_eq!(expected.provenance.version, m.version);
        assert!(!expected.faces.is_empty(), "positive fixture required");
        let mut runtime = OnnxModel::load(m, &root().join("cache"), &library(), false).unwrap();
        let actual = faces::detect(&image, &mut runtime).unwrap();
        assert_eq!(actual.faces.len(), expected.faces.len());
        for (a, b) in actual.faces.iter().zip(expected.faces.iter()) {
            assert!((a.score - b.score).abs() <= 0.001);
            for (x, y) in [a.bbox.x, a.bbox.y, a.bbox.width, a.bbox.height]
                .into_iter()
                .zip([b.bbox.x, b.bbox.y, b.bbox.width, b.bbox.height])
            {
                assert!((x - y).abs() <= 1.);
            }
        }
        let blank = VisionImage::load(&root().join("face-negative.png")).unwrap();
        assert!(
            faces::detect(&blank, &mut runtime)
                .unwrap()
                .faces
                .is_empty()
        );
    }
}
#[test]
#[ignore = "heavy: models"]
fn real_quality_exports_match_frozen_source_scalars() {
    let r = registry();
    let image = VisionImage::load(&root().join("quality.png")).unwrap();
    let reference = VisionImage::load(&root().join("reference.png")).unwrap();
    for metric in [
        LearnedMetric::LpipsAlexV01,
        LearnedMetric::Dists,
        LearnedMetric::MusiqTechnical,
    ] {
        let m = r.model(metric.model_id()).unwrap();
        assert!(m.parity_sha256.is_some(), "source parity artifact required");
        let expected: QualityReport = json(&format!("{}.quality.json", metric.model_id()));
        let reference = metric.needs_reference().then_some(&reference);
        expected.validate(metric, &image, reference).unwrap();
        assert_eq!(expected.named_metrics[0].provenance.version, m.version);
        let mut runtime = OnnxModel::load(m, &root().join("cache"), &library(), false).unwrap();
        let actual = quality::measure(metric, &image, reference, &mut runtime).unwrap();
        assert!((actual.named_metrics[0].value - expected.named_metrics[0].value).abs() <= 0.001);
        if metric.needs_reference() {
            let identity = quality::measure(metric, &image, Some(&image), &mut runtime).unwrap();
            assert!(identity.named_metrics[0].value <= 0.001);
        }
    }
}
#[test]
#[ignore = "heavy: models"]
fn selected_detection_segmentation_source_receipts_are_compatible() {
    // Contract compatibility only: selected native detector/SAM adapters are deferred.
    let image = VisionImage::load(&root().join("locate.png")).unwrap();
    let r = registry();
    for (detector, segmenter) in [
        ("grounding-dino-tiny", "sam-2.1-tiny"),
        ("owlv2-base", "efficientsam-ti"),
    ] {
        let receipt: super::vision::LocateReport = json(&format!("{detector}.locate.json"));
        receipt.validate(&image, "small object", true).unwrap();
        assert_eq!(receipt.detector.model_id, detector);
        assert_eq!(receipt.segmenter.as_ref().unwrap().model_id, segmenter);
        assert!(!receipt.detections.is_empty());
        for id in [detector, segmenter] {
            assert!(r.model(id).unwrap().parity_sha256.is_some());
        }
    }
}
#[test]
#[ignore = "heavy: models"]
fn selected_trustmark_source_receipt_is_compatible() {
    // Contract compatibility only: decoder variant/ECC/native adapter remain deferred.
    let image = VisionImage::load(&root().join("watermarked.png")).unwrap();
    let r: super::watermark::WatermarkReport = json("trustmark.watermark.json");
    r.validate(&image).unwrap();
    assert!(r.findings.iter().any(|f| f.scheme.starts_with("trustmark")
        && f.status == "detected"
        && f.payload_hex.is_some()));
    assert!(
        registry()
            .model("trustmark")
            .unwrap()
            .parity_sha256
            .is_some()
    );
}
#[test]
#[ignore = "heavy: models"]
fn legacy_decoder_matches_frozen_embedding_workflow() {
    let image = VisionImage::load(&root().join("legacy-watermarked.png")).unwrap();
    let payload: Vec<u8> = json("legacy-payload.json");
    let finding = super::watermark::decode_dwt(
        &image,
        &super::watermark::DwtConfig {
            expected_payload: payload,
            quantization_step: 36.,
            minimum_agreement: 0.9,
        },
    )
    .unwrap();
    assert_eq!(finding.status, "detected");
}
#[cfg(feature = "local-vlm")]
#[test]
#[ignore = "heavy: network"]
fn local_vlm_endpoint_smoke_with_explicit_model_identity() {
    use super::observation::ObservationProvider;
    let request: super::observation::ObservationRequest = json("local-vlm.request.json");
    let endpoint =
        std::env::var("WAVE7_LOCAL_VLM_ENDPOINT").expect("explicit loopback local VLM endpoint");
    let runtime_revision = std::env::var("WAVE7_LOCAL_VLM_REVISION")
        .expect("explicit runtime/processor/template/quantization revision");
    let mut runtime = super::local_vlm::LocalVlm {
        endpoint,
        runtime_revision,
    };
    let report = runtime.observe(&request).unwrap();
    report.validate(&request).unwrap();
    assert!(!report.statements.is_empty());
    assert!(report.advisory_only);
}
