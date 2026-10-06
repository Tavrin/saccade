//! Face detection and declared crop safety; no identity recognition.
use super::{
    models::{Result, VisionError},
    vision::{Provenance, Rect, VisionImage},
};
use serde::{Deserialize, Serialize};
/// Face observation contract.
pub const FACES_SCHEMA: &str = "saccade-faces.v1";
/// Declared crop assessment contract.
pub const CROP_SCHEMA: &str = "saccade-crop-check.v1";
/// Face detector recall limits, mandatory in reports.
pub const FACE_LIMIT: &str = "Protect detected faces only. No detection does not certify that no face is present; small, occluded or out-of-domain faces may be missed. This is not identity recognition.";
/// Face box and optional landmarks, never an identity.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Face {
    /// Original-pixel box.
    pub bbox: Rect,
    /// Predicted detection score.
    pub score: f32,
    /// Original-pixel landmarks; empty for exports without a landmark head.
    pub landmarks: Vec<[f32; 2]>,
}
/// Source-bound face detector receipt.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FaceReport {
    /// FACES_SCHEMA.
    pub schema: String,
    /// Exact encoded image digest.
    pub image_sha256: String,
    /// Original resolution.
    pub image_size: [u32; 2],
    /// Detected faces, never recognition labels.
    pub faces: Vec<Face>,
    /// Exact model/export identity.
    pub provenance: Provenance,
    /// Mandatory detection-recall limits.
    pub limitations: String,
}
impl FaceReport {
    /// Reject malformed geometry/scores and stale input receipts.
    pub fn validate(&self, image: &VisionImage) -> Result<()> {
        if crate::report_links::original_schema(&self.schema) != FACES_SCHEMA
            || self.image_sha256 != image.sha256
            || self.image_size != image.size()
            || self.faces.len() > 256
            || self.limitations != FACE_LIMIT
        {
            return Err(VisionError::Invalid(
                "face receipt binding/count/limits".into(),
            ));
        }
        self.provenance.validate()?;
        for f in &self.faces {
            f.bbox.validate(image.size())?;
            if !f.score.is_finite()
                || !(0.0..=1.0).contains(&f.score)
                || !matches!(f.landmarks.len(), 0 | 5)
                || f.landmarks.iter().any(|p| {
                    p.iter().any(|v| !v.is_finite())
                        || p[0] < 0.
                        || p[1] < 0.
                        || p[0] >= image.size()[0] as f32
                        || p[1] >= image.size()[1] as f32
                })
            {
                return Err(VisionError::Invalid("face score/landmarks".into()));
            }
        }
        Ok(())
    }
}
/// Clean face-detector boundary for the coordinator's crop/inspect integration.
pub trait FaceDetector {
    /// Detect original-pixel boxes/landmarks with explicit attribution.
    fn faces(&mut self, image: &VisionImage) -> Result<(Vec<Face>, Provenance)>;
}
/// Execute a detector and validate its returned receipt.
pub fn detect(image: &VisionImage, detector: &mut dyn FaceDetector) -> Result<FaceReport> {
    let (faces, provenance) = detector.faces(image)?;
    let r = FaceReport {
        schema: FACES_SCHEMA.into(),
        image_sha256: image.sha256.clone(),
        image_size: image.size(),
        faces,
        provenance,
        limitations: FACE_LIMIT.into(),
    };
    r.validate(image)?;
    Ok(r)
}
/// Face-preserving declared crop, ratio or original-pixel rectangle.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CropSpec {
    /// Largest crop with specified width/height ratio.
    Ratio {
        /// Positive horizontal ratio component.
        width: f32,
        /// Positive vertical ratio component.
        height: f32,
    },
    /// Declared original-pixel rectangle.
    Rectangle {
        /// Explicit crop, never silently clamped.
        rect: Rect,
    },
}
/// Independent outcome for each detected face.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FaceCropStatus {
    /// Entire face retained.
    Included,
    /// Crop intersects and cuts through the face.
    Cut,
    /// Entire detected face excluded.
    Excluded,
}
/// Per-declaration evidence; safe suggestion can be unavailable.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CropResult {
    /// Original declaration preserved.
    pub declared: CropSpec,
    /// Actual original-pixel crop at the declared focal point.
    pub crop: Rect,
    /// Status in face-detector order.
    pub faces: Vec<FaceCropStatus>,
    /// A crop with the same ratio retaining all detected faces, if geometrically possible.
    pub suggested_safe_crop: Option<Rect>,
    /// Why a suggestion exists/does not exist; recall limits remain independent.
    pub suggestion_reason: String,
}
/// Crop assessment carries the complete face receipt.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CropReport {
    /// CROP_SCHEMA.
    pub schema: String,
    /// Face detector evidence including provenance and recall limits.
    pub detection: FaceReport,
    /// Original-pixel focal point, defaults to image center.
    pub focal_point: [f32; 2],
    /// One result per declared crop; never dropped.
    pub crops: Vec<CropResult>,
}
fn ratio_crop(size: [u32; 2], ratio: f32, focal: [f32; 2]) -> Result<Rect> {
    if !ratio.is_finite() || !(0.001..=1000.).contains(&ratio) {
        return Err(VisionError::Invalid("crop aspect ratio".into()));
    }
    let width = (size[1] as f32 * ratio).min(size[0] as f32);
    let height = width / ratio;
    let crop = Rect {
        x: (focal[0] - width / 2.).clamp(0., size[0] as f32 - width),
        y: (focal[1] - height / 2.).clamp(0., size[1] as f32 - height),
        width,
        height,
    };
    crop.validate(size)?;
    Ok(crop)
}
fn safe_crop(size: [u32; 2], crop: Rect, faces: &[Face], focal: [f32; 2]) -> Option<Rect> {
    if faces.is_empty() {
        return Some(crop);
    }
    let minx = faces.iter().map(|f| f.bbox.x).fold(f32::INFINITY, f32::min);
    let miny = faces.iter().map(|f| f.bbox.y).fold(f32::INFINITY, f32::min);
    let maxx = faces
        .iter()
        .map(|f| f.bbox.x + f.bbox.width)
        .fold(0., f32::max);
    let maxy = faces
        .iter()
        .map(|f| f.bbox.y + f.bbox.height)
        .fold(0., f32::max);
    let xlo = (maxx - crop.width).max(0.);
    let xhi = minx.min(size[0] as f32 - crop.width);
    let ylo = (maxy - crop.height).max(0.);
    let yhi = miny.min(size[1] as f32 - crop.height);
    if xlo > xhi || ylo > yhi {
        return None;
    }
    Some(Rect {
        x: (focal[0] - crop.width / 2.).clamp(xlo, xhi),
        y: (focal[1] - crop.height / 2.).clamp(ylo, yhi),
        ..crop
    })
}
/// Evaluate every declared crop and suggest translations retaining all detections.
pub fn crop_check(
    image: &VisionImage,
    detection: &FaceReport,
    specs: &[CropSpec],
    focal: Option<[f32; 2]>,
) -> Result<CropReport> {
    detection.validate(image)?;
    if specs.is_empty() || specs.len() > 32 {
        return Err(VisionError::Invalid("declare 1..32 crops".into()));
    }
    let size = image.size();
    let focal = focal.unwrap_or([size[0] as f32 / 2., size[1] as f32 / 2.]);
    if focal.iter().any(|v| !v.is_finite())
        || focal[0] < 0.
        || focal[1] < 0.
        || focal[0] >= size[0] as f32
        || focal[1] >= size[1] as f32
    {
        return Err(VisionError::Invalid("focal point outside image".into()));
    }
    let crops = specs
        .iter()
        .map(|declared| {
            let crop = match declared {
                CropSpec::Ratio { width, height } => {
                    if !width.is_finite() || !height.is_finite() || *width <= 0. || *height <= 0. {
                        return Err(VisionError::Invalid("crop ratio components".into()));
                    }
                    ratio_crop(size, width / height, focal)?
                }
                CropSpec::Rectangle { rect } => {
                    rect.validate(size)?;
                    *rect
                }
            };
            let statuses = detection
                .faces
                .iter()
                .map(|f| {
                    if crop.contains(f.bbox) {
                        FaceCropStatus::Included
                    } else if crop.intersection(f.bbox) > 0. {
                        FaceCropStatus::Cut
                    } else {
                        FaceCropStatus::Excluded
                    }
                })
                .collect();
            let suggestion = safe_crop(size, crop, &detection.faces, focal);
            Ok(CropResult {
                declared: declared.clone(),
                crop,
                faces: statuses,
                suggested_safe_crop: suggestion,
                suggestion_reason: if detection.faces.is_empty() {
                    "No detections; this is not a certificate of face absence."
                } else if suggestion.is_some() {
                    "Translation of this crop retains all detected face boxes."
                } else {
                    "No crop of these dimensions can retain all detected faces."
                }
                .into(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(CropReport {
        schema: CROP_SCHEMA.into(),
        detection: detection.clone(),
        focal_point: focal,
        crops,
    })
}
/// Privacy output: strong box redaction (constant average per detected face),
/// conservative 15% margin. Stronger than a reversible weak Gaussian blur.
pub fn blur_faces(image: &VisionImage, report: &FaceReport) -> Result<image::RgbImage> {
    report.validate(image)?;
    let mut out = image.pixels.clone();
    let [w, h] = image.size();
    for f in &report.faces {
        let margin = f.bbox.width.max(f.bbox.height) * 0.15;
        let x0 = (f.bbox.x - margin).max(0.).floor() as u32;
        let y0 = (f.bbox.y - margin).max(0.).floor() as u32;
        let x1 = (f.bbox.x + f.bbox.width + margin).min(w as f32).ceil() as u32;
        let y1 = (f.bbox.y + f.bbox.height + margin).min(h as f32).ceil() as u32;
        let mut sums = [0u64; 3];
        let mut count = 0u64;
        for y in y0..y1 {
            for x in x0..x1 {
                let p = image.pixels.get_pixel(x, y);
                for c in 0..3 {
                    sums[c] += u64::from(p[c]);
                }
                count += 1;
            }
        }
        if count == 0 {
            return Err(VisionError::Invalid("empty face redaction".into()));
        }
        let p = image::Rgb(sums.map(|s| (s / count) as u8));
        for y in y0..y1 {
            for x in x0..x1 {
                out.put_pixel(x, y, p);
            }
        }
    }
    Ok(out)
}
#[cfg(any(feature = "local-models", test))]
fn nms(mut faces: Vec<Face>, threshold: f32) -> Vec<Face> {
    faces.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then(a.bbox.x.total_cmp(&b.bbox.x))
            .then(a.bbox.y.total_cmp(&b.bbox.y))
    });
    let mut keep: Vec<Face> = vec![];
    for f in faces {
        if keep.len() == 256 {
            break;
        }
        if keep.iter().all(|k| {
            let intersection = f.bbox.intersection(k.bbox);
            intersection
                / (f.bbox.width * f.bbox.height + k.bbox.width * k.bbox.height - intersection)
                <= threshold
        }) {
            keep.push(f);
        }
    }
    keep
}
#[cfg(feature = "local-models")]
impl FaceDetector for super::runtime::OnnxModel {
    fn faces(&mut self, image: &VisionImage) -> Result<(Vec<Face>, Provenance)> {
        if self.model.task != "face_detection" {
            return Err(VisionError::Invalid("face model task".into()));
        }
        let outputs = self.run(image, None)?;
        let [iw, ih] = image.size();
        let [w, h] = if self.model.input.adapter == "yunet-v1" {
            image.size().map(|v| v.div_ceil(32) * 32)
        } else {
            self.model.input.resolution
        };
        if w == 0 || h == 0 {
            return Err(VisionError::Invalid(
                "face export requires fixed input resolution".into(),
            ));
        }
        let mut faces = vec![];
        match self.model.input.adapter.as_str() {
            "yunet-v1" => {
                if self.model.id != "yunet-2026may" || self.model.input.color != "BGR" {
                    return Err(VisionError::Invalid(
                        "YuNet identity/channel contract".into(),
                    ));
                }
                for stride in [8u32, 16, 32] {
                    let count = (w / stride * h / stride) as usize;
                    let get = |prefix: &str, channels: usize| -> Result<&[f32]> {
                        let name = format!("{prefix}_{stride}");
                        let data = outputs
                            .get(&name)
                            .ok_or_else(|| VisionError::Invalid(format!("YuNet missing {name}")))?;
                        if data.shape != [1, count as i64, channels as i64] {
                            return Err(VisionError::Invalid("YuNet tensor shape".into()));
                        }
                        Ok(&data.values)
                    };
                    let cls = get("cls", 1)?;
                    let obj = get("obj", 1)?;
                    let boxes = get("bbox", 4)?;
                    let landmarks = get("kps", 10)?;
                    for index in 0..count {
                        if !(0.0..=1.0).contains(&cls[index]) || !(0.0..=1.0).contains(&obj[index])
                        {
                            return Err(VisionError::Invalid("YuNet score range".into()));
                        }
                        let score = (cls[index] * obj[index]).sqrt();
                        if score < 0.6 {
                            continue;
                        }
                        let x = (index as u32 % (w / stride)) as f32;
                        let y = (index as u32 / (w / stride)) as f32;
                        let b = &boxes[index * 4..index * 4 + 4];
                        let cx = (x + b[0]) * stride as f32;
                        let cy = (y + b[1]) * stride as f32;
                        let bw = b[2].exp() * stride as f32;
                        let bh = b[3].exp() * stride as f32;
                        if !bw.is_finite() || !bh.is_finite() {
                            return Err(VisionError::Invalid("YuNet nonfinite extent".into()));
                        }
                        // YuNet coordinates are in the native image's padded frame.
                        let x0 = (cx - bw / 2.).clamp(0., iw as f32);
                        let y0 = (cy - bh / 2.).clamp(0., ih as f32);
                        let x1 = (cx + bw / 2.).clamp(0., iw as f32);
                        let y1 = (cy + bh / 2.).clamp(0., ih as f32);
                        if x1 <= x0 || y1 <= y0 {
                            continue;
                        }
                        let points = (0..5)
                            .map(|k| {
                                [
                                    ((x + landmarks[index * 10 + k * 2]) * stride as f32)
                                        .clamp(0., iw.saturating_sub(1) as f32),
                                    ((y + landmarks[index * 10 + k * 2 + 1]) * stride as f32)
                                        .clamp(0., ih.saturating_sub(1) as f32),
                                ]
                            })
                            .collect();
                        faces.push(Face {
                            bbox: Rect {
                                x: x0,
                                y: y0,
                                width: x1 - x0,
                                height: y1 - y0,
                            },
                            score,
                            landmarks: points,
                        });
                    }
                }
            }
            "ultraface-v1" => {
                if self.model.id != "ultraface-rfb" {
                    return Err(VisionError::Invalid("UltraFace identity".into()));
                }
                let boxes = outputs
                    .get("boxes")
                    .ok_or_else(|| VisionError::Invalid("UltraFace boxes".into()))?;
                let scores = outputs
                    .get("scores")
                    .ok_or_else(|| VisionError::Invalid("UltraFace scores".into()))?;
                if boxes.shape.len() != 3
                    || boxes.shape[0] != 1
                    || boxes.shape[2] != 4
                    || scores.shape != [1, boxes.shape[1], 2]
                {
                    return Err(VisionError::Invalid("UltraFace shape".into()));
                }
                for (b, s) in boxes
                    .values
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(scores.values.as_chunks::<2>().0.iter())
                {
                    if s[1] >= 0.6 {
                        let x0 = (b[0] * iw as f32).clamp(0., iw as f32);
                        let y0 = (b[1] * ih as f32).clamp(0., ih as f32);
                        let x1 = (b[2] * iw as f32).clamp(0., iw as f32);
                        let y1 = (b[3] * ih as f32).clamp(0., ih as f32);
                        if x1 <= x0 || y1 <= y0 {
                            continue;
                        }
                        faces.push(Face {
                            bbox: Rect {
                                x: x0,
                                y: y0,
                                width: x1 - x0,
                                height: y1 - y0,
                            },
                            score: s[1],
                            landmarks: vec![],
                        });
                    }
                }
            }
            _ => {
                return Err(VisionError::Unavailable(
                    "face export adapter unsupported".into(),
                ));
            }
        }
        Ok((nms(faces, 0.3), self.provenance(image)))
    }
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::super::models::digest;
    use super::*;
    fn image() -> VisionImage {
        VisionImage {
            pixels: image::RgbImage::from_fn(100, 80, |x, y| image::Rgb([x as u8, y as u8, 200])),
            sha256: digest(b"fixture"),
        }
    }
    fn report(i: &VisionImage) -> FaceReport {
        FaceReport {
            schema: FACES_SCHEMA.into(),
            image_sha256: i.sha256.clone(),
            image_size: i.size(),
            faces: vec![Face {
                bbox: Rect {
                    x: 5.,
                    y: 20.,
                    width: 20.,
                    height: 20.,
                },
                score: 0.9,
                landmarks: vec![],
            }],
            provenance: Provenance::fixture("yunet-2026may"),
            limitations: FACE_LIMIT.into(),
        }
    }
    #[test]
    fn distinguishes_cut_excluded_included_and_suggests_safe_translation() {
        let i = image();
        let r = report(&i);
        let crops = [
            CropSpec::Rectangle {
                rect: Rect {
                    x: 15.,
                    y: 0.,
                    width: 50.,
                    height: 80.,
                },
            },
            CropSpec::Rectangle {
                rect: Rect {
                    x: 40.,
                    y: 0.,
                    width: 50.,
                    height: 80.,
                },
            },
            CropSpec::Ratio {
                width: 1.,
                height: 1.,
            },
        ];
        let result = crop_check(&i, &r, &crops, None).unwrap();
        assert_eq!(result.crops[0].faces, [FaceCropStatus::Cut]);
        assert_eq!(result.crops[1].faces, [FaceCropStatus::Excluded]);
        assert!(
            result.crops[0]
                .suggested_safe_crop
                .unwrap()
                .contains(r.faces[0].bbox)
        );
    }
    #[test]
    fn impossible_safe_crop_and_invalid_focal_are_explicit() {
        let i = image();
        let mut r = report(&i);
        r.faces.push(Face {
            bbox: Rect {
                x: 80.,
                y: 20.,
                width: 20.,
                height: 20.,
            },
            score: 0.8,
            landmarks: vec![],
        });
        let specs = [CropSpec::Ratio {
            width: 1.,
            height: 2.,
        }];
        assert!(
            crop_check(&i, &r, &specs, None).unwrap().crops[0]
                .suggested_safe_crop
                .is_none()
        );
        assert!(crop_check(&i, &r, &specs, Some([f32::NAN, 0.])).is_err());
    }
    #[test]
    fn privacy_redacts_detected_region_and_preserves_original() {
        let i = image();
        let original = i.pixels.clone();
        let out = blur_faces(&i, &report(&i)).unwrap();
        assert_eq!(i.pixels, original);
        assert_eq!(out.get_pixel(10, 25), out.get_pixel(20, 30));
        assert_eq!(out.get_pixel(99, 79), i.pixels.get_pixel(99, 79));
    }
    #[test]
    fn nms_is_deterministic_for_overlapping_proposals() {
        let i = image();
        let f = report(&i).faces[0].clone();
        assert_eq!(nms(vec![f.clone(), f], 0.3).len(), 1);
    }
}
