//! Offline reference alignment and tile differences relative to measured reference noise.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
/// Offline-reference artifact discriminator.
pub const SCHEMA: &str = "saccade-reference-evidence.v1";
/// Versioned noise/alignment policy.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
    /// Must be reference-1.
    pub version: String,
    /// Regular tile width/height.
    pub tile_size: u32,
    /// Fit one global render-to-reference exposure gain in linear RGB.
    pub fit_exposure: bool,
    /// Optional aces, hable or reinhard, applied identically to both sides before scoring.
    pub tonemap: Option<String>,
    /// Noise-floor multiplier.
    pub noise_k: f64,
    /// Absolute linear/mapped luminance tolerance above the noise floor.
    pub absolute_tolerance: f64,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            version: "reference-1".into(),
            tile_size: 32,
            fit_exposure: true,
            tonemap: None,
            noise_k: 3.0,
            absolute_tolerance: 0.001,
        }
    }
}
impl Policy {
    /// Validate the explicit policy.
    pub fn validate(&self) -> Result<()> {
        if self.version != "reference-1"
            || self.tile_size < 4
            || self.tile_size > 4096
            || !self.noise_k.is_finite()
            || self.noise_k <= 0.0
            || !self.absolute_tolerance.is_finite()
            || self.absolute_tolerance < 0.0
        {
            return Err(Error::Config("invalid reference policy".into()));
        }
        if let Some(name) = &self.tonemap {
            crate::hdr::Tonemapper::parse(name)?;
        }
        Ok(())
    }
}
/// Exposure fit and the declared common mapping.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Alignment {
    /// Scalar multiplying render linear RGB.
    pub exposure_scale: f64,
    /// log2(exposure_scale).
    pub exposure_stops: f64,
    /// Declared mapping; absent means raw linear RGB.
    pub tonemap: Option<String>,
    /// Fit algorithm identity.
    pub method: String,
}
/// Difference evidence for one noisy-reference tile.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    /// Pixel box.
    pub rect_px: [u32; 4],
    /// Included sample count.
    pub pixels: u64,
    /// Signed aligned render-minus-reference luminance.
    pub signed_mean: f64,
    /// Root mean squared luminance difference.
    pub rms_difference: f64,
    /// Standard deviation of the reference estimator.
    pub reference_noise_sigma: f64,
    /// Noise-derived RMS tolerance including absolute floor.
    pub rms_limit: f64,
    /// Noise-derived signed mean tolerance including absolute floor.
    pub mean_limit: f64,
    /// Either RMS or signed bias exceeds its recorded noise floor.
    pub beyond_noise: bool,
}
/// Offline reference result; source hashes are attached by the transport.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// Optional transport-validated arm identity and explicit exceptions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arm_validation: Option<crate::arms::Check>,
    /// Versioned discriminator.
    pub schema: String,
    /// Raw source identities filled by file transports.
    #[serde(default)]
    pub input_sha256: std::collections::BTreeMap<String, String>,
    /// Exact policy.
    pub policy: Policy,
    /// Fitted alignment parameters.
    pub alignment: Alignment,
    /// multiple_seeds, supplied_variance, or high_frequency_estimate.
    pub noise_method: String,
    /// Number of reference seeds averaged (1 for single-reference modes).
    pub reference_seeds: usize,
    /// within_noise_floor or reference_difference.
    pub verdict: String,
    /// Recorded tile differences and floors.
    pub tiles: Vec<Tile>,
    /// Mean aligned/display-mapped FLIP (secondary diagnostic).
    pub mean_flip: f64,
    /// Interpretation limits.
    pub limits: Vec<String>,
}
/// Reference noise source; the variance map uses linear luminance squared in native units.
pub enum Noise<'a> {
    /// Supplied per-pixel sample-mean variance, not population variance.
    Variance(&'a [f64]),
    /// Additional independent-seed references, each matching the reference geometry.
    Seeds(&'a [image::DynamicImage]),
    /// Robust local high-frequency estimate; contains scene-detail contamination.
    Estimate,
}
fn linear_image(img: &image::DynamicImage) -> Result<Vec<[f64; 3]>> {
    if img.width() == 0
        || img.height() == 0
        || u64::from(img.width()) * u64::from(img.height()) > 16_777_216
    {
        return Err(Error::Config("invalid reference geometry".into()));
    }
    let floating = matches!(
        img.color(),
        image::ColorType::Rgb32F | image::ColorType::Rgba32F
    );
    let rgba = img.to_rgba32f();
    let out: Vec<_> = rgba
        .pixels()
        .map(|p| {
            std::array::from_fn(|ch| {
                let v = f64::from(p[ch]);
                (if floating { v } else { crate::color::linear(v) }) * f64::from(p[3])
            })
        })
        .collect();
    if out.iter().flatten().any(|v| !v.is_finite() || *v < 0.0) {
        return Err(Error::Config(
            "reference/render has nonfinite or negative radiance".into(),
        ));
    }
    Ok(out)
}
fn mapped(values: &[[f64; 3]], tonemap: Option<&str>) -> Result<Vec<[f64; 3]>> {
    let mapper = tonemap.map(crate::hdr::Tonemapper::parse).transpose()?;
    let out: Vec<_> = values
        .iter()
        .map(|rgb| {
            if let Some(tm) = mapper {
                crate::hdr::map_colour(rgb.map(|v| v as f32), tm).map(f64::from)
            } else {
                *rgb
            }
        })
        .collect();
    if out.iter().flatten().any(|v| !v.is_finite() || *v < 0.0) {
        return Err(Error::Config(
            "mapped reference radiance is nonfinite or negative".into(),
        ));
    }
    Ok(out)
}
fn display(values: &[[f64; 3]], w: u32, h: u32) -> image::RgbImage {
    image::RgbImage::from_fn(w, h, |x, y| {
        image::Rgb(values[(y * w + x) as usize].map(|v| {
            let v = v.clamp(0.0, 1.0);
            let s = if v <= 0.0031308 {
                12.92 * v
            } else {
                1.055 * v.powf(1.0 / 2.4) - 0.055
            };
            (s * 255.0).round() as u8
        }))
    })
}
/// Fit exposure and compare each tile against reference-estimator noise.
/// A supplied exclusion bitmap restricts fitting, differences and deciding FLIP.
pub fn compare(
    render: &image::DynamicImage,
    reference: &image::DynamicImage,
    noise: Noise<'_>,
    excluded: Option<&[bool]>,
    policy: &Policy,
    opts: &crate::compare::CompareOptions,
) -> Result<Report> {
    policy.validate()?;
    let (w, h) = (reference.width(), reference.height());
    let n = (u64::from(w) * u64::from(h)) as usize;
    if (render.width(), render.height()) != (w, h) || excluded.is_some_and(|m| m.len() != n) {
        return Err(Error::Config(
            "reference pair/scope dimensions differ".into(),
        ));
    }
    let included = |i: usize| !excluded.is_some_and(|m| m[i]);
    if !(0..n).any(included) {
        return Err(Error::Config("reference scope is empty".into()));
    }
    let mut r = linear_image(render)?;
    let mut seed_raw = vec![linear_image(reference)?];
    let supplied = match noise {
        Noise::Variance(v) => {
            if v.len() != n || v.iter().any(|v| !v.is_finite() || *v < 0.0) {
                return Err(Error::Config("reference variance map invalid".into()));
            }
            Some(v)
        }
        Noise::Seeds(seeds) => {
            if seeds.is_empty()
                || seeds.len() > 31
                || n.saturating_mul(seeds.len() + 1) > 16_777_216
            {
                return Err(Error::Config(
                    "reference seed allocation/count limit exceeded".into(),
                ));
            }
            for seed in seeds {
                if (seed.width(), seed.height()) != (w, h) {
                    return Err(Error::Config("reference seed geometry differs".into()));
                }
                seed_raw.push(linear_image(seed)?);
            }
            None
        }
        Noise::Estimate => None,
    };
    let raw_mean: Vec<[f64; 3]> = (0..n)
        .map(|i| {
            std::array::from_fn(|ch| {
                seed_raw.iter().map(|s| s[i][ch]).sum::<f64>() / seed_raw.len() as f64
            })
        })
        .collect();
    let mut xy = 0.0;
    let mut xx = 0.0;
    for i in 0..n {
        if !included(i) {
            continue;
        }
        for ch in 0..3 {
            xy += r[i][ch] * raw_mean[i][ch];
            xx += r[i][ch].powi(2);
        }
    }
    let gain = if policy.fit_exposure {
        if xx <= 1e-20 {
            return Err(Error::Config(
                "render has insufficient energy for exposure alignment".into(),
            ));
        }
        let gain = xy / xx;
        if !gain.is_finite() || !(0.001..=1000.0).contains(&gain) {
            return Err(Error::Config("reference exposure fit out of range".into()));
        }
        gain
    } else {
        1.0
    };
    for pixel in &mut r {
        for v in pixel {
            *v *= gain;
        }
    }
    let rm = mapped(&r, policy.tonemap.as_deref())?;
    let seeds: Vec<_> = seed_raw
        .iter()
        .map(|s| mapped(s, policy.tonemap.as_deref()))
        .collect::<Result<_>>()?;
    let mean: Vec<[f64; 3]> = (0..n)
        .map(|i| {
            std::array::from_fn(|ch| {
                seeds.iter().map(|s| s[i][ch]).sum::<f64>() / seeds.len() as f64
            })
        })
        .collect();
    let rl: Vec<_> = rm.iter().copied().map(super::spatial::luminance).collect();
    let ml: Vec<_> = mean
        .iter()
        .copied()
        .map(super::spatial::luminance)
        .collect();
    let seed_luma: Vec<Vec<_>> = seeds
        .iter()
        .map(|s| s.iter().copied().map(super::spatial::luminance).collect())
        .collect();
    let mut variance = vec![0.0; n];
    let method = if seeds.len() > 1 {
        for i in 0..n {
            variance[i] = seed_luma
                .iter()
                .map(|s| (s[i] - ml[i]).powi(2))
                .sum::<f64>()
                / ((seeds.len() - 1) * seeds.len()) as f64;
        }
        "multiple_seeds"
    } else if let Some(v) = supplied {
        if policy.tonemap.is_none() {
            variance.copy_from_slice(v);
        } else {
            for i in 0..n {
                let sigma = v[i].sqrt();
                let low = raw_mean[i].map(|v| (v - sigma).max(0.0));
                let high = raw_mean[i].map(|v| v + sigma);
                let mapped = mapped(&[low, high], policy.tonemap.as_deref())?;
                variance[i] = ((super::spatial::luminance(mapped[1])
                    - super::spatial::luminance(mapped[0]))
                    / 2.0)
                    .powi(2);
            }
        }
        "supplied_variance"
    } else {
        "high_frequency_estimate"
    };
    let mut tiles = Vec::new();
    for y in (0..h).step_by(policy.tile_size as usize) {
        for x in (0..w).step_by(policy.tile_size as usize) {
            let rect = [
                x,
                y,
                policy.tile_size.min(w - x),
                policy.tile_size.min(h - y),
            ];
            let indexes: Vec<_> = (y..y + rect[3])
                .flat_map(|yy| (x..x + rect[2]).map(move |xx| (yy * w + xx) as usize))
                .filter(|i| included(*i))
                .collect();
            if indexes.is_empty() {
                continue;
            }
            let count = indexes.len() as f64;
            let sigma = if method == "high_frequency_estimate" {
                let mut high: Vec<_> = indexes
                    .iter()
                    .filter_map(|i| {
                        let xx = *i as u32 % w;
                        if xx > 0 && xx + 1 < w && included(*i - 1) && included(*i + 1) {
                            Some((ml[*i] - (ml[*i - 1] + ml[*i + 1]) / 2.0).abs())
                        } else {
                            None
                        }
                    })
                    .collect();
                if high.is_empty() {
                    0.0
                } else {
                    high.sort_by(f64::total_cmp);
                    high[high.len() / 2] / (0.67448975 * 1.5f64.sqrt())
                }
            } else {
                (indexes.iter().map(|i| variance[*i]).sum::<f64>() / count).sqrt()
            };
            let signed = indexes.iter().map(|i| rl[*i] - ml[*i]).sum::<f64>() / count;
            let rms = (indexes
                .iter()
                .map(|i| (rl[*i] - ml[*i]).powi(2))
                .sum::<f64>()
                / count)
                .sqrt();
            let rms_limit = policy.noise_k * sigma + policy.absolute_tolerance;
            let mean_limit = policy.noise_k * sigma / count.sqrt() + policy.absolute_tolerance;
            tiles.push(Tile {
                rect_px: rect,
                pixels: indexes.len() as u64,
                signed_mean: signed,
                rms_difference: rms,
                reference_noise_sigma: sigma,
                rms_limit,
                mean_limit,
                beyond_noise: rms > rms_limit || signed.abs() > mean_limit,
            });
        }
    }
    let cmp = crate::compare::compare(&display(&rm, w, h), &display(&mean, w, h), opts)?;
    let mean_flip = crate::compare::masked_metrics(&cmp.error_map, excluded, w, h, [0, 0, w, h])
        .ok_or_else(|| Error::Config("empty reference scope".into()))?
        .mean;
    let verdict = if tiles.iter().any(|t| t.beyond_noise) {
        "reference_difference"
    } else {
        "within_noise_floor"
    };
    Ok(Report {arm_validation:None,schema:SCHEMA.into(),input_sha256:Default::default(),policy:policy.clone(),alignment:Alignment {exposure_scale:gain,exposure_stops:gain.log2(),tonemap:policy.tonemap.clone(),method:if policy.fit_exposure {"linear_rgb_global_least_squares"} else {"declared_unity"}.into()},noise_method:method.into(),reference_seeds:seeds.len(),verdict:verdict.into(),tiles,mean_flip,limits:vec!["Exposure alignment can hide a global gain error; fitted parameters remain explicit and fit_exposure=false measures it directly.".into(),"Provided variance is the variance of the reference estimator in linear luminance squared; nonlinear mapping uses a symmetric local propagation approximation.".into(),"Independent-seed variance is divided by seed count for the averaged reference. Single-frame high-frequency estimates contain scene detail; tile bias limits assume independent pixels and are diagnostic under correlation.".into(),"FLIP is a secondary clipped display diagnostic; the deciding noise floors use unquantized linear or declared mapped luminance.".into()]})
}
#[cfg(test)]
mod tests {
    use super::*;
    fn frame(value: f32) -> image::DynamicImage {
        image::DynamicImage::ImageRgb32F(image::Rgb32FImage::from_pixel(
            64,
            64,
            image::Rgb([value; 3]),
        ))
    }
    #[test]
    fn global_exposure_is_fitted_and_recorded() {
        let opts = crate::compare::CompareOptions::default();
        let r = compare(
            &frame(0.2),
            &frame(0.4),
            Noise::Variance(&[0.0; 4096]),
            None,
            &Policy::default(),
            &opts,
        )
        .unwrap();
        assert!((r.alignment.exposure_scale - 2.0).abs() < 1e-6);
        assert_eq!(r.verdict, "within_noise_floor");
        let r = compare(
            &frame(0.2),
            &frame(0.4),
            Noise::Variance(&[0.0; 4096]),
            None,
            &Policy {
                fit_exposure: false,
                ..Default::default()
            },
            &opts,
        )
        .unwrap();
        assert_eq!(r.verdict, "reference_difference");
    }
    #[test]
    fn independent_seed_mean_tolerates_noise_but_not_regional_bias() {
        let reference =
            image::DynamicImage::ImageRgb32F(image::Rgb32FImage::from_fn(64, 64, |x, y| {
                image::Rgb([if (x + y) % 2 == 0 { 0.19 } else { 0.21 }; 3])
            }));
        let opposite =
            image::DynamicImage::ImageRgb32F(image::Rgb32FImage::from_fn(64, 64, |x, y| {
                image::Rgb([if (x + y) % 2 == 0 { 0.21 } else { 0.19 }; 3])
            }));
        let policy = Policy {
            fit_exposure: false,
            tonemap: Some("reinhard".into()),
            ..Default::default()
        };
        let opts = crate::compare::CompareOptions::default();
        assert_eq!(
            compare(
                &frame(0.2),
                &reference,
                Noise::Seeds(std::slice::from_ref(&opposite)),
                None,
                &policy,
                &opts
            )
            .unwrap()
            .verdict,
            "within_noise_floor"
        );
        let biased =
            image::DynamicImage::ImageRgb32F(image::Rgb32FImage::from_fn(64, 64, |x, _| {
                image::Rgb([if x < 32 { 0.24 } else { 0.2 }; 3])
            }));
        let report = compare(
            &biased,
            &reference,
            Noise::Seeds(&[opposite]),
            None,
            &policy,
            &opts,
        )
        .unwrap();
        assert_eq!(report.verdict, "reference_difference");
        assert!(
            report
                .tiles
                .iter()
                .filter(|t| t.beyond_noise)
                .all(|t| t.rect_px[0] == 0)
        );
    }
}
