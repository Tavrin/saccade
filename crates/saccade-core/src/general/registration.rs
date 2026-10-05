//! Pure Rust FAST/oriented BRIEF matching and deterministic RANSAC registration.
//! Transform direction is reference coordinates to capture coordinates, suitable for inverse warping.
use image::{GrayImage, RgbaImage};
use serde::{Deserialize, Serialize};

/// Versioned registration evidence.
pub const SCHEMA: &str = "saccade-registration.v1";
/// Geometric model; auto tries the least flexible successful model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Model {
    /// No registration.
    None,
    /// Translation only.
    Translation,
    /// Rotation, uniform scale and translation.
    Similarity,
    /// Affine transform.
    Affine,
    /// Projective transform.
    Homography,
    /// Least flexible model with sufficient consensus.
    Auto,
}
/// Explicit comparison scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scale {
    /// Keep the reference dimensions.
    Reference,
    /// Fit both images to the smaller reference-relative pixel count.
    Common,
}
/// Registration failure does not yield a guessed transform.
#[derive(Debug, thiserror::Error)]
pub enum RegistrationError {
    /// No sufficient reliable correspondence consensus.
    #[error("insufficient inliers: {matches} ratio-tested matches")]
    InsufficientInliers {
        /// Match count.
        matches: usize,
    },
    /// Invalid image geometry.
    #[error("empty image, singular transform, or excessive raster size")]
    InvalidGeometry,
}
/// Numerical evidence bound to a registration operation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Evidence {
    /// Actual fitted model.
    pub model: Model,
    /// Descriptor and detector identity.
    pub detector: String,
    /// Reference-to-capture homogeneous matrix, row major, in original input pixels.
    pub matrix: [f64; 9],
    /// Number of reciprocal ratio-tested matches.
    pub matches: usize,
    /// Consensus size.
    pub inliers: usize,
    /// Inliers divided by matches (zero for no registration).
    pub inlier_ratio: f64,
    /// RMS reprojection residual in reference pixels.
    pub residual_px: f64,
    /// Resample policy.
    pub scale: Scale,
    /// Original reference dimensions.
    pub reference_size: [u32; 2],
    /// Original capture dimensions.
    pub capture_size: [u32; 2],
    /// Comparison raster dimensions.
    pub output_size: [u32; 2],
    /// Pixels without capture coverage, explicitly excluded by geometry.
    pub excluded_by_geometry: u64,
}
/// Warped capture and explicitly visible geometric exclusions.
pub struct Registered {
    /// Resampled reference.
    pub reference: RgbaImage,
    /// Inverse-warped capture; border values are not comparison data.
    pub capture: RgbaImage,
    /// True means excluded by geometry.
    pub excluded: Vec<bool>,
    /// Model and geometry evidence.
    pub evidence: Evidence,
}
#[derive(Clone)]
struct Feature {
    x: f64,
    y: f64,
    descriptor: [u64; 4],
    score: i32,
}
const CIRCLE: [(i32, i32); 16] = [
    (0, -3),
    (1, -3),
    (2, -2),
    (3, -1),
    (3, 0),
    (3, 1),
    (2, 2),
    (1, 3),
    (0, 3),
    (-1, 3),
    (-2, 2),
    (-3, 1),
    (-3, 0),
    (-3, -1),
    (-2, -2),
    (-1, -3),
];
fn intensity(im: &GrayImage, x: i32, y: i32) -> i32 {
    i32::from(im.get_pixel(x as u32, y as u32)[0])
}
fn fast(im: &GrayImage, x: i32, y: i32) -> i32 {
    let centre = intensity(im, x, y);
    let d = CIRCLE.map(|(dx, dy)| intensity(im, x + dx, y + dy) - centre);
    let mut best = 0;
    for start in 0..16 {
        let mut bright = 255;
        let mut dark = 255;
        for k in 0..9 {
            let v = d[(start + k) % 16];
            bright = bright.min(v);
            dark = dark.min(-v);
        }
        best = best.max(bright).max(dark);
    }
    best
}
fn rng(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}
fn features(image: &RgbaImage) -> Vec<Feature> {
    let max_side = image.width().max(image.height()) as f64;
    let initial = (1024.0 / max_side).min(1.0);
    let gray = image::DynamicImage::ImageRgba8(image.clone()).to_luma8();
    let mut out = Vec::new();
    for level in 0..5 {
        let scale = initial * 2f64.powf(-f64::from(level) * 0.5);
        let w = (f64::from(image.width()) * scale).round() as u32;
        let h = (f64::from(image.height()) * scale).round() as u32;
        if w < 40 || h < 40 {
            continue;
        }
        let im = image::imageops::resize(&gray, w, h, image::imageops::FilterType::Triangle);
        let mut corners = Vec::new();
        for y in 16..h as i32 - 16 {
            for x in 16..w as i32 - 16 {
                let score = fast(&im, x, y);
                if score >= 20
                    && (-1..=1).all(|dy| {
                        (-1..=1)
                            .all(|dx| (dx == 0 && dy == 0) || fast(&im, x + dx, y + dy) <= score)
                    })
                {
                    corners.push((score, x, y));
                }
            }
        }
        corners.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.2.cmp(&b.2)).then(a.1.cmp(&b.1)));
        let mut selected: Vec<(i32, i32)> = Vec::new();
        for (score, x, y) in corners {
            if selected
                .iter()
                .any(|&(a, b)| (a - x).abs() < 7 && (b - y).abs() < 7)
            {
                continue;
            }
            selected.push((x, y));
            let mut mx = 0i64;
            let mut my = 0i64;
            for dy in -7..=7 {
                for dx in -7..=7 {
                    if dx * dx + dy * dy <= 49 {
                        let v = i64::from(intensity(&im, x + dx, y + dy));
                        mx += i64::from(dx) * v;
                        my += i64::from(dy) * v;
                    }
                }
            }
            let angle = (my as f64).atan2(mx as f64);
            let (s, c) = angle.sin_cos();
            let mut state = 0x5accade123u64;
            let mut descriptor = [0; 4];
            for bit in 0..256 {
                let mut point = || {
                    let a = (rng(&mut state) % 19) as i32 - 9;
                    let b = (rng(&mut state) % 19) as i32 - 9;
                    (
                        x + (f64::from(a) * c - f64::from(b) * s).round() as i32,
                        y + (f64::from(a) * s + f64::from(b) * c).round() as i32,
                    )
                };
                let a = point();
                let b = point();
                if intensity(&im, a.0, a.1) < intensity(&im, b.0, b.1) {
                    descriptor[bit / 64] |= 1u64 << (bit % 64);
                }
            }
            out.push(Feature {
                x: f64::from(x) / scale,
                y: f64::from(y) / scale,
                descriptor,
                score,
            });
            if selected.len() >= 240 {
                break;
            }
        }
    }
    out.sort_unstable_by_key(|a| std::cmp::Reverse(a.score));
    out.truncate(1200);
    out
}
fn distance(a: &Feature, b: &Feature) -> u32 {
    a.descriptor
        .iter()
        .zip(b.descriptor)
        .map(|(x, y)| (x ^ y).count_ones())
        .sum()
}
fn nearest(a: &Feature, bs: &[Feature]) -> Option<usize> {
    let mut first = (u32::MAX, 0);
    let mut second = u32::MAX;
    for (i, b) in bs.iter().enumerate() {
        let d = distance(a, b);
        if d < first.0 {
            second = first.0;
            first = (d, i);
        } else if d < second {
            second = d;
        }
    }
    (first.0 <= 80 && first.0 * 5 < second.saturating_mul(4) && second != u32::MAX)
        .then_some(first.1)
}
#[derive(Clone, Copy)]
struct Match {
    a: [f64; 2],
    b: [f64; 2],
}
fn correspondences(a: &[Feature], b: &[Feature]) -> Vec<Match> {
    let reverse: Vec<_> = b.iter().map(|f| nearest(f, a)).collect();
    a.iter()
        .enumerate()
        .filter_map(|(i, f)| {
            let j = nearest(f, b)?;
            (reverse[j] == Some(i)).then_some(Match {
                a: [f.x, f.y],
                b: [b[j].x, b[j].y],
            })
        })
        .collect()
}
fn solve(mut rows: Vec<Vec<f64>>, mut rhs: Vec<f64>) -> Option<Vec<f64>> {
    let n = rhs.len();
    for i in 0..n {
        let pivot = (i..n).max_by(|&a, &b| rows[a][i].abs().total_cmp(&rows[b][i].abs()))?;
        if rows[pivot][i].abs() < 1e-10 {
            return None;
        }
        rows.swap(i, pivot);
        rhs.swap(i, pivot);
        let div = rows[i][i];
        for value in rows[i].iter_mut().skip(i) {
            *value /= div;
        }
        rhs[i] /= div;
        let pivot_row = rows[i].clone();
        for j in 0..n {
            if j == i {
                continue;
            }
            let f = rows[j][i];
            for (value, pivot) in rows[j].iter_mut().zip(&pivot_row).skip(i) {
                *value -= f * pivot;
            }
            rhs[j] -= f * rhs[i];
        }
    }
    rhs.iter().all(|v| v.is_finite()).then_some(rhs)
}
fn project(m: &[f64; 9], p: [f64; 2]) -> Option<[f64; 2]> {
    let z = m[6] * p[0] + m[7] * p[1] + m[8];
    if z.abs() < 1e-8 {
        return None;
    }
    let out = [
        (m[0] * p[0] + m[1] * p[1] + m[2]) / z,
        (m[3] * p[0] + m[4] * p[1] + m[5]) / z,
    ];
    out.iter().all(|v| v.is_finite()).then_some(out)
}
fn fit(matches: &[Match], model: Model) -> Option<[f64; 9]> {
    let n = match model {
        Model::Translation => 2,
        Model::Similarity => 4,
        Model::Affine => 6,
        Model::Homography => 8,
        _ => return None,
    };
    // Coordinate normalization keeps homography normal equations well conditioned.
    let scale = matches
        .iter()
        .flat_map(|m| m.a.into_iter().chain(m.b))
        .map(f64::abs)
        .fold(1.0, f64::max);
    let mut normal = vec![vec![0.0; n]; n];
    let mut rhs = vec![0.0; n];
    let mut add = |r: Vec<f64>, v: f64| {
        for i in 0..n {
            rhs[i] += r[i] * v;
            for j in 0..n {
                normal[i][j] += r[i] * r[j];
            }
        }
    };
    for m in matches {
        let [x, y] = m.a.map(|v| v / scale);
        let [u, v] = m.b.map(|v| v / scale);
        match model {
            Model::Translation => {
                add(vec![1., 0.], u - x);
                add(vec![0., 1.], v - y);
            }
            Model::Similarity => {
                add(vec![x, -y, 1., 0.], u);
                add(vec![y, x, 0., 1.], v);
            }
            Model::Affine => {
                add(vec![x, y, 1., 0., 0., 0.], u);
                add(vec![0., 0., 0., x, y, 1.], v);
            }
            Model::Homography => {
                add(vec![x, y, 1., 0., 0., 0., -u * x, -u * y], u);
                add(vec![0., 0., 0., x, y, 1., -v * x, -v * y], v);
            }
            _ => return None,
        }
    }
    let p = solve(normal, rhs)?;
    let m = match model {
        Model::Translation => [1., 0., p[0] * scale, 0., 1., p[1] * scale, 0., 0., 1.],
        Model::Similarity => [
            p[0],
            -p[1],
            p[2] * scale,
            p[1],
            p[0],
            p[3] * scale,
            0.,
            0.,
            1.,
        ],
        Model::Affine => [
            p[0],
            p[1],
            p[2] * scale,
            p[3],
            p[4],
            p[5] * scale,
            0.,
            0.,
            1.,
        ],
        Model::Homography => [
            p[0],
            p[1],
            p[2] * scale,
            p[3],
            p[4],
            p[5] * scale,
            p[6] / scale,
            p[7] / scale,
            1.,
        ],
        _ => return None,
    };
    let det = m[0] * (m[4] * m[8] - m[5] * m[7]) - m[1] * (m[3] * m[8] - m[5] * m[6])
        + m[2] * (m[3] * m[7] - m[4] * m[6]);
    (det.abs() > 1e-8).then_some(m)
}
fn error(m: &[f64; 9], pair: Match) -> f64 {
    project(m, pair.a).map_or(f64::INFINITY, |p| {
        (p[0] - pair.b[0]).hypot(p[1] - pair.b[1])
    })
}
fn estimate(pairs: &[Match], model: Model, tolerance: f64) -> Option<([f64; 9], Vec<Match>, f64)> {
    let sample = match model {
        Model::Translation => 1,
        Model::Similarity => 2,
        Model::Affine => 3,
        Model::Homography => 4,
        _ => return None,
    };
    if pairs.len() < 6.max(sample * 2) {
        return None;
    }
    let mut state = 0x6accadeu64;
    let mut best = Vec::new();
    let mut best_cost = f64::INFINITY;
    for _ in 0..768 {
        let mut ids = Vec::new();
        while ids.len() < sample {
            let i = (rng(&mut state) % pairs.len() as u64) as usize;
            if !ids.contains(&i) {
                ids.push(i);
            }
        }
        let chosen: Vec<_> = ids.iter().map(|&i| pairs[i]).collect();
        let Some(m) = fit(&chosen, model) else {
            continue;
        };
        let inliers: Vec<_> = pairs
            .iter()
            .copied()
            .filter(|&p| error(&m, p) <= tolerance)
            .collect();
        let cost = inliers.iter().map(|&p| error(&m, p).powi(2)).sum::<f64>();
        if inliers.len() > best.len() || (inliers.len() == best.len() && cost < best_cost) {
            best = inliers;
            best_cost = cost;
        }
    }
    if best.len() < 6.max(sample * 2) || best.len() * 4 < pairs.len() {
        return None;
    }
    let m = fit(&best, model)?;
    let best: Vec<_> = pairs
        .iter()
        .copied()
        .filter(|&p| error(&m, p) <= tolerance)
        .collect();
    if best.len() < 6.max(sample * 2) || best.len() * 4 < pairs.len() {
        return None;
    }
    let m = fit(&best, model)?;
    let rms = (best.iter().map(|&p| error(&m, p).powi(2)).sum::<f64>() / best.len() as f64).sqrt();
    (rms <= tolerance).then_some((m, best, rms))
}
/// Registers capture into reference coordinates. Bounded at 16M pixels and 1200 features/image.
/// Six inliers (eight for homography), at least 25% consensus, and <=3px RMS are required.
/// Auto chooses the least flexible successful model; no failed fit becomes identity.
pub fn register(
    reference: &RgbaImage,
    capture: &RgbaImage,
    model: Model,
    scale: Scale,
) -> Result<Registered, RegistrationError> {
    let valid = |i: &RgbaImage| {
        i.width() > 0
            && i.height() > 0
            && u64::from(i.width()) * u64::from(i.height()) <= super::input::MAX_PIXELS
    };
    if !valid(reference) || !valid(capture) {
        return Err(RegistrationError::InvalidGeometry);
    }
    let (matrix, matches, inliers, residual, actual) = if model == Model::None {
        let sx = f64::from(capture.width()) / f64::from(reference.width());
        let sy = f64::from(capture.height()) / f64::from(reference.height());
        (
            [sx, 0., (sx - 1.) * 0.5, 0., sy, (sy - 1.) * 0.5, 0., 0., 1.],
            0,
            0,
            0.,
            model,
        )
    } else {
        let pairs = correspondences(&features(reference), &features(capture));
        let candidates = if model == Model::Auto {
            vec![
                Model::Translation,
                Model::Similarity,
                Model::Affine,
                Model::Homography,
            ]
        } else {
            vec![model]
        };
        let capture_to_ref = f64::from(reference.width().max(reference.height()))
            / f64::from(capture.width().max(capture.height()));
        let tolerance = 3.0 / capture_to_ref;
        let fitted = candidates.into_iter().find_map(|m| {
            estimate(&pairs, m, tolerance)
                .map(|(mat, ins, rms)| (mat, pairs.len(), ins.len(), rms * capture_to_ref, m))
        });
        fitted.ok_or(RegistrationError::InsufficientInliers {
            matches: pairs.len(),
        })?
    };
    let factor = match scale {
        Scale::Reference => 1.,
        Scale::Common => (f64::from(capture.width()) * f64::from(capture.height())
            / (f64::from(reference.width()) * f64::from(reference.height())))
        .sqrt()
        .min(1.),
    };
    let w = (f64::from(reference.width()) * factor).round().max(1.) as u32;
    let h = (f64::from(reference.height()) * factor).round().max(1.) as u32;
    let base = image::imageops::resize(reference, w, h, image::imageops::FilterType::Triangle);
    let mut warped = RgbaImage::new(w, h);
    let mut excluded = vec![true; w as usize * h as usize];
    for (x, y, pixel) in warped.enumerate_pixels_mut() {
        let q = [
            (f64::from(x) + 0.5) * f64::from(reference.width()) / f64::from(w) - 0.5,
            (f64::from(y) + 0.5) * f64::from(reference.height()) / f64::from(h) - 0.5,
        ];
        let Some([sx, sy]) = project(&matrix, q) else {
            continue;
        };
        if sx < -0.5
            || sy < -0.5
            || sx >= f64::from(capture.width()) - 0.5
            || sy >= f64::from(capture.height()) - 0.5
        {
            continue;
        }
        let sx = sx.clamp(0., f64::from(capture.width() - 1));
        let sy = sy.clamp(0., f64::from(capture.height() - 1));
        let (x0, y0) = (sx.floor() as u32, sy.floor() as u32);
        let x1 = (x0 + 1).min(capture.width() - 1);
        let y1 = (y0 + 1).min(capture.height() - 1);
        let fx = sx - f64::from(x0);
        let fy = sy - f64::from(y0);
        // Interpolate premultiplied alpha, then return straight alpha.
        let samples = [
            (x0, y0, (1. - fx) * (1. - fy)),
            (x1, y0, fx * (1. - fy)),
            (x0, y1, (1. - fx) * fy),
            (x1, y1, fx * fy),
        ];
        let alpha = samples
            .iter()
            .map(|&(a, b, t)| f64::from(capture.get_pixel(a, b)[3]) * t)
            .sum::<f64>();
        pixel[3] = alpha.round() as u8;
        if alpha > 0. {
            for c in 0..3 {
                pixel[c] = (samples
                    .iter()
                    .map(|&(a, b, t)| {
                        let p = capture.get_pixel(a, b);
                        f64::from(p[c]) * f64::from(p[3]) * t
                    })
                    .sum::<f64>()
                    / alpha)
                    .round() as u8;
            }
        }
        excluded[y as usize * w as usize + x as usize] = false;
    }
    let count = excluded.iter().filter(|&&v| v).count() as u64;
    if count == u64::from(w) * u64::from(h) {
        return Err(RegistrationError::InvalidGeometry);
    }
    Ok(Registered {
        reference: base,
        capture: warped,
        excluded,
        evidence: Evidence {
            model: actual,
            detector: if model == Model::None {
                "none; explicit resampling".into()
            } else {
                "FAST-9/oriented-BRIEF-256; pyramid-v1; reciprocal-ratio-0.8; RANSAC-seed-0x6accade"
                    .into()
            },
            matrix,
            matches,
            inliers,
            inlier_ratio: if matches > 0 {
                inliers as f64 / matches as f64
            } else {
                0.
            },
            residual_px: residual,
            scale,
            reference_size: [reference.width(), reference.height()],
            capture_size: [capture.width(), capture.height()],
            output_size: [w, h],
            excluded_by_geometry: count,
        },
    })
}
/// A translation-clustered copy-move candidate, not proof of manipulation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CopyMove {
    /// First duplicated-region bounding box in image pixels.
    pub first: [u32; 4],
    /// Second duplicated-region bounding box in image pixels.
    pub second: [u32; 4],
    /// Reciprocal descriptor matches supporting the displacement cluster.
    pub matches: usize,
    /// Average displacement between matched features.
    pub displacement: [f64; 2],
    /// RMS displacement residual in pixels.
    pub residual_px: f64,
}
/// Self-matches the bounded detector with reciprocal ratio testing, excluding features within 24px.
/// Groups by 12px translation bins, requires four matches, and returns at most 32 candidates.
/// Repeated natural structures can produce candidates; rotations/projective copies may be missed.
pub fn copy_move(image: &RgbaImage) -> Result<Vec<CopyMove>, RegistrationError> {
    if image.width() == 0
        || image.height() == 0
        || u64::from(image.width()) * u64::from(image.height()) > super::input::MAX_PIXELS
    {
        return Err(RegistrationError::InvalidGeometry);
    }
    let features = features(image);
    let nearest: Vec<_> = features
        .iter()
        .enumerate()
        .map(|(id, a)| {
            let mut first = (u32::MAX, 0);
            let mut second = u32::MAX;
            for (j, b) in features.iter().enumerate() {
                if id == j || (a.x - b.x).hypot(a.y - b.y) < 24. {
                    continue;
                }
                let d = distance(a, b);
                if d < first.0 {
                    second = first.0;
                    first = (d, j);
                } else if d < second {
                    second = d;
                }
            }
            (first.0 <= 80 && first.0 * 5 < second.saturating_mul(4) && second != u32::MAX)
                .then_some(first.1)
        })
        .collect();
    type Pair = ([f64; 2], [f64; 2]);
    let mut clusters: std::collections::BTreeMap<(i32, i32), Vec<Pair>> =
        std::collections::BTreeMap::new();
    for (i, other) in nearest.iter().enumerate() {
        if let Some(j) = *other {
            if j <= i || nearest[j] != Some(i) {
                continue;
            }
            let a = &features[i];
            let b = &features[j];
            let (mut a, mut b) = ([a.x, a.y], [b.x, b.y]);
            if a[0] > b[0] || (a[0] == b[0] && a[1] > b[1]) {
                std::mem::swap(&mut a, &mut b);
            }
            let key = (
                ((b[0] - a[0]) / 12.).round() as i32,
                ((b[1] - a[1]) / 12.).round() as i32,
            );
            clusters.entry(key).or_default().push((a, b));
        }
    }
    let bounds = |points: Vec<[f64; 2]>| {
        let x = points.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min) - 16.;
        let y = points.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min) - 16.;
        let x1 = (points.iter().map(|p| p[0]).fold(0., f64::max) + 16.)
            .ceil()
            .min(f64::from(image.width()));
        let y1 = (points.iter().map(|p| p[1]).fold(0., f64::max) + 16.)
            .ceil()
            .min(f64::from(image.height()));
        let (x, y) = (x.floor().max(0.) as u32, y.floor().max(0.) as u32);
        [x, y, x1 as u32 - x, y1 as u32 - y]
    };
    let mut candidates = Vec::new();
    for pairs in clusters.into_values() {
        if pairs.len() < 4 {
            continue;
        }
        let n = pairs.len() as f64;
        let dx = pairs.iter().map(|(a, b)| b[0] - a[0]).sum::<f64>() / n;
        let dy = pairs.iter().map(|(a, b)| b[1] - a[1]).sum::<f64>() / n;
        let residual = (pairs
            .iter()
            .map(|(a, b)| (b[0] - a[0] - dx).powi(2) + (b[1] - a[1] - dy).powi(2))
            .sum::<f64>()
            / n)
            .sqrt();
        candidates.push(CopyMove {
            first: bounds(pairs.iter().map(|(a, _)| *a).collect()),
            second: bounds(pairs.iter().map(|(_, b)| *b).collect()),
            matches: pairs.len(),
            displacement: [dx, dy],
            residual_px: residual,
        });
    }
    candidates.sort_by_key(|c| (std::cmp::Reverse(c.matches), c.first, c.second));
    candidates.truncate(32);
    Ok(candidates)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn models_recover_known_correspondences_with_outliers() {
        for model in [Model::Similarity, Model::Affine, Model::Homography] {
            let m = match model {
                Model::Similarity => [0.8, -0.2, 31., 0.2, 0.8, -17., 0., 0., 1.],
                Model::Affine => [0.9, 0.2, 31., -0.1, 1.1, -17., 0., 0., 1.],
                _ => [0.9, 0.2, 31., -0.1, 1.1, 17., 0.0004, -0.0003, 1.],
            };
            let mut pairs = Vec::new();
            for y in 0..5 {
                for x in 0..6 {
                    let a = [f64::from(x) * 40. + 20., f64::from(y) * 40. + 20.];
                    pairs.push(Match {
                        a,
                        b: project(&m, a).expect("projection"),
                    });
                }
            }
            for i in 0..10 {
                pairs.push(Match {
                    a: [i as f64 * 7., 8.],
                    b: [999., i as f64 * 10.],
                });
            }
            let (got, ins, rms) = estimate(&pairs, model, 3.).expect("fit");
            assert_eq!(ins.len(), 30);
            assert!(rms < 1e-6);
            assert!(error(&got, pairs[7]) < 1e-6);
        }
    }
    #[test]
    fn blank_images_refuse_alignment() {
        let a = RgbaImage::from_pixel(64, 64, image::Rgba([100, 100, 100, 255]));
        assert!(matches!(
            register(&a, &a, Model::Auto, Scale::Reference),
            Err(RegistrationError::InsufficientInliers { .. })
        ));
    }
    #[test]
    fn geometry_border_and_common_scale_are_explicit() {
        let a = RgbaImage::from_pixel(40, 30, image::Rgba([20, 20, 20, 255]));
        let b = RgbaImage::from_pixel(20, 15, image::Rgba([20, 20, 20, 255]));
        let r = register(&a, &b, Model::None, Scale::Reference).expect("warp");
        assert_eq!(r.evidence.excluded_by_geometry, 0);
        let r = register(&a, &b, Model::None, Scale::Common).expect("warp");
        assert_eq!(r.evidence.output_size, [20, 15]);
        assert_eq!(r.evidence.excluded_by_geometry, 0);
    }
    fn fixture() -> RgbaImage {
        let mut state = 123456u64;
        let mut out = RgbaImage::from_pixel(256, 256, image::Rgba([30, 30, 30, 255]));
        for _ in 0..110 {
            let x = (rng(&mut state) % 220 + 12) as u32;
            let y = (rng(&mut state) % 220 + 12) as u32;
            let v = (rng(&mut state) % 160 + 80) as u8;
            for dy in 0..9 {
                for dx in 0..9 {
                    out.put_pixel(x + dx, y + dy, image::Rgba([v, v, v, 255]));
                }
            }
        }
        out
    }
    #[test]
    fn detector_recovers_translation() {
        let a = fixture();
        let mut b = RgbaImage::from_pixel(256, 256, image::Rgba([30, 30, 30, 255]));
        for y in 0..244 {
            for x in 0..239 {
                b.put_pixel(x + 17, y + 12, *a.get_pixel(x, y));
            }
        }
        let r = register(&a, &b, Model::Translation, Scale::Reference).expect("registration");
        assert!((r.evidence.matrix[2] - 17.).abs() < 1.);
        assert!((r.evidence.matrix[5] - 12.).abs() < 1.);
        assert!(r.evidence.excluded_by_geometry > 0);
    }
    #[test]
    fn self_matches_find_translated_copy_candidates() {
        let mut state = 0x12345678u64;
        let mut image = RgbaImage::from_pixel(256, 128, image::Rgba([30, 30, 30, 255]));
        for y in 12..112 {
            for x in 12..112 {
                let v = (rng(&mut state) % 256) as u8;
                let p = image::Rgba([v, v, v, 255]);
                image.put_pixel(x, y, p);
                image.put_pixel(x + 128, y, p);
            }
        }
        let candidates = copy_move(&image).expect("copy candidates");
        assert!(candidates.iter().any(|c| c.matches >= 4
            && (c.displacement[0] - 128.).abs() < 3.
            && c.displacement[1].abs() < 3.));
    }
    #[test]
    #[ignore = "heavy: registration"]
    fn generated_rotation_scale_and_perspective() {
        let a = fixture();
        for (model, m) in [
            (
                Model::Similarity,
                [0.94, -0.12, 20., 0.12, 0.94, 0., 0., 0., 1.],
            ),
            (Model::Affine, [0.9, 0.08, 10., -0.06, 1., 12., 0., 0., 1.]),
            (
                Model::Homography,
                [0.95, 0.03, 8., 0.01, 1., 6., 0.0003, 0.0001, 1.],
            ),
        ] {
            let mut b = RgbaImage::from_pixel(256, 256, image::Rgba([30, 30, 30, 255]));
            for (y, row) in a.rows().enumerate() {
                for (x, p) in row.enumerate() {
                    if let Some([u, v]) = project(&m, [x as f64, y as f64])
                        && (0.0..256.).contains(&u)
                        && (0.0..256.).contains(&v)
                    {
                        b.put_pixel(u.round().min(255.) as u32, v.round().min(255.) as u32, *p);
                    }
                }
            }
            let r = register(&a, &b, model, Scale::Reference).expect("synthetic registration");
            assert!(r.evidence.inliers >= 8);
            for p in [[70., 70.], [170., 160.]] {
                let expected = project(&m, p).expect("expected");
                let got = project(&r.evidence.matrix, p).expect("actual");
                assert!((got[0] - expected[0]).hypot(got[1] - expected[1]) < 3.);
            }
        }
    }
}
