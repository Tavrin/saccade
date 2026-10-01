//! HDR (`.exr`, `.hdr`) support: decoding to linear `f32` RGB and HDR-FLIP.
//!
//! HDR-FLIP evaluates LDR-FLIP at a range of exposures and keeps, per pixel,
//! the largest error. The exposure range is derived from the reference
//! image's luminance, following the procedure of NVIDIA's reference
//! implementation (see `THIRD_PARTY.md`).
//!
//! **Approximation.** The reference implementation keeps every exposure in
//! floating point. Here each exposure is tone-mapped, clamped, encoded to
//! sRGB and quantised to 8 bits before it enters the existing LDR-FLIP path,
//! so very small differences in dark or saturated regions are quantised away.

use std::path::Path;

use serde::Deserialize;

use crate::compare::{CompareOptions, Comparison, compare, metrics_of};
use crate::error::{Error, Result};
use crate::report::{HdrInfo, Properties};

/// Largest linear value kept after decoding and after the exposure gain.
/// Larger values (and infinities) are clamped so the tone mappers' `x * x`
/// stays within `f32` (`1e15 ^ 2 = 1e30 < f32::MAX`).
const MAX_LINEAR: f32 = 1.0e15;
/// Upper bound on automatically chosen exposure counts.
const MAX_AUTO_EXPOSURES: u32 = 64;
/// Output level of the tone mapper that defines the top of the exposure range.
const RANGE_TARGET: f32 = 0.85;

/// Tone mapper applied to each exposure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tonemapper {
    /// Narkowicz ACES approximation (default).
    #[default]
    Aces,
    /// Hable (Uncharted 2) filmic curve.
    Hable,
    /// Luminance-based Reinhard.
    Reinhard,
}

impl Tonemapper {
    /// The name used in reports, config files and flags.
    pub fn name(self) -> &'static str {
        match self {
            Self::Aces => "aces",
            Self::Hable => "hable",
            Self::Reinhard => "reinhard",
        }
    }

    /// Parses `aces`, `hable` or `reinhard` (case-insensitive).
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "aces" => Ok(Self::Aces),
            "hable" => Ok(Self::Hable),
            "reinhard" => Ok(Self::Reinhard),
            other => Err(Error::Config(format!(
                "unknown tonemapper {other:?} (expected aces, hable or reinhard)"
            ))),
        }
    }

    /// Rational-curve coefficients `[a0, a1, a2, b0, b1, b2]` such that the
    /// curve is `(a0 x^2 + a1 x + a2) / (b0 x^2 + b1 x + b2)`. Reinhard is
    /// luminance-based and listed here only for the exposure-range solve.
    fn coefficients(self) -> [f32; 6] {
        match self {
            Self::Reinhard => [0.0, 1.0, 0.0, 0.0, 1.0, 1.0],
            // 0.6 cancels the usual ACES pre-exposure.
            Self::Aces => [
                0.6 * 0.6 * 2.51,
                0.6 * 0.03,
                0.0,
                0.6 * 0.6 * 2.43,
                0.6 * 0.59,
                0.14,
            ],
            Self::Hable => [0.231_683, 0.013_791, 0.0, 0.18, 0.3, 0.018],
        }
    }
}

/// HDR-FLIP settings (`[hdr]` in `flipdiff.toml`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct HdrConfig {
    /// Tone mapper.
    pub tonemapper: Tonemapper,
    /// First exposure in stops; `None` computes it from the reference.
    pub start_exposure: Option<f32>,
    /// Last exposure in stops; `None` computes it from the reference.
    pub stop_exposure: Option<f32>,
    /// Number of exposures; `None` derives it from the range.
    pub num_exposures: Option<u32>,
}

impl HdrConfig {
    /// Checks that given exposures are finite, ordered, and the count is positive.
    pub fn validate(&self) -> Result<()> {
        for (what, v) in [("start", self.start_exposure), ("stop", self.stop_exposure)] {
            if v.is_some_and(|v| !v.is_finite()) {
                return Err(Error::Config(format!("hdr {what}_exposure must be finite")));
            }
        }
        if let (Some(a), Some(b)) = (self.start_exposure, self.stop_exposure) {
            if a > b {
                return Err(Error::Config(format!(
                    "hdr start_exposure ({a}) must not exceed stop_exposure ({b})"
                )));
            }
        }
        if self.num_exposures == Some(0) {
            return Err(Error::Config("hdr num_exposures must be at least 1".into()));
        }
        Ok(())
    }

