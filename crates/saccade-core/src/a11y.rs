//! Accessibility pre-check: Machado CVD, CIEDE2000 information loss and
//! user-declared WCAG contrast regions. No certification or compliance claim.

use crate::safety::{color, opaque, output, thresholds};
use crate::{Error, Result};
use image::RgbImage;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Report marker.
pub const FILE: &str = "saccade-a11y.v1.json";
const MODES: [&str; 6] = [
    "protanopia",
    "deuteranopia",
    "tritanopia",
    "protanomaly",
    "deuteranomaly",
    "tritanomaly",
];

/// A user-confirmed region; coordinates are image fractions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    /// Name in reports.
    pub name: String,
    /// `text` or `ui`; only explicitly declared regions are checked.
    pub kind: String,
    /// Fractional `[x,y,w,h]` wholly inside the image.
    pub rect: [f64; 4],
    /// Optional relative-name glob.
    #[serde(default)]
    pub glob: Option<String>,
    /// User declares text size >= 18pt, or >= 14pt bold.
    #[serde(default)]
    pub large: bool,
    /// AA (default) or AAA. UI always uses SC 1.4.11's 3:1.
    #[serde(default = "aa")]
    pub level: String,
}
fn aa() -> String {
    "AA".into()
}

/// Information-loss sampling scale, in pixels (block means, default 4).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// Compare horizontal/vertical adjacent blocks at this pixel scale.
    #[serde(default = "scale")]
    pub scale: u32,
}
fn scale() -> u32 {
    4
}
impl Default for Settings {
    fn default() -> Self {
        Self { scale: scale() }
    }
}

/// Optional config and explicit AI consent; no network by default.
#[derive(Debug, Default, Clone)]
pub struct Options {
    /// Explicit saccade.toml; region entries without `kind` belong to compare.
    pub config: Option<PathBuf>,
    /// Only true when the user explicitly requests image upload for proposals.
    pub suggest_regions: bool,
    /// Same key-directory policy as judge; never ambient API keys.
    pub keys_dir: Option<PathBuf>,
}

/// Candidate colour information loss; this requires human task interpretation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// `information_loss`.
    pub kind: String,
    /// WARN for a candidate; no automatic semantic FAIL from pixel neighbours.
    pub severity: String,
    /// Colour-vision display mode.
    pub simulation: String,
    /// Fractional bounding box of connected affected blocks.
    pub rect: [f64; 4],
    /// Largest original CIEDE2000 in the connected component.
    pub original_delta_e: f64,
    /// Smallest simulated CIEDE2000 in the component.
    pub simulated_delta_e: f64,
    /// Plain-language interpretation and limitation.
    pub message: String,
}

/// Measured contrast in one confirmed region.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contrast {
    /// User's region name.
    pub name: String,
    /// User's region kind.
    pub kind: String,
    /// Fractional region.
    pub rect: [f64; 4],
    /// PASS, WARN (unmeasurable) or FAIL.
    pub verdict: String,
    /// Estimated foreground sRGB bytes, normally the minority cluster.
    pub foreground: Option<[u8; 3]>,
    /// Estimated background sRGB bytes, normally the majority cluster.
    pub background: Option<[u8; 3]>,
    /// WCAG ratio; null if the two-colour model is not supported.
    pub ratio: Option<f64>,
    /// Required ratio for the declared region and level.
    pub threshold: f64,
    /// Requested AA or AAA.
    pub level: String,
    /// Method and limits.
    pub note: String,
}

/// AI proposals never change config or participate in contrast verdicts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    /// `text` or `ui`.
    pub kind: String,
    /// Fractional box (coarse 4x4 candidate grid).
    pub rect: [f64; 4],
    /// Always false: copy into config only after human confirmation.
    pub confirmed: bool,
    /// Model that actually answered, including fallback.
    pub model: String,
    /// Provider probability, if given.
    pub probability: Option<f64>,
}

