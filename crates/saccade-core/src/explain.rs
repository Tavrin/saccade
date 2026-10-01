//! The explain pack: crops and data that let an agent, or a vision-model
//! judge, see *what* changed at each hotspot without opening the HTML report.
//!
//! [`explain`] reads a report directory and writes
//!
//! ```text
//! explain/
//!   explain.json            every file below with its hotspot data
//!   explain.md              token-lean summary
//!   thumbs/<name>.png       whole-frame strip, hotspot boxes drawn
//!   hotspots/<name>.d/hN.png  [baseline | capture | heatmap] crop strip
//! ```
//!
//! With `blind` each strip is `[A | B]` in a seeded random order and the
//! heatmap is left out, because it shows which side is the reference. The key
//! (which side was A or B) is written only to [`ExplainOptions::key_out`],
//! which must be outside the pack so the pack can be handed to a judge as is;
//! the pack itself then names neither the report nor the sides.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use image::{Rgb, RgbImage, imageops};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::report::{Hotspot, Labels, Metric, Report, Status};
use crate::run::io_err;
use crate::view::{MAX_SEED, is_safe_name, shuffled_order};

/// Schema identifier written to [`ExplainPack::schema`].
pub const EXPLAIN_SCHEMA: &str = "saccade-explain.v1";

/// Schema identifier of the blind key.
pub const EXPLAIN_BLIND_KEY_SCHEMA: &str = "saccade-explain-blind-key.v1";

/// File name of the pack's data file.
pub const EXPLAIN_FILE: &str = "explain.json";

/// File name of the pack's Markdown summary.
pub const EXPLAIN_MD_FILE: &str = "explain.md";

/// Suggested file name of the blind key (it is written to `key_out`).
pub const EXPLAIN_BLIND_KEY_FILE: &str = "blind-key.json";

/// Smallest side, in pixels, a crop is upscaled to (nearest neighbour).
const MIN_PANEL: u32 = 256;

/// Largest side a crop panel is downscaled to.
const MAX_PANEL: u32 = 768;

/// Largest width of one panel of the whole-frame thumbnail strip.
const THUMB_PANEL: u32 = 480;

/// Widest strip written: wider ones are scaled down so a vision model or a
/// chat transcript does not choke on them.
pub const MAX_STRIP: u32 = 1536;

/// Height of the label bar above each strip.
const BAR_H: u32 = 18;

/// Options for [`explain`].
#[derive(Debug, Clone)]
pub struct ExplainOptions {
    /// Hotspots per entry (the largest first).
    pub top: usize,
    /// Pixels of context around each hotspot box.
    pub pad: u32,
    /// Contrast-stretch dark crops (same gain on both images).
    pub stretch: bool,
    /// Shuffle which side is A or B and leave out the heatmap.
    pub blind: bool,
    /// Blind shuffle seed; `None` derives one from the clock.
    pub seed: Option<u64>,
    /// Entry names to explain; empty means every failing entry.
    pub entries: Vec<String>,
    /// Where the blind key goes. Required with `blind`, and must lie outside
    /// the pack directory; ignored otherwise.
    pub key_out: Option<PathBuf>,
    /// Hotspots carrying less than this share of the total error (`0..=1`)
    /// are left out.
    pub hotspot_min_share: f64,
    /// Opt in to recording absolute paths in the pack.
    pub record_absolute_paths: bool,
}

impl Default for ExplainOptions {
    fn default() -> Self {
        Self {
            top: 3,
            pad: 16,
            stretch: false,
            blind: false,
            seed: None,
            entries: Vec::new(),
            key_out: None,
            hotspot_min_share: crate::hotspots::DEFAULT_HOTSPOT_MIN_SHARE,
            record_absolute_paths: false,
        }
    }
}

/// The settings an explain pack was made with.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExplainSettings {
    /// Hotspots per entry.
    pub top: usize,
    /// Context padding in pixels.
    pub pad: u32,
    /// Whether dark crops were contrast-stretched.
    pub stretch: bool,
}

