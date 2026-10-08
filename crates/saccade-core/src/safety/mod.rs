//! Photosensitivity pre-check using WCAG flash and BT.1702 pattern guidance.
//! See `docs/safety-a11y.md` for assumptions, source citations and limitations.

pub use crate::color;
mod input;
pub mod output;
mod patterns;
pub mod thresholds;

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::Path;
use thresholds::value;

/// Versioned report marker.
pub const FILE: &str = "saccade-safety.v1.json";

/// Display geometry: pixels, diagonal inches and viewing distance in metres.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Display {
    /// Native horizontal pixels (used for aspect ratio).
    pub width: u32,
    /// Native vertical pixels.
    pub height: u32,
    /// Physical diagonal, inches.
    pub diagonal_inches: f64,
    /// Viewing distance, metres.
    pub distance_m: f64,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            diagonal_inches: 55.0,
            distance_m: 4.0,
        }
    }
}

impl Display {
    /// Parses `WxH@inches,distance` (distance in metres).
    pub fn parse(text: &str) -> Result<Self> {
        let invalid = || {
            Error::Config(
                "display must be WxH@inches,distance_m with positive finite values".into(),
            )
        };
        let (pixels, physical) = text.split_once('@').ok_or_else(invalid)?;
        let (w, h) = pixels.split_once('x').ok_or_else(invalid)?;
        let (diagonal, distance) = physical.split_once(',').ok_or_else(invalid)?;
        let d = Self {
            width: w.parse().map_err(|_| invalid())?,
            height: h.parse().map_err(|_| invalid())?,
            diagonal_inches: diagonal.parse().map_err(|_| invalid())?,
            distance_m: distance.parse().map_err(|_| invalid())?,
        };
        d.validate()?;
        Ok(d)
    }
    fn validate(self) -> Result<()> {
        if self.width == 0
            || self.height == 0
            || !self.diagonal_inches.is_finite()
            || self.diagonal_inches <= 0.0
            || !self.distance_m.is_finite()
            || self.distance_m <= 0.0
        {
            return Err(Error::Config(
                "display geometry must have positive finite dimensions and distance".into(),
            ));
        }
        Ok(())
    }
    fn metres(self) -> (f64, f64) {
        let diagonal = self.diagonal_inches * 0.0254;
        let norm = f64::from(self.width).hypot(f64::from(self.height));
        (
            diagonal * f64::from(self.width) / norm,
            diagonal * f64::from(self.height) / norm,
        )
    }
    /// Exact solid angle of the rectangular screen as seen from its centre.
    pub fn solid_angle(self) -> f64 {
        let (w, h) = self.metres();
        4.0 * ((w / 2.0) * (h / 2.0)
            / (self.distance_m
                * (self.distance_m.powi(2) + (w / 2.0).powi(2) + (h / 2.0).powi(2)).sqrt()))
        .atan()
    }
}

/// Standard selects flash area/luminance rules; pattern guidance is BT.1702.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Standard {
    /// Broadcast criteria with the explicitly assumed SDR peak luminance.
    #[default]
    ItuBt1702,
    /// WCAG SC 2.3.1 relative luminance and local solid-angle area.
    Wcag,
}

impl Standard {
    /// Accepted CLI spellings.
    pub fn parse(text: &str) -> Result<Self> {
        match text {
            "itu-bt1702" => Ok(Self::ItuBt1702),
            "wcag" => Ok(Self::Wcag),
            _ => Err(Error::Config("standard must be itu-bt1702 or wcag".into())),
        }
    }
}

/// Deterministic input settings.
#[derive(Debug, Default, Clone)]
pub struct Options {
    /// Overrides metadata; frame directories without metadata assume 60 fps.
    pub fps: Option<f64>,
    /// Viewing geometry.
    pub display: Display,
    /// Flash criteria.
    pub standard: Standard,
}

/// Artifact and source numbering for an individual frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    /// Sorted zero-based index.
    pub index: usize,
    /// Number in the source filename.
    pub number: u64,
    /// Portable source-relative name.
    pub name: String,
    /// Seconds since first frame.
    pub timestamp: f64,
    /// Static PNG relative to report HTML.
    pub image: String,
}

