//! Required occupancy, independent of pixel equality or perceptual thresholds.
use super::layers::Predicate;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Required effect contract version.
pub const SCHEMA: &str = "saccade-required-effect.v1";
/// Generic inclusion source, never an exclusion mask.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Selection {
    /// Source-bound named layer from the image-adjacent manifest.
    NamedLayer {
        /// Layer name.
        name: String,
        /// Native inclusion predicate.
        predicate: Predicate,
        /// Optional plain manifest filename; defaults to <filename>.layers.json.
        #[serde(default)]
        manifest: Option<String>,
    },
    /// White/nonzero image pixels; exact dimensions required.
    Mask {
        /// Path relative to policy directory.
        image: String,
    },
    /// Pixel boxes with exclusive far edges.
    Boxes {
        /// [x,y,width,height] boxes.
        boxes: Vec<[u32; 4]>,
    },
    /// Native side-specific layer.
    Layer {
        /// Layer file relative to each image's parent.
        image: String,
        /// Native predicate.
        predicate: Predicate,
    },
}
/// Which side must contain the required effect.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sides {
    /// Both captures (default).
    #[default]
    Both,
    /// Baseline only.
    Baseline,
    /// Candidate only.
    Candidate,
}
/// Opt-in occupancy gate for a named effect.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequiredEffect {
    /// Name recorded in the report.
    pub name: String,
    /// Relative entry glob; ** selects all.
    #[serde(default = "all")]
    pub glob: String,
    /// Inclusion source.
    pub selection: Selection,
    /// Minimum occupied pixels. Defaults to one to forbid vacuous proofs.
    #[serde(default = "one")]
    pub min_pixels: u64,
    /// Minimum fraction of the full image. Both minima must hold.
    #[serde(default)]
    pub min_fraction: f64,
    /// Side(s) on which to enforce occupancy.
    #[serde(default)]
    pub sides: Sides,
}
fn all() -> String {
    "**".into()
}
fn one() -> u64 {
    1
}
impl RequiredEffect {
    /// Check policy before comparison.
    pub fn validate(&self) -> Result<()> {
        crate::config::compile_glob(&self.glob)?;
        if self.name.is_empty()
            || !self.min_fraction.is_finite()
            || !(0.0..=1.0).contains(&self.min_fraction)
            || (self.min_pixels == 0 && self.min_fraction == 0.0)
        {
            return Err(Error::Config(
                "effect needs a name and positive coverage minimum".into(),
            ));
        }
        if let Selection::Layer { predicate, .. } | Selection::NamedLayer { predicate, .. } =
            &self.selection
        {
            predicate.validate()?;
        }
        Ok(())
    }
}
/// Typed non-vacuity failure retained even when FLIP is zero.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Failure {
    /// Effect occupancy is below the declared minimum.
    InsufficientEffectCoverage {
        /// Side being checked.
        side: String,
        /// Measured count.
        covered: u64,
        /// Required count after resolving the fractional minimum.
        required: u64,
    },
}
/// Statistics from the full-frame error map within a selected footprint.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    /// Pixels in this footprint.
    pub pixels: u64,
    /// Native RGBA pixels that differ.
    pub changed_pixels: u64,
    /// Mean FLIP; absent for an empty footprint.
    pub mean_flip: Option<f64>,
    /// Maximum FLIP; absent for an empty footprint.
    pub max_flip: Option<f64>,
}
/// Required effect evidence and its exact policy.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectResult {
    /// Contract discriminator.
    pub schema: String,
    /// Declared policy.
    pub policy: RequiredEffect,
    /// Occupied baseline pixels.
    pub baseline_pixels: u64,
    /// Occupied candidate pixels.
    pub candidate_pixels: u64,
    /// Baseline fraction.
    pub baseline_fraction: f64,
    /// Candidate fraction.
    pub candidate_fraction: f64,
    /// Union of effect footprints.
    pub inside: Stats,
    /// Complement of that union.
    pub outside: Stats,
    /// Explicit coverage failures.
    pub failures: Vec<Failure>,
}
/// Resolve an inclusion source, without resizing.
pub fn select(
    selection: &Selection,
    image: &Path,
    policy_root: &Path,
    dimensions: (u32, u32),
) -> Result<Vec<bool>> {
    let (w, h) = dimensions;
    let n = u64::from(w) * u64::from(h);
    if n == 0 || n > 16_777_216 {
        return Err(Error::Config("invalid effect dimensions".into()));
    }
    match selection {
        Selection::Mask { image } => {
            let path = super::relative(policy_root, image)?;
            let img = super::image(&path)?;
            if (img.width(), img.height()) != dimensions {
                return Err(Error::Config("effect mask dimensions differ".into()));
            }
            Ok(img.to_luma8().pixels().map(|p| p.0[0] > 0).collect())
        }
        Selection::Boxes { boxes } => {
            if boxes.len() > 4096 {
                return Err(Error::Config("too many effect boxes".into()));
            }
            let mut selected = vec![false; n as usize];
            for &[x, y, bw, bh] in boxes {
                if bw == 0
                    || bh == 0
                    || x.checked_add(bw).is_none_or(|v| v > w)
                    || y.checked_add(bh).is_none_or(|v| v > h)
                {
                    return Err(Error::Config("effect box lies outside the image".into()));
                }
                for yy in y..y + bh {
                    for xx in x..x + bw {
                        selected[(yy * w + xx) as usize] = true;
                    }
                }
            }
            Ok(selected)
        }
        Selection::NamedLayer {
            name,
            predicate,
            manifest,
        } => {
            let policy = super::layers::Policy {
                dump: None,
                manifest: manifest.clone(),
                scope: None,
                attribution: false,
            };
            let loaded = super::layers::load(image, &policy, dimensions)?;
            super::layers::selection(
                &loaded,
                &super::layers::LayerScope {
                    layer: name.clone(),
                    predicate: predicate.clone(),
                    mode: Default::default(),
                },
            )
        }
        Selection::Layer {
            image: name,
            predicate,
        } => {
            predicate.validate()?;
            let path = super::relative(image.parent().unwrap_or(Path::new(".")), name)?;
            Ok(super::layers::scalar(&path, dimensions)?
                .into_iter()
                .map(|v| predicate.matches(v))
                .collect())
        }
    }
}
/// Compute occupancy and independent inside/outside differences.
pub fn measure(
    policy: &RequiredEffect,
    b: &[bool],
    c: &[bool],
    base: &image::RgbaImage,
    candidate: &image::RgbaImage,
    errors: &[f32],
) -> Result<EffectResult> {
    policy.validate()?;
    let n = base.len() / 4;
    if n == 0
        || candidate.dimensions() != base.dimensions()
        || b.len() != n
        || c.len() != n
        || errors.len() != n
    {
        return Err(Error::Config("effect map dimensions differ".into()));
    }
    let baseline_pixels = b.iter().filter(|v| **v).count() as u64;
    let candidate_pixels = c.iter().filter(|v| **v).count() as u64;
    let required = policy
        .min_pixels
        .max((policy.min_fraction * n as f64).ceil() as u64);
    let mut failures = vec![];
    for (side, covered, check) in [
        (
            "baseline",
            baseline_pixels,
            policy.sides != Sides::Candidate,
        ),
        (
            "candidate",
            candidate_pixels,
            policy.sides != Sides::Baseline,
        ),
    ] {
        if check && covered < required {
            failures.push(Failure::InsufficientEffectCoverage {
                side: side.into(),
                covered,
                required,
            });
        }
    }
    let mut inside = Stats::default();
    let mut outside = Stats::default();
    for (i, (bp, cp)) in base.pixels().zip(candidate.pixels()).enumerate() {
        let stats = if b[i] || c[i] {
            &mut inside
        } else {
            &mut outside
        };
        stats.pixels += 1;
        stats.changed_pixels += u64::from(bp != cp);
        stats.mean_flip = Some(stats.mean_flip.unwrap_or(0.0) + f64::from(errors[i]));
        stats.max_flip = Some(stats.max_flip.unwrap_or(0.0).max(f64::from(errors[i])));
    }
    for stats in [&mut inside, &mut outside] {
        if let Some(sum) = stats.mean_flip {
            stats.mean_flip = Some(sum / stats.pixels as f64);
        }
    }
    Ok(EffectResult {
        schema: SCHEMA.into(),
        policy: policy.clone(),
        baseline_pixels,
        candidate_pixels,
        baseline_fraction: baseline_pixels as f64 / n as f64,
        candidate_fraction: candidate_pixels as f64 / n as f64,
        inside,
        outside,
        failures,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zero_mask_fails_at_zero_flip() {
        let p = RequiredEffect {
            name: "effect".into(),
            glob: "**".into(),
            selection: Selection::Boxes { boxes: vec![] },
            min_pixels: 1,
            min_fraction: 0.0,
            sides: Sides::Both,
        };
        let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([80, 80, 80, 255]));
        let r = measure(&p, &[false; 64], &[false; 64], &img, &img, &[0.0; 64]).unwrap();
        assert_eq!(r.failures.len(), 2);
        assert_eq!(r.outside.mean_flip, Some(0.0));
        assert_eq!(r.inside.mean_flip, None);
    }
    #[test]
    fn disappearance_and_side_minima_are_independent() {
        let mut p = RequiredEffect {
            name: "patch".into(),
            glob: "**".into(),
            selection: Selection::Boxes {
                boxes: vec![[0, 0, 4, 4]],
            },
            min_pixels: 4,
            min_fraction: 0.5,
            sides: Sides::Both,
        };
        let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([80, 80, 80, 255]));
        let r = measure(&p, &[true; 4], &[false; 4], &img, &img, &[0.0; 4]).unwrap();
        assert_eq!(r.failures.len(), 1);
        assert_eq!(r.inside.pixels, 4);
        p.sides = Sides::Baseline;
        assert!(
            measure(&p, &[true; 4], &[false; 4], &img, &img, &[0.0; 4])
                .unwrap()
                .failures
                .is_empty()
        );
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    #[test]
    fn compare_empty_effect_fails_with_measured_zero_error() {
        let tmp = tempfile::tempdir().unwrap();
        let b = tmp.path().join("b.png");
        let c = tmp.path().join("c.png");
        let mask = tmp.path().join("mask.png");
        image::RgbaImage::from_pixel(16, 16, image::Rgba([80, 80, 80, 255]))
            .save(&b)
            .unwrap();
        std::fs::copy(&b, &c).unwrap();
        image::GrayImage::new(16, 16).save(mask).unwrap();
        let cfg = crate::config::RunConfig {
            config_dir: Some(tmp.path().into()),
            required_effect: vec![RequiredEffect {
                name: "occupancy".into(),
                glob: "**".into(),
                selection: Selection::Mask {
                    image: "mask.png".into(),
                },
                min_pixels: 1,
                min_fraction: 0.0,
                sides: Sides::Both,
            }],
            ..Default::default()
        };
        let report = crate::run::run(&b, &c, &tmp.path().join("out"), &cfg).unwrap();
        assert!(report.is_regression());
        assert_eq!(report.entries[0].value, Some(0.0));
        assert_eq!(report.entries[0].status, crate::Status::Fail);
        assert_eq!(report.entries[0].required_effects[0].failures.len(), 2);
    }
}
