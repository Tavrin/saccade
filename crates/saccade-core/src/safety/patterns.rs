//! Regular-pattern pre-check: scanline autocorrelation via Wiener-Khinchin.
//!
//! ITU-R BT.1702-3 Annex 1 Attachment 1 pattern guidance (see THRESHOLDS). This detector
//! conservatively treats any changing pattern as the five-pair case. It does
//! not establish smooth one-direction motion or replace a laboratory detector.

use super::thresholds::value;
use rustfft::{FftPlanner, num_complex::Complex};

fn periodic(samples: &[f64], limit: f64) -> bool {
    if samples.len() < 20 {
        return false;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let lo = sorted[sorted.len() / 10];
    let hi = sorted[sorted.len() * 9 / 10];
    // Attachment 1 uses the same luminance difference as Guideline 1.
    if (hi - lo) * value("broadcast_peak_cd_m2") < value("broadcast_change_cd_m2")
        || lo * value("broadcast_peak_cd_m2") >= value("broadcast_darker_cd_m2")
    {
        return false;
    }
    let mid = (lo + hi) / 2.0;
    let mut light = samples[0] > mid;
    let mut edges = 0;
    for &v in &samples[1..] {
        let next = if v >= mid + (hi - lo) * 0.1 {
            true
        } else if v <= mid - (hi - lo) * 0.1 {
            false
        } else {
            light
        };
        if next != light {
            edges += 1;
            light = next;
        }
    }
    if f64::from(edges) / 2.0 <= limit {
        return false;
    }
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    let n = (samples.len() * 2).next_power_of_two();
    let mut data = vec![Complex::new(0.0, 0.0); n];
    for (dst, &v) in data.iter_mut().zip(samples) {
        dst.re = v - mean;
    }
    let mut planner = FftPlanner::<f64>::new();
    planner.plan_fft_forward(n).process(&mut data);
    for v in &mut data {
        *v = Complex::new(v.norm_sqr(), 0.0);
    }
    planner.plan_fft_inverse(n).process(&mut data);
    let energy = data[0].re;
    if energy <= 0.0 {
        return false;
    }
    // Search local maxima, excluding lag 0 and monotone/near-zero-lag fits.
    let max_lag = (samples.len() as f64 / (limit + 1.0)).floor() as usize;
    (2..max_lag.min(samples.len() / 2)).any(|lag| {
        let corr = data[lag].re / energy * samples.len() as f64 / (samples.len() - lag) as f64;
        corr >= value("periodicity_correlation")
            && data[lag].re >= data[lag - 1].re
            && data[lag].re > data[lag + 1].re
    })
}

pub(crate) fn mask(luma: &[f64], width: usize, height: usize, changing: bool) -> Vec<bool> {
    let mut mask = vec![false; luma.len()];
    let limit = value(if changing {
        "pattern_changing_pairs"
    } else {
        "pattern_static_pairs"
    });
    // Sample each 8-pixel band. Mark its bounding scanline area conservatively.
    // Horizontal and vertical profiles also detect oblique stripes and grids
    // when a scanline resolves enough light/dark pairs.
    for y in (0..height).step_by(8) {
        let samples = &luma[y * width..(y + 1) * width];
        if periodic(samples, limit) {
            for row in y..(y + 8).min(height) {
                mask[row * width..(row + 1) * width].fill(true);
            }
        }
    }
    for x in (0..width).step_by(8) {
        let samples: Vec<_> = (0..height).map(|y| luma[y * width + x]).collect();
        if periodic(&samples, limit) {
            for col in x..(x + 8).min(width) {
                for y in 0..height {
                    mask[y * width + col] = true;
                }
            }
        }
    }
    mask
}
