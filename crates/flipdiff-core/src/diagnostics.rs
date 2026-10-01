//! Diagnostics: *why* two images differ, in numbers an agent can quote.
//!
//! FLIP says how visible a difference is. [`diagnose`] says what kind of
//! difference it is, by testing explanations that can each be checked against
//! the pixels:
//!
//! * **Global tone shift.** A per-channel gain (plus bias when the image has
//!   enough tonal range) of the capture against the baseline is fitted in
//!   linear light, by trimmed least squares over low-gradient pixels. The
//!   inverse fit is applied to the capture and FLIP is run again;
//!   `tone_explained_fraction` is the share of the mean FLIP error that
//!   removed.
//! * **Sub-pixel shift.** Phase correlation of the luminance (Hann window, FFT)
//!   finds the offset; when it is at least `shift_min_px` with confidence at
//!   least `shift_min_confidence`, the capture is resampled by the inverse
//!   offset, FLIP is run again and `shift_explained_fraction` is the share
//!   removed.
//! * **Signed difference**: which way the luminance moved, and where
//!   (`signed_diff.png`: blue darker, orange brighter).
//! * **Non-finite samples** in an HDR capture, located in a mask PNG.
//!
//! The change is then classified ([`ChangeClass`]) with the thresholds of
//! [`DiagnosticsConfig`] and described by a fixed template
//! ([`Diagnostics::description`]). The description only states a cause when
//! its explained fraction reaches `partial_min`.
//!
//! The analysis uses sequential arithmetic over fixed sample grids, so the
//! same pair always yields the same numbers and text. Memory stays near the
//! size of the images: pixels are read through [`Pixels`] instead of being
//! converted to float planes.

use std::path::Path;
use std::sync::OnceLock;
use std::time::Instant;

use rayon::prelude::*;
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};

use crate::compare::{CompareOptions, Comparison, compare_rgba};
use crate::error::{Error, Result};
use crate::hdr::{HdrImage, compare_hdr};
use crate::hotspots::{HotspotOptions, find_hotspots};
use crate::meta::MetaChecker;
use crate::report::{Hotspot, Properties};

/// Default sidecar key globs treated as timings (case-insensitive).
pub const DEFAULT_PERF_KEYS: &[&str] = &["*_ms", "gpu_ms", "frame_ms", "*.ms", "timing.*"];

/// Mean FLIP below which no cause search is run: nothing is visible to explain.
const NEGLIGIBLE_MEAN_FLIP: f64 = 1.0e-4;

/// Largest side, in pixels, of the luminance plane the shift estimate starts from.
const SHIFT_COARSE_MAX: usize = 1024;

/// Pixels from the tone fit sampling grid (about; the grid stride is derived).
const TONE_SAMPLES: f64 = 1.0e6;

/// Brightness change, in display luminance, below which a pixel counts as unchanged.
const SIGNED_EPSILON: f32 = 0.5 / 255.0;

/// Smallest robust maximum of the signed-difference colour scale.
const SIGNED_MIN_SCALE: f32 = 0.004;

/// Clusters of non-finite samples whose boxes are reported.
pub const MAX_NONFINITE_CLUSTERS: usize = 16;

/// Settings of the diagnostics engine (`[diagnostics]` in `flipdiff.toml`).
#[derive(Debug, Clone, PartialEq)]
pub struct DiagnosticsConfig {
    /// Run diagnostics at all (`enabled`, default true).
    pub enabled: bool,
    /// Run the sub-pixel shift estimate (`shift_detection`, default true).
    pub shift_detection: bool,
    /// Smallest shift, in pixels, that counts as detected (`shift_min_px`, 0.25).
    pub shift_min_px: f64,
    /// Smallest phase-correlation confidence in `[0, 1]` that counts as
    /// detected (`shift_min_confidence`, 0.5).
    pub shift_min_confidence: f64,
    /// Peak FLIP at or below which an unexplained difference is `noise`
    /// (`noise_max_flip`, 0.05).
    pub noise_max_flip: f64,
    /// Share of the mean error a single cause must remove to name the whole
    /// change (`explained_min`, 0.8).
    pub explained_min: f64,
    /// Share a cause must remove to be mentioned at all (`partial_min`, 0.2);
    /// below it on both causes a visible change is `local_structure`.
    pub partial_min: f64,
    /// Sidecar key globs read as timings (`perf_keys`).
    pub perf_keys: Vec<String>,
}

impl Default for DiagnosticsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            shift_detection: true,
            shift_min_px: 0.25,
            shift_min_confidence: 0.5,
            noise_max_flip: 0.05,
            explained_min: 0.8,
            partial_min: 0.2,
            perf_keys: DEFAULT_PERF_KEYS.iter().map(|k| (*k).to_owned()).collect(),
        }
    }
}

impl DiagnosticsConfig {
    /// Checks ranges and globs.
    pub fn validate(&self) -> Result<()> {
        let unit = |name: &str, v: f64| {
            if v.is_finite() && (0.0..=1.0).contains(&v) {
                Ok(())
            } else {
                Err(Error::Config(format!(
                    "diagnostics {name} must be in [0, 1], got {v}"
                )))
            }
        };
        unit("shift_min_confidence", self.shift_min_confidence)?;
        unit("noise_max_flip", self.noise_max_flip)?;
        unit("explained_min", self.explained_min)?;
        unit("partial_min", self.partial_min)?;
        if self.partial_min > self.explained_min {
            return Err(Error::Config(format!(
                "diagnostics partial_min ({}) must not exceed explained_min ({})",
                self.partial_min, self.explained_min
            )));
        }
        if !self.shift_min_px.is_finite() || self.shift_min_px < 0.0 {
            return Err(Error::Config(format!(
                "diagnostics shift_min_px must be finite and >= 0, got {}",
                self.shift_min_px
            )));
        }
        for k in &self.perf_keys {
            crate::config::compile_glob(k)?;
        }
        Ok(())
    }
}

/// What kind of change separates the capture from the baseline.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeClass {
    /// Decoded pixels are exactly equal.
    Identical,
    /// A difference whose peak FLIP is at or below `noise_max_flip` and that
    /// no tone shift or offset explains.
    Noise,
    /// One global tone fit (exposure, tint, black level) removes at least
    /// `explained_min` of the error.
    GlobalTone,
    /// Neither a tone fit nor an offset removes `partial_min` of the error and
    /// the error is above noise level.
    LocalStructure,
    /// A tone fit or an offset explains part (at least `partial_min`, below
    /// `explained_min`) of the error; the rest is something else.
    Mixed,
    /// A detected sub-pixel or whole-pixel offset removes at least
    /// `explained_min` of the error.
    Misaligned,
    /// The capture is unusable: all black or all white (the baseline is not),
    /// a flat colour, or it holds NaN or infinite samples.
    BrokenFrame,
}

impl ChangeClass {
    /// The snake_case name used in the JSON.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Identical => "identical",
            Self::Noise => "noise",
            Self::GlobalTone => "global_tone",
            Self::LocalStructure => "local_structure",
            Self::Mixed => "mixed",
            Self::Misaligned => "misaligned",
            Self::BrokenFrame => "broken_frame",
        }
    }
}

/// Global tone fit of the capture against the baseline, in linear light.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToneShift {
    /// `capture = gain * baseline + bias`, per channel `[r, g, b]`.
    pub gain: [f64; 3],
    /// The per-channel bias, in linear light (0 when the fit was gain-only).
    pub bias: [f64; 3],
    /// `affine` when gain and bias were fitted, `gain` when the baseline had
    /// too little tonal range for a bias.
    pub model: String,
    /// Exposure change in stops: `log2` of the luminance-weighted gain.
    /// Negative is darker.
    pub exposure_stops: f64,
    /// Tint relative to green, for example `warmer (red +2.1%, blue -1.8% vs
    /// green)`, or `neutral`.
    pub white_balance: String,
    /// One human-readable line, for example `3.1% darker (-0.05 stops), +0.02
    /// red offset`.
    pub summary: String,
    /// Share of the mean FLIP error removed by applying the inverse fit to the
    /// capture, in `[0, 1]`; 0 when the fit is negligible.
    pub tone_explained_fraction: f64,
    /// Pixels the fit used.
    pub samples: u64,
}

/// FLIP error that remains once the global tone shift is removed.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Residual {
    /// Mean FLIP error of the tone-corrected capture (the original mean when
    /// the tone fit is negligible).
    pub flip_mean: f64,
    /// Maximum FLIP error of the tone-corrected capture.
    pub flip_max: f64,
    /// Where that error is concentrated, largest first (at most 3).
    pub hotspots: Vec<Hotspot>,
}

