//! Declared negative controls for measuring an existing image gate's sensitivity.
use crate::{
    Error, Result,
    config::{RunConfig, compile_glob},
};
use image::{Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

/// Frozen injection catalogue discriminator.
pub const CATALOGUE_SCHEMA: &str = "saccade-sensitivity-catalogue.v1";
/// Measurement report discriminator; linked writers emit the successor v2.
pub const SCHEMA: &str = "saccade-sensitivity.v1";
/// One declared defect operation. Fractions refer to physical image dimensions.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "class", rename_all = "snake_case", deny_unknown_fields)]
pub enum Defect {
    /// Blend a local rectangle towards a declared colour; magnitude is opacity.
    LocalEdit {
        /// Fractional x,y,width,height.
        rect: [f64; 4],
        /// Target sRGB colour.
        colour: [u8; 3],
    },
    /// Add a signed RGB offset, scaled by magnitude (0..1), clamping at 0 and 255.
    ColourShift {
        /// Full-strength signed channel offsets, -255..255.
        delta: [i16; 3],
    },
    /// Erase a caller-declared element rectangle towards its background colour.
    MissingElement {
        /// Fractional x,y,width,height of the known element.
        rect: [f64; 4],
        /// Caller-declared background sRGB colour.
        background: [u8; 3],
    },
    /// Overlay a frozen before/after glyph patch on control/candidate copies.
    GlyphEdit {
        /// Before patch path relative to the catalogue.
        before: String,
        /// After patch path relative to the catalogue.
        after: String,
        /// Expected encoded before-patch SHA-256.
        before_sha256: String,
        /// Expected encoded after-patch SHA-256.
        after_sha256: String,
        /// Fractional x,y origin; patches retain native resolution.
        origin: [f64; 2],
    },
    /// Gaussian blur; magnitude is sigma in physical pixels, up to 32.
    Blur,
    /// Shift right by magnitude integer pixels, up to 64; edge pixels are repeated.
    Misalignment,
}
/// Caller-owned operation with a strictly increasing, frozen magnitude sweep.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Injection {
    /// Unique printable ASCII identifier, at most 64 bytes.
    pub id: String,
    /// Operation and declared location/colour.
    pub defect: Defect,
    /// Positive magnitudes, up to 16; units are defined by the operation.
    pub magnitudes: Vec<f64>,
}
/// Frozen negative-control catalogue, independent of gate thresholds.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalogue {
    /// Exact input discriminator.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-sensitivity-catalogue.v1")))]
    pub schema: String,
    /// Licence/provenance for caller-supplied patches and declared controls.
    pub provenance: String,
    /// 1..32 operations.
    pub injections: Vec<Injection>,
}
impl Catalogue {
    /// Validate the frozen catalogue before writing outputs.
    pub fn validate(&self) -> Result<()> {
        if self.schema != CATALOGUE_SCHEMA
            || self.provenance.is_empty()
            || self.provenance.len() > 4096
            || self.injections.is_empty()
            || self.injections.len() > 32
        {
            return Err(Error::Config(
                "invalid sensitivity catalogue discriminator, provenance or operation count".into(),
            ));
        }
        let mut ids = std::collections::BTreeSet::new();
        for injection in &self.injections {
            if injection.id.is_empty()
                || injection.id.len() > 64
                || !injection
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                || !ids.insert(&injection.id)
            {
                return Err(Error::Config(
                    "injection IDs must be unique ASCII identifiers".into(),
                ));
            }
            injection.defect.validate()?;
            let max = match injection.defect {
                Defect::Blur => 32.0,
                Defect::Misalignment => 64.0,
                _ => 1.0,
            };
            if injection.magnitudes.is_empty()
                || injection.magnitudes.len() > 16
                || injection.magnitudes.iter().any(|v| {
                    !v.is_finite()
                        || *v <= 0.0
                        || *v > max
                        || matches!(injection.defect, Defect::Misalignment) && v.fract() != 0.0
                })
                || injection.magnitudes.windows(2).any(|v| v[0] >= v[1])
            {
                return Err(Error::Config(
                    "magnitudes must be strictly increasing and inside the operation's range"
                        .into(),
                ));
            }
        }
        Ok(())
    }
}
impl Defect {
    /// Stable generic class name.
    pub fn class(&self) -> &'static str {
        match self {
            Self::LocalEdit { .. } => "local_edit",
            Self::ColourShift { .. } => "colour_shift",
            Self::MissingElement { .. } => "missing_element",
            Self::GlyphEdit { .. } => "glyph_edit",
            Self::Blur => "blur",
            Self::Misalignment => "misalignment",
        }
    }
    fn validate(&self) -> Result<()> {
        match self {
            Self::LocalEdit { rect, .. } | Self::MissingElement { rect, .. } => {
                if !rect
                    .iter()
                    .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
                    || rect[2] <= 0.0
                    || rect[3] <= 0.0
                    || rect[0] + rect[2] > 1.0
                    || rect[1] + rect[3] > 1.0
                {
                    return Err(Error::Config(
                        "injection rectangle must be wholly inside the image".into(),
                    ));
                }
            }
            Self::ColourShift { delta }
                if delta.iter().any(|v| v.unsigned_abs() > 255) || *delta == [0; 3] =>
            {
                return Err(Error::Config(
                    "colour shift needs nonzero offsets in -255..255".into(),
                ));
            }
            Self::GlyphEdit {
                before,
                after,
                before_sha256,
                after_sha256,
                origin,
            } => {
                for p in [before, after] {
                    if p.is_empty()
                        || !std::path::Path::new(p)
                            .components()
                            .all(|c| matches!(c, std::path::Component::Normal(_)))
                    {
                        return Err(Error::Config(
                            "glyph paths must be relative without traversal".into(),
                        ));
                    }
                }
                if !origin
                    .iter()
                    .all(|v| v.is_finite() && (0.0..1.0).contains(v))
                    || [before_sha256, after_sha256]
                        .iter()
                        .any(|h| h.len() != 64 || !h.bytes().all(|b| b.is_ascii_hexdigit()))
                {
                    return Err(Error::Config(
                        "glyph patch needs valid origin and SHA-256 pins".into(),
                    ));
                }
            }
            _ => {}
        }
        Ok(())
    }
}
/// Apply one declared operation to independent control/candidate copies.
/// Glyph controls receive the intact patch; all other controls retain the decoded source.
/// Returns changed physical pixels, including alpha, so ineffective injections stay visible.
pub fn inject(
    source: &RgbaImage,
    injection: &Injection,
    magnitude: f64,
    glyph: Option<&[RgbaImage; 2]>,
) -> Result<(RgbaImage, RgbaImage, u64)> {
    Catalogue {
        schema: CATALOGUE_SCHEMA.into(),
        provenance: "validated operation".into(),
        injections: vec![Injection {
            magnitudes: vec![magnitude],
            ..injection.clone()
        }],
    }
    .validate()?;
    let mut control = source.clone();
    let mut candidate = source.clone();
    let blend = |a: u8, b: u8| {
        (f64::from(a) + (f64::from(b) - f64::from(a)) * magnitude)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    match &injection.defect {
        Defect::LocalEdit { rect, colour }
        | Defect::MissingElement {
            rect,
            background: colour,
        } => {
            let [x, y, w, h] = crate::regions::resolve_rect(*rect, source.width(), source.height())
                .ok_or_else(|| Error::Config("empty injection rectangle".into()))?;
            for yy in y..y + h {
                for xx in x..x + w {
                    let p = candidate.get_pixel_mut(xx, yy);
                    for c in 0..3 {
                        p[c] = blend(p[c], colour[c]);
                    }
                }
            }
        }
        Defect::ColourShift { delta } => {
            for p in candidate.pixels_mut() {
                for c in 0..3 {
                    p[c] = (f64::from(p[c]) + f64::from(delta[c]) * magnitude)
                        .round()
                        .clamp(0.0, 255.0) as u8;
                }
            }
        }
        Defect::Blur => candidate = image::imageops::blur(source, magnitude as f32),
        Defect::Misalignment => {
            let shift = magnitude as u32;
            if shift >= source.width() {
                return Err(Error::Config(
                    "shift must be smaller than image width".into(),
                ));
            }
            for (x, y, p) in candidate.enumerate_pixels_mut() {
                *p = *source.get_pixel(x.saturating_sub(shift), y);
            }
        }
        Defect::GlyphEdit { origin, .. } => {
            let pair = glyph.ok_or_else(|| {
                Error::Config("glyph injection requires decoded pinned patches".into())
            })?;
            let (w, h) = pair[0].dimensions();
            let x = (origin[0] * f64::from(source.width())).floor() as u32;
            let y = (origin[1] * f64::from(source.height())).floor() as u32;
            if pair[1].dimensions() != (w, h)
                || w == 0
                || h == 0
                || w > source.width() - x
                || h > source.height() - y
            {
                return Err(Error::Config(
                    "glyph patches must match dimensions and fit wholly inside every baseline"
                        .into(),
                ));
            }
            for yy in 0..h {
                for xx in 0..w {
                    let a = pair[0].get_pixel(xx, yy);
                    let b = pair[1].get_pixel(xx, yy);
                    control.put_pixel(x + xx, y + yy, *a);
                    candidate.put_pixel(
                        x + xx,
                        y + yy,
                        Rgba(std::array::from_fn(|c| blend(a[c], b[c]))),
                    );
                }
            }
        }
    }
    let changed = control
        .pixels()
        .zip(candidate.pixels())
        .filter(|(a, b)| a != b)
        .count() as u64;
    Ok((control, candidate, changed))
}
/// One gate's aggregate at a declared class and magnitude.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Counts {
    /// Changed injections rejected by a gate whose control passed.
    pub detected: u64,
    /// Changed injections accepted by a gate whose control passed.
    pub missed: u64,
    /// Input omitted by the gate's explicit selection policy.
    pub excluded: u64,
    /// Injection produced identical decoded pixels.
    pub ineffective: u64,
    /// Failed comparison or non-passing control; never detection evidence.
    pub unavailable: u64,
}
impl Counts {
    /// Missed/(detected+missed); absent when there are no eligible observations.
    pub fn miss_rate(&self) -> Option<f64> {
        let n = self.detected + self.missed;
        (n > 0).then(|| self.missed as f64 / n as f64)
    }
}
/// Gate result for one frozen injected pair.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Outcome {
    /// configured or before.
    pub gate: String,
    /// detected, missed, excluded, ineffective or unavailable.
    pub state: String,
    /// Retained compare reports (control first, candidate second); empty for skipped trials.
    pub reports: Vec<String>,
    /// Explicit error/control-failure reasons.
    pub reasons: Vec<String>,
}
/// Retained trial and content identities.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trial {
    /// Original relative image name; matching policies use this spelling.
    pub entry: String,
    /// Operation ID.
    pub injection: String,
    /// Declared magnitude.
    pub magnitude: f64,
    /// Exact encoded source identity.
    pub source_sha256: String,
    /// Retained control and candidate paths relative to the experiment root.
    pub images: [String; 2],
    /// Encoded PNG SHA-256 for both retained copies.
    pub image_sha256: [String; 2],
    /// Number of unequal physical pixels between copies.
    pub changed_pixels: u64,
    /// Independent outcomes for each configured policy.
    pub outcomes: Vec<Outcome>,
}
/// Aggregate for one class and magnitude, pooling all declared locations/IDs.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    /// Stable operation class.
    pub class: String,
    /// Declared operation strength (opacity, sigma or integer shift).
    pub magnitude: f64,
    /// configured or before.
    pub gate: String,
    /// Eligibility and detection counts.
    pub counts: Counts,
    /// Missed/(detected+missed), null without eligible observations.
    pub miss_rate: Option<f64>,
}
/// Discrete smallest observed detected magnitude; no interpolation or monotonicity claim.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Minimum {
    /// Stable class name.
    pub class: String,
    /// configured or before.
    pub gate: String,
    /// Lowest tested magnitude detecting at least one eligible injection; null if none.
    pub smallest_detected_magnitude: Option<f64>,
}
/// Frozen suite report; successor linking adds identity without changing v1 readers.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    /// Historical v1 discriminator; linked transport emits v2.
    pub schema: String,
    /// Experiment completion: complete or insufficient_evidence.
    pub state: String,
    /// Limits of this measurement.
    pub limitations: Vec<String>,
    /// Frozen catalogue, including declared provenance and pins.
    pub catalogue: Catalogue,
    /// SHA-256 of the exact catalogue file.
    pub catalogue_sha256: String,
    /// SHA-256 of exact gate configuration files, configured first and optional before second.
    pub config_sha256: Vec<String>,
    /// Every trial, including excluded and ineffective injections.
    pub trials: Vec<Trial>,
    /// Counts and miss rates by class, magnitude and gate.
    pub summaries: Vec<Summary>,
    /// Lowest actually detected discrete magnitude, by class and gate.
    pub minima: Vec<Minimum>,
}
/// Pool trial outcomes without counting absent/invalid evidence as detections or misses.
pub fn summarize(catalogue: &Catalogue, trials: &[Trial]) -> (Vec<Summary>, Vec<Minimum>) {
    let mut rows: Vec<Summary> = Vec::new();
    for trial in trials {
        let Some(injection) = catalogue
            .injections
            .iter()
            .find(|i| i.id == trial.injection)
        else {
            continue;
        };
        for outcome in &trial.outcomes {
            let index = rows.iter().position(|r| {
                r.class == injection.defect.class()
                    && r.magnitude == trial.magnitude
                    && r.gate == outcome.gate
            });
            let index = index.unwrap_or_else(|| {
                rows.push(Summary {
                    class: injection.defect.class().into(),
                    magnitude: trial.magnitude,
                    gate: outcome.gate.clone(),
                    counts: Counts::default(),
                    miss_rate: None,
                });
                rows.len() - 1
            });
            let c = &mut rows[index].counts;
            match outcome.state.as_str() {
                "detected" => c.detected += 1,
                "missed" => c.missed += 1,
                "excluded" => c.excluded += 1,
                "ineffective" => c.ineffective += 1,
                _ => c.unavailable += 1,
            }
        }
    }
    rows.sort_by(|a, b| {
        a.class
            .cmp(&b.class)
            .then(a.gate.cmp(&b.gate))
            .then(a.magnitude.total_cmp(&b.magnitude))
    });
    let mut minima: Vec<Minimum> = Vec::new();
    for row in &mut rows {
        row.miss_rate = row.counts.miss_rate();
        if !minima
            .iter()
            .any(|m| m.class == row.class && m.gate == row.gate)
        {
            minima.push(Minimum {
                class: row.class.clone(),
                gate: row.gate.clone(),
                smallest_detected_magnitude: None,
            });
        }
        if row.counts.detected > 0 {
            for m in &mut minima {
                if m.class == row.class
                    && m.gate == row.gate
                    && m.smallest_detected_magnitude.is_none()
                {
                    m.smallest_detected_magnitude = Some(row.magnitude);
                }
            }
        }
    }
    (rows, minima)
}