    /// Parses the CLI form `START:STOP:N` (stops, stops, count).
    pub fn parse_exposures(&mut self, s: &str) -> Result<()> {
        let bad = || Error::Config(format!("--hdr-exposures expects START:STOP:N, got {s:?}"));
        let mut parts = s.split(':');
        let (Some(a), Some(b), Some(n), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(bad());
        };
        self.start_exposure = Some(a.trim().parse().map_err(|_| bad())?);
        self.stop_exposure = Some(b.trim().parse().map_err(|_| bad())?);
        self.num_exposures = Some(n.trim().parse().map_err(|_| bad())?);
        self.validate()
    }
}

/// A decoded linear-light RGB image, tightly packed `f32`.
#[derive(Debug, Clone, PartialEq)]
pub struct HdrImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 3` linear values, row-major.
    pub data: Vec<f32>,
    /// What decoding replaced: the NaN, infinite and negative samples of the
    /// file (`data` itself is already cleaned).
    pub replaced: ReplacedSamples,
}

/// Counts of samples [`decode_hdr`] had to replace to keep `data` finite.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReplacedSamples {
    /// NaN samples (became 0).
    pub nan: u64,
    /// Infinite samples (became the clamp maximum or 0).
    pub inf: u64,
    /// Finite samples below zero (became 0).
    pub negative: u64,
}

/// Whether `path` has an HDR extension (`exr` or `hdr`, case-insensitive).
pub fn is_hdr_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("exr") || e.eq_ignore_ascii_case("hdr"))
}

/// Decodes an `.exr` or `.hdr` file to linear `f32` RGB. Alpha is dropped;
/// NaN and negative values become 0 and values are clamped to a large finite
/// maximum, so later arithmetic stays finite.
pub fn decode_hdr(path: &Path) -> Result<HdrImage> {
    let img = image::open(path).map_err(|source| Error::Decode {
        path: path.to_path_buf(),
        source,
    })?;
    let rgb = img.to_rgb32f();
    let (width, height) = rgb.dimensions();
    let mut replaced = ReplacedSamples::default();
    let data = rgb
        .into_raw()
        .into_iter()
        .map(|v| {
            if v.is_nan() {
                replaced.nan += 1;
                0.0
            } else {
                if v.is_infinite() {
                    replaced.inf += 1;
                } else if v < 0.0 {
                    replaced.negative += 1;
                }
                v.clamp(0.0, MAX_LINEAR)
            }
        })
        .collect();
    Ok(HdrImage {
        width,
        height,
        data,
        replaced,
    })
}

/// Errors with [`Error::HdrMismatch`] when exactly one of the two paths is HDR.
pub fn check_same_kind(baseline: &Path, capture: &Path) -> Result<()> {
    if is_hdr_path(baseline) == is_hdr_path(capture) {
        return Ok(());
    }
    let kind = |p: &Path| if is_hdr_path(p) { "HDR" } else { "LDR" };
    Err(Error::HdrMismatch(format!(
        "baseline is {}, capture is {}",
        kind(baseline),
        kind(capture)
    )))
}

fn luminance(px: &[f32]) -> f32 {
    match px {
        [r, g, b] => 0.2126 * r + 0.7152 * g + 0.0722 * b,
        _ => 0.0,
    }
}

/// Smallest and largest root of `a x^2 + b x + c = 0` (equal when linear).
fn solve_second_degree(a: f32, b: f32, c: f32) -> (f32, f32) {
    if a == 0.0 {
        let x = -c / b;
        return (x, x);
    }
    let d1 = -0.5 * (b / a);
    let d2 = ((d1 * d1) - (c / a)).sqrt();
    (d1 - d2, d1 + d2)
}

/// Exposure range `(start, stop)` in stops for a reference image, following
/// the reference HDR-FLIP: `start` makes the brightest pixel reach the tone
/// mapper's 0.85 output level, `stop` does the same for the median luminance.
///
/// Degenerate references: when the median luminance is 0, the median of the
/// non-zero luminances is used instead (the reference would divide by zero);
/// an all-black image gives `(0, 0)`.
pub fn auto_exposure_range(reference: &HdrImage, tm: Tonemapper) -> (f32, f32) {
    let tc = tm.coefficients();
    let a = tc[0] - RANGE_TARGET * tc[3];
    let b = tc[1] - RANGE_TARGET * tc[4];
    let c = tc[2] - RANGE_TARGET * tc[5];
    let (_, x_max) = solve_second_degree(a, b, c);

    let mut lum: Vec<f32> = reference.data.chunks_exact(3).map(luminance).collect();
    lum.sort_unstable_by(f32::total_cmp);
    let y_max = lum.last().copied().unwrap_or(0.0);
    if y_max <= 0.0 || !x_max.is_finite() {
        return (0.0, 0.0);
    }
    let median_of = |v: &[f32]| -> f32 {
        let n = v.len();
        match (n, v.get(n / 2)) {
            (0, _) | (_, None) => 0.0,
            (1, Some(&m)) => m,
            (_, Some(&hi)) => (v.get(n / 2 - 1).copied().unwrap_or(hi) + hi) * 0.5,
        }
    };
    let mut y_median = median_of(&lum);
    if y_median <= 0.0 {
        let first_nonzero = lum.partition_point(|&v| v <= 0.0);
        y_median = median_of(lum.get(first_nonzero..).unwrap_or(&[]));
    }
    let start = (x_max / y_max).log2();
    let stop = (x_max / y_median.max(f32::MIN_POSITIVE)).log2();
    (start, stop.max(start))
}

/// The exposures that will be evaluated and how they were chosen.
#[derive(Debug, Clone, PartialEq)]
pub struct ExposurePlan {
    /// First exposure in stops.
    pub start: f32,
    /// Last exposure in stops.
    pub stop: f32,
    /// Number of exposures (evenly spaced, inclusive of both ends).
    pub count: u32,
    /// True when neither end was given.
    pub auto_range: bool,
}

impl ExposurePlan {
    /// Plans the exposures for `reference` under `cfg`. A missing end of the
    /// range is computed from the reference; a missing count is
    /// `max(2, ceil(stop - start))` (1 when the range is empty), capped at 64.
    pub fn new(reference: &HdrImage, cfg: &HdrConfig) -> Self {
        let (auto_start, auto_stop) = if cfg.start_exposure.is_some() && cfg.stop_exposure.is_some()
        {
            (0.0, 0.0)
        } else {
            auto_exposure_range(reference, cfg.tonemapper)
        };
        let start = cfg.start_exposure.unwrap_or(auto_start);
        let stop = cfg.stop_exposure.unwrap_or(auto_stop).max(start);
        let count = cfg.num_exposures.unwrap_or_else(|| {
            if stop > start {
                ((stop - start).ceil() as u32).clamp(2, MAX_AUTO_EXPOSURES)
            } else {
                1
            }
        });
        Self {
            start,
            stop,
            count,
            auto_range: cfg.start_exposure.is_none() && cfg.stop_exposure.is_none(),
        }
    }

    fn exposures(&self) -> impl Iterator<Item = f32> + '_ {
        let step = if self.count > 1 {
            (self.stop - self.start) / (self.count - 1) as f32
        } else {
            0.0
        };
        (0..self.count).map(move |i| self.start + step * i as f32)
    }

    /// The report record of this plan.
    pub fn info(&self, tm: Tonemapper) -> HdrInfo {
        HdrInfo {
            tonemapper: tm.name().to_string(),
            start_exposure: self.start,
            stop_exposure: self.stop,
            num_exposures: self.count,
            auto_range: self.auto_range,
        }
    }
}

fn tonemap_px(c: [f32; 3], tm: Tonemapper) -> [f32; 3] {
    if tm == Tonemapper::Reinhard {
        let f = 1.0 / (1.0 + luminance(&c));
        return [c[0] * f, c[1] * f, c[2] * f];
    }
    let t = tm.coefficients();
    c.map(|x| {
        let x2 = x * x;
        (x2 * t[0] + x * t[1] + t[2]) / (x2 * t[3] + x * t[4] + t[5])
    })
}

fn srgb_byte(linear: f32) -> u8 {
    let v = if linear.is_nan() {
        0.0
    } else {
        linear.clamp(0.0, 1.0)
    };
    let e = if v <= 0.003_130_8 {
        12.92 * v
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    (e * 255.0 + 0.5) as u8
}

/// Applies `2^stops`, the tone mapper, a clamp and the sRGB transfer function.
pub fn tonemap_to_srgb8(img: &HdrImage, tm: Tonemapper, stops: f32) -> image::RgbImage {
    let gain = stops.exp2();
    let mut out = Vec::with_capacity(img.data.len());
    for px in img.data.chunks_exact(3) {
        if let [r, g, b] = px {
            let exposed = [r * gain, g * gain, b * gain]
                .map(|v| if v.is_nan() { 0.0 } else { v.min(MAX_LINEAR) });
            let mapped = tonemap_px(exposed, tm);
            out.extend(mapped.map(srgb_byte));
        }
    }
    image::RgbImage::from_raw(img.width, img.height, out)
        .unwrap_or_else(|| image::RgbImage::new(img.width, img.height))
}

/// The display PNG content: tone-mapped at exposure 0 with `tm`.
pub fn display_image(img: &HdrImage, tm: Tonemapper) -> image::RgbImage {
    tonemap_to_srgb8(img, tm, 0.0)
}

/// Structural checks on linear luminance (Rec. 709 weights, no transfer
/// function). `is_all_white` means every channel is at least 1.0.
pub fn validate_hdr(img: &HdrImage) -> Properties {
    let count = img.data.len() / 3;
    if count == 0 {
        return Properties {
            is_all_black: false,
            is_all_white: false,
            mean_luminance: 0.0,
            min_luminance: 0.0,
            max_luminance: 0.0,
            nan_count: 0,
            inf_count: 0,
            negative_count: 0,
        };
    }
    let black = img.data.iter().all(|&v| v == 0.0);
    let white = img.data.iter().all(|&v| v >= 1.0);
    // Decoding already replaced these samples, so the file's counts are used.
    let ReplacedSamples {
        nan: nan_count,
        inf: inf_count,
        negative: negative_count,
    } = img.replaced;
    // Luminance statistics cover finite pixels only, so a NaN or infinite
    // sample cannot turn them into values JSON cannot carry.
    let (mut sum, mut min, mut max, mut finite) = (0.0f64, f32::INFINITY, f32::NEG_INFINITY, 0u64);
    for px in img.data.chunks_exact(3) {
        let l = luminance(px);
        if !l.is_finite() {
            continue;
        }
        sum += f64::from(l);
        min = min.min(l);
        max = max.max(l);
        finite += 1;
    }
    if finite == 0 {
        (min, max) = (0.0, 0.0);
    }
    Properties {
        is_all_black: black,
        is_all_white: white,
        mean_luminance: if finite == 0 {
            0.0
        } else {
            (sum / finite as f64) as f32
        },
        min_luminance: min,
        max_luminance: max,
        nan_count,
        inf_count,
        negative_count,
    }
}

/// HDR-FLIP of `capture` against `baseline` (the reference): the exposure
/// range comes from the baseline unless `opts.hdr` gives it, each exposure is
/// tone-mapped to 8-bit sRGB and compared with LDR-FLIP, and the per-pixel
/// maximum over exposures is the error map.
///
/// Errors as [`compare`] does (dimension mismatch, empty image, bad ppd).
pub fn compare_hdr(
    capture: &HdrImage,
    baseline: &HdrImage,
    opts: &CompareOptions,
) -> Result<(Comparison, HdrInfo)> {
    let cfg = &opts.hdr;
    cfg.validate()?;
    let plan = ExposurePlan::new(baseline, cfg);
    let mut error_map: Vec<f32> = Vec::new();
    for stops in plan.exposures() {
        let cmp = compare(
            &tonemap_to_srgb8(capture, cfg.tonemapper, stops),
            &tonemap_to_srgb8(baseline, cfg.tonemapper, stops),
            opts,
        )?;
        if error_map.is_empty() {
            error_map = cmp.error_map;
        } else {
            for (acc, v) in error_map.iter_mut().zip(&cmp.error_map) {
                if *v > *acc || v.is_nan() {
                    *acc = *v;
                }
            }
        }
    }
    let metrics = metrics_of(&error_map, capture.width, capture.height);
    Ok((Comparison { metrics, error_map }, plan.info(cfg.tonemapper)))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    const N: u32 = 128;

    /// A scene with a smooth mid-tone gradient and a bright patch of `peak`.
    fn scene(peak: f32) -> HdrImage {
        let mut data = Vec::new();
        for y in 0..N {
            for x in 0..N {
                let in_patch = (32..96).contains(&x) && (32..96).contains(&y);
                let base = 0.05 + 0.3 * (x as f32 / N as f32);
                let v = if in_patch { peak } else { base };
                data.extend([v, v * 0.9, v * 0.8]);
            }
        }
        HdrImage {
            width: N,
            height: N,
            data,
            replaced: ReplacedSamples::default(),
        }
    }

    fn write_exr(dir: &Path, name: &str, img: &HdrImage) -> std::path::PathBuf {
        let buf = image::Rgb32FImage::from_raw(img.width, img.height, img.data.clone())
            .expect("buffer size");
        let path = dir.join(name);
        buf.save(&path).expect("write exr");
        path
    }

    #[test]
    fn identical_pair_scores_zero_and_exr_round_trips() {
        let dir = tempfile::tempdir().expect("tempdir");
        let img = scene(8.0);
        let path = write_exr(dir.path(), "a.exr", &img);
        let back = decode_hdr(&path).expect("decode");
        assert_eq!((back.width, back.height), (N, N));
        let (cmp, info) = compare_hdr(&back, &back, &CompareOptions::default()).expect("cmp");
        assert_eq!(cmp.metrics.max, 0.0);
        assert_eq!(cmp.metrics.mean, 0.0);
        assert!(info.auto_range && info.num_exposures >= 2);
    }

    #[test]
    fn one_stop_highlight_change_is_seen_by_hdr_flip_but_not_by_a_single_ldr_exposure() {
        let (base, cap) = (scene(32.0), scene(64.0));
        let opts = CompareOptions::default();
        let (hdr, _) = compare_hdr(&cap, &base, &opts).expect("hdr");
        let ldr = compare(
            &display_image(&cap, Tonemapper::Aces),
            &display_image(&base, Tonemapper::Aces),
            &opts,
        )
        .expect("ldr");
        eprintln!(
            "HDR-FLIP mean {:.5} max {:.5}; tone-mapped-only mean {:.5} max {:.5}",
            hdr.metrics.mean, hdr.metrics.max, ldr.metrics.mean, ldr.metrics.max
        );
        assert!(hdr.metrics.mean > 0.01, "hdr mean {}", hdr.metrics.mean);
        assert!(
            hdr.metrics.mean > 5.0 * ldr.metrics.mean,
            "hdr {} vs ldr {}",
            hdr.metrics.mean,
            ldr.metrics.mean
        );
    }

    #[test]
    fn mixed_ldr_hdr_pair_is_an_error() {
        let err = check_same_kind(Path::new("a/x.exr"), Path::new("b/x.png")).expect_err("mixed");
        assert!(err.to_string().contains("baseline is HDR, capture is LDR"));
        assert!(check_same_kind(Path::new("x.EXR"), Path::new("y.hdr")).is_ok());
    }

    #[test]
    fn auto_range_comes_from_the_reference() {
        let base = scene(8.0);
        let (start, stop) = auto_exposure_range(&base, Tonemapper::Aces);
        // x_max solves ACES(x) = 0.85; the peak luminance is 8 * (0.2126 + 0.9 * 0.7152 + 0.8 * 0.0722).
        let peak = 8.0 * (0.2126 + 0.9 * 0.7152 + 0.8 * 0.0722);
        let (_, x_max) = solve_second_degree(
            Tonemapper::Aces.coefficients()[0] - 0.85 * Tonemapper::Aces.coefficients()[3],
            Tonemapper::Aces.coefficients()[1] - 0.85 * Tonemapper::Aces.coefficients()[4],
            -0.85 * Tonemapper::Aces.coefficients()[5],
        );
        assert!(
            (start - (x_max / peak).log2()).abs() < 1e-4,
            "start {start}"
        );
        assert!(start < 0.0 && stop > start);
        // A brighter reference moves the range down; a given range is kept as is.
        let (start2, _) = auto_exposure_range(&scene(16.0), Tonemapper::Aces);
        assert!((start2 - (start - 1.0)).abs() < 1e-4);
        let given = HdrConfig {
            start_exposure: Some(-2.0),
            stop_exposure: Some(1.0),
            num_exposures: Some(4),
            ..HdrConfig::default()
        };
        let plan = ExposurePlan::new(&base, &given);
        assert_eq!(
            (plan.start, plan.stop, plan.count, plan.auto_range),
            (-2.0, 1.0, 4, false)
        );
        assert!(ExposurePlan::new(&base, &HdrConfig::default()).auto_range);
    }
}
