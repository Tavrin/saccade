//! Automatic pixel accessibility pre-checks. Detected candidates never certify coverage.
use crate::{Error, Result, a11y};
use image::{RgbImage, RgbaImage};
use saccade_core::text_quality as tq;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Automatic report contract, separate from declared-region contracts.
pub const SCHEMA: &str = "saccade-auto-a11y.v1";
/// Report artifact filename.
pub const FILE: &str = "saccade-auto-a11y.v1.json";
/// Deterministic edge/stroke grouping detector identity.
pub const FALLBACK: &str = "edge-stroke-lines/2";
/// Precision-first closed component detector identity.
pub const UI: &str = "closed-edge-components/1";
/// Bounded analysis: larger captures fail explicitly, without resampling away text.
const MAX_PIXELS: u64 = 8 * 1024 * 1024;
const MAX_COMPONENTS: usize = 8192;

/// Typed WCAG text threshold selection.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Level {
    /// AA: 4.5 normal / 3 large.
    #[default]
    AA,
    /// AAA: 7 normal / 4.5 large.
    AAA,
}
/// Offline automatic options; no network/download option exists.
#[derive(Debug, Clone)]
pub struct Options {
    /// Existing declared-region config, authoritative on overlapping candidates.
    pub config: Option<PathBuf>,
    /// Text contrast target.
    pub level: Level,
    /// Capture pixels per point, used only when scale is explicitly supplied.
    pub px_per_pt: f64,
    /// Whether the caller supplied physical display scale.
    pub scale_known: bool,
    /// Provisioned pinned OCR/runtime cache; None means model explicitly unavailable.
    pub model_cache: Option<PathBuf>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            config: None,
            level: Level::AA,
            px_per_pt: 96. / 72.,
            scale_known: false,
            model_cache: None,
        }
    }
}
/// Per-check states. Missing evidence cannot become PASS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Verdict {
    /// Threshold met within this candidate only.
    Pass,
    /// Measured threshold failed.
    Fail,
    /// A heuristic candidate requires interpretation.
    Warn,
    /// Pixels/detection do not support measurement.
    Unmeasurable,
}
impl std::fmt::Display for Verdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::Warn => "WARN",
            Self::Unmeasurable => "UNMEASURABLE",
        })
    }
}
/// Detector availability and exact interpretation of scores.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectorStatus {
    /// Versioned implementation identity.
    pub detector: String,
    /// available or unavailable.
    pub state: String,
    /// Explicit reason/score limitations.
    pub reason: String,
}
/// A detected region, never a human-confirmed declaration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Region {
    /// text or ui candidate.
    pub kind: String,
    /// Capture pixels [x,y,width,height], including a two-pixel measurement margin.
    pub rect_px: [u32; 4],
    /// Unpadded detected body height, for the size assumption.
    pub text_height_px: u32,
    /// Always auto_detected.
    pub provenance: String,
    /// Versioned detector identity.
    pub detector: String,
    /// Heuristic support score in [0,1], not calibrated probability.
    pub confidence: f64,
    /// Suppressed by an overlapping authoritative declaration; retained for coverage.
    pub overridden_by_declared: bool,
}
/// One automatic region and its independent checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Detection metadata and box.
    pub region: Region,
    /// Contrast verdict.
    pub verdict: Verdict,
    /// Foreground sRGB swatch from the worst supported local core.
    pub foreground: Option<[u8; 3]>,
    /// Background sRGB swatch from that component’s local ring.
    pub background: Option<[u8; 3]>,
    /// Worst supported local contrast or thin-stroke bound; absent for ambiguous evidence.
    pub ratio: Option<f64>,
    /// WCAG target, including assumed large text.
    pub required_ratio: f64,
    /// SC 1.4.3 (AA text), 1.4.6 (AAA text), or 1.4.11 (UI candidate).
    pub success_criterion: String,
    /// Inferred >=18pt body; no bold inference.
    pub assumed_large: bool,
    /// Per-component text-quality measurements, absent for UI/overridden candidates.
    pub legibility: Option<tq::RegionResult>,
    /// PASS/FAIL/UNMEASURABLE pixel legibility, absent for UI/overridden candidates.
    pub legibility_verdict: Option<Verdict>,
    /// Candidate tofu boxes use absolute capture coordinates.
    pub missing_glyphs: Vec<tq::Candidate>,
    /// WARN for tofu candidates; otherwise UNMEASURABLE, never glyph completeness PASS.
    pub glyph_verdict: Verdict,
    /// Measurement assumptions and abstention reasons.
    pub reasons: Vec<String>,
}
/// Criterion states keep unsupported checks separate from measured verdicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Status {
    /// Measured regions meet the criterion.
    Pass,
    /// A measured region fails.
    Fail,
    /// A measured heuristic needs review.
    Warn,
    /// Applicable regions cannot be measured.
    Unmeasurable,
    /// The available check cannot verify this property.
    NotVerified,
    /// No applicable regions.
    NotApplicable,
}
impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::Warn => "WARN",
            Self::Unmeasurable => "UNMEASURABLE",
            Self::NotVerified => "NOT_VERIFIED",
            Self::NotApplicable => "NOT_APPLICABLE",
        })
    }
}
impl From<Verdict> for Status {
    fn from(v: Verdict) -> Self {
        match v {
            Verdict::Pass => Self::Pass,
            Verdict::Fail => Self::Fail,
            Verdict::Warn => Self::Warn,
            Verdict::Unmeasurable => Self::Unmeasurable,
        }
    }
}
/// Counts of independent region observations for a criterion.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Counts {
    /// Measured passing observations.
    pub pass: usize,
    /// Measured failing observations.
    pub fail: usize,
    /// Warning observations.
    pub warn: usize,
    /// Applicable but unmeasurable observations.
    pub unmeasurable: usize,
    /// Observations with verification unavailable.
    pub not_verified: usize,
}
/// Stable reference into an image's evidence arrays.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionRef {
    /// automatic, declared_contrast, or colour_vision_findings.
    pub source: String,
    /// Zero-based observation index in that array.
    pub index: usize,
}
/// Aggregate evidence for one criterion, with the worst observation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriterionSummary {
    /// Aggregate criterion status.
    pub status: Status,
    /// Region observation counts.
    pub counts: Counts,
    /// Reference to the worst observation, absent if no regions apply.
    pub worst_region: Option<RegionRef>,
    /// Human-readable worst measurement or limitation.
    pub detail: String,
    /// WCAG success criterion when applicable.
    pub success_criterion: Option<String>,
    #[serde(skip)]
    worst_score: Option<f64>,
}
impl CriterionSummary {
    fn empty(status: Status, detail: &str, sc: Option<&str>) -> Self {
        Self {
            status,
            counts: Counts::default(),
            worst_region: None,
            detail: detail.into(),
            success_criterion: sc.map(str::to_owned),
            worst_score: None,
        }
    }
    fn observe(
        &mut self,
        status: Status,
        source: &str,
        index: usize,
        detail: String,
        score: Option<f64>,
    ) {
        let rank = |s| match s {
            Status::Fail => 5,
            Status::Warn => 4,
            Status::Unmeasurable => 2,
            Status::NotVerified => 1,
            Status::Pass => 3,
            Status::NotApplicable => 0,
        };
        let first = self.worst_region.is_none();
        let replace = first
            || rank(status) > rank(self.status)
            || (status == self.status
                && score.is_some_and(|v| self.worst_score.is_some_and(|old| v < old)));
        match status {
            Status::Pass => self.counts.pass += 1,
            Status::Fail => self.counts.fail += 1,
            Status::Warn => self.counts.warn += 1,
            Status::Unmeasurable => self.counts.unmeasurable += 1,
            Status::NotVerified => self.counts.not_verified += 1,
            Status::NotApplicable => {}
        }
        if replace {
            self.worst_score = score;
            self.status = status;
            self.worst_region = Some(RegionRef {
                source: source.into(),
                index,
            });
            self.detail = detail;
        }
        // Unknown regions remain counted and limited, but cannot erase measured PASS.
        if self.counts.fail > 0 {
            self.status = Status::Fail;
        } else if self.counts.warn > 0 {
            self.status = Status::Warn;
        } else if self.counts.pass > 0 {
            self.status = Status::Pass;
        }
    }
}
/// Required per-image criterion summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    /// SC 1.4.3 or 1.4.6 source-colour contrast.
    pub wcag_text_contrast: CriterionSummary,
    /// SC 1.4.11 candidate boundary contrast.
    pub wcag_non_text_contrast: CriterionSummary,
    /// Displayed contrast, size, stroke and sharpness checks.
    pub rendered_legibility: CriterionSummary,
    /// Glyph-shaped candidates; completeness remains unverified.
    pub missing_glyphs: CriterionSummary,
    /// Colour-vision loss candidates; semantic coverage remains unverified.
    pub colour_vision_loss: CriterionSummary,
}
impl Summary {
    fn entries(&self) -> [(&str, &str, &CriterionSummary); 5] {
        [
            (
                "wcag_text_contrast",
                "WCAG text contrast",
                &self.wcag_text_contrast,
            ),
            (
                "wcag_non_text_contrast",
                "WCAG non-text contrast",
                &self.wcag_non_text_contrast,
            ),
            (
                "rendered_legibility",
                "rendered legibility",
                &self.rendered_legibility,
            ),
            ("missing_glyphs", "missing glyphs", &self.missing_glyphs),
            (
                "colour_vision_loss",
                "colour vision loss",
                &self.colour_vision_loss,
            ),
        ]
    }
    fn build(automatic: &[Finding], declared: &a11y::ImageReport, level: Level) -> Self {
        let mut s = Self {
            wcag_text_contrast: CriterionSummary::empty(
                Status::Unmeasurable,
                "no measured text regions",
                Some(if level == Level::AA { "1.4.3" } else { "1.4.6" }),
            ),
            wcag_non_text_contrast: CriterionSummary::empty(
                Status::NotApplicable,
                "no candidate UI regions",
                Some("1.4.11"),
            ),
            rendered_legibility: CriterionSummary::empty(
                Status::Unmeasurable,
                "no measured text regions",
                None,
            ),
            missing_glyphs: CriterionSummary::empty(
                Status::NotVerified,
                "glyph completeness unverified",
                None,
            ),
            colour_vision_loss: CriterionSummary::empty(
                Status::NotVerified,
                "semantic colour information completeness unverified",
                None,
            ),
        };
        for (i, f) in automatic
            .iter()
            .enumerate()
            .filter(|(_, f)| !f.region.overridden_by_declared)
        {
            let c = if f.region.kind == "text" {
                &mut s.wcag_text_contrast
            } else {
                &mut s.wcag_non_text_contrast
            };
            let detail = match f.ratio {
                Some(r) if f.verdict == Verdict::Unmeasurable => {
                    format!("not determinable (thin text, lower bound {r:.2}:1)")
                }
                Some(r) => format!("{r:.2} :1 source-colour; requires {}:1", f.required_ratio),
                None => "not determinable (unsupported local pixels)".into(),
            };
            c.observe(f.verdict.into(), "automatic", i, detail, f.ratio);
            if f.region.kind == "text" {
                if let Some(v) = f.legibility_verdict {
                    let q = f.legibility.as_ref();
                    let ratio = q.and_then(|q| q.contrast);
                    let detail = match ratio {
                        Some(r) => format!(
                            "{r:.2} :1 displayed; {}",
                            q.map(|q| q.reasons.join(", ")).unwrap_or_default()
                        ),
                        None => "not determinable (unsupported local pixels)".into(),
                    };
                    s.rendered_legibility
                        .observe(v.into(), "automatic", i, detail, ratio);
                }
                s.missing_glyphs.observe(
                    if f.glyph_verdict == Verdict::Warn {
                        Status::Warn
                    } else {
                        Status::NotVerified
                    },
                    "automatic",
                    i,
                    format!(
                        "{} glyph-shaped candidates; glyph completeness unverified",
                        f.missing_glyphs.len()
                    ),
                    None,
                );
            }
        }
        for (i, c) in declared.contrast.iter().enumerate() {
            let target = if c.kind == "text" {
                &mut s.wcag_text_contrast
            } else {
                &mut s.wcag_non_text_contrast
            };
            // Classify the unchanged declared measurement against this summary's target.
            // Keep the configured declared verdict/threshold in its original report.
            let large = c.kind == "text"
                && ((c.level == "AA" && c.threshold == 3.)
                    || (c.level == "AAA" && c.threshold == 4.5));
            let required = if c.kind == "ui" {
                3.
            } else {
                match (level, large) {
                    (Level::AA, false) | (Level::AAA, true) => 4.5,
                    (Level::AA, true) => 3.,
                    (Level::AAA, false) => 7.,
                }
            };
            let status = match c.ratio {
                Some(r) if r + 1e-12 >= required => Status::Pass,
                Some(_) if !c.note.contains("lower bound") => Status::Fail,
                _ => Status::Unmeasurable,
            };
            target.observe(
                status,
                "declared_contrast",
                i,
                format!(
                    "{}; summary requires {required}:1; configured {} verdict {}; {}",
                    c.ratio
                        .map(|r| format!("{r:.2} :1 source-colour"))
                        .unwrap_or_else(|| "not determinable".into()),
                    c.level,
                    c.verdict,
                    c.note
                ),
                c.ratio,
            );
        }
        for (i, f) in declared.findings.iter().enumerate() {
            s.colour_vision_loss.observe(
                Status::Warn,
                "colour_vision_findings",
                i,
                f.message.clone(),
                Some(f.simulated_delta_e),
            );
        }
        s
    }
    fn verdict(&self, text_detected: bool) -> Verdict {
        let statuses: Vec<_> = self.entries().iter().map(|(_, _, c)| c.status).collect();
        if statuses.contains(&Status::Fail) {
            Verdict::Fail
        } else if statuses.contains(&Status::Warn) {
            Verdict::Warn
        } else if text_detected && statuses.contains(&Status::Pass) {
            Verdict::Pass
        } else {
            Verdict::Unmeasurable
        }
    }
}

