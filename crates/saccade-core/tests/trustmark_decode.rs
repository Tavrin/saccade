//! Reference-encoder acceptance: explicit local cache/runtime, never network.
#![cfg(feature = "local-models")]
#![allow(clippy::unwrap_used, clippy::expect_used)]
use image::{
    Rgb, RgbImage,
    imageops::{FilterType, resize},
};
use saccade_core::wave7::{
    models::{Registry, digest},
    trustmark::TrustMarkQ,
    vision::VisionImage,
    watermark::{self},
};
use std::path::PathBuf;

fn image(pixels: RgbImage) -> VisionImage {
    let sha256 = digest(pixels.as_raw());
    VisionImage { pixels, sha256 }
}
fn negative(index: u32) -> VisionImage {
    let mut state = 0x9e3779b9u32 ^ index;
    image(RgbImage::from_fn(320, 256, |x, y| {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let v = match index % 8 {
            0 => [index as u8 * 2; 3],
            1 => [x as u8, y as u8, (x + y) as u8],
            2 => [state as u8, (state >> 8) as u8, (state >> 16) as u8],
            3 => [if (x / 16 + y / 16) % 2 == 0 { 40 } else { 210 }; 3],
            4 => [((x.abs_diff(160).pow(2) + y.abs_diff(128).pow(2)) / 32) as u8; 3],
            5 => [
                ((f64::from(x) * 0.1).sin() * 70. + 128.) as u8,
                ((f64::from(y) * 0.1).cos() * 70. + 128.) as u8,
                100,
            ],
            6 => [
                ((x * 3 + y * 2 + index) % 170 + 40) as u8,
                ((x + y * 3) % 160 + 50) as u8,
                ((x * 2 + y) % 150 + 60) as u8,
            ],
            _ => {
                [if x > 80 && x < 240 && y > 64 && y < 192 {
                    220
                } else {
                    60
                }; 3]
            }
        };
        Rgb(v)
    }))
}

#[test]
#[ignore = "heavy: trustmark reference fixtures; supply G25_TRUSTMARK_CACHE and G25_TRUSTMARK_RUNTIME"]
fn reference_payloads_survive_jpeg_resize_and_never_match_negatives() {
    let cache = PathBuf::from(std::env::var_os("G25_TRUSTMARK_CACHE").expect("explicit cache"));
    let library =
        PathBuf::from(std::env::var_os("G25_TRUSTMARK_RUNTIME").expect("explicit runtime"));
    let registry = Registry::pinned_wave7().unwrap();
    let model = registry.model("trustmark").unwrap();
    let mut decoder = TrustMarkQ::load(model, &cache, &library, false).unwrap();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/trustmark");
    let source = std::fs::read(dir.join("provenance.json")).unwrap();
    let receipt: serde_json::Value = serde_json::from_slice(&source).unwrap();
    let cases = receipt["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 8);
    let targets = cases
        .iter()
        .step_by(2)
        .map(|v| v["payload_bits"].as_str().unwrap())
        .collect::<Vec<_>>();
    let mut details = Vec::new();
    let mut exact = 0;
    let mut wrong_payload_target_hits = 0;
    for (index, case) in cases.iter().enumerate() {
        let original = VisionImage::load(&dir.join(case["file"].as_str().unwrap())).unwrap();
        assert_eq!(original.sha256, case["sha256"].as_str().unwrap());
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 90)
            .encode_image(&original.pixels)
            .unwrap();
        let jpeg_image = VisionImage {
            pixels: image::load_from_memory(&jpeg).unwrap().to_rgb8(),
            sha256: digest(&jpeg),
        };
        let small = image(resize(&original.pixels, 288, 230, FilterType::Triangle));
        for (transform, input) in [
            ("original", original),
            ("jpeg-90", jpeg_image),
            ("resize-90-percent", small),
        ] {
            let report = watermark::inspect(&input, Some(&mut decoder), None).unwrap();
            let finding = &report.findings[0];
            assert_eq!(
                finding.status, "detected",
                "{index} {transform}: {finding:?}"
            );
            assert_eq!(
                finding.payload_bits.as_deref(),
                case["payload_bits"].as_str(),
                "{index} {transform}"
            );
            assert_eq!(
                finding.schema_value.map(u64::from),
                case["schema_value"].as_u64()
            );
            exact += 1;
            if index % 2 == 1 && targets.contains(&finding.payload_bits.as_deref().unwrap()) {
                wrong_payload_target_hits += 1;
            }
            details.push(
                serde_json::json!({"fixture":case["file"],"transform":transform,"finding":finding}),
            );
        }
    }
    let (mut false_positives, mut negative_target_hits) = (0, 0);
    for index in 0..128 {
        let input = negative(index);
        let report = watermark::inspect(&input, Some(&mut decoder), None).unwrap();
        let finding = &report.findings[0];
        if finding.status == "detected" {
            false_positives += 1;
        }
        if finding
            .payload_bits
            .as_deref()
            .is_some_and(|p| targets.contains(&p))
        {
            negative_target_hits += 1;
        }
        details.push(serde_json::json!({"negative_index":index,"finding":finding}));
    }
    assert_eq!(wrong_payload_target_hits, 0);
    assert_eq!(negative_target_hits, 0);
    let result = serde_json::json!({
        "decoder_sha256":model.artifacts[0].sha256,"fixture_manifest_sha256":digest(&source),
        "positive_exact":exact,"positive_total":24,"wrong_payload_total":12,
        "wrong_payload_target_hits":wrong_payload_target_hits,"negative_total":128,
        "negative_target_hits":negative_target_hits,"false_positives":false_positives,
        "false_positive_rate":f64::from(false_positives)/128.,
        "negative_generator":"fixed seed 0x9e3779b9 xor index, 320x256, eight families: flat, ramps, noise, checker, radial, sinusoidal, modular texture, rectangle",
        "details":details,
    });
    println!(
        "TrustMark accuracy {exact}/24; wrong-payload target hits {wrong_payload_target_hits}/12; unwatermarked target hits {negative_target_hits}/128; false positives {false_positives}/128"
    );
    if let Some(path) = std::env::var_os("G25_TRUSTMARK_RECEIPT") {
        std::fs::write(path, serde_json::to_vec_pretty(&result).unwrap()).unwrap();
    }
}