/// Sub-pixel shift of the capture against the baseline.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShiftEstimate {
    /// Horizontal offset of the capture's content in pixels; positive is right.
    pub dx: f64,
    /// Vertical offset in pixels; positive is down.
    pub dy: f64,
    /// Weighted phase coherence of the cross-spectrum around the fitted
    /// shift, in `[0, 1]`: 1 for a pure translation, near 0 for unrelated
    /// content.
    pub confidence: f64,
    /// Whether the offset reached `shift_min_px` with `shift_min_confidence`.
    pub detected: bool,
    /// Share of the mean FLIP error removed by resampling the capture back by
    /// `(dx, dy)`; `None` unless `detected`.
    pub shift_explained_fraction: Option<f64>,
    /// Factor the first estimate's luminance plane was downsampled by.
    pub coarse_downsample: u32,
    /// Wall time of the estimate (excluding the compensated FLIP run), in ms.
    pub estimate_ms: f64,
}

/// Which way the luminance moved, over the whole frame.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignedDiff {
    /// Mean of `capture - baseline` display luminance (encoded Rec. 709 for
    /// LDR, linear for HDR). Positive is brighter.
    pub mean_delta: f64,
    /// Fraction of pixels brighter by more than half a code value.
    pub frac_brighter: f64,
    /// Fraction of pixels darker by more than half a code value.
    pub frac_darker: f64,
    /// Luminance difference mapped to full colour in `signed_diff.png` (the
    /// 99.5th percentile of the absolute difference, at least 0.004).
    pub scale: f64,
}

/// A connected group of non-finite samples.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NonFiniteCluster {
    /// Bounding box `[x, y, w, h]` in pixels.
    pub rect_px: [u32; 4],
    /// Pixels in the cluster.
    pub pixels: u64,
}

/// Where the capture holds NaN, infinite or negative samples (HDR only).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NonFiniteMap {
    /// NaN samples.
    pub nan: u64,
    /// Infinite samples.
    pub inf: u64,
    /// Finite samples below zero.
    pub negative: u64,
    /// Number of 8-connected groups of affected pixels.
    pub cluster_count: u64,
    /// Boxes of the first [`MAX_NONFINITE_CLUSTERS`] groups in raster order.
    pub clusters: Vec<NonFiniteCluster>,
}

/// One timing key present on both sides.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerfDelta {
    /// Sidecar key.
    pub key: String,
    /// Value on the baseline side.
    pub baseline: f64,
    /// Value on the capture side.
    pub capture: f64,
    /// `capture - baseline`.
    pub delta: f64,
    /// `delta` as a percentage of `baseline`; `None` when `baseline` is 0.
    pub delta_pct: Option<f64>,
}

/// What the diagnostics engine found for one compared pair.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostics {
    /// The kind of change.
    pub class: ChangeClass,
    /// A deterministic, template-based sentence; every cause it names is
    /// backed by the numbers below.
    pub description: String,
    /// Global tone fit; `None` for an identical pair or a negligible error.
    #[serde(default)]
    pub tone: Option<ToneShift>,
    /// Error left after the tone fit; present exactly when `tone` is.
    #[serde(default)]
    pub residual: Option<Residual>,
    /// Sub-pixel shift; `None` when disabled, identical or negligible.
    #[serde(default)]
    pub shift: Option<ShiftEstimate>,
    /// Luminance direction; `None` for an identical pair.
    #[serde(default)]
    pub signed: Option<SignedDiff>,
    /// Non-finite samples of the capture; `None` when there are none.
    #[serde(default)]
    pub nonfinite: Option<NonFiniteMap>,
    /// Timing keys present on both sides' sidecars, largest relative change first.
    #[serde(default)]
    pub perf: Vec<PerfDelta>,
    /// Wall time of the whole analysis in milliseconds.
    #[serde(default)]
    pub elapsed_ms: f64,
}

impl Diagnostics {
    /// The verdict line: the class (`bit-identical` for bit-identical pixels)
    /// followed by the timing changes, for example `bit-identical · gpu_ms 4.85
    /// → 2.71 (−44%)`.
    pub fn verdict_line(&self, bit_identical: Option<bool>) -> String {
        let head = match (self.class, bit_identical) {
            (ChangeClass::Identical, Some(true)) => "bit-identical",
            (ChangeClass::Identical, _) => "pixels identical",
            (c, _) => c.as_str(),
        };
        match perf_summary(&self.perf) {
            Some(p) => format!("{head} · {p}"),
            None => head.to_owned(),
        }
    }
}

/// Pixel access to either kind of decoded image, without float copies.
#[derive(Clone, Copy)]
pub enum Pixels<'a> {
    /// 8-bit sRGB with alpha; composited over black.
    Ldr(&'a image::RgbaImage),
    /// Linear `f32` RGB.
    Hdr(&'a HdrImage),
}

fn srgb_to_lin(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn lin_to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn srgb_lut() -> &'static [f32; 256] {
    static LUT: OnceLock<[f32; 256]> = OnceLock::new();
    LUT.get_or_init(|| {
        let mut t = [0.0f32; 256];
        for (i, v) in t.iter_mut().enumerate() {
            *v = srgb_to_lin(i as f32 / 255.0);
        }
        t
    })
}

impl Pixels<'_> {
    fn dims(&self) -> (usize, usize) {
        match self {
            Pixels::Ldr(i) => (i.width() as usize, i.height() as usize),
            Pixels::Hdr(i) => (i.width as usize, i.height as usize),
        }
    }

    fn is_ldr(&self) -> bool {
        matches!(self, Pixels::Ldr(_))
    }

    /// Linear RGB of pixel `i` (row-major).
    fn lin(&self, i: usize) -> [f32; 3] {
        match self {
            Pixels::Ldr(img) => {
                let raw = img.as_raw();
                let p = &raw[i * 4..i * 4 + 4];
                if p[3] == 255 {
                    let lut = srgb_lut();
                    [
                        lut[usize::from(p[0])],
                        lut[usize::from(p[1])],
                        lut[usize::from(p[2])],
                    ]
                } else {
                    let a = f32::from(p[3]) / 255.0;
                    let enc = |c: u8| srgb_to_lin(f32::from(c) / 255.0 * a);
                    [enc(p[0]), enc(p[1]), enc(p[2])]
                }
            }
            Pixels::Hdr(img) => {
                let p = &img.data[i * 3..i * 3 + 3];
                [p[0], p[1], p[2]]
            }
        }
    }

    /// Display-referred luminance of pixel `i`: Rec. 709 of the encoded values
    /// for LDR (as in `Properties`), of the linear values for HDR.
    fn luma(&self, i: usize) -> f32 {
        match self {
            Pixels::Ldr(img) => {
                let raw = img.as_raw();
                let p = &raw[i * 4..i * 4 + 4];
                let a = f32::from(p[3]) / (255.0 * 255.0);
                a * (0.2126 * f32::from(p[0]) + 0.7152 * f32::from(p[1]) + 0.0722 * f32::from(p[2]))
            }
            Pixels::Hdr(_) => {
                let [r, g, b] = self.lin(i);
                0.2126 * r + 0.7152 * g + 0.0722 * b
            }
        }
    }
}

/// An owned corrected capture for a FLIP re-run.
enum Owned {
    Ldr(image::RgbaImage),
    Hdr(HdrImage),
}

/// Everything [`diagnose`] needs about one compared pair.
pub struct DiagnoseRequest<'a> {
    /// Baseline pixels (the FLIP reference).
    pub baseline: Pixels<'a>,
    /// Capture pixels.
    pub capture: Pixels<'a>,
    /// The pair's FLIP comparison.
    pub comparison: &'a Comparison,
    /// FLIP settings of the run, reused for the re-runs.
    pub flip: &'a CompareOptions,
    /// Whether the files' samples are exactly equal.
    pub bit_identical: Option<bool>,
    /// Structural checks of the baseline.
    pub baseline_properties: Option<Properties>,
    /// Structural checks of the capture.
    pub capture_properties: Option<Properties>,
    /// The entry's hotspots of the original error map.
    pub hotspots: &'a [Hotspot],
    /// Hotspot settings, reused for the residual map.
    pub hotspot_options: HotspotOptions,
    /// Thresholds.
    pub config: &'a DiagnosticsConfig,
    /// The capture file, to locate non-finite samples of an HDR image.
    pub capture_path: Option<&'a Path>,
    /// Where to write the signed-difference and mask PNGs; `None` writes none.
    pub out: Option<DiagOut<'a>>,
}

/// Output location of the diagnostic images.
#[derive(Clone, Copy)]
pub struct DiagOut<'a> {
    /// The report directory.
    pub report_dir: &'a Path,
    /// The entry name (images go to `images/<name>/`).
    pub name: &'a str,
}

/// Result of [`diagnose`].
#[derive(Debug, Clone, PartialEq)]
pub struct DiagnoseOutput {
    /// The findings.
    pub diagnostics: Diagnostics,
    /// `images/<name>/signed_diff.png`, relative to the report directory.
    pub signed_diff: Option<String>,
    /// `images/<name>/nonfinite_mask.png`, relative to the report directory.
    pub nonfinite_mask: Option<String>,
}

fn finite(v: f64) -> f64 {
    if v.is_finite() { v } else { 0.0 }
}

