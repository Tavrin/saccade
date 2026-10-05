//! Frozen inclusion regions and independent collateral measurements.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Frozen inclusion mask; never recomputed from surviving candidate objects.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenRegion {
    /// saccade-frozen-region.v1.
    pub schema: String,
    /// Stable region name.
    pub region_id: String,
    /// include; black/zero is outside, one is inside.
    pub semantics: String,
    /// Original reference file hash, not a preview hash.
    pub reference_sha256: String,
    /// Image dimensions.
    pub dimensions: [u32; 2],
    /// Binary 0/1 values, row major.
    pub inclusion: Vec<u8>,
    /// SHA-256 of binary inclusion bytes.
    pub mask_sha256: String,
    /// Authoring method, selector/phrase and model/export provenance when applicable.
    pub provenance: BTreeMap<String, String>,
}
/// Exact sample and full-frame FLIP statistics within a scope.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scope {
    /// Number of pixels in the scope.
    pub pixels: u64,
    /// Native samples differ at these pixels, including alpha and native bit depth.
    pub changed_pixels: u64,
    /// Ordered mean of full-frame FLIP values in the scope.
    pub mean_flip: f64,
    /// Maximum full-frame FLIP value in the scope.
    pub max_flip: f32,
}
/// Measurements do not establish success of the requested semantic edit.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Measurement {
    /// saccade-localized.v1.
    pub schema: String,
    /// Frozen region and its capture identity.
    pub region: FrozenRegion,
    /// Candidate encoded file hash.
    pub candidate_sha256: String,
    /// Full-frame viewing condition.
    pub pixels_per_degree: f32,
    /// Intended-region measurements.
    pub inside: Scope,
    /// Protected complement, with no configuration exclusions applied.
    pub outside: Scope,
    /// One-pixel four-neighbor boundary, including both sides of the mask.
    pub boundary: Scope,
    /// Whether native sample change occurred inside.
    pub intended_change_detected: bool,
    /// Whether exact outside preservation was requested.
    pub exact_outside: bool,
    /// Declared perceptual complement threshold when exact_outside is false.
    pub maximum_outside_flip: f32,
    /// preserved or collateral_change; never semantic approval.
    pub collateral: String,
    /// Numerical/interpretation limits.
    pub limits: Vec<String>,
}
/// Frozen collateral policy; exact preservation is the default CLI policy.
#[derive(Debug, Clone, Copy)]
pub struct Policy {
    /// FLIP viewing condition.
    pub ppd: f32,
    /// Compare native samples outside, including alpha and precision.
    pub exact_outside: bool,
    /// Maximum complement error when exact preservation is disabled.
    pub maximum_outside_flip: f32,
}
/// Geometry supplied by a DOM capture producer for one selector.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectorGeometry {
    /// Selector exactly as resolved by the producer.
    pub selector: String,
    /// Capture-pixel boxes [x,y,width,height], after scroll/device-scale conversion.
    pub boxes: Vec<[u32; 4]>,
}
/// Capture-bound DOM geometry. Empty matches remain missing evidence.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomMetadata {
    /// saccade-dom-regions.v1.
    pub schema: String,
    /// SHA-256 of the reference screenshot bytes.
    pub reference_sha256: String,
    /// Screenshot pixel dimensions.
    pub dimensions: [u32; 2],
    /// Producer-resolved selectors.
    pub selectors: Vec<SelectorGeometry>,
}
/// SHA-256 for immutable image or binary mask bytes.
pub fn digest(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
/// Freezes a nonempty proper inclusion mask before numerical measurement.
pub fn freeze(
    reference_sha256: String,
    dimensions: [u32; 2],
    inclusion: Vec<u8>,
    provenance: BTreeMap<String, String>,
) -> Result<FrozenRegion> {
    let region = FrozenRegion {
        schema: "saccade-frozen-region.v1".into(),
        region_id: "intended".into(),
        semantics: "include".into(),
        reference_sha256,
        dimensions,
        mask_sha256: digest(&inclusion),
        inclusion,
        provenance,
    };
    validate(&region)?;
    Ok(region)
}
fn validate(region: &FrozenRegion) -> Result<()> {
    let [w, h] = region.dimensions;
    if region.schema != "saccade-frozen-region.v1"
        || region.region_id.is_empty()
        || region.semantics != "include"
        || w == 0
        || h == 0
        || (w as u64 * h as u64) > 16_777_216
        || region.inclusion.len() as u64 != w as u64 * h as u64
        || region.inclusion.iter().any(|&v| v > 1)
        || !region.inclusion.contains(&1)
        || !region.inclusion.contains(&0)
        || region.mask_sha256 != digest(&region.inclusion)
        || region.reference_sha256.len() != 64
        || !region
            .reference_sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(Error::Config(
            "invalid frozen region, hash, dimensions or empty inside/complement (max 16M pixels)"
                .into(),
        ));
    }
    Ok(())
}
/// Freezes union of explicit pixel boxes; invalid/out-of-frame boxes are rejected.
pub fn boxes(
    reference_sha256: String,
    dimensions: [u32; 2],
    boxes: &[[u32; 4]],
    provenance: BTreeMap<String, String>,
) -> Result<FrozenRegion> {
    let [w, h] = dimensions;
    if w as u64 * h as u64 > 16_777_216 {
        return Err(Error::Config("region pixel budget exceeded".into()));
    }
    let mut inclusion = vec![0; w as usize * h as usize];
    for &[x, y, bw, bh] in boxes {
        if bw == 0
            || bh == 0
            || x.checked_add(bw).is_none_or(|v| v > w)
            || y.checked_add(bh).is_none_or(|v| v > h)
        {
            return Err(Error::Config("box outside reference pixels".into()));
        }
        for row in y..y + bh {
            for col in x..x + bw {
                inclusion[(row * w + col) as usize] = 1;
            }
        }
    }
    freeze(reference_sha256, dimensions, inclusion, provenance)
}
/// Resolves a unique selector from reference-bound producer metadata; no browser is queried.
pub fn selector(
    metadata: &DomMetadata,
    name: &str,
    reference_sha256: &str,
    dimensions: [u32; 2],
) -> Result<FrozenRegion> {
    let matches: Vec<_> = metadata
        .selectors
        .iter()
        .filter(|s| s.selector == name)
        .collect();
    if metadata.schema != "saccade-dom-regions.v1"
        || metadata.reference_sha256 != reference_sha256
        || metadata.dimensions != dimensions
        || matches.len() != 1
        || matches[0].boxes.len() != 1
    {
        return Err(Error::Config(
            "selector missing, ambiguous or not bound to the reference capture".into(),
        ));
    }
    boxes(
        reference_sha256.into(),
        dimensions,
        &matches[0].boxes,
        BTreeMap::from([
            ("method".into(), "dom_selector".into()),
            ("selector".into(), name.into()),
        ]),
    )
}
/// Computes full-frame FLIP first, then aggregates inside, complement and boundary.
/// Exact collateral uses native samples and ignores ordinary comparison exclusions.
pub fn measure(
    reference: &image::DynamicImage,
    candidate: &image::DynamicImage,
    reference_sha256: &str,
    candidate_sha256: String,
    region: FrozenRegion,
    policy: Policy,
) -> Result<Measurement> {
    validate(&region)?;
    let Policy {
        ppd,
        exact_outside,
        maximum_outside_flip,
    } = policy;
    if region.reference_sha256 != reference_sha256
        || region.dimensions != [reference.width(), reference.height()]
        || reference.width() != candidate.width()
        || reference.height() != candidate.height()
        || reference.color() != candidate.color()
        || !maximum_outside_flip.is_finite()
        || !(0.0..=1.0).contains(&maximum_outside_flip)
        || !ppd.is_finite()
        || ppd <= 0.0
        || matches!(
            reference.color(),
            image::ColorType::Rgb32F | image::ColorType::Rgba32F
        )
    {
        return Err(Error::Config("localized inputs differ in dimensions/sample type, are HDR or have invalid policy/identity".into()));
    }
    let comparison = crate::compare::compare_rgba(
        &candidate.to_rgba8(),
        &reference.to_rgba8(),
        &crate::compare::CompareOptions {
            pixels_per_degree: ppd,
            ..Default::default()
        },
    )?;
    let [w, h] = region.dimensions;
    let boundary: Vec<_> = region
        .inclusion
        .iter()
        .enumerate()
        .map(|(i, &value)| {
            let x = i as u32 % w;
            let y = i as u32 / w;
            [
                (x > 0).then(|| i - 1),
                (x + 1 < w).then(|| i + 1),
                (y > 0).then(|| i - w as usize),
                (y + 1 < h).then(|| i + w as usize),
            ]
            .into_iter()
            .flatten()
            .any(|n| region.inclusion[n] != value)
        })
        .collect();
    let stride = reference.color().bytes_per_pixel() as usize;
    let changed: Vec<_> = reference
        .as_bytes()
        .chunks_exact(stride)
        .zip(candidate.as_bytes().chunks_exact(stride))
        .map(|(a, b)| a != b)
        .collect();
    let scope = |select: &dyn Fn(usize) -> bool| -> Scope {
        let (mut pixels, mut changed_pixels, mut sum, mut max) = (0, 0, 0.0, 0.0f32);
        for (i, &error) in comparison.error_map.iter().enumerate() {
            if select(i) {
                pixels += 1;
                changed_pixels += u64::from(changed[i]);
                sum += error as f64;
                max = max.max(error);
            }
        }
        Scope {
            pixels,
            changed_pixels,
            mean_flip: if pixels > 0 { sum / pixels as f64 } else { 0.0 },
            max_flip: max,
        }
    };
    let inside = scope(&|i| region.inclusion[i] == 1);
    let outside = scope(&|i| region.inclusion[i] == 0);
    let boundary = scope(&|i| boundary[i]);
    let collateral = if (exact_outside && outside.changed_pixels > 0)
        || (!exact_outside && outside.max_flip > maximum_outside_flip)
    {
        "collateral_change"
    } else {
        "preserved"
    }
    .into();
    Ok(Measurement{schema:"saccade-localized.v1".into(),region,candidate_sha256,pixels_per_degree:ppd,intended_change_detected:inside.changed_pixels>0,inside,outside,boundary,exact_outside,maximum_outside_flip,collateral,limits:vec!["Spatial change does not establish success of the requested semantic edit.".into(),"Full-frame FLIP neighborhood support can cross the region boundary; exact sample differences are reported independently, including alpha.".into(),"SDR perceptual measurement converts 16-bit samples to 8-bit; exact collateral retains native precision. HDR/float inputs are rejected.".into()]})
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn independent_complement_catches_tiny_boundary_and_hidden_defects() {
        let reference = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            32,
            32,
            image::Rgba([60, 70, 80, 255]),
        ));
        let hash = digest(b"reference");
        let region = boxes(hash.clone(), [32, 32], &[[8, 8, 8, 8]], BTreeMap::new()).unwrap();
        let mut candidate = reference.to_rgba8();
        candidate.put_pixel(10, 10, image::Rgba([100, 70, 80, 255]));
        let run = |c: image::RgbaImage| {
            measure(
                &reference,
                &image::DynamicImage::ImageRgba8(c),
                &hash,
                digest(b"candidate"),
                region.clone(),
                Policy {
                    ppd: 67.0,
                    exact_outside: true,
                    maximum_outside_flip: 1.0,
                },
            )
            .unwrap()
        };
        let inside = run(candidate.clone());
        assert_eq!(inside.inside.changed_pixels, 1);
        assert_eq!(inside.collateral, "preserved");
        candidate.put_pixel(0, 0, image::Rgba([61, 70, 80, 255]));
        let tiny = run(candidate.clone());
        assert_eq!(tiny.outside.changed_pixels, 1);
        assert_eq!(tiny.collateral, "collateral_change");
        candidate.put_pixel(7, 9, image::Rgba([60, 70, 80, 254]));
        assert!(run(candidate).boundary.changed_pixels > 0);
        assert!(!run(reference.to_rgba8()).intended_change_detected);
        let metadata = DomMetadata {
            schema: "saccade-dom-regions.v1".into(),
            reference_sha256: hash.clone(),
            dimensions: [32, 32],
            selectors: vec![],
        };
        assert!(selector(&metadata, "main .card", &hash, [32, 32]).is_err());
        let mut stale = region.clone();
        stale.inclusion[0] = 1;
        assert!(
            measure(
                &reference,
                &reference,
                &hash,
                hash.clone(),
                stale,
                Policy {
                    ppd: 67.0,
                    exact_outside: true,
                    maximum_outside_flip: 0.0
                }
            )
            .is_err()
        );
        assert!(boxes(hash, [32, 32], &[[0, 0, 32, 32]], BTreeMap::new()).is_err());
    }
}
