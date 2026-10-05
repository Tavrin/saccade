//! Pinned TrustMark Q neural decoder. ECC and upstream resizer parity remain deferred.
use super::{
    models::{Model, Result, VisionError},
    runtime::OnnxModel,
    vision::{Provenance, VisionImage},
    watermark::{WatermarkDecoder, WatermarkFinding},
};
use std::path::Path;
/// Q decoder with a verified complete graph. Raw neural bits are never watermark detection.
pub struct TrustMarkQ {
    graph: OnnxModel,
}
impl TrustMarkQ {
    /// Load explicitly cached, pinned Q decoder on CPU.
    pub fn load(model: &Model, cache: &Path, library: &Path, download: bool) -> Result<Self> {
        if model.id != "trustmark" || model.input.adapter != "trustmark-q-v1" {
            return Err(VisionError::Invalid("TrustMark Q adapter".into()));
        }
        Ok(Self {
            graph: OnnxModel::load(model, cache, library, download)?,
        })
    }
    /// Run the neural graph and preserve all 100 logits. This does not verify a BCH payload.
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
        let (_, provenance) = self.logits(image)?;
        Ok(WatermarkFinding {
            scheme:"trustmark-Q".into(), status:"unavailable".into(), payload_hex:None, confidence:None,
            interpretation:"Pinned Q neural graph executed (100 finite logits). BCH/ECC decoding, upstream antialiased resizer parity and a positive encoded sample are deferred; raw bits do not establish watermark presence or absence.".into(),
            provenance:Some(provenance),
        })
    }
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
}
