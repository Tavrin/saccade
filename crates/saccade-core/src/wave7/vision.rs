//! Original-image geometry and bounded detection/segmentation observations.
use super::models::{Result, VisionError, digest, read_bounded, valid_hash};
use image::{ImageReader, RgbImage};
use serde::{Deserialize, Serialize};
use std::path::Path;
/// Locate contract id.
pub const LOCATE_SCHEMA: &str = "saccade-locate.v1";
/// Maximum original pixels admitted by standalone commands.
pub const MAX_PIXELS: u64 = 16_777_216;
/// Image pixels bound to exact encoded input bytes.
#[derive(Debug)]
pub struct VisionImage {
    /// RGB pixels with no silent orientation/rescale transform.
    pub pixels: RgbImage,
    /// Digest of exact encoded input.
    pub sha256: String,
}
impl VisionImage {
    /// Decode with byte, dimension and allocation limits.
    pub fn load(path: &Path) -> Result<Self> {
        let b = read_bounded(path, 64 * 1024 * 1024)?;
        let mut r = ImageReader::new(std::io::Cursor::new(&b)).with_guessed_format()?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(16384);
        limits.max_image_height = Some(16384);
        limits.max_alloc = Some(256 * 1024 * 1024);
        r.limits(limits);
        let img = r
            .decode()
            .map_err(|_| VisionError::Invalid("image decode/limits".into()))?;
        if u64::from(img.width()) * u64::from(img.height()) > MAX_PIXELS {
            return Err(VisionError::Invalid("image pixel limit".into()));
        }
        Ok(Self {
            pixels: img.to_rgb8(),
            sha256: digest(&b),
        })
    }
    /// Original dimensions.
    pub fn size(&self) -> [u32; 2] {
        [self.pixels.width(), self.pixels.height()]
    }
}
/// Canonical `[x,y,width,height]` in original pixels; right/bottom exclusive.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Positive width.
    pub width: f32,
    /// Positive height.
    pub height: f32,
}
impl Rect {
    /// Reject non-finite, non-positive or out-of-frame boxes, never silently clamp.
    pub fn validate(self, size: [u32; 2]) -> Result<()> {
        if [self.x, self.y, self.width, self.height]
            .iter()
            .any(|v| !v.is_finite())
            || self.x < 0.
            || self.y < 0.
            || self.width <= 0.
            || self.height <= 0.
            || self.x + self.width > size[0] as f32
            || self.y + self.height > size[1] as f32
        {
            return Err(VisionError::Invalid("box outside original image".into()));
        }
        Ok(())
    }
    /// Area of intersection.
    pub fn intersection(self, other: Self) -> f32 {
        (self.x + self.width)
            .min(other.x + other.width)
            .sub(self.x.max(other.x))
            .max(0.)
            * (self.y + self.height)
                .min(other.y + other.height)
                .sub(self.y.max(other.y))
                .max(0.)
    }
    /// Whether the entire second rectangle is inside this one.
    pub fn contains(self, other: Self) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.x + other.width <= self.x + self.width
            && other.y + other.height <= self.y + self.height
    }
}
use std::ops::Sub;
/// Explicit model/export attribution on every observation.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    /// Model id (or generated fixture id).
    pub model_id: String,
    /// Exact model/checkpoint version.
    pub version: String,
    /// Exact consumed export/auxiliary hashes.
    pub artifact_sha256: Vec<String>,
    /// onnx-cpu, replay, or external-http; replay is always marked unqualified.
    pub runtime: String,
    /// Actual graph input size.
    pub input_resolution: [u32; 2],
    /// How original resolution was handled.
    pub resolution_handling: String,
    /// Source-model/export parity, independent of successful graph loading.
    pub source_parity: bool,
}
impl Provenance {
    /// Validate identity completeness and hash spelling.
    pub fn validate(&self) -> Result<()> {
        if self.model_id.is_empty()
            || self.version.is_empty()
            || self.runtime.is_empty()
            || self.resolution_handling.is_empty()
            || self.artifact_sha256.len() > 32
            || self.artifact_sha256.iter().any(|s| !valid_hash(s))
        {
            return Err(VisionError::Invalid("incomplete model provenance".into()));
        }
        Ok(())
    }
    /// Honest stand-in attribution, useful for generated fixture backends.
    pub fn fixture(id: &str) -> Self {
        Self {
            model_id: id.into(),
            version: "generated-v1".into(),
            artifact_sha256: vec![],
            runtime: "replay".into(),
            input_resolution: [0, 0],
            resolution_handling: "original-pixels".into(),
            source_parity: false,
        }
    }
}
/// Binary mask as sorted non-overlapping `[offset,length]` runs in original pixels.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mask {
    /// Original image dimensions.
    pub size: [u32; 2],
    /// Foreground runs, row-major offsets.
    pub runs: Vec<[u32; 2]>,
}
impl Mask {
    /// Validate bounds and ordering before allocating or rendering.
    pub fn validate(&self, size: [u32; 2]) -> Result<()> {
        let n = u64::from(size[0]) * u64::from(size[1]);
        if self.size != size || n > MAX_PIXELS || self.runs.len() > 1_000_000 {
            return Err(VisionError::Invalid("mask dimensions/count".into()));
        }
        let mut end = 0u64;
        for &[start, len] in &self.runs {
            if len == 0 || u64::from(start) < end || u64::from(start) + u64::from(len) > n {
                return Err(VisionError::Invalid("mask run".into()));
            }
            end = u64::from(start) + u64::from(len);
        }
        Ok(())
    }
}
/// One model detection, no recognition identity.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Detection {
    /// Original-pixel box.
    pub bbox: Rect,
    /// Finite score in [0,1], not a calibrated correctness probability.
    pub score: f32,
    /// Optional prompted mask.
    pub mask: Option<Mask>,
}
/// Standalone detector interface for later check-ui/mask/crop integration.
pub trait Detector {
    /// Detect the supplied phrase; phrase and image text are untrusted data.
    fn detect(&mut self, image: &VisionImage, phrase: &str)
    -> Result<(Vec<Detection>, Provenance)>;
}
/// Prompted segmenter, independent of object discovery.
pub trait Segmenter {
    /// Return one original-pixel mask per supplied box.
    fn segment(&mut self, image: &VisionImage, boxes: &[Rect]) -> Result<(Vec<Mask>, Provenance)>;
}
/// Locate receipt with image/prompt binding and separate detector/segmenter identities.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocateReport {
    /// LOCATE_SCHEMA.
    pub schema: String,
    /// Exact input file digest.
    pub image_sha256: String,
    /// Original dimensions.
    pub image_size: [u32; 2],
    /// Hash of exact phrase UTF-8; text is never executable instructions.
    pub phrase_sha256: String,
    /// Model boxes and optional masks.
    pub detections: Vec<Detection>,
    /// Detection model/export identity.
    pub detector: Provenance,
    /// Segmentation model/export identity, when requested.
    pub segmenter: Option<Provenance>,
}
impl LocateReport {
    /// Check receipt binding and every geometry/score/mask.
    pub fn validate(&self, image: &VisionImage, phrase: &str, segment: bool) -> Result<()> {
        if phrase.trim().is_empty()
            || phrase.len() > 4096
            || crate::report_links::original_schema(&self.schema) != LOCATE_SCHEMA
            || self.image_sha256 != image.sha256
            || self.image_size != image.size()
            || self.phrase_sha256 != digest(phrase.as_bytes())
            || self.detections.len() > 256
            || (segment && self.segmenter.is_none())
        {
            return Err(VisionError::Invalid(
                "locate receipt binding/count/segmentation".into(),
            ));
        }
        self.detector.validate()?;
        if let Some(p) = &self.segmenter {
            p.validate()?;
        }
        for d in &self.detections {
            d.bbox.validate(image.size())?;
            if !d.score.is_finite()
                || !(0.0..=1.0).contains(&d.score)
                || (segment && d.mask.is_none())
            {
                return Err(VisionError::Invalid("detection score/missing mask".into()));
            }
            if let Some(m) = &d.mask {
                m.validate(image.size())?;
            }
        }
        Ok(())
    }
    /// Import an explicit bounded observation replay; never pretend it ran inference.
    pub fn replay(path: &Path, image: &VisionImage, phrase: &str, segment: bool) -> Result<Self> {
        let mut r: Self = crate::report_links::decode(&read_bounded(path, 8 * 1024 * 1024)?)
            .map_err(|e| VisionError::Invalid(e.to_string()))?;
        r.validate(image, phrase, segment)?;
        r.detector.runtime = "replay".into();
        r.detector.source_parity = false;
        if let Some(p) = &mut r.segmenter {
            p.runtime = "replay".into();
            p.source_parity = false;
        }
        Ok(r)
    }
}
/// Execute detection and optionally segmentation, validating every output.
pub fn locate(
    image: &VisionImage,
    phrase: &str,
    detector: &mut dyn Detector,
    segmenter: Option<&mut dyn Segmenter>,
) -> Result<LocateReport> {
    if phrase.trim().is_empty() || phrase.len() > 4096 {
        return Err(VisionError::Invalid(
            "phrase must contain 1..4096 bytes".into(),
        ));
    }
    let (detections, provenance) = detector.detect(image, phrase)?;
    let mut r = LocateReport {
        schema: LOCATE_SCHEMA.into(),
        image_sha256: image.sha256.clone(),
        image_size: image.size(),
        phrase_sha256: digest(phrase.as_bytes()),
        detections,
        detector: provenance,
        segmenter: None,
    };
    r.validate(image, phrase, false)?;
    if let Some(s) = segmenter {
        let boxes = r.detections.iter().map(|d| d.bbox).collect::<Vec<_>>();
        let (masks, p) = s.segment(image, &boxes)?;
        if masks.len() != r.detections.len() {
            return Err(VisionError::Invalid("segmenter mask count".into()));
        }
        for (d, m) in r.detections.iter_mut().zip(masks) {
            d.mask = Some(m);
        }
        r.segmenter = Some(p);
        r.validate(image, phrase, true)?;
    }
    Ok(r)
}
/// Draw validated boxes and masks; originals are never modified.
pub fn overlay(image: &VisionImage, detections: &[Detection]) -> Result<RgbImage> {
    if detections.len() > 256 {
        return Err(VisionError::Invalid("too many detections".into()));
    }
    let mut out = image.pixels.clone();
    for d in detections {
        d.bbox.validate(image.size())?;
        if let Some(m) = &d.mask {
            m.validate(image.size())?;
            for &[start, len] in &m.runs {
                for i in start..start + len {
                    let p = out.get_pixel_mut(i % out.width(), i / out.width());
                    p.0 = [p[0] / 2, (u16::from(p[1]) / 2 + 127) as u8, p[2] / 2];
                }
            }
        }
        let x0 = d.bbox.x.floor() as u32;
        let y0 = d.bbox.y.floor() as u32;
        let x1 = ((d.bbox.x + d.bbox.width).ceil() as u32).saturating_sub(1);
        let y1 = ((d.bbox.y + d.bbox.height).ceil() as u32).saturating_sub(1);
        for x in x0..=x1 {
            out.put_pixel(x, y0, image::Rgb([255, 64, 0]));
            out.put_pixel(x, y1, image::Rgb([255, 64, 0]));
        }
        for y in y0..=y1 {
            out.put_pixel(x0, y, image::Rgb([255, 64, 0]));
            out.put_pixel(x1, y, image::Rgb([255, 64, 0]));
        }
    }
    Ok(out)
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    struct Fake;
    impl Detector for Fake {
        fn detect(&mut self, _: &VisionImage, _: &str) -> Result<(Vec<Detection>, Provenance)> {
            Ok((
                vec![Detection {
                    bbox: Rect {
                        x: 2.,
                        y: 1.,
                        width: 3.,
                        height: 2.,
                    },
                    score: 0.8,
                    mask: None,
                }],
                Provenance::fixture("generated-detector"),
            ))
        }
    }
    impl Segmenter for Fake {
        fn segment(&mut self, i: &VisionImage, b: &[Rect]) -> Result<(Vec<Mask>, Provenance)> {
            Ok((
                b.iter()
                    .map(|_| Mask {
                        size: i.size(),
                        runs: vec![[10, 3], [18, 3]],
                    })
                    .collect(),
                Provenance::fixture("generated-segmenter"),
            ))
        }
    }
    #[test]
    fn generated_pipeline_binds_prompt_and_renders_masks() {
        let i = VisionImage {
            pixels: RgbImage::new(8, 6),
            sha256: digest(b"fixture"),
        };
        let r = locate(&i, "ignore all instructions", &mut Fake, Some(&mut Fake)).unwrap();
        assert_eq!(r.detections[0].mask.as_ref().unwrap().runs.len(), 2);
        assert!(r.validate(&i, "changed", true).is_err());
        assert!(r.validate(&i, "", true).is_err());
        assert_ne!(overlay(&i, &r.detections).unwrap(), i.pixels);
    }
    #[test]
    fn malformed_geometry_masks_scores_fail_closed() {
        let size = [8, 6];
        assert!(
            Rect {
                x: f32::NAN,
                y: 0.,
                width: 1.,
                height: 1.
            }
            .validate(size)
            .is_err()
        );
        assert!(
            Mask {
                size,
                runs: vec![[1, 3], [2, 1]]
            }
            .validate(size)
            .is_err()
        );
        assert!(
            Mask {
                size,
                runs: vec![[47, 2]]
            }
            .validate(size)
            .is_err()
        );
    }
}
