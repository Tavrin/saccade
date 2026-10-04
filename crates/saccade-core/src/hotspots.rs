//! Hotspots: where on the frame the FLIP error is concentrated.
//!
//! An agent that cannot look at the heatmap still needs to know *where* the
//! visible difference is. [`find_hotspots`] thresholds the error map, groups
//! the pixels above the threshold into 8-connected components with a single
//! raster pass (union-find, two label rows, no full-frame label buffer), merges
//! components whose boxes are close, and ranks the groups by summed error.

use crate::report::Hotspot;

/// Default error value above which a pixel counts as part of a hotspot.
pub const DEFAULT_HOTSPOT_THRESHOLD: f32 = 0.1;

/// Default number of hotspots kept per entry.
pub const DEFAULT_HOTSPOTS: usize = 5;

/// Default smallest share of the frame's total error a hotspot must carry to
/// be kept: groups below 1% are noise next to the real concentrations.
pub const DEFAULT_HOTSPOT_MIN_SHARE: f64 = 0.01;

/// Components kept (by summed error) before the box-merging step, which is
/// quadratic. A frame with more is mostly noise; the rest still count towards
/// the total error that `share_of_total_error` is measured against.
const MAX_CANDIDATES: usize = 256;

/// Settings for [`find_hotspots`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HotspotOptions {
    /// Pixels with an error strictly above this are "hot".
    pub threshold: f32,
    /// How many hotspots to keep; `0` disables the search.
    pub top_k: usize,
    /// Hotspots carrying less than this share of the total error (`0..=1`)
    /// are dropped.
    pub min_share: f64,
}

