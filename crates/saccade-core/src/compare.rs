//! FLIP comparison of two same-sized RGB images.

use crate::error::{Error, Result};
use crate::report::Metrics;

/// saccade's default viewing condition, kept at 67 pixels per degree for
/// compatibility. flip-rs's geometric default is slightly higher (67.02).
pub const DEFAULT_PIXELS_PER_DEGREE: f32 = 67.0;

/// Whether two image files hold exactly the same decoded samples at their
/// native bit depth and channel count (alpha included, NaN payloads and
/// negative values untouched). `false` when either file cannot be decoded.
pub fn native_samples_identical(a: &std::path::Path, b: &std::path::Path) -> bool {
    let (Ok(a), Ok(b)) = (image::open(a), image::open(b)) else {
        return false;
    };
    a.color() == b.color()
        && a.width() == b.width()
        && a.height() == b.height()
        && a.as_bytes() == b.as_bytes()
}

/// Options for [`compare`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompareOptions {
    /// FLIP observer setting (pixels per degree of visual angle).
    pub pixels_per_degree: f32,
    /// HDR-FLIP settings, used only by [`crate::hdr::compare_hdr`].
    pub hdr: crate::hdr::HdrConfig,
}

impl Default for CompareOptions {
    fn default() -> Self {
        Self {
            pixels_per_degree: DEFAULT_PIXELS_PER_DEGREE,
            hdr: crate::hdr::HdrConfig::default(),
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
        let Ok(flip_map) = flip_rs::ErrorMap::new(w as usize, h as usize, self.error_map.clone())
        else {
            return fallback();
        };
        let Ok(coloured) = flip_map.colorize() else {
            return fallback();
        };
        let coloured = coloured
            .into_pixels()
            .into_iter()
            .map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8)
            .collect();
        image::RgbImage::from_raw(w, h, coloured).unwrap_or_else(fallback)
    }
}

/// Composites `img` over a uniform grey background (`0` = black, `255` =
/// white) using straight alpha, dropping the alpha channel.
pub fn flatten_over(img: &image::RgbaImage, background: u8) -> image::RgbImage {
    let bg = u32::from(background);
    let mut out = image::RgbImage::new(img.width(), img.height());
    for (src, dst) in img.pixels().zip(out.pixels_mut()) {
        let a = u32::from(src.0[3]);
        for (d, &c) in dst.0.iter_mut().zip(&src.0[..3]) {
            *d = ((u32::from(c) * a + bg * (255 - a) + 127) / 255) as u8;
        }
    }
    out
}

/// Compares two images that may carry an alpha channel.
///
/// When both images are fully opaque this is [`compare`]. Otherwise both are
/// composited over black and over white, FLIP runs on each pair, and the error
/// map is the per-pixel maximum of the two. An alpha-only change is therefore
/// visible, and RGB hidden under alpha 0 is not.
///
/// Errors as [`compare`] does.
pub fn compare_rgba(
    capture: &image::RgbaImage,
    baseline: &image::RgbaImage,
    opts: &CompareOptions,
) -> Result<Comparison> {
    let opaque = |i: &image::RgbaImage| i.pixels().all(|p| p.0[3] == 255);
    let on_black = compare(&flatten_over(capture, 0), &flatten_over(baseline, 0), opts)?;
    if opaque(capture) && opaque(baseline) {
        return Ok(on_black);
    }
    let on_white = compare(
        &flatten_over(capture, 255),
        &flatten_over(baseline, 255),
        opts,
    )?;
    let error_map: Vec<f32> = on_black
        .error_map
        .iter()
        .zip(&on_white.error_map)
        .map(|(&a, &b)| if b > a || a.is_nan() { b } else { a })
        .collect();
    let (w, h) = capture.dimensions();
    let metrics = metrics_of(&error_map, w, h);
    Ok(Comparison { metrics, error_map })
}

