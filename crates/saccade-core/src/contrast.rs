//! Reusable linear-luminance and robust foreground/background measurements.
use crate::{Error, Result, color};
use image::RgbImage;
use std::collections::BTreeMap;
/// Two-colour fit, with unsupported evidence kept absent.
#[derive(Debug, Clone)]
pub struct Estimate {
    /// Minority-cluster sRGB swatch.
    pub foreground: Option<[u8; 3]>,
    /// Majority-cluster sRGB swatch.
    pub background: Option<[u8; 3]>,
    /// Relative luminance ratio, absent for unsupported pixels.
    pub ratio: Option<f64>,
    /// Method and abstention reason.
    pub note: String,
}
/// Relative luminance ratio of two linear RGB colours; no policy threshold.
pub fn contrast_ratio(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (a, b) = (color::luminance(a), color::luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// Two-cluster component-median estimator over a fractional image rectangle.
pub fn estimate(image: &RgbImage, rect: [f64; 4]) -> Result<Estimate> {
    let [x, y, w, h] = crate::regions::resolve_rect(rect, image.width(), image.height())
        .ok_or_else(|| Error::Config("contrast region resolves to zero pixels".into()))?;
    let mut result=Estimate {foreground:None,background:None,ratio:None,note:"Deterministic two-cluster fit with component medians; minority is estimated foreground. No OCR, font-size inference, or automatic WCAG applicability judgement.".into()};
    let pixels: Vec<_> = (y..y + h)
        .flat_map(|y| (x..x + w).map(move |x| color::rgb(image.get_pixel(x, y).0)))
        .collect();
    if pixels.len() < 2 {
        result.note.push_str(" Insufficient pixels.");
        return Ok(result);
    }
    let dist =
        |a: [f64; 3], b: [f64; 3]| a.iter().zip(b).map(|(a, b)| (a - b).powi(2)).sum::<f64>();
    // Mode in 5-bit/channel histogram seeds background; farthest real pixel
    // seeds foreground. Medians limit antialias/outlier influence.
    let mut hist: BTreeMap<[u8; 3], (usize, [f64; 3])> = BTreeMap::new();
    for &p in &pixels {
        let bucket = color::bytes(p).map(|v| v / 8);
        let e = hist.entry(bucket).or_insert((0, p));
        e.0 += 1;
    }
    let seed = hist.values().max_by_key(|v| v.0).map_or(pixels[0], |v| v.1);
    let far = pixels
        .iter()
        .max_by(|a, b| dist(**a, seed).total_cmp(&dist(**b, seed)))
        .copied()
        .unwrap_or(seed);
    let mut centres = [seed, far];
    let mut clusters = [Vec::new(), Vec::new()];
    for _ in 0..12 {
        clusters.iter_mut().for_each(Vec::clear);
        for &p in &pixels {
            let i = usize::from(dist(p, centres[1]) < dist(p, centres[0]));
            clusters[i].push(p);
        }
        if clusters.iter().any(Vec::is_empty) {
            result.note.push_str(" No two supported dominant colours.");
            return Ok(result);
        }
        let next = std::array::from_fn(|i| {
            std::array::from_fn(|c| {
                let mut channel: Vec<_> = clusters[i].iter().map(|p| p[c]).collect();
                channel.sort_by(f64::total_cmp);
                channel[channel.len() / 2]
            })
        });
        if next == centres {
            break;
        }
        centres = next;
    }
    let minority = usize::from(clusters[1].len() < clusters[0].len());
    let coverage = clusters[minority].len() as f64 / pixels.len() as f64;
    let residual = clusters
        .iter()
        .enumerate()
        .map(|(i, ps)| ps.iter().map(|&p| dist(p, centres[i])).sum::<f64>())
        .sum::<f64>()
        / pixels.len() as f64;
    if coverage < 0.01 || residual.sqrt() > 0.08 {
        result.note.push_str(" Weak minority support (<1%) or diffuse colours (linear RGB RMS >0.08); confirm a tighter two-colour region.");
        return Ok(result);
    }
    let ratio = contrast_ratio(centres[0], centres[1]);
    result.ratio = Some(ratio);
    result.foreground = Some(color::bytes(centres[minority]));
    result.background = Some(color::bytes(centres[1 - minority]));
    Ok(result)
}