/// One input and all generated display modes/evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageReport {
    /// Relative input name.
    pub name: String,
    /// Pre-check verdict for this image.
    pub verdict: String,
    /// Original artifact.
    pub original: String,
    /// Six severity-1.0 simulation artifacts.
    pub simulations: BTreeMap<String, String>,
    /// Per-mode information-loss maps (anomalous aliases share artifacts).
    pub heatmaps: BTreeMap<String, String>,
    /// Connected information-loss candidates.
    pub findings: Vec<Finding>,
    /// Checks on confirmed config regions only.
    pub contrast: Vec<Contrast>,
    /// Explicitly requested Gemini proposals, awaiting confirmation.
    pub proposals: Vec<Proposal>,
}

/// Full JSON/HTML/text model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A11yReport {
    /// `saccade-a11y.v1`.
    pub schema: String,
    /// PASS, WARN or FAIL.
    pub verdict: String,
    /// Required framing.
    pub disclaimer: String,
    /// Machado model citation and severity.
    pub model: String,
    /// Pixel block scale of neighbouring-colour comparisons.
    pub scale: u32,
    /// Same single numeric criterion table as safety.
    pub thresholds: Vec<thresholds::Threshold>,
    /// Approximation and uncovered-region notes.
    pub warnings: Vec<String>,
    /// Per-image results, sorted by portable relative name.
    pub images: Vec<ImageReport>,
}

impl A11yReport {
    /// Stable CLI text, with the pre-check disclaimer.
    pub fn text(&self) -> String {
        let mut s = format!(
            "a11y: {} ({} images)\n{}\n",
            self.verdict,
            self.images.len(),
            self.disclaimer
        );
        for image in &self.images {
            s.push_str(&format!("{} {}: {} information-loss findings, {} contrast checks, {} unconfirmed proposals\n", image.verdict,image.name,image.findings.len(),image.contrast.len(),image.proposals.len()));
            for c in &image.contrast {
                s.push_str(&format!(
                    "  {} {}: {} (requires {:.1}:1 {})\n",
                    c.verdict,
                    c.name,
                    c.ratio
                        .map_or_else(|| "unmeasurable".into(), |r| format!("{r:.3}:1")),
                    c.threshold,
                    c.level
                ));
            }
        }
        for warning in &self.warnings {
            s.push_str(&format!("warning: {warning}\n"));
        }
        s
    }
    /// Each confirmed region is a testcase; image candidates are warnings.
    pub fn junit_cases(&self) -> Vec<(String, bool, String)> {
        self.images
            .iter()
            .flat_map(|im| {
                let mut cases = vec![(
                    im.name.clone(),
                    false,
                    format!(
                        "{}; {} information-loss candidates. {}",
                        im.verdict,
                        im.findings.len(),
                        self.disclaimer
                    ),
                )];
                cases.extend(im.contrast.iter().map(|c| {
                    (
                        format!("{}:{}", im.name, c.name),
                        c.verdict == "FAIL",
                        format!(
                            "{} ratio {:?}, requires {}:1. {}",
                            c.verdict, c.ratio, c.threshold, self.disclaimer
                        ),
                    )
                }));
                cases
            })
            .collect()
    }
}

fn configuration(path: Option<&Path>) -> Result<(Settings, Vec<Region>)> {
    let Some(path) = path else {
        return Ok((Settings::default(), Vec::new()));
    };
    let text =
        std::fs::read_to_string(path).map_err(crate::run::io_err("reading a11y config".into()))?;
    // Other saccade config tables are preserved/ignored, not reinterpreted.
    let doc: toml::Value =
        toml::from_str(&text).map_err(|e| Error::Config(format!("a11y TOML: {e}")))?;
    let settings: Settings = doc
        .get("a11y")
        .cloned()
        .map(|v| v.try_into())
        .transpose()
        .map_err(|e| Error::Config(format!("a11y settings: {e}")))?
        .unwrap_or_default();
    if settings.scale == 0 || settings.scale > 128 {
        return Err(Error::Config("a11y.scale must be in 1..=128".into()));
    }
    let mut regions = Vec::new();
    if let Some(v) = doc.get("region") {
        let list = v
            .as_array()
            .ok_or_else(|| Error::Config("region must be [[region]] tables".into()))?;
        for value in list {
            if value.get("kind").is_none() {
                continue;
            }
            let mut r: Region = value
                .clone()
                .try_into()
                .map_err(|e| Error::Config(format!("a11y region: {e}")))?;
            r.level.make_ascii_uppercase();
            if r.name.is_empty()
                || !matches!(r.kind.as_str(), "text" | "ui")
                || !matches!(r.level.as_str(), "AA" | "AAA")
                || !r
                    .rect
                    .iter()
                    .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
                || r.rect[2] <= 0.0
                || r.rect[3] <= 0.0
                || r.rect[0] + r.rect[2] > 1.0 + 1e-9
                || r.rect[1] + r.rect[3] > 1.0 + 1e-9
            {
                return Err(Error::Config("a11y regions require a name, kind text/ui, AA/AAA, and a nonempty fractional rect wholly inside the image".into()));
            }
            if let Some(glob) = &r.glob {
                crate::config::compile_glob(glob)?;
            }
            if regions
                .iter()
                .any(|old: &Region| old.name == r.name && old.glob == r.glob)
            {
                return Err(Error::Config("duplicate a11y region name/glob".into()));
            }
            regions.push(r);
        }
    }
    Ok((settings, regions))
}