impl Default for HotspotOptions {
    fn default() -> Self {
        Self {
            threshold: DEFAULT_HOTSPOT_THRESHOLD,
            top_k: DEFAULT_HOTSPOTS,
            min_share: DEFAULT_HOTSPOT_MIN_SHARE,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Comp {
    parent: u32,
    count: u64,
    sum: f64,
    /// Inclusive bounding box.
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
}

impl Comp {
    fn merge(&mut self, o: &Comp) {
        self.count += o.count;
        self.sum += o.sum;
        self.x0 = self.x0.min(o.x0);
        self.y0 = self.y0.min(o.y0);
        self.x1 = self.x1.max(o.x1);
        self.y1 = self.y1.max(o.y1);
    }

    fn near(&self, o: &Comp, gap: u32) -> bool {
        self.x0 <= o.x1.saturating_add(gap)
            && o.x0 <= self.x1.saturating_add(gap)
            && self.y0 <= o.y1.saturating_add(gap)
            && o.y0 <= self.y1.saturating_add(gap)
    }
}

fn find(comps: &mut [Comp], mut i: u32) -> u32 {
    loop {
        let p = comps[i as usize].parent;
        if p == i {
            return i;
        }
        let gp = comps[p as usize].parent;
        comps[i as usize].parent = gp;
        i = gp;
    }
}

/// Coarse 3x3 position of a rectangle's centre: `top-left`, `top-center`,
/// `top-right`, `middle-left`, `center`, `middle-right`, `bottom-left`,
/// `bottom-center`, `bottom-right`.
pub fn position_label(rect: [u32; 4], width: u32, height: u32) -> &'static str {
    const NAMES: [[&str; 3]; 3] = [
        ["top-left", "top-center", "top-right"],
        ["middle-left", "center", "middle-right"],
        ["bottom-left", "bottom-center", "bottom-right"],
    ];
    let cell = |origin: u32, len: u32, full: u32| -> usize {
        let centre2 = u64::from(origin) * 2 + u64::from(len);
        let idx = centre2 * 3 / (u64::from(full.max(1)) * 2);
        idx.min(2) as usize
    };
    NAMES[cell(rect[1], rect[3], height)][cell(rect[0], rect[2], width)]
}

/// Finds the top hotspots of `error_map` (`width * height`, row-major).
///
/// Masked pixels (`mask[i]`) are excluded everywhere. Components are 8-connected
/// runs of pixels above `opts.threshold`; components whose bounding boxes lie
/// within about 1% of the long side of each other are merged. Hotspots are
/// returned by decreasing summed error. An empty vector when `opts.top_k` is 0,
/// the buffer does not match the size, or nothing exceeds the threshold.
pub fn find_hotspots(
    error_map: &[f32],
    mask: Option<&[bool]>,
    width: u32,
    height: u32,
    opts: &HotspotOptions,
) -> Vec<Hotspot> {
    let (w, h) = (width as usize, height as usize);
    if opts.top_k == 0
        || w == 0
        || h == 0
        || error_map.len() != w * h
        || mask.is_some_and(|m| m.len() != w * h)
    {
        return Vec::new();
    }
    let thr = opts.threshold;

    // Single raster pass. `prev`/`cur` hold the component label (1-based, 0 =
    // none) of the previous and current row; stats merge at union time.
    let mut comps: Vec<Comp> = Vec::new();
    let mut prev = vec![0u32; w];
    let mut cur = vec![0u32; w];
    let mut labels = vec![0u32; w * h];
    let mut total = 0.0f64;
    for y in 0..h {
        let row = &error_map[y * w..(y + 1) * w];
        let mrow = mask.map(|m| &m[y * w..(y + 1) * w]);
        for x in 0..w {
            cur[x] = 0;
            let v = row[x];
            if !v.is_finite() || mrow.is_some_and(|m| m[x]) {
                continue;
            }
            total += f64::from(v);
            if v <= thr {
                continue;
            }
            let mut root = 0u32;
            let mut touch = |label: u32, comps: &mut Vec<Comp>| {
                if label == 0 {
                    return;
                }
                let r = find(comps, label - 1) + 1;
                if root == 0 {
                    root = r;
                } else if r != root {
                    let other = comps[r as usize - 1];
                    comps[r as usize - 1].parent = root - 1;
                    comps[root as usize - 1].merge(&other);
                }
            };
            if x > 0 {
                touch(cur[x - 1], &mut comps);
            }
            if y > 0 {
                if x > 0 {
                    touch(prev[x - 1], &mut comps);
                }
                touch(prev[x], &mut comps);
                if x + 1 < w {
                    touch(prev[x + 1], &mut comps);
                }
            }
            let (xu, yu) = (x as u32, y as u32);
            if root == 0 {
                comps.push(Comp {
                    parent: comps.len() as u32,
                    count: 0,
                    sum: 0.0,
                    x0: xu,
                    y0: yu,
                    x1: xu,
                    y1: yu,
                });
                root = comps.len() as u32;
            }
            let c = &mut comps[root as usize - 1];
            c.count += 1;
            c.sum += f64::from(v);
            c.x0 = c.x0.min(xu);
            c.x1 = c.x1.max(xu);
            c.y1 = c.y1.max(yu);
            cur[x] = root;
            labels[y * w + x] = root;
        }
        std::mem::swap(&mut prev, &mut cur);
    }

    for label in &mut labels {
        if *label != 0 {
            *label = find(&mut comps, *label - 1) + 1;
        }
    }
    let mut roots: Vec<(Comp, Vec<u32>)> = comps
        .iter()
        .enumerate()
        .filter(|(i, c)| c.parent as usize == *i)
        .map(|(i, c)| (*c, vec![i as u32 + 1]))
        .collect();
    if roots.is_empty() {
        return Vec::new();
    }
    roots.sort_by(|a, b| b.0.sum.total_cmp(&a.0.sum));
    roots.truncate(MAX_CANDIDATES);

    // Merge nearby components until no two boxes are within `gap`.
    let gap = (width.max(height) / 100).max(2);
    'outer: loop {
        for i in 0..roots.len() {
            for j in i + 1..roots.len() {
                if roots[i].0.near(&roots[j].0, gap) {
                    let other = roots.swap_remove(j);
                    roots[i].0.merge(&other.0);
                    roots[i].1.extend(other.1);
                    continue 'outer;
                }
            }
        }
        break;
    }
    roots.sort_by(|a, b| b.0.sum.total_cmp(&a.0.sum));
    if opts.min_share > 0.0 && total > 0.0 {
        roots.retain(|c| c.0.sum / total >= opts.min_share);
    }
    roots.truncate(opts.top_k);

    roots
        .iter()
        .map(|(c, members)| {
            describe(
                c,
                (members, &labels),
                error_map,
                mask,
                (width, height),
                total,
            )
        })
        .collect()
}

/// Detect a severe connected component over the full map. Display caps and
/// minimum error-share filtering must never affect this qualification rule.
pub fn has_severe_component(
    error_map: &[f32],
    mask: Option<&[bool]>,
    width: u32,
    height: u32,
    threshold: f64,
    min_pixels: u32,
) -> bool {
    let (w, h) = (width as usize, height as usize);
    if w == 0
        || h == 0
        || error_map.len() != w.saturating_mul(h)
        || mask.is_some_and(|m| m.len() != error_map.len())
    {
        return false;
    }
    let mut seen = vec![false; error_map.len()];
    let mut stack = Vec::new();
    for start in 0..error_map.len() {
        if seen[start]
            || mask.is_some_and(|m| m[start])
            || !error_map[start].is_finite()
            || f64::from(error_map[start]) < threshold
        {
            continue;
        }
        seen[start] = true;
        stack.push(start);
        let mut count = 0u32;
        while let Some(i) = stack.pop() {
            count += 1;
            if count >= min_pixels {
                return true;
            }
            let x = i % w;
            let y = i / w;
            for ny in y.saturating_sub(1)..=(y + 1).min(h - 1) {
                for nx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                    let n = ny * w + nx;
                    if !seen[n]
                        && !mask.is_some_and(|m| m[n])
                        && error_map[n].is_finite()
                        && f64::from(error_map[n]) >= threshold
                    {
                        seen[n] = true;
                        stack.push(n);
                    }
                }
            }
        }
    }
    false
}