/// Automatic evidence for one capture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageReport {
    /// Portable relative input name.
    pub name: String,
    /// Aggregate; zero text detection cannot PASS.
    pub verdict: Verdict,
    /// Independent criterion verdicts and counts.
    pub summary: Summary,
    /// Scope of every image PASS.
    pub scope: String,
    /// Explicit gaps that do not erase measured results.
    pub limitations: Vec<String>,
    /// Exact detector availability, including absent pinned model.
    pub detectors: Vec<DetectorStatus>,
    /// Explicit text detection count/no-text statement.
    pub text_detection: String,
    /// Automatic findings, separately attributed from declared checks.
    pub automatic: Vec<Finding>,
    /// Original and CVD artifacts plus authoritative declared-region contrast checks.
    pub declared_and_colour_vision: a11y::ImageReport,
}
/// End-to-end offline report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    /// saccade-auto-a11y.v1.
    pub schema: String,
    /// Content-addressed transport identity, populated on emission.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_id: Option<String>,
    /// Optional invocation backlinks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<String>,
    /// Failures dominate unknown/warning coverage.
    pub verdict: Verdict,
    /// Scope of a measured run PASS.
    pub scope: String,
    /// PRE-CHECK framing.
    pub disclaimer: String,
    /// Effective text target.
    pub level: Level,
    /// Effective physical scale assumption.
    pub px_per_pt: f64,
    /// Size inference and coverage limits.
    pub assumptions: Vec<String>,
    /// Sorted input evidence.
    pub images: Vec<ImageReport>,
}
fn validate(options: &Options) -> Result<()> {
    if !options.px_per_pt.is_finite() || !(0.1..=100.).contains(&options.px_per_pt) {
        return Err(Error::Config(
            "px-per-pt must be finite in 0.1..=100".into(),
        ));
    }
    Ok(())
}
fn padded(r: [u32; 4], size: (u32, u32)) -> [u32; 4] {
    let x = r[0].saturating_sub(2);
    let y = r[1].saturating_sub(2);
    [
        x,
        y,
        (r[0] + r[2] + 2).min(size.0) - x,
        (r[1] + r[3] + 2).min(size.1) - y,
    ]
}
fn region(kind: &str, r: [u32; 4], size: (u32, u32), detector: &str, confidence: f64) -> Region {
    Region {
        kind: kind.into(),
        rect_px: padded(r, size),
        text_height_px: r[3].saturating_sub(1),
        provenance: "auto_detected".into(),
        detector: detector.into(),
        confidence,
        overridden_by_declared: false,
    }
}
fn overlap(a: [u32; 4], b: [u32; 4]) -> bool {
    a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
}