/// Bounded baseline discovery using compare's image/sidecar rules.
/// HDR candidates remain visible for explicit SDR rejection instead of disappearing from the set.
pub fn baseline_files(root: &std::path::Path) -> Result<Vec<std::path::PathBuf>> {
    // Validate bounded traversal and symlink policy before using the shared collector.
    crate::general::input::files(root, 64)?;
    let collected = crate::run::collect_images(root)?;
    if !collected.problems.is_empty() || collected.files.len() > 64 {
        return Err(Error::Config(
            "invalid or oversized sensitivity baseline set".into(),
        ));
    }
    Ok(collected.files.into_values().collect())
}

fn selected(config: &RunConfig, name: &str) -> Result<bool> {
    let matches = |globs: &[String]| -> Result<bool> {
        for glob in globs {
            if compile_glob(glob)?.is_match(name) {
                return Ok(true);
            }
        }
        Ok(false)
    };
    Ok(!matches(&config.ignore)? && (config.entries.is_empty() || matches(&config.entries)?))
}
/// Resolve original-name scope, metrics, overrides, masks and regions for a lossless trial pair.
pub fn config_for_entry(config: &RunConfig, name: &str) -> Result<Option<RunConfig>> {
    if !selected(config, name)? {
        return Ok(None);
    }
    let mut result = config.clone();
    (result.default_metric, result.default_threshold) = config.effective_for(name);
    result.overrides.clear();
    result.ignore.clear();
    result.entries.clear();
    result.regions.clear();
    result.masks.clear();
    for region in &config.regions {
        if region
            .glob
            .as_ref()
            .map(|g| compile_glob(g).map(|m| m.is_match(name)))
            .transpose()?
            .unwrap_or(true)
        {
            let mut r = region.clone();
            r.glob = None;
            result.regions.push(r);
        }
    }
    for mask in &config.masks {
        if mask
            .glob
            .as_ref()
            .map(|g| compile_glob(g).map(|m| m.is_match(name)))
            .transpose()?
            .unwrap_or(true)
        {
            let mut m = mask.clone();
            m.glob = None;
            result.masks.push(m);
        }
    }
    Ok(Some(result))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn operation(defect: Defect, magnitudes: Vec<f64>) -> Injection {
        Injection {
            id: "control".into(),
            defect,
            magnitudes,
        }
    }
    #[test]
    fn catalogue_rejects_invalid_strengths_locations_and_duplicate_ids() {
        let injection = operation(Defect::Misalignment, vec![1.0, 2.0]);
        let mut catalogue = Catalogue {
            schema: CATALOGUE_SCHEMA.into(),
            provenance: "procedural; MIT".into(),
            injections: vec![injection],
        };
        assert!(catalogue.validate().is_ok());
        for strengths in [
            vec![0.0],
            vec![1.5],
            vec![2.0, 1.0],
            vec![1.0, 1.0],
            vec![65.0],
            vec![f64::NAN],
        ] {
            catalogue.injections[0].magnitudes = strengths;
            assert!(catalogue.validate().is_err());
        }
        catalogue.injections[0] = operation(
            Defect::LocalEdit {
                rect: [0.9, 0.0, 0.2, 1.0],
                colour: [0; 3],
            },
            vec![1.0],
        );
        assert!(catalogue.validate().is_err());
        catalogue.injections[0] = operation(Defect::Blur, vec![1.0]);
        catalogue.injections.push(catalogue.injections[0].clone());
        assert!(catalogue.validate().is_err());
    }
    #[test]
    fn injections_preserve_source_and_retain_noops_and_glyph_controls() {
        let source = RgbaImage::from_pixel(16, 16, Rgba([100, 100, 100, 255]));
        let local = operation(
            Defect::LocalEdit {
                rect: [0.25, 0.25, 0.25, 0.25],
                colour: [200, 0, 100],
            },
            vec![0.5],
        );
        let (control, candidate, changed) = inject(&source, &local, 0.5, None).unwrap();
        assert_eq!(control, source);
        assert_eq!(changed, 16);
        assert_eq!(candidate.get_pixel(4, 4).0, [150, 50, 100, 255]);
        assert_eq!(source.get_pixel(4, 4).0, [100, 100, 100, 255]);
        let blur = operation(Defect::Blur, vec![1.0]);
        assert_eq!(inject(&source, &blur, 1.0, None).unwrap().2, 0);
        let glyph = operation(
            Defect::GlyphEdit {
                before: "a.png".into(),
                after: "b.png".into(),
                before_sha256: "0".repeat(64),
                after_sha256: "1".repeat(64),
                origin: [0.25, 0.25],
            },
            vec![1.0],
        );
        let pair = [
            RgbaImage::from_pixel(2, 2, Rgba([0, 0, 0, 255])),
            RgbaImage::from_pixel(2, 2, Rgba([255, 255, 255, 255])),
        ];
        let (control, candidate, changed) = inject(&source, &glyph, 1.0, Some(&pair)).unwrap();
        assert_eq!(control.get_pixel(4, 4).0, [0, 0, 0, 255]);
        assert_eq!(candidate.get_pixel(4, 4).0, [255; 4]);
        assert_eq!(changed, 4);
        assert!(inject(&source, &glyph, 1.0, None).is_err());
    }
    #[test]
    fn aggregates_pool_classes_and_exclude_absent_evidence_from_rates() {
        let catalogue = Catalogue {
            schema: CATALOGUE_SCHEMA.into(),
            provenance: "MIT".into(),
            injections: vec![operation(Defect::Blur, vec![1.0, 2.0])],
        };
        let trial = |state: &str, magnitude| Trial {
            entry: "a.png".into(),
            injection: "control".into(),
            magnitude,
            source_sha256: String::new(),
            images: Default::default(),
            image_sha256: Default::default(),
            changed_pixels: 1,
            outcomes: vec![Outcome {
                gate: "configured".into(),
                state: state.into(),
                reports: vec![],
                reasons: vec![],
            }],
        };
        let (rows, minima) = summarize(
            &catalogue,
            &[
                trial("missed", 1.0),
                trial("detected", 2.0),
                trial("missed", 2.0),
                trial("unavailable", 2.0),
                trial("ineffective", 2.0),
                trial("excluded", 2.0),
            ],
        );
        assert_eq!(rows[0].miss_rate, Some(1.0));
        assert_eq!(rows[1].miss_rate, Some(0.5));
        assert_eq!(minima[0].smallest_detected_magnitude, Some(2.0));
        assert_eq!(Counts::default().miss_rate(), None);
    }
    #[test]
    fn original_names_drive_override_scope_region_and_mask_matching() {
        let config = RunConfig::from_toml_str("threshold=0.02\nmetric='mean'\nignore=['skip/**']\n[[override]]\nglob='nested/*.jpg'\nthreshold=0.3\nmetric='max'\n[[mask]]\nglob='nested/*.jpg'\nrect=[0.0,0.0,0.1,0.1]\n[[region]]\nname='detail'\nglob='other/*.png'\nrect=[0.0,0.0,0.2,0.2]\n").unwrap();
        let mapped = config_for_entry(&config, "nested/a.jpg").unwrap().unwrap();
        assert_eq!(mapped.default_threshold, 0.3);
        assert_eq!(mapped.default_metric, crate::report::Metric::Max);
        assert_eq!(mapped.masks.len(), 1);
        assert!(mapped.masks[0].glob.is_none());
        assert!(mapped.regions.is_empty());
        assert!(config_for_entry(&config, "skip/a.png").unwrap().is_none());
    }
}
