//! Conditional global-translation evidence; never a replacement FLIP verdict.
use super::*;
use crate::evidence::analysis::{Analysis, Capability, Provenance, Resource};
use sha2::{Digest, Sha256};

/// Per-hotspot interpretation under the recorded global translation hypothesis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum MotionClass {
    /// Displacement and aligned residual are below the declared diagnostic limits.
    Stable,
    /// Qualified global displacement explains this hotspot's appearance residual.
    Moved,
    /// Significant appearance residual without appreciable global displacement.
    Changed,
    /// Appreciable global displacement and significant remaining appearance residual.
    MovedAndChanged,
    /// Correspondence, texture or valid coverage is insufficient.
    Unknown,
}
/// One original hotspot measured without changing its selected pixel mask.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct MotionHotspot {
    /// Index in the original ordered hotspot list.
    pub hotspot: usize,
    /// Conditional diagnostic class, not an acceptance decision.
    pub class: MotionClass,
    /// Number of original hotspot pixels retained after border exclusion.
    pub valid_pixels: usize,
    /// Fraction of original hotspot pixels retained.
    pub valid_fraction: f64,
    /// Original FLIP mean on those same valid pixels.
    pub raw_flip_mean: Option<f64>,
    /// Aligned FLIP mean on those same valid pixels.
    pub aligned_flip_mean: Option<f64>,
    /// Peak aligned FLIP on the same valid pixels; prevents small defects being diluted in a large hotspot.
    pub aligned_flip_max: Option<f64>,
    /// Fraction of raw mean removed by alignment, clamped to [0,1].
    pub explained_fraction: Option<f64>,
    /// Baseline encoded-luminance standard deviation on the valid pixels.
    pub texture_std: f64,
    /// Why correspondence or classification remains unresolved.
    pub reasons: Vec<String>,
}
/// Reproducible evidence alongside unaligned FLIP.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct MotionEvidence {
    /// Conditional global correspondence qualification.
    pub correspondence: Capability,
    /// Capture displacement against baseline, right/down positive, in pixels.
    pub translation: Option<[f64; 2]>,
    /// Weighted phase coherence; not a probability.
    pub phase_coherence: Option<f64>,
    /// Best correlation peak divided by the strongest peak outside a 3-pixel neighbourhood.
    pub peak_ratio: Option<f64>,
    /// Length of forward + reverse displacement, in pixels.
    pub forward_backward_error: Option<f64>,
    /// Original complete-frame FLIP mean; never replaced with aligned measurements.
    pub raw_full_frame_flip_mean: f64,
    /// Aligned FLIP mean only on valid interior pixels.
    pub aligned_valid_flip_mean: Option<f64>,
    /// Conservative interpolation plus FLIP filter border margin.
    pub border_margin_px: usize,
    /// Pixels excluded from aligned evidence, as row-major [start,length] runs.
    pub invalid_border_runs: Vec<[u32; 2]>,
    /// Per-original-hotspot diagnostics.
    pub hotspots: Vec<MotionHotspot>,
    /// Dense independent-object correspondence is unsupported in this build.
    pub dense_flow: Capability,
}