/// WCAG SC 1.4.3/1.4.6 ratio of two linear RGB colours.
pub fn contrast_ratio(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (a, b) = (color::luminance(a), color::luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn contrast(image: &RgbImage, region: &Region) -> Result<Contrast> {
    let [x, y, w, h] = crate::regions::resolve_rect(region.rect, image.width(), image.height())
        .ok_or_else(|| Error::Config("contrast region resolves to zero pixels".into()))?;
    let threshold = thresholds::value(if region.kind == "ui" {
        "contrast_ui"
    } else {
        match (region.level.as_str(), region.large) {
            ("AAA", true) => "contrast_aaa_large",
            ("AAA", false) => "contrast_aaa_text",
            (_, true) => "contrast_aa_large",
            _ => "contrast_aa_text",
        }
    });
    let mut result = Contrast { name: region.name.clone(),kind: region.kind.clone(),rect: region.rect,verdict: "WARN".into(),foreground: None,background: None,ratio: None,threshold,level: region.level.clone(),note: "Deterministic two-cluster fit with component medians; minority is estimated foreground. No OCR, font-size inference, or automatic WCAG applicability judgement.".into() };
    let pixels: Vec<_> = (y..y + h)
        .flat_map(|y| (x..x + w).map(move |x| color::rgb(image.get_pixel(x, y).0)))
        .collect();
    if pixels.len() < 2 {
        result.note.push_str(" Insufficient pixels.");
        return Ok(result);
    }
    let dist =
        |a: [f64; 3], b: [f64; 3]| a.iter().zip(b).map(|(a, b)| (a - b).powi(2)).sum::<f64>();
    // Mode in 5-bit/channel histogram seeds background; farthest real pixel
    // seeds foreground. Medians limit antialias/outlier influence.
    let mut hist: BTreeMap<[u8; 3], (usize, [f64; 3])> = BTreeMap::new();
    for &p in &pixels {
        let bucket = color::bytes(p).map(|v| v / 8);
        let e = hist.entry(bucket).or_insert((0, p));
        e.0 += 1;
    }
    let seed = hist.values().max_by_key(|v| v.0).map_or(pixels[0], |v| v.1);
    let far = pixels
        .iter()
        .max_by(|a, b| dist(**a, seed).total_cmp(&dist(**b, seed)))
        .copied()
        .unwrap_or(seed);
    let mut centres = [seed, far];
    let mut clusters = [Vec::new(), Vec::new()];
    for _ in 0..12 {
        clusters.iter_mut().for_each(Vec::clear);
        for &p in &pixels {
            let i = usize::from(dist(p, centres[1]) < dist(p, centres[0]));
            clusters[i].push(p);
        }
        if clusters.iter().any(Vec::is_empty) {
            result.note.push_str(" No two supported dominant colours.");
            return Ok(result);
        }
        let next = std::array::from_fn(|i| {
            std::array::from_fn(|c| {
                let mut channel: Vec<_> = clusters[i].iter().map(|p| p[c]).collect();
                channel.sort_by(f64::total_cmp);
                channel[channel.len() / 2]
            })
        });
        if next == centres {
            break;
        }
        centres = next;
    }
    let minority = usize::from(clusters[1].len() < clusters[0].len());
    let coverage = clusters[minority].len() as f64 / pixels.len() as f64;
    let residual = clusters
        .iter()
        .enumerate()
        .map(|(i, ps)| ps.iter().map(|&p| dist(p, centres[i])).sum::<f64>())
        .sum::<f64>()
        / pixels.len() as f64;
    if coverage < 0.01 || residual.sqrt() > 0.08 {
        result.note.push_str(" Weak minority support (<1%) or diffuse colours (linear RGB RMS >0.08); confirm a tighter two-colour region.");
        return Ok(result);
    }
    let ratio = contrast_ratio(centres[0], centres[1]);
    result.ratio = Some(ratio);
    result.foreground = Some(color::bytes(centres[minority]));
    result.background = Some(color::bytes(centres[1 - minority]));
    result.verdict = if ratio + 1e-12 >= threshold {
        "PASS"
    } else {
        "FAIL"
    }
    .into();
    Ok(result)
}

fn information_loss(image: &RgbImage, scale: u32, kind: usize) -> (Vec<Finding>, RgbImage) {
    let (w, h) = (
        image.width().div_ceil(scale) as usize,
        image.height().div_ceil(scale) as usize,
    );
    let mut colors = vec![[0.0; 3]; w * h];
    let mut counts = vec![0u32; w * h];
    for (x, y, p) in image.enumerate_pixels() {
        let i = (y / scale) as usize * w + (x / scale) as usize;
        let c = color::rgb(p.0);
        for (dst, v) in colors[i].iter_mut().zip(c) {
            *dst += v;
        }
        counts[i] += 1;
    }
    for (c, n) in colors.iter_mut().zip(counts) {
        for v in c {
            *v /= f64::from(n);
        }
    }
    let orig: Vec<_> = colors.iter().map(|&c| color::lab(c)).collect();
    let sim: Vec<_> = colors
        .iter()
        .map(|&c| color::lab(color::simulate(c, kind)))
        .collect();
    let mut loss = vec![false; w * h];
    let mut before: Vec<f64> = vec![0.0; w * h];
    let mut after: Vec<f64> = vec![f64::INFINITY; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            for j in [(x + 1 < w).then_some(i + 1), (y + 1 < h).then_some(i + w)]
                .into_iter()
                .flatten()
            {
                let (a, b) = (
                    color::delta_e(orig[i], orig[j]),
                    color::delta_e(sim[i], sim[j]),
                );
                if a >= thresholds::value("cvd_distinct_delta_e")
                    && b < thresholds::value("cvd_jnd_delta_e")
                {
                    for p in [i, j] {
                        loss[p] = true;
                        before[p] = before[p].max(a);
                        after[p] = after[p].min(b);
                    }
                }
            }
        }
    }
    let heat = RgbImage::from_fn(image.width(), image.height(), |x, y| {
        if loss[(y / scale) as usize * w + (x / scale) as usize] {
            image::Rgb([255, 110, 20])
        } else {
            image::Rgb([12, 16, 24])
        }
    });
    let mut visited = vec![false; w * h];
    let mut findings = Vec::new();
    for i in 0..loss.len() {
        if !loss[i] || visited[i] {
            continue;
        }
        let mut queue = VecDeque::from([i]);
        visited[i] = true;
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
        let (mut a, mut b) = (0f64, f64::INFINITY);
        while let Some(p) = queue.pop_front() {
            let (x, y) = (p % w, p / w);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x + 1);
            y1 = y1.max(y + 1);
            a = a.max(before[p]);
            b = b.min(after[p]);
            for q in [
                (x > 0).then(|| p - 1),
                (x + 1 < w).then_some(p + 1),
                (y > 0).then(|| p - w),
                (y + 1 < h).then_some(p + w),
            ]
            .into_iter()
            .flatten()
            {
                if loss[q] && !visited[q] {
                    visited[q] = true;
                    queue.push_back(q);
                }
            }
        }
        let (fw, fh) = (f64::from(image.width()), f64::from(image.height()));
        let rect = [
            x0 as f64 * f64::from(scale) / fw,
            y0 as f64 * f64::from(scale) / fh,
            ((x1 as u32 * scale).min(image.width()) - x0 as u32 * scale) as f64 / fw,
            ((y1 as u32 * scale).min(image.height()) - y0 as u32 * scale) as f64 / fh,
        ];
        findings.push(Finding { kind: "information_loss".into(),severity: "WARN".into(),simulation: MODES[kind].into(),rect,original_delta_e:a,simulated_delta_e:b,message:format!("Distinct neighbouring colours may become indistinguishable under {} (original ΔE00 {a:.2}, simulated {b:.2}); e.g. a red/green status indicator. Confirm that colour conveys information; no semantics inferred.",MODES[kind]) });
    }
    (findings, heat)
}

