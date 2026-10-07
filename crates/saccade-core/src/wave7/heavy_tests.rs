//! Opt-in model tests; fixture roots and cached reviewed models must be explicit.
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

fn model_cache() -> PathBuf {
    std::env::var_os("WAVE7_MODEL_CACHE")
        .map(PathBuf::from)
        .expect("set WAVE7_MODEL_CACHE to the reviewed model fixture cache")
}
fn pinned_registry() -> Registry {
    std::env::var_os("WAVE7_MODEL_REGISTRY")
        .map(PathBuf::from)
        .map(|p| Registry::load(&p).unwrap())
        .unwrap_or_else(|| Registry::pinned_wave7().unwrap())
}
fn generated(name: &str) -> VisionImage {
    VisionImage::load(&model_cache().join("fixtures").join(name)).unwrap()
}
#[test]
#[ignore = "heavy: models"]
fn pinned_detection_finds_generated_bottle() {
    use super::{
        native::TextDetector,
        vision::{self, Rect},
    };
    let image = generated("bottle.png");
    let registry = pinned_registry();
    let expected = Rect {
        x: 89.,
        y: 23.,
        width: 78.,
        height: 206.,
    };
    for id in ["grounding-dino-tiny", "owlv2-base"] {
        let mut detector = TextDetector::load(
            registry.model(id).unwrap(),
            &model_cache(),
            &library(),
            false,
        )
        .unwrap();
        let r = vision::locate(&image, "bottle", &mut detector, None).unwrap();
        assert_eq!(r.detector.model_id, id);
        assert_eq!(r.detector.runtime, "onnx-cpu");
        assert!(
            r.detections
                .iter()
                .any(|d| d.bbox.intersection(expected) / (expected.width * expected.height) > 0.7),
            "{id}: generated object must be localized"
        );
    }
}
#[test]
#[ignore = "heavy: models"]
fn pinned_efficientsam_segments_generated_bottle() {
    use super::{
        native::EfficientSam,
        vision::{Rect, Segmenter},
    };
    let image = generated("bottle.png");
    let registry = pinned_registry();
    let mut segmenter = EfficientSam::load(
        registry.model("efficientsam-ti").unwrap(),
        &model_cache(),
        &library(),
        false,
    )
    .unwrap();
    let bbox = Rect {
        x: 88.,
        y: 22.,
        width: 80.,
        height: 208.,
    };
    let (masks, p) = segmenter.segment(&image, &[bbox]).unwrap();
    assert_eq!(p.runtime, "onnx-cpu");
    assert_eq!(masks.len(), 1);
    masks[0].validate(image.size()).unwrap();
    let area: u32 = masks[0].runs.iter().map(|r| r[1]).sum();
    assert!(
        (7000..18000).contains(&area),
        "bottle foreground area: {area}"
    );
    let inside = |x: u32, y: u32| {
        masks[0]
            .runs
            .iter()
            .any(|r| r[0] <= y * 256 + x && y * 256 + x < r[0] + r[1])
    };
    assert!(inside(128, 100));
    assert!(!inside(5, 5));
}
#[test]
#[ignore = "heavy: models"]
fn pinned_faces_detect_generated_portrait_and_reject_blank() {
    use super::vision::Rect;
    let image = generated("face.png");
    let blank = generated("blank.png");
    let registry = pinned_registry();
    let expected = Rect {
        x: 63.,
        y: 26.,
        width: 129.,
        height: 186.,
    };
    for id in ["yunet-2026may", "ultraface-rfb"] {
        let mut runtime = OnnxModel::load(
            registry.model(id).unwrap(),
            &model_cache(),
            &library(),
            false,
        )
        .unwrap();
        let actual = faces::detect(&image, &mut runtime).unwrap();
        assert_eq!(actual.provenance.runtime, "onnx-cpu");
        assert!(
            actual
                .faces
                .iter()
                .any(|f| f.bbox.intersection(expected) / (expected.width * expected.height) > 0.5),
            "{id}: generated portrait must be detected"
        );
        assert!(
            faces::detect(&blank, &mut runtime)
                .unwrap()
                .faces
                .is_empty()
        );
        let crops = faces::crop_check(
            &image,
            &actual,
            &[faces::CropSpec::Ratio {
                width: 1.,
                height: 1.,
            }],
            None,
        )
        .unwrap();
        assert!(
            crops.crops[0]
                .faces
                .iter()
                .all(|f| *f == faces::FaceCropStatus::Included)
        );
    }
}
#[test]
#[ignore = "heavy: models"]
fn pinned_pair_metrics_identity_and_distortion() {
    // Runnable after reviewed complete exports replace the explicit missing-pin disposition.
    let original = generated("bottle.png");
    let registry = pinned_registry();
    let temporary = tempfile::tempdir().unwrap();
    let distorted = |mix: f32| {
        let pixels = image::RgbImage::from_fn(256, 256, |x, y| {
            let p = original.pixels.get_pixel(x, y);
            let noise = if (x * 17 + y * 29) % 7 < 3 { 0. } else { 255. };
            image::Rgb(p.0.map(|v| ((1. - mix) * f32::from(v) + mix * noise) as u8))
        });
        let path = temporary.path().join(format!("distortion-{mix}.png"));
        pixels.save(&path).unwrap();
        VisionImage::load(&path).unwrap()
    };
    for metric in [LearnedMetric::LpipsAlexV01, LearnedMetric::Dists] {
        let m = registry
            .model(metric.model_id())
            .expect("DEFERRED: complete export pin absent; never substitute a fake metric");
        let mut runtime = OnnxModel::load(m, &model_cache(), &library(), false).unwrap();
        let score = |runtime: &mut OnnxModel, image: &VisionImage| {
            quality::measure(metric, image, Some(&original), runtime)
                .unwrap()
                .named_metrics[0]
                .value
        };
        let identity = score(&mut runtime, &original);
        let small = score(&mut runtime, &distorted(0.05));
        let large = score(&mut runtime, &distorted(0.35));
        assert!(identity.abs() < 1e-4);
        assert!(small > identity);
        assert!(large > small);
    }
}

