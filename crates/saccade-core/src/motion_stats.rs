//! Independent motion diagnostics; distances are not a perceptual quality score.
use crate::{Error, Result, dense_motion, frame_map};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::f64::consts::TAU;

/// Objective report discriminator.
pub const SCHEMA: &str = "saccade-motion-stats.v1";
/// Calibration report discriminator.
pub const CALIBRATION_SCHEMA: &str = "saccade-motion-calibration.v1";
/// Validated in-memory sequence, with actual presentation timestamps.
pub struct Clip {
    /// Decoded opaque SDR captures.
    pub frames: Vec<image::RgbaImage>,
    /// Producer timestamp and source-index contract.
    pub map: frame_map::FrameMap,
}
/// Separate scalar measurements and empirical histograms.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Statistics {
    /// Values keyed by measurement and unit. Unavailable measurements are absent.
    pub values: BTreeMap<String, f64>,
    /// Magnitude histogram edges [0, 15, 30, 60, 120, 240, 480, infinity], pixels/s.
    pub magnitude: Option<Vec<f64>>,
    /// Eight equal direction bins from -pi to pi, radians; speed > 0.1 pixels/step.
    pub direction: Option<Vec<f64>>,
    /// DC-removed per-pixel average power in [0,2), [2,6), [6,Nyquist] Hz.
    pub spectrum: Option<Vec<f64>>,
    /// Timing or flow qualifications, never hidden substitutions.
    pub limitations: Vec<String>,
}
/// Paired evidence with independent reference distances.
#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    /// Versioned discriminator.
    pub schema: String,
    /// Reference diagnostics.
    pub reference: Statistics,
    /// Candidate diagnostics.
    pub candidate: Statistics,
    /// Absolute scalar distances; histogram JS in nats, spectrum L1 in intensity squared.
    pub distances: BTreeMap<String, f64>,
}
fn invalid(s: &str) -> Error {
    Error::Config(s.into())
}
fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len().max(1) as f64
}
fn normalize(v: Vec<f64>) -> Option<Vec<f64>> {
    let sum: f64 = v.iter().sum();
    (sum > 0.0).then(|| v.into_iter().map(|x| x / sum).collect())
}
/// Validate bounded clips before computation; no implicit resampling.
pub fn validate(c: &Clip) -> Result<()> {
    frame_map::check(&c.map, None, frame_map::CheckPolicy::default(), None)?;
    if !(4..=128).contains(&c.frames.len()) || c.frames.len() != c.map.frames.len() {
        return Err(invalid("motion statistics require 4..128 mapped frames"));
    }
    let d = c.frames[0].dimensions();
    if !(16..=256).contains(&d.0)
        || !(16..=256).contains(&d.1)
        || c.frames
            .iter()
            .any(|i| i.dimensions() != d || i.pixels().any(|p| p[3] != 255))
    {
        return Err(invalid(
            "motion statistics require equal opaque SDR dimensions 16..256",
        ));
    }
    Ok(())
}
/// Measure all selected pixels; mask is a static binary inclusion region.
pub fn measure(c: &Clip, mask: Option<&[bool]>) -> Result<Statistics> {
    validate(c)?;
    let w = c.frames[0].width() as usize;
    let pixels = w * c.frames[0].height() as usize;
    if mask.is_some_and(|m| m.len() != pixels || !m.iter().any(|v| *v)) {
        return Err(invalid(
            "mask requires matching dimensions and nonempty inclusion",
        ));
    }
    let selected: Vec<usize> = (0..pixels).filter(|&i| mask.is_none_or(|m| m[i])).collect();
    let luminance: Vec<Vec<f64>> = c
        .frames
        .iter()
        .map(|im| {
            selected
                .iter()
                .map(|&i| {
                    let p = im.as_raw();
                    let i = i * 4;
                    (0.2126 * p[i] as f64 + 0.7152 * p[i + 1] as f64 + 0.0722 * p[i + 2] as f64)
                        / 255.0
                })
                .collect()
        })
        .collect();
    let intervals: Vec<f64> = c
        .map
        .frames
        .windows(2)
        .map(|p| p[1].timestamp_s - p[0].timestamp_s)
        .collect();
    let mut sorted = intervals.clone();
    sorted.sort_by(f64::total_cmp);
    let step = sorted[sorted.len() / 2];
    let check = frame_map::check(&c.map, None, frame_map::CheckPolicy::default(), None)?;
    let mut values = BTreeMap::new();
    values.insert("median_interval_s".into(), step);
    let source_step = check.timing.median_step_s.unwrap_or(step);
    values.insert(
        "hitch_fraction".into(),
        c.map
            .frames
            .windows(2)
            .filter(|p| {
                (p[1].timestamp_s - p[0].timestamp_s) / (p[1].index - p[0].index) as f64
                    > source_step * 1.5
            })
            .count() as f64
            / intervals.len() as f64,
    );
    values.insert(
        "interval_cv".into(),
        (mean(
            &intervals
                .iter()
                .map(|s| (s - mean(&intervals)).powi(2))
                .collect::<Vec<_>>(),
        ))
        .sqrt()
            / mean(&intervals),
    );
    values.insert(
        "missing_frames".into(),
        c.map
            .frames
            .windows(2)
            .map(|p| p[1].index - p[0].index - 1)
            .sum::<u64>() as f64,
    );
    let mut changes = Vec::new();
    let mut duplicates = 0;
    let mut frozen = 0;
    for (t, p) in luminance.windows(2).enumerate() {
        let d = mean(
            &p[0]
                .iter()
                .zip(&p[1])
                .map(|(a, b)| (b - a).abs())
                .collect::<Vec<_>>(),
        );
        changes.push(d);
        duplicates += usize::from(selected.iter().all(|&i| {
            c.frames[t].as_raw()[i * 4..i * 4 + 4] == c.frames[t + 1].as_raw()[i * 4..i * 4 + 4]
        }));
        frozen += usize::from(d <= 1.0 / 255.0);
    }
    values.insert(
        "duplicate_fraction".into(),
        duplicates as f64 / changes.len() as f64,
    );
    values.insert(
        "frozen_fraction".into(),
        frozen as f64 / changes.len() as f64,
    );
    values.insert("mean_change_intensity".into(), mean(&changes));
    let second: Vec<f64> = luminance
        .windows(3)
        .flat_map(|p| (0..selected.len()).map(move |i| (p[2][i] - 2.0 * p[1][i] + p[0][i]).powi(2)))
        .collect();
    values.insert(
        "flicker_second_difference_intensity_squared".into(),
        mean(&second),
    );
    let mut limitations = vec!["SDR encoded luma, content-dependent diagnostics; no naturalness verdict. DIS qualified-flow coverage is reported; zero support is unavailable.".into()];
    let spectrum = if check.timing.state == "constant" && check.missing_total == 0 {
        let n = luminance.len();
        let stride = selected.len().div_ceil(4096);
        let mut bands = vec![0.0; 3];
        let mut count = 0;
        for i in (0..selected.len()).step_by(stride) {
            count += 1;
            let dc = mean(&luminance.iter().map(|f| f[i]).collect::<Vec<_>>());
            for k in 1..=n / 2 {
                let (mut re, mut im) = (0.0, 0.0);
                for (t, f) in luminance.iter().enumerate() {
                    let angle = TAU * k as f64 * t as f64 / n as f64;
                    re += (f[i] - dc) * angle.cos();
                    im += (f[i] - dc) * angle.sin();
                }
                let hz = k as f64 / (n as f64 * step);
                let b = if hz < 2.0 {
                    0
                } else if hz < 6.0 {
                    1
                } else {
                    2
                };
                let factor = if n.is_multiple_of(2) && k == n / 2 {
                    1.0
                } else {
                    2.0
                };
                bands[b] += factor * (re * re + im * im) / (n * n) as f64;
            }
        }
        for b in &mut bands {
            *b /= count as f64;
        }
        values.insert("high_frequency_power_intensity_squared".into(), bands[2]);
        Some(bands)
    } else {
        limitations.push("Spectrum unavailable: timestamp sampling must be uniform and contiguous; no resampling.".into());
        None
    };
    let mut mags = vec![0.0; 7];
    let mut dirs = vec![0.0; 8];
    let mut speeds = Vec::new();
    let mut coherence = Vec::new();
    let mut support = 0;
    if cfg!(feature = "dense-motion") {
        for (t, pair) in c.frames.windows(2).enumerate() {
            let f = dense_motion::correspondence(&pair[0], &pair[1])?;
            for &i in &selected {
                if f.states[i] != dense_motion::State::Qualified {
                    continue;
                }
                support += 1;
                let [x, y] = f.vectors[i];
                let length = (x * x + y * y).sqrt() as f64;
                let speed = length / intervals[t];
                speeds.push(speed);
                let bin = [15.0, 30.0, 60.0, 120.0, 240.0, 480.0]
                    .iter()
                    .position(|&edge| speed < edge)
                    .unwrap_or(6);
                mags[bin] += 1.0;
                if length > 0.1 {
                    let bin = (((y as f64).atan2(x as f64) + std::f64::consts::PI) / TAU * 8.0)
                        .floor() as usize;
                    dirs[bin.min(7)] += 1.0;
                }
                if i % w + 1 < w
                    && mask.is_none_or(|m| m[i + 1])
                    && f.states[i + 1] == dense_motion::State::Qualified
                {
                    let v = f.vectors[i + 1];
                    coherence.push(
                        ((x - v[0]).powi(2) + (y - v[1]).powi(2)).sqrt() as f64 / intervals[t],
                    );
                }
            }
        }
    } else {
        limitations.push("Flow unavailable: enable dense-motion.".into());
    }
    values.insert(
        "flow_qualified_fraction".into(),
        support as f64 / (selected.len() * intervals.len()) as f64,
    );
    if !speeds.is_empty() {
        values.insert("flow_mean_px_per_s".into(), mean(&speeds));
    }
    if !coherence.is_empty() {
        values.insert("flow_neighbor_difference_px_per_s".into(), mean(&coherence));
    }
    Ok(Statistics {
        values,
        magnitude: normalize(mags),
        direction: normalize(dirs),
        spectrum,
        limitations,
    })
}
fn js(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(&x, &y)| {
            let m = (x + y) * 0.5;
            (if x > 0.0 { x * (x / m).ln() } else { 0.0 }) * 0.5
                + (if y > 0.0 { y * (y / m).ln() } else { 0.0 }) * 0.5
        })
        .sum()
}
/// Compare independently measured statistics; unavailable evidence stays absent.
pub fn compare(reference: Statistics, candidate: Statistics) -> Report {
    let mut distances: BTreeMap<String, f64> = reference
        .values
        .iter()
        .filter_map(|(k, v)| candidate.values.get(k).map(|b| (k.clone(), (v - b).abs())))
        .collect();
    for (name, a, b) in [
        (
            "magnitude_js_nats",
            &reference.magnitude,
            &candidate.magnitude,
        ),
        (
            "direction_js_nats",
            &reference.direction,
            &candidate.direction,
        ),
    ] {
        if let (Some(a), Some(b)) = (a, b) {
            distances.insert(name.into(), js(a, b));
        }
    }
    if let (Some(a), Some(b)) = (&reference.spectrum, &candidate.spectrum) {
        distances.insert(
            "spectrum_l1_intensity_squared".into(),
            a.iter().zip(b).map(|(a, b)| (a - b).abs()).sum(),
        );
        for i in 0..3 {
            if a[i] > 0.0 {
                distances.insert(format!("spectral_band_{i}_ratio"), b[i] / a[i]);
            }
        }
    }
    Report {
        schema: SCHEMA.into(),
        reference,
        candidate,
        distances,
    }
}
/// Supported deterministic degradation classes (controls included).
pub const CLASSES: &[&str] = &[
    "frozen",
    "flicker",
    "speed_up",
    "speed_down",
    "looped_hitch",
    "overlay_blobs",
    "temporal_blur",
    "frame_drops",
    "spatial_blur",
    "spatial_noise",
];
/// Construct a negative. Strength in (0,1], PRNG seed and transform are explicit.
pub fn degrade(c: &Clip, class: &str, strength: f64, seed: u64) -> Result<Clip> {
    validate(c)?;
    if !CLASSES.contains(&class)
        || !strength.is_finite()
        || !(0.0..=1.0).contains(&strength)
        || strength == 0.0
    {
        return Err(invalid("unknown degradation or strength outside (0,1]"));
    }
    let mut out = Clip {
        frames: c.frames.clone(),
        map: c.map.clone(),
    };
    let n = c.frames.len();
    let mut rng = seed;
    for t in 0..n {
        match class {
            "frozen" => {
                if t >= ((1.0 - strength) * (n - 1) as f64) as usize {
                    out.frames[t] = c.frames[((1.0 - strength) * (n - 1) as f64) as usize].clone();
                }
            }
            "speed_up" | "speed_down" => {
                let factor = if class == "speed_up" {
                    1.0 + strength
                } else {
                    1.0 / (1.0 + strength)
                };
                out.map.frames[t].timestamp_s = c.map.frames[0].timestamp_s
                    + (c.map.frames[t].timestamp_s - c.map.frames[0].timestamp_s) / factor;
                out.map.nominal_fps = c.map.nominal_fps.map(|fps| fps * factor);
            }
            "looped_hitch" => {
                let period = 8;
                let hold = (strength * 4.0).ceil() as usize;
                if t % period < hold {
                    out.frames[t] = c.frames[t / period * period].clone();
                }
                if t > 0 {
                    out.map.frames[t].timestamp_s = out.map.frames[t - 1].timestamp_s
                        + (c.map.frames[t].timestamp_s - c.map.frames[t - 1].timestamp_s)
                            * if t % period == hold {
                                1.0 + strength * 4.0
                            } else {
                                1.0
                            };
                }
            }
            "temporal_blur" => {
                if t > 0 {
                    for (p, b) in out.frames[t].pixels_mut().zip(c.frames[t - 1].pixels()) {
                        for k in 0..3 {
                            p[k] = ((1.0 - strength * 0.5) * p[k] as f64
                                + strength * 0.5 * b[k] as f64)
                                .round() as u8;
                        }
                    }
                }
            }
            "spatial_blur" => {
                out.frames[t] = image::imageops::blur(&c.frames[t], (strength * 3.0) as f32);
            }
            "flicker" | "spatial_noise" => {
                for p in out.frames[t].pixels_mut() {
                    for k in 0..3 {
                        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                        let change = if class == "flicker" {
                            if t.is_multiple_of(2) { 1.0 } else { -1.0 }
                        } else {
                            (rng >> 32) as f64 / u32::MAX as f64 * 2.0 - 1.0
                        };
                        p[k] = (p[k] as f64 + change * strength * 64.0)
                            .round()
                            .clamp(0.0, 255.0) as u8;
                    }
                }
            }
            "overlay_blobs" => {
                let (w, h) = out.frames[t].dimensions();
                let radius = (strength * w.min(h) as f64 / 4.0).ceil() as i64;
                let cx = ((seed % u64::from(w)) as usize + t * 3) % w as usize;
                let cy = ((seed.rotate_left(17) % u64::from(h)) as usize + t * 2) % h as usize;
                for (x, y, p) in out.frames[t].enumerate_pixels_mut() {
                    if (x as i64 - cx as i64).pow(2) + (y as i64 - cy as i64).pow(2)
                        <= radius * radius
                    {
                        *p = image::Rgba([240, 240, 240, 255]);
                    }
                }
            }
            _ => {}
        }
    }
    if class == "frame_drops" {
        let period = (1.0 / strength).ceil() as usize + 1;
        let keep: Vec<usize> = (0..n).filter(|i| i % period != period - 1).collect();
        out.frames = keep.iter().map(|&i| out.frames[i].clone()).collect();
        out.map.frames = keep.iter().map(|&i| out.map.frames[i].clone()).collect();
    }
    validate(&out)?;
    if out.frames == c.frames
        && out.map.frames.iter().map(|f| (f.index, f.timestamp_s)).eq(c
            .map
            .frames
            .iter()
            .map(|f| (f.index, f.timestamp_s)))
    {
        return Err(invalid(
            "degradation has no effect at this strength on this source; not a known negative",
        ));
    }
    Ok(out)
}
/// A paired higher-is-better scorer observation; ties count half in accuracy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Score {
    /// Stable generated negative identifier.
    pub id: String,
    /// Independent metric/scorer name.
    pub scorer: String,
    /// Known degradation class.
    pub class: String,
    /// Declared transform strength.
    pub strength: f64,
    /// Positive score.
    pub positive: f64,
    /// Negative score.
    pub negative: f64,
}
/// Compute stratified paired accuracy and exact binomial bounds on strict wins.
/// Trust requires the lower 95% two-sided bound to pass, at every declared strength.
/// These are paired ranking bounds, not pooled AUC or naturalness qualification.
pub fn calibrate(scores: &[Score], threshold: f64) -> Result<serde_json::Value> {
    if scores.is_empty()
        || scores.len() > 100_000
        || !threshold.is_finite()
        || !(0.5..=1.0).contains(&threshold)
    {
        return Err(invalid(
            "calibration needs observations and threshold in [0.5,1]",
        ));
    }
    let mut groups: BTreeMap<(String, String, String), Vec<&Score>> = BTreeMap::new();
    let mut ids = std::collections::BTreeSet::new();
    for s in scores {
        if !s.positive.is_finite()
            || !s.negative.is_finite()
            || !s.strength.is_finite()
            || s.strength <= 0.0
            || s.strength > 1.0
            || !CLASSES.contains(&s.class.as_str())
            || s.scorer.is_empty()
            || s.id.is_empty()
            || !ids.insert((&s.id, &s.scorer))
        {
            return Err(invalid("invalid or duplicate calibration observation"));
        }
        groups
            .entry((s.scorer.clone(), s.class.clone(), s.strength.to_string()))
            .or_default()
            .push(s);
    }
    let mut rows = Vec::new();
    let mut trusts: BTreeMap<(String, String), bool> = BTreeMap::new();
    for ((scorer, class, strength), g) in groups {
        let n = g.len();
        let wins = g.iter().filter(|s| s.positive > s.negative).count();
        let ties = g.iter().filter(|s| s.positive == s.negative).count();
        let lower = if wins == 0 {
            0.0
        } else {
            binomial_bound(n, wins, true)
        };
        let upper = if wins == n {
            1.0
        } else {
            binomial_bound(n, wins, false)
        };
        let nonloss = wins + ties;
        let accuracy_upper = if nonloss == n {
            1.0
        } else {
            binomial_bound(n, nonloss, false)
        };
        let pass = lower >= threshold;
        trusts
            .entry((scorer.clone(), class.clone()))
            .and_modify(|v| *v &= pass)
            .or_insert(pass);
        rows.push(serde_json::json!({"scorer":scorer,"class":class,"strength":strength,"pairs":n,"wins":wins,"ties":ties,"pairwise_accuracy":(wins as f64+0.5*ties as f64)/n as f64,"strict_win_bounds_95":[lower,upper],"pairwise_accuracy_bounds_95":[lower,accuracy_upper],"trusted":pass}));
    }
    let mut scorers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for ((s, c), pass) in trusts {
        let v = scorers.entry(s).or_default();
        if pass {
            v.push(c);
        }
    }
    Ok(
        serde_json::json!({"schema":CALIBRATION_SCHEMA,"threshold":threshold,"bounds":"Clopper-Pearson strict-win bounds; conservative exact 95% paired-accuracy bounds from strict-win lower and non-loss upper; ties are failures for trust","rows":rows,"scorers":scorers.iter().map(|(s,c)|serde_json::json!({"scorer":s,"trusted_for":c,"untrusted_elsewhere":true})).collect::<Vec<_>>(),"limitations":["Constructed negatives only. Paired observations must be independent for binomial coverage; variants of one source are not independent domain evidence.","No fused score, pooled AUC, human labels, or external judge execution."]}),
    )
}
fn binomial_bound(n: usize, k: usize, lower: bool) -> f64 {
    // Stable binomial CDF via log probabilities, including endpoint handling.
    let cdf = |p: f64, end: usize| {
        let mut log = n as f64 * (1.0 - p).ln();
        let mut sum = log.exp();
        for j in 1..=end {
            log += ((n - j + 1) as f64 / j as f64).ln() + p.ln() - (1.0 - p).ln();
            sum += log.exp();
        }
        sum
    };
    let (mut lo, mut hi) = (1e-14, 1.0 - 1e-14);
    for _ in 0..64 {
        let p = (lo + hi) * 0.5;
        let target = if lower { 0.975 } else { 0.025 };
        let end = if lower { k - 1 } else { k };
        if cdf(p, end) > target {
            lo = p;
        } else {
            hi = p;
        }
    }
    (lo + hi) * 0.5
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn clip() -> Clip {
        let frames = (0..24)
            .map(|t| {
                image::RgbaImage::from_fn(32, 32, |x, y| {
                    let v = (((x as f64 - t as f64) * 0.4).sin() * 50.0
                        + ((y as f64) * 0.53).cos() * 40.0
                        + 128.0) as u8;
                    image::Rgba([v, v, v, 255])
                })
            })
            .collect();
        let map = frame_map::FrameMap {
            schema: frame_map::SCHEMA.into(),
            nominal_fps: Some(30.0),
            frames: (0..24)
                .map(|i| frame_map::Frame {
                    index: i,
                    timestamp_s: i as f64 / 30.0,
                    file: format!("{i}.png"),
                    sha256: None,
                })
                .collect(),
        };
        Clip { frames, map }
    }
    #[test]
    fn constructed_effects_and_timing_refusals() {
        let c = clip();
        let r = measure(&c, None).unwrap();
        let frozen = measure(&degrade(&c, "frozen", 1.0, 42).unwrap(), None).unwrap();
        assert_eq!(frozen.values["duplicate_fraction"], 1.0);
        assert!(frozen.values["frozen_fraction"] > r.values["frozen_fraction"]);
        let flicker = measure(&degrade(&c, "flicker", 1.0, 42).unwrap(), None).unwrap();
        assert!(
            flicker.values["flicker_second_difference_intensity_squared"]
                > r.values["flicker_second_difference_intensity_squared"] * 10.0
        );
        let hitch = measure(&degrade(&c, "looped_hitch", 1.0, 42).unwrap(), None).unwrap();
        assert!(hitch.values["hitch_fraction"] > 0.0);
        assert!(hitch.spectrum.is_none());
        let drops = measure(&degrade(&c, "frame_drops", 0.5, 42).unwrap(), None).unwrap();
        assert!(drops.values["missing_frames"] > 0.0);
        assert_eq!(drops.values["hitch_fraction"], 0.0);
        assert!(drops.spectrum.is_none());
        for class in ["speed_up", "speed_down"] {
            let v = measure(&degrade(&c, class, 1.0, 42).unwrap(), None).unwrap();
            assert!(compare(r.clone(), v.clone()).distances["median_interval_s"] > 0.01);
            if let Some(speed) = r.values.get("flow_mean_px_per_s") {
                let factor = if class == "speed_up" { 2.0 } else { 0.5 };
                assert!((v.values["flow_mean_px_per_s"] / speed - factor).abs() < 1e-6);
            }
        }
        for class in CLASSES {
            let a = degrade(&c, class, 0.5, 42).unwrap();
            let b = degrade(&c, class, 0.5, 42).unwrap();
            assert_eq!(a.frames, b.frames);
            assert_eq!(a.map, b.map);
        }
        assert!(degrade(&c, "frozen", f64::NAN, 0).is_err());
        assert!(degrade(&c, "frame_drops", 0.001, 0).is_err());
        let mut still = clip();
        still.frames = vec![still.frames[0].clone(); 24];
        assert!(degrade(&still, "frozen", 1.0, 0).is_err());
        assert!(measure(&c, Some(&vec![false; 1024])).is_err());
        let mut malformed = clip();
        malformed.map.frames[1].timestamp_s = 0.0;
        assert!(measure(&malformed, None).is_err());
    }
    #[test]
    fn binomial_bounds_ties_and_stratification_fail_closed() {
        assert!((binomial_bound(10, 10, true) - 0.025_f64.powf(0.1)).abs() < 1e-10);
        assert!((binomial_bound(10, 0, false) - (1.0 - 0.025_f64.powf(0.1))).abs() < 1e-10);
        let mut scores: Vec<_> = (0..20)
            .map(|i| Score {
                id: format!("p{i}"),
                scorer: "metric".into(),
                class: "frozen".into(),
                strength: 1.0,
                positive: 0.0,
                negative: -1.0,
            })
            .collect();
        let report = calibrate(&scores, 0.8).unwrap();
        assert_eq!(report["scorers"][0]["trusted_for"][0], "frozen");
        scores.push(Score {
            id: "weak".into(),
            scorer: "metric".into(),
            class: "frozen".into(),
            strength: 0.5,
            positive: 0.0,
            negative: 0.0,
        });
        let report = calibrate(&scores, 0.8).unwrap();
        assert_eq!(report["scorers"][0]["trusted_for"], serde_json::json!([]));
        let tied = report["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["ties"] == 1)
            .unwrap();
        assert_eq!(tied["pairwise_accuracy"], 0.5);
        assert_eq!(
            tied["pairwise_accuracy_bounds_95"],
            serde_json::json!([0.0, 1.0])
        );
        scores.push(scores[0].clone());
        assert!(calibrate(&scores, 0.8).is_err());
    }
    #[test]
    fn masks_remove_outside_flicker_and_flow_unavailability_is_explicit() {
        let c = clip();
        let mut d = clip();
        for (t, im) in d.frames.iter_mut().enumerate() {
            for (x, _, p) in im.enumerate_pixels_mut() {
                if x >= 16 {
                    *p = image::Rgba([if t % 2 == 0 { 0 } else { 255 }; 4]);
                    p[3] = 255;
                }
            }
        }
        let mask: Vec<bool> = (0..1024).map(|i| i % 32 < 16).collect();
        let r = measure(&c, Some(&mask)).unwrap();
        let v = measure(&d, Some(&mask)).unwrap();
        assert_eq!(
            r.values["mean_change_intensity"],
            v.values["mean_change_intensity"]
        );
        if !cfg!(feature = "dense-motion") {
            assert!(r.magnitude.is_none());
            assert!(!r.values.contains_key("flow_mean_px_per_s"));
        }
    }
}