fn clamp01(v: f64) -> f64 {
    finite(v).clamp(0.0, 1.0)
}

/// Runs every analysis on one pair. Only writing a PNG can fail.
pub fn diagnose(req: &DiagnoseRequest<'_>) -> Result<DiagnoseOutput> {
    let started = Instant::now();
    let cfg = req.config;
    let metrics = &req.comparison.metrics;
    let mean0 = metrics.mean;
    let identical = req.bit_identical == Some(true) || metrics.max == 0.0;
    let mut out = DiagnoseOutput {
        diagnostics: Diagnostics {
            class: ChangeClass::Identical,
            description: String::new(),
            tone: None,
            residual: None,
            shift: None,
            signed: None,
            nonfinite: None,
            perf: Vec::new(),
            elapsed_ms: 0.0,
        },
        signed_diff: None,
        nonfinite_mask: None,
    };
    let (w, h) = req.baseline.dims();
    let dims_ok = (w, h) == req.capture.dims() && w > 0 && h > 0;

    if identical || !dims_ok || !mean0.is_finite() {
        out.diagnostics.class = if identical {
            ChangeClass::Identical
        } else {
            ChangeClass::Noise
        };
        out.diagnostics.description = if identical {
            if req.bit_identical == Some(true) {
                "Bit-identical.".to_owned()
            } else {
                "Decoded pixels are identical; the files differ only in encoding.".to_owned()
            }
        } else {
            "No analysis: the error map is not usable.".to_owned()
        };
        out.diagnostics.elapsed_ms = elapsed_ms(started);
        return Ok(out);
    }

    // Signed difference, flat-frame statistics and the PNG.
    let signed = signed_analysis(req, w, h, &mut out)?;
    let flat_capture = signed.cap_std < 1.0e-3 && signed.base_std > 0.02;

    // Non-finite samples.
    if let (Some(p), Some(path)) = (req.capture_properties, req.capture_path) {
        if p.nan_count + p.inf_count + p.negative_count > 0 {
            out.diagnostics.nonfinite =
                locate_nonfinite(path, &p, req.out, &mut out.nonfinite_mask)?;
        }
    }

    // Both hypotheses are fitted first, then their FLIP re-runs (the costly
    // part) go side by side.
    let analyse = mean0 >= NEGLIGIBLE_MEAN_FLIP;
    let fit = if analyse {
        fit_tone(req.baseline, req.capture)
    } else {
        None
    };
    let t0 = Instant::now();
    let est = if analyse && cfg.shift_detection {
        estimate_shift(req.baseline, req.capture)
    } else {
        None
    };
    let estimate_ms = elapsed_ms(t0);
    let detected = est.as_ref().is_some_and(|e| {
        e.dx.hypot(e.dy) >= cfg.shift_min_px && e.confidence >= cfg.shift_min_confidence
    });
    let (tone_cmp, shift_cmp) = rayon::join(
        || {
            let fit = fit.as_ref().filter(|f| !f.negligible())?;
            rerun(req, &apply_tone(req.capture, fit))
        },
        || {
            let e = est.as_ref().filter(|_| detected)?;
            rerun(req, &shift_capture(req.capture, e.dx, e.dy))
        },
    );

    let mut tone_frac = 0.0;
    if let Some(fit) = &fit {
        let mut residual = Residual {
            flip_mean: mean0,
            flip_max: finite(metrics.max),
            hotspots: Vec::new(),
        };
        if let Some(cmp) = &tone_cmp {
            tone_frac = explained(mean0, cmp.metrics.mean);
            residual.flip_mean = finite(cmp.metrics.mean);
            residual.flip_max = finite(cmp.metrics.max);
            let mut opts = req.hotspot_options;
            opts.top_k = opts.top_k.clamp(1, 3);
            residual.hotspots = find_hotspots(
                &cmp.error_map,
                None,
                cmp.metrics.width,
                cmp.metrics.height,
                &opts,
            );
        }
        out.diagnostics.tone = Some(fit.report(tone_frac));
        out.diagnostics.residual = Some(residual);
    }
    let shift_explained = shift_cmp
        .as_ref()
        .map(|cmp| explained(mean0, cmp.metrics.mean));
    let shift_frac = shift_explained.unwrap_or(0.0);
    if let Some(est) = &est {
        out.diagnostics.shift = Some(ShiftEstimate {
            dx: finite(est.dx),
            dy: finite(est.dy),
            confidence: clamp01(est.confidence),
            detected,
            shift_explained_fraction: shift_explained,
            coarse_downsample: est.coarse_downsample,
            estimate_ms,
        });
    }

    let broken = broken_reason(req, flat_capture, out.diagnostics.nonfinite.as_ref());
    let class = if broken.is_some() {
        ChangeClass::BrokenFrame
    } else if shift_frac >= cfg.explained_min && shift_frac >= tone_frac {
        ChangeClass::Misaligned
    } else if tone_frac >= cfg.explained_min {
        ChangeClass::GlobalTone
    } else if tone_frac.max(shift_frac) < cfg.partial_min {
        if metrics.max <= cfg.noise_max_flip {
            ChangeClass::Noise
        } else {
            ChangeClass::LocalStructure
        }
    } else {
        ChangeClass::Mixed
    };
    out.diagnostics.class = class;
    out.diagnostics.description = describe(&Facts {
        class,
        cfg,
        metrics,
        hotspots: req.hotspots,
        tone: out.diagnostics.tone.as_ref(),
        residual: out.diagnostics.residual.as_ref(),
        shift: out.diagnostics.shift.as_ref(),
        signed: out.diagnostics.signed.as_ref(),
        broken: broken.as_deref(),
    });
    out.diagnostics.elapsed_ms = elapsed_ms(started);
    Ok(out)
}

fn elapsed_ms(t: Instant) -> f64 {
    (t.elapsed().as_secs_f64() * 1000.0 * 10.0).round() / 10.0
}

/// Share of `before` removed by going to `after`, in `[0, 1]`.
fn explained(before: f64, after: f64) -> f64 {
    if before <= 0.0 || !after.is_finite() {
        return 0.0;
    }
    clamp01((before - after) / before)
}

fn rerun(req: &DiagnoseRequest<'_>, corrected: &Owned) -> Option<Comparison> {
    match (req.baseline, corrected) {
        (Pixels::Ldr(b), Owned::Ldr(c)) => compare_rgba(c, b, req.flip).ok(),
        (Pixels::Hdr(b), Owned::Hdr(c)) => compare_hdr(c, b, req.flip).ok().map(|(cmp, _)| cmp),
        _ => None,
    }
}

fn broken_reason(
    req: &DiagnoseRequest<'_>,
    flat_capture: bool,
    nonfinite: Option<&NonFiniteMap>,
) -> Option<String> {
    if let Some(nf) = nonfinite {
        if nf.nan > 0 || nf.inf > 0 {
            return Some(format!(
                "Capture has non-finite samples ({} NaN, {} infinite) in {} area{}",
                nf.nan,
                nf.inf,
                nf.cluster_count,
                if nf.cluster_count == 1 { "" } else { "s" }
            ));
        }
    }
    let (cap, base) = (req.capture_properties?, req.baseline_properties);
    if cap.nan_count > 0 || cap.inf_count > 0 {
        return Some(format!(
            "Capture has non-finite samples ({} NaN, {} infinite)",
            cap.nan_count, cap.inf_count
        ));
    }
    if cap.is_all_black && !base.is_some_and(|b| b.is_all_black) {
        return Some("Capture is entirely black".to_owned());
    }
    if cap.is_all_white && !base.is_some_and(|b| b.is_all_white) {
        return Some("Capture is entirely white".to_owned());
    }
    flat_capture.then(|| "Capture is a single flat colour".to_owned())
}

// ---------------------------------------------------------------------------
// Tone fit
// ---------------------------------------------------------------------------

struct ToneFit {
    gain: [f64; 3],
    bias: [f64; 3],
    affine: bool,
    samples: usize,
}

impl ToneFit {
    fn luminance_gain(&self) -> f64 {
        0.2126 * self.gain[0] + 0.7152 * self.gain[1] + 0.0722 * self.gain[2]
    }

    fn negligible(&self) -> bool {
        self.gain.iter().all(|g| (g - 1.0).abs() < 0.004)
            && self.bias.iter().all(|b| b.abs() < 0.002)
    }

    fn report(&self, tone_explained_fraction: f64) -> ToneShift {
        let gy = self.luminance_gain();
        let stops = if gy > 0.0 { gy.log2() } else { 0.0 };
        let white_balance = white_balance(&self.gain);
        let mut parts = Vec::new();
        let pct = (1.0 - gy).abs() * 100.0;
        if pct >= 0.05 {
            parts.push(format!(
                "{pct:.1}% {} ({stops:+.2} stops)",
                if gy < 1.0 { "darker" } else { "brighter" }
            ));
        }
        if white_balance != "neutral" {
            parts.push(white_balance.clone());
        }
        let names = ["red", "green", "blue"];
        for (b, n) in self.bias.iter().zip(names) {
            if b.abs() >= 0.005 {
                parts.push(format!("{b:+.2} {n} offset"));
            }
        }
        let summary = if parts.is_empty() {
            "no global tone change".to_owned()
        } else {
            parts.join(", ")
        };
        ToneShift {
            gain: self.gain.map(finite),
            bias: self.bias.map(finite),
            model: if self.affine { "affine" } else { "gain" }.to_owned(),
            exposure_stops: finite(stops),
            white_balance,
            summary,
            tone_explained_fraction,
            samples: self.samples as u64,
        }
    }
}