#[test]
#[ignore = "heavy: models"]
fn pinned_single_image_cpu_smoke() {
    use super::{
        native::{EfficientSam, TextDetector},
        vision::{Detector, Rect, Segmenter},
    };
    let id = std::env::var("WAVE7_SMOKE_MODEL").expect("one explicit model per bounded CPU smoke");
    let started = std::time::Instant::now();
    let registry = pinned_registry();
    let m = registry.model(&id).unwrap();
    let name = if matches!(id.as_str(), "yunet-2026may" | "ultraface-rfb") {
        "face.png"
    } else {
        "bottle.png"
    };
    let image = generated(name);
    let summary = match id.as_str() {
        "grounding-dino-tiny" | "owlv2-base" => {
            let mut detector = TextDetector::load(m, &model_cache(), &library(), false).unwrap();
            let (detections, p) = detector.detect(&image, "bottle").unwrap();
            let expected = Rect {
                x: 89.,
                y: 23.,
                width: 78.,
                height: 206.,
            };
            assert!(
                detections.iter().any(|d| d.bbox.intersection(expected)
                    / (expected.width * expected.height)
                    > 0.7)
            );
            assert_eq!(p.model_id, id);
            serde_json::json!({"detections":detections,"provenance":p})
        }
        "efficientsam-ti" => {
            let mut segmenter = EfficientSam::load(m, &model_cache(), &library(), false).unwrap();
            let (masks, p) = segmenter
                .segment(
                    &image,
                    &[Rect {
                        x: 88.,
                        y: 22.,
                        width: 80.,
                        height: 208.,
                    }],
                )
                .unwrap();
            masks[0].validate(image.size()).unwrap();
            let area: u32 = masks[0].runs.iter().map(|r| r[1]).sum();
            assert!((7000..18000).contains(&area));
            let contains = |x: u32, y: u32| {
                masks[0]
                    .runs
                    .iter()
                    .any(|r| r[0] <= y * 256 + x && y * 256 + x < r[0] + r[1])
            };
            assert!(contains(128, 100));
            assert!(!contains(5, 5));
            serde_json::json!({"area":area,"provenance":p})
        }
        "yunet-2026may" | "ultraface-rfb" => {
            let mut graph = OnnxModel::load(m, &model_cache(), &library(), false).unwrap();
            let result = faces::detect(&image, &mut graph).unwrap();
            let expected = Rect {
                x: 63.,
                y: 26.,
                width: 129.,
                height: 186.,
            };
            assert!(
                result.faces.iter().any(|f| f.bbox.intersection(expected)
                    / (expected.width * expected.height)
                    > 0.5)
            );
            serde_json::to_value(result).unwrap()
        }
        "trustmark" => {
            let mut decoder =
                super::trustmark::TrustMarkQ::load(m, &model_cache(), &library(), false).unwrap();
            let (logits, p) = decoder.logits(&image).unwrap();
            assert!(logits.iter().all(|v| v.is_finite()));
            serde_json::json!({"logits":logits.as_slice(),"provenance":p,"qualification":"neural-only; no payload accuracy claim"})
        }
        _ => panic!("unsupported smoke model"),
    };
    assert!(started.elapsed().as_secs_f64() < 60.);
    println!(
        "SMOKE {}",
        serde_json::json!({"model":id,"seconds":started.elapsed().as_secs_f64(),"image_sha256":image.sha256,"result":summary})
    );
}
#[test]
#[ignore = "heavy: models"]
fn pinned_dino_rectangle_maps_to_original_image() {
    use super::{
        native::TextDetector,
        vision::{Detector, Rect},
    };
    let mut image = generated("bottle.png");
    let mut pixels = image::RgbImage::from_pixel(384, 256, image::Rgb([255, 255, 255]));
    image::imageops::replace(&mut pixels, &image.pixels, 0, 0);
    image.pixels = pixels;
    image.sha256 = super::models::digest(image.pixels.as_raw());
    let registry = pinned_registry();
    let mut detector = TextDetector::load(
        registry.model("grounding-dino-tiny").unwrap(),
        &model_cache(),
        &library(),
        false,
    )
    .unwrap();
    let (detections, p) = detector.detect(&image, "bottle").unwrap();
    assert_eq!(p.model_id, "grounding-dino-tiny");
    let expected = Rect {
        x: 89.,
        y: 23.,
        width: 78.,
        height: 206.,
    };
    assert!(
        detections.iter().any(|d| {
            let intersection = d.bbox.intersection(expected);
            intersection
                / (d.bbox.width * d.bbox.height + expected.width * expected.height - intersection)
                > 0.8
        }),
        "{detections:?}"
    );
    assert!(
        detections
            .iter()
            .all(|d| d.bbox.validate([384, 256]).is_ok())
    );
    println!("RECTANGLE {}", serde_json::to_string(&detections).unwrap());
}
