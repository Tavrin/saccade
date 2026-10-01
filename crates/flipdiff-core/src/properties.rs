//! Structural sanity checks on a single image.

use crate::report::Properties;

/// Computes black/white/luminance statistics for `img`.
///
/// Luminance is Rec. 709 (`0.2126 R + 0.7152 G + 0.0722 B`) applied directly to
/// the sRGB-encoded bytes divided by 255. It is deliberately **not**
/// linearised: the checks catch black frames and blowouts, not photometry.
/// An empty image reports all-zero luminance and neither flag.
pub fn validate(img: &image::RgbImage) -> Properties {
    let count = img.pixels().len();
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
    let (mut black, mut white) = (true, true);
    let (mut sum, mut min, mut max) = (0.0f64, f32::INFINITY, f32::NEG_INFINITY);
    for p in img.pixels() {
        let [r, g, b] = p.0;
        black &= r == 0 && g == 0 && b == 0;
        white &= r == 255 && g == 255 && b == 255;
        let lum = 0.2126 * (f32::from(r) / 255.0)
            + 0.7152 * (f32::from(g) / 255.0)
            + 0.0722 * (f32::from(b) / 255.0);
        sum += f64::from(lum);
        min = min.min(lum);
        max = max.max(lum);
    }
    Properties {
        is_all_black: black,
        is_all_white: white,
        mean_luminance: (sum / count as f64) as f32,
        min_luminance: min,
        max_luminance: max,
        nan_count: 0,
        inf_count: 0,
        negative_count: 0,
    }
}
