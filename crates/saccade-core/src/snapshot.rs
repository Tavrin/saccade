//! Renders a view state to PNG on the server, with no browser: what an agent
//! sees when it opens a report or a view with a `#...` state in the URL.
//!
//! [`ViewState::parse`] reads the hash grammar shared with the pages (see the
//! README): `set` or `entry`, `layout`, `split`, `vertical`, `zoom`, `at`,
//! `heat`, `channel`, `ev`, `roi`, `hotspot`. Unknown or invalid keys are
//! ignored. [`snapshot`] draws the state from a report JSON or a view
//! directory.
//!
//! Layouts: `side` is every image in a row under a label; `swipe` shows the
//! first image left of (or above) a divider and the second on the other side,
//! with an optional heatmap overlay on the second; `heatmap` is the second
//! image under its FLIP heatmap; `flicker` is one frame per image, written as
//! separate PNGs (an APNG encoder is not worth a dependency).
//!
//! Text uses the 5x7 bitmap font of [`crate::explain`]. A blind view keeps its
//! neutral labels and draws neither heatmaps nor hotspots.

use std::path::{Path, PathBuf};

use image::imageops::{FilterType, crop_imm, resize};
use image::{Rgb, RgbImage};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::explain::{draw_text, fill, outline};
use crate::report::{REPORT_FILE_NAME, Report, Status};

/// Width used when none is given.
pub const DEFAULT_WIDTH: u32 = 1600;

/// Widest snapshot [`snapshot`] draws.
pub const MAX_WIDTH: u32 = 4096;

const GAP: u32 = 6;
const BG: Rgb<u8> = Rgb([24, 24, 28]);
const FG: Rgb<u8> = Rgb([240, 240, 240]);
const HOT: Rgb<u8> = Rgb([255, 176, 0]);
const HOT_SELECTED: Rgb<u8> = Rgb([255, 64, 64]);
const ROI: Rgb<u8> = Rgb([0, 200, 255]);

/// How the images are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// Every image in a row.
    Side,
    /// First image on one side of a divider, second on the other.
    Swipe,
    /// One frame per image.
    Flicker,
    /// The second image under its FLIP heatmap.
    Heatmap,
}

/// Display channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    /// All three channels.
    Rgb,
    /// Red only, shown as grey.
    R,
    /// Green only, shown as grey.
    G,
    /// Blue only, shown as grey.
    B,
    /// Rec. 709 luma of the encoded values.
    Luma,
}

/// Zoom of the stage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Zoom {
    /// The whole image fits the stage.
    Fit,
    /// Output pixels per image pixel, with the stage `width` wide.
    Scale(f64),
}

/// The state a page's hash describes.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewState {
    /// Entry (report) or set (view) name; `set` and `entry` are synonyms.
    pub name: Option<String>,
    /// Layout; `None` picks `swipe` for two images and `side` otherwise.
    pub layout: Option<Layout>,
    /// Divider position, 0 to 1.
    pub split: f64,
    /// Whether the divider runs horizontally (the images stack).
    pub vertical: bool,
    /// Zoom.
    pub zoom: Zoom,
    /// Image-pixel centre of the zoom.
    pub at: Option<(f64, f64)>,
    /// Heatmap opacity, 0 to 1; 0 is off.
    pub heat: f64,
    /// Display channel.
    pub channel: Channel,
    /// Display exposure in stops.
    pub ev: f64,
    /// Region of interest `[x, y, w, h]` in image pixels.
    pub roi: Option<[u32; 4]>,
    /// The 1-based hotspot to highlight (and zoom to when no zoom is given).
    pub hotspot: Option<usize>,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            name: None,
            layout: None,
            split: 0.5,
            vertical: false,
            zoom: Zoom::Fit,
            at: None,
            heat: 0.0,
            channel: Channel::Rgb,
            ev: 0.0,
            roi: None,
            hotspot: None,
        }
    }
}

fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn floats<const N: usize>(v: &str) -> Option<[f64; N]> {
    let parts: Vec<f64> = v.split(',').filter_map(|p| p.trim().parse().ok()).collect();
    let arr: [f64; N] = parts.try_into().ok()?;
    arr.iter().all(|x| x.is_finite()).then_some(arr)
}