/// One hotspot's strip.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExplainHotspot {
    /// 1-based index; the strip is `hotspots/<name>.d/h<index>.png`.
    pub index: u32,
    /// The hotspot's measurements, from the report.
    pub hotspot: Hotspot,
    /// Strip PNG, relative to the pack directory.
    pub strip: String,
    /// The padded crop that was cut from the frame, `[x, y, w, h]` in pixels.
    pub crop_rect_px: [u32; 4],
    /// Factor the crop was resized by (above 1 is a nearest-neighbour upscale).
    pub scale: f64,
    /// Gain applied to both images when `stretch` was on, else 1.
    pub gain: f64,
    /// Panel labels left to right: the side labels and `heatmap`, or `A`, `B`
    /// in a blind pack.
    pub panels: Vec<String>,
}

/// One explained entry.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExplainEntry {
    /// Image name.
    pub name: String,
    /// Status from the report.
    pub status: Status,
    /// Deciding metric.
    pub metric_used: Metric,
    /// Threshold the value was held against.
    pub threshold: f64,
    /// Deciding value.
    pub value: Option<f64>,
    /// Whole-frame strip with the hotspot boxes drawn, relative to the pack
    /// directory; `None` when an image could not be read.
    pub thumbnail: Option<String>,
    /// Why there is nothing to show, when that is the case.
    pub note: Option<String>,
    /// Kind of change from the report's diagnostics; `None` in a blind pack
    /// (it would name the sides) or when diagnostics did not run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<crate::diagnostics::ChangeClass>,
    /// The diagnostics description; `None` as `class` is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Strips of the top hotspots.
    pub hotspots: Vec<ExplainHotspot>,
}

/// The `explain.json` file.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExplainPack {
    /// Always [`EXPLAIN_SCHEMA`].
    pub schema: String,
    /// Report JSON path relative to the pack, absolute only by opt-in; `None` in a
    /// blind pack, whose location could name the sides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<String>,
    /// Pack directory relative to the working directory, absolute only by opt-in; strip paths are
    /// relative to it.
    pub dir: String,
    /// Whether sides are shuffled and anonymous (`A`/`B`).
    pub blind: bool,
    /// Side names; `None` in a blind pack, which must not name the sides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<Labels>,
    /// Settings.
    pub settings: ExplainSettings,
    /// Entries, worst first.
    pub entries: Vec<ExplainEntry>,
}

/// One blind choice made for the key.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlindItem {
    /// Image name.
    pub entry: String,
    /// Hotspot index, or `null` for the whole-frame strip.
    pub hotspot: Option<u32>,
    /// True label of the side shown as `A`.
    pub a: String,
    /// True label of the side shown as `B`.
    pub b: String,
}

/// The `blind-key.json` file of a blind pack. Keep it from the judge.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExplainBlindKey {
    /// Always [`EXPLAIN_BLIND_KEY_SCHEMA`].
    pub schema: String,
    /// The shuffle seed.
    pub seed: u64,
    /// One item per strip.
    pub items: Vec<BlindItem>,
}

// 5x7 glyphs, one byte per row, bit 4 is the leftmost column.
const GLYPHS: &[(char, [u8; 7])] = &[
    ('A', [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11]),
    ('B', [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E]),
    ('C', [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E]),
    ('D', [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E]),
    ('E', [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F]),
    ('F', [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10]),
    ('G', [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F]),
    ('H', [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11]),
    ('I', [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E]),
    ('J', [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C]),
    ('K', [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11]),
    ('L', [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F]),
    ('M', [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11]),
    ('N', [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11]),
    ('O', [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E]),
    ('P', [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10]),
    ('Q', [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D]),
    ('R', [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11]),
    ('S', [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E]),
    ('T', [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04]),
    ('U', [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E]),
    ('V', [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04]),
    ('W', [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A]),
    ('X', [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11]),
    ('Y', [0x11, 0x11, 0x11, 0x0A, 0x04, 0x04, 0x04]),
    ('Z', [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F]),
    ('0', [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E]),
    ('1', [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E]),
    ('2', [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F]),
    ('3', [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E]),
    ('4', [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02]),
    ('5', [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E]),
    ('6', [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E]),
    ('7', [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08]),
    ('8', [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E]),
    ('9', [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C]),
    ('-', [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00]),
    ('_', [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F]),
    ('.', [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C]),
    (':', [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00]),
    ('/', [0x01, 0x01, 0x02, 0x04, 0x08, 0x10, 0x10]),
    ('|', [0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04]),
    ('#', [0x0A, 0x0A, 0x1F, 0x0A, 0x1F, 0x0A, 0x0A]),
];