/// Exact thresholded pixel mask independent of display hotspot limits.
pub fn threshold_runs(error_map: &[f32], mask: Option<&[bool]>, threshold: f32) -> Vec<[u32; 2]> {
    if mask.is_some_and(|m| m.len() != error_map.len()) {
        return Vec::new();
    }
    let mut runs: Vec<[u32; 2]> = Vec::new();
    for (i, value) in error_map.iter().enumerate() {
        if value.is_finite() && *value > threshold && !mask.is_some_and(|m| m[i]) {
            let i = i as u32;
            if let Some(last) = runs.last_mut()
                && last[0] + last[1] == i
            {
                last[1] += 1;
                continue;
            }
            runs.push([i, 1]);
        }
    }
    runs
}

/// Statistics of one merged group over its bounding box.
fn describe(
    c: &Comp,
    membership: (&[u32], &[u32]),
    error_map: &[f32],
    mask: Option<&[bool]>,
    dimensions: (u32, u32),
    total: f64,
) -> Hotspot {
    let (members, labels) = membership;
    let (width, height) = dimensions;
    let mut pixel_runs: Vec<[u32; 2]> = Vec::new();
    for (i, label) in labels.iter().enumerate() {
        if members.contains(label) && *label != 0 {
            let i = i as u32;
            if let Some(last) = pixel_runs.last_mut()
                && last[0] + last[1] == i
            {
                last[1] += 1;
                continue;
            }
            pixel_runs.push([i, 1]);
        }
    }
    let w = width as usize;
    let (mut sum, mut n, mut max) = (0.0f64, 0u64, 0.0f32);
    for y in c.y0..=c.y1 {
        for x in c.x0..=c.x1 {
            let i = y as usize * w + x as usize;
            let v = error_map[i];
            if v.is_finite() && !mask.is_some_and(|m| m[i]) {
                sum += f64::from(v);
                n += 1;
                max = max.max(v);
            }
        }
    }
    let rect = [c.x0, c.y0, c.x1 - c.x0 + 1, c.y1 - c.y0 + 1];
    let (fw, fh) = (f64::from(width), f64::from(height));
    Hotspot {
        pixel_runs,
        rect_px: rect,
        rect_frac: [
            f64::from(rect[0]) / fw,
            f64::from(rect[1]) / fh,
            f64::from(rect[2]) / fw,
            f64::from(rect[3]) / fh,
        ],
        area_px: c.count,
        area_frac: c.count as f64 / (fw * fh),
        mean_flip: if n == 0 { 0.0 } else { sum / n as f64 },
        max_flip: f64::from(max),
        share_of_total_error: if total > 0.0 { c.sum / total } else { 0.0 },
        position: position_label(rect, width, height).to_string(),
    }
}

