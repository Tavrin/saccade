//! Content-dependent no-reference SDR quality measurements; no heuristic quality verdict.
use serde::{Deserialize, Serialize};
/// Assessment evidence schema.
pub const SCHEMA: &str = "saccade-assess.v1";
/// Measured quality indicators in encoded 8-bit luminance/sample units.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Measures {
    /// Variance of the 4-neighbour Laplacian, luma-squared; lower can indicate blur or flat content.
    pub laplacian_variance: f64,
    /// Mean edge contrast divided by local maximum slope, in pixels; None if no qualifying edges.
    pub edge_width_px: Option<f64>,
    /// Robust MAD high-pass estimate over low-gradient regions, in luma units; None with too few samples.
    pub noise_sigma: Option<f64>,
    /// Excess mean adjacent-pixel difference at 8-pixel boundaries versus interiors; not proof of JPEG history.
    pub block_boundary_excess: Option<f64>,
    /// Fraction of occupied 8-bit luminance levels; small values can indicate posterisation or flat content.
    pub occupied_luma_fraction: f64,
    /// Fraction of adjacent shallow jumps (3..24 luma) surrounded by plateaus; banding candidate measure.
    pub shallow_plateau_step_fraction: f64,
    /// Fraction of pixels whose RGB channels all equal zero after compositing.
    pub black_clip_fraction: f64,
    /// Fraction of pixels whose RGB channels all equal 255 after compositing.
    pub white_clip_fraction: f64,
    /// Fraction of RGB channel samples equal to zero.
    pub low_channel_fraction: f64,
    /// Fraction of RGB channel samples equal to 255.
    pub high_channel_fraction: f64,
    /// Fraction of pixels with alpha below 255; compositing can affect all indicators.
    pub transparent_fraction: f64,
}
/// Measures an 8-bit SDR raster. Alpha is composited over white and explicitly counted.
/// Sampling bounds noise storage at 262144 values and edge-width work near 16384 candidates.
/// All thresholds are content-dependent; learned scores are deliberately unavailable.
pub fn assess(image: &image::RgbaImage) -> crate::Result<Measures> {
    let (w, h) = image.dimensions();
    let pixels = u64::from(w) * u64::from(h);
    if w < 3 || h < 3 || pixels > super::input::MAX_PIXELS {
        return Err(crate::Error::Config(
            "assessment needs >=3x3 pixels within raster limits".into(),
        ));
    }
    let rgb = crate::compare::flatten_over(image, 255);
    let gray = image::DynamicImage::ImageRgb8(rgb.clone()).to_luma8();
    let at = |x: u32, y: u32| f64::from(gray.get_pixel(x, y)[0]);
    let mut sum = 0.;
    let mut squared = 0.;
    let mut count = 0.;
    let mut residuals = Vec::new();
    let stride = ((pixels as f64 / 262144.).sqrt().ceil() as u32).max(1);
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let c = at(x, y);
            let n = [at(x - 1, y), at(x + 1, y), at(x, y - 1), at(x, y + 1)];
            let lap = n.iter().sum::<f64>() - 4. * c;
            sum += lap;
            squared += lap * lap;
            count += 1.;
            if x % stride == 0
                && y % stride == 0
                && n.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                    - n.iter().copied().fold(f64::INFINITY, f64::min)
                    <= 24.
            {
                residuals.push((lap / 4.).abs());
            }
        }
    }
    residuals.sort_by(f64::total_cmp);
    let noise_sigma = (residuals.len() >= 32)
        .then(|| residuals[residuals.len() / 2] / (0.67448975 * 1.25f64.sqrt()));
    let edge_stride = ((pixels as f64 / 16384.).sqrt().ceil() as u32).max(1);
    let mut widths = Vec::new();
    if w >= 9 && h >= 9 {
        for y in (4..h - 4).step_by(edge_stride as usize) {
            for x in (4..w - 4).step_by(edge_stride as usize) {
                let gx = (at(x + 1, y) - at(x - 1, y)).abs() / 2.;
                let gy = (at(x, y + 1) - at(x, y - 1)).abs() / 2.;
                if gx.max(gy) < 8. {
                    continue;
                }
                let horizontal = gx >= gy;
                let mut lo = f64::INFINITY;
                let mut hi = f64::NEG_INFINITY;
                let mut slope: f64 = 0.;
                for k in -3i32..=3 {
                    let (xx, yy) = if horizontal {
                        ((x as i32 + k) as u32, y)
                    } else {
                        (x, (y as i32 + k) as u32)
                    };
                    let v = at(xx, yy);
                    lo = lo.min(v);
                    hi = hi.max(v);
                    let g = if horizontal {
                        (at(xx + 1, yy) - at(xx - 1, yy)).abs() / 2.
                    } else {
                        (at(xx, yy + 1) - at(xx, yy - 1)).abs() / 2.
                    };
                    slope = slope.max(g);
                }
                if slope > 0. {
                    widths.push((hi - lo) / slope);
                }
            }
        }
    }
    let edge_width_px =
        (!widths.is_empty()).then(|| widths.iter().sum::<f64>() / widths.len() as f64);
    let mut boundary = 0.;
    let mut inside = 0.;
    let mut bn = 0u64;
    let mut inn = 0u64;
    let mut plateau = 0u64;
    let mut adjacent = 0u64;
    for y in 0..h {
        for x in 1..w {
            let d = (at(x, y) - at(x - 1, y)).abs();
            if x % 8 == 0 {
                boundary += d;
                bn += 1;
            } else {
                inside += d;
                inn += 1;
            }
            adjacent += 1;
            if x >= 2
                && x + 1 < w
                && (3.0..=24.).contains(&d)
                && at(x - 1, y) == at(x - 2, y)
                && at(x, y) == at(x + 1, y)
            {
                plateau += 1;
            }
        }
    }
    for x in 0..w {
        for y in 1..h {
            let d = (at(x, y) - at(x, y - 1)).abs();
            if y % 8 == 0 {
                boundary += d;
                bn += 1;
            } else {
                inside += d;
                inn += 1;
            }
            adjacent += 1;
            if y >= 2
                && y + 1 < h
                && (3.0..=24.).contains(&d)
                && at(x, y - 1) == at(x, y - 2)
                && at(x, y) == at(x, y + 1)
            {
                plateau += 1;
            }
        }
    }
    let block_boundary_excess =
        (bn > 0 && inn > 0).then(|| (boundary / bn as f64 - inside / inn as f64).max(0.));
    let mut bins = [false; 256];
    for p in gray.pixels() {
        bins[p[0] as usize] = true;
    }
    let mut black = 0;
    let mut white = 0;
    let mut low = 0;
    let mut high = 0;
    for p in rgb.pixels() {
        black += u64::from(p.0 == [0; 3]);
        white += u64::from(p.0 == [255; 3]);
        for v in p.0 {
            low += u64::from(v == 0);
            high += u64::from(v == 255);
        }
    }
    let denominator = pixels as f64;
    Ok(Measures {
        laplacian_variance: (squared / count - (sum / count).powi(2)).max(0.),
        edge_width_px,
        noise_sigma,
        block_boundary_excess,
        occupied_luma_fraction: bins.iter().filter(|&&v| v).count() as f64 / 256.,
        shallow_plateau_step_fraction: plateau as f64 / adjacent as f64,
        black_clip_fraction: black as f64 / denominator,
        white_clip_fraction: white as f64 / denominator,
        low_channel_fraction: low as f64 / (denominator * 3.),
        high_channel_fraction: high as f64 / (denominator * 3.),
        transparent_fraction: image.pixels().filter(|p| p[3] < 255).count() as f64 / denominator,
    })
}
#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    #[test]
    fn blur_lowers_laplacian_and_widens_edges() {
        let sharp = image::RgbaImage::from_fn(64, 64, |x, _| {
            image::Rgba(if x < 32 { [0, 0, 0, 255] } else { [255; 4] })
        });
        let blurred = image::imageops::blur(&sharp, 2.);
        let a = assess(&sharp).expect("sharp");
        let b = assess(&blurred).expect("blur");
        assert!(b.laplacian_variance < a.laplacian_variance);
        assert!(b.edge_width_px.expect("edge") > a.edge_width_px.expect("edge"));
    }
    #[test]
    fn noise_in_flat_regions_is_measured() {
        let flat = image::RgbaImage::from_pixel(64, 64, image::Rgba([128, 128, 128, 255]));
        let mut state = 98765u64;
        let noisy = image::RgbaImage::from_fn(64, 64, |_, _| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let v = (128i32 + (state % 17) as i32 - 8) as u8;
            image::Rgba([v, v, v, 255])
        });
        assert!(
            assess(&noisy).expect("noise").noise_sigma.expect("sigma")
                > assess(&flat).expect("flat").noise_sigma.expect("sigma")
        );
    }
    #[test]
    fn block_boundaries_and_posterised_plateaus_are_not_verdicts() {
        let blocks = image::RgbaImage::from_fn(64, 32, |x, _| {
            let v = ((x / 8) * 8 + 64) as u8;
            image::Rgba([v, v, v, 255])
        });
        let m = assess(&blocks).expect("blocks");
        assert!(m.block_boundary_excess.expect("blockiness") > 0.);
        assert!(m.shallow_plateau_step_fraction > 0.);
        assert_eq!(m.occupied_luma_fraction, 8. / 256.);
    }
    #[test]
    fn clipping_and_transparency_are_explicit() {
        let black = image::RgbaImage::from_pixel(8, 8, image::Rgba([0, 0, 0, 255]));
        let m = assess(&black).expect("black");
        assert_eq!(m.black_clip_fraction, 1.);
        assert_eq!(m.white_clip_fraction, 0.);
        assert_eq!(m.edge_width_px, None);
        let invisible = image::RgbaImage::from_pixel(8, 8, image::Rgba([0, 0, 0, 0]));
        let m = assess(&invisible).expect("alpha");
        assert_eq!(m.transparent_fraction, 1.);
        assert_eq!(m.white_clip_fraction, 1.);
    }
}