/// `neutral`, or the red and blue gains relative to green.
fn white_balance(gain: &[f64; 3]) -> String {
    if gain[1] <= 0.0 {
        return "neutral".to_owned();
    }
    let rel = |g: f64| (g / gain[1] - 1.0) * 100.0;
    let (r, b) = (rel(gain[0]), rel(gain[2]));
    let mut parts = Vec::new();
    if r.abs() >= 1.5 {
        parts.push(format!("red {r:+.1}%"));
    }
    if b.abs() >= 1.5 {
        parts.push(format!("blue {b:+.1}%"));
    }
    if parts.is_empty() {
        return "neutral".to_owned();
    }
    let lead = if r >= 1.5 && b <= -1.5 {
        "warmer"
    } else if r <= -1.5 && b >= 1.5 {
        "cooler"
    } else {
        "tint"
    };
    format!("{lead} ({} vs green)", parts.join(", "))
}

struct Sample {
    grad: f32,
    base: [f32; 3],
    cap: [f32; 3],
}

/// Trimmed least-squares fit of `capture = gain * baseline + bias` per channel
/// over the lower-gradient half of a sampling grid.
fn fit_tone(base: Pixels<'_>, cap: Pixels<'_>) -> Option<ToneFit> {
    let (w, h) = base.dims();
    if w < 3 || h < 3 {
        return None;
    }
    let stride = ((w * h) as f64 / TONE_SAMPLES).sqrt().ceil().max(1.0) as usize;
    let mut samples = Vec::new();
    let mut y = 1;
    while y < h - 1 {
        let mut x = 1;
        while x < w - 1 {
            let i = y * w + x;
            let grad = (base.luma(i + 1) - base.luma(i - 1)).abs()
                + (base.luma(i + w) - base.luma(i - w)).abs();
            samples.push(Sample {
                grad,
                base: base.lin(i),
                cap: cap.lin(i),
            });
            x += stride;
        }
        y += stride;
    }
    if samples.len() < 64 {
        return None;
    }
    let mut grads: Vec<f32> = samples.iter().map(|s| s.grad).collect();
    let mid = grads.len() / 2;
    let median = *grads.select_nth_unstable_by(mid, f32::total_cmp).1;
    let low: Vec<&Sample> = samples.iter().filter(|s| s.grad <= median).collect();
    let pool: Vec<&Sample> = if low.len() >= 64 {
        low
    } else {
        samples.iter().collect()
    };
    let ldr = base.is_ldr();
    let clipped = |v: f32| v <= 0.0 || (ldr && v >= 0.999);
    let (mut gain, mut bias) = ([1.0; 3], [0.0; 3]);
    let (mut affine, mut used) = (true, usize::MAX);
    for c in 0..3 {
        let pairs: Vec<(f32, f32)> = pool
            .iter()
            .filter(|s| !clipped(s.base[c]) && !clipped(s.cap[c]))
            .map(|s| (s.base[c], s.cap[c]))
            .collect();
        let (g, b, aff) = robust_fit(&pairs)?;
        if g <= 0.0 || !g.is_finite() {
            return None;
        }
        (gain[c], bias[c]) = (g, b);
        affine &= aff;
        used = used.min(pairs.len());
    }
    Some(ToneFit {
        gain,
        bias,
        affine,
        samples: used,
    })
}

/// `(gain, bias, affine)` of `y = gain * x + bias` by four rounds of least
/// squares, each dropping the 20% of points with the largest residual.
fn robust_fit(pairs: &[(f32, f32)]) -> Option<(f64, f64, bool)> {
    if pairs.len() < 64 {
        return None;
    }
    let mut keep: Vec<(f32, f32)> = pairs.to_vec();
    let mut model = ols(&keep)?;
    for _ in 0..3 {
        let res: Vec<f32> = pairs
            .iter()
            .map(|&(x, y)| (f64::from(y) - model.0 * f64::from(x) - model.1).abs() as f32)
            .collect();
        let mut sorted = res.clone();
        let k = (sorted.len() * 4 / 5).min(sorted.len() - 1);
        let thr = *sorted.select_nth_unstable_by(k, f32::total_cmp).1;
        let next: Vec<(f32, f32)> = pairs
            .iter()
            .zip(&res)
            .filter(|(_, r)| **r <= thr)
            .map(|(p, _)| *p)
            .collect();
        if next.len() < 64 {
            break;
        }
        keep = next;
        model = ols(&keep)?;
    }
    Some(model)
}

fn ols(pairs: &[(f32, f32)]) -> Option<(f64, f64, bool)> {
    let n = pairs.len() as f64;
    let (mut sx, mut sy, mut sxx, mut sxy) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for &(x, y) in pairs {
        let (x, y) = (f64::from(x), f64::from(y));
        sx += x;
        sy += y;
        sxx += x * x;
        sxy += x * y;
    }
    let var = sxx / n - (sx / n) * (sx / n);
    if var.max(0.0).sqrt() >= 0.02 {
        let g = (sxy / n - (sx / n) * (sy / n)) / var;
        Some((g, sy / n - g * sx / n, true))
    } else if sxx > 0.0 {
        Some((sxy / sxx, 0.0, false))
    } else {
        None
    }
}

/// The capture with the inverse of `fit` applied.
fn apply_tone(cap: Pixels<'_>, fit: &ToneFit) -> Owned {
    let inv = |v: f32, c: usize| ((f64::from(v) - fit.bias[c]) / fit.gain[c]) as f32;
    match cap {
        Pixels::Ldr(img) => {
            let mut out = (*img).clone();
            let lut = srgb_lut();
            let raw: &mut [u8] = &mut out;
            raw.par_chunks_mut(4).for_each(|p| {
                let a = p[3];
                if a == 0 {
                    return;
                }
                let af = f32::from(a) / 255.0;
                for c in 0..3 {
                    let lin = if a == 255 {
                        lut[usize::from(p[c])]
                    } else {
                        srgb_to_lin(f32::from(p[c]) / 255.0 * af)
                    };
                    let enc = lin_to_srgb(inv(lin, c).clamp(0.0, 1.0));
                    p[c] = ((enc / af).clamp(0.0, 1.0) * 255.0).round() as u8;
                }
            });
            Owned::Ldr(out)
        }
        Pixels::Hdr(img) => {
            let mut out = (*img).clone();
            out.data.par_chunks_mut(3).for_each(|p| {
                for (c, v) in p.iter_mut().enumerate() {
                    *v = inv(*v, c).clamp(0.0, 1.0e15);
                }
            });
            Owned::Hdr(out)
        }
    }
}

// ---------------------------------------------------------------------------
// Sub-pixel shift
// ---------------------------------------------------------------------------

struct RawShift {
    dx: f64,
    dy: f64,
    confidence: f64,
    coarse_downsample: u32,
}

/// Luminance plane for the correlation: the box average over `f x f` blocks of
/// a compressed luminance `sqrt(y / (1 + y))`, over the `cw x ch` window at
/// `(x0, y0)`.
fn luma_plane(
    px: Pixels<'_>,
    x0: usize,
    y0: usize,
    cw: usize,
    ch: usize,
    f: usize,
) -> (Vec<f32>, usize, usize) {
    let (w, _) = px.dims();
    let (ow, oh) = (cw.div_ceil(f), ch.div_ceil(f));
    let mut out = vec![0.0f32; ow * oh];
    for oy in 0..oh {
        for ox in 0..ow {
            let (mut sum, mut n) = (0.0f32, 0.0f32);
            for yy in (oy * f)..((oy + 1) * f).min(ch) {
                for xx in (ox * f)..((ox + 1) * f).min(cw) {
                    let [r, g, b] = px.lin((y0 + yy) * w + x0 + xx);
                    let y = (0.2126 * r + 0.7152 * g + 0.0722 * b).max(0.0);
                    sum += (y / (1.0 + y)).sqrt();
                    n += 1.0;
                }
            }
            out[oy * ow + ox] = sum / n.max(1.0);
        }
    }
    (out, ow, oh)
}

fn transpose(src: &[Complex<f32>], w: usize, h: usize) -> Vec<Complex<f32>> {
    let mut dst = vec![Complex::new(0.0, 0.0); src.len()];
    const B: usize = 32;
    for by in (0..h).step_by(B) {
        for bx in (0..w).step_by(B) {
            for y in by..(by + B).min(h) {
                for x in bx..(bx + B).min(w) {
                    dst[x * h + y] = src[y * w + x];
                }
            }
        }
    }
    dst
}

