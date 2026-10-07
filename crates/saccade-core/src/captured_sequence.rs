//! Budgeted transitions and animation consistency over external image captures.
use crate::{Error, Result};
use image::RgbaImage;
use serde::{Deserialize, Serialize};

/// Capture input contract, shared by both commands.
pub const PLAN_SCHEMA: &str = "saccade-captured-sequence-plan.v1";
/// Transition report contract.
pub const TRANSITION_SCHEMA: &str = "saccade-transition.v1";
/// Animation report contract.
pub const ANIMATION_SCHEMA: &str = "saccade-animation.v1";

/// Motion correspondence used only for supplementary aligned measurements.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum Alignment {
    /// Compare original coordinates.
    None,
    /// Existing phase-correlation global translation, confidence at least 0.6.
    Translation,
    /// Existing qualified dense image correspondence; requires dense-motion.
    Dense,
}
/// One externally captured observation on the common declared timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Capture {
    /// Relative image path inside the plan directory.
    pub image: String,
    /// Declared timestamp in milliseconds; never inferred from filenames.
    pub timestamp_ms: f64,
}
/// Explicit capture lists, avoiding silent pairing or missing-frame truncation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Plan {
    /// Version discriminator.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-captured-sequence-plan.v1")))]
    pub schema: String,
    /// Candidate observations, in strictly increasing timestamp order.
    pub candidate: Vec<Capture>,
    /// Independent matched reference; required for animation and level pairs.
    pub reference: Option<Vec<Capture>>,
    /// Optional binary L8 inclusion mask per frame, on the original reference grid.
    pub masks: Option<Vec<String>>,
    /// Optional L8/L16 ID buffer per frame, on the original reference grid.
    pub id_buffers: Option<Vec<String>>,
    /// IDs to include; combined with masks by intersection, never candidate-derived.
    pub include_ids: Vec<u16>,
    /// Alignment diagnostics do not override raw error budgets.
    pub alignment: Alignment,
}
/// Declared transition budgets and observation windows.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct TransitionPolicy {
    /// Switch request index; null selects a matched pair of steady levels.
    pub change_frame: Option<usize>,
    /// Maximum positive frame-to-frame RGB error above motion baseline.
    pub maximum_pop: f64,
    /// Maximum time from request to K settled frames, in milliseconds.
    pub maximum_duration_ms: f64,
    /// Maximum final-window paired level difference, normalized RGB MAE.
    pub maximum_steady_error: f64,
    /// Maximum tile error for convergence to the target observation.
    pub settle_threshold: f64,
    /// Number of consecutive observations required for convergence.
    pub consecutive: usize,
    /// Final observation window length and pre-switch baseline window length.
    pub window: usize,
    /// Spatial tile edge in pixels.
    pub tile_size: u32,
}
/// Animation budgets; both whole-frame and local errors are authoritative.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct AnimationPolicy {
    /// Maximum raw full-frame normalized RGB MAE.
    pub maximum_frame_error: f64,
    /// Maximum raw tile normalized RGB MAE.
    pub maximum_local_error: f64,
    /// Maximum tile signed-residual change, divided by two to normalize to 0..1.
    pub maximum_flicker: f64,
    /// Tile edge in pixels.
    pub tile_size: u32,
}
/// Spatially localized error at one observation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Region {
    /// Reference-grid rectangle x,y,width,height.
    pub rect_px: [u32; 4],
    /// Scoped mean absolute normalized RGB difference.
    pub error: f64,
}
/// Matched frame measurements and explicit alignment support.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Frame {
    /// Zero-based input index.
    pub index: usize,
    /// Common declared timestamp.
    pub timestamp_ms: f64,
    /// Included original-grid pixels.
    pub pixels: usize,
    /// Raw error; correspondence never changes this score.
    pub raw_error: f64,
    /// Largest scoped tile mean error.
    pub local_error: f64,
    /// Supplementary aligned error, null when no correspondence is supported.
    pub aligned_error: Option<f64>,
    /// Supported aligned pixels; geometric/flow exclusions are explicit.
    pub aligned_pixels: usize,
    /// Estimated dx,dy,confidence for translation; null in other modes.
    pub translation: Option<[f64; 3]>,
    /// Raw tiles exceeding the declared local/settling threshold.
    pub regions: Vec<Region>,
    /// Supplementary aligned tile errors for temporal inspection.
    pub aligned_regions: Vec<Region>,
    /// Maximum tile signed-residual change from the previous frame, divided by two.
    /// Null for the first frame or incomplete persistent correspondence support.
    pub residual_flicker: Option<f64>,
}
/// Complete animation packet, with an explicit failure when alignment is unavailable.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct AnimationReport {
    /// Version discriminator.
    pub schema: String,
    /// pass or regression.
    pub verdict: String,
    /// Effective capture declaration, including scope and timestamps.
    pub plan: Plan,
    /// Effective budgets.
    pub policy: AnimationPolicy,
    /// Frame and spatial trajectories.
    pub frames: Vec<Frame>,
    /// Earliest raw frame/local budget failure, zero based.
    pub first_divergence_frame: Option<usize>,
    /// Maximum tile signed-residual change divided by two; null with missing support.
    pub temporal_flicker: Option<f64>,
    /// First later frame in a flicker pair over budget.
    pub first_flicker_frame: Option<usize>,
    /// Explicit interpretation boundaries.
    pub limits: Vec<String>,
}
/// Complete transition packet, separate from static geometry proof.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct TransitionReport {
    /// Version discriminator.
    pub schema: String,
    /// pass or regression.
    pub verdict: String,
    /// Effective capture declaration.
    pub plan: Plan,
    /// Effective budgets and switch request.
    pub policy: TransitionPolicy,
    /// Target/reference differences, including localized regions.
    pub frames: Vec<Frame>,
    /// Positive excess over baseline, indexed by later frame; index zero is zero.
    pub pop_curve: Vec<f64>,
    /// Maximum excess in the declared switch interval (null for level pairs).
    pub popping: Option<f64>,
    /// Earliest maximum in the switch interval; null if no positive excess.
    pub pop_frame: Option<usize>,
    /// Raw temporal motion baseline from the reference, or pre-switch window.
    pub motion_baseline: Vec<f64>,
    /// First K-frame target-convergence interval; null for level pairs/unsettled.
    pub settle_frame: Option<usize>,
    /// Timestamp difference from request to settling; null if unmeasured.
    pub transition_duration_ms: Option<f64>,
    /// Final-window paired level difference, or pre/post window difference for a single switch.
    pub steady_state_difference: f64,
    /// matched_reference or pre_post_windows; single-sequence values include temporal motion.
    pub steady_state_basis: String,
    /// Explicit interpretation boundaries.
    pub limits: Vec<String>,
}
/// Validated decoded inputs; scope is frozen on the original reference grid.
pub struct Images {
    /// Decoded candidate captures.
    pub candidate: Vec<RgbaImage>,
    /// Optional decoded reference captures.
    pub reference: Option<Vec<RgbaImage>>,
    /// One nonempty inclusion bitmap per frame.
    pub scope: Vec<Vec<bool>>,
}
fn invalid(message: &str) -> Error {
    Error::Config(format!("captured sequence: {message}"))
}
fn unit(v: f64) -> bool {
    v.is_finite() && (0.0..=1.0).contains(&v)
}
impl Plan {
    /// Validate count, time equality, scope declarations and bounded paths.
    pub fn validate(&self) -> Result<()> {
        let n = self.candidate.len();
        if self.schema != PLAN_SCHEMA || !(3..=512).contains(&n) {
            return Err(invalid(
                "declare 3..512 observations and the supported schema",
            ));
        }
        let valid = |frames: &[Capture]| {
            frames.len() == n
                && frames
                    .iter()
                    .all(|f| f.timestamp_ms.is_finite() && f.timestamp_ms >= 0.0)
                && frames
                    .windows(2)
                    .all(|w| w[1].timestamp_ms > w[0].timestamp_ms)
        };
        if !valid(&self.candidate)
            || self.reference.as_ref().is_some_and(|r| {
                !valid(r)
                    || r.iter()
                        .zip(&self.candidate)
                        .any(|(a, b)| a.timestamp_ms != b.timestamp_ms)
            })
            || self.masks.as_ref().is_some_and(|m| m.len() != n)
            || self.id_buffers.as_ref().is_some_and(|m| m.len() != n)
            || (self.id_buffers.is_some() != !self.include_ids.is_empty())
            || self.include_ids.len() > 256
        {
            return Err(invalid(
                "counts, timestamps or scope declarations do not match",
            ));
        }
        for path in self
            .candidate
            .iter()
            .chain(self.reference.iter().flatten())
            .map(|f| &f.image)
            .chain(self.masks.iter().flatten())
            .chain(self.id_buffers.iter().flatten())
        {
            if path.len() > 1024
                || path.is_empty()
                || !std::path::Path::new(path)
                    .components()
                    .all(|c| matches!(c, std::path::Component::Normal(_)))
            {
                return Err(invalid("input paths must be contained relative paths"));
            }
        }
        Ok(())
    }
}
fn validate(plan: &Plan, images: &Images, tile: u32) -> Result<()> {
    plan.validate()?;
    if plan.alignment == Alignment::Dense && !cfg!(feature = "dense-motion") {
        return Err(Error::FeatureUnavailable {
            feature: "dense-motion",
        });
    }
    let n = plan.candidate.len();
    if images.candidate.len() != n
        || images.reference.as_ref().map(Vec::len) != plan.reference.as_ref().map(Vec::len)
        || images.scope.len() != n
        || !(4..=1024).contains(&tile)
    {
        return Err(invalid("decoded inputs or tile size do not match the plan"));
    }
    let dims = images.candidate[0].dimensions();
    let pixels = dims.0 as usize * dims.1 as usize;
    if dims.0 < 16
        || dims.1 < 16
        || dims.0 > 1024
        || dims.1 > 1024
        || pixels * n * (1 + usize::from(images.reference.is_some())) > (256 << 20) / 4
        || images
            .candidate
            .iter()
            .chain(images.reference.iter().flatten())
            .any(|f| f.dimensions() != dims || f.pixels().any(|p| p[3] != 255))
        || images
            .scope
            .iter()
            .any(|s| s.len() != pixels || !s.iter().any(|p| *p))
    {
        return Err(invalid(
            "equal opaque 16..1024 SDR images and nonempty scopes required; decoded limit 256 MiB",
        ));
    }
    Ok(())
}
// Reuse settling's spatial RGB trajectories instead of a second error engine.
fn trajectories(
    candidate: &[RgbaImage],
    reference: &[RgbaImage],
    scope: &[Vec<bool>],
    tile: u32,
) -> Result<Vec<Vec<Region>>> {
    let mut a = candidate.to_vec();
    let mut b = reference.to_vec();
    for ((a, b), s) in a.iter_mut().zip(&mut b).zip(scope) {
        for ((a, b), included) in a.pixels_mut().zip(b.pixels_mut()).zip(s) {
            if !included {
                *a = image::Rgba([0, 0, 0, 255]);
                *b = *a;
            }
        }
    }
    let r = crate::settling::analyze(
        &a,
        Some(&b),
        crate::settling::Policy {
            change_frame: 1,
            fps: 1.0,
            tile_size: tile,
            threshold: 0.0,
            consecutive: 1,
            final_frames: 1,
        },
    )?;
    let w = a[0].width();
    let mut out = vec![Vec::new(); a.len()];
    for t in r.tiles {
        let [x, y, rw, rh] = t.rect;
        for (i, row) in out.iter_mut().enumerate() {
            let count = (y..y + rh)
                .flat_map(|yy| (x..x + rw).map(move |xx| (yy * w + xx) as usize))
                .filter(|j| scope[i][*j])
                .count();
            if count > 0 {
                row.push(Region {
                    rect_px: t.rect,
                    error: t.error[i] * f64::from(rw * rh) / count as f64,
                });
            }
        }
    }
    Ok(out)
}
fn mean(a: &RgbaImage, b: &RgbaImage, s: &[bool]) -> f64 {
    let count = s.iter().filter(|v| **v).count();
    a.pixels()
        .zip(b.pixels())
        .zip(s)
        .filter(|(_, v)| **v)
        .map(|((a, b), _)| {
            (0..3)
                .map(|c| (f64::from(a[c]) - f64::from(b[c])).abs() / 255.0)
                .sum::<f64>()
                / 3.0
        })
        .sum::<f64>()
        / count as f64
}
fn correspondence(
    a: &RgbaImage,
    b: &RgbaImage,
    mode: Alignment,
) -> Result<(RgbaImage, Vec<bool>, Option<[f64; 3]>)> {
    let n = a.width() as usize * a.height() as usize;
    if mode == Alignment::None || a == b {
        return Ok((b.clone(), vec![true; n], None));
    }
    let (vectors, valid, shift) = match mode {
        Alignment::Translation => {
            let shift = crate::diagnostics::global_translation(a, b);
            let Some(v) = shift.filter(|s| s[2] >= 0.6 && s[0].abs() <= 64.0 && s[1].abs() <= 64.0)
            else {
                return Ok((b.clone(), vec![false; n], shift));
            };
            (vec![[v[0] as f32, v[1] as f32]; n], vec![true; n], Some(v))
        }
        Alignment::Dense => {
            let r = crate::dense_motion::review(
                a,
                b,
                [
                    crate::localized::digest(a.as_raw()),
                    crate::localized::digest(b.as_raw()),
                ],
                None,
                67.0,
                1.0,
            )?;
            let [field, _] = r.fields;
            let valid = field
                .states
                .iter()
                .map(|s| *s == crate::dense_motion::State::Qualified)
                .collect();
            (field.vectors, valid, None)
        }
        Alignment::None => return Ok((b.clone(), vec![true; n], None)),
    };
    let mut warped = b.clone();
    let mut support = vec![false; n];
    // Bounded nearest-sample correspondence preserves encoded samples; no invented border pixels.
    for (j, ((v, ok), p)) in vectors
        .iter()
        .zip(valid)
        .zip(warped.pixels_mut())
        .enumerate()
    {
        let x = (j as u32 % a.width()) as f32 + v[0];
        let y = (j as u32 / a.width()) as f32 + v[1];
        let x = x.round() as i32;
        let y = y.round() as i32;
        if ok && x >= 0 && y >= 0 && x < b.width() as i32 && y < b.height() as i32 {
            *p = *b.get_pixel(x as u32, y as u32);
            support[j] = true;
        }
    }
    Ok((warped, support, shift))
}
fn measurements(
    plan: &Plan,
    images: &Images,
    reference: &[RgbaImage],
    tile: u32,
    threshold: f64,
) -> Result<Vec<Frame>> {
    let raw = trajectories(&images.candidate, reference, &images.scope, tile)?;
    let mut warped = Vec::new();
    let mut supports = Vec::new();
    let mut shifts = Vec::new();
    for ((a, b), s) in reference.iter().zip(&images.candidate).zip(&images.scope) {
        let (w, mut support, shift) = correspondence(a, b, plan.alignment)?;
        for (ok, included) in support.iter_mut().zip(s) {
            *ok &= *included;
        }
        warped.push(w);
        supports.push(support);
        shifts.push(shift);
    }
    let aligned = trajectories(&warped, reference, &supports, tile)?;
    let mut residual_flicker = vec![None; reference.len()];
    let width = reference[0].width();
    for i in 1..reference.len() {
        let persistent: Vec<_> = images.scope[i - 1]
            .iter()
            .zip(&images.scope[i])
            .map(|(a, b)| *a && *b)
            .collect();
        let count = persistent.iter().filter(|v| **v).count();
        if count == 0
            || persistent
                .iter()
                .enumerate()
                .any(|(j, included)| *included && !(supports[i - 1][j] && supports[i][j]))
        {
            continue;
        }
        let mut worst = 0.0_f64;
        for r in &aligned[i] {
            let [x, y, w, h] = r.rect_px;
            let mut sum = 0.0;
            let mut count = 0;
            for yy in y..y + h {
                for xx in x..x + w {
                    let j = (yy * width + xx) as usize;
                    if !persistent[j] {
                        continue;
                    }
                    count += 1;
                    let a = warped[i - 1].get_pixel(xx, yy);
                    let b = warped[i].get_pixel(xx, yy);
                    let ra = reference[i - 1].get_pixel(xx, yy);
                    let rb = reference[i].get_pixel(xx, yy);
                    sum += (0..3)
                        .map(|c| {
                            ((f64::from(b[c]) - f64::from(rb[c]))
                                - (f64::from(a[c]) - f64::from(ra[c])))
                            .abs()
                                / 510.0
                        })
                        .sum::<f64>()
                        / 3.0;
                }
            }
            if count > 0 {
                worst = worst.max(sum / count as f64);
            }
        }
        residual_flicker[i] = Some(worst);
    }
    Ok((0..reference.len())
        .map(|i| {
            let pixels = images.scope[i].iter().filter(|v| **v).count();
            let aligned_pixels = supports[i].iter().filter(|v| **v).count();
            Frame {
                index: i,
                timestamp_ms: plan.candidate[i].timestamp_ms,
                pixels,
                raw_error: mean(&images.candidate[i], &reference[i], &images.scope[i]),
                local_error: raw[i].iter().map(|r| r.error).fold(0.0, f64::max),
                aligned_error: (aligned_pixels > 0)
                    .then(|| mean(&warped[i], &reference[i], &supports[i])),
                aligned_pixels,
                translation: shifts[i],
                regions: raw[i]
                    .iter()
                    .filter(|r| r.error > threshold)
                    .cloned()
                    .collect(),
                aligned_regions: aligned[i].clone(),
                residual_flicker: residual_flicker[i],
            }
        })
        .collect())
}
fn limits() -> Vec<String> {
    vec!["External capture/timestamp assertions are not rendering, skinning or capture attestation.".into(),"Scores are normalized encoded SDR RGB absolute errors, not perceptual visibility or causal deformation labels. Raw budgets remain authoritative.".into(),"Alignment is supplementary nearest-sample correspondence; global translation cannot align independent deformations. Dense confidence is heuristic. Missing support cannot establish consistency.".into()]
}
/// Compare matched animation captures, preserving raw localized failures.
pub fn animation(plan: Plan, images: &Images, policy: AnimationPolicy) -> Result<AnimationReport> {
    validate(&plan, images, policy.tile_size)?;
    if ![
        policy.maximum_frame_error,
        policy.maximum_local_error,
        policy.maximum_flicker,
    ]
    .into_iter()
    .all(unit)
    {
        return Err(invalid("invalid animation budgets"));
    }
    let reference = images
        .reference
        .as_ref()
        .ok_or_else(|| invalid("animation requires matched reference captures"))?;
    let frames = measurements(
        &plan,
        images,
        reference,
        policy.tile_size,
        policy.maximum_local_error,
    )?;
    let first_divergence_frame = frames.iter().position(|f| {
        f.raw_error > policy.maximum_frame_error || f.local_error > policy.maximum_local_error
    });
    let mut flicker = 0.0_f64;
    let mut first_flicker_frame = None;
    let supported = frames.iter().all(|f| f.aligned_pixels == f.pixels);
    for f in frames.iter().skip(1) {
        if let Some(change) = f.residual_flicker {
            flicker = flicker.max(change);
            if change > policy.maximum_flicker && first_flicker_frame.is_none() {
                first_flicker_frame = Some(f.index);
            }
        }
    }
    let supported = supported && frames.iter().skip(1).all(|f| f.residual_flicker.is_some());
    Ok(AnimationReport {
        schema: ANIMATION_SCHEMA.into(),
        verdict:
            if first_divergence_frame.is_none() && first_flicker_frame.is_none() && supported {
                "pass"
            } else {
                "regression"
            }
            .into(),
        plan,
        policy,
        frames,
        first_divergence_frame,
        temporal_flicker: supported.then_some(flicker),
        first_flicker_frame,
        limits: limits(),
    })
}
/// Measure a declared switch, or steady differences between paired levels.
pub fn transition(
    plan: Plan,
    images: &Images,
    policy: TransitionPolicy,
) -> Result<TransitionReport> {
    validate(&plan, images, policy.tile_size)?;
    let n = images.candidate.len();
    if ![
        policy.maximum_pop,
        policy.maximum_steady_error,
        policy.settle_threshold,
    ]
    .into_iter()
    .all(unit)
        || !policy.maximum_duration_ms.is_finite()
        || policy.maximum_duration_ms < 0.0
        || policy.window < 2
        || policy.window > n
        || policy.consecutive == 0
        || policy.consecutive > n
        || policy.change_frame.is_some_and(|c| {
            c < 2
                || c >= n
                || policy.consecutive > n - c
                || policy.window > c
                || policy.window > n - c
        })
        || (policy.change_frame.is_none() && images.reference.is_none())
    {
        return Err(invalid(
            "invalid transition windows or budgets; level pairs need reference",
        ));
    }
    let inferred = vec![images.candidate[n - 1].clone(); n];
    let target = images.reference.as_ref().unwrap_or(&inferred);
    let frames = measurements(
        &plan,
        images,
        target,
        policy.tile_size,
        policy.settle_threshold,
    )?;
    let temporal = |seq: &[RgbaImage]| -> Result<Vec<f64>> {
        let mut values = vec![0.0];
        for (i, w) in seq.windows(2).enumerate() {
            let scope: Vec<_> = images.scope[i]
                .iter()
                .zip(&images.scope[i + 1])
                .map(|(a, b)| *a && *b)
                .collect();
            if !scope.iter().any(|v| *v) {
                return Err(invalid("adjacent scopes have no persistent support"));
            }
            values.push(mean(&w[0], &w[1], &scope));
        }
        Ok(values)
    };
    let candidate_delta = temporal(&images.candidate)?;
    let motion_baseline = if let Some(reference) = &images.reference {
        temporal(reference)?
    } else {
        let c = policy
            .change_frame
            .ok_or_else(|| invalid("declare switch"))?;
        let start = c.saturating_sub(policy.window).max(1);
        let v = if start < c {
            candidate_delta[start..c].iter().sum::<f64>() / (c - start) as f64
        } else {
            0.0
        };
        vec![v; n]
    };
    let pop_curve: Vec<_> = candidate_delta
        .iter()
        .zip(&motion_baseline)
        .map(|(a, b)| (a - b).max(0.0))
        .collect();
    let settle_frame = policy.change_frame.and_then(|c| {
        (c..=n - policy.consecutive).find(|i| {
            frames[*i..*i + policy.consecutive]
                .iter()
                .all(|f| f.local_error <= policy.settle_threshold)
        })
    });
    let transition_duration_ms = settle_frame
        .zip(policy.change_frame)
        .map(|(s, c)| plan.candidate[s].timestamp_ms - plan.candidate[c].timestamp_ms);
    // Observe all post-request edges through the convergence confirmation window.
    let end = settle_frame.map_or(n, |s| s + policy.consecutive);
    let popping = policy
        .change_frame
        .map(|c| pop_curve[c..end].iter().copied().fold(0.0, f64::max));
    let pop_frame = policy.change_frame.zip(popping).and_then(|(c, p)| {
        (p > 0.0)
            .then(|| (c..end).find(|i| pop_curve[*i] == p))
            .flatten()
    });
    let steady_state_difference = if images.reference.is_some() {
        frames[n - policy.window..]
            .iter()
            .map(|f| f.raw_error)
            .sum::<f64>()
            / policy.window as f64
    } else {
        let c = policy
            .change_frame
            .ok_or_else(|| invalid("declare switch"))?;
        let mut sum = 0.0;
        for (a, b) in (c - policy.window..c).zip(n - policy.window..n) {
            let scope: Vec<_> = images.scope[a]
                .iter()
                .zip(&images.scope[b])
                .map(|(a, b)| *a && *b)
                .collect();
            if !scope.iter().any(|v| *v) {
                return Err(invalid("pre/post scopes have no persistent support"));
            }
            sum += mean(&images.candidate[a], &images.candidate[b], &scope);
        }
        sum / policy.window as f64
    };
    let pass = steady_state_difference <= policy.maximum_steady_error
        && policy.change_frame.is_none_or(|_| {
            popping.is_some_and(|p| p <= policy.maximum_pop)
                && transition_duration_ms.is_some_and(|t| t <= policy.maximum_duration_ms)
        });
    let steady_state_basis = if images.reference.is_some() {
        "matched_reference"
    } else {
        "pre_post_windows"
    }
    .into();
    let mut limits = limits();
    limits.push("Without reference, the pre-switch raw temporal mean is the motion baseline and the last image is the target. Moving viewpoints/content can confound convergence; independent target captures are recommended. Steady level difference compares pre/post windows and therefore includes temporal motion; it is not an independent same-timestamp level comparison.".into());
    limits.push("Level pairs have no observed switch: popping and duration remain null. Duration is sampled convergence, not continuous-time transition length or future stability.".into());
    Ok(TransitionReport {
        schema: TRANSITION_SCHEMA.into(),
        verdict: if pass { "pass" } else { "regression" }.into(),
        plan,
        policy,
        frames,
        pop_curve,
        popping,
        pop_frame,
        motion_baseline,
        settle_frame,
        transition_duration_ms,
        steady_state_difference,
        steady_state_basis,
        limits,
    })
}