/// One continuous or overlapping run of risk windows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    /// `general_flash`, `red_flash` or `pattern`.
    pub kind: String,
    /// PASS is implicit; risk segments are WARN or FAIL.
    pub verdict: String,
    /// Beginning sorted frame index, including the first paired transition.
    pub start_frame: usize,
    /// Last affected frame index.
    pub end_frame: usize,
    /// Inclusive start time in seconds.
    pub start_seconds: f64,
    /// Exclusive end time in seconds.
    pub end_seconds: f64,
    /// Peak completed non-overlapping opposing-transition pairs per second.
    pub peak_flash_rate: f64,
    /// Maximum affected screen area, percentage.
    pub flashing_area_percent: f64,
    /// Maximum local solid angle (WCAG), or whole affected solid angle (BT).
    pub area_sr: f64,
    /// Orange risk mask; static PNG relative to report.
    pub heatmap: String,
}

/// Complete versioned output, including framing and threshold provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyReport {
    /// `saccade-safety.v1`.
    pub schema: String,
    /// PASS, WARN or FAIL for this pre-check only.
    pub verdict: String,
    /// Explicit pre-check disclaimer.
    pub disclaimer: String,
    /// Selected criteria.
    pub standard: Standard,
    /// Effective frame rate.
    pub fps: f64,
    /// Effective viewing geometry.
    pub display: Display,
    /// Derived flash screen-area threshold (percent); local WCAG measurement still applies.
    pub area_threshold_percent: f64,
    /// Numeric criteria and verification notes.
    pub thresholds: Vec<thresholds::Threshold>,
    /// Assumptions and detector limits.
    pub warnings: Vec<String>,
    /// Input frame artifacts.
    pub frames: Vec<Frame>,
    /// Clickable risk timeline.
    pub segments: Vec<Segment>,
}

impl SafetyReport {
    /// Human output; reproducible across platforms and output locations.
    pub fn text(&self) -> String {
        let mut text = format!(
            "safety: {} ({} frames, {:.3} fps, {})\n{}\nflash area threshold: {:.3}% of screen\nrisk segments: {}\n",
            self.verdict,
            self.frames.len(),
            self.fps,
            if self.standard == Standard::Wcag {
                "wcag"
            } else {
                "itu-bt1702"
            },
            self.disclaimer,
            self.area_threshold_percent,
            self.segments.len()
        );
        for s in &self.segments {
            text.push_str(&format!(
                "{} {}: frames {}..{}, {:.3}..{:.3}s, peak {:.0} flashes/s, area {:.3}%\n",
                s.verdict,
                s.kind,
                s.start_frame,
                s.end_frame,
                s.start_seconds,
                s.end_seconds,
                s.peak_flash_rate,
                s.flashing_area_percent
            ));
        }
        for warning in &self.warnings {
            text.push_str(&format!("warning: {warning}\n"));
        }
        text
    }
    /// One testcase per risk segment, or a passing whole-sequence testcase.
    pub fn junit_cases(&self) -> Vec<(String, bool, String)> {
        if self.segments.is_empty() {
            return vec![("sequence".into(), false, self.text())];
        }
        self.segments
            .iter()
            .map(|s| {
                (
                    format!("{}:{}-{}", s.kind, s.start_frame, s.end_frame),
                    s.verdict == "FAIL",
                    format!(
                        "{}: {} flashes/s; {:.3}% area. {}",
                        s.verdict, s.peak_flash_rate, s.flashing_area_percent, self.disclaimer
                    ),
                )
            })
            .collect()
    }
}

struct Geometry {
    weights: Vec<f64>,
    width: usize,
    height: usize,
    cone_w: usize,
    cone_h: usize,
}

