//! The evidence encoder of judge mode: what a judge that cannot see pixels is
//! told about a pair of images, and the blind strips a vision judge sees.
//!
//! The encoder never emits raw pixels. For a text judge it adds to the
//! decision-request state a coarse 8x8 FLIP grid (means rounded to 2
//! decimals), the colour change inside each changed region as English colour
//! names (nearest neighbour over CIELAB in a small built-in table), and an
//! optional OCR text difference from an external command (`--ocr-cmd`, no
//! dependency). A vision judge gets only hotspot crops, contrast-stretched,
//! labelled `1` and `2` in either order, never a full frame.

use std::path::Path;
use std::process::{Command, Stdio};

use image::{Rgb, RgbImage, imageops};
use serde::Serialize;

use crate::report::Hotspot;

/// Cells per side of the FLIP grid.
pub const GRID: usize = 8;

/// Longest OCR line kept, in characters.
const OCR_LINE: usize = 120;
/// Most OCR lines kept per side of the diff.
const OCR_LINES: usize = 20;
/// Largest OCR output read, in bytes.
const OCR_BYTES: usize = 64 * 1024;

/// A named reference colour (sRGB).
const NAMES: &[(&str, [u8; 3])] = &[
    ("black", [0, 0, 0]),
    ("near-black", [20, 20, 24]),
    ("very dark grey", [45, 45, 45]),
    ("dark grey", [80, 80, 80]),
    ("grey", [128, 128, 128]),
    ("light grey", [180, 180, 180]),
    ("very light grey", [215, 215, 215]),
    ("near-white", [242, 242, 242]),
    ("white", [255, 255, 255]),
    ("red", [220, 30, 30]),
    ("dark red", [120, 20, 20]),
    ("orange", [240, 140, 20]),
    ("brown", [110, 70, 30]),
    ("beige", [220, 200, 160]),
    ("yellow", [240, 220, 30]),
    ("olive", [120, 120, 30]),
    ("lime", [150, 220, 40]),
    ("green", [40, 160, 50]),
    ("dark green", [20, 80, 35]),
    ("teal", [20, 140, 130]),
    ("cyan", [40, 210, 230]),
    ("sky blue", [120, 180, 235]),
    ("blue", [35, 70, 220]),
    ("navy", [20, 30, 100]),
    ("purple", [120, 50, 160]),
    ("magenta", [210, 40, 170]),
    ("pink", [245, 150, 185]),
];

fn lin(c: f64) -> f64 {
    let c = c / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// sRGB (components 0 to 255) to CIELAB (D65).
pub fn rgb_to_lab(rgb: [f64; 3]) -> [f64; 3] {
    let (r, g, b) = (lin(rgb[0]), lin(rgb[1]), lin(rgb[2]));
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    let f = |t: f64| {
        if t > 216.0 / 24389.0 {
            t.cbrt()
        } else {
            (24389.0 / 27.0 * t + 16.0) / 116.0
        }
    };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// The English name of the nearest table colour in CIELAB.
pub fn colour_name(rgb: [f64; 3]) -> &'static str {
    let lab = rgb_to_lab(rgb);
    let dist = |c: &[u8; 3]| {
        let t = rgb_to_lab([f64::from(c[0]), f64::from(c[1]), f64::from(c[2])]);
        (0..3).map(|i| (lab[i] - t[i]).powi(2)).sum::<f64>()
    };
    NAMES
        .iter()
        .min_by(|a, b| dist(&a.1).total_cmp(&dist(&b.1)))
        .map_or("grey", |(n, _)| n)
}

/// How the tint moved from `from` to `to`: a hue word with a strength
/// (`slightly`, none, `strongly`), or `None` when the chroma barely changed.
fn tint(from: [f64; 3], to: [f64; 3]) -> Option<String> {
    let (a, b) = (to[1] - from[1], to[2] - from[2]);
    let c = a.hypot(b);
    if c < 3.0 {
        return None;
    }
    let angle = b.atan2(a).to_degrees().rem_euclid(360.0);
    // CIELAB hue angles are not equally spaced sRGB hue sectors. In
    // particular a blue displacement is near 295 degrees, not 270.
    let hue = match angle {
        x if !(50.0..340.0).contains(&x) => "red",
        x if x < 85.0 => "orange",
        x if x < 120.0 => "yellow",
        x if x < 175.0 => "green",
        x if x < 240.0 => "teal",
        x if x < 310.0 => "blue",
        _ => "purple",
    };
    let strength = if c < 8.0 {
        "slightly "
    } else if c < 20.0 {
        ""
    } else {
        "strongly "
    };
    Some(format!("{strength}{hue}"))
}

/// A colour change in English: `dark grey -> near-black, slightly blue`.
pub fn describe_shift(from: [f64; 3], to: [f64; 3]) -> String {
    let (nf, nt) = (colour_name(from), colour_name(to));
    let base = format!("{nf} \u{2192} {nt}");
    match tint(rgb_to_lab(from), rgb_to_lab(to)) {
        Some(t) => format!("{base}, {t}"),
        None => base,
    }
}

/// The colour change inside one changed region.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ColourShift {
    /// 1-based hotspot the region is.
    pub hotspot: u32,
    /// Mean colour of the reference over the region's changed pixels, as a name.
    pub reference: String,
    /// The same for the candidate.
    pub candidate: String,
    /// The change in one phrase, for example `dark grey -> near-black, slightly blue`.
    pub change: String,
}