/// Draws `text` (letters, digits, `-_.`; anything else is blank) with the
/// built-in 5x7 font at integer `scale`, top-left at `(x, y)`. Stops at the
/// right edge of the image.
pub(crate) fn draw_text(
    img: &mut RgbImage,
    x: u32,
    y: u32,
    text: &str,
    scale: u32,
    color: Rgb<u8>,
) {
    let mut cx = x;
    for ch in text.chars() {
        let up = ch.to_ascii_uppercase();
        let rows = GLYPHS.iter().find(|(c, _)| *c == up).map(|(_, r)| r);
        if cx + 5 * scale > img.width() {
            return;
        }
        if let Some(rows) = rows {
            for (ry, bits) in rows.iter().enumerate() {
                for rx in 0..5u32 {
                    if bits >> (4 - rx) & 1 == 1 {
                        fill(
                            img,
                            cx + rx * scale,
                            y + ry as u32 * scale,
                            scale,
                            scale,
                            color,
                        );
                    }
                }
            }
        }
        cx += 6 * scale;
    }
}

pub(crate) fn fill(img: &mut RgbImage, x: u32, y: u32, w: u32, h: u32, color: Rgb<u8>) {
    for yy in y..(y + h).min(img.height()) {
        for xx in x..(x + w).min(img.width()) {
            img.put_pixel(xx, yy, color);
        }
    }
}

pub(crate) fn outline(img: &mut RgbImage, rect: [u32; 4], thickness: u32, color: Rgb<u8>) {
    let [x, y, w, h] = rect;
    let t = thickness.min(w).min(h).max(1);
    fill(img, x, y, w, t, color);
    fill(img, x, (y + h).saturating_sub(t), w, t, color);
    fill(img, x, y, t, h, color);
    fill(img, (x + w).saturating_sub(t), y, t, h, color);
}

/// Joins panels left to right under a label bar, with a 1 px separator. A
/// strip wider than [`MAX_STRIP`] is built from proportionally smaller panels;
/// the second value is the factor they were shrunk by (1 when untouched).
fn compose(panels: &[(String, RgbImage)]) -> (RgbImage, f64) {
    let seps = panels.len().saturating_sub(1) as u32;
    let sum: u32 = panels.iter().map(|(_, p)| p.width()).sum();
    if sum + seps > MAX_STRIP {
        let f = f64::from(MAX_STRIP - seps) / f64::from(sum);
        let shrunk: Vec<(String, RgbImage)> = panels
            .iter()
            .map(|(l, p)| {
                let w = ((f64::from(p.width()) * f).floor() as u32).max(1);
                let h = ((f64::from(p.height()) * f).round() as u32).max(1);
                (
                    l.clone(),
                    imageops::resize(p, w, h, imageops::FilterType::Triangle),
                )
            })
            .collect();
        return (join(&shrunk), f);
    }
    (join(panels), 1.0)
}

fn join(panels: &[(String, RgbImage)]) -> RgbImage {
    let ph = panels.iter().map(|(_, p)| p.height()).max().unwrap_or(1);
    let total: u32 =
        panels.iter().map(|(_, p)| p.width()).sum::<u32>() + panels.len().saturating_sub(1) as u32;
    let mut out = RgbImage::from_pixel(total.max(1), BAR_H + ph, Rgb([24, 24, 24]));
    let mut x = 0;
    for (i, (label, p)) in panels.iter().enumerate() {
        if i > 0 {
            fill(&mut out, x, 0, 1, BAR_H + ph, Rgb([200, 200, 200]));
            x += 1;
        }
        imageops::replace(&mut out, p, i64::from(x), i64::from(BAR_H));
        let mut bar = RgbImage::from_pixel(p.width(), BAR_H, Rgb([24, 24, 24]));
        draw_text(&mut bar, 3, 2, label, 2, Rgb([235, 235, 235]));
        imageops::replace(&mut out, &bar, i64::from(x), 0);
        x += p.width();
    }
    out
}

fn load_rgb(report_dir: &Path, rel: &str) -> Result<RgbImage> {
    if !is_safe_name(rel) {
        return Err(Error::Config(format!(
            "unsafe image path {rel:?} in the report"
        )));
    }
    let path = report_dir.join(rel);
    let rgba = image::open(&path)
        .map_err(|source| Error::Decode {
            path: path.clone(),
            source,
        })?
        .to_rgba8();
    Ok(crate::compare::flatten_over(&rgba, 0))
}