impl ViewState {
    /// Parses a hash string (`#a=1&b=2`, the leading `#` optional). Keys that
    /// are unknown or whose value is invalid are ignored.
    pub fn parse(hash: &str) -> Self {
        let mut st = Self::default();
        for pair in hash.trim_start_matches('#').split('&') {
            let Some((k, raw)) = pair.split_once('=') else {
                continue;
            };
            let Some(v) = percent_decode(raw) else {
                continue;
            };
            let v = v.as_str();
            match k {
                "set" | "entry" if !v.is_empty() => st.name = Some(v.to_owned()),
                "layout" => {
                    st.layout = match v {
                        "side" => Some(Layout::Side),
                        "swipe" => Some(Layout::Swipe),
                        "flicker" => Some(Layout::Flicker),
                        "heatmap" => Some(Layout::Heatmap),
                        _ => st.layout,
                    }
                }
                "split" => {
                    if let Ok(x) = v.parse::<f64>()
                        && x.is_finite()
                    {
                        st.split = x.clamp(0.0, 1.0);
                    }
                }
                "vertical" => match v {
                    "0" => st.vertical = false,
                    "1" => st.vertical = true,
                    _ => {}
                },
                "zoom" => match v {
                    "fit" => st.zoom = Zoom::Fit,
                    _ => {
                        if let Ok(z) = v.parse::<f64>()
                            && z.is_finite()
                            && z > 0.0
                        {
                            st.zoom = Zoom::Scale(z.min(64.0));
                        }
                    }
                },
                "at" => {
                    if let Some([x, y]) = floats::<2>(v) {
                        st.at = Some((x, y));
                    }
                }
                "heat" => {
                    if let Ok(x) = v.parse::<f64>()
                        && x.is_finite()
                    {
                        st.heat = x.clamp(0.0, 1.0);
                    }
                }
                "channel" => {
                    st.channel = match v {
                        "rgb" => Channel::Rgb,
                        "r" => Channel::R,
                        "g" => Channel::G,
                        "b" => Channel::B,
                        "luma" => Channel::Luma,
                        _ => st.channel,
                    }
                }
                "ev" => {
                    if let Ok(x) = v.parse::<f64>()
                        && x.is_finite()
                    {
                        st.ev = x.clamp(-16.0, 16.0);
                    }
                }
                "roi" => {
                    if let Some([x, y, w, h]) = floats::<4>(v)
                        && [x, y, w, h].iter().all(|n| *n >= 0.0 && n.fract() == 0.0)
                        && w > 0.0
                        && h > 0.0
                    {
                        st.roi = Some([x as u32, y as u32, w as u32, h as u32]);
                    }
                }
                "hotspot" => {
                    if let Ok(n) = v.parse::<usize>() {
                        st.hotspot = (n > 0).then_some(n);
                    }
                }
                _ => {}
            }
        }
        st
    }
}

/// One image of the entry being drawn.
#[derive(Debug, Clone)]
pub struct SnapPane {
    /// Label shown above or on the image.
    pub label: String,
    /// The image file; `None` when this side has no such image.
    pub path: Option<PathBuf>,
    /// FLIP heatmap of this image against the reference, if any.
    pub heat: Option<PathBuf>,
    /// Whether this is the FLIP reference side.
    pub reference: bool,
    /// Hotspot rectangles `[x, y, w, h]` in image pixels.
    pub hotspots: Vec<[u32; 4]>,
}

/// The images and metadata of one entry or set.
#[derive(Debug, Clone)]
pub struct SnapSource {
    /// Entry or set name.
    pub name: String,
    /// Blind views keep neutral labels and show no FLIP data.
    pub blind: bool,
    /// Images in display order.
    pub panes: Vec<SnapPane>,
}

fn cfg_err(msg: impl Into<String>) -> Error {
    Error::Config(msg.into())
}