/// An OCR text difference between the two images.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OcrDiff {
    /// Lines only the reference has.
    pub removed: Vec<String>,
    /// Lines only the candidate has.
    pub added: Vec<String>,
    /// Lines both have.
    pub unchanged_lines: usize,
}

/// What the encoder adds to a decision-request state. Numbers and words only.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Extras {
    /// Mean FLIP error of an 8x8 grid of cells, rows top to bottom, rounded to 2 decimals.
    pub flip_grid_8x8: Vec<Vec<f64>>,
    /// Colour change inside the largest hotspots.
    pub colour_shifts: Vec<ColourShift>,
    /// OCR text difference, when an OCR command is configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ocr_diff: Option<OcrDiff>,
}

/// Mean of the error map per grid cell, rounded to 2 decimals. Non-finite
/// values count as 0 (they are reported elsewhere).
pub fn flip_grid(error_map: &[f32], width: u32, height: u32) -> Vec<Vec<f64>> {
    let (w, h) = (width as usize, height as usize);
    let mut out = vec![vec![0.0; GRID]; GRID];
    if w == 0 || h == 0 || error_map.len() != w * h {
        return out;
    }
    for (gy, row) in out.iter_mut().enumerate() {
        for (gx, cell) in row.iter_mut().enumerate() {
            let (x0, x1) = (gx * w / GRID, ((gx + 1) * w / GRID).max(gx * w / GRID + 1));
            let (y0, y1) = (gy * h / GRID, ((gy + 1) * h / GRID).max(gy * h / GRID + 1));
            let (mut sum, mut n) = (0.0f64, 0u64);
            for y in y0..y1.min(h) {
                for &v in &error_map[y * w + x0..y * w + x1.min(w)] {
                    if v.is_finite() {
                        sum += f64::from(v);
                    }
                    n += 1;
                }
            }
            *cell = if n == 0 {
                0.0
            } else {
                (sum / n as f64 * 100.0).round() / 100.0
            };
        }
    }
    out
}

/// Mean colour of `img` over the changed pixels (error above 0.1) of `rect`,
/// or over the whole rectangle when none exceeds it.
fn region_mean(img: &RgbImage, error_map: &[f32], rect: [u32; 4], changed_only: bool) -> [f64; 3] {
    let (w, h) = img.dimensions();
    let mut acc = [0.0f64; 3];
    let mut n = 0u64;
    for y in rect[1]..rect[1].saturating_add(rect[3]).min(h) {
        for x in rect[0]..rect[0].saturating_add(rect[2]).min(w) {
            let e = error_map.get(y as usize * w as usize + x as usize);
            if changed_only && !e.is_some_and(|e| *e > 0.1) {
                continue;
            }
            let p = img.get_pixel(x, y).0;
            for (a, c) in acc.iter_mut().zip(p) {
                *a += f64::from(c);
            }
            n += 1;
        }
    }
    if n == 0 {
        return if changed_only {
            region_mean(img, error_map, rect, false)
        } else {
            [0.0; 3]
        };
    }
    acc.map(|a| a / n as f64)
}