fn fft2(
    planner: &mut FftPlanner<f32>,
    data: &mut Vec<Complex<f32>>,
    w: usize,
    h: usize,
    inverse: bool,
) {
    let plan = |p: &mut FftPlanner<f32>, n: usize| {
        if inverse {
            p.plan_fft_inverse(n)
        } else {
            p.plan_fft_forward(n)
        }
    };
    let rows = plan(planner, w);
    data.par_chunks_mut(w).for_each(|r| rows.process(r));
    let mut t = transpose(data, w, h);
    let cols = plan(planner, h);
    t.par_chunks_mut(h).for_each(|r| cols.process(r));
    *data = transpose(&t, h, w);
}

/// Mean-removed, Hann-windowed, zero-padded spectrum of a `w x h` plane.
fn spectrum(
    planner: &mut FftPlanner<f32>,
    plane: &[f32],
    w: usize,
    h: usize,
    pw: usize,
    ph: usize,
) -> Vec<Complex<f32>> {
    let mean = plane.iter().map(|&v| f64::from(v)).sum::<f64>() / plane.len().max(1) as f64;
    let hann = |i: usize, n: usize| {
        0.5 - 0.5 * (2.0 * std::f64::consts::PI * (i as f64 + 0.5) / n as f64).cos()
    };
    let wx: Vec<f64> = (0..w).map(|i| hann(i, w)).collect();
    let mut data = vec![Complex::new(0.0f32, 0.0); pw * ph];
    for y in 0..h {
        let wy = hann(y, h);
        for x in 0..w {
            data[y * pw + x].re = ((f64::from(plane[y * w + x]) - mean) * wx[x] * wy) as f32;
        }
    }
    fft2(planner, &mut data, pw, ph, false);
    data
}

fn signed_freq(k: usize, n: usize) -> f64 {
    if k < n / 2 {
        k as f64
    } else {
        k as f64 - n as f64
    }
}

/// Phase correlation of two equal-size planes: `(dx, dy, confidence)` of
/// `capture` against `baseline`, positive meaning the content moved right or
/// down. The peak of the whitened correlation gives the integer offset; a
/// weighted fit of the residual phase slope over the low frequencies gives the
/// sub-pixel part.
fn phase_correlate(base: &[f32], cap: &[f32], w: usize, h: usize) -> Option<(f64, f64, f64)> {
    if w < 16 || h < 16 {
        return None;
    }
    let (pw, ph) = (w.next_power_of_two(), h.next_power_of_two());
    let mut planner = FftPlanner::<f32>::new();
    let fa = spectrum(&mut planner, base, w, h, pw, ph);
    let fb = spectrum(&mut planner, cap, w, h, pw, ph);
    let cross: Vec<Complex<f32>> = fb.iter().zip(&fa).map(|(b, a)| b * a.conj()).collect();
    let n = cross.len();
    let mean_abs = cross.iter().map(|c| f64::from(c.norm())).sum::<f64>() / n as f64;
    if mean_abs < 1.0e-9 {
        return None;
    }
    let eps = (mean_abs * 1.0e-3) as f32;
    let whitened: Vec<Complex<f32>> = cross.iter().map(|c| c / (c.norm() + eps)).collect();
    let mut surface = whitened.clone();
    fft2(&mut planner, &mut surface, pw, ph, true);
    let (mut best, mut idx) = (f32::NEG_INFINITY, 0usize);
    for (i, c) in surface.iter().enumerate() {
        if c.re > best {
            (best, idx) = (c.re, i);
        }
    }
    if best <= 0.0 {
        return None;
    }
    let (px, py) = (idx % pw, idx / pw);
    let wrap = |p: usize, n: usize| {
        if p < n / 2 {
            p as isize
        } else {
            p as isize - n as isize
        }
    };
    let (ix, iy) = (wrap(px, pw), wrap(py, ph));
    // Sub-pixel: residual phase slope over low frequencies.
    let max_cross = cross
        .iter()
        .map(|c| f64::from(c.norm()))
        .fold(0.0f64, f64::max)
        .max(1.0e-30);
    let tau = 2.0 * std::f64::consts::PI;
    let (mut suu, mut suv, mut svv, mut suy, mut svy) = (0.0f64, 0.0f64, 1.0e-9f64, 0.0f64, 0.0f64);
    let mut bins: Vec<[f64; 4]> = Vec::new();
    for l in 0..ph {
        let v = signed_freq(l, ph);
        if v.abs() * 4.0 > ph as f64 {
            continue;
        }
        for k in 0..pw {
            let u = signed_freq(k, pw);
            if u.abs() * 4.0 > pw as f64 {
                continue;
            }
            // Weight by the raw cross-power: bins that carry no signal have
            // arbitrary phase and must not outvote the ones that do.
            let r = whitened[l * pw + k];
            let wgt = f64::from(cross[l * pw + k].norm()) / max_cross;
            if wgt < 1.0e-4 {
                continue;
            }
            let (uu, vv) = (tau * u / pw as f64, tau * v / ph as f64);
            let mut phi = f64::from(r.im).atan2(f64::from(r.re)) + uu * ix as f64 + vv * iy as f64;
            phi = (phi + std::f64::consts::PI).rem_euclid(tau) - std::f64::consts::PI;
            suu += wgt * uu * uu;
            suv += wgt * uu * vv;
            svv += wgt * vv * vv;
            suy += wgt * uu * phi;
            svy += wgt * vv * phi;
            bins.push([wgt, uu, vv, phi]);
        }
    }
    let det = suu * svv - suv * suv;
    let (ex, ey) = if det.abs() > 1.0e-12 {
        (
            -(svv * suy - suv * svy) / det,
            -(suu * svy - suv * suy) / det,
        )
    } else {
        (0.0, 0.0)
    };
    // The residual of a correct integer peak is within half a pixel.
    let (ex, ey) = (ex.clamp(-1.0, 1.0), ey.clamp(-1.0, 1.0));
    // Confidence: how well one shift explains the phases of the bins that
    // carry the signal, the weighted phase coherence of the fit residuals
    // (1 for a pure translation, near 0 for unrelated content).
    let (mut re, mut im, mut total) = (0.0f64, 0.0f64, 0.0f64);
    for [wgt, uu, vv, phi] in bins {
        let rho = phi + uu * ex + vv * ey;
        re += wgt * rho.cos();
        im += wgt * rho.sin();
        total += wgt;
    }
    let confidence = if total > 0.0 {
        (re.hypot(im) / total).clamp(0.0, 1.0)
    } else {
        0.0
    };
    Some((ix as f64 + ex, iy as f64 + ey, confidence))
}

/// Shift of `cap` against `base`: a coarse estimate on a plane of at most
/// [`SHIFT_COARSE_MAX`] pixels per side, refined at full resolution on a
/// window of the same size.
fn estimate_shift(base: Pixels<'_>, cap: Pixels<'_>) -> Option<RawShift> {
    let (w, h) = base.dims();
    let f = w.max(h).div_ceil(SHIFT_COARSE_MAX).max(1);
    let (mut rx, mut ry) = (0isize, 0isize);
    if f > 1 {
        let (a, aw, ah) = luma_plane(base, 0, 0, w, h, f);
        let (b, _, _) = luma_plane(cap, 0, 0, w, h, f);
        let (dx, dy, _) = phase_correlate(&a, &b, aw, ah)?;
        rx = (dx * f as f64).round() as isize;
        ry = (dy * f as f64).round() as isize;
    }
    // Full-resolution window; the capture window sits (rx, ry) away.
    let window = |len: usize, r: isize| -> Option<(usize, usize)> {
        let size = len.saturating_sub(r.unsigned_abs()).min(SHIFT_COARSE_MAX);
        if size < 32 {
            return None;
        }
        let lo = (-r).max(0) as usize;
        let hi = (len - size).min(len - size - r.max(0) as usize);
        let o = ((len - size) / 2).clamp(lo, hi.max(lo));
        Some((o, size))
    };
    let ((ox, cw), (oy, ch)) = (window(w, rx)?, window(h, ry)?);
    let (a, aw, ah) = luma_plane(base, ox, oy, cw, ch, 1);
    let cap_x = (ox as isize + rx) as usize;
    let cap_y = (oy as isize + ry) as usize;
    let (b, _, _) = luma_plane(cap, cap_x, cap_y, cw, ch, 1);
    let (ex, ey, confidence) = phase_correlate(&a, &b, aw, ah)?;
    Some(RawShift {
        dx: rx as f64 + ex,
        dy: ry as f64 + ey,
        confidence,
        coarse_downsample: f as u32,
    })
}

