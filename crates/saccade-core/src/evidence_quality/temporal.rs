//! Fixed-camera tile flicker, with existing phase-correlation motion qualification.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
/// Temporal stability schema.
pub const SCHEMA: &str = "saccade-tile-temporal.v1";
/// Declared fixed-camera temporal policy.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
    /// Required declaration, independently checked for global translation.
    pub fixed_camera: bool,
    /// Grid width/height in pixels.
    pub tile_size: u32,
    /// Frame rate for interpretation; no resampling.
    pub fps: f64,
    /// Minimum increase in temporal delta variance.
    pub variance_increase: f64,
    /// Minimum increase in second-difference energy.
    pub energy_increase: f64,
    /// Minimum ratio above the baseline temporal variance/energy floor.
    pub increase_ratio: f64,
    /// Maximum detected global displacement in pixels.
    pub motion_max_px: f64,
    /// Minimum phase-coherence confidence for a motion finding.
    pub motion_confidence: f64,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            fixed_camera: true,
            tile_size: 32,
            fps: 60.0,
            variance_increase: 0.00005,
            energy_increase: 0.00005,
            increase_ratio: 1.2,
            motion_max_px: 0.5,
            motion_confidence: 0.6,
        }
    }
}
impl Policy {
    /// Check policy and finite positive thresholds.
    pub fn validate(&self) -> Result<()> {
        if !self.fixed_camera
            || self.tile_size < 4
            || self.tile_size > 4096
            || [
                self.fps,
                self.variance_increase,
                self.energy_increase,
                self.increase_ratio,
                self.motion_max_px,
                self.motion_confidence,
            ]
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.0)
            || self.increase_ratio <= 1.0
            || self.motion_confidence > 1.0
        {
            return Err(Error::Config("invalid fixed-camera temporal policy".into()));
        }
        Ok(())
    }
}
/// Consecutive-frame global translation estimate; unavailable motion is explicit.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Motion {
    /// baseline or candidate.
    pub side: String,
    /// Pair starts at this index.
    pub frame: usize,
    /// dx,dy in original pixels.
    pub displacement: Option<[f64; 2]>,
    /// Phase coherence.
    pub confidence: Option<f64>,
    /// Motion contradicts fixed-camera declaration.
    pub camera_not_fixed: bool,
}
/// Per-tile temporal metrics for one side.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Energy {
    /// Mean over pixels of variance of frame-to-frame luminance differences.
    pub delta_variance: f64,
    /// Mean squared second temporal difference divided by four.
    pub high_frequency_energy: f64,
}
/// Paired tile stability evidence.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    /// Full-resolution pixel box.
    pub rect_px: [u32; 4],
    /// Baseline temporal energies.
    pub baseline: Energy,
    /// Candidate energies.
    pub candidate: Energy,
    /// Candidate minus baseline variance.
    pub variance_increase: f64,
    /// Candidate minus baseline high-frequency energy.
    pub energy_increase: f64,
    /// Increase exceeds absolute and relative thresholds.
    pub shimmer_increase: bool,
}
/// Sequence verdict and motion flags, independent of still-image FLIP thresholds.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// Versioned discriminator.
    pub schema: String,
    /// Effective fixed-camera policy.
    pub policy: Policy,
    /// stable, shimmer_increase, camera_not_fixed, or unqualified_motion.
    pub verdict: String,
    /// Per-tile paired flicker.
    pub tiles: Vec<Tile>,
    /// Bounding boxes of connected shimmering tiles.
    pub regions: Vec<[u32; 4]>,
    /// Motion estimates for both sides.
    pub motion: Vec<Motion>,
    /// Explicit interpretive limits.
    pub limits: Vec<String>,
}
fn energy(planes: &[Vec<f64>], w: u32, r: [u32; 4]) -> Energy {
    let mut variance = 0.0;
    let mut hf = 0.0;
    let [x, y, rw, rh] = r;
    for yy in y..y + rh {
        for xx in x..x + rw {
            let i = (yy * w + xx) as usize;
            let delta: Vec<_> = planes.windows(2).map(|p| p[1][i] - p[0][i]).collect();
            let mean = delta.iter().sum::<f64>() / delta.len() as f64;
            variance += delta.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / delta.len() as f64;
            hf += planes
                .windows(3)
                .map(|p| (p[2][i] - 2.0 * p[1][i] + p[0][i]).powi(2) / 4.0)
                .sum::<f64>()
                / (planes.len() - 2) as f64;
        }
    }
    let n = f64::from(rw) * f64::from(rh);
    Energy {
        delta_variance: variance / n,
        high_frequency_energy: hf / n,
    }
}
/// Compute tile flicker and qualify the fixed-camera declaration against global motion.
pub fn analyze(
    base: &[image::RgbaImage],
    candidate: &[image::RgbaImage],
    policy: &Policy,
) -> Result<Report> {
    policy.validate()?;
    let first = base.first().ok_or(Error::EmptyImage)?;
    let (w, h) = first.dimensions();
    if base.len() < 3
        || base.len() != candidate.len()
        || base.len() > 256
        || u64::from(w) * u64::from(h) * base.len() as u64 > 16_777_216
        || base
            .iter()
            .chain(candidate)
            .any(|i| i.dimensions() != (w, h))
    {
        return Err(Error::Config("temporal tiles need 3..256 equally sized paired frames within the sequence allocation limit".into()));
    }
    let mut motion = Vec::new();
    for (side, frames) in [("baseline", base), ("candidate", candidate)] {
        for (frame, pair) in frames.windows(2).enumerate() {
            let estimate = if pair[0] == pair[1] {
                Some([0.0, 0.0, 1.0])
            } else {
                crate::diagnostics::global_translation(&pair[0], &pair[1])
            };
            motion.push(Motion {
                side: side.into(),
                frame,
                displacement: estimate.map(|e| [e[0], e[1]]),
                confidence: estimate.map(|e| e[2]),
                camera_not_fixed: estimate.is_some_and(|e| {
                    e[0].hypot(e[1]) > policy.motion_max_px && e[2] >= policy.motion_confidence
                }),
            });
        }
    }
    let lb: Vec<_> = base
        .iter()
        .map(|i| {
            super::spatial::rgb(i)
                .into_iter()
                .map(super::spatial::luminance)
                .collect()
        })
        .collect();
    let lc: Vec<_> = candidate
        .iter()
        .map(|i| {
            super::spatial::rgb(i)
                .into_iter()
                .map(super::spatial::luminance)
                .collect()
        })
        .collect();
    let mut tiles = Vec::new();
    for y in (0..h).step_by(policy.tile_size as usize) {
        for x in (0..w).step_by(policy.tile_size as usize) {
            let r = [
                x,
                y,
                policy.tile_size.min(w - x),
                policy.tile_size.min(h - y),
            ];
            let b = energy(&lb, w, r);
            let c = energy(&lc, w, r);
            let vi = c.delta_variance - b.delta_variance;
            let ei = c.high_frequency_energy - b.high_frequency_energy;
            let increased = (vi > policy.variance_increase
                && c.delta_variance
                    > policy.increase_ratio
                        * b.delta_variance.max(policy.variance_increase / 10.0))
                || (ei > policy.energy_increase
                    && c.high_frequency_energy
                        > policy.increase_ratio
                            * b.high_frequency_energy.max(policy.energy_increase / 10.0));
            tiles.push(Tile {
                rect_px: r,
                baseline: b,
                candidate: c,
                variance_increase: vi,
                energy_increase: ei,
                shimmer_increase: increased,
            });
        }
    }
    let cols = w.div_ceil(policy.tile_size);
    let mut seen = vec![false; tiles.len()];
    let mut regions = Vec::new();
    for start in 0..tiles.len() {
        if seen[start] || !tiles[start].shimmer_increase {
            continue;
        }
        let mut q = std::collections::VecDeque::from([start]);
        seen[start] = true;
        let mut r = tiles[start].rect_px;
        let mut far = [r[0] + r[2], r[1] + r[3]];
        while let Some(i) = q.pop_front() {
            let b = tiles[i].rect_px;
            r[0] = r[0].min(b[0]);
            r[1] = r[1].min(b[1]);
            far[0] = far[0].max(b[0] + b[2]);
            far[1] = far[1].max(b[1] + b[3]);
            let x = i % cols as usize;
            for j in [
                (x > 0).then(|| i - 1),
                (x + 1 < cols as usize).then(|| i + 1),
                i.checked_sub(cols as usize),
                i.checked_add(cols as usize).filter(|j| *j < tiles.len()),
            ]
            .into_iter()
            .flatten()
            {
                if !seen[j] && tiles[j].shimmer_increase {
                    seen[j] = true;
                    q.push_back(j);
                }
            }
        }
        r[2] = far[0] - r[0];
        r[3] = far[1] - r[1];
        regions.push(r);
    }
    let verdict = if motion.iter().any(|m| m.camera_not_fixed) {
        "camera_not_fixed"
    } else if motion.iter().any(|m| m.displacement.is_none()) {
        "unqualified_motion"
    } else if regions.is_empty() {
        "stable"
    } else {
        "shimmer_increase"
    };
    Ok(Report {schema:SCHEMA.into(),policy:policy.clone(),verdict:verdict.into(),tiles,regions,motion,limits:vec!["Translation phase correlation checks global displacement; moving objects, rotation and scene changes can confound camera attribution.".into(),"Unmeasurable motion is flagged and cannot establish stable fixed-camera evidence. Energy is normalized sRGB squared per frame; fps is recorded, no temporal resampling.".into()]})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stable_and_flickering_region_are_distinguished() {
        let b = image::RgbaImage::from_fn(96, 96, |x, y| {
            let v = ((x * 13 + y * 37 + x * y * 3) % 150 + 50) as u8;
            image::Rgba([v, v, v, 255])
        });
        let bases = vec![b.clone(); 5];
        let mut candidate = bases.clone();
        for (i, img) in candidate.iter_mut().enumerate() {
            for y in 32..64 {
                for x in 32..64 {
                    let v = if i % 2 == 0 { 120 } else { 145 };
                    img.put_pixel(x, y, image::Rgba([v, v, v, 255]));
                }
            }
        }
        assert_eq!(
            analyze(&bases, &bases, &Policy::default()).unwrap().verdict,
            "stable"
        );
        let r = analyze(&bases, &candidate, &Policy::default()).unwrap();
        assert!(r.regions.contains(&[32, 32, 32, 32]));
        assert_eq!(r.verdict, "shimmer_increase");
    }
    #[test]
    fn translated_sequence_cannot_claim_fixed_camera() {
        let b = image::RgbaImage::from_fn(96, 96, |x, y| {
            let v = ((x * 13 + y * 37 + x * y * 3) % 200 + 20) as u8;
            image::Rgba([v, v, v, 255])
        });
        let frames: Vec<_> = (0..4)
            .map(|i| {
                image::RgbaImage::from_fn(96, 96, |x, y| *b.get_pixel((x + 96 - i * 2) % 96, y))
            })
            .collect();
        let r = analyze(&frames, &frames, &Policy::default()).unwrap();
        assert_eq!(r.verdict, "camera_not_fixed");
        assert!(r.motion.iter().any(|m| m.camera_not_fixed));
    }
}