/// The colour shifts of the first `top` hotspots, reference to candidate.
pub fn colour_shifts(
    base: &RgbImage,
    cap: &RgbImage,
    error_map: &[f32],
    hotspots: &[Hotspot],
    top: usize,
) -> Vec<ColourShift> {
    hotspots
        .iter()
        .take(top)
        .enumerate()
        .map(|(i, h)| {
            let from = region_mean(base, error_map, h.rect_px, true);
            let to = region_mean(cap, error_map, h.rect_px, true);
            ColourShift {
                hotspot: (i + 1) as u32,
                reference: colour_name(from).to_owned(),
                candidate: colour_name(to).to_owned(),
                change: describe_shift(from, to),
            }
        })
        .collect()
}

/// Runs `cmd` on `image` and returns its standard output, at most 64 KiB. The
/// command is split on whitespace (no shell); `{}` is replaced by the image
/// path (also accepts `{image}`), or appended when neither placeholder exists.
fn run_ocr(cmd: &str, image: &Path) -> Result<String, String> {
    let mut parts = cmd.split_whitespace();
    let program = parts.next().ok_or("the OCR command is empty")?;
    let path = image.to_string_lossy();
    let mut args: Vec<String> = parts
        .map(|p| p.replace("{image}", &path).replace("{}", &path))
        .collect();
    if !cmd.contains("{}") && !cmd.contains("{image}") {
        args.push(path.into_owned());
    }
    let out = Command::new(program)
        .args(&args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("running the OCR command {program:?}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "the OCR command {program:?} exited with {}",
            out.status
        ));
    }
    let take = &out.stdout[..out.stdout.len().min(OCR_BYTES)];
    Ok(String::from_utf8_lossy(take).into_owned())
}

fn lines_of(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .map(|l| l.chars().take(OCR_LINE).collect())
        .collect()
}

/// The line-level text difference of two OCR outputs (multiset semantics).
pub fn ocr_diff_of(reference: &str, candidate: &str) -> OcrDiff {
    let (a, mut b) = (lines_of(reference), lines_of(candidate));
    let mut removed = Vec::new();
    let mut unchanged = 0;
    for line in a {
        match b.iter().position(|l| *l == line) {
            Some(i) => {
                b.remove(i);
                unchanged += 1;
            }
            None => removed.push(line),
        }
    }
    removed.truncate(OCR_LINES);
    b.truncate(OCR_LINES);
    OcrDiff {
        removed,
        added: b,
        unchanged_lines: unchanged,
    }
}

/// Runs the OCR command on both image files and diffs the text.
///
/// The text of a private screenshot goes to the judges in the evidence; point
/// this only at images you may share (see the README's privacy note).
pub fn ocr_diff(cmd: &str, reference: &Path, candidate: &Path) -> Result<OcrDiff, String> {
    Ok(ocr_diff_of(
        &run_ocr(cmd, reference)?,
        &run_ocr(cmd, candidate)?,
    ))
}

/// Padding around a hotspot crop, in pixels.
const STRIP_PAD: u32 = 12;
/// Tallest panel of a strip, in pixels.
const STRIP_H: u32 = 384;
/// Panels smaller than this are enlarged (nearest-neighbour).
const STRIP_MIN: u32 = 160;
/// Widest whole-frame panel, in pixels (the fallback when there is no hotspot).
/// Height of the label bar.
const BAR: u32 = 18;

fn stretch_gain(a: &RgbImage, b: &RgbImage, r: [u32; 4]) -> f32 {
    let mut max = 0u8;
    for img in [a, b] {
        for y in r[1]..r[1] + r[3] {
            for x in r[0]..r[0] + r[2] {
                let p = img.get_pixel(x, y).0;
                max = max.max(p[0]).max(p[1]).max(p[2]);
            }
        }
    }
    if max == 0 || max >= 200 {
        1.0
    } else {
        255.0 / f32::from(max)
    }
}