/// Compares `capture` against `baseline` with NVIDIA FLIP (baseline is the
/// reference image). Alpha is not considered; use [`compare_rgba`] for images
/// that may be transparent.
///
/// Errors with [`Error::EmptyImage`] for a 0-pixel image,
/// [`Error::DimensionMismatch`] when the sizes differ and [`Error::Config`]
/// when `pixels_per_degree` is not finite and positive. Percentiles use
/// nearest-rank over the sorted error map, with every NaN mapped to +infinity
/// first so it sorts as the largest value.
pub fn compare(
    capture: &image::RgbImage,
    baseline: &image::RgbImage,
    opts: &CompareOptions,
) -> Result<Comparison> {
    check_ppd(opts.pixels_per_degree)?;
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

    // ldr_flip takes sRGB floats and performs the sRGB-to-linear conversion.
    // Dividing the decoded bytes by 255 preserves the previous input semantics.
    let srgb = |img: &image::RgbImage| {
        flip_rs::RgbImage::new(
            w as usize,
            h as usize,
            img.as_raw().iter().map(|&v| f32::from(v) / 255.0).collect(),
        )
    };
    let error_map =
        flip_rs::ldr_flip(&srgb(baseline)?, &srgb(capture)?, opts.pixels_per_degree)?.into_pixels();

    let metrics = metrics_of(&error_map, w, h);
    Ok(Comparison { metrics, error_map })
}

/// Rejects a pixels-per-degree that is not finite and positive.
pub(crate) fn check_ppd(ppd: f32) -> Result<()> {
    if ppd.is_finite() && ppd > 0.0 {
        Ok(())
    } else {
        Err(Error::Config(format!(
            "pixels per degree must be finite and > 0, got {ppd}"
        )))
    }
}

pub(crate) fn metrics_of(error_map: &[f32], width: u32, height: u32) -> Metrics {
    let n = error_map.len().max(1) as f64;
    let mean = error_map.iter().map(|&v| f64::from(v)).sum::<f64>() / n;
    let mut sorted: Vec<f32> = error_map
        .iter()
        .map(|&v| if v.is_nan() { f32::INFINITY } else { v })
        .collect();
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

/// Statistics of the error map over the rectangle `rect` (`[x, y, w, h]` in
/// pixels, inside the `width` x `height` frame), skipping pixels where `mask`
/// is `true`. Same nearest-rank percentiles and NaN rules as [`compare`]; the
/// returned `width`/`height` are the rectangle's. `None` when no pixel is left.
pub fn masked_metrics(
    error_map: &[f32],
    mask: Option<&[bool]>,
    width: u32,
    height: u32,
    rect: [u32; 4],
) -> Option<Metrics> {
    let [x, y, w, h] = rect;
    if error_map.len() != width as usize * height as usize
        || x.checked_add(w)? > width
        || y.checked_add(h)? > height
    {
        return None;
    }
    let mut kept = Vec::with_capacity(w as usize * h as usize);
    for row in y..y + h {
        let start = row as usize * width as usize + x as usize;
        let span = start..start + w as usize;
        for (i, &v) in span.clone().zip(&error_map[span]) {
            if !mask.is_some_and(|m| m.get(i).copied().unwrap_or(false)) {
                kept.push(v);
            }
        }
    }
    (!kept.is_empty()).then(|| metrics_of(&kept, w, h))
}

/// Paints masked pixels of `heatmap` as a neutral grey diagonal hatch, so they
/// read as "not measured" rather than as zero error.
pub fn hatch_masked(heatmap: &mut image::RgbImage, mask: &[bool]) {
    let w = heatmap.width() as usize;
    if w == 0 || mask.len() != w * heatmap.height() as usize {
        return;
    }
    for (i, px) in heatmap.pixels_mut().enumerate() {
        if mask[i] {
            let (x, y) = (i % w, i / w);
            let v = if (x + y) % 8 < 3 { 150 } else { 96 };
            *px = image::Rgb([v, v, v]);
        }
    }
}
