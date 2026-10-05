//! Watermark evidence, kept separate from signed provenance and origin verdicts.
use super::{
    models::{Result, VisionError},
    vision::{Provenance, VisionImage},
};
use serde::{Deserialize, Serialize};
/// Watermark evidence contract identifier.
pub const WATERMARK_SCHEMA: &str = "saccade-watermark.v1";
/// Meaning of missing evidence, required in every report.
pub const ABSENCE_LIMIT: &str = "No detected watermark does not establish human origin, absence of AI generation, or authenticity. Decoders cover only named compatible schemes; cropping, resizing and encoding may destroy markers. Recovered payloads do not authenticate a generator or signer.";
/// Independent outcome for one compatible scheme.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WatermarkFinding {
    /// Decoder scheme/variant/ECC identity, not inferred generator identity.
    pub scheme: String,
    /// detected, not_detected, or unavailable.
    pub status: String,
    /// Recovered payload bytes, hex encoded; absent when not verified/matched.
    pub payload_hex: Option<String>,
    /// Decoder score, not authenticity probability; unknown when unavailable.
    pub confidence: Option<f32>,
    /// Meaning and limitations of this particular decoder's evidence.
    pub interpretation: String,
    /// Exact model/runtime/export attribution when a model is involved.
    pub provenance: Option<Provenance>,
}
/// Combinable watermark evidence for wave 6 inspect-image (C2PA stays independent).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WatermarkReport {
    /// WATERMARK_SCHEMA.
    pub schema: String,
    /// Exact image digest.
    pub image_sha256: String,
    /// Independent scheme findings, including unavailable primary decoder.
    pub findings: Vec<WatermarkFinding>,
    /// No detection is never evidence of human origin.
    pub absence_limit: String,
}
impl WatermarkReport {
    /// Validate binding, schemes, payloads and confidence before exposing evidence.
    pub fn validate(&self, image: &VisionImage) -> Result<()> {
        if self.schema != WATERMARK_SCHEMA
            || self.image_sha256 != image.sha256
            || self.absence_limit != ABSENCE_LIMIT
            || self.findings.len() > 8
        {
            return Err(VisionError::Invalid("watermark receipt binding".into()));
        }
        let mut schemes = std::collections::BTreeSet::new();
        for f in &self.findings {
            if f.scheme.is_empty()
                || !schemes.insert(&f.scheme)
                || !matches!(
                    f.status.as_str(),
                    "detected" | "not_detected" | "unavailable"
                )
                || f.confidence
                    .is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
                || f.interpretation.is_empty()
                || (f.status == "detected" && f.payload_hex.is_none())
                || f.payload_hex.as_ref().is_some_and(|p| {
                    p.len() > 1024 || p.len() % 2 != 0 || !p.bytes().all(|b| b.is_ascii_hexdigit())
                })
            {
                return Err(VisionError::Invalid("watermark finding".into()));
            }
            if let Some(p) = &f.provenance {
                p.validate()?;
            }
        }
        Ok(())
    }
}
/// Primary TrustMark boundary; exact decoder variant/ECC belongs to the adapter.
pub trait WatermarkDecoder {
    /// Decode a compatible scheme; failure/unavailability is never absence.
    fn decode(&mut self, image: &VisionImage) -> Result<WatermarkFinding>;
}
/// Known-message legacy DWT/DCT marker settings; arbitrary bits are never detection.
#[derive(Debug, Clone)]
pub struct DwtConfig {
    /// Caller-specified known compatible payload (1..64 bytes).
    pub expected_payload: Vec<u8>,
    /// Coefficient quantization step, normally declared by the embedding workflow.
    pub quantization_step: f32,
    /// Required fraction of block votes supporting the expected bits (0.75..1).
    pub minimum_agreement: f32,
}
fn ac_coefficient(block: &[f32; 16]) -> f32 {
    let mut best = 0.;
    for v in 0..4 {
        for u in 0..4 {
            if u == 0 && v == 0 {
                continue;
            }
            let mut coefficient = 0.;
            for y in 0..4 {
                for x in 0..4 {
                    coefficient += block[y * 4 + x]
                        * (((2 * x + 1) as f32 * u as f32 * std::f32::consts::PI) / 8.).cos()
                        * (((2 * y + 1) as f32 * v as f32 * std::f32::consts::PI) / 8.).cos();
                }
            }
            coefficient *=
                0.5 * if u == 0 {
                    std::f32::consts::FRAC_1_SQRT_2
                } else {
                    1.
                } * if v == 0 {
                    std::f32::consts::FRAC_1_SQRT_2
                } else {
                    1.
                };
            if coefficient.abs() > best.abs() {
                best = coefficient;
            }
        }
    }
    best.abs()
}
/// Native configurable legacy DWT/DCT known-message decoder. No learned weights.
/// Its exact parity with any upstream embedding workflow must be separately tested.
pub fn decode_dwt(image: &VisionImage, c: &DwtConfig) -> Result<WatermarkFinding> {
    if c.expected_payload.is_empty()
        || c.expected_payload.len() > 64
        || !c.quantization_step.is_finite()
        || c.quantization_step <= 0.
        || !c.minimum_agreement.is_finite()
        || !(0.75..=1.).contains(&c.minimum_agreement)
    {
        return Err(VisionError::Invalid("legacy watermark settings".into()));
    }
    let w = image.pixels.width() / 2;
    let h = image.pixels.height() / 2;
    let bits = c.expected_payload.len() * 8;
    let mut votes = vec![[0u32; 2]; bits];
    let mut index = 0usize;
    let u = |x, y| {
        let p = image.pixels.get_pixel(x, y);
        let r = f32::from(p[0]);
        let g = f32::from(p[1]);
        let b = f32::from(p[2]);
        let l = 0.299 * r + 0.587 * g + 0.114 * b;
        128. + 0.492 * (b - l)
    };
    for by in (0..h.saturating_sub(3)).step_by(4) {
        for bx in (0..w.saturating_sub(3)).step_by(4) {
            let mut block = [0.; 16];
            for y in 0..4 {
                for x in 0..4 {
                    let xx = 2 * (bx + x);
                    let yy = 2 * (by + y);
                    block[(y * 4 + x) as usize] =
                        (u(xx, yy) + u(xx + 1, yy) + u(xx, yy + 1) + u(xx + 1, yy + 1)) / 2.;
                }
            }
            let value = ac_coefficient(&block);
            let bit = usize::from(value % c.quantization_step > c.quantization_step / 2.);
            votes[index % bits][bit] += 1;
            index += 1;
        }
    }
    if index < bits * 3 {
        return Ok(WatermarkFinding {
            scheme: "invisible-watermark-dwt-dct".into(),
            status: "unavailable".into(),
            payload_hex: None,
            confidence: None,
            interpretation:
                "Too few complete blocks for three repetitions of the declared payload.".into(),
            provenance: None,
        });
    }
    Ok(decide_votes(&votes, c))
}
fn decide_votes(votes: &[[u32; 2]], c: &DwtConfig) -> WatermarkFinding {
    let mut recovered = vec![0u8; c.expected_payload.len()];
    let mut minimum = 1.0f32;
    for (bit, v) in votes.iter().enumerate() {
        let expected = usize::from((c.expected_payload[bit / 8] >> (7 - bit % 8)) & 1);
        let n = v[0] + v[1];
        let agreement = if n == 0 {
            0.
        } else {
            v[expected] as f32 / n as f32
        };
        minimum = minimum.min(agreement);
        if v[1] > v[0] {
            recovered[bit / 8] |= 1 << (7 - bit % 8);
        }
    }
    let detected = recovered == c.expected_payload && minimum >= c.minimum_agreement;
    WatermarkFinding {
        scheme: "invisible-watermark-dwt-dct".into(),
        status: if detected { "detected" } else { "not_detected" }.into(),
        payload_hex: detected.then(|| recovered.iter().map(|b| format!("{b:02x}")).collect()),
        confidence: Some(minimum),
        interpretation: format!(
            "Known-message match only, minimum per-bit block agreement; U-channel Haar LL, 4x4 orthonormal DCT, maximum AC magnitude, step {}. Not generator authentication. Upstream/export parity remains unqualified.",
            c.quantization_step
        ),
        provenance: None,
    }
}
/// Compose primary and optional legacy findings without a real/fake verdict.
pub fn inspect(
    image: &VisionImage,
    primary: Option<&mut dyn WatermarkDecoder>,
    legacy: Option<&DwtConfig>,
) -> Result<WatermarkReport> {
    let first = if let Some(p) = primary {
        p.decode(image)?
    } else {
        WatermarkFinding {
            scheme: "trustmark".into(),
            status: "unavailable".into(),
            payload_hex: None,
            confidence: None,
            interpretation:
                "Exact decoder variant, ECC scheme, export pin and runtime parity not installed."
                    .into(),
            provenance: None,
        }
    };
    let mut findings = vec![first];
    if let Some(c) = legacy {
        findings.push(decode_dwt(image, c)?);
    }
    let r = WatermarkReport {
        schema: WATERMARK_SCHEMA.into(),
        image_sha256: image.sha256.clone(),
        findings,
        absence_limit: ABSENCE_LIMIT.into(),
    };
    r.validate(image)?;
    Ok(r)
}
#[cfg(test)]
mod tests {
    use super::super::models::digest;
    use super::*;
    #[test]
    fn random_payload_recovery_is_never_detection() {
        let c = DwtConfig {
            expected_payload: vec![0xaa],
            quantization_step: 36.,
            minimum_agreement: 0.8,
        };
        let wrong = vec![[10, 0]; 8];
        assert_eq!(decide_votes(&wrong, &c).status, "not_detected");
        let votes = (0..8)
            .map(|b| if b % 2 == 0 { [0, 10] } else { [10, 0] })
            .collect::<Vec<_>>();
        let f = decide_votes(&votes, &c);
        assert_eq!(f.payload_hex.as_deref(), Some("aa"));
        assert_eq!(f.confidence, Some(1.));
    }
    #[test]
    fn absence_and_unavailable_are_distinct_from_authenticity() {
        let i = VisionImage {
            pixels: image::RgbImage::new(16, 16),
            sha256: digest(b"fixture"),
        };
        let c = DwtConfig {
            expected_payload: vec![1],
            quantization_step: 36.,
            minimum_agreement: 0.8,
        };
        let r = inspect(&i, None, Some(&c)).unwrap();
        assert_eq!(r.findings[0].status, "unavailable");
        assert_eq!(r.findings[1].status, "unavailable");
        assert!(r.absence_limit.contains("does not establish human origin"));
    }
    #[test]
    fn native_dwt_recovers_generated_known_marker() {
        let c = DwtConfig {
            expected_payload: vec![0xaa],
            quantization_step: 36.,
            minimum_agreement: 0.8,
        };
        let mut pixels = image::RgbImage::new(128, 128);
        for by in 0..16 {
            for bx in 0..16 {
                let bit = ((c.expected_payload[0] >> (7 - (by * 16 + bx) % 8)) & 1) as f32;
                let coefficient = if bit == 0. { 45. } else { 63. };
                for y in 0..4 {
                    for x in 0..4 {
                        let ll = 256.
                            + coefficient
                                * 0.35355338
                                * (((2 * x + 1) as f32 * std::f32::consts::PI) / 8.).cos();
                        let channel_u = ll / 2.;
                        let blue = (128. + (channel_u - 128.) / (0.492 * (1. - 0.114)))
                            .round()
                            .clamp(0., 255.) as u8;
                        for dy in 0..2 {
                            for dx in 0..2 {
                                pixels.put_pixel(
                                    bx * 8 + x * 2 + dx,
                                    by * 8 + y * 2 + dy,
                                    image::Rgb([128, 128, blue]),
                                );
                            }
                        }
                    }
                }
            }
        }
        let i = VisionImage {
            pixels,
            sha256: digest(b"generated"),
        };
        assert_eq!(decode_dwt(&i, &c).unwrap().status, "detected");
    }
}