/// Catmull-Rom taps (indices clamped to the edge) for sampling at `i + d`.
fn cubic_taps(n: usize, d: f64) -> Vec<([usize; 4], [f32; 4])> {
    let last = n as isize - 1;
    (0..n)
        .map(|i| {
            let s = i as f64 + d;
            let base = s.floor();
            let t = (s - base) as f32;
            let b = base as isize;
            let w = [
                -0.5 * t * t * t + t * t - 0.5 * t,
                1.5 * t * t * t - 2.5 * t * t + 1.0,
                -1.5 * t * t * t + 2.0 * t * t + 0.5 * t,
                0.5 * t * t * t - 0.5 * t * t,
            ];
            let idx = [b - 1, b, b + 1, b + 2].map(|k| k.clamp(0, last) as usize);
            (idx, w)
        })
        .collect()
}

/// The capture resampled so that its content moves back by `(dx, dy)`:
/// `out(x, y) = capture(x + dx, y + dy)`, bicubic.
fn shift_capture(cap: Pixels<'_>, dx: f64, dy: f64) -> Owned {
    let (w, h) = cap.dims();
    let (tx, ty) = (cubic_taps(w, dx), cubic_taps(h, dy));
    match cap {
        Pixels::Ldr(img) => {
            let src = img.as_raw();
            let mut out = vec![0u8; src.len()];
            out.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
                let (yi, yw) = &ty[y];
                for (x, px) in row.chunks_exact_mut(4).enumerate() {
                    let (xi, xw) = &tx[x];
                    for (c, o) in px.iter_mut().enumerate() {
                        let mut acc = 0.0f32;
                        for j in 0..4 {
                            let mut line = 0.0f32;
                            for i in 0..4 {
                                line += xw[i] * f32::from(src[(yi[j] * w + xi[i]) * 4 + c]);
                            }
                            acc += yw[j] * line;
                        }
                        *o = acc.round().clamp(0.0, 255.0) as u8;
                    }
                }
            });
            let rebuilt = image::RgbaImage::from_raw(w as u32, h as u32, out);
            Owned::Ldr(rebuilt.unwrap_or_else(|| (*img).clone()))
        }
        Pixels::Hdr(img) => {
            let src = &img.data;
            let mut out = vec![0.0f32; src.len()];
            out.par_chunks_mut(w * 3).enumerate().for_each(|(y, row)| {
                let (yi, yw) = &ty[y];
                for (x, px) in row.chunks_exact_mut(3).enumerate() {
                    let (xi, xw) = &tx[x];
                    for (c, o) in px.iter_mut().enumerate() {
                        let mut acc = 0.0f32;
                        for j in 0..4 {
                            let mut line = 0.0f32;
                            for i in 0..4 {
                                line += xw[i] * src[(yi[j] * w + xi[i]) * 3 + c];
                            }
                            acc += yw[j] * line;
                        }
                        *o = acc.clamp(0.0, 1.0e15);
                    }
                }
            });
            Owned::Hdr(HdrImage {
                width: img.width,
                height: img.height,
                data: out,
                replaced: img.replaced,
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Signed difference
// ---------------------------------------------------------------------------

struct SignedStats {
    base_std: f64,
    cap_std: f64,
}

fn signed_analysis(
    req: &DiagnoseRequest<'_>,
    w: usize,
    h: usize,
    out: &mut DiagnoseOutput,
) -> Result<SignedStats> {
    let n = w * h;
    let mut delta = Vec::with_capacity(n);
    let (mut sum, mut brighter, mut darker) = (0.0f64, 0u64, 0u64);
    let (mut sb, mut sbb, mut sc, mut scc) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for i in 0..n {
        let (b, c) = (req.baseline.luma(i), req.capture.luma(i));
        let d = if (c - b).is_finite() { c - b } else { 0.0 };
        delta.push(d);
        sum += f64::from(d);
        if d > SIGNED_EPSILON {
            brighter += 1;
        } else if d < -SIGNED_EPSILON {
            darker += 1;
        }
        let (b, c) = (f64::from(b), f64::from(c));
        sb += b;
        sbb += b * b;
        sc += c;
        scc += c * c;
    }
    let nf = n as f64;
    let std = |s: f64, ss: f64| (ss / nf - (s / nf) * (s / nf)).max(0.0).sqrt();

    // Robust scale: the 99.5th percentile of |delta| from a histogram.
    let max_abs = delta.iter().fold(0.0f32, |m, d| m.max(d.abs()));
    let scale = if max_abs <= 0.0 {
        SIGNED_MIN_SCALE
    } else {
        const BINS: usize = 4096;
        let mut hist = vec![0u64; BINS];
        for d in &delta {
            let bin = ((d.abs() / max_abs) * (BINS as f32 - 1.0)) as usize;
            hist[bin.min(BINS - 1)] += 1;
        }
        let target = (nf * 0.995).ceil() as u64;
        let (mut acc, mut at) = (0u64, BINS - 1);
        for (i, c) in hist.iter().enumerate() {
            acc += c;
            if acc >= target {
                at = i;
                break;
            }
        }
        (((at + 1) as f32 / BINS as f32) * max_abs).max(SIGNED_MIN_SCALE)
    };
    out.diagnostics.signed = Some(SignedDiff {
        mean_delta: finite(sum / nf),
        frac_brighter: brighter as f64 / nf,
        frac_darker: darker as f64 / nf,
        scale: f64::from(scale),
    });

    if let Some(o) = req.out {
        let mut img = image::RgbImage::new(w as u32, h as u32);
        for (px, &d) in img.pixels_mut().zip(&delta) {
            let t = (d.abs() / scale).clamp(0.0, 1.0);
            let (zero, full): ([f32; 3], [f32; 3]) = if d >= 0.0 {
                ([30.0, 30.0, 34.0], [255.0, 140.0, 0.0])
            } else {
                ([30.0, 30.0, 34.0], [0.0, 120.0, 255.0])
            };
            for c in 0..3 {
                px.0[c] = (zero[c] + (full[c] - zero[c]) * t).round() as u8;
            }
        }
        out.signed_diff = Some(save_png(&img, o, "signed_diff.png")?);
    }
    Ok(SignedStats {
        base_std: std(sb, sbb),
        cap_std: std(sc, scc),
    })
}

fn save_png(img: &image::RgbImage, o: DiagOut<'_>, file: &str) -> Result<String> {
    let rel = format!("images/{}/{file}", o.name);
    let dest = o.report_dir.join(&rel);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|source| Error::Io {
            context: format!("creating {}", parent.display()),
            source,
        })?;
    }
    img.save(&dest)
        .map_err(|source| Error::Encode { path: dest, source })?;
    Ok(rel)
}

// ---------------------------------------------------------------------------
// Non-finite samples
// ---------------------------------------------------------------------------

/// Re-reads the HDR capture, writes the mask (magenta NaN, yellow infinite,
/// cyan negative, black elsewhere) and boxes the first clusters.
fn locate_nonfinite(
    path: &Path,
    props: &Properties,
    out: Option<DiagOut<'_>>,
    mask_path: &mut Option<String>,
) -> Result<Option<NonFiniteMap>> {
    let Ok(img) = image::open(path) else {
        return Ok(None);
    };
    let rgb = img.to_rgb32f();
    let (w, h) = (rgb.width() as usize, rgb.height() as usize);
    // 0 clean, 1 NaN, 2 infinite, 3 negative.
    let mut code = vec![0u8; w * h];
    for (c, px) in code.iter_mut().zip(rgb.as_raw().chunks_exact(3)) {
        for &v in px {
            let k = if v.is_nan() {
                1
            } else if v.is_infinite() {
                2
            } else if v < 0.0 {
                3
            } else {
                0
            };
            if k != 0 && (*c == 0 || k < *c) {
                *c = k;
            }
        }
    }
    if let Some(o) = out {
        let mut mask = image::RgbImage::new(w as u32, h as u32);
        for (px, &c) in mask.pixels_mut().zip(&code) {
            px.0 = match c {
                1 => [255, 0, 255],
                2 => [255, 220, 0],
                3 => [0, 200, 255],
                _ => [0, 0, 0],
            };
        }
        *mask_path = Some(save_png(&mask, o, "nonfinite_mask.png")?);
    }
    let (mut clusters, mut count) = (Vec::new(), 0u64);
    let mut stack: Vec<usize> = Vec::new();
    for start in 0..code.len() {
        if code[start] == 0 {
            continue;
        }
        count += 1;
        let (mut x0, mut y0, mut x1, mut y1, mut pixels) = (usize::MAX, usize::MAX, 0, 0, 0u64);
        code[start] = 0;
        stack.push(start);
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            pixels += 1;
            for ny in y.saturating_sub(1)..=(y + 1).min(h - 1) {
                for nx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                    let j = ny * w + nx;
                    if code[j] != 0 {
                        code[j] = 0;
                        stack.push(j);
                    }
                }
            }
        }
        if clusters.len() < MAX_NONFINITE_CLUSTERS {
            clusters.push(NonFiniteCluster {
                rect_px: [
                    x0 as u32,
                    y0 as u32,
                    (x1 - x0 + 1) as u32,
                    (y1 - y0 + 1) as u32,
                ],
                pixels,
            });
        }
    }
    Ok(Some(NonFiniteMap {
        nan: props.nan_count,
        inf: props.inf_count,
        negative: props.negative_count,
        cluster_count: count,
        clusters,
    }))
}

