//! Bounded, deterministic connected-component analysis on opaque SDR pixels.
use super::*;
use crate::{Error, Result};
use image::RgbaImage;

/// Reject empty/out-of-image geometry, including arithmetic overflow.
pub fn validate_rect(size: [u32; 2], r: [u32; 4]) -> Result<()> {
    if r[2] == 0
        || r[3] == 0
        || r[0].checked_add(r[2]).is_none_or(|v| v > size[0])
        || r[1].checked_add(r[3]).is_none_or(|v| v > size[1])
    {
        return Err(Error::Config(
            "text region must be nonempty and inside its capture".into(),
        ));
    }
    Ok(())
}
fn luminance(p: &image::Rgba<u8>) -> f64 {
    let linear = |c: u8| {
        let v = f64::from(c) / 255.;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(p[0]) + 0.7152 * linear(p[1]) + 0.0722 * linear(p[2])
}
struct Field {
    width: usize,
    height: usize,
    values: Vec<f64>,
    bits: Vec<bool>,
    background: f64,
}
fn quantile(mut values: Vec<f64>, fraction: f64) -> f64 {
    values.sort_by(f64::total_cmp);
    values[((values.len().saturating_sub(1)) as f64 * fraction) as usize]
}
fn field(
    image: &RgbaImage,
    r: [u32; 4],
    mask: Option<&RgbaImage>,
    text_sampling: bool,
) -> Result<Option<Field>> {
    validate_rect([image.width(), image.height()], r)?;
    if u64::from(r[2]) * u64::from(r[3]) > crate::general::input::MAX_PIXELS {
        return Err(Error::Config("text region exceeds pixel limit".into()));
    }
    let mut values = Vec::with_capacity((r[2] * r[3]) as usize);
    let mut selected = Vec::with_capacity(values.capacity());
    let mut histogram = [0usize; 256];
    let mut sums = [0.; 256];
    for y in r[1]..r[1] + r[3] {
        for x in r[0]..r[0] + r[2] {
            let p = image.get_pixel(x, y);
            let include = mask.is_none_or(|m| m.get_pixel(x, y)[0] != 0);
            if include && p[3] != 255 {
                return Ok(None);
            }
            let v = luminance(p);
            values.push(v);
            selected.push(include);
            if include {
                let bin = (v * 255.).round() as usize;
                histogram[bin] += 1;
                sums[bin] += v;
            }
        }
    }
    let count = selected.iter().filter(|v| **v).count();
    let bin = (0..256).max_by_key(|i| histogram[*i]).unwrap_or(0);
    if count < 16 || histogram[bin] * 2 < count {
        return Ok(None);
    }
    let bg = sums[bin] / histogram[bin] as f64;
    let deviations: Vec<_> = values
        .iter()
        .zip(&selected)
        .filter(|(_, s)| **s)
        .map(|(v, _)| (v - bg).abs())
        .collect();
    let span = deviations.iter().copied().fold(0., f64::max);
    if span < 0.002 {
        return Ok(None);
    }
    let cutoff = if text_sampling {
        (span * 0.5).min(0.01)
    } else {
        span * 0.5
    };
    let rgb_bg = if text_sampling {
        let mut colors = std::collections::BTreeMap::new();
        for yy in r[1]..r[1] + r[3] {
            for xx in r[0]..r[0] + r[2] {
                let p = image.get_pixel(xx, yy);
                *colors.entry([p[0], p[1], p[2]]).or_insert(0usize) += 1;
            }
        }
        colors.into_iter().max_by_key(|(_, n)| *n).map(|(p, _)| p)
    } else {
        None
    };
    let rgb_cutoff = rgb_bg.map(|bg| {
        values
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let p = image.get_pixel(r[0] + i as u32 % r[2], r[1] + i as u32 / r[2]);
                (0..3).map(|c| p[c].abs_diff(bg[c])).max().unwrap_or(0)
            })
            .max()
            .map_or(3, |span| ((f64::from(span) * 0.15).ceil() as u8).max(3))
    });
    let bits: Vec<_> = values
        .iter()
        .zip(&selected)
        .enumerate()
        .map(|(i, (v, s))| {
            let p = image.get_pixel(r[0] + i as u32 % r[2], r[1] + i as u32 / r[2]);
            *s && rgb_bg.map_or((v - bg).abs() >= cutoff, |b| {
                (0..3).any(|c| p[c].abs_diff(b[c]) >= rgb_cutoff.unwrap_or(3))
            })
        })
        .collect();
    let fg: Vec<_> = values
        .iter()
        .zip(&bits)
        .filter(|(_, b)| **b)
        .map(|(v, _)| *v)
        .collect();
    if fg.len() < 4 {
        return Ok(None);
    }
    Ok(Some(Field {
        width: r[2] as usize,
        height: r[3] as usize,
        values,
        bits,
        background: bg,
    }))
}
struct Component {
    r: [usize; 4],
    indices: Vec<usize>,
}
fn components(f: &Field) -> Option<Vec<Component>> {
    let mut seen = vec![false; f.bits.len()];
    let mut out = Vec::new();
    for seed in 0..f.bits.len() {
        if !f.bits[seed] || seen[seed] {
            continue;
        }
        if out.len() >= 4096 {
            return None;
        }
        let mut stack = vec![seed];
        seen[seed] = true;
        let mut indices = Vec::new();
        let (mut x0, mut y0, mut x1, mut y1) = (f.width, f.height, 0, 0);
        while let Some(i) = stack.pop() {
            let (x, y) = (i % f.width, i / f.width);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
            indices.push(i);
            // Eight-connected keeps diagonal diamond borders intact.
            for yy in y.saturating_sub(1)..=(y + 1).min(f.height - 1) {
                for xx in x.saturating_sub(1)..=(x + 1).min(f.width - 1) {
                    let n = yy * f.width + xx;
                    if f.bits[n] && !seen[n] {
                        seen[n] = true;
                        stack.push(n);
                    }
                }
            }
        }
        out.push(Component {
            r: [x0, y0, x1 - x0 + 1, y1 - y0 + 1],
            indices,
        });
    }
    Some(out)
}
fn rectangle_score(f: &Field, r: [usize; 4]) -> f64 {
    let [mut x, mut y, mut w, mut h] = r;
    if !(0.3..=0.85).contains(&(w as f64 / h as f64)) {
        return 0.;
    }
    // Trim sparse rasterization tails for shape classification only; retain the
    // original component bounds in the report. No perimeter threshold is relaxed.
    while h > 8 && (x..x + w).filter(|xx| f.bits[y * f.width + xx]).count() * 2 < w {
        y += 1;
        h -= 1;
    }
    while h > 8
        && (x..x + w)
            .filter(|xx| f.bits[(y + h - 1) * f.width + xx])
            .count()
            * 2
            < w
    {
        h -= 1;
    }
    while w > 5 && (y..y + h).filter(|yy| f.bits[yy * f.width + x]).count() * 2 < h {
        x += 1;
        w -= 1;
    }
    while w > 5
        && (y..y + h)
            .filter(|yy| f.bits[yy * f.width + x + w - 1])
            .count()
            * 2
            < h
    {
        w -= 1;
    }
    if !(0.3..=0.85).contains(&(w as f64 / h as f64)) {
        return 0.;
    }
    let mut score: f64 = 0.;
    for thickness in 1..=w.min(h) / 4 {
        let mut border = 0;
        let mut border_on = 0;
        let mut inside = 0;
        let mut inside_on = 0;
        for yy in 0..h {
            for xx in 0..w {
                let on = f.bits[(y + yy) * f.width + x + xx];
                if xx < thickness || yy < thickness || xx + thickness >= w || yy + thickness >= h {
                    border += 1;
                    border_on += usize::from(on);
                } else {
                    inside += 1;
                    inside_on += usize::from(on);
                }
            }
        }
        let coverage = border_on as f64 / border as f64;
        let interior = inside_on as f64 / inside.max(1) as f64;
        if coverage >= 0.82 && interior < 0.10 {
            score = score.max(coverage * (1. - interior));
        }
    }
    score
}
fn detect(f: &Field, origin: [u32; 4]) -> Option<Vec<Candidate>> {
    let mut out = Vec::new();
    for c in components(f)? {
        let [x, y, w, h] = c.r;
        if !(5..=128).contains(&w) || !(8..=192).contains(&h) {
            continue;
        }
        let aspect = w as f64 / h as f64;
        let rectangle_score = rectangle_score(f, c.r);
        let shape = if (0.3..=0.85).contains(&aspect) && rectangle_score > 0. {
            Some(("hollow_rectangle", rectangle_score))
        } else if (0.75..=1.3).contains(&aspect) {
            let mut diamond = 0;
            let mut outside = 0;
            for i in &c.indices {
                let dx =
                    ((i % f.width - x) as f64 - (w - 1) as f64 / 2.).abs() / ((w - 1) as f64 / 2.);
                let dy =
                    ((i / f.width - y) as f64 - (h - 1) as f64 / 2.).abs() / ((h - 1) as f64 / 2.);
                if dx + dy <= 1.2 {
                    diamond += 1;
                } else {
                    outside += 1;
                }
            }
            let fill = c.indices.len() as f64 / (w * h) as f64;
            (outside * 20 < diamond && (0.25..0.65).contains(&fill))
                .then_some(("replacement_diamond", 0.7))
        } else {
            None
        };
        if let Some((shape, confidence)) = shape {
            out.push(Candidate {
                rect_px: [
                    origin[0] + x as u32,
                    origin[1] + y as u32,
                    w as u32,
                    h as u32,
                ],
                shape: shape.into(),
                confidence,
            });
        }
    }
    Some(out)
}
/// Inspect a capture (optionally masked); missing shapes never yield a clean verdict.
pub fn tofu(
    image: &RgbaImage,
    mask: Option<&RgbaImage>,
    hash: String,
    mask_hash: Option<String>,
    expected_text: Option<String>,
    ocr: OcrEvidence,
) -> Result<TofuReport> {
    if let Some(m) = mask
        && m.dimensions() != image.dimensions()
    {
        return Err(Error::Config(
            "text mask dimensions must match image".into(),
        ));
    }
    let r = [0, 0, image.width(), image.height()];
    let regions = field(image, r, mask, false)?.and_then(|f| detect(&f, r));
    let mut reasons=vec!["shape scores are heuristic; square boxes are ambiguous with valid glyphs; no candidate does not prove font coverage".into()];
    let regions = match regions {
        Some(v) => v,
        None => {
            reasons.push(
                "insufficient opaque uniform-background pixels or component budget exceeded".into(),
            );
            vec![]
        }
    };
    let state = if regions.is_empty() {
        reasons
            .push("no supported pixel shape found; glyph completeness remains unverified".into());
        State::InsufficientEvidence
    } else {
        State::Candidates
    };
    Ok(TofuReport {
        report_id: None,
        source_refs: Vec::new(),
        schema: TOFU_SCHEMA.into(),
        state,
        image_sha256: hash,
        dimensions: [image.width(), image.height()],
        mask_sha256: mask_hash,
        expected_text,
        regions,
        ocr,
        reasons,
    })
}
fn stroke(f: &Field, c: &Component) -> f64 {
    let [x, y, w, h] = c.r;
    let mut runs = Vec::new();
    for yy in y..y + h {
        let mut n = 0;
        for xx in x..=x + w {
            if xx < x + w && f.bits[yy * f.width + xx] {
                n += 1;
            } else if n > 0 {
                runs.push(n as f64);
                n = 0;
            }
        }
    }
    for xx in x..x + w {
        let mut n = 0;
        for yy in y..=y + h {
            if yy < y + h && f.bits[yy * f.width + xx] {
                n += 1;
            } else if n > 0 {
                runs.push(n as f64);
                n = 0;
            }
        }
    }
    quantile(runs, 0.25)
}
// Letter bodies exclude detached marks, punctuation and joined components.
// Counters are holes in an eight-connected body, never independent samples.
fn body_heights(f: &Field, cs: &[Component]) -> Vec<f64> {
    let max_height = cs
        .iter()
        .filter(|c| c.indices.len() >= 6 && c.r[2] <= c.r[3] * 2)
        .map(|c| c.r[3])
        .max()
        .unwrap_or(0);
    cs.iter()
        .filter(|c| {
            c.indices.len() >= 6
                && (c.r[3] >= 5 || c.r[2] * 4 <= c.r[3] * 3 && c.indices.len() == c.r[2] * c.r[3])
                && (c.r[3] * 5 >= max_height * 3
                    || c.r[2] * 4 <= c.r[3] * 3 && c.indices.len() == c.r[2] * c.r[3])
                && c.r[2] <= c.r[3] * 2
                && c.r[0] > 0
                && c.r[1] > 0
                && c.r[0] + c.r[2] < f.width
                && c.r[1] + c.r[3] < f.height
        })
        .map(|c| c.r[3] as f64)
        .collect()
}
/// Robust x-height proxy: lower quartile of substantial disconnected letter bodies.
/// Dots, punctuation, diacritics, counters and joined components cannot set size.
pub fn x_height(image: &RgbaImage, r: [u32; 4]) -> Result<Option<f64>> {
    let Some(f) = field(image, r, None, true)? else {
        return Ok(None);
    };
    let Some(cs) = components(&f) else {
        return Ok(None);
    };
    let heights = body_heights(&f, &cs);
    Ok((!heights.is_empty()).then(|| quantile(heights, 0.25)))
}
/// Robust letter-body height for explicit display-scale classification.
/// Uses the same supported bodies as legibility, with the upper quartile.
pub fn line_height(image: &RgbaImage, r: [u32; 4]) -> Result<Option<f64>> {
    let Some(f) = field(image, r, None, true)? else {
        return Ok(None);
    };
    let Some(cs) = components(&f) else {
        return Ok(None);
    };
    let heights = body_heights(&f, &cs);
    Ok((!heights.is_empty()).then(|| quantile(heights, 0.75)))
}
/// Measure one region. No glyphs, clipping, mixed/transparent backgrounds abstain.
pub fn legibility(
    image: &RgbaImage,
    r: [u32; 4],
    region: usize,
    policy: Policy,
) -> Result<RegionResult> {
    if !policy.minimum_contrast.is_finite()
        || !(1.0..=21.).contains(&policy.minimum_contrast)
        || !policy.minimum_x_height_px.is_finite()
        || policy.minimum_x_height_px <= 0.
        || !policy.minimum_sharpness.is_finite()
        || !(0.0..=1.).contains(&policy.minimum_sharpness)
        || !policy.minimum_stroke_px.is_finite()
        || policy.minimum_stroke_px <= 0.
    {
        return Err(Error::Config("invalid text legibility thresholds".into()));
    }
    let mut result = RegionResult {
        region,
        rect_px: r,
        state: State::InsufficientEvidence,
        contrast: None,
        x_height_px: None,
        sharpness: None,
        stroke_px: None,
        ocr: unavailable_ocr("OCR not supplied"),
        reasons: vec![],
    };
    let Some(f) = field(image, r, None, true)? else {
        result
            .reasons
            .push("no separable text pixels or opaque dominant background".into());
        return Ok(result);
    };
    let Some(cs) = components(&f) else {
        result.reasons.push("component budget exceeded".into());
        return Ok(result);
    };
    let heights = body_heights(&f, &cs);
    let cs: Vec<_> = cs
        .iter()
        .filter(|c| c.indices.len() >= 3 && c.r[3] >= 3)
        .collect();
    if cs.is_empty() {
        result.reasons.push("no measurable glyph bodies".into());
        return Ok(result);
    }
    if !heights.is_empty() {
        result.x_height_px = Some(quantile(heights.clone(), 0.25));
    }
    if cs.iter().any(|c| {
        c.r[0] == 0 || c.r[1] == 0 || c.r[0] + c.r[2] == f.width || c.r[1] + c.r[3] == f.height
    }) {
        result.reasons.push(
            "foreground touches region boundary; clipping or correspondence uncertain".into(),
        );
        return Ok(result);
    }
    let component_foreground = |c: &Component| {
        quantile(
            c.indices.iter().map(|i| f.values[*i]).collect(),
            if f.background > 0.5 { 0.25 } else { 0.75 },
        )
    };
    let opaque = image::RgbImage::from_fn(image.width(), image.height(), |x, y| {
        let p = image.get_pixel(x, y);
        image::Rgb([p[0], p[1], p[2]])
    });
    let estimate = crate::contrast::estimate_rendered(
        &opaque,
        [
            r[0] as f64 / image.width() as f64,
            r[1] as f64 / image.height() as f64,
            r[2] as f64 / image.width() as f64,
            r[3] as f64 / image.height() as f64,
        ],
    )?;
    let contrast = estimate.ratio;
    let uncertain_contrast = contrast.is_none();
    let height = (!heights.is_empty()).then(|| quantile(heights, 0.25));
    let sharpness = cs
        .iter()
        .map(|c| {
            let range = (f.background - component_foreground(c)).abs();
            let [x, y, w, h] = c.r;
            let mut step: f64 = 0.;
            for yy in y.saturating_sub(1)..(y + h + 1).min(f.height) {
                for xx in x.saturating_sub(1)..(x + w + 1).min(f.width) {
                    let i = yy * f.width + xx;
                    if xx + 1 < f.width {
                        step = step.max((f.values[i] - f.values[i + 1]).abs());
                    }
                    if yy + 1 < f.height {
                        step = step.max((f.values[i] - f.values[i + f.width]).abs());
                    }
                }
            }
            (step / range).min(1.)
        })
        .fold(f64::INFINITY, f64::min);
    let strokes = cs
        .iter()
        .map(|c| stroke(&f, c))
        .fold(f64::INFINITY, f64::min);
    result.contrast = contrast;
    result.x_height_px = height;
    result.sharpness = Some(sharpness);
    result.stroke_px = Some(strokes);
    for (value, min, reason) in [
        (
            height.unwrap_or(f64::INFINITY),
            policy.minimum_x_height_px,
            "too_small",
        ),
        (sharpness, policy.minimum_sharpness, "blurred"),
        (strokes, policy.minimum_stroke_px, "undersampled_strokes"),
    ] {
        if value < min {
            result.reasons.push(reason.into());
        }
    }
    if detect(&f, r).is_some_and(|v| !v.is_empty()) {
        result.reasons.push("missing_glyph_candidate".into());
    }
    if contrast.is_some_and(|c| c < policy.minimum_contrast) {
        result.reasons.push("low_contrast".into());
    }
    // Missing size cannot hide independently supported blur/stroke/contrast
    // failures; joined blurred bodies still support a sharpness measurement.
    let failed = !result.reasons.is_empty();
    if height.is_none() {
        result
            .reasons
            .push("no supported disconnected letter bodies (marks or undersampled text)".into());
    }
    result.state = if failed {
        State::Illegible
    } else if uncertain_contrast || height.is_none() {
        State::InsufficientEvidence
    } else {
        result.reasons.push("pixel thresholds satisfied; human readability and glyph completeness are not certified".into());
        State::Legible
    };
    if uncertain_contrast {
        result.reasons.push(estimate.note);
    }
    Ok(result)
}
