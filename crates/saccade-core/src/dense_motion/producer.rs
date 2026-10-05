//! Independent native implementation of DIS inverse search and densification.
//! Kroeger et al., arXiv:1603.03590, algorithm 1 without variational refinement.
//! No upstream source copied. Numeric intensity scale is 0..255.
use super::*;
const PATCH: usize = 8;
const STRIDE: usize = 4;
const ITERATIONS: usize = 20;
#[derive(Clone)]
struct Plane {
    w: usize,
    h: usize,
    p: Vec<f32>,
}
impl Plane {
    fn image(image: &image::RgbaImage) -> Self {
        Self {
            w: image.width() as usize,
            h: image.height() as usize,
            p: image
                .pixels()
                .map(|p| 0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32)
                .collect(),
        }
    }
    fn sample(&self, x: f32, y: f32) -> f32 {
        let x = x.clamp(0.0, (self.w - 1) as f32);
        let y = y.clamp(0.0, (self.h - 1) as f32);
        let ix = x as usize;
        let iy = y as usize;
        let nx = (ix + 1).min(self.w - 1);
        let ny = (iy + 1).min(self.h - 1);
        let a = x - ix as f32;
        let b = y - iy as f32;
        (1.0 - b) * ((1.0 - a) * self.p[iy * self.w + ix] + a * self.p[iy * self.w + nx])
            + b * ((1.0 - a) * self.p[ny * self.w + ix] + a * self.p[ny * self.w + nx])
    }
    fn half(&self) -> Self {
        let w = self.w.div_ceil(2);
        let h = self.h.div_ceil(2);
        let mut p = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                p.push(
                    (self.sample((2 * x) as f32, (2 * y) as f32)
                        + self.sample((2 * x + 1) as f32, (2 * y) as f32)
                        + self.sample((2 * x) as f32, (2 * y + 1) as f32)
                        + self.sample((2 * x + 1) as f32, (2 * y + 1) as f32))
                        * 0.25,
                );
            }
        }
        Self { w, h, p }
    }
    fn gradient(&self, x: usize, y: usize) -> [f32; 2] {
        [
            (self.sample(x as f32 + 1.0, y as f32) - self.sample(x as f32 - 1.0, y as f32)) * 0.5,
            (self.sample(x as f32, y as f32 + 1.0) - self.sample(x as f32, y as f32 - 1.0)) * 0.5,
        ]
    }
}
struct Patch {
    x: usize,
    y: usize,
    t: [f32; 64],
    g: [[f32; 2]; 64],
    h: [f32; 3],
    mean: f32,
    textured: bool,
}
impl Patch {
    fn new(a: &Plane, x: usize, y: usize) -> Self {
        let mut p = Self {
            x,
            y,
            t: [0.0; 64],
            g: [[0.0; 2]; 64],
            h: [0.0; 3],
            mean: 0.0,
            textured: false,
        };
        let mut gm = [0.0; 2];
        for dy in 0..PATCH {
            for dx in 0..PATCH {
                let i = dy * PATCH + dx;
                p.t[i] = a.p[(y + dy) * a.w + x + dx];
                p.g[i] = a.gradient(x + dx, y + dy);
                p.mean += p.t[i] / 64.0;
                gm[0] += p.g[i][0] / 64.0;
                gm[1] += p.g[i][1] / 64.0;
            }
        }
        for i in 0..64 {
            p.t[i] -= p.mean;
            p.g[i][0] -= gm[0];
            p.g[i][1] -= gm[1];
            let [gx, gy] = p.g[i];
            p.h[0] += gx * gx;
            p.h[1] += gx * gy;
            p.h[2] += gy * gy;
        }
        let [a, b, c] = p.h;
        p.textured = (a + c - ((a - c).powi(2) + 4.0 * b * b).sqrt()) / 128.0 > 4.0;
        p
    }
    fn residual(&self, b: &Plane, u: [f32; 2]) -> Option<[f32; 64]> {
        if self.x as f32 + u[0] < 0.0
            || self.y as f32 + u[1] < 0.0
            || (self.x + PATCH - 1) as f32 + u[0] > (b.w - 1) as f32
            || (self.y + PATCH - 1) as f32 + u[1] > (b.h - 1) as f32
        {
            return None;
        }
        let mut r = [0.0; 64];
        let mut mean = 0.0;
        for dy in 0..PATCH {
            for dx in 0..PATCH {
                let i = dy * PATCH + dx;
                r[i] = b.sample((self.x + dx) as f32 + u[0], (self.y + dy) as f32 + u[1]);
                mean += r[i] / 64.0;
            }
        }
        for (i, r) in r.iter_mut().enumerate() {
            *r -= mean + self.t[i];
        }
        Some(r)
    }
    fn cost(&self, b: &Plane, u: [f32; 2]) -> f32 {
        self.residual(b, u).map_or(f32::INFINITY, |r| {
            r.iter().map(|r| r * r).sum::<f32>() / 64.0
        })
    }
    fn search(&self, b: &Plane, initial: [f32; 2], neighbor: Option<[f32; 2]>) -> [f32; 2] {
        if !self.textured {
            return initial;
        }
        let mut u = initial;
        let mut cost = self.cost(b, u);
        if let Some(n) = neighbor {
            let nc = self.cost(b, n);
            if nc < cost {
                u = n;
                cost = nc;
            }
        }
        // Bounded local initialization extends the gradient descent basin.
        for dy in -2..=2 {
            for dx in -2..=2 {
                let n = [initial[0] + dx as f32, initial[1] + dy as f32];
                let nc = self.cost(b, n);
                if nc < cost {
                    u = n;
                    cost = nc;
                }
            }
        }
        let [a, ab, c] = self.h;
        let det = a * c - ab * ab;
        if det <= 1e-6 {
            return initial;
        }
        for _ in 0..ITERATIONS {
            let Some(r) = self.residual(b, u) else {
                break;
            };
            let mut d = [0.0; 2];
            for (r, g) in r.iter().zip(self.g) {
                d[0] += r * g[0];
                d[1] += r * g[1];
            }
            let step = [
                ((c * d[0] - ab * d[1]) / det).clamp(-2.0, 2.0),
                ((a * d[1] - ab * d[0]) / det).clamp(-2.0, 2.0),
            ];
            let next = [u[0] - step[0], u[1] - step[1]];
            let nc = self.cost(b, next);
            if nc >= cost {
                break;
            }
            u = next;
            cost = nc;
            if step[0] * step[0] + step[1] * step[1] < 1e-5 {
                break;
            }
        }
        if norm(sub(u, initial)) > PATCH as f32 {
            initial
        } else {
            u
        }
    }
}
fn sub(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
fn norm(v: [f32; 2]) -> f32 {
    v[0].hypot(v[1])
}
fn positions(size: usize) -> Vec<usize> {
    let mut p: Vec<_> = (0..=size - PATCH).step_by(STRIDE).collect();
    if p.last() != Some(&(size - PATCH)) {
        p.push(size - PATCH);
    }
    p
}
fn sample_flow(flow: &[[f32; 2]], w: usize, h: usize, x: f32, y: f32) -> [f32; 2] {
    let x = x.clamp(0.0, (w - 1) as f32);
    let y = y.clamp(0.0, (h - 1) as f32);
    let ix = x as usize;
    let iy = y as usize;
    let nx = (ix + 1).min(w - 1);
    let ny = (iy + 1).min(h - 1);
    let a = x - ix as f32;
    let b = y - iy as f32;
    std::array::from_fn(|c| {
        (1.0 - b) * ((1.0 - a) * flow[iy * w + ix][c] + a * flow[iy * w + nx][c])
            + b * ((1.0 - a) * flow[ny * w + ix][c] + a * flow[ny * w + nx][c])
    })
}
fn solve(a: &Plane, b: &Plane) -> (Vec<[f32; 2]>, Vec<bool>) {
    let mut aa = vec![a.clone()];
    let mut bb = vec![b.clone()];
    while aa.len() < 4 && aa[aa.len() - 1].w >= 32 && aa[aa.len() - 1].h >= 32 {
        aa.push(aa[aa.len() - 1].half());
        bb.push(bb[bb.len() - 1].half());
    }
    let mut previous = Vec::new();
    let mut previous_dims = [0, 0];
    let mut textured = vec![false; a.w * a.h];
    for level in (0..aa.len()).rev() {
        let a = &aa[level];
        let b = &bb[level];
        let mut initial = vec![[0.0; 2]; a.w * a.h];
        if !previous.is_empty() {
            for y in 0..a.h {
                for x in 0..a.w {
                    let f = sample_flow(
                        &previous,
                        previous_dims[0],
                        previous_dims[1],
                        x as f32 * 0.5,
                        y as f32 * 0.5,
                    );
                    initial[y * a.w + x] = f.map(|v| v * 2.0);
                }
            }
        }
        let mut sum = vec![[0.0; 2]; a.w * a.h];
        let mut weights = vec![0.0; a.w * a.h];
        let xs = positions(a.w);
        let ys = positions(a.h);
        let mut patches = Vec::new();
        let mut flows = Vec::new();
        for &y in &ys {
            for &x in &xs {
                let p = Patch::new(a, x, y);
                let prior = initial[(y + PATCH / 2) * a.w + x + PATCH / 2];
                let neighbor = flows.last().copied();
                let flow = p.search(b, prior, neighbor);
                patches.push(p);
                flows.push(flow);
            }
        }
        for i in (0..patches.len()).rev() {
            let p = &patches[i];
            let u = p.search(b, flows[i], flows.get(i + 1).copied());
            flows[i] = u;
        }
        if level == 0 {
            textured.fill(false);
        }
        for (p, u) in patches.iter().zip(flows) {
            let Some(r) = p.residual(b, u) else {
                continue;
            };
            let cost = p.cost(b, u);
            let ambiguous = [[8.0, 0.0], [-8.0, 0.0], [0.0, 8.0], [0.0, -8.0]]
                .iter()
                .any(|d| p.cost(b, [u[0] + d[0], u[1] + d[1]]) <= cost + 1.0);
            for dy in 0..PATCH {
                for dx in 0..PATCH {
                    let i = (p.y + dy) * a.w + p.x + dx;
                    let weight = 1.0 / r[dy * PATCH + dx].abs().max(1.0);
                    sum[i][0] += weight * u[0];
                    sum[i][1] += weight * u[1];
                    weights[i] += weight;
                    if level == 0 && p.textured && !ambiguous && cost < 64.0 {
                        textured[i] = true;
                    }
                }
            }
        }
        for i in 0..sum.len() {
            sum[i] = if weights[i] > 0.0 {
                sum[i].map(|v| v / weights[i])
            } else {
                initial[i]
            };
        }
        previous = sum;
        previous_dims = [a.w, a.h];
    }
    (previous, textured)
}
fn qualify(
    a: &Plane,
    b: &Plane,
    flow: Vec<[f32; 2]>,
    reverse: &[[f32; 2]],
    texture: &[bool],
    reverse_texture: &[bool],
    direction: Direction,
) -> Field {
    let mut states = vec![State::Qualified; flow.len()];
    for y in 0..a.h {
        for x in 0..a.w {
            let i = y * a.w + x;
            let u = flow[i];
            let q = [x as f32 + u[0], y as f32 + u[1]];
            states[i] = if x < 4
                || y < 4
                || x + 4 >= a.w
                || y + 4 >= a.h
                || q[0] < 4.0
                || q[1] < 4.0
                || q[0] >= (b.w - 4) as f32
                || q[1] >= (b.h - 4) as f32
            {
                State::Boundary
            } else if !texture[i]
                || !reverse_texture[q[1].round() as usize * b.w + q[0].round() as usize]
            {
                State::AmbiguousTexture
            } else if norm(sub(
                u,
                sample_flow(reverse, b.w, b.h, q[0], q[1]).map(|v| -v),
            )) > 0.75
            {
                State::PossibleOcclusionOrMismatch
            } else if (a.p[i] - b.sample(q[0], q[1])).abs() > 20.0 {
                State::AppearanceUncertain
            } else if [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]
                .iter()
                .any(|&(x, y)| norm(sub(u, flow[y * a.w + x])) > 0.75)
            {
                State::MotionBoundary
            } else {
                State::Qualified
            };
        }
    }
    let qualified_pixels = states.iter().filter(|&&s| s == State::Qualified).count();
    Field {
        direction,
        vectors: flow,
        states,
        qualified_pixels,
    }
}
pub(super) fn review(
    reference: &image::RgbaImage,
    candidate: &image::RgbaImage,
    pins: [String; 2],
    supplied: Option<(&Sidecar, &Buffer, &str)>,
    ppd: f32,
    threshold: f32,
) -> Result<Report> {
    let dimensions = [reference.width(), reference.height()];
    if dimensions != [candidate.width(), candidate.height()]
        || dimensions.iter().any(|&d| !(16..=1024).contains(&d))
        || u64::from(dimensions[0]) * u64::from(dimensions[1]) > 1024 * 1024
        || !ppd.is_finite()
        || ppd <= 0.0
        || !threshold.is_finite()
        || !(0.0..=1.0).contains(&threshold)
        || pins.iter().any(|p| {
            p.len() != 64
                || !p
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
    {
        return Err(invalid(
            "dense motion requires equal 16..1024 pixel dimensions, valid identities, PPD and threshold",
        ));
    }
    if reference
        .pixels()
        .chain(candidate.pixels())
        .any(|p| p[3] != 255)
    {
        return Err(invalid(
            "dense motion v1 requires opaque SDR images; transparency is unqualified",
        ));
    }
    let normalized = if let Some((sidecar, buffer, digest)) = supplied {
        if sidecar.dimensions != dimensions {
            return Err(invalid(
                "motion sidecar resolution differs; dynamic resolution is unsupported",
            ));
        }
        Some(sidecar.normalize(buffer, &[pins[0].clone(), pins[1].clone(), digest.into()])?)
    } else {
        None
    };
    let a = Plane::image(reference);
    let b = Plane::image(candidate);
    let (f, ft) = solve(&a, &b);
    let (r, rt) = solve(&b, &a);
    let fields = [
        qualify(
            &a,
            &b,
            f.clone(),
            &r,
            &ft,
            &rt,
            Direction::ReferenceToCandidate,
        ),
        qualify(&b, &a, r, &f, &rt, &ft, Direction::CandidateToReference),
    ];
    let renderer = if let (Some((sidecar, buffer, _)), Some(normalized)) = (supplied, normalized) {
        let field = &fields[usize::from(sidecar.direction == Direction::CandidateToReference)];
        let sign = if sidecar.direction == Direction::ReferenceToCandidate {
            1.0
        } else {
            -1.0
        };
        let jitter: [f32; 2] = std::array::from_fn(|i| {
            sign * (sidecar.candidate_jitter_px[i] - sidecar.reference_jitter_px[i])
        });
        let mut endpoints = Vec::new();
        let mut angles = Vec::new();
        let mut compared = vec![false; field.vectors.len()];
        for (i, (&measured, &provided)) in field.vectors.iter().zip(&normalized).enumerate() {
            if field.states[i] != State::Qualified || !buffer.valid[i] {
                continue;
            }
            let measured = sub(measured, jitter);
            compared[i] = true;
            endpoints.push(norm(sub(measured, provided)) as f64);
            if norm(measured) > 0.1 && norm(provided) > 0.1 {
                let cosine = ((measured[0] * provided[0] + measured[1] * provided[1])
                    / (norm(measured) * norm(provided)))
                .clamp(-1.0, 1.0);
                angles.push(cosine.acos().to_degrees() as f64);
            }
        }
        endpoints.sort_by(f64::total_cmp);
        let count = endpoints.len();
        Some(Validation {
            sidecar: sidecar.clone(),
            declared_valid_pixels: buffer.valid.iter().filter(|&&v| v).count(),
            compared_pixels: count,
            coverage: count as f64 / compared.len() as f64,
            mean_endpoint_px: (!endpoints.is_empty())
                .then(|| endpoints.iter().sum::<f64>() / count as f64),
            p95_endpoint_px: (!endpoints.is_empty())
                .then(|| endpoints[(count as f64 * 0.95).ceil() as usize - 1]),
            maximum_endpoint_px: endpoints.last().copied(),
            mean_angle_degrees: (!angles.is_empty())
                .then(|| angles.iter().sum::<f64>() / angles.len() as f64),
            angular_pixels: angles.len(),
            compared,
        })
    } else {
        None
    };
    let comparison = crate::compare::compare_rgba(
        candidate,
        reference,
        &crate::compare::CompareOptions {
            pixels_per_degree: ppd,
            ..Default::default()
        },
    )?;
    Ok(Report {schema:"saccade-motion-review.v1".into(),image_sha256:pins,dimensions,
        method:serde_json::json!({"backend":"native_dis_inverse_search_v1","patch":PATCH,"stride":STRIDE,"maximum_pyramid_levels":4,"downscale":2,"inverse_iterations":ITERATIONS,"local_initialization_radius":2,"spatial_passes":2,"variational_refinement":false,"intensity":"encoded_srgb_bt709_luma_0_255","patch_mean_normalization":true,"fb_maximum_px":0.75,"appearance_maximum_intensity_error":20,"texture_minimum_eigenvalue":4,"patch_mse_maximum":64,"ambiguity_offsets_px":[[8,0],[-8,0],[0,8],[0,-8]]}),raw_regression:comparison.metrics.mean>f64::from(threshold),raw_flip:comparison.metrics,pixels_per_degree:ppd,maximum_raw_mean:threshold,fields,renderer,
        limitations:vec!["Unaligned raw FLIP and its declared threshold remain authoritative; no alignment changes acceptance.".into(),"Flow is apparent image correspondence, not geometric ground truth. Reflections, particles, shading, transparency and deformation need separate renderer evidence.".into(),"Forward/backward inconsistency means possible occlusion or mismatch, not confirmed visibility. Texture and residual checks are heuristic exclusions, not calibrated confidence.".into(),"Native DIS inverse search and residual-weighted densification omit variational refinement. No benchmark parity, throughput or large-motion guarantee; opaque SDR, equal dimensions and bounded inputs only.".into(),"Jitter, validity, frame interval and producer identity are declarations; dynamic-resolution resampling and packed GPU vector formats are unsupported.".into()]})
}