// ---------------------------------------------------------------------------
// Description
// ---------------------------------------------------------------------------

struct Facts<'a> {
    class: ChangeClass,
    cfg: &'a DiagnosticsConfig,
    metrics: &'a crate::report::Metrics,
    hotspots: &'a [Hotspot],
    tone: Option<&'a ToneShift>,
    residual: Option<&'a Residual>,
    shift: Option<&'a ShiftEstimate>,
    signed: Option<&'a SignedDiff>,
    broken: Option<&'a str>,
}

fn pct(f: f64) -> i64 {
    (f * 100.0).round() as i64
}

/// What the tone fit changed, in words: the exposure when it is at least 0.03
/// stops, the tint when there is one and any black-level offset, so a small
/// exposure number never stands in for a tint or offset that did the work.
fn tone_phrase(t: &ToneShift) -> String {
    let mut parts = Vec::new();
    let a = t.exposure_stops.abs();
    if a >= 0.03 {
        let dir = if t.exposure_stops < 0.0 {
            "darker"
        } else {
            "brighter"
        };
        parts.push(if a >= 0.1 {
            format!("{a:.1} stops {dir}")
        } else {
            format!("{a:.2} stops {dir}")
        });
    }
    if t.white_balance != "neutral" {
        parts.push(format!("tint {}", t.white_balance));
    }
    let offsets: Vec<String> = ["red", "green", "blue"]
        .iter()
        .zip(&t.bias)
        .filter(|(_, b)| b.abs() >= 0.005)
        .map(|(n, b)| format!("{b:+.2} {n}"))
        .collect();
    if !offsets.is_empty() {
        parts.push(format!("black-level offset {}", offsets.join(" ")));
    }
    if parts.is_empty() {
        "small tone offset".to_owned()
    } else {
        parts.join(", ")
    }
}

/// `capture mostly brighter (64% of pixels brighter, 4% darker)` when one
/// direction clearly dominates.
fn direction_clause(sd: &SignedDiff) -> Option<String> {
    let (hi, lo) = (
        sd.frac_brighter.max(sd.frac_darker),
        sd.frac_brighter.min(sd.frac_darker),
    );
    if hi < 0.2 || hi < 3.0 * lo {
        return None;
    }
    let (way, other) = if sd.frac_brighter > sd.frac_darker {
        ("brighter", "darker")
    } else {
        ("darker", "brighter")
    };
    Some(format!(
        "capture mostly {way} ({}% of pixels {way}, {}% {other})",
        pct(hi),
        pct(lo)
    ))
}

fn shift_text(dx: f64, dy: f64) -> String {
    let mut parts = Vec::new();
    let px = |v: f64| {
        if v.abs() >= 10.0 {
            format!("{:.0}", v.abs())
        } else {
            format!("{:.1}", v.abs())
        }
    };
    if dx.abs() >= 0.05 {
        parts.push(format!(
            "{} px {}",
            px(dx),
            if dx > 0.0 { "right" } else { "left" }
        ));
    }
    if dy.abs() >= 0.05 {
        parts.push(format!(
            "{} px {}",
            px(dy),
            if dy > 0.0 { "down" } else { "up" }
        ));
    }
    parts.join(" and ")
}

/// `Local structural change at bottom-center (149×39, 20% of error)` and what
/// the numbers allow to say about the rest of the frame.
fn local_sentence(
    f: &Facts<'_>,
    hotspots: &[Hotspot],
    mean_flip: f64,
    lead: &str,
    frame_px: f64,
) -> String {
    let Some(top) = hotspots.first() else {
        return format!(
            "{lead} below the hotspot threshold and spread across the frame (mean FLIP {mean_flip:.4}, max {:.2})",
            f.metrics.max
        );
    };
    let box_frac = top.rect_frac[2] * top.rect_frac[3];
    let shown: Vec<&Hotspot> = hotspots.iter().take(3).collect();
    let share: f64 = shown.iter().map(|h| h.share_of_total_error).sum();
    let area: f64 = shown
        .iter()
        .map(|h| f64::from(h.rect_px[2]) * f64::from(h.rect_px[3]))
        .sum();
    if box_frac >= 0.5 {
        return format!(
            "{lead} across most of the frame (largest region {}, {}×{}, {}% of error)",
            top.position,
            top.rect_px[2],
            top.rect_px[3],
            pct(top.share_of_total_error)
        );
    }
    let mut s = format!(
        "{lead} at {} ({}×{}, {}% of error)",
        top.position,
        top.rect_px[2],
        top.rect_px[3],
        pct(top.share_of_total_error)
    );
    if shown.len() > 1 {
        s.push_str(&format!(
            " and {} more region{}",
            shown.len() - 1,
            if shown.len() == 2 { "" } else { "s" }
        ));
    }
    if share >= 0.5 && area < frame_px * 0.9 {
        let outside = mean_flip * frame_px * (1.0 - share).max(0.0) / (frame_px - area);
        if outside <= 0.001 {
            s.push_str("; rest of frame unchanged");
        } else if outside <= 0.01 {
            s.push_str("; rest of frame within noise");
        } else {
            s.push_str(&format!(
                "; the rest of the frame also differs (mean FLIP {outside:.3})"
            ));
        }
    }
    s
}

fn describe(f: &Facts<'_>) -> String {
    let frame_px = f64::from(f.metrics.width) * f64::from(f.metrics.height);
    let mean = f.metrics.mean;
    match f.class {
        ChangeClass::Identical => "Bit-identical.".to_owned(),
        ChangeClass::BrokenFrame => format!("{}.", f.broken.unwrap_or("Capture is unusable")),
        ChangeClass::Noise => format!(
            "Noise-level difference only (peak FLIP {:.3}, mean {:.4}).",
            f.metrics.max, mean
        ),
        ChangeClass::Misaligned => {
            let (dx, dy, frac) = f.shift.map_or((0.0, 0.0, 0.0), |s| {
                (s.dx, s.dy, s.shift_explained_fraction.unwrap_or(0.0))
            });
            format!(
                "Capture shifted {} (explains {}%).",
                shift_text(dx, dy),
                pct(frac)
            )
        }
        ChangeClass::GlobalTone => {
            let Some(t) = f.tone else {
                return "Global tone change.".to_owned();
            };
            let tail = match f.residual {
                Some(r) if r.flip_max <= f.cfg.noise_max_flip.max(0.1) => {
                    "; no structural change".to_owned()
                }
                Some(r) => match r.hotspots.first() {
                    Some(h) => format!(
                        "; residual local error at {} ({}×{}, max FLIP {:.2})",
                        h.position, h.rect_px[2], h.rect_px[3], h.max_flip
                    ),
                    None => format!("; residual FLIP max {:.2}", r.flip_max),
                },
                None => String::new(),
            };
            format!(
                "Whole frame {} (tone shift explains {}% of the difference){tail}.",
                tone_phrase(t),
                pct(t.tone_explained_fraction)
            )
        }
        ChangeClass::LocalStructure => {
            let mut s = local_sentence(f, f.hotspots, mean, "Local structural change", frame_px);
            if s.starts_with("Local structural change across") {
                s = s.replacen("Local structural change", "Structural change", 1);
                if let Some(d) = f.signed.and_then(direction_clause) {
                    s.push_str(&format!("; {d}"));
                }
            }
            format!("{s}.")
        }
        ChangeClass::Mixed => {
            let mut causes = Vec::new();
            if let Some(t) = f
                .tone
                .filter(|t| t.tone_explained_fraction >= f.cfg.partial_min)
            {
                causes.push(format!(
                    "tone shift ({}) explains {}%",
                    tone_phrase(t),
                    pct(t.tone_explained_fraction)
                ));
            }
            if let Some((s, fr)) = f.shift.and_then(|s| {
                s.shift_explained_fraction
                    .filter(|fr| *fr >= f.cfg.partial_min)
                    .map(|fr| (s, fr))
            }) {
                causes.push(format!(
                    "capture shifted {} explains {}%",
                    shift_text(s.dx, s.dy),
                    pct(fr)
                ));
            }
            let (hot, residual_mean): (&[Hotspot], f64) = match f.residual {
                Some(r) if r.flip_mean < mean => (&r.hotspots, r.flip_mean),
                _ => (f.hotspots, mean),
            };
            let mut rest =
                local_sentence(f, hot, residual_mean, "local structural change", frame_px);
            if rest.starts_with("local structural change across") {
                rest = rest.replacen("local structural change", "structural change", 1);
                if let Some(d) = f.signed.and_then(direction_clause) {
                    rest.push_str(&format!("; {d}"));
                }
            }
            format!("Mixed change: {}; remaining {rest}.", causes.join("; "))
        }
    }
}

// ---------------------------------------------------------------------------
// Perf pairing
// ---------------------------------------------------------------------------

fn numeric(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
    .filter(|x| x.is_finite())
}

