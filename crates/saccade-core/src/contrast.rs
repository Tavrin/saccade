//! Reusable linear-luminance and robust foreground/background measurements.
use crate::{Error, Result, color};
use image::RgbImage;
use std::collections::BTreeMap;
/// One supported component/core colour measurement.
#[derive(Debug, Clone)]
pub struct LocalPair {
    /// Component index, starting at one.
    pub component: usize,
    /// Segment pixel index within the resolved rectangle.
    pub segment: usize,
    /// Supported core pixel count.
    pub support: usize,
    /// Actual foreground pixel swatch.
    pub foreground: [u8; 3],
    /// Adjacent background ring swatch.
    pub background: [u8; 3],
    /// Relative luminance ratio.
    pub ratio: f64,
    /// Thin core: ratio is only a lower bound.
    pub lower_bound: bool,
}
/// Worst supported local ink/background pair; unsupported evidence stays absent.
#[derive(Debug, Clone)]
pub struct Estimate {
    /// Worst supported stroke-core sRGB swatch.
    pub foreground: Option<[u8; 3]>,
    /// Local ring background sRGB swatch.
    pub background: Option<[u8; 3]>,
    /// Relative luminance ratio, absent for unsupported pixels.
    pub ratio: Option<f64>,
    /// Method and abstention reason.
    pub note: String,
    /// Ratio is a conservative thin-stroke lower bound, never sufficient for FAIL.
    pub lower_bound: bool,
    /// All supported core colour pairs, including independent point evidence.
    pub pairs: Vec<LocalPair>,
    /// Every component has a supported near-uniform background ring.
    pub background_uniform: bool,
}
impl Estimate {
    /// Lowest supported point measurement, without any contrast policy.
    /// An unsupported background prevents a region-wide point conclusion.
    pub fn lowest_point(&self) -> Option<&LocalPair> {
        if !self.background_uniform {
            return None;
        }
        self.pairs
            .iter()
            .filter(|p| !p.lower_bound)
            .min_by(|a, b| a.ratio.total_cmp(&b.ratio))
    }
}
/// Relative luminance ratio of two linear RGB colours; no policy threshold.
pub fn contrast_ratio(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (a, b) = (color::luminance(a), color::luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn distance(a: [u8; 3], b: [u8; 3]) -> u8 {
    (0..3).map(|c| a[c].abs_diff(b[c])).max().unwrap_or(0)
}
fn mode(pixels: impl Iterator<Item = [u8; 3]>) -> Option<[u8; 3]> {
    let mut counts = BTreeMap::new();
    for p in pixels {
        *counts.entry(p).or_insert(0usize) += 1;
    }
    counts.into_iter().max_by_key(|(_, n)| *n).map(|(p, _)| p)
}
/// Component-local stroke-core estimator in encoded sRGB. Every supported core
/// colour is checked; the worst component/colour wins. Thin cores are lower bounds.
pub fn estimate(image: &RgbImage, rect: [f64; 4]) -> Result<Estimate> {
    estimate_components(image, rect, false, false)
}
/// Check supported UI boundary segments separately against adjacent ring pixels.
/// A segment occupies an 8x8 tile and requires two core pixels of one colour.
pub fn estimate_boundary(image: &RgbImage, rect: [f64; 4]) -> Result<Estimate> {
    estimate_components(image, rect, true, false)
}
/// Contrast of displayed stroke cores, independent of source-colour plateau support.
/// Mixed channel directions are measurable here because actual pixels are the quantity.
pub fn estimate_rendered(image: &RgbImage, rect: [f64; 4]) -> Result<Estimate> {
    estimate_components(image, rect, false, true)
}
fn estimate_components(
    image: &RgbImage,
    rect: [f64; 4],
    boundary: bool,
    rendered: bool,
) -> Result<Estimate> {
    let [x, y, w, h] = crate::regions::resolve_rect(rect, image.width(), image.height())
        .ok_or_else(|| Error::Config("contrast region resolves to zero pixels".into()))?;
    if u64::from(w) * u64::from(h) > crate::general::input::MAX_PIXELS {
        return Err(Error::Config("contrast region exceeds pixel budget".into()));
    }
    let mut result = Estimate { foreground: None, background: None, ratio: None,
        pairs: Vec::new(), background_uniform: true, lower_bound: false, note: "Component-local encoded-sRGB ink, guard-band ring background, supported stroke-core colours; worst component wins.".into() };
    let pixels: Vec<_> = (y..y + h)
        .flat_map(|yy| (x..x + w).map(move |xx| image.get_pixel(xx, yy).0))
        .collect();
    let neighbours = |i: usize, radius: i32| {
        let (xx, yy) = (i as i32 % w as i32, i as i32 / w as i32);
        (-radius..=radius).flat_map(move |dy| {
            (-radius..=radius).filter_map(move |dx| {
                let (a, b) = (xx + dx, yy + dy);
                (a >= 0 && b >= 0 && a < w as i32 && b < h as i32)
                    .then_some((b * w as i32 + a) as usize)
            })
        })
    };
    // Spatial mode seeds only ink selection, never the reported background.
    let bits: Vec<_> = (0..pixels.len())
        .map(|i| {
            let bg = mode(
                neighbours(i, 10)
                    .filter(|j| {
                        let dx = (*j as i32 % w as i32 - i as i32 % w as i32).abs();
                        let dy = (*j as i32 / w as i32 - i as i32 / w as i32).abs();
                        dx % 2 == 0 && dy % 2 == 0
                    })
                    .map(|j| pixels[j]),
            )
            .unwrap_or(pixels[i]);
            distance(pixels[i], bg) >= 1
        })
        .collect();
    let mut seen = vec![false; pixels.len()];
    let mut count = 0;
    let mut unknown = false;
    let mut worst = f64::INFINITY;
    let mut worst_lower = false;
    let mut mixed_thin = false;
    for seed in 0..pixels.len() {
        if !bits[seed] || seen[seed] {
            continue;
        }
        count += 1;
        if count > 8192 {
            result.ratio = None;
            result.note.push_str(" Component budget exceeded.");
            return Ok(result);
        }
        let mut stack = vec![seed];
        seen[seed] = true;
        let mut component = Vec::new();
        while let Some(i) = stack.pop() {
            component.push(i);
            for j in neighbours(i, 1) {
                if bits[j] && !seen[j] {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        let segments = if boundary {
            let mut tiles: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
            for i in component {
                tiles
                    .entry((i % w as usize / 8, i / w as usize / 8))
                    .or_default()
                    .push(i);
            }
            tiles.into_values().collect::<Vec<_>>()
        } else {
            vec![component]
        };
        for component in segments {
            // Two pixels are meaningful support, including narrow strokes and dots.
            if component.len() < 2
                || (rendered
                    && (component.len() < 3
                        || component.iter().map(|i| i / w as usize).max().unwrap_or(0)
                            - component.iter().map(|i| i / w as usize).min().unwrap_or(0)
                            < 2))
            {
                continue;
            }
            let mut ring = BTreeMap::new();
            for &i in &component {
                for j in neighbours(i, 3) {
                    if !bits[j] && !neighbours(j, 1).any(|k| bits[k]) {
                        ring.insert(j, pixels[j]);
                    }
                }
            }
            let Some(bg) = mode(ring.values().copied()) else {
                result.background_uniform = false;
                unknown = true;
                continue;
            };
            if ring.len() < 4 || ring.values().any(|p| distance(*p, bg) > 6) {
                result.background_uniform = false;
                unknown = true;
                continue;
            }
            let mut cores: BTreeMap<[u8; 3], (usize, bool)> = BTreeMap::new();
            for &i in &component {
                let d = distance(pixels[i], bg);
                if d < 1 {
                    continue;
                }
                if neighbours(i, 1).any(|j| bits[j] && distance(pixels[j], bg) > d) {
                    continue;
                }
                let colour_plateau = (boundary
                    && neighbours(i, 1).filter(|j| pixels[*j] == pixels[i]).count() >= 3)
                    || [-1i32, 0].into_iter().any(|dy| {
                        [-1i32, 0].into_iter().any(|dx| {
                            let (xx, yy) = (i as i32 % w as i32 + dx, i as i32 / w as i32 + dy);
                            xx >= 0
                                && yy >= 0
                                && xx + 1 < w as i32
                                && yy + 1 < h as i32
                                && [0, 1].into_iter().all(|b| {
                                    [0, 1].into_iter().all(|a| {
                                        pixels[((yy + b) * w as i32 + xx + a) as usize] == pixels[i]
                                    })
                                })
                        })
                    });
                // A source-colour point needs a full 3x3 constant core. A
                // 2x2 antialias shelf is not evidence of the original ink.
                let plateau = colour_plateau
                    && (boundary
                        || neighbours(i, 1).count() == 9
                            && neighbours(i, 1).all(|j| pixels[j] == pixels[i]))
                    && d > 6;
                let entry = cores.entry(pixels[i]).or_default();
                entry.0 += 1;
                entry.1 |= plateau;
            }
            // Supported extreme: 90th percentile of local stroke maxima,
            // with at least two pixels in the upper tail. Ignore isolated peaks;
            // retain distinct supported plateaus to catch mixed source inks.
            let mut tail: Vec<_> = cores
                .iter()
                .flat_map(|(p, (n, _))| std::iter::repeat_n(*p, *n))
                .collect();
            tail.sort_by_key(|p| distance(*p, bg));
            let peak =
                (tail.len() >= 2).then(|| tail[((tail.len() - 1) * 9 / 10).min(tail.len() - 2)]);
            let max_distance = peak.map_or(0, |p| distance(p, bg));
            let mut supported = false;
            let segment = component[0];
            for (fg, (n, plateau)) in cores {
                let plateau = plateau && n >= 2;
                if !plateau && Some(fg) != peak {
                    continue;
                }
                // Encoded interpolation with opposite channel directions can
                // cross a luminance minimum: sampled contrast can exceed ink
                // contrast. Such a thin sample is not a valid lower bound.
                if !rendered
                    && !plateau
                    && (0..3).any(|c| fg[c] < bg[c])
                    && (0..3).any(|c| fg[c] > bg[c])
                {
                    unknown = true;
                    mixed_thin = true;
                    continue;
                }
                supported = true;
                let r = contrast_ratio(color::rgb(fg), color::rgb(bg));
                result.pairs.push(LocalPair {
                    component: count,
                    segment,
                    support: n,
                    foreground: fg,
                    background: bg,
                    ratio: r,
                    lower_bound: !plateau && !rendered,
                });
                if r < worst {
                    worst = r;
                    worst_lower = !plateau && !rendered;
                    result.foreground = Some(fg);
                    result.background = Some(bg);
                    result.note = format!(
                        "Worst component {count}, segment pixel {segment}, core colour support {n} pixels; local ring {} pixels (guard 1px, radius 3px, encoded-sRGB range <=6). {}",
                        ring.len(),
                        if rendered {
                            "displayed stroke core"
                        } else if plateau {
                            "stroke-core plateau"
                        } else {
                            "lower bound: supported stroke-core 90th percentile, minimum two upper-tail pixels; thin stroke has no supported plateau"
                        }
                    );
                }
            }
            if !supported && component.iter().any(|i| distance(pixels[*i], bg) >= 1) {
                unknown = true;
                result.note.push_str(&format!(" Unsupported core component {count}, {size} ink pixels, max distance {max_distance}.", size=component.len()));
            }
        }
    }
    if !boundary && !rendered && !unknown {
        // Thin components constrain the common text ink from below. Use the
        // strongest supported body, while every supported independent plateau
        // remains worst-point evidence (including differently coloured ink).
        let point = result
            .pairs
            .iter()
            .filter(|p| !p.lower_bound)
            .min_by(|a, b| a.ratio.total_cmp(&b.ratio));
        let bound = result
            .pairs
            .iter()
            .filter(|p| p.lower_bound)
            .max_by(|a, b| a.ratio.total_cmp(&b.ratio));
        let selected = match (point, bound) {
            (Some(p), Some(b)) => Some(if p.ratio <= b.ratio { p } else { b }),
            (p, b) => p.or(b),
        };
        if let Some(p) = selected {
            worst = p.ratio;
            worst_lower = p.lower_bound;
            result.foreground = Some(p.foreground);
            result.background = Some(p.background);
            result.note = format!(
                "Common-ink thin-body lower bound: supported local core 90th percentile, minimum two upper-tail pixels; independent plateaus retain worst-point evidence. Selected component {}, support {} pixels. {}",
                p.component,
                p.support,
                if p.lower_bound {
                    "lower bound"
                } else {
                    "stroke-core plateau"
                }
            );
        }
    }
    if unknown {
        result
            .note
            .push_str(" Unsupported component or nonuniform local ring; UNMEASURABLE.");
    } else if worst.is_finite() {
        result.ratio = Some(worst);
    }
    if mixed_thin {
        result
            .note
            .push_str(" Thin mixed-direction colour cannot establish a contrast lower bound.");
    }
    result.lower_bound = worst_lower;
    if worst_lower {
        result
            .note
            .push_str(" Lower bound; below threshold must abstain, never FAIL.");
    }
    Ok(result)
}