fn padded(rect: [u32; 4], pad: u32, width: u32, height: u32) -> [u32; 4] {
    let x0 = rect[0].saturating_sub(pad);
    let y0 = rect[1].saturating_sub(pad);
    let x1 = (rect[0] + rect[2] + pad).min(width);
    let y1 = (rect[1] + rect[3] + pad).min(height);
    [
        x0,
        y0,
        x1.saturating_sub(x0).max(1),
        y1.saturating_sub(y0).max(1),
    ]
}

/// Crops `rect`, optionally applies `gain`, and resizes: nearest-neighbour
/// up to `MIN_PANEL`, smooth down to `MAX_PANEL`. Returns the panel and the
/// scale factor.
fn crop_panel(img: &RgbImage, rect: [u32; 4], gain: f32) -> (RgbImage, f64) {
    let mut c = imageops::crop_imm(img, rect[0], rect[1], rect[2], rect[3]).to_image();
    if gain != 1.0 {
        for p in c.pixels_mut() {
            for v in &mut p.0 {
                *v = (f32::from(*v) * gain).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    let big = rect[2].max(rect[3]);
    if big < MIN_PANEL {
        let s = MIN_PANEL.div_ceil(big);
        let out = imageops::resize(
            &c,
            c.width() * s,
            c.height() * s,
            imageops::FilterType::Nearest,
        );
        (out, f64::from(s))
    } else if big > MAX_PANEL {
        let f = f64::from(MAX_PANEL) / f64::from(big);
        let (w, h) = (
            ((f64::from(c.width()) * f).round() as u32).max(1),
            ((f64::from(c.height()) * f).round() as u32).max(1),
        );
        (
            imageops::resize(&c, w, h, imageops::FilterType::Triangle),
            f,
        )
    } else {
        (c, 1.0)
    }
}

/// Gain that brings the brightest sample of both crops to 255, when they are
/// dark (brightest below 200); 1 otherwise.
fn stretch_gain(a: &RgbImage, b: &RgbImage, rect: [u32; 4]) -> f32 {
    let mut max = 0u8;
    for img in [a, b] {
        for y in rect[1]..rect[1] + rect[3] {
            for x in rect[0]..rect[0] + rect[2] {
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

fn save(img: &RgbImage, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(io_err(format!("creating {}", parent.display())))?;
    }
    img.save(path).map_err(|source| Error::Encode {
        path: path.to_path_buf(),
        source,
    })
}

fn clock_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos() as u64)
        & MAX_SEED
}

/// Pairs a `(label, image)` list in blind order: `A` and `B` in the order the
/// seed picks for `key`. Returns the true labels of A and B.
fn blind_pair(
    seed: u64,
    key: &str,
    labels: &Labels,
    base: RgbImage,
    cap: RgbImage,
) -> (Vec<(String, RgbImage)>, [String; 2]) {
    let sides = [
        (labels.baseline.clone(), base),
        (labels.capture.clone(), cap),
    ];
    let order = shuffled_order(seed, key, 2);
    let mut it = order.into_iter().map(|i| sides[i].clone());
    let first = it.next();
    let second = it.next();
    match (first, second) {
        (Some((la, ia)), Some((lb, ib))) => {
            (vec![("A".to_string(), ia), ("B".to_string(), ib)], [la, lb])
        }
        _ => (Vec::new(), [String::new(), String::new()]),
    }
}

/// Removes what an earlier pack left in `out_dir` (and only that).
fn clear_previous(out_dir: &Path) -> Result<()> {
    for leaf in [
        EXPLAIN_FILE,
        EXPLAIN_MD_FILE,
        EXPLAIN_BLIND_KEY_FILE,
        "thumbs",
        "hotspots",
    ] {
        let path = out_dir.join(leaf);
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        let removed = if meta.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        removed.map_err(io_err(format!("removing stale {}", path.display())))?;
    }
    Ok(())
}

/// Reads a report JSON (which must sit in its report directory, next to
/// `images/`) and writes the explain pack into `out_dir`. Returns the pack as
/// written to `explain.json`.
///
/// Entries are the failing ones, worst value first, or the names in
/// `opts.entries`. A failing entry with no hotspot still gets its whole-frame
/// strip. Unreadable images make that entry carry a `note` instead of failing
/// the pack.
pub fn explain(report_json: &Path, out_dir: &Path, opts: &ExplainOptions) -> Result<ExplainPack> {
    let key_out = if opts.blind {
        let key_out = opts.key_out.as_deref().ok_or_else(|| {
            Error::Config("a blind pack needs `key_out`, a path outside the pack".into())
        })?;
        check_key_out(out_dir, key_out)?;
        Some(key_out)
    } else {
        None
    };
    let text = std::fs::read_to_string(report_json)
        .map_err(io_err(format!("reading report {}", report_json.display())))?;
    let report: Report = serde_json::from_str(&text)?;
    let report_dir = report_json
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);

    let mut chosen: Vec<&crate::report::Entry> = if opts.entries.is_empty() {
        report
            .entries
            .iter()
            .filter(|e| e.status == Status::Fail)
            .collect()
    } else {
        let mut v = Vec::new();
        for name in &opts.entries {
            let e = report
                .entries
                .iter()
                .find(|e| &e.name == name)
                .ok_or_else(|| Error::Config(format!("the report has no entry {name:?}")))?;
            v.push(e);
        }
        v
    };
    chosen.sort_by(|a, b| {
        b.value
            .unwrap_or(f64::NEG_INFINITY)
            .total_cmp(&a.value.unwrap_or(f64::NEG_INFINITY))
            .then_with(|| a.name.cmp(&b.name))
    });

    crate::run::guard_output_dir(out_dir, &[], &[EXPLAIN_FILE, crate::run::RUN_SENTINEL])?;
    std::fs::create_dir_all(out_dir).map_err(io_err(format!("creating {}", out_dir.display())))?;
    clear_previous(out_dir)?;
    let sentinel = out_dir.join(crate::run::RUN_SENTINEL);
    std::fs::write(&sentinel, b"incomplete saccade run\n")
        .map_err(io_err(format!("writing {}", sentinel.display())))?;

    let seed = opts.seed.unwrap_or_else(clock_seed) & MAX_SEED;
    let labels = &report.config.labels;
    let mut key_items = Vec::new();
    let mut entries = Vec::new();
    for e in chosen {
        if !is_safe_name(&e.name) {
            return Err(Error::Config(format!("unsafe entry name {:?}", e.name)));
        }
        let mut ee = ExplainEntry {
            name: e.name.clone(),
            status: e.status,
            metric_used: e.metric_used,
            threshold: e.threshold,
            value: e.value,
            thumbnail: None,
            note: None,
            class: e
                .diagnostics
                .as_ref()
                .filter(|_| !opts.blind)
                .map(|d| d.class),
            description: e
                .diagnostics
                .as_ref()
                .filter(|_| !opts.blind)
                .map(|d| d.description.clone()),
            hotspots: Vec::new(),
        };
        let (Some(bp), Some(cp)) = (&e.paths.baseline, &e.paths.capture) else {
            ee.note = Some("an image is missing; nothing to compare".into());
            entries.push(ee);
            continue;
        };
        let loaded =
            load_rgb(&report_dir, bp).and_then(|b| load_rgb(&report_dir, cp).map(|c| (b, c)));
        let (base, cap) = match loaded {
            Ok(v) => v,
            Err(err) => {
                ee.note = Some(format!("cannot read images: {err}"));
                entries.push(ee);
                continue;
            }
        };
        let heat = if opts.blind {
            None
        } else {
            e.paths
                .heatmap
                .as_deref()
                .and_then(|p| load_rgb(&report_dir, p).ok())
        };
        let spots: Vec<&Hotspot> = e
            .hotspots
            .iter()
            .filter(|h| h.share_of_total_error >= opts.hotspot_min_share)
            .take(opts.top)
            .collect();

        // Whole-frame strip with the boxes.
        let mut tb = base.clone();
        let mut tc = cap.clone();
        let mut th = heat.clone();
        let panel_scale = (f64::from(THUMB_PANEL) / f64::from(tb.width())).min(1.0);
        let shrink = |img: &RgbImage| -> RgbImage {
            if panel_scale >= 1.0 {
                img.clone()
            } else {
                imageops::resize(
                    img,
                    ((f64::from(img.width()) * panel_scale).round() as u32).max(1),
                    ((f64::from(img.height()) * panel_scale).round() as u32).max(1),
                    imageops::FilterType::Triangle,
                )
            }
        };
        tb = shrink(&tb);
        tc = shrink(&tc);
        th = th.as_ref().map(shrink);
        for (i, h) in spots.iter().enumerate() {
            let r = h.rect_px.map(f64::from);
            let rs = [
                (r[0] * panel_scale).floor() as u32,
                (r[1] * panel_scale).floor() as u32,
                ((r[2] * panel_scale).ceil() as u32).max(2),
                ((r[3] * panel_scale).ceil() as u32).max(2),
            ];
            for img in [Some(&mut tb), Some(&mut tc), th.as_mut()]
                .into_iter()
                .flatten()
            {
                outline(img, rs, 1, Rgb([255, 220, 0]));
                let ty = rs[1].saturating_sub(9).min(img.height().saturating_sub(8));
                draw_text(
                    img,
                    rs[0] + 1,
                    ty,
                    &(i + 1).to_string(),
                    1,
                    Rgb([255, 220, 0]),
                );
            }
        }
        let thumb_panels: Vec<(String, RgbImage)> = if opts.blind {
            let (p, ab) = blind_pair(seed, &format!("{}#thumb", e.name), labels, tb, tc);
            key_items.push(BlindItem {
                entry: e.name.clone(),
                hotspot: None,
                a: ab[0].clone(),
                b: ab[1].clone(),
            });
            p
        } else {
            let mut v = vec![(labels.baseline.clone(), tb), (labels.capture.clone(), tc)];
            if let Some(h) = th {
                v.push(("heatmap".to_string(), h));
            }
            v
        };
        let thumb_rel = if e.name.to_ascii_lowercase().ends_with(".png") {
            format!("thumbs/{}", e.name)
        } else {
            format!("thumbs/{}.png", e.name)
        };
        save(&compose(&thumb_panels).0, &out_dir.join(&thumb_rel))?;
        ee.thumbnail = Some(thumb_rel);

        if spots.is_empty() {
            ee.note = Some(
                "no hotspot above the threshold: the difference is diffuse or below it".into(),
            );
        }
        for (i, h) in spots.iter().enumerate() {
            let rect = padded(h.rect_px, opts.pad, base.width(), base.height());
            let gain = if opts.stretch {
                stretch_gain(&base, &cap, rect)
            } else {
                1.0
            };
            let (pb, scale) = crop_panel(&base, rect, gain);
            let (pc, _) = crop_panel(&cap, rect, gain);
            let index = i as u32 + 1;
            let panels: Vec<(String, RgbImage)> = if opts.blind {
                let (p, ab) = blind_pair(seed, &format!("{}#{index}", e.name), labels, pb, pc);
                key_items.push(BlindItem {
                    entry: e.name.clone(),
                    hotspot: Some(index),
                    a: ab[0].clone(),
                    b: ab[1].clone(),
                });
                p
            } else {
                let mut v = vec![(labels.baseline.clone(), pb), (labels.capture.clone(), pc)];
                if let Some(hm) = &heat {
                    v.push(("heatmap".to_string(), crop_panel(hm, rect, 1.0).0));
                }
                v
            };
            let rel = format!("hotspots/{}.d/h{index}.png", e.name);
            let (strip, shrink) = compose(&panels);
            save(&strip, &out_dir.join(&rel))?;
            ee.hotspots.push(ExplainHotspot {
                index,
                hotspot: (*h).clone(),
                strip: rel,
                crop_rect_px: rect,
                scale: scale * shrink,
                gain: f64::from(gain),
                panels: panels.iter().map(|(l, _)| l.clone()).collect(),
            });
        }
        entries.push(ee);
    }

    let pack = ExplainPack {
        schema: EXPLAIN_SCHEMA.to_string(),
        report: (!opts.blind)
            .then(|| crate::paths::record(report_json, out_dir, opts.record_absolute_paths)),
        dir: crate::paths::cwd(out_dir, opts.record_absolute_paths),
        blind: opts.blind,
        labels: (!opts.blind).then(|| labels.clone()),
        settings: ExplainSettings {
            top: opts.top,
            pad: opts.pad,
            stretch: opts.stretch,
        },
        entries,
    };
    let write = |name: &str, body: String| -> Result<()> {
        let p = out_dir.join(name);
        std::fs::write(&p, body).map_err(io_err(format!("writing {}", p.display())))
    };
    write(
        EXPLAIN_FILE,
        format!("{}\n", serde_json::to_string_pretty(&pack)?),
    )?;
    // `explain.json` now marks the directory as the pack's own.
    std::fs::remove_file(&sentinel).map_err(io_err(format!("removing {}", sentinel.display())))?;
    write(EXPLAIN_MD_FILE, markdown(&pack, &report))?;
    if let Some(key_out) = key_out {
        let key = ExplainBlindKey {
            schema: EXPLAIN_BLIND_KEY_SCHEMA.to_string(),
            seed,
            items: key_items,
        };
        if let Some(parent) = key_out.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)
                .map_err(io_err(format!("creating {}", parent.display())))?;
        }
        std::fs::write(
            key_out,
            format!("{}\n", serde_json::to_string_pretty(&key)?),
        )
        .map_err(io_err(format!("writing {}", key_out.display())))?;
    }
    Ok(pack)
}

/// `p` made absolute against the working directory, with the longest existing
/// prefix canonicalised (symlinks resolved) before folding the missing suffix, so a
/// path that does not exist yet can still be compared with one that does. The
/// result is what an operation on `p` would touch, which makes it safe to test
/// with `starts_with`.
pub fn absolute(p: &Path) -> PathBuf {
    crate::run::normalise_path(p)
}

/// Refuses a blind key destination inside the pack directory: the pack is
/// what the judge sees, so the key must live elsewhere.
pub fn check_key_out(out_dir: &Path, key_out: &Path) -> Result<()> {
    if absolute(key_out).starts_with(absolute(out_dir)) {
        return Err(Error::Config(format!(
            "the blind key {} must be outside the pack directory {}",
            key_out.display(),
            out_dir.display()
        )));
    }
    Ok(())
}

/// `1 entry`, `2 entries`.
pub fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Token-lean Markdown of a pack, for pasting into an agent's context.
fn markdown(pack: &ExplainPack, report: &Report) -> String {
    let mut out = String::new();
    let t = &report.totals;
    let sides = match &pack.labels {
        Some(l) => format!("{} vs {}", l.baseline, l.capture),
        None => "A vs B (blind)".to_string(),
    };
    out.push_str(&format!(
        "# saccade explain: {} ({sides}); {} fail, {} pass of {}\n",
        plural(pack.entries.len(), "entry", "entries"),
        t.fail,
        t.pass,
        t.total
    ));
    if !pack.blind
        && let Some(v) = &report.combined_verdict
    {
        out.push_str(&format!("{}\n", crate::perf::clean(v)));
    }
    if pack.blind {
        out.push_str("Strips are [A | B]; which side is the reference is hidden.\n");
    } else {
        out.push_str("Strips are [baseline | capture | heatmap]; error scale 0 (none) to 1.\n");
    }
    for e in &pack.entries {
        out.push('\n');
        let value = e.value.map_or("-".to_string(), |v| format!("{v:.4}"));
        out.push_str(&format!(
            "## {} {:?} {:?}={value} (limit {})\n",
            e.name, e.status, e.metric_used, e.threshold
        ));
        if let Some(t) = &e.thumbnail {
            out.push_str(&format!("frame: {t}\n"));
        }
        if let (Some(c), Some(d)) = (e.class, &e.description) {
            out.push_str(&format!("{}: {d}\n", c.as_str()));
        }
        if let Some(n) = &e.note {
            out.push_str(&format!("note: {n}\n"));
        }
        for h in &e.hotspots {
            let s = &h.hotspot;
            out.push_str(&format!(
                "{}. {} {}x{} at ({},{}) {:.0}% of error, mean {:.2} max {:.2}, hot px {} ({:.2}% of frame), box {:.1}% of frame: {}\n",
                h.index,
                s.position,
                s.rect_px[2],
                s.rect_px[3],
                s.rect_px[0],
                s.rect_px[1],
                s.share_of_total_error * 100.0,
                s.mean_flip,
                s.max_flip,
                s.area_px,
                s.area_frac * 100.0,
                s.rect_frac[2] * s.rect_frac[3] * 100.0,
                h.strip
            ));
        }
    }
    out
}
