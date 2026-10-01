//! FLIP comparison of two same-sized RGB images.

use crate::error::{Error, Result};
use crate::report::Metrics;

/// Options for [`compare`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompareOptions {
    /// FLIP observer setting (pixels per degree of visual angle).
    pub pixels_per_degree: f32,
}

impl Default for CompareOptions {
    fn default() -> Self {
        Self {
            pixels_per_degree: nv_flip::DEFAULT_PIXELS_PER_DEGREE,
        }
    }
}

/// Result of [`compare`]: summary statistics plus the per-pixel error map.
#[derive(Debug, Clone)]
pub struct Comparison {
    /// Summary statistics of the error map.
    pub metrics: Metrics,
    /// FLIP error per pixel in `[0, 1]`, row-major, `width * height` values.
    pub error_map: Vec<f32>,
}

impl Comparison {
    /// Renders the error map through FLIP's magma colour map.
    pub fn heatmap_rgb(&self) -> image::RgbImage {
        let (w, h) = (self.metrics.width, self.metrics.height);
        let fallback = || image::RgbImage::new(w, h);
        if self.error_map.len() != w as usize * h as usize || w == 0 || h == 0 {
            return fallback();
        }
        let flip_map = nv_flip::FlipImageFloat::with_data(w, h, &self.error_map);
        let coloured = flip_map.apply_color_lut(&nv_flip::magma_lut());
        image::RgbImage::from_raw(w, h, coloured.to_vec()).unwrap_or_else(fallback)
    }
}

/// Compares `capture` against `baseline` with NVIDIA FLIP (baseline is the
/// reference image).
///
/// Errors with [`Error::EmptyImage`] for a 0-pixel image and
/// [`Error::DimensionMismatch`] when the sizes differ. Percentiles use
/// nearest-rank over the sorted error map, with NaN sorted as the largest value.
pub fn compare(
    capture: &image::RgbImage,
    baseline: &image::RgbImage,
    opts: &CompareOptions,
) -> Result<Comparison> {
    let (w, h) = capture.dimensions();
    let (bw, bh) = baseline.dimensions();
    if w == 0 || h == 0 || bw == 0 || bh == 0 {
        return Err(Error::EmptyImage);
    }
    if (w, h) != (bw, bh) {
        return Err(Error::DimensionMismatch {
            test_w: w,
            test_h: h,
            ref_w: bw,
            ref_h: bh,
        });
    }

    // `RgbImage` is tightly packed RGB, which is what FlipImageRgb8 expects.
    let flip_ref = nv_flip::FlipImageRgb8::with_data(w, h, baseline.as_raw());
    let flip_test = nv_flip::FlipImageRgb8::with_data(w, h, capture.as_raw());
    let error_map = nv_flip::flip(flip_ref, flip_test, opts.pixels_per_degree).to_vec();

    let metrics = metrics_of(&error_map, w, h);
    Ok(Comparison { metrics, error_map })
}

fn metrics_of(error_map: &[f32], width: u32, height: u32) -> Metrics {
    let n = error_map.len().max(1) as f64;
    let mean = error_map.iter().map(|&v| f64::from(v)).sum::<f64>() / n;
    let mut sorted = error_map.to_vec();
    sorted.sort_unstable_by(f32::total_cmp);
    let pct = |p: f64| -> f64 {
        let rank = (p * sorted.len() as f64).ceil() as usize;
        let idx = rank.clamp(1, sorted.len().max(1)) - 1;
        sorted.get(idx).copied().map_or(0.0, f64::from)
    };
    let frac_above = |t: f32| error_map.iter().filter(|&&v| v > t).count() as f64 / n;
    Metrics {
        mean,
        max: pct(1.0),
        p50: pct(0.50),
        p95: pct(0.95),
        p99: pct(0.99),
        frac_above_0_1: frac_above(0.1),
        frac_above_0_5: frac_above(0.5),
        width,
        height,
    }
}