impl Geometry {
    fn new(d: Display, width: usize, height: usize) -> Self {
        let (w, h) = d.metres();
        let primitive = |x: f64, y: f64| {
            (x * y / (d.distance_m * (d.distance_m.powi(2) + x * x + y * y).sqrt())).atan()
        };
        let mut weights = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                let (x0, x1) = (
                    w * (x as f64 / width as f64 - 0.5),
                    w * ((x + 1) as f64 / width as f64 - 0.5),
                );
                let (y0, y1) = (
                    h * (y as f64 / height as f64 - 0.5),
                    h * ((y + 1) as f64 / height as f64 - 0.5),
                );
                weights.push(
                    primitive(x1, y1) - primitive(x0, y1) - primitive(x1, y0) + primitive(x0, y0),
                );
            }
        }
        // Circumscribed square, enlarged one pixel to cover arbitrary cone centres.
        let side = 2.0 * d.distance_m * (value("wcag_field_degrees") / 2.0).to_radians().tan();
        Self {
            weights,
            width,
            height,
            cone_w: ((side / w * width as f64).ceil() as usize)
                .saturating_add(1)
                .min(width),
            cone_h: ((side / h * height as f64).ceil() as usize)
                .saturating_add(1)
                .min(height),
        }
    }
    fn area(&self, mask: &[bool], standard: Standard) -> (f64, f64) {
        let fraction = mask.iter().filter(|&&v| v).count() as f64 / mask.len() as f64;
        if standard == Standard::ItuBt1702 {
            return (
                fraction,
                mask.iter()
                    .zip(&self.weights)
                    .filter(|(m, _)| **m)
                    .map(|(_, w)| w)
                    .sum(),
            );
        }
        let stride = self.width + 1;
        let mut summed = vec![0.0; stride * (self.height + 1)];
        for y in 0..self.height {
            let mut row = 0.0;
            for x in 0..self.width {
                let i = y * self.width + x;
                if mask[i] {
                    row += self.weights[i];
                }
                summed[(y + 1) * stride + x + 1] = summed[y * stride + x + 1] + row;
            }
        }
        let mut max: f64 = 0.0;
        for y in 0..=self.height - self.cone_h {
            for x in 0..=self.width - self.cone_w {
                let (x1, y1) = (x + self.cone_w, y + self.cone_h);
                max = max.max(
                    summed[y1 * stride + x1] - summed[y * stride + x1] - summed[y1 * stride + x]
                        + summed[y * stride + x],
                );
            }
        }
        (fraction, max)
    }
}

struct Event {
    end: usize,
    start: usize,
    pixels: Vec<usize>,
}
struct Flashes {
    pending: Vec<(i8, usize)>,
    events: VecDeque<Event>,
    counts: Vec<u32>,
}
impl Flashes {
    fn new(n: usize) -> Self {
        Self {
            pending: vec![(0, 0); n],
            events: VecDeque::new(),
            counts: vec![0; n],
        }
    }
    fn update(&mut self, directions: &[i8], index: usize, fps: f64) {
        while self
            .events
            .front()
            .is_some_and(|e| (index - e.end) as f64 / fps >= value("window_seconds") - 1e-10)
        {
            if let Some(e) = self.events.pop_front() {
                for p in e.pixels {
                    self.counts[p] -= 1;
                }
            }
        }
        let mut pixels = Vec::new();
        let mut start = index;
        for (p, &d) in directions.iter().enumerate() {
            if d == 0 {
                continue;
            }
            let previous = self.pending[p];
            if previous.0 == -d {
                pixels.push(p);
                self.counts[p] += 1;
                start = start.min(previous.1);
                self.pending[p] = (0, 0); // Non-overlapping pairs: never count one transition twice.
            } else {
                self.pending[p] = (d, index.saturating_sub(1));
            }
        }
        if !pixels.is_empty() {
            self.events.push_back(Event {
                end: index,
                start,
                pixels,
            });
        }
    }
}

struct Risk {
    segment: Segment,
    mask: Vec<bool>,
}

#[allow(clippy::too_many_arguments)]
fn add_risk(
    risks: &mut Vec<Risk>,
    kind: &str,
    verdict: &str,
    range: (usize, usize),
    rate: f64,
    mask: Vec<bool>,
    geometry: &Geometry,
    standard: Standard,
) {
    let (fraction, sr) = geometry.area(&mask, standard);
    if let Some(last) = risks
        .iter_mut()
        .rev()
        .find(|r| r.segment.kind == kind && r.segment.end_frame.saturating_add(1) >= range.0)
    {
        last.segment.end_frame = range.1;
        if verdict == "FAIL" {
            last.segment.verdict = verdict.into();
        }
        last.segment.peak_flash_rate = last.segment.peak_flash_rate.max(rate);
        last.segment.flashing_area_percent =
            last.segment.flashing_area_percent.max(fraction * 100.0);
        last.segment.area_sr = last.segment.area_sr.max(sr);
        for (dst, src) in last.mask.iter_mut().zip(mask) {
            *dst |= src;
        }
    } else {
        risks.push(Risk {
            segment: Segment {
                kind: kind.into(),
                verdict: verdict.into(),
                start_frame: range.0,
                end_frame: range.1,
                start_seconds: 0.0,
                end_seconds: 0.0,
                peak_flash_rate: rate,
                flashing_area_percent: fraction * 100.0,
                area_sr: sr,
                heatmap: String::new(),
            },
            mask,
        });
    }
}