/// Reads one entry of a report as a two-image source.
pub fn report_source(report_json: &Path, entry: Option<&str>) -> Result<SnapSource> {
    let text = std::fs::read_to_string(report_json).map_err(|source| Error::Io {
        context: format!("reading {}", report_json.display()),
        source,
    })?;
    let report: Report = serde_json::from_str(&text)?;
    let dir = report_json
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let e = match entry {
        Some(n) => report
            .entries
            .iter()
            .find(|e| e.name == n)
            .ok_or_else(|| cfg_err(format!("the report has no entry {n:?}")))?,
        None => report
            .entries
            .iter()
            .find(|e| e.status == Status::Fail)
            .or_else(|| report.entries.first())
            .ok_or_else(|| cfg_err("the report has no entries"))?,
    };
    let join = |p: &Option<String>| p.as_ref().map(|p| dir.join(p));
    let hotspots = if e.status == Status::Fail {
        e.hotspots.iter().map(|h| h.rect_px).collect()
    } else {
        Vec::new()
    };
    Ok(SnapSource {
        name: e.name.clone(),
        blind: false,
        panes: vec![
            SnapPane {
                label: report.config.labels.baseline.clone(),
                path: join(&e.paths.baseline),
                heat: None,
                reference: true,
                hotspots: Vec::new(),
            },
            SnapPane {
                label: report.config.labels.capture.clone(),
                path: join(&e.paths.capture),
                heat: join(&e.paths.heatmap),
                reference: false,
                hotspots,
            },
        ],
    })
}

/// The model embedded in a view directory's `index.html`, as JSON.
pub fn view_model(view_dir: &Path) -> Result<Value> {
    let index = view_dir.join("index.html");
    let html = std::fs::read_to_string(&index).map_err(|source| Error::Io {
        context: format!("reading {}", index.display()),
        source,
    })?;
    let tag = "id=\"saccade-data\">";
    let start = html
        .find(tag)
        .map(|i| i + tag.len())
        .ok_or_else(|| cfg_err(format!("{} is not a saccade view", index.display())))?;
    let end = html[start..].find("</script>").ok_or_else(|| {
        cfg_err(format!(
            "{} has an unterminated data block",
            index.display()
        ))
    })?;
    Ok(serde_json::from_str(&html[start..start + end])?)
}

