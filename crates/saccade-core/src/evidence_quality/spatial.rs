//! Area pyramids and tile statistics distinguish fine texture from clustered bias.
use super::effect::Selection;
use crate::diagnostics::ChangeClass;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Spatial report discriminator.
pub const SCHEMA: &str = "saccade-spatial-evidence.v1";
/// Threshold semantics, frozen on generated fixtures before held-out evaluation.
pub const POLICY_VERSION: &str = "spatial-1";
/// Optional background definition for gap/coverage statistics.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Background {
    /// Luminance at or below a normalized sRGB threshold.
    Luminance {
        /// Upper bound.
        max: f64,
    },
    /// All RGB channels within a normalized distance from a colour.
    Colour {
        /// Normalized RGB.
        rgb: [f64; 3],
        /// Maximum per-channel distance.
        tolerance: f64,
    },
    /// Supplied mask or side-specific native layer selection marks background.
    Selection {
        /// Inclusion source.
        selection: Selection,
    },
}
/// Versioned, recorded numerical policy; opt-in to normal compare.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
    /// Must equal POLICY_VERSION.
    pub version: String,
    /// Integer area-reduction factors; full resolution is always reported.
    pub scales: Vec<u32>,
    /// Regular tile width and height in capture pixels; edge tiles are retained.
    pub tile_size: u32,
    /// Optional pinned SSIMULACRA2 at meaningful (>=8x8 opaque) scales.
    pub ssimulacra2: bool,
    /// Coarse normalized mean absolute RGB tolerance (half an 8-bit code).
    pub coarse_mad_max: f64,
    /// Full-resolution/coarse FLIP attenuation threshold for texture noise.
    pub coarse_flip_ratio_max: f64,
    /// Absolute tile luminance shift threshold.
    pub absolute_shift: f64,
    /// Relative tile luminance shift threshold.
    pub relative_shift: f64,
    /// Minimum luminance denominator for relative shifts and contrast.
    pub luminance_floor: f64,
    /// Normal-approximation CI multiplier over paired pixel residuals.
    pub ci_z: f64,
    /// Minimum connected significant tiles for a clustered bias finding.
    pub min_cluster_tiles: usize,
    /// Sparse absolute outlier tiles tolerated by small coarse-error evidence.
    pub sparse_tiles: usize,
    /// Fraction of relative-shifted tiles above which bias cannot be called benign.
    pub shifted_share_max: f64,
    /// Maximum relative change in absolute Laplacian detail energy.
    pub detail_ratio_tolerance: f64,
    /// Minimum absolute gap-fraction change to mark a tile.
    pub gap_shift: f64,
    /// Optional declared background predicate.
    pub background: Option<Background>,
    /// If true, use the structural verdict for acceptance; historical metric remains recorded.
    pub decide: bool,
    /// Maximum gallery regions; zero disables generated crops.
    pub gallery_top: usize,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            version: POLICY_VERSION.into(),
            scales: vec![2, 4, 8],
            tile_size: 32,
            ssimulacra2: false,
            coarse_mad_max: 0.002,
            coarse_flip_ratio_max: 0.3,
            absolute_shift: 0.02,
            relative_shift: 0.02,
            luminance_floor: 0.02,
            ci_z: 1.96,
            min_cluster_tiles: 3,
            sparse_tiles: 2,
            shifted_share_max: 0.25,
            detail_ratio_tolerance: 0.1,
            gap_shift: 0.05,
            background: None,
            decide: false,
            gallery_top: 5,
        }
    }
}
impl Policy {
    /// Validate all controls without acquiring images.
    pub fn validate(&self) -> Result<()> {
        if self.version != POLICY_VERSION
            || self.scales.is_empty()
            || self.scales.len() > 8
            || self.scales.iter().any(|s| *s < 2 || *s > 64)
            || self
                .scales
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != self.scales.len()
            || self.tile_size < 4
            || self.tile_size > 4096
            || self.min_cluster_tiles == 0
            || self.gallery_top > 32
        {
            return Err(Error::Config(
                "invalid spatial policy version/grid/scales".into(),
            ));
        }
        for v in [
            self.coarse_mad_max,
            self.coarse_flip_ratio_max,
            self.absolute_shift,
            self.relative_shift,
            self.luminance_floor,
            self.ci_z,
            self.shifted_share_max,
            self.detail_ratio_tolerance,
            self.gap_shift,
        ] {
            if !v.is_finite() || v <= 0.0 {
                return Err(Error::Config(
                    "spatial thresholds must be finite and positive".into(),
                ));
            }
        }
        if self.shifted_share_max > 1.0 || self.gap_shift > 1.0 {
            return Err(Error::Config("spatial shares must be <=1".into()));
        }
        if let Some(background) = &self.background {
            match background {
                Background::Luminance { max } if !max.is_finite() || !(0.0..=1.0).contains(max) => {
                    return Err(Error::Config(
                        "background luminance must be in [0,1]".into(),
                    ));
                }
                Background::Colour { rgb, tolerance }
                    if !tolerance.is_finite()
                        || *tolerance < 0.0
                        || rgb
                            .iter()
                            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v)) =>
                {
                    return Err(Error::Config("invalid background colour".into()));
                }
                _ => {}
            }
        }
        #[cfg(not(feature = "compression"))]
        if self.ssimulacra2 {
            return Err(Error::FeatureUnavailable {
                feature: "compression",
            });
        }
        Ok(())
    }
}
/// Independent perceptual statistics and floating area RGB difference at a scale.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scale {
    /// Area reduction divisor; 1 is the original.
    pub divisor: u32,
    /// Reduced dimensions.
    pub dimensions: [u32; 2],
    /// Mean FLIP.
    pub mean_flip: f64,
    /// Maximum FLIP.
    pub max_flip: f64,
    /// Floating area mean absolute normalized RGB difference.
    pub mad_rgb: f64,
    /// SSIMULACRA2, absent for disabled/unmeaningful scales.
    pub ssimulacra2: Option<f64>,
}
/// One tile, including signed effects that max FLIP obscures.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    /// `[x,y,width,height]` in full-resolution pixels.
    #[cfg_attr(
        feature = "schema",
        schemars(description = "[x,y,width,height] in full-resolution pixels.")
    )]
    pub rect_px: [u32; 4],
    /// Included pixels.
    pub pixels: u64,
    /// Baseline mean normalized luminance.
    pub baseline_mean: f64,
    /// Candidate mean luminance.
    pub candidate_mean: f64,
    /// Candidate minus baseline.
    pub signed_shift: f64,
    /// Signed shift divided by max(baseline_mean,luminance_floor).
    pub relative_shift: f64,
    /// Normal approximation CI over paired pixel shifts.
    pub shift_ci: [f64; 2],
    /// Baseline coefficient of variation.
    pub baseline_contrast: f64,
    /// Candidate coefficient of variation.
    pub candidate_contrast: f64,
    /// Candidate variance / baseline variance, null if baseline zero and candidate nonzero.
    pub variance_ratio: Option<f64>,
    /// Candidate/base absolute Laplacian energy.
    pub detail_energy_ratio: Option<f64>,
    /// Baseline share satisfying the declared background criterion.
    pub baseline_gap: Option<f64>,
    /// Candidate gap share.
    pub candidate_gap: Option<f64>,
    /// Signed gap share change.
    pub gap_change: Option<f64>,
    /// Full-resolution mean FLIP.
    pub mean_flip: f64,
    /// Coarsest-scale mean FLIP for the tile footprint.
    pub coarse_mean_flip: f64,
    /// CI excludes zero and absolute threshold exceeded.
    pub absolute_shifted: bool,
    /// CI excludes zero and relative threshold exceeded.
    pub relative_shifted: bool,
}
/// Spatially connected tile finding.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Region {
    /// Pixel bounding box.
    pub rect_px: [u32; 4],
    /// Contributing tile indexes in row-major order.
    pub tiles: Vec<usize>,
    /// Signed average tile bias.
    pub signed_shift: f64,
    /// Reason categories (bias, coverage or detail).
    pub reasons: Vec<String>,
}
/// Structural evidence; the original FLIP value remains available independently.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpatialReport {
    /// Versioned artifact discriminator.
    pub schema: String,
    /// Exact thresholds and version.
    pub policy: Policy,
    /// Extended ChangeClass.
    pub class: ChangeClass,
    /// Full and reduced statistics.
    pub scales: Vec<Scale>,
    /// Regular row-major grid.
    pub tiles: Vec<Tile>,
    /// Connected systematic changes.
    pub regions: Vec<Region>,
    /// Whole-frame detail-energy ratio.
    pub detail_energy_ratio: Option<f64>,
    /// Number of absolute shifted tiles.
    pub absolute_shifted_tiles: usize,
    /// Absolute shifted-tile share.
    pub absolute_shifted_share: f64,
    /// Relative shifted-tile share.
    pub relative_shifted_share: f64,
    /// Baseline/candidate non-background coverage ratio.
    pub coverage_ratio: Option<f64>,
    /// Gap-mask XOR / union, if a background predicate was declared.
    pub silhouette_disagreement: Option<f64>,
    /// Interpretation limits.
    pub limits: Vec<String>,
}
/// Flatten normalized straight-alpha sRGB over black before luminance/detail statistics.
pub fn rgb(image: &image::RgbaImage) -> Vec<[f64; 3]> {
    image
        .pixels()
        .map(|p| {
            let a = f64::from(p[3]) / 255.0;
            [
                f64::from(p[0]) / 255.0 * a,
                f64::from(p[1]) / 255.0 * a,
                f64::from(p[2]) / 255.0 * a,
            ]
        })
        .collect()
}
/// Normalized sRGB luminance, not physical linear radiance.
pub fn luminance(rgb: [f64; 3]) -> f64 {
    rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722
}
/// Descriptive single-capture tile statistics, without a paired change verdict.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureTile {
    /// Capture-pixel rectangle; partial edge tiles are retained.
    pub rect_px: [u32; 4],
    /// Number of contributing pixels.
    pub pixels: u64,
    /// Mean normalized sRGB luminance over black.
    pub mean_luminance: f64,
    /// Population luminance variance.
    pub variance: f64,
    /// Standard deviation divided by the spatial policy's luminance floor or mean.
    pub contrast: f64,
    /// Absolute interior Laplacian energy, as in paired spatial evidence.
    pub detail_energy: f64,
}
/// Measure the same luminance, contrast and detail basis used by paired evidence.
/// No FLIP, bias, confidence interval or acceptance class exists for a single image.
pub fn capture_tiles(image: &image::RgbaImage, tile_size: u32) -> Result<Vec<CaptureTile>> {
    let (w, h) = image.dimensions();
    if !(4..=4096).contains(&tile_size)
        || w == 0
        || h == 0
        || u64::from(w) * u64::from(h) > 16_777_216
    {
        return Err(Error::Config(
            "invalid capture tile grid or pixel budget".into(),
        ));
    }
    let l: Vec<_> = rgb(image).into_iter().map(luminance).collect();
    let floor = Policy::default().luminance_floor;
    let mut tiles = Vec::new();
    for y in (0..h).step_by(tile_size as usize) {
        for x in (0..w).step_by(tile_size as usize) {
            let r = [x, y, tile_size.min(w - x), tile_size.min(h - y)];
            let luminances = &l;
            let values = || {
                (y..y + r[3])
                    .flat_map(|yy| (x..x + r[2]).map(move |xx| luminances[(yy * w + xx) as usize]))
            };
            let count = u64::from(r[2]) * u64::from(r[3]);
            let mean = values().sum::<f64>() / count as f64;
            let variance = values().map(|v| (v - mean).powi(2)).sum::<f64>() / count as f64;
            tiles.push(CaptureTile {
                rect_px: r,
                pixels: count,
                mean_luminance: mean,
                variance,
                contrast: variance.sqrt() / mean.max(floor),
                detail_energy: detail(&l, w, h, r),
            });
        }
    }
    Ok(tiles)
}
/// Exact area integration retaining all edge pixels; no nearest-neighbor sampling.
pub fn area(values: &[[f64; 3]], w: u32, h: u32, divisor: u32) -> (Vec<[f64; 3]>, u32, u32) {
    let nw = (w / divisor).max(1);
    let nh = (h / divisor).max(1);
    let mut result = Vec::with_capacity((nw * nh) as usize);
    for oy in 0..nh {
        for ox in 0..nw {
            let x0 = f64::from(ox) * f64::from(w) / f64::from(nw);
            let x1 = f64::from(ox + 1) * f64::from(w) / f64::from(nw);
            let y0 = f64::from(oy) * f64::from(h) / f64::from(nh);
            let y1 = f64::from(oy + 1) * f64::from(h) / f64::from(nh);
            let mut sum = [0.0; 3];
            for y in y0.floor() as u32..(y1.ceil() as u32).min(h) {
                for x in x0.floor() as u32..(x1.ceil() as u32).min(w) {
                    let weight = (x1.min(f64::from(x + 1)) - x0.max(f64::from(x)))
                        * (y1.min(f64::from(y + 1)) - y0.max(f64::from(y)));
                    for (ch, s) in sum.iter_mut().enumerate() {
                        *s += values[(y * w + x) as usize][ch] * weight;
                    }
                }
            }
            result.push(sum.map(|v| v / ((x1 - x0) * (y1 - y0))));
        }
    }
    (result, nw, nh)
}
fn image(values: &[[f64; 3]], w: u32, h: u32) -> image::RgbImage {
    image::RgbImage::from_fn(w, h, |x, y| {
        image::Rgb(values[(y * w + x) as usize].map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8))
    })
}
fn ratio(a: f64, b: f64) -> Option<f64> {
    if a.abs() < 1e-15 {
        if b.abs() < 1e-15 { Some(1.0) } else { None }
    } else {
        Some(b / a)
    }
}
/// Absolute Laplacian energy in a tile, using only valid interior pixels.
pub fn detail(l: &[f64], w: u32, h: u32, r: [u32; 4]) -> f64 {
    detail_scoped(l, w, h, r, None)
}
/// Absolute Laplacian energy only where the full stencil belongs to the scope.
pub fn detail_scoped(l: &[f64], w: u32, h: u32, r: [u32; 4], excluded: Option<&[bool]>) -> f64 {
    let [x, y, rw, rh] = r;
    if rw < 3 || rh < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    let mut n = 0;
    for yy in y + 1..(y + rh - 1).min(h - 1) {
        for xx in x + 1..(x + rw - 1).min(w - 1) {
            let i = (yy * w + xx) as usize;
            if excluded.is_some_and(|m| {
                [i, i - 1, i + 1, i - w as usize, i + w as usize]
                    .iter()
                    .any(|j| m[*j])
            }) {
                continue;
            }
            sum += (4.0 * l[i] - l[i - 1] - l[i + 1] - l[i - w as usize] - l[i + w as usize]).abs();
            n += 1;
        }
    }
    if n == 0 { 0.0 } else { sum / n as f64 }
}
/// Resolve a declared background criterion on one supplied capture.
pub fn background(
    criterion: &Background,
    img: &image::RgbaImage,
    path: &Path,
    root: &Path,
) -> Result<Vec<bool>> {
    let samples = rgb(img);
    match criterion {
        Background::Selection { selection } => {
            super::effect::select(selection, path, root, img.dimensions())
        }
        Background::Luminance { max } => {
            Ok(samples.into_iter().map(|p| luminance(p) <= *max).collect())
        }
        Background::Colour { rgb, tolerance } => Ok(samples
            .into_iter()
            .map(|p| p.iter().zip(rgb).all(|(a, b)| (a - b).abs() <= *tolerance))
            .collect()),
    }
}
/// Analyze the pair using the already computed full-resolution FLIP map.
/// `excluded` restricts every statistic; background masks are optional separate gap evidence.
#[allow(clippy::too_many_arguments)]
pub fn analyze(
    base: &image::RgbaImage,
    candidate: &image::RgbaImage,
    errors: &[f32],
    excluded: Option<&[bool]>,
    gaps: Option<(&[bool], &[bool])>,
    policy: &Policy,
    opts: &crate::compare::CompareOptions,
    fallback: ChangeClass,
) -> Result<SpatialReport> {
    policy.validate()?;
    let (w, h) = base.dimensions();
    let n = (u64::from(w) * u64::from(h)) as usize;
    if n == 0
        || n > 16_777_216
        || candidate.dimensions() != base.dimensions()
        || errors.len() != n
        || excluded.is_some_and(|m| m.len() != n)
        || gaps.is_some_and(|(b, c)| b.len() != n || c.len() != n)
    {
        return Err(Error::Config("spatial pair/map dimensions differ".into()));
    }
    let included = |i: usize| !excluded.is_some_and(|m| m[i]);
    if !(0..n).any(included) {
        return Err(Error::Config("spatial scope is empty".into()));
    }
    let b = rgb(base);
    let mut c = rgb(candidate);
    // Excluded samples are neutralized before area filtering to prevent hidden differences leaking into scope metrics.
    for i in 0..n {
        if !included(i) {
            c[i] = b[i];
        }
    }
    let lb: Vec<_> = b.iter().copied().map(luminance).collect();
    let lc: Vec<_> = c.iter().copied().map(luminance).collect();
    let mut scales = Vec::new();
    let mut coarsest = None;
    let mut divisors = policy.scales.clone();
    divisors.push(1);
    divisors.sort_unstable();
    for divisor in divisors {
        let mut weighted_b = b.clone();
        let mut weighted_c = c.clone();
        let weights: Vec<_> = (0..n)
            .map(|i| if included(i) { [1.0; 3] } else { [0.0; 3] })
            .collect();
        for i in 0..n {
            if !included(i) {
                weighted_b[i] = [0.0; 3];
                weighted_c[i] = [0.0; 3];
            }
        }
        let (weight, _, _) = area(&weights, w, h, divisor);
        let (mut sb, sw, sh) = area(&weighted_b, w, h, divisor);
        let (mut sc, _, _) = area(&weighted_c, w, h, divisor);
        for i in 0..sb.len() {
            if weight[i][0] > 0.0 {
                for ch in 0..3 {
                    sb[i][ch] /= weight[i][0];
                    sc[i][ch] /= weight[i][0];
                }
            }
        }
        let reduced_excluded: Vec<_> = weight.iter().map(|v| v[0] == 0.0).collect();
        let bi = image(&sb, sw, sh);
        let ci = image(&sc, sw, sh);
        let cmp = crate::compare::compare(&ci, &bi, opts)?;
        let selected_weight = weight.iter().map(|v| v[0]).sum::<f64>();
        let mad = sb
            .iter()
            .zip(&sc)
            .zip(&weight)
            .map(|((b, c), weight)| {
                b.iter().zip(c).map(|(b, c)| (c - b).abs()).sum::<f64>() * weight[0]
            })
            .sum::<f64>()
            / (selected_weight * 3.0);
        let scored = if divisor == 1 {
            crate::compare::masked_metrics(errors, excluded, w, h, [0, 0, w, h])
        } else {
            crate::compare::masked_metrics(
                &cmp.error_map,
                Some(&reduced_excluded),
                sw,
                sh,
                [0, 0, sw, sh],
            )
        }
        .ok_or_else(|| Error::Config("empty reduced scope".into()))?;
        let mut quality = None;
        #[cfg(feature = "compression")]
        if policy.ssimulacra2
            && excluded.is_none()
            && sw >= 8
            && sh >= 8
            && base.pixels().chain(candidate.pixels()).all(|p| p[3] == 255)
        {
            quality = Some(crate::quality::score(&bi, &ci)?);
        }
        #[cfg(not(feature = "compression"))]
        let _ = &mut quality;
        scales.push(Scale {
            divisor,
            dimensions: [sw, sh],
            mean_flip: scored.mean,
            max_flip: scored.max,
            mad_rgb: mad,
            ssimulacra2: quality,
        });
        coarsest = Some((sw, sh, cmp.error_map));
    }
    let (cw, ch, coarse) =
        coarsest.ok_or_else(|| Error::Config("missing spatial scales".into()))?;
    let cols = w.div_ceil(policy.tile_size);
    let rows = h.div_ceil(policy.tile_size);
    let mut tiles = Vec::new();
    for ty in 0..rows {
        for tx in 0..cols {
            let x = tx * policy.tile_size;
            let y = ty * policy.tile_size;
            let r = [
                x,
                y,
                policy.tile_size.min(w - x),
                policy.tile_size.min(h - y),
            ];
            let indexes: Vec<_> = (y..y + r[3])
                .flat_map(|yy| (x..x + r[2]).map(move |xx| (yy * w + xx) as usize))
                .filter(|i| included(*i))
                .collect();
            if indexes.is_empty() {
                tiles.push(Tile {
                    rect_px: r,
                    pixels: 0,
                    baseline_mean: 0.0,
                    candidate_mean: 0.0,
                    signed_shift: 0.0,
                    relative_shift: 0.0,
                    shift_ci: [0.0; 2],
                    baseline_contrast: 0.0,
                    candidate_contrast: 0.0,
                    variance_ratio: None,
                    detail_energy_ratio: None,
                    baseline_gap: None,
                    candidate_gap: None,
                    gap_change: None,
                    mean_flip: 0.0,
                    coarse_mean_flip: 0.0,
                    absolute_shifted: false,
                    relative_shifted: false,
                });
                continue;
            }
            let count = indexes.len() as f64;
            let bm = indexes.iter().map(|i| lb[*i]).sum::<f64>() / count;
            let cm = indexes.iter().map(|i| lc[*i]).sum::<f64>() / count;
            let shift = cm - bm;
            let bv = indexes.iter().map(|i| (lb[*i] - bm).powi(2)).sum::<f64>() / count;
            let cv = indexes.iter().map(|i| (lc[*i] - cm).powi(2)).sum::<f64>() / count;
            let dv = indexes
                .iter()
                .map(|i| (lc[*i] - lb[*i] - shift).powi(2))
                .sum::<f64>()
                / (count - 1.0).max(1.0);
            let half = policy.ci_z * (dv / count).sqrt();
            let ci = [shift - half, shift + half];
            let significant = ci[0] > 0.0 || ci[1] < 0.0;
            let rel = shift / bm.max(policy.luminance_floor);
            let (bg, cg) = gaps.map_or((None, None), |(b, c)| {
                (
                    Some(indexes.iter().filter(|i| b[**i]).count() as f64 / count),
                    Some(indexes.iter().filter(|i| c[**i]).count() as f64 / count),
                )
            });
            let cx0 = u64::from(x) * u64::from(cw) / u64::from(w);
            let cx1 = (u64::from(x + r[2]) * u64::from(cw)).div_ceil(u64::from(w));
            let cy0 = u64::from(y) * u64::from(ch) / u64::from(h);
            let cy1 = (u64::from(y + r[3]) * u64::from(ch)).div_ceil(u64::from(h));
            let cs: Vec<_> = (cy0..cy1)
                .flat_map(|yy| (cx0..cx1).map(move |xx| (yy * u64::from(cw) + xx) as usize))
                .collect();
            tiles.push(Tile {
                rect_px: r,
                pixels: indexes.len() as u64,
                baseline_mean: bm,
                candidate_mean: cm,
                signed_shift: shift,
                relative_shift: rel,
                shift_ci: ci,
                baseline_contrast: bv.sqrt() / bm.max(policy.luminance_floor),
                candidate_contrast: cv.sqrt() / cm.max(policy.luminance_floor),
                variance_ratio: ratio(bv, cv),
                detail_energy_ratio: ratio(
                    detail_scoped(&lb, w, h, r, excluded),
                    detail_scoped(&lc, w, h, r, excluded),
                ),
                baseline_gap: bg,
                candidate_gap: cg,
                gap_change: bg.zip(cg).map(|(b, c)| c - b),
                mean_flip: indexes.iter().map(|i| f64::from(errors[*i])).sum::<f64>() / count,
                coarse_mean_flip: cs.iter().map(|i| f64::from(coarse[*i])).sum::<f64>()
                    / cs.len().max(1) as f64,
                absolute_shifted: significant && shift.abs() > policy.absolute_shift,
                relative_shifted: significant && rel.abs() > policy.relative_shift,
            });
        }
    }
    let active = tiles.iter().filter(|t| t.pixels > 0).count().max(1);
    let abs = tiles.iter().filter(|t| t.absolute_shifted).count();
    let rel = tiles.iter().filter(|t| t.relative_shifted).count();
    let energy = ratio(
        detail_scoped(&lb, w, h, [0, 0, w, h], excluded),
        detail_scoped(&lc, w, h, [0, 0, w, h], excluded),
    );
    let (coverage_ratio, silhouette) = gaps.map_or((None, None), |(gb, gc)| {
        let nb = (0..n).filter(|i| included(*i) && !gb[*i]).count();
        let nc = (0..n).filter(|i| included(*i) && !gc[*i]).count();
        let union = (0..n)
            .filter(|i| included(*i) && (!gb[*i] || !gc[*i]))
            .count();
        let xor = (0..n).filter(|i| included(*i) && gb[*i] != gc[*i]).count();
        (
            ratio(nb as f64, nc as f64),
            Some(xor as f64 / union.max(1) as f64),
        )
    });
    let q4 = scales
        .iter()
        .find(|s| s.divisor == 4)
        .unwrap_or(&scales[scales.len() - 1]);
    let small = q4.mad_rgb <= policy.coarse_mad_max;
    let detail_change = energy.is_none_or(|r| (r - 1.0).abs() > policy.detail_ratio_tolerance);
    let coverage_change = coverage_ratio.is_some_and(|r| (r - 1.0).abs() > policy.gap_shift)
        || silhouette.is_some_and(|v| v > policy.gap_shift);
    let benign = small
        && !detail_change
        && !coverage_change
        && abs <= policy.sparse_tiles
        && rel as f64 / active as f64 <= policy.shifted_share_max;
    let selected: Vec<_> = tiles
        .iter()
        .map(|t| {
            !benign
                && (t.absolute_shifted
                    || t.relative_shifted
                    || t.gap_change.is_some_and(|v| v.abs() > policy.gap_shift)
                    || (!small
                        && detail_change
                        && t.detail_energy_ratio
                            .is_none_or(|r| (r - 1.0).abs() > policy.detail_ratio_tolerance)))
        })
        .collect();
    let regions = clusters(&tiles, &selected, cols, policy.min_cluster_tiles);
    let attenuated = scales
        .last()
        .is_some_and(|s| s.mean_flip <= scales[0].mean_flip * policy.coarse_flip_ratio_max);
    let class = if base == candidate {
        ChangeClass::Identical
    } else if benign || (attenuated && regions.is_empty() && !detail_change && !coverage_change) {
        ChangeClass::TextureNoiseOnly
    } else if !regions.is_empty() || coverage_change {
        ChangeClass::SystematicShift
    } else {
        fallback
    };
    Ok(SpatialReport {schema:SCHEMA.into(),policy:policy.clone(),class,scales,tiles,regions,detail_energy_ratio:energy,absolute_shifted_tiles:abs,absolute_shifted_share:abs as f64/active as f64,relative_shifted_share:rel as f64/active as f64,coverage_ratio,silhouette_disagreement:silhouette,limits:vec!["Normalized sRGB over black; area RGB MAD retains floating means; perceptual metrics use rounded RGB8 area images.".into(),"Pixel CI assumes independent residuals; spatial correlation reduces its inferential strength. Thresholds and connected tiles supply additional practical gates.".into(),"Scope exclusions neutralize samples before downsampling; texture classification is evidence, not causal or timing qualification.".into()]})
}
/// Four-connected tile components, retaining reason categories and signed bias.
pub fn clusters(tiles: &[Tile], selected: &[bool], cols: u32, min_tiles: usize) -> Vec<Region> {
    let mut seen = vec![false; tiles.len()];
    let mut result = Vec::new();
    for start in 0..tiles.len() {
        if seen[start] || !selected[start] {
            continue;
        }
        let mut queue = std::collections::VecDeque::from([start]);
        seen[start] = true;
        let mut group = Vec::new();
        while let Some(i) = queue.pop_front() {
            group.push(i);
            let x = i % cols as usize;
            for next in [
                (x > 0).then(|| i - 1),
                (x + 1 < cols as usize).then(|| i + 1),
                i.checked_sub(cols as usize),
                i.checked_add(cols as usize).filter(|i| *i < tiles.len()),
            ]
            .into_iter()
            .flatten()
            {
                if !seen[next] && selected[next] {
                    seen[next] = true;
                    queue.push_back(next);
                }
            }
        }
        if group.len() < min_tiles {
            continue;
        }
        group.sort_unstable();
        let x = group
            .iter()
            .map(|i| tiles[*i].rect_px[0])
            .min()
            .unwrap_or(0);
        let y = group
            .iter()
            .map(|i| tiles[*i].rect_px[1])
            .min()
            .unwrap_or(0);
        let ex = group
            .iter()
            .map(|i| tiles[*i].rect_px[0] + tiles[*i].rect_px[2])
            .max()
            .unwrap_or(x);
        let ey = group
            .iter()
            .map(|i| tiles[*i].rect_px[1] + tiles[*i].rect_px[3])
            .max()
            .unwrap_or(y);
        let shift = group.iter().map(|i| tiles[*i].signed_shift).sum::<f64>() / group.len() as f64;
        let mut reasons = Vec::new();
        if group
            .iter()
            .any(|i| tiles[*i].absolute_shifted || tiles[*i].relative_shifted)
        {
            reasons.push("signed_bias".into());
        }
        if group
            .iter()
            .any(|i| tiles[*i].gap_change.is_some_and(|g| g != 0.0))
        {
            reasons.push("coverage".into());
        }
        if reasons.is_empty() {
            reasons.push("detail_energy".into());
        }
        result.push(Region {
            rect_px: [x, y, ex - x, ey - y],
            tiles: group,
            signed_shift: shift,
            reasons,
        });
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_tiles_keep_edges_and_use_alpha_over_black_without_a_verdict() {
        let image = image::RgbaImage::from_fn(5, 4, |x, _| {
            image::Rgba([255, 255, 255, if x < 2 { 0 } else { 255 }])
        });
        let tiles = capture_tiles(&image, 4).unwrap();
        assert_eq!(tiles.len(), 2);
        assert_eq!(tiles[0].pixels, 16);
        assert!((tiles[0].mean_luminance - 0.5).abs() < 1e-12);
        assert!((tiles[0].variance - 0.25).abs() < 1e-12);
        assert!((tiles[0].contrast - 1.0).abs() < 1e-12);
        assert!(tiles[0].detail_energy > 0.0);
        assert_eq!(tiles[1].rect_px, [4, 0, 1, 4]);
        assert!((tiles[1].mean_luminance - 1.0).abs() < 1e-12);
        assert_eq!(tiles[1].variance, 0.0);
        assert_eq!(tiles[1].detail_energy, 0.0);
        assert!(capture_tiles(&image, 0).is_err());
        assert!(capture_tiles(&image, 4097).is_err());
        assert!(capture_tiles(&image::RgbaImage::new(0, 4), 4).is_err());
    }
    fn analyze_pair(b: &image::RgbaImage, c: &image::RgbaImage) -> SpatialReport {
        let opts = crate::compare::CompareOptions::default();
        let cmp = crate::compare::compare_rgba(c, b, &opts).unwrap();
        analyze(
            b,
            c,
            &cmp.error_map,
            None,
            None,
            &Policy::default(),
            &opts,
            ChangeClass::LocalStructure,
        )
        .unwrap()
    }
    #[cfg(feature = "compression")]
    #[test]
    fn perceptual_quality_is_only_scored_for_opaque_meaningful_scales() {
        let image = image::RgbaImage::from_fn(32, 32, |x, y| {
            image::Rgba([((x * 19 + y * 31) % 256) as u8; 4])
        });
        let mut image = image;
        for p in image.pixels_mut() {
            p[3] = 255;
        }
        let policy = Policy {
            ssimulacra2: true,
            ..Default::default()
        };
        let report = analyze(
            &image,
            &image,
            &vec![0.0; 1024],
            None,
            None,
            &policy,
            &Default::default(),
            ChangeClass::Identical,
        )
        .unwrap();
        assert!(
            report
                .scales
                .iter()
                .filter(|s| s.dimensions[0] >= 8)
                .all(|s| s.ssimulacra2.is_some_and(|v| v > 99.0))
        );
        assert!(
            report
                .scales
                .iter()
                .filter(|s| s.dimensions[0] < 8)
                .all(|s| s.ssimulacra2.is_none())
        );
        image.get_pixel_mut(0, 0)[3] = 128;
        let report = analyze(
            &image,
            &image,
            &vec![0.0; 1024],
            None,
            None,
            &policy,
            &Default::default(),
            ChangeClass::Identical,
        )
        .unwrap();
        assert!(report.scales.iter().all(|s| s.ssimulacra2.is_none()));
    }
    #[test]
    fn jittered_speckle_same_mean_is_texture_only() {
        let b = image::RgbaImage::from_fn(128, 128, |x, y| {
            let v = if (x + y) % 2 == 0 { 90 } else { 150 };
            image::Rgba([v, v, v, 255])
        });
        let c = image::RgbaImage::from_fn(128, 128, |x, y| {
            let v = if (x + y) % 2 == 0 { 150 } else { 90 };
            image::Rgba([v, v, v, 255])
        });
        let report = analyze_pair(&b, &c);
        assert_eq!(report.class, ChangeClass::TextureNoiseOnly);
        assert!(report.scales[0].mean_flip > 0.0);
        assert_eq!(report.scales[2].mad_rgb, 0.0);
        assert_eq!(report.relative_shifted_share, 0.0);
    }
    #[test]
    fn three_percent_brightness_patch_is_systematic_with_region() {
        let b = image::RgbaImage::from_pixel(128, 128, image::Rgba([160, 160, 160, 255]));
        let mut c = b.clone();
        for y in 32..96 {
            for x in 32..96 {
                c.put_pixel(x, y, image::Rgba([165, 165, 165, 255]));
            }
        }
        let report = analyze_pair(&b, &c);
        assert_eq!(report.class, ChangeClass::SystematicShift);
        assert!(report.regions.iter().any(|r| r.rect_px == [32, 32, 64, 64]));
        assert_eq!(report.relative_shifted_share, 0.25);
    }
    #[test]
    fn coarse_mottling_and_missing_silhouette_cannot_pass_as_texture() {
        let b = image::RgbaImage::from_fn(128, 128, |x, y| {
            let v = if (x + y) % 2 == 0 { 25 } else { 35 };
            image::Rgba([v, v, v, 255])
        });
        let c = image::RgbaImage::from_fn(128, 128, |x, y| {
            let v = if (x / 32 + y / 32) % 2 == 0 { 25 } else { 35 };
            image::Rgba([v, v, v, 255])
        });
        let report = analyze_pair(&b, &c);
        assert_eq!(report.class, ChangeClass::SystematicShift);
        assert!(report.detail_energy_ratio.unwrap() < 0.9);
        assert!(!report.regions.is_empty());
    }
    #[test]
    fn area_retains_partial_edge_and_preserves_constant_mean() {
        let v = vec![[0.5; 3]; 7 * 9];
        let (out, w, h) = area(&v, 7, 9, 4);
        assert_eq!((w, h), (1, 2));
        assert!(
            out.iter()
                .all(|v| v.iter().all(|x| (*x - 0.5).abs() < 1e-12))
        );
    }
}

#[cfg(test)]
mod coverage_tests {
    use super::*;
    #[test]
    fn generated_silhouette_expansion_is_not_noise() {
        let mut b = image::RgbaImage::from_pixel(128, 128, image::Rgba([0, 0, 0, 255]));
        let mut c = b.clone();
        for y in 16..112 {
            for x in 32..64 {
                b.put_pixel(x, y, image::Rgba([80, 120, 80, 255]));
            }
        }
        for y in 16..112 {
            for x in 32..80 {
                c.put_pixel(x, y, image::Rgba([80, 120, 80, 255]));
            }
        }
        let p = Policy {
            background: Some(Background::Luminance { max: 0.001 }),
            ..Default::default()
        };
        let gb = background(
            p.background.as_ref().unwrap(),
            &b,
            Path::new("b"),
            Path::new("."),
        )
        .unwrap();
        let gc = background(
            p.background.as_ref().unwrap(),
            &c,
            Path::new("c"),
            Path::new("."),
        )
        .unwrap();
        let opts = crate::compare::CompareOptions::default();
        let cmp = crate::compare::compare_rgba(&c, &b, &opts).unwrap();
        let r = analyze(
            &b,
            &c,
            &cmp.error_map,
            None,
            Some((&gb, &gc)),
            &p,
            &opts,
            ChangeClass::LocalStructure,
        )
        .unwrap();
        assert_eq!(r.class, ChangeClass::SystematicShift);
        assert_eq!(r.coverage_ratio, Some(1.5));
        assert!(r.silhouette_disagreement.unwrap() > 0.3);
    }
}