/// Analyze every frame and write static evidence, versioned JSON, text and HTML.
pub fn run(path: &Path, out: &Path, options: &Options) -> Result<SafetyReport> {
    options.display.validate()?;
    let input = input::load(path, options.fps)?;
    let first = opaque(&input.frames[0].2)?;
    let (w, h) = first.dimensions();
    let geometry = Geometry::new(options.display, w as usize, h as usize);
    if !options.display.solid_angle().is_finite()
        || options.display.solid_angle() <= 0.0
        || geometry.weights.iter().any(|v| !v.is_finite() || *v <= 0.0)
    {
        return Err(Error::Config(
            "display geometry has non-finite or unresolvable solid angles".into(),
        ));
    }
    output::prepare(out, &[path], FILE)?;
    let mut report = SafetyReport {
        schema: "saccade-safety.v1".into(),
        verdict: "PASS".into(),
        disclaimer: output::DISCLAIMER.into(),
        standard: options.standard,
        fps: input.fps,
        display: options.display,
        area_threshold_percent: 100.0
            * if options.standard == Standard::ItuBt1702 {
                value("broadcast_area_fraction")
            } else {
                (value("wcag_area_sr") / options.display.solid_angle()).min(1.0)
            },
        thresholds: thresholds::table(),
        warnings: input.warnings,
        frames: Vec::new(),
        segments: Vec::new(),
    };
    report.warnings.push("SDR only; opaque sRGB, assumed 200 cd/m2 peak for BT.1702. Verify THRESHOLDS against the published edition.".into());
    report.warnings.push("Pattern scanline autocorrelation is approximate; all changing patterns use the five-pair limit. WCAG mode includes supplementary BT.1702 pattern guidance.".into());
    if options.standard == Standard::Wcag {
        report.warnings.push("WCAG area uses exact pixel solid angles inside conservative 10-degree squares; cone-edge findings require human verification.".into());
    }
    report.warnings.push("Red-flash detection uses the legacy WCAG 2.0/2.1 numeric working definition, not the WCAG 2.2 UCS definition; verify the required edition.".into());
    let n = first.len() / 3;
    let mut general = Flashes::new(n);
    let mut red = Flashes::new(n);
    let mut previous: Option<Vec<[f64; 3]>> = None;
    let mut risks = Vec::new();
    for (index, (name, number, path)) in input.frames.iter().enumerate() {
        let image = if index == 0 {
            first.clone()
        } else {
            opaque(path)?
        };
        if image.dimensions() != (w, h) {
            return Err(Error::Config(
                "all safety frames must have identical dimensions".into(),
            ));
        }
        let colors: Vec<_> = image.pixels().map(|p| color::rgb(p.0)).collect();
        let luma: Vec<_> = colors.iter().map(|&c| color::luminance(c)).collect();
        let mut changing = false;
        if let Some(prev) = &previous {
            let mut gd = vec![0; n];
            let mut rd = vec![0; n];
            for p in 0..n {
                let (a, b) = (color::luminance(prev[p]), luma[p]);
                let (delta, darker) = if options.standard == Standard::Wcag {
                    (value("luminance_change"), value("darker_luminance"))
                } else {
                    (
                        value("broadcast_change_cd_m2") / value("broadcast_peak_cd_m2"),
                        value("broadcast_darker_cd_m2") / value("broadcast_peak_cd_m2"),
                    )
                };
                if (b - a).abs() >= delta && a.min(b) < darker {
                    gd[p] = if b > a { 1 } else { -1 };
                }
                let sat = |c: [f64; 3]| {
                    c[0] + c[1] + c[2] > 0.0
                        && c[0] / (c[0] + c[1] + c[2]) >= value("red_saturation")
                };
                let redness = |c: [f64; 3]| (c[0] - c[1] - c[2]).max(0.0) * value("red_scale");
                let (ra, rb) = (redness(prev[p]), redness(colors[p]));
                if (sat(prev[p]) || sat(colors[p])) && (rb - ra).abs() > value("red_excursion") {
                    rd[p] = if rb > ra { 1 } else { -1 };
                }
                changing |= (b - a).abs() > 0.01;
            }
            general.update(&gd, index, input.fps);
            red.update(&rd, index, input.fps);
            for (kind, state) in [("general_flash", &general), ("red_flash", &red)] {
                let peak = f64::from(state.counts.iter().copied().max().unwrap_or(0));
                if peak < value("warn_flashes") {
                    continue;
                }
                let limit = if options.standard == Standard::Wcag {
                    value("wcag_area_sr")
                } else {
                    value("broadcast_area_fraction")
                };
                let measure = |mask: &[bool]| {
                    let (f, sr) = geometry.area(mask, options.standard);
                    if options.standard == Standard::Wcag {
                        sr
                    } else {
                        f
                    }
                };
                let fail_mask: Vec<_> = state
                    .counts
                    .iter()
                    .map(|&c| f64::from(c) > value("max_flashes"))
                    .collect();
                let fail_area = measure(&fail_mask);
                let warn_mask: Vec<_> = state
                    .counts
                    .iter()
                    .map(|&c| f64::from(c) >= value("warn_flashes"))
                    .collect();
                let (verdict, mask) = if fail_area > limit {
                    ("FAIL", fail_mask)
                } else if fail_area >= limit * value("warn_area_fraction")
                    || measure(&warn_mask) > limit
                {
                    ("WARN", warn_mask)
                } else {
                    continue;
                };
                let start = state.events.front().map_or(index, |e| e.start);
                add_risk(
                    &mut risks,
                    kind,
                    verdict,
                    (start, index),
                    peak,
                    mask,
                    &geometry,
                    options.standard,
                );
            }
        }
        let mask = patterns::mask(&luma, w as usize, h as usize, changing);
        let area = mask.iter().filter(|&&v| v).count() as f64 / n as f64;
        let pattern_area = value(if changing {
            "pattern_changing_area_fraction"
        } else {
            "pattern_static_area_fraction"
        });
        if area >= pattern_area * value("warn_area_fraction") {
            add_risk(
                &mut risks,
                "pattern",
                if area > pattern_area { "FAIL" } else { "WARN" },
                (index, index),
                0.0,
                mask,
                &geometry,
                Standard::ItuBt1702,
            );
        }
        let image = output::image(out, &format!("frame-{index:09}"), &image)?;
        report.frames.push(Frame {
            index,
            number: *number,
            name: name.clone(),
            timestamp: index as f64 / input.fps,
            image,
        });
        previous = Some(colors);
    }
    risks.sort_by(|a, b| {
        a.segment
            .start_frame
            .cmp(&b.segment.start_frame)
            .then(a.segment.kind.cmp(&b.segment.kind))
    });
    for (i, mut r) in risks.into_iter().enumerate() {
        r.segment.start_seconds = r.segment.start_frame as f64 / input.fps;
        r.segment.end_seconds = (r.segment.end_frame + 1) as f64 / input.fps;
        r.segment.heatmap = output::image(
            out,
            &format!("risk-{i:04}"),
            &output::heatmap(&r.mask, w, h),
        )?;
        report.segments.push(r.segment);
    }
    if report.segments.iter().any(|s| s.verdict == "FAIL") {
        report.verdict = "FAIL".into();
    } else if !report.segments.is_empty() {
        report.verdict = "WARN".into();
    }
    output::finish(
        out,
        FILE,
        &report,
        &report.text(),
        "Photosensitivity pre-check",
    )?;
    Ok(report)
}

/// Reject HDR and transparent inputs rather than guessing their displayed light.
pub fn opaque(path: &Path) -> Result<image::RgbImage> {
    if crate::hdr::is_hdr_path(path) {
        return Err(Error::Config(
            "pre-check requires SDR sRGB; export the displayed HDR tone-map first".into(),
        ));
    }
    let image = crate::run::decode(path)?;
    if image.width() == 0 || image.height() == 0 {
        return Err(Error::EmptyImage);
    }
    if image.pixels().any(|p| p[3] != 255) {
        return Err(Error::Config(
            "pre-check requires opaque images; composite on the actual background first".into(),
        ));
    }
    Ok(image::RgbImage::from_fn(
        image.width(),
        image.height(),
        |x, y| {
            let p = image.get_pixel(x, y);
            image::Rgb([p[0], p[1], p[2]])
        },
    ))
}