/// One compact line for a text table or a Markdown list: the hotspot count and
/// the top few, for example
/// `2 hotspots: 120x80 @ bottom-center (61% of error), 40x40 @ top-left (12%)`.
/// `None` when there are none.
pub fn summary_line(hotspots: &[Hotspot]) -> Option<String> {
    if hotspots.is_empty() {
        return None;
    }
    let shown: Vec<String> = hotspots
        .iter()
        .take(3)
        .enumerate()
        .map(|(i, h)| {
            let pct = (h.share_of_total_error * 100.0).round();
            let tail = if i == 0 {
                format!("{pct:.0}% of error")
            } else {
                format!("{pct:.0}%")
            };
            format!(
                "{}x{} @ {} ({tail})",
                h.rect_px[2], h.rect_px[3], h.position
            )
        })
        .collect();
    let n = hotspots.len();
    // One box over most of the frame is a global change, not a place.
    let global = hotspots
        .first()
        .is_some_and(|h| h.rect_frac[2] * h.rect_frac[3] >= 0.5 && h.area_frac >= 0.25);
    let more = if n > 3 { ", ..." } else { "" };
    Some(format!(
        "{n} hotspot{}: {}{more}{}",
        if n == 1 { "" } else { "s" },
        shown.join(", "),
        if global { " [frame-wide change]" } else { "" }
    ))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn severe_small_component_survives_display_share_and_cap() {
        let (w, h) = (100usize, 100usize);
        let mut map = vec![0.1f32; w * h];
        for y in 80..84 {
            for x in 80..84 {
                map[y * w + x] = 0.6;
            }
        }
        let displayed = find_hotspots(
            &map,
            None,
            w as u32,
            h as u32,
            &HotspotOptions {
                threshold: 0.5,
                top_k: 0,
                min_share: 0.5,
            },
        );
        assert!(displayed.is_empty());
        assert!(has_severe_component(
            &map, None, w as u32, h as u32, 0.5, 16
        ));
    }

    fn blob(map: &mut [f32], w: usize, rect: [usize; 4], v: f32) {
        for y in rect[1]..rect[1] + rect[3] {
            for x in rect[0]..rect[0] + rect[2] {
                map[y * w + x] = v;
            }
        }
    }

    #[test]
    fn finds_injected_blob_and_ignores_masked_pixels() {
        let (w, h) = (200usize, 120usize);
        let mut map = vec![0.0f32; w * h];
        blob(&mut map, w, [150, 80, 30, 20], 0.6);
        blob(&mut map, w, [20, 10, 10, 10], 0.3);
        let found = find_hotspots(&map, None, w as u32, h as u32, &HotspotOptions::default());
        assert_eq!(found.len(), 2);
        let top = &found[0];
        for (got, want) in top.rect_px.iter().zip([150u32, 80, 30, 20]) {
            assert!(got.abs_diff(want) <= 2, "{:?}", top.rect_px);
        }
        assert_eq!(top.area_px, 600);
        assert_eq!(top.position, "bottom-right");
        assert!((top.max_flip - 0.6).abs() < 1e-6);
        assert!((top.share_of_total_error - 360.0 / 390.0).abs() < 1e-4);

        // Mask the big blob away: only the small one is left.
        let mut mask = vec![false; w * h];
        blob_mask(&mut mask, w, [140, 70, 50, 40]);
        let found = find_hotspots(
            &map,
            Some(&mask),
            w as u32,
            h as u32,
            &HotspotOptions::default(),
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].position, "top-left");
        assert!((found[0].share_of_total_error - 1.0).abs() < 1e-9);
    }

    fn blob_mask(mask: &mut [bool], w: usize, rect: [usize; 4]) {
        for y in rect[1]..rect[1] + rect[3] {
            for x in rect[0]..rect[0] + rect[2] {
                mask[y * w + x] = true;
            }
        }
    }
}