fn proposals(image: &RgbImage, keys_dir: Option<PathBuf>) -> Result<Vec<Proposal>> {
    use crate::judge_provider::{AskRequest, Backend, Keys, LiveBackend, Prompt, Retry};
    // Parse the repo's panel, so fallback models and key policy cannot drift
    // into an independently maintained a11y provider implementation.
    let panel = crate::judge::Panel::parse(include_str!("../../../examples/panel.toml"))?;
    let spec = panel
        .judges
        .iter()
        .find(|s| s.provider == crate::judge::Provider::Gemini && s.vision)
        .ok_or_else(|| Error::Config("judge panel has no Gemini vision chain".into()))?;
    let keys = Keys::new(keys_dir);
    keys.for_spec(spec).map_err(Error::Config)?;
    let backend = LiveBackend::new(
        keys,
        Retry::default(),
        Duration::from_secs(u64::from(spec.timeout_secs)),
    );
    let answers = vec!["text".into(), "ui".into(), "none".into(), "abstain".into()];
    let mut proposals = Vec::new();
    // 16 explicitly uploaded crops per image; proposals are coarse bounds,
    // never automatic checks or config writes. Fixed grid keeps answers closed.
    for row in 0..4 {
        for col in 0..4 {
            let rect = [f64::from(col) / 4.0, f64::from(row) / 4.0, 0.25, 0.25];
            let Some([x, y, w, h]) =
                crate::regions::resolve_rect(rect, image.width(), image.height())
            else {
                continue;
            };
            let crop = image::imageops::crop_imm(image, x, y, w, h).to_image();
            let mut png = Cursor::new(Vec::new());
            crop.write_to(&mut png, image::ImageFormat::Png)
                .map_err(|e| Error::Config(format!("encoding vision crop: {e}")))?;
            let images = vec![png.into_inner()];
            let state = serde_json::json!({"candidate_rect":rect});
            let prompt=Prompt { system:"Classify a screenshot crop for candidate accessibility regions. This is a proposal only, never a compliance judgement.".into(),user:"Does this crop contain likely text, an essential UI control, or neither? Choose text, ui, none or abstain. Respond with JSON answer and probability. The user must confirm and refine the candidate box in config.".into() };
            let outcome = backend.ask(&AskRequest {
                spec,
                question: "a11y_region_proposal",
                question_text: &prompt.user,
                kind: "rubric",
                wire_answers: &answers,
                state: &state,
                prompt: &prompt,
                images: &images,
            });
            let raw = outcome
                .result
                .map_err(|e| Error::Config(format!("Gemini region proposals failed: {e}")))?;
            if matches!(raw.answer.as_str(), "text" | "ui") {
                proposals.push(Proposal {
                    kind: raw.answer,
                    rect,
                    confirmed: false,
                    model: raw.model,
                    probability: raw.prob,
                });
            }
        }
    }
    Ok(proposals)
}