fn runs(bits: &[bool]) -> Vec<[u32; 2]> {
    let mut out: Vec<[u32; 2]> = Vec::new();
    for (i, &bit) in bits.iter().enumerate() {
        if bit {
            match out.last_mut() {
                Some([start, len]) if (*start as usize + *len as usize) == i => *len += 1,
                _ => out.push([i as u32, 1]),
            }
        }
    }
    out
}
fn texture_rank(base: Pixels<'_>) -> bool {
    let (w, h) = base.dims();
    let (mut xx, mut yy, mut xy) = (0.0, 0.0, 0.0);
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            let i = y * w + x;
            let dx = f64::from(base.luma(i + 1) - base.luma(i - 1));
            let dy = f64::from(base.luma(i + w) - base.luma(i - w));
            xx += dx * dx;
            yy += dy * dy;
            xy += dx * dy;
        }
    }
    xx > 1e-8 && yy > 1e-8 && (xx * yy - xy * xy) / (xx * yy) > 0.01
}
// Hann windows can select one phase peak on an exactly repeating texture.
// Independently search the unwindowed, overlap-normalized texture
// autocorrelation for distant local peaks. Linear zero padding avoids inventing
// repetitions across image edges; integral energies normalize overlap loss.
fn periodic_alias(px: Pixels<'_>) -> bool {
    let (iw, ih) = px.dims();
    let factor = iw.max(ih).div_ceil(256).max(1);
    let (plane, w, h) = luma_plane(px, 0, 0, iw, ih, factor, None);
    if w < 16 || h < 16 {
        return false;
    }
    let (pw, ph) = ((2 * w).next_power_of_two(), (2 * h).next_power_of_two());
    let mean = plane.iter().map(|&v| f64::from(v)).sum::<f64>() / plane.len() as f64;
    let mut data = vec![Complex::new(0.0, 0.0); pw * ph];
    let mut energy = vec![0.0; (w + 1) * (h + 1)];
    for y in 0..h {
        for x in 0..w {
            let v = (f64::from(plane[y * w + x]) - mean) as f32;
            data[y * pw + x].re = v;
            let i = (y + 1) * (w + 1) + x + 1;
            energy[i] =
                f64::from(v).powi(2) + energy[i - 1] + energy[i - w - 1] - energy[i - w - 2];
        }
    }
    let rect = |x0: usize, y0: usize, x1: usize, y1: usize| {
        energy[y1 * (w + 1) + x1] - energy[y0 * (w + 1) + x1] - energy[y1 * (w + 1) + x0]
            + energy[y0 * (w + 1) + x0]
    };
    if rect(0, 0, w, h) < 1e-8 {
        return false;
    }
    let mut planner = FftPlanner::new();
    fft2(&mut planner, &mut data, pw, ph, false);
    for value in &mut data {
        *value = Complex::new(value.norm_sqr(), 0.0);
    }
    fft2(&mut planner, &mut data, pw, ph, true);
    let correlation = |dx: isize, dy: isize| {
        let (x0, y0) = (dx.max(0) as usize, dy.max(0) as usize);
        let (x1, y1) = (w - (-dx).max(0) as usize, h - (-dy).max(0) as usize);
        let first = rect(x0, y0, x1, y1);
        let second = rect(
            (x0 as isize - dx) as usize,
            (y0 as isize - dy) as usize,
            (x1 as isize - dx) as usize,
            (y1 as isize - dy) as usize,
        );
        let denom = (first * second).sqrt();
        if denom < 1e-10 {
            return 0.0;
        }
        let x = dx.rem_euclid(pw as isize) as usize;
        let y = dy.rem_euclid(ph as isize) as usize;
        f64::from(data[y * pw + x].re) / (pw * ph) as f64 / denom
    };
    for dy in -(h as isize / 2)..=h as isize / 2 {
        for dx in 0..=w as isize / 2 {
            if dx.abs().max(dy.abs()) <= 3 {
                continue;
            }
            let peak = correlation(dx, dy);
            if peak >= 0.9995
                && [(dx - 1, dy), (dx + 1, dy), (dx, dy - 1), (dx, dy + 1)]
                    .iter()
                    .all(|&(x, y)| peak + 1e-6 >= correlation(x, y))
            {
                return true;
            }
        }
    }
    false
}
/// Records why the computation did not run, including explicit user exclusions.
pub(super) fn unavailable(reason: &str, excluded: bool) -> Analysis<MotionEvidence> {
    Analysis {
        capability: if excluded {
            Capability::Excluded {
                reason: reason.into(),
            }
        } else {
            Capability::Unknown {
                reason: reason.into(),
            }
        },
        provenance: None,
        evidence: None,
        limitations: vec!["Raw unaligned FLIP remains authoritative.".into()],
    }
}
pub(super) fn analyze(
    req: &DiagnoseRequest<'_>,
    estimate: Option<&RawShift>,
    already_aligned: Option<&Comparison>,
) -> Analysis<MotionEvidence> {
    if !req.config.enabled || !req.config.shift_detection {
        return unavailable("shift_detection_disabled", true);
    }
    let (Pixels::Ldr(base), Pixels::Ldr(cap)) = (req.baseline, req.capture) else {
        return Analysis {
            capability: Capability::Unsupported {
                reason: "motion_hotspot_hdr_unqualified".into(),
            },
            provenance: None, evidence: None, limitations: vec!["Raw unaligned FLIP remains authoritative; this motion capability was not qualified.".into()]
        };
    };
    if base.pixels().chain(cap.pixels()).any(|p| p.0[3] != 255) {
        return Analysis {
            capability: Capability::Unsupported {
                reason: "motion_hotspot_transparency_unqualified".into(),
            },
            provenance: None, evidence: None, limitations: vec!["Raw unaligned FLIP remains authoritative; this motion capability was not qualified.".into()]
        };
    }
    let (w, h) = req.baseline.dims();
    let reverse = estimate.and_then(|_| estimate_shift(req.capture, req.baseline, None));
    let consistency = estimate
        .zip(reverse.as_ref())
        .map(|(a, b)| (a.dx + b.dx).hypot(a.dy + b.dy));
    let qualified = estimate.zip(reverse.as_ref()).is_some_and(|(a, b)| {
        a.confidence >= req.config.shift_min_confidence
            && b.confidence >= req.config.shift_min_confidence
            && a.peak_ratio >= 1.2
            && b.peak_ratio >= 1.2
    }) && consistency.is_some_and(|e| e <= 0.2)
        && texture_rank(req.baseline)
        && !periodic_alias(req.baseline)
        && !periodic_alias(req.capture);
    let correspondence = if qualified {
        Capability::Available
    } else {
        Capability::Unknown {
            reason: "global_translation_ambiguous_or_inconsistent_or_untextured".into(),
        }
    };
    let extra_aligned = if qualified && already_aligned.is_none() {
        estimate.and_then(|e| rerun(req, &shift_capture(req.capture, e.dx, e.dy)))
    } else {
        None
    };
    let aligned = already_aligned
        .or(extra_aligned.as_ref())
        .filter(|_| qualified);
    // Pinned flip-rs 0.1.2 filters.rs: the larger colour/feature support radius,
    // plus two Catmull-Rom taps. No clamped border sample qualifies alignment.
    let ppd = f64::from(req.flip.pixels_per_degree);
    let radius = (3.0 * (0.04 / (2.0 * std::f64::consts::PI.powi(2))).sqrt() * ppd)
        .max(3.0 * 0.5 * 0.082 * ppd)
        .ceil();
    let margin = radius as usize + 2;
    let (dx, dy) = estimate.map_or((0.0, 0.0), |e| (e.dx, e.dy));
    let invalid: Vec<_> = (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) as f64, (i / w) as f64);
            let m = margin as f64;
            x < m
                || y < m
                || x >= w as f64 - m
                || y >= h as f64 - m
                || x + dx < m
                || y + dy < m
                || x + dx >= w as f64 - m
                || y + dy >= h as f64 - m
        })
        .collect();
    let valid_n = invalid.iter().filter(|&&x| !x).count();
    let aligned_mean = aligned.filter(|_| valid_n > 0).map(|c| {
        c.error_map
            .iter()
            .zip(&invalid)
            .filter(|(_, bad)| !**bad)
            .map(|(&v, _)| f64::from(v))
            .sum::<f64>()
            / valid_n as f64
    });
    let moved = qualified && dx.hypot(dy) >= req.config.shift_min_px;
    let hotspots = req
        .hotspots
        .iter()
        .enumerate()
        .map(|(index, spot)| {
            let (mut count, mut raw, mut residual, mut residual_max, mut luma, mut luma2) =
                (0usize, 0.0, 0.0, 0.0f64, 0.0, 0.0);
            let mut total = 0usize;
            for &[start, len] in &spot.pixel_runs {
                for (i, &bad) in invalid
                    .iter()
                    .enumerate()
                    .take((start as usize + len as usize).min(w * h))
                    .skip(start as usize)
                {
                    total += 1;
                    if bad {
                        continue;
                    }
                    count += 1;
                    raw += f64::from(req.comparison.error_map[i]);
                    if let Some(a) = aligned {
                        residual += f64::from(a.error_map[i]);
                        residual_max = residual_max.max(f64::from(a.error_map[i]));
                    }
                    let v = f64::from(req.baseline.luma(i));
                    luma += v;
                    luma2 += v * v;
                }
            }
            let valid_fraction = count as f64 / total.max(1) as f64;
            let raw_mean = (count > 0).then_some(raw / count.max(1) as f64);
            let aligned_mean = aligned
                .filter(|_| count > 0)
                .map(|_| residual / count as f64);
            let texture_std = (luma2 / count.max(1) as f64 - (luma / count.max(1) as f64).powi(2))
                .max(0.0)
                .sqrt();
            let fraction = raw_mean
                .zip(aligned_mean)
                .map(|(raw, aligned)| explained(raw, aligned));
            let mut reasons = Vec::new();
            if !qualified {
                reasons.push("global_correspondence_unqualified".into());
            }
            if aligned.is_none() {
                reasons.push("aligned_error_unavailable".into());
            }
            if count < 8 || valid_fraction < 0.5 {
                reasons.push("insufficient_valid_interior_pixels".into());
            }
            if texture_std < 0.002 {
                reasons.push("local_texture_insufficient".into());
            }
            let class = if !reasons.is_empty() {
                MotionClass::Unknown
            } else {
                let changed = residual_max > req.config.noise_max_flip
                    || aligned_mean.is_some_and(|v| v > req.config.noise_max_flip)
                    || (moved && fraction.is_some_and(|v| v < req.config.explained_min));
                match (moved, changed) {
                    (true, true) => MotionClass::MovedAndChanged,
                    (true, false) => MotionClass::Moved,
                    (false, true) => MotionClass::Changed,
                    (false, false) => MotionClass::Stable,
                }
            };
            MotionHotspot {
                hotspot: index,
                class,
                valid_pixels: count,
                valid_fraction,
                raw_flip_mean: raw_mean,
                aligned_flip_mean: aligned_mean,
                aligned_flip_max: aligned.filter(|_| count > 0).map(|_| residual_max),
                explained_fraction: fraction,
                texture_std,
                reasons,
            }
        })
        .collect();
    let mut provenance = Provenance::native("phase-correlation-hotspots/2");
    provenance.resources = [("baseline", base), ("capture", cap)]
        .into_iter()
        .map(|(name, image)| Resource {
            name: name.into(),
            role: "decoded_rgba8_diagnostic_pixels".into(),
            sha256: Some(format!("{:x}", Sha256::digest(image.as_raw()))),
            license: None,
        })
        .collect();
    provenance
        .settings
        .insert("pixels_per_degree".into(), ppd.to_string());
    provenance
        .settings
        .insert("coarse_max_side".into(), SHIFT_COARSE_MAX.to_string());
    provenance.settings.insert(
        "fft_precision".into(),
        "f32 with ordered f64 reductions".into(),
    );
    provenance.settings.insert(
        "minimum_global_gradient_rank".into(),
        "0.01; gradient energies each > 1e-8".into(),
    );
    provenance.settings.insert("periodic_alias_test".into(), "unwindowed linear autocorrelation; distant local peak >=0.9995 with at least half-axis overlap; maximum side 256".into());
    provenance.settings.extend([("dimensions".into(),format!("{w}x{h}")),("phase".into(),"RustFFT 6.4.1 (MIT OR Apache-2.0); Hann window; whitened cross-spectrum; low-frequency phase-slope subpixel fit".into()),("interpolation".into(),"one Catmull-Rom bicubic inverse warp in encoded RGBA8; rounded/clamped samples; border support excluded".into()),("minimum_peak_ratio".into(),"1.2 outside 3px peak neighbourhood".into()),("maximum_forward_backward_error_px".into(),"0.2".into()),("minimum_phase_coherence".into(),req.config.shift_min_confidence.to_string()),("minimum_displacement_px".into(),req.config.shift_min_px.to_string()),("maximum_aligned_flip_mean_and_peak".into(),req.config.noise_max_flip.to_string()),("minimum_explained_fraction".into(),req.config.explained_min.to_string()),("minimum_local_texture_std".into(),"0.002".into()),("minimum_valid_pixels_and_fraction".into(),"8 and 0.5".into())]);
    Analysis { capability:Capability::Available,provenance:Some(provenance),evidence:Some(MotionEvidence { correspondence,translation:estimate.map(|e| [e.dx,e.dy]),phase_coherence:estimate.map(|e| e.confidence),peak_ratio:estimate.map(|e| e.peak_ratio),forward_backward_error:consistency,raw_full_frame_flip_mean:req.comparison.metrics.mean,aligned_valid_flip_mean:aligned_mean,border_margin_px:margin,invalid_border_runs:runs(&invalid),hotspots,dense_flow:Capability::Unsupported { reason:"dis_not_in_this_build".into() } }),limitations:vec!["Unaligned FLIP, entry thresholds and verdict remain authoritative; movement can itself be a regression.".into(),"Hotspot labels are conditional on one global translation, not independent-object flow or causal proof. Interior disocclusion and local deformation are not qualified.".into(),"HDR, transparency and dense DIS flow are unsupported for these hotspot labels. Ambiguous, untextured or border-dominated hotspots stay unknown.".into()] }
}
