//! Pinned TrustMark Q payload decoder with reference-compatible BCH schemas.
use super::{
    models::{Model, Result, VisionError},
    runtime::OnnxModel,
    vision::{Provenance, VisionImage},
    watermark::{WatermarkDecoder, WatermarkEcc, WatermarkFinding, bits_hex},
};
use std::path::Path;
/// Q decoder with a verified complete graph; only successful BCH is detection.
pub struct TrustMarkQ {
    graph: OnnxModel,
}
impl TrustMarkQ {
    /// Load explicitly cached, pinned Q decoder on CPU.
    pub fn load(model: &Model, cache: &Path, library: &Path, download: bool) -> Result<Self> {
        if download {
            return Err(VisionError::Invalid(
                "TrustMark decoding never downloads; explicitly provision with models pull first"
                    .into(),
            ));
        }
        let i = &model.input;
        if model.id != "trustmark"
            || i.adapter != "trustmark-q-bch-v1"
            || i.resolution != [256, 256]
            || i.color != "RGB"
            || i.scale != 2. / 255.
            || i.mean != [1.; 3]
            || i.std != [1.; 3]
            || i.image_input != "image"
            || i.output != "output"
            || i.reference_input.is_some()
        {
            return Err(VisionError::Invalid("TrustMark Q adapter".into()));
        }
        Ok(Self {
            graph: OnnxModel::load(model, cache, library, false)?,
        })
    }
    /// Run the neural graph and preserve all 100 logits, before BCH validation.
    pub fn logits(&mut self, image: &VisionImage) -> Result<([f32; 100], Provenance)> {
        let pixels = prepare(image);
        let prepared = VisionImage {
            pixels,
            sha256: image.sha256.clone(),
        };
        let output = self.graph.run(&prepared, None)?;
        let tensor = output
            .get("output")
            .ok_or_else(|| VisionError::Invalid("TrustMark output missing".into()))?;
        if tensor.shape != [1, 100] {
            return Err(VisionError::Invalid("TrustMark output shape".into()));
        }
        let values = tensor
            .values
            .as_slice()
            .try_into()
            .map_err(|_| VisionError::Invalid("TrustMark bit count".into()))?;
        Ok((values, self.graph.provenance(image)))
    }
}
fn prepare(image: &VisionImage) -> image::RgbImage {
    let [w, h] = image.size();
    if w as f64 / h as f64 > 2. || (w as f64 / h as f64) < 0.5 {
        let side = w.min(h);
        image::imageops::crop_imm(&image.pixels, (w - side) / 2, (h - side) / 2, side, side)
            .to_image()
    } else {
        image.pixels.clone()
    }
}
impl WatermarkDecoder for TrustMarkQ {
    fn decode(&mut self, image: &VisionImage) -> Result<WatermarkFinding> {
        let (logits, provenance) = self.logits(image)?;
        let mut finding = decode_logits(&logits)?;
        finding.provenance = Some(provenance);
        Ok(finding)
    }
}
fn decode_logits(logits: &[f32; 100]) -> Result<WatermarkFinding> {
    if logits.iter().any(|v| !v.is_finite()) {
        return Err(VisionError::Invalid("TrustMark nonfinite logits".into()));
    }
    let bits = logits.map(|v| u8::from(v > 0.));
    let schema = bits[98] * 2 + bits[99];
    let mut finding = WatermarkFinding {
        scheme: "trustmark-Q".into(), status: "not_detected".into(), payload_hex: None,
        payload_bits: None, schema_value: None, confidence: None, provenance: None,
        ecc: Some(WatermarkEcc {
            variant: super::trustmark_bch::NAMES[usize::from(schema)].into(),
            status: "failed".into(), corrected_bits: None,
            max_correctable_bits: super::trustmark_bch::limit(schema),
        }),
        interpretation: "Q neural logits thresholded at >0; reference data/ECC byte padding, GF(128) polynomial 137. Detection requires a valid or corrected BCH codeword under the transmitted schema; no schema guessing. Confidence is uncalibrated. Payloads do not authenticate a signer or generator; ECC-valid false positives are possible.".into(),
    };
    if let Some((payload, schema, corrections)) = super::trustmark_bch::decode(&bits) {
        finding.status = "detected".into();
        finding.payload_hex = Some(bits_hex(&payload));
        finding.payload_bits = Some(payload);
        finding.schema_value = Some(schema);
        if let Some(ecc) = &mut finding.ecc {
            ecc.status = if corrections == 0 {
                "valid"
            } else {
                "corrected"
            }
            .into();
            ecc.corrected_bits = Some(corrections);
        }
    }
    Ok(finding)
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn q_crop_policy_only_crops_extreme_aspect_ratios() {
        for (w, h, expected) in [
            (400, 200, [400, 200]),
            (401, 200, [200, 200]),
            (100, 201, [100, 100]),
            (100, 200, [100, 200]),
        ] {
            let image = VisionImage {
                pixels: image::RgbImage::new(w, h),
                sha256: super::super::models::digest(b"fixture"),
            };
            let pixels = prepare(&image);
            assert_eq!([pixels.width(), pixels.height()], expected);
        }
    }
    #[test]
    fn ecc_failure_hides_payload_and_nonfinite_logits_are_errors() {
        let failed = decode_logits(&[1.; 100]).unwrap();
        assert_eq!(failed.status, "not_detected");
        assert!(failed.payload_bits.is_none());
        assert!(failed.payload_hex.is_none());
        assert!(failed.schema_value.is_none());
        assert_eq!(failed.ecc.unwrap().status, "failed");
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut logits = [0.; 100];
            logits[0] = value;
            assert!(decode_logits(&logits).is_err());
        }
    }
    #[test]
    fn verified_fields_and_historical_versions_are_validated() {
        let receipt: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/trustmark/provenance.json"
        ))
        .unwrap();
        let packet = receipt["bch_vectors"][1]["packet"].as_str().unwrap();
        let mut logits = [0.; 100];
        for (l, b) in logits.iter_mut().zip(packet.bytes()) {
            *l = if b == b'1' { 1. } else { -1. };
        }
        let finding = decode_logits(&logits).unwrap();
        let input = VisionImage {
            pixels: image::RgbImage::new(16, 16),
            sha256: super::super::models::digest(b"test"),
        };
        let mut r = super::super::watermark::inspect(&input, None, None).unwrap();
        r.findings[0] = finding;
        r.validate(&input).unwrap();
        r.findings[0].schema_value = Some(1);
        assert!(r.validate(&input).is_err());
        r.findings[0].schema_value = Some(0);
        r.findings[0].payload_hex = Some("00".into());
        assert!(r.validate(&input).is_err());
        r.findings[0] = decode_logits(&logits).unwrap();
        r.schema = "saccade-watermark.v1".into();
        assert!(r.validate(&input).is_err());
        r.schema = "saccade-watermark.v4".into();
        r.validate(&input).unwrap();
    }
}