/// Timing keys (matching `cfg.perf_keys`) with a numeric value in both sides'
/// sidecars of image `name`, largest relative change first. Unreadable
/// sidecars give no pairs (the meta check reports them).
pub fn perf_pairs(
    checker: &MetaChecker,
    cfg: &DiagnosticsConfig,
    baseline_root: &Path,
    capture_root: &Path,
    name: &str,
) -> Vec<PerfDelta> {
    let globs: Vec<_> = cfg
        .perf_keys
        .iter()
        .filter_map(|g| crate::config::compile_glob(g).ok())
        .collect();
    let (Ok(Some(b)), Ok(Some(c))) = (
        checker.load(baseline_root, name),
        checker.load(capture_root, name),
    ) else {
        return Vec::new();
    };
    let mut out: Vec<PerfDelta> = b
        .iter()
        .filter(|(k, _)| globs.iter().any(|g| g.is_match(k.as_str())))
        .filter_map(|(k, bv)| {
            let (bv, cv) = (numeric(bv)?, numeric(c.get(k)?)?);
            Some(PerfDelta {
                key: k.clone(),
                baseline: bv,
                capture: cv,
                delta: cv - bv,
                delta_pct: (bv != 0.0).then(|| (cv - bv) / bv.abs() * 100.0),
            })
        })
        .collect();
    out.sort_by(|a, b| {
        let mag = |p: &PerfDelta| p.delta_pct.map_or(0.0, f64::abs);
        mag(b).total_cmp(&mag(a)).then_with(|| a.key.cmp(&b.key))
    });
    out
}

fn num_text(v: f64) -> String {
    if v.abs() >= 100.0 {
        format!("{v:.0}")
    } else if v.abs() >= 10.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    }
}

/// `gpu_ms 4.85 → 2.71 (−44%)`, up to four keys joined by ` · `.
pub fn perf_summary(perf: &[PerfDelta]) -> Option<String> {
    if perf.is_empty() {
        return None;
    }
    let parts: Vec<String> = perf
        .iter()
        .take(4)
        .map(|p| {
            if p.baseline == p.capture {
                return format!("{} {} (same)", p.key, num_text(p.baseline));
            }
            let change = p.delta_pct.map_or(String::new(), |d| {
                let text = if d.abs() < 1.0 {
                    format!("{:.1}", d.abs())
                } else {
                    format!("{:.0}", d.abs())
                };
                format!(" ({}{text}%)", if d < 0.0 { '−' } else { '+' })
            });
            format!(
                "{} {} → {}{change}",
                p.key,
                num_text(p.baseline),
                num_text(p.capture)
            )
        })
        .collect();
    let more = perf.len().saturating_sub(4);
    let mut s = parts.join(" · ");
    if more > 0 {
        s.push_str(&format!(" · +{more} more"));
    }
    Some(s)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    const N: u32 = 192;

    /// Smooth, band-limited linear-light scene (value in about 0.2..0.7).
    fn scene(x: f64, y: f64) -> f64 {
        let t = 2.0 * std::f64::consts::PI;
        let blob = (-((x - 60.0).powi(2) + (y - 70.0).powi(2)) / 200.0).exp();
        0.4 + 0.1 * (t * (x / 41.0 + y / 29.0)).cos()
            + 0.08 * (t * (x / 17.0 - y / 23.0)).cos()
            + 0.06 * (t * y / 13.0).cos()
            + 0.15 * blob
    }

    fn encode(v: f64) -> u8 {
        (lin_to_srgb(v as f32) * 255.0).round() as u8
    }

    fn render(f: impl Fn(f64, f64) -> f64) -> RgbaImage {
        RgbaImage::from_fn(N, N, |x, y| {
            let v = f(f64::from(x), f64::from(y));
            Rgba([encode(v), encode(v * 0.9), encode(v * 0.8), 255])
        })
    }

    fn analyse(base: &RgbaImage, cap: &RgbaImage) -> Diagnostics {
        let opts = CompareOptions::default();
        let cmp = compare_rgba(cap, base, &opts).expect("compare");
        let hotspots = find_hotspots(&cmp.error_map, None, N, N, &HotspotOptions::default());
        let cfg = DiagnosticsConfig::default();
        let req = DiagnoseRequest {
            baseline: Pixels::Ldr(base),
            capture: Pixels::Ldr(cap),
            comparison: &cmp,
            flip: &opts,
            bit_identical: Some(false),
            baseline_properties: None,
            capture_properties: None,
            hotspots: &hotspots,
            hotspot_options: HotspotOptions::default(),
            config: &cfg,
            capture_path: None,
            out: None,
        };
        diagnose(&req).expect("diagnose").diagnostics
    }

    #[test]
    fn half_stop_exposure_shift_is_global_tone() {
        let base = render(scene);
        let cap = render(|x, y| scene(x, y) * 2f64.powf(-0.5));
        let d = analyse(&base, &cap);
        let tone = d.tone.as_ref().expect("tone fit");
        assert_eq!(d.class, ChangeClass::GlobalTone, "{}", d.description);
        assert!(
            (tone.exposure_stops + 0.5).abs() <= 0.05,
            "stops {}",
            tone.exposure_stops
        );
        assert!(tone.tone_explained_fraction >= 0.8);
    }

    #[test]
    fn one_and_half_pixel_shifts_are_found_to_a_tenth() {
        let base = render(scene);
        for (dx, dy) in [(1.0, 0.0), (0.5, 0.0)] {
            let cap = render(|x, y| scene(x - dx, y - dy));
            let d = analyse(&base, &cap);
            let s = d.shift.as_ref().expect("shift estimate");
            assert!(
                (s.dx - dx).abs() <= 0.1 && (s.dy - dy).abs() <= 0.1,
                "{s:?}"
            );
            assert!(s.detected, "{s:?}");
        }
    }

    #[test]
    fn a_local_blob_is_local_structure() {
        let base = render(scene);
        let cap = render(|x, y| {
            let patch = ((x - 150.0).powi(2) + (y - 150.0).powi(2)).sqrt() < 14.0;
            scene(x, y) + if patch { 0.3 } else { 0.0 }
        });
        let d = analyse(&base, &cap);
        assert_eq!(d.class, ChangeClass::LocalStructure, "{}", d.description);
        assert!(d.description.contains("bottom-right"), "{}", d.description);
    }

    #[test]
    fn small_noise_is_noise() {
        let base = render(scene);
        let mut cap = base.clone();
        let mut state = 12345u32;
        for p in cap.pixels_mut() {
            for c in 0..3 {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let n = ((state >> 24) % 3) as i32 - 1;
                p.0[c] = (i32::from(p.0[c]) + n).clamp(0, 255) as u8;
            }
        }
        let d = analyse(&base, &cap);
        assert_eq!(d.class, ChangeClass::Noise, "{}", d.description);
    }

    #[test]
    fn timing_keys_of_both_sidecars_are_paired() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (b, c) = (dir.path().join("b"), dir.path().join("c"));
        std::fs::create_dir_all(&b).expect("mkdir");
        std::fs::create_dir_all(&c).expect("mkdir");
        std::fs::write(
            b.join("flipdiff-meta.json"),
            r#"{"gpu_ms": 4.85, "frame_ms": 16.6, "driver": "a", "only_base_ms": 1}"#,
        )
        .expect("write");
        std::fs::write(
            c.join("flipdiff-meta.json"),
            r#"{"gpu_ms": 2.71, "frame_ms": 16.6, "driver": "b"}"#,
        )
        .expect("write");
        let checker = crate::meta::MetaOptions::default()
            .checker()
            .expect("checker");
        let perf = perf_pairs(&checker, &DiagnosticsConfig::default(), &b, &c, "a.png");
        let keys: Vec<&str> = perf.iter().map(|p| p.key.as_str()).collect();
        assert_eq!(keys, ["gpu_ms", "frame_ms"]);
        assert!((perf[0].delta_pct.expect("pct") + 44.1).abs() < 0.1);
        assert_eq!(
            perf_summary(&perf).as_deref(),
            Some("gpu_ms 4.85 → 2.71 (−44%) · frame_ms 16.6 (same)")
        );
    }

    #[test]
    fn description_names_a_shift_only_when_one_was_found() {
        let base = render(scene);
        let tone = render(|x, y| scene(x, y) * 0.707);
        let blob = render(|x, y| {
            scene(x, y)
                + if (x - 100.0).abs() < 12.0 && (y - 40.0).abs() < 12.0 {
                    0.3
                } else {
                    0.0
                }
        });
        for cap in [&tone, &blob] {
            let d = analyse(&base, cap);
            assert!(!d.description.contains("shifted"), "{}", d.description);
            assert!(!d.description.contains(" px "), "{}", d.description);
        }
        let moved = render(|x, y| scene(x - 0.5, y));
        let d = analyse(&base, &moved);
        assert!(
            d.description.contains("shifted 0.5 px right"),
            "{}",
            d.description
        );
    }
}