/// Reads one set of a view directory.
pub fn view_source(view_dir: &Path, set: Option<&str>) -> Result<SnapSource> {
    let model = view_model(view_dir)?;
    let sets = model["sets"]
        .as_array()
        .ok_or_else(|| cfg_err("the view has no sets"))?;
    let s = match set {
        Some(n) => sets
            .iter()
            .find(|s| s["name"] == n)
            .ok_or_else(|| cfg_err(format!("the view has no set {n:?}")))?,
        None => sets
            .first()
            .ok_or_else(|| cfg_err("the view has no sets"))?,
    };
    let blind = model["blind"].as_bool().unwrap_or(false);
    let reference = model["reference"].as_u64().unwrap_or(0) as usize;
    let labels: Vec<String> = model["labels"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|l| l.as_str().unwrap_or("?").to_owned())
                .collect()
        })
        .unwrap_or_default();
    let order: Vec<usize> = s["order"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_u64().map(|n| n as usize))
                .collect()
        })
        .unwrap_or_default();
    let mut panes = Vec::new();
    for (pos, idx) in order.into_iter().enumerate() {
        let p = &s["panes"][idx];
        let path = |k: &str| p[k].as_str().map(|r| view_dir.join(r));
        let label = if blind {
            // The page letters a blind set's panes by display position.
            char::from(b'A' + u8::try_from(pos.min(25)).unwrap_or(0)).to_string()
        } else {
            labels
                .get(idx)
                .cloned()
                .unwrap_or_else(|| format!("P{}", idx + 1))
        };
        panes.push(SnapPane {
            label,
            path: path("path"),
            heat: if blind { None } else { path("heatmap") },
            reference: !blind && idx == reference,
            hotspots: if blind {
                Vec::new()
            } else {
                p["hotspots"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|h| {
                                let r = h["rect_px"].as_array()?;
                                let n =
                                    |i: usize| r.get(i).and_then(Value::as_u64).map(|v| v as u32);
                                Some([n(0)?, n(1)?, n(2)?, n(3)?])
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            },
        });
    }
    Ok(SnapSource {
        name: s["name"].as_str().unwrap_or_default().to_owned(),
        blind,
        panes,
    })
}

struct Loaded {
    label: String,
    img: Option<RgbImage>,
    heat: Option<RgbImage>,
    hotspots: Vec<[u32; 4]>,
}

fn load(path: &Path) -> Result<RgbImage> {
    image::open(path)
        .map(|i| i.to_rgb8())
        .map_err(|source| Error::Decode {
            path: path.to_path_buf(),
            source,
        })
}

/// The part of the reference image the stage shows, in image pixels.
#[derive(Clone, Copy)]
struct Region {
    x0: f64,
    y0: f64,
    w: f64,
    h: f64,
}

fn region(dims: (u32, u32), (ow, oh): (u32, u32), st: &ViewState, hot: Option<[u32; 4]>) -> Region {
    let (iw, ih) = (f64::from(dims.0), f64::from(dims.1));
    let fit = f64::from(ow) / iw;
    let mut scale = match st.zoom {
        Zoom::Fit => fit,
        Zoom::Scale(z) => z,
    };
    let mut centre = st.at;
    if let (Some(b), Zoom::Fit, None) = (hot, st.zoom, st.at) {
        let (bw, bh) = (f64::from(b[2].max(4)), f64::from(b[3].max(4)));
        scale = (0.8 * (f64::from(ow) / bw).min(f64::from(oh) / bh)).clamp(fit, 64.0);
    }
    if let (Some(b), None) = (hot, centre) {
        centre = Some((
            f64::from(b[0]) + f64::from(b[2]) / 2.0,
            f64::from(b[1]) + f64::from(b[3]) / 2.0,
        ));
    }
    scale = scale.max(fit);
    let (w, h) = (
        (f64::from(ow) / scale).min(iw),
        (f64::from(oh) / scale).min(ih),
    );
    let (cx, cy) = centre.unwrap_or((iw / 2.0, ih / 2.0));
    Region {
        x0: (cx - w / 2.0).clamp(0.0, iw - w),
        y0: (cy - h / 2.0).clamp(0.0, ih - h),
        w,
        h,
    }
}

/// Crops `img` to `reg` (given in reference pixels) and resizes it to `w x h`.
fn crop_scaled(img: &RgbImage, dims: (u32, u32), reg: Region, (w, h): (u32, u32)) -> RgbImage {
    let (fx, fy) = (
        f64::from(img.width()) / f64::from(dims.0),
        f64::from(img.height()) / f64::from(dims.1),
    );
    let x = ((reg.x0 * fx).round() as u32).min(img.width().saturating_sub(1));
    let y = ((reg.y0 * fy).round() as u32).min(img.height().saturating_sub(1));
    let cw = ((reg.w * fx).round() as u32).clamp(1, img.width() - x);
    let ch = ((reg.h * fy).round() as u32).clamp(1, img.height() - y);
    let crop = crop_imm(img, x, y, cw, ch).to_image();
    let filter = if f64::from(w) / reg.w >= 1.0 {
        FilterType::Nearest
    } else {
        FilterType::Triangle
    };
    resize(&crop, w, h, filter)
}

fn blend(dst: &mut RgbImage, top: &RgbImage, alpha: f64) {
    for (d, t) in dst.pixels_mut().zip(top.pixels()) {
        for c in 0..3 {
            d.0[c] = (f64::from(d.0[c]) * (1.0 - alpha) + f64::from(t.0[c]) * alpha).round() as u8;
        }
    }
}

fn display(img: &mut RgbImage, st: &ViewState) {
    let gain = 2f64.powf(st.ev);
    for p in img.pixels_mut() {
        let [r, g, b] = [f64::from(p.0[0]), f64::from(p.0[1]), f64::from(p.0[2])];
        let v = match st.channel {
            Channel::Rgb => [r, g, b],
            Channel::R => [r; 3],
            Channel::G => [g; 3],
            Channel::B => [b; 3],
            Channel::Luma => [0.2126 * r + 0.7152 * g + 0.0722 * b; 3],
        };
        for (dst, src) in p.0.iter_mut().zip(v) {
            *dst = (src * gain).round().clamp(0.0, 255.0) as u8;
        }
    }
}

fn text_scale(width: u32) -> u32 {
    (width / 800).clamp(1, 3)
}

fn chip(img: &mut RgbImage, x: u32, y: u32, text: &str) {
    let s = text_scale(img.width());
    let w = text.chars().count() as u32 * 6 * s + 4 * s;
    fill(img, x, y, w, 7 * s + 4 * s, BG);
    draw_text(img, x + 2 * s, y + 2 * s, text, s, FG);
}

/// Boxes drawn over a stage placed at `origin` with size `size`.
fn overlays(
    img: &mut RgbImage,
    origin: (u32, u32),
    size: (u32, u32),
    reg: Region,
    st: &ViewState,
    hotspots: &[[u32; 4]],
) {
    let to_out = |r: [u32; 4]| -> Option<[u32; 4]> {
        let (sx, sy) = (f64::from(size.0) / reg.w, f64::from(size.1) / reg.h);
        let x0 = ((f64::from(r[0]) - reg.x0) * sx).max(0.0);
        let y0 = ((f64::from(r[1]) - reg.y0) * sy).max(0.0);
        let x1 = ((f64::from(r[0] + r[2]) - reg.x0) * sx).min(f64::from(size.0));
        let y1 = ((f64::from(r[1] + r[3]) - reg.y0) * sy).min(f64::from(size.1));
        (x1 > x0 && y1 > y0).then(|| {
            [
                origin.0 + x0 as u32,
                origin.1 + y0 as u32,
                (x1 - x0).ceil() as u32,
                (y1 - y0).ceil() as u32,
            ]
        })
    };
    let t = text_scale(img.width());
    for (i, h) in hotspots.iter().enumerate() {
        let selected = st.hotspot == Some(i + 1);
        if let Some(r) = to_out(*h) {
            let c = if selected { HOT_SELECTED } else { HOT };
            outline(img, r, if selected { 3 } else { 2 }, c);
            let n = (i + 1).to_string();
            let w = n.len() as u32 * 6 * t + 3 * t;
            fill(img, r[0], r[1], w, 9 * t, c);
            draw_text(img, r[0] + t, r[1] + t, &n, t, Rgb([0, 0, 0]));
        }
    }
    if let Some(r) = st.roi.and_then(to_out) {
        outline(img, r, 2, ROI);
    }
}

/// Draws the divider line of a swipe at the split position.
fn divider(img: &mut RgbImage, st: &ViewState) {
    let (w, h) = img.dimensions();
    if st.vertical {
        let y = (st.split * f64::from(h)).round() as u32;
        fill(img, 0, y.saturating_sub(2), w, 5, BG);
        fill(img, 0, y.saturating_sub(1), w, 3, FG);
    } else {
        let x = (st.split * f64::from(w)).round() as u32;
        fill(img, x.saturating_sub(2), 0, 5, h, BG);
        fill(img, x.saturating_sub(1), 0, 3, h, FG);
    }
}

fn placeholder(w: u32, h: u32, text: &str) -> RgbImage {
    let mut img = RgbImage::from_pixel(w, h, Rgb([60, 60, 66]));
    chip(&mut img, 6, h / 2, text);
    img
}

/// Draws `source` in state `st`, `width` pixels wide. Returns one image, or
/// one per picture for the `flicker` layout.
pub fn render(source: &SnapSource, st: &ViewState, width: u32) -> Result<Vec<RgbImage>> {
    let width = width.clamp(64, MAX_WIDTH);
    let mut panes = Vec::new();
    for p in &source.panes {
        let img = p.path.as_deref().map(load).transpose()?;
        let heat = if source.blind {
            None
        } else {
            p.heat.as_deref().map(load).transpose()?
        };
        panes.push(Loaded {
            label: p.label.clone(),
            img,
            heat,
            hotspots: if source.blind {
                Vec::new()
            } else {
                p.hotspots.clone()
            },
        });
    }
    let dims = panes
        .iter()
        .find_map(|p| p.img.as_ref().map(RgbImage::dimensions))
        .ok_or_else(|| cfg_err(format!("{}: no image to draw", source.name)))?;
    let present: Vec<usize> = (0..panes.len())
        .filter(|i| panes[*i].img.is_some())
        .collect();
    let layout = match st.layout {
        Some(l) => l,
        None if present.len() == 2 => Layout::Swipe,
        None => Layout::Side,
    };
    // Swipe and heatmap need two pictures; fall back to a row otherwise.
    let layout = if matches!(layout, Layout::Swipe | Layout::Heatmap) && present.len() < 2 {
        Layout::Side
    } else {
        layout
    };
    let oh = ((f64::from(width) * f64::from(dims.1) / f64::from(dims.0)).round() as u32).max(1);
    // The hotspot a `hotspot=n` state zooms to: the n-th of the first pane that has any.
    let hot_box = st.hotspot.and_then(|n| {
        panes
            .iter()
            .find(|p| !p.hotspots.is_empty())
            .and_then(|p| p.hotspots.get(n - 1))
            .copied()
    });
    let reg = region(dims, (width, oh), st, hot_box);
    let stage = |i: usize, w: u32, h: u32, heat: f64| -> RgbImage {
        let Some(img) = &panes[i].img else {
            return placeholder(w, h, "MISSING");
        };
        let mut out = crop_scaled(img, dims, reg, (w, h));
        if heat > 0.0
            && let Some(hm) = &panes[i].heat
        {
            blend(&mut out, &crop_scaled(hm, dims, reg, (w, h)), heat);
        }
        out
    };
    let label_pad = 7 * text_scale(width) + 6 * text_scale(width);
    match layout {
        Layout::Side => {
            let n = u32::try_from(panes.len()).unwrap_or(1).max(1);
            let cw = ((width - GAP * (n - 1)) / n).max(1);
            let ch =
                ((f64::from(cw) * f64::from(dims.1) / f64::from(dims.0)).round() as u32).max(1);
            let mut canvas = RgbImage::from_pixel(width, label_pad + ch, BG);
            for (i, p) in panes.iter().enumerate() {
                let ox = i as u32 * (cw + GAP);
                let mut s = stage(i, cw, ch, 0.0);
                display(&mut s, st);
                image::imageops::replace(&mut canvas, &s, i64::from(ox), i64::from(label_pad));
                overlays(&mut canvas, (ox, label_pad), (cw, ch), reg, st, &p.hotspots);
                chip(&mut canvas, ox, 0, &p.label);
            }
            Ok(vec![canvas])
        }
        Layout::Swipe => {
            let (a, b) = (present[0], present[1]);
            let mut out = stage(a, width, oh, 0.0);
            let top = stage(b, width, oh, st.heat);
            let (sx, sy) = (
                (st.split * f64::from(width)).round() as u32,
                (st.split * f64::from(oh)).round() as u32,
            );
            for y in 0..oh {
                for x in 0..width {
                    if (!st.vertical && x >= sx) || (st.vertical && y >= sy) {
                        out.put_pixel(x, y, *top.get_pixel(x, y));
                    }
                }
            }
            display(&mut out, st);
            divider(&mut out, st);
            let hp = if panes[b].hotspots.is_empty() { a } else { b };
            overlays(&mut out, (0, 0), (width, oh), reg, st, &panes[hp].hotspots);
            chip(&mut out, 0, 0, &panes[a].label);
            let w = panes[b].label.chars().count() as u32 * 6 * text_scale(width)
                + 4 * text_scale(width);
            if st.vertical {
                chip(&mut out, 0, oh.saturating_sub(label_pad), &panes[b].label);
            } else {
                chip(&mut out, width.saturating_sub(w), 0, &panes[b].label);
            }
            Ok(vec![out])
        }
        Layout::Heatmap => {
            let i = present[1];
            let mut out = stage(i, width, oh, if st.heat > 0.0 { st.heat } else { 0.6 });
            display(&mut out, st);
            overlays(&mut out, (0, 0), (width, oh), reg, st, &panes[i].hotspots);
            let note = if panes[i].heat.is_some() {
                ""
            } else {
                " (NO HEATMAP)"
            };
            chip(&mut out, 0, 0, &format!("{}{note}", panes[i].label));
            Ok(vec![out])
        }
        Layout::Flicker => {
            let mut frames = Vec::new();
            for &i in &present {
                let mut f = stage(i, width, oh, 0.0);
                display(&mut f, st);
                overlays(&mut f, (0, 0), (width, oh), reg, st, &panes[i].hotspots);
                chip(&mut f, 0, 0, &panes[i].label);
                frames.push(f);
            }
            Ok(frames)
        }
    }
}

/// Renders the entry or set `entry` of `target` (a report JSON or a view
/// directory) in state `st`.
pub fn snapshot(
    target: &Path,
    entry: Option<&str>,
    st: &ViewState,
    width: u32,
) -> Result<Vec<RgbImage>> {
    let wanted = entry.or(st.name.as_deref());
    let source = if target.is_dir() {
        view_source(target, wanted)?
    } else {
        report_source(target, wanted)?
    };
    render(&source, st, width)
}

/// Whether `target` names something [`snapshot`] can read: a report JSON file
/// (by its usual name or any `.json`) or a view directory.
pub fn looks_like_target(target: &Path) -> bool {
    target.is_dir()
        || target.file_name().is_some_and(|n| n == REPORT_FILE_NAME)
        || target.extension().is_some_and(|e| e == "json")
}

/// Writes `frames` as PNGs. One frame goes to `out`; several go to
/// `<stem>-1.png`, `<stem>-2.png`, ... next to it. Returns the paths written.
pub fn write_frames(frames: &[RgbImage], out: &Path) -> Result<Vec<PathBuf>> {
    if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|source| Error::Io {
            context: format!("creating {}", dir.display()),
            source,
        })?;
    }
    let stem = out.file_stem().map_or_else(
        || "snapshot".to_owned(),
        |s| s.to_string_lossy().into_owned(),
    );
    let mut paths = Vec::new();
    for (i, f) in frames.iter().enumerate() {
        let path = if frames.len() == 1 {
            out.to_path_buf()
        } else {
            out.with_file_name(format!("{stem}-{}.png", i + 1))
        };
        f.save(&path).map_err(|source| Error::Encode {
            path: path.clone(),
            source,
        })?;
        paths.push(path);
    }
    Ok(paths)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn pane(label: &str, path: PathBuf, reference: bool, colour: [u8; 3]) -> SnapPane {
        RgbImage::from_pixel(40, 20, Rgb(colour))
            .save(&path)
            .unwrap();
        SnapPane {
            label: label.into(),
            path: Some(path),
            heat: None,
            reference,
            hotspots: Vec::new(),
        }
    }

    #[test]
    fn swipe_has_the_asked_width_and_splits_at_the_fraction() {
        let tmp = tempfile::tempdir().unwrap();
        let src = SnapSource {
            name: "x.png".into(),
            blind: false,
            panes: vec![
                pane("base", tmp.path().join("a.png"), true, [200, 0, 0]),
                pane("cap", tmp.path().join("b.png"), false, [0, 0, 200]),
            ],
        };
        let st = ViewState::parse("#layout=swipe&split=0.25&bogus=1&zoom=nope");
        assert_eq!(
            (st.layout, st.split, st.zoom),
            (Some(Layout::Swipe), 0.25, Zoom::Fit)
        );
        let frames = render(&src, &st, 400).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].dimensions(), (400, 200));
        // The first image is left of the divider (x = 100), the second right of it.
        assert_eq!(frames[0].get_pixel(50, 100).0, [200, 0, 0]);
        assert_eq!(frames[0].get_pixel(300, 100).0, [0, 0, 200]);
        // The side layout is a row of both images; flicker is one frame each.
        let side = render(&src, &ViewState::parse("layout=side"), 400).unwrap();
        assert_eq!(side[0].width(), 400);
        assert_eq!(
            render(&src, &ViewState::parse("layout=flicker"), 400)
                .unwrap()
                .len(),
            2
        );
    }
}