/// Bounded model-free edge components and aligned stroke groups. No image semantics inferred.
pub fn fallback(image: &RgbImage) -> Result<Vec<Region>> {
    let (w, h) = image.dimensions();
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(Error::Config(
            "automatic a11y requires 1..=8388608 pixels".into(),
        ));
    }
    let mut edges = vec![false; w as usize * h as usize];
    let distance = |a: [u8; 3], b: [u8; 3]| {
        a.into_iter()
            .zip(b)
            .map(|(x, y)| x.abs_diff(y))
            .max()
            .unwrap_or(0)
    };
    for y in 0..h {
        for x in 0..w {
            let p = image.get_pixel(x, y).0;
            edges[(y * w + x) as usize] = (x + 1 < w
                && distance(p, image.get_pixel(x + 1, y).0) >= 8)
                || (y + 1 < h && distance(p, image.get_pixel(x, y + 1).0) >= 8);
        }
    }
    let mut seen = vec![false; edges.len()];
    let mut boxes = Vec::new();
    for seed in 0..edges.len() {
        if !edges[seed] || seen[seed] {
            continue;
        }
        if boxes.len() >= MAX_COMPONENTS {
            return Err(Error::Config(
                "automatic detector component budget exceeded".into(),
            ));
        }
        let mut stack = vec![seed];
        seen[seed] = true;
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
        let mut count = 0;
        while let Some(i) = stack.pop() {
            let (x, y) = (i as u32 % w, i as u32 / w);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x + 1);
            y1 = y1.max(y + 1);
            count += 1;
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    let (xx, yy) = (i64::from(x) + dx, i64::from(y) + dy);
                    if xx < 0 || yy < 0 || xx >= i64::from(w) || yy >= i64::from(h) {
                        continue;
                    }
                    let j = (yy as u32 * w + xx as u32) as usize;
                    if edges[j] && !seen[j] {
                        seen[j] = true;
                        stack.push(j);
                    }
                }
            }
        }
        boxes.push(([x0, y0, x1 - x0, y1 - y0], count));
    }
    boxes.sort_by_key(|(r, _)| (r[1], r[0]));
    let glyph = |r: [u32; 4]| {
        (5..=128).contains(&r[3]) && r[2] >= 2 && r[2] * 8 >= r[3] && r[2] <= r[3] * 3 / 2
    };
    let mut used = vec![false; boxes.len()];
    let mut regions = Vec::new();
    for i in 0..boxes.len() {
        if used[i] || !glyph(boxes[i].0) {
            continue;
        }
        let mut group = vec![i];
        let mut r = boxes[i].0;
        loop {
            let next = (0..boxes.len())
                .filter(|j| !used[*j])
                .filter(|j| {
                    let b = boxes[*j].0;
                    glyph(b)
                        && b[0] >= r[0] + r[2]
                        && b[0] - (r[0] + r[2]) <= r[3] * 2
                        && b[3].min(r[3]) * 2 >= b[3].max(r[3])
                        && (b[1] + b[3])
                            .min(r[1] + r[3])
                            .saturating_sub(b[1].max(r[1]))
                            * 2
                            >= b[3].min(r[3])
                })
                .min_by_key(|j| boxes[*j].0[0]);
            let Some(j) = next else {
                break;
            };
            let b = boxes[j].0;
            let bottom = (r[1] + r[3]).max(b[1] + b[3]);
            r[1] = r[1].min(b[1]);
            r[2] = b[0] + b[2] - r[0];
            r[3] = bottom - r[1];
            group.push(j);
        }
        if group.len() >= 3 {
            for j in group {
                used[j] = true;
            }
            regions.push(region("text", r, (w, h), FALLBACK, 0.7));
        }
    }
    // Rejoin supported fragments of the same line (mixed cap/x-height and
    // disconnected strokes can split the seed groups). Never bridge distant lines.
    let aligned = |a: [u32; 4], b: [u32; 4]| {
        let vertical = (a[1] + a[3])
            .min(b[1] + b[3])
            .saturating_sub(a[1].max(b[1]));
        let gap = a[0]
            .max(b[0])
            .saturating_sub((a[0] + a[2]).min(b[0] + b[2]));
        vertical * 2 >= a[3].min(b[3]) && gap <= a[3].max(b[3]) * 3
    };
    let union = |a: [u32; 4], b: [u32; 4]| {
        let x = a[0].min(b[0]);
        let y = a[1].min(b[1]);
        [
            x,
            y,
            (a[0] + a[2]).max(b[0] + b[2]) - x,
            (a[1] + a[3]).max(b[1] + b[3]) - y,
        ]
    };
    loop {
        let pair = (0..regions.len()).find_map(|i| {
            ((i + 1)..regions.len())
                .find(|j| aligned(regions[i].rect_px, regions[*j].rect_px))
                .map(|j| (i, j))
        });
        let Some((i, j)) = pair else { break };
        let b = regions.remove(j);
        regions[i].rect_px = union(regions[i].rect_px, b.rect_px);
        regions[i].text_height_px = regions[i].text_height_px.max(b.text_height_px);
    }
    // Attach remaining substantial letter components to a supported line. This
    // extends its bounds without allowing isolated components to invent text.
    for (i, (b, _)) in boxes.iter().enumerate() {
        if used[i] || !glyph(*b) {
            continue;
        }
        let candidate = padded(*b, (w, h));
        if let Some(r) = regions.iter_mut().find(|r| {
            aligned(r.rect_px, candidate)
                && b[3] <= r.text_height_px * 3 / 2 + 2
                && b[3] * 2 >= r.text_height_px
        }) {
            r.rect_px = union(r.rect_px, candidate);
            r.text_height_px = r.text_height_px.max(b[3].saturating_sub(1));
            used[i] = true;
        }
    }
    // Closed compact boundaries only: four supported sides; avoid labelling arbitrary texture as UI.
    for (i, (r, count)) in boxes.iter().enumerate() {
        if used[i]
            || !(6..=128).contains(&r[2])
            || !(6..=128).contains(&r[3])
            || r[2] > r[3] * 3
            || r[3] > r[2] * 3
            || *count > (r[2] * r[3] / 2) as usize
        {
            continue;
        }
        let side = |horizontal: bool, end: bool| {
            let n = if horizontal { r[2] } else { r[3] };
            (0..n)
                .filter(|v| {
                    let x = if horizontal {
                        r[0] + v
                    } else {
                        r[0] + if end { r[2] - 1 } else { 0 }
                    };
                    let y = if horizontal {
                        r[1] + if end { r[3] - 1 } else { 0 }
                    } else {
                        r[1] + v
                    };
                    edges[(y * w + x) as usize]
                })
                .count() as f64
                / f64::from(n)
        };
        if [
            side(true, false),
            side(true, true),
            side(false, false),
            side(false, true),
        ]
        .into_iter()
        .all(|v| v >= 0.7)
        {
            regions.push(region("ui", *r, (w, h), UI, 0.75));
        }
    }
    regions.sort_by_key(|r| (r.rect_px[1], r.rect_px[0]));
    Ok(regions)
}
fn fractional(r: [u32; 4], image: &RgbImage) -> [f64; 4] {
    [
        f64::from(r[0]) / f64::from(image.width()),
        f64::from(r[1]) / f64::from(image.height()),
        f64::from(r[2]) / f64::from(image.width()),
        f64::from(r[3]) / f64::from(image.height()),
    ]
}
/// Measure one candidate using the existing contrast and text-quality estimators.
/// Region geometry is validated; declared overrides produce no automatic PASS/FAIL.
pub fn measure(image: &RgbImage, region: Region, options: &Options) -> Result<Finding> {
    validate(options)?;
    tq::validate_rect([image.width(), image.height()], region.rect_px)?;
    let [x, y, w, h] = region.rect_px;
    let rgb = RgbaImage::from_fn(w, h, |xx, yy| {
        let p = image.get_pixel(x + xx, y + yy).0;
        image::Rgba([p[0], p[1], p[2], 255])
    });
    let body_height = if region.kind == "text" && !region.overridden_by_declared {
        tq::line_height(&rgb, [0, 0, w, h])?
    } else {
        None
    };
    let large = options.scale_known
        && region.kind == "text"
        && body_height.is_some_and(|height| {
            height.min(f64::from(region.text_height_px)) / options.px_per_pt >= 18.
        });
    let declared = a11y::Region {
        name: "automatic candidate".into(),
        kind: region.kind.clone(),
        rect: fractional(region.rect_px, image),
        glob: None,
        large,
        level: format!("{:?}", options.level),
    };
    if !matches!(region.kind.as_str(), "text" | "ui") {
        return Err(Error::Config(
            "automatic region kind must be text/ui".into(),
        ));
    }
    let c = a11y::contrast(image, &declared)?;
    let mut result = Finding {
        region,
        verdict: Verdict::Unmeasurable,
        foreground: c.foreground,
        background: c.background,
        ratio: c.ratio,
        required_ratio: c.threshold,
        success_criterion: if declared.kind == "ui" {
            "1.4.11"
        } else if options.level == Level::AAA {
            "1.4.6"
        } else {
            "1.4.3"
        }
        .into(),
        assumed_large: large,
        legibility: None,
        legibility_verdict: None,
        missing_glyphs: Vec::new(),
        glyph_verdict: Verdict::Unmeasurable,
        reasons: vec![c.note],
    };
    if options.scale_known && result.region.kind == "text" {
        result.reasons.push(format!(
            "explicit scale: robust ascender body height {body_height:?} px, capped by detected height {} px; {} px-per-pt; large-text assumption {large}",
            result.region.text_height_px, options.px_per_pt
        ));
    }
    if !options.scale_known && result.region.kind == "text" {
        result
            .reasons
            .push("scale unknown: normal-text threshold applied".into());
    }
    if result.region.overridden_by_declared {
        result.ratio = None;
        result.reasons = vec![
            "overlapping declared region takes precedence; automatic checks suppressed".into(),
        ];
        return Ok(result);
    }
    if result.region.kind == "text" {
        let mut q = tq::legibility(
            &rgb,
            [0, 0, w, h],
            0,
            tq::Policy {
                minimum_contrast: c.threshold,
                ..Default::default()
            },
        )?;
        q.rect_px = result.region.rect_px;
        let v = match q.state {
            tq::State::Legible => Verdict::Pass,
            tq::State::Illegible if q.reasons.iter().any(|r| r == "low_contrast") => Verdict::Fail,
            // Automatic geometry/font proxies are heuristic, not a declared
            // minimum-size policy; keep them reviewable without a false FAIL.
            tq::State::Illegible => Verdict::Warn,
            _ => Verdict::Unmeasurable,
        };
        result.legibility_verdict = Some(v);
        let tofu = tq::tofu(
            &rgb,
            None,
            String::new(),
            None,
            None,
            tq::unavailable_ocr("detection only; no recognition requested"),
        )?;
        result.missing_glyphs = tofu
            .regions
            .into_iter()
            .map(|mut c| {
                c.rect_px[0] += x;
                c.rect_px[1] += y;
                c
            })
            .collect();
        if !result.missing_glyphs.is_empty() {
            result.glyph_verdict = Verdict::Warn;
        }
        result.reasons.extend(tofu.reasons);
        if c.verdict == "WARN" && v == Verdict::Fail {
            result.reasons.push(
                "displayed text is below the required contrast; source colour not determinable"
                    .into(),
            );
        }
        result.legibility = Some(q);
    }
    result.verdict = match result.ratio {
        Some(r) if r + 1e-12 >= c.threshold => Verdict::Pass,
        Some(_) if result.reasons.iter().any(|s| s.contains("lower bound")) => {
            Verdict::Unmeasurable
        }
        Some(_) => Verdict::Fail,
        None => Verdict::Unmeasurable,
    };
    Ok(result)
}
fn aggregate(values: impl Iterator<Item = Verdict>) -> Verdict {
    let values: Vec<_> = values.collect();
    if values.contains(&Verdict::Fail) {
        Verdict::Fail
    } else if values.contains(&Verdict::Warn) {
        Verdict::Warn
    } else if values.contains(&Verdict::Pass) {
        Verdict::Pass
    } else {
        Verdict::Unmeasurable
    }
}
impl Report {
    /// Stable text, with detector misses and region measurements made explicit.
    pub fn text(&self) -> String {
        let mut s = format!(
            "a11y auto: {}\n{}\npx-per-pt: {} (see scale assumption)\nscope: detected regions; detection completeness unknown\n",
            self.verdict, self.disclaimer, self.px_per_pt
        );
        for im in &self.images {
            s.push_str(&format!(
                "{} {} — {}\n",
                im.verdict,
                im.name,
                im.summary
                    .entries()
                    .iter()
                    .map(|(_, label, c)| format!("{label}: {} {}", c.status, c.detail))
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
            s.push_str(&format!("  {}; scope: {}\n", im.text_detection, im.scope));
            for limitation in &im.limitations {
                s.push_str(&format!("  limitation: {limitation}\n"));
            }
            for d in &im.detectors {
                s.push_str(&format!("  {} {}: {}\n", d.detector, d.state, d.reason));
            }
            for f in &im.automatic {
                s.push_str(&format!("  WCAG source-colour {} {} {:?}: ratio {:?}, requires {}:1; {} confidence {}; rendered legibility {:?} (ratio {:?}); glyphs {}\n",f.verdict,f.region.kind,f.region.rect_px,f.ratio,f.required_ratio,f.region.detector,f.region.confidence,f.legibility_verdict,f.legibility.as_ref().and_then(|q| q.contrast),f.glyph_verdict));
                for reason in f.reasons.iter().filter(|r| r.starts_with("displayed text")) {
                    s.push_str(&format!("    {reason}\n"));
                }
            }
            for c in &im.declared_and_colour_vision.contrast {
                s.push_str(&format!(
                    "  declared {} {}: ratio {:?}, requires {}:1\n",
                    c.verdict, c.name, c.ratio, c.threshold
                ));
            }
        }
        for a in &self.assumptions {
            s.push_str(&format!("assumption: {a}\n"));
        }
        s
    }
    /// Unknown/warning evidence is a skipped testcase, never a successful measurement.
    pub fn junit(&self, path: &Path, inputs: &[&Path]) -> Result<()> {
        let normal = saccade_core::run::normalise_path(path);
        if inputs
            .iter()
            .any(|p| normal.starts_with(saccade_core::run::normalise_path(p)))
            || std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
        {
            return Err(Error::Config(
                "JUnit destination must not overwrite inputs or follow symlinks".into(),
            ));
        }
        let xml = |s: &str| {
            s.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
                .replace('\'', "&apos;")
        };
        let mut body = String::from(
            "<?xml version=\"1.0\"?><testsuite name=\"automatic accessibility pre-check\">",
        );
        for im in &self.images {
            for (key, _, c) in im.summary.entries() {
                body.push_str(&format!(
                    "<testcase classname=\"{}\" name=\"{}\">",
                    xml(&im.name),
                    key
                ));
                match c.status {
                    Status::Fail => body.push_str("<failure message=\"measured failure\"/>"),
                    Status::Pass => {}
                    _ => body.push_str(&format!("<skipped message=\"{}\"/>", c.status)),
                }
                let note = format!(
                    "{}; scope: {}; summary: {}",
                    c.detail,
                    im.scope,
                    serde_json::to_string(c)?
                );
                body.push_str(&format!(
                    "<system-out>{}</system-out></testcase>",
                    xml(&note)
                ));
            }
        }
        body.push_str("</testsuite>\n");
        std::fs::write(path, body).map_err(crate::io_err("writing auto a11y JUnit".into()))
    }
}

/// Run all deterministic checks over an image/directory. No process exit or network calls.
pub fn run(input: &Path, out: &Path, options: &Options) -> Result<Report> {
    validate(options)?;
    let mut protected = vec![input];
    if let Some(c) = options.config.as_deref() {
        protected.push(c);
    }
    if let Some(c) = options.model_cache.as_deref() {
        protected.push(c);
    }
    saccade_core::run::guard_output_dir(out, &protected, &[FILE, a11y::FILE, ".saccade-precheck"])?;
    for leaf in [
        FILE,
        a11y::FILE,
        "index.html",
        "report.txt",
        "images",
        ".saccade-precheck",
    ] {
        if protected.iter().any(|p| {
            saccade_core::run::normalise_path(p)
                .starts_with(saccade_core::run::normalise_path(&out.join(leaf)))
        }) {
            return Err(Error::Config(
                "automatic artifacts would overwrite input/config/cache".into(),
            ));
        }
    }
    let automatic = out.join(FILE);
    if std::fs::symlink_metadata(&automatic).is_ok() {
        std::fs::remove_file(&automatic)
            .map_err(crate::io_err("removing stale automatic report".into()))?;
    }
    let declared = a11y::run(
        input,
        out,
        &a11y::Options {
            config: options.config.clone(),
            ..Default::default()
        },
    )?;
    #[allow(unused_mut)]
    let mut model_status = DetectorStatus {
        detector: "PP-OCRv5-mobile-det/pinned".into(),
        state: "unavailable".into(),
        reason: "ocr feature or provisioned model cache absent; no downloads attempted; next action: saccade models pull runtime and saccade models pull ocr".into(),
    };
    #[cfg(feature="ocr")]
    let mut model=options.model_cache.as_deref().and_then(|cache|match saccade_core::general::ocr::Detector::load(cache) {
        Ok(d)=>{model_status.state="available".into();model_status.reason="verified built-in SHA-256; DB mean >=0.6; score is a support lower bound, uncalibrated".into();Some(d)},
        Err(_)=>{model_status.reason="provisioned pinned detector/runtime unavailable or invalid; no downloads attempted; next action: saccade models pull runtime and saccade models pull ocr".into();None}
    });
    std::fs::write(
        out.join(".saccade-precheck"),
        b"incomplete automatic pre-check\n",
    )
    .map_err(crate::io_err("writing automatic sentinel".into()))?;
    let mut report=Report {schema:SCHEMA.into(),report_id:None,source_refs:vec![],verdict:Verdict::Unmeasurable,scope:"detected regions; detection completeness unknown".into(),disclaimer:saccade_core::safety::output::DISCLAIMER.into(),level:options.level,px_per_pt:options.px_per_pt,assumptions:vec!["Upper-quartile ascender-height letter body (small detached marks excluded), capped by detected height, / px-per-pt >=18 assumes large text; font size, bold and display scaling are not observed. Without explicit scale, scale unknown: normal-text threshold applied.".into(),"Automatic regions are candidates, not WCAG applicability or complete detection. Fallback misses short, tiny, rotated, joined or textured text; closed-edge UI candidates omit many icons/controls.".into(),"Glyph checks report WARN/NOT_VERIFIED, never font completeness. CVD information loss is a candidate warning with no semantic inference.".into()],images:vec![]};
    for im in declared.images {
        let image = saccade_core::safety::opaque(&out.join(&im.original))?;
        let mut regions = fallback(&image)?;
        #[allow(unused_mut)]
        let mut status = model_status.clone();
        #[cfg(feature = "ocr")]
        if let Some(d) = model.as_mut() {
            match d.detect(&image) {
                Ok(quads) => {
                    for q in quads {
                        let x = q
                            .iter()
                            .map(|p| p[0])
                            .fold(f32::INFINITY, f32::min)
                            .max(0.)
                            .floor() as u32;
                        let y = q
                            .iter()
                            .map(|p| p[1])
                            .fold(f32::INFINITY, f32::min)
                            .max(0.)
                            .floor() as u32;
                        let right = (q.iter().map(|p| p[0]).fold(0., f32::max).ceil() as u32)
                            .min(image.width());
                        let bottom = (q.iter().map(|p| p[1]).fold(0., f32::max).ceil() as u32)
                            .min(image.height());
                        if right > x && bottom > y {
                            let r = region(
                                "text",
                                [x, y, right - x, bottom - y],
                                image.dimensions(),
                                &status.detector,
                                0.6,
                            );
                            regions.retain(|old| {
                                old.detector != FALLBACK || !overlap(old.rect_px, r.rect_px)
                            });
                            regions.push(r);
                        }
                    }
                }
                Err(_) => {
                    status.state = "unavailable".into();
                    status.reason = "pinned detector inference failed; fallback retained; next action: saccade models pull runtime and saccade models pull ocr".into();
                }
            }
        }
        let text_boxes: Vec<_> = regions
            .iter()
            .filter(|r| r.kind == "text")
            .map(|r| r.rect_px)
            .collect();
        // Text inside a control could conceal a failing boundary in a two-colour fit.
        regions.retain(|r| r.kind != "ui" || !text_boxes.iter().any(|b| overlap(*b, r.rect_px)));
        let text_count = text_boxes.len();
        let detectors=vec![status,DetectorStatus {detector:FALLBACK.into(),state:"available".into(),reason:"three or more aligned similar-height edge/stroke components; confidence 0.7 is a heuristic score".into()},DetectorStatus {detector:UI.into(),state:"available".into(),reason:"compact boundaries with >=70% support on each of four sides; confidence 0.75 is heuristic".into()}];
        let text_detection = if text_count == 0 {
            format!(
                "no text detected by {FALLBACK} and PP-OCRv5-mobile-det/pinned (see availability)"
            )
        } else {
            format!("{text_count} candidate text regions detected; completeness unknown")
        };
        let mut automatic = Vec::new();
        for mut r in regions {
            r.overridden_by_declared = im.contrast.iter().any(|c| {
                saccade_core::regions::resolve_rect(c.rect, image.width(), image.height())
                    .is_some_and(|b| overlap(b, r.rect_px))
            });
            automatic.push(measure(&image, r, options)?);
        }
        let summary = Summary::build(&automatic, &im, options.level);
        let verdict = summary.verdict(text_count > 0);
        let mut limitations = vec![
            "detection completeness unknown".into(),
            "glyph completeness unverified".into(),
            "semantic colour information completeness unverified".into(),
        ];
        for (key, _, c) in summary.entries() {
            if c.counts.unmeasurable > 0 || c.counts.not_verified > 0 {
                limitations.push(format!(
                    "{key}: {} unmeasurable, {} not verified regions",
                    c.counts.unmeasurable, c.counts.not_verified
                ));
            }
        }
        report.images.push(ImageReport {
            name: im.name.clone(),
            verdict,
            summary,
            scope: "detected regions; detection completeness unknown".into(),
            limitations,
            detectors,
            text_detection,
            automatic,
            declared_and_colour_vision: im,
        });
    }
    report.verdict = aggregate(report.images.iter().map(|i| i.verdict));
    // Separate report artifact and readable HTML; declared report remains intact.
    let linked = saccade_core::report_links::write(&out.join(FILE), &report)?;
    let text = report.text();
    std::fs::write(out.join("report.txt"), &text)
        .map_err(crate::io_err("writing auto a11y text".into()))?;
    let safe = serde_json::to_string(&linked)?
        .replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e");
    let html = format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>Automatic accessibility pre-check</title><h1>Automatic accessibility pre-check</h1><pre id=\"report\"></pre><script type=\"application/json\" id=\"data\">{safe}</script><script>document.getElementById('report').textContent=JSON.stringify(JSON.parse(document.getElementById('data').textContent),null,2);</script></html>"
    );
    std::fs::write(out.join("index.html"), html)
        .map_err(crate::io_err("writing auto a11y HTML".into()))?;
    std::fs::remove_file(out.join(".saccade-precheck"))
        .map_err(crate::io_err("completing automatic report".into()))?;
    Ok(report)
}