/// Analyze an image or recursively collected directory and write artifacts.
pub fn run(input: &Path, out: &Path, options: &Options) -> Result<A11yReport> {
    let root = crate::paths::canonicalize(input)
        .map_err(crate::run::io_err("opening a11y input".into()))?;
    let files = if root.is_dir() {
        let collected = crate::run::collect_images(&root)?;
        if !collected.problems.is_empty() {
            return Err(Error::Config(format!(
                "unreadable/symlink a11y inputs: {:?}",
                collected.problems
            )));
        }
        collected.files
    } else {
        BTreeMap::from([(
            root.file_name()
                .map_or_else(|| "image".into(), |n| n.to_string_lossy().into_owned()),
            root.clone(),
        )])
    };
    if files.is_empty() {
        return Err(Error::Config("a11y input contains no images".into()));
    }
    let (settings, regions) = configuration(options.config.as_deref())?;
    let mut protected = vec![input];
    if let Some(config) = options.config.as_deref() {
        protected.push(config);
    }
    output::prepare(out, &protected, FILE)?;
    let mut report=A11yReport {schema:"saccade-a11y.v1".into(),verdict:"PASS".into(),disclaimer:output::DISCLAIMER.into(),model:"Machado, Oliveira & Fernandes (2009), IEEE TVCG 15(6), doi:10.1109/TVCG.2009.113; severity 1.0, linear RGB; anomalous variants equal dichromacy endpoints".into(),scale:settings.scale,thresholds:thresholds::table(),warnings:vec!["Information-loss candidates compare adjacent linear-RGB block means with CIEDE2000; colour task semantics and small indicators below the chosen scale need human review.".into(),"Contrast is a two-colour cluster estimate, not OCR. Antialiasing, gradients, multiple colours, contextual UI boundaries and undeclared regions can invalidate or escape checks.".into()],images:Vec::new()};
    if regions.is_empty() {
        report.warnings.push("No text/UI regions declared: contrast was not checked. PASS applies only to the performed pixel pre-check.".into());
    }
    if options.suggest_regions {
        report.warnings.push("Explicit AI assist: 16 coarse crop classifications/image sent through the judge Gemini chain. All boxes are unconfirmed proposals excluded from verdicts.".into());
    }
    let mut matched = vec![false; regions.len()];
    for (index, (name, path)) in files.iter().enumerate() {
        let image = opaque(path)?;
        let mut im = ImageReport {
            name: name.clone(),
            verdict: "PASS".into(),
            original: output::image(out, &format!("image-{index:06}"), &image)?,
            simulations: BTreeMap::new(),
            heatmaps: BTreeMap::new(),
            findings: Vec::new(),
            contrast: Vec::new(),
            proposals: Vec::new(),
        };
        for kind in 0..3 {
            let simulation = RgbImage::from_fn(image.width(), image.height(), |x, y| {
                image::Rgb(color::bytes(color::simulate(
                    color::rgb(image.get_pixel(x, y).0),
                    kind,
                )))
            });
            let artifact = output::image(
                out,
                &format!("image-{index:06}-{}", MODES[kind]),
                &simulation,
            )?;
            im.simulations.insert(MODES[kind].into(), artifact.clone());
            im.simulations.insert(MODES[kind + 3].into(), artifact);
            let (findings, heat) = information_loss(&image, settings.scale, kind);
            im.findings.extend(findings);
            let heat = output::image(
                out,
                &format!("image-{index:06}-{}-loss", MODES[kind]),
                &heat,
            )?;
            im.heatmaps.insert(MODES[kind].into(), heat.clone());
            im.heatmaps.insert(MODES[kind + 3].into(), heat);
        }
        for (i, r) in regions.iter().enumerate() {
            if r.glob
                .as_ref()
                .map(|g| crate::config::compile_glob(g).map(|m| m.is_match(name)))
                .transpose()?
                .unwrap_or(true)
            {
                matched[i] = true;
                im.contrast.push(contrast(&image, r)?);
            }
        }
        if options.suggest_regions {
            im.proposals = proposals(&image, options.keys_dir.clone())?;
        }
        if im.contrast.iter().any(|c| c.verdict == "FAIL") {
            im.verdict = "FAIL".into();
        } else if !im.findings.is_empty() || im.contrast.iter().any(|c| c.verdict == "WARN") {
            im.verdict = "WARN".into();
        }
        report.images.push(im);
    }
    for (i, m) in matched.into_iter().enumerate() {
        if !m {
            return Err(Error::Config(format!(
                "declared a11y region {:?} matched no input image",
                regions[i].name
            )));
        }
    }
    if report.images.iter().any(|i| i.verdict == "FAIL") {
        report.verdict = "FAIL".into();
    } else if report.images.iter().any(|i| i.verdict == "WARN") {
        report.verdict = "WARN".into();
    }
    output::finish(
        out,
        FILE,
        &report,
        &report.text(),
        "Accessibility pre-check",
    )?;
    Ok(report)
}