fn panel(img: &RgbImage, r: [u32; 4], gain: f32) -> RgbImage {
    let mut c = imageops::crop_imm(img, r[0], r[1], r[2], r[3]).to_image();
    if gain != 1.0 {
        for p in c.pixels_mut() {
            for v in &mut p.0 {
                *v = (f32::from(*v) * gain).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    let big = c.width().max(c.height());
    if big < STRIP_MIN {
        let s = STRIP_MIN.div_ceil(big);
        imageops::resize(
            &c,
            c.width() * s,
            c.height() * s,
            imageops::FilterType::Nearest,
        )
    } else if c.height() > STRIP_H || c.width() > 768 {
        let f = (f64::from(STRIP_H) / f64::from(c.height())).min(768.0 / f64::from(c.width()));
        let w = ((f64::from(c.width()) * f).round() as u32).max(1);
        let h = ((f64::from(c.height()) * f).round() as u32).max(1);
        imageops::resize(&c, w, h, imageops::FilterType::Triangle)
    } else {
        c
    }
}

fn join_two(first: &RgbImage, second: &RgbImage) -> RgbImage {
    let ph = first.height().max(second.height());
    let mut out = RgbImage::from_pixel(first.width() + second.width() + 1, BAR + ph, Rgb([24; 3]));
    imageops::replace(&mut out, first, 0, i64::from(BAR));
    imageops::replace(
        &mut out,
        second,
        i64::from(first.width()) + 1,
        i64::from(BAR),
    );
    crate::explain::fill(&mut out, first.width(), 0, 1, BAR + ph, Rgb([200; 3]));
    crate::explain::draw_text(&mut out, 4, 2, "1", 2, Rgb([235; 3]));
    crate::explain::draw_text(&mut out, first.width() + 5, 2, "2", 2, Rgb([235; 3]));
    out
}

/// The padded, clamped crop rectangle of a hotspot.
fn crop_rect(rect: [u32; 4], w: u32, h: u32) -> [u32; 4] {
    let x0 = rect[0].saturating_sub(STRIP_PAD).min(w.saturating_sub(1));
    let y0 = rect[1].saturating_sub(STRIP_PAD).min(h.saturating_sub(1));
    let x1 = rect[0]
        .saturating_add(rect[2])
        .saturating_add(STRIP_PAD)
        .min(w);
    let y1 = rect[1]
        .saturating_add(rect[3])
        .saturating_add(STRIP_PAD)
        .min(h);
    [x0, y0, (x1 - x0).max(1), (y1 - y0).max(1)]
}

/// Blind strips for a vision judge: one per hotspot (at most `top`), the two
/// crops side by side, contrast-stretched when dark, labelled `1` and `2`
/// only. `swap` puts the second image first. Without hotspots there are no
/// strips; full frames are never attached as a fallback.
pub fn strips(
    first: &RgbImage,
    second: &RgbImage,
    hotspots: &[Hotspot],
    top: usize,
    swap: bool,
) -> Vec<RgbImage> {
    let (a, b) = if swap {
        (second, first)
    } else {
        (first, second)
    };
    let (w, h) = first.dimensions();
    if (w, h) != second.dimensions() || w == 0 || h == 0 {
        return Vec::new();
    }
    let rects: Vec<[u32; 4]> = hotspots
        .iter()
        .take(top)
        .map(|hs| crop_rect(hs.rect_px, w, h))
        .collect();
    rects
        .into_iter()
        .map(|r| {
            let gain = stretch_gain(a, b, r);
            let (pa, pb) = (panel(a, r, gain), panel(b, r, gain));
            join_two(&pa, &pb)
        })
        .collect()
}

/// Encodes an image as PNG bytes.
pub fn png_bytes(img: &RgbImage) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    let dynimg = image::DynamicImage::ImageRgb8(img.clone());
    match dynimg.write_to(&mut out, image::ImageFormat::Png) {
        Ok(()) => out.into_inner(),
        Err(_) => Vec::new(),
    }
}
