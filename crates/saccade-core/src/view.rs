//! The review viewer model (`saccade view`, see `docs/design.md`) and the decisions file
//! it exports. Historical viewer finals are readable but cannot authorize updates.
//!
//! [`build_view`] pairs 2 to 6 directories by relative path, compares every
//! non-reference image against the reference with FLIP, copies the images under
//! `<out>/images/`, and writes a self-contained `<out>/index.html`.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::compare::{CompareOptions, compare_rgba};
use crate::error::{Error, Result};
use crate::render::write_view_html;
use crate::report::Metrics;
use crate::run::{collect_images, copy_into_report, decode, io_err};

/// Schema tag of the view model embedded in `index.html`.
pub const VIEW_SCHEMA: &str = "saccade-view.v1";
/// Schema tag of the decisions file exported by the viewer.
pub const DECISIONS_SCHEMA: &str = "saccade-decisions.v1";

/// Schema tag of the blind key written next to a `--blind` view.
pub const BLIND_KEY_SCHEMA: &str = "saccade-blind-key.v1";
/// File name of the blind key inside the view directory. The page never
/// references it; the judge loads it through a file picker to reveal labels.
pub const BLIND_KEY_FILE: &str = "blind-key.json";

/// Minimum and maximum number of directories a view accepts.
pub const MIN_DIRS: usize = 2;
/// See [`MIN_DIRS`].
pub const MAX_DIRS: usize = 6;

/// Small file `build_view` writes first into its output directory: the
/// marker that makes the directory recognisably saccade's own, so a later
/// run may replace its contents (and nothing else is ever overwritten).
pub const VIEW_MARKER_FILE: &str = "saccade-view.v1.json";

/// Largest seed the viewer can round-trip through a JavaScript number (2^53 - 1).
pub const MAX_SEED: u64 = (1 << 53) - 1;

/// Options for [`build_view`].
#[derive(Debug, Clone)]
pub struct ViewOptions {
    /// One label per directory; `None` derives them from the directory names.
    pub labels: Option<Vec<String>>,
    /// The FLIP reference: a label or a directory path. `None` is the first.
    pub reference: Option<String>,
    /// Pairwise judging: every set gets its own pane order, panes carry
    /// neutral positional labels and random file names, and no FLIP data,
    /// order, reference or seed is embedded, so the page cannot reveal which
    /// directory is which. `reference` is ignored.
    pub blind: bool,
    /// Shuffle seed. `None` draws one from the operating system's randomness
    /// in blind mode and from the clock otherwise.
    pub seed: Option<u64>,
    /// Where a blind view's key goes (default: `blind-key.json` inside the
    /// view directory, which must then be kept from the judge).
    pub key_out: Option<PathBuf>,
    /// FLIP pixels per degree.
    pub pixels_per_degree: f32,
    /// HDR-FLIP settings for `.exr`/`.hdr` images.
    pub hdr: crate::hdr::HdrConfig,
    /// Config regions, offered in the viewer as preset ROIs.
    pub regions: Vec<crate::regions::RegionSpec>,
    /// Metadata-sidecar name and ignore list (`required`/`declared` are not
    /// used: a view gives no verdict).
    pub meta: crate::meta::MetaOptions,
    /// Diagnostics engine settings (class, tone and shift findings, signed
    /// difference and non-finite mask images, timing deltas). Not run for a
    /// blind view.
    pub diagnostics: crate::diagnostics::DiagnosticsConfig,
    /// Run-level performance evidence.
    pub perf: crate::perf::PerfOptions,
    /// Name globs to include; empty includes all.
    pub entries: Vec<String>,
    /// Opt in to absolute source paths.
    pub record_absolute_paths: bool,
}

impl Default for ViewOptions {
    fn default() -> Self {
        Self {
            labels: None,
            reference: None,
            blind: false,
            seed: None,
            key_out: None,
            pixels_per_degree: CompareOptions::default().pixels_per_degree,
            hdr: crate::hdr::HdrConfig::default(),
            regions: Vec::new(),
            meta: crate::meta::MetaOptions::default(),
            diagnostics: crate::diagnostics::DiagnosticsConfig::default(),
            perf: crate::perf::PerfOptions::default(),
            entries: Vec::new(),
            record_absolute_paths: false,
        }
    }
}

/// One image of an image set, from one directory.
#[derive(Debug, Clone, Serialize)]
pub struct ViewPane {
    /// Image path relative to the view directory; `None` when the directory
    /// has no image of this name.
    pub path: Option<String>,
    /// Image width in pixels (0 when missing or undecodable).
    pub width: u32,
    /// Image height in pixels (0 when missing or undecodable).
    pub height: u32,
    /// FLIP heatmap against the reference (non-reference panes only).
    pub heatmap: Option<String>,
    /// Downsampling factor of this pane's inspector pixel data (1 = full
    /// resolution). The data itself lives in the set's lazily loaded
    /// `pixels_script`, never in the page.
    pub inspector_scale: u32,
    /// Structural checks of this image (black, white, non-finite samples).
    pub properties: Option<crate::report::Properties>,
    /// Non-fatal observations about this image: all black, all white,
    /// non-finite samples.
    pub warnings: Vec<String>,
    /// FLIP statistics against the reference.
    pub metrics: Option<Metrics>,
    /// Where the FLIP error against the reference is concentrated, largest
    /// first (same fields and defaults as a report entry's hotspots).
    pub hotspots: Vec<crate::report::Hotspot>,
    /// Why this pane could not be compared or decoded.
    pub error: Option<String>,
    /// Sidecar keys that differ from the reference directory's (`baseline` is
    /// the reference, `capture` this pane). Empty for the reference pane and
    /// in blind mode, where a value could reveal which side is which.
    pub meta_diff: Vec<crate::report::MetaDiff>,
    /// Why the sidecars of this pane or the reference could not be read.
    pub meta_error: Option<String>,
    /// SHA-256 (hex) of this pane's image file; recorded in the decisions so
    /// `approve` can tell the files it copies are the ones that were judged.
    pub sha256: Option<String>,
    /// Why this pane differs from the reference (non-reference panes of a
    /// non-blind view, when diagnostics are enabled).
    pub diagnostics: Option<crate::diagnostics::Diagnostics>,
    /// Signed luminance difference against the reference (blue darker, orange
    /// brighter), relative to the view directory.
    pub signed_diff: Option<String>,
    /// Mask of non-finite samples of an HDR pane, relative to the view directory.
    pub nonfinite_mask: Option<String>,
}

/// All panes sharing one relative path.
#[derive(Debug, Clone, Serialize)]
pub struct ViewSet {
    /// Relative image path, `/`-separated.
    pub name: String,
    /// Display order of pane indices (shuffled in blind mode).
    pub order: Vec<usize>,
    /// One pane per directory, in directory order.
    pub panes: Vec<ViewPane>,
    /// Script that defines per-pane data URIs, loaded only when the browser
    /// refuses to read pixels from the image files (canvas taint on `file://`).
    pub pixels_script: String,
    /// How the set compares against the reference, for triage ordering;
    /// `None` in blind mode, where it would reveal which side is which.
    pub status: Option<SetStatus>,
    /// Worst (largest) mean FLIP of any pane against the reference; `None`
    /// when there is no FLIP data or in blind mode.
    pub worst_flip: Option<f64>,
    /// Config regions that apply to this set, resolved to pixels of the
    /// reference image.
    pub presets: Vec<ViewPreset>,
}

/// Triage status of an image set (non-blind views only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SetStatus {
    /// A pane failed to decode or compare.
    Error,
    /// Some directory has no image of this name.
    Missing,
    /// At least one pane differs from the reference.
    Changed,
    /// Every pane matches the reference exactly (FLIP max 0).
    Identical,
}

fn image_warnings(p: &crate::report::Properties) -> Vec<String> {
    let mut w = Vec::new();
    if p.is_all_black {
        w.push("image is all black".to_owned());
    }
    if p.is_all_white {
        w.push("image is all white".to_owned());
    }
    if p.nan_count > 0 || p.inf_count > 0 {
        w.push(format!(
            "image has non-finite samples ({} NaN, {} infinite)",
            p.nan_count, p.inf_count
        ));
    }
    w
}

/// Triage status and worst mean FLIP of a set's panes.
fn triage(panes: &[ViewPane], reference: usize) -> (SetStatus, Option<f64>) {
    let worst = panes
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != reference)
        .filter_map(|(_, p)| p.metrics.as_ref().map(|m| m.mean))
        .fold(None, |a: Option<f64>, v| Some(a.map_or(v, |a| a.max(v))));
    let status = if panes.iter().any(|p| p.path.is_none()) {
        SetStatus::Missing
    } else if panes.iter().any(|p| p.error.is_some()) {
        SetStatus::Error
    } else if panes
        .iter()
        .filter_map(|p| p.metrics.as_ref())
        .all(|m| m.max <= 0.0)
    {
        SetStatus::Identical
    } else {
        SetStatus::Changed
    };
    (status, worst)
}

/// A config region offered as a preset ROI.
#[derive(Debug, Clone, Serialize)]
pub struct ViewPreset {
    /// Region name.
    pub name: String,
    /// Left edge in pixels.
    pub x: u32,
    /// Top edge in pixels.
    pub y: u32,
    /// Width in pixels.
    pub w: u32,
    /// Height in pixels.
    pub h: u32,
}

/// The data embedded in the viewer's `index.html`.
#[derive(Debug, Clone, Serialize)]
pub struct ViewModel {
    /// Always [`VIEW_SCHEMA`].
    pub schema: String,
    /// `saccade` version.
    pub tool_version: String,
    /// Generation time, Unix seconds.
    pub generated_at_unix: u64,
    /// Keys the viewer's `localStorage` entries.
    pub id: String,
    /// One label per directory.
    pub labels: Vec<String>,
    /// Index of the FLIP reference directory; the number of directories (no
    /// such pane) in blind mode.
    pub reference: usize,
    /// Blind mode: labels are hidden until the judge reveals them.
    pub blind: bool,
    /// The shuffle seed (the per-set order derives from it and the set name);
    /// in blind mode a random token unrelated to the shuffle, which only pairs
    /// the page with its key and decisions.
    pub seed: u64,
    /// FLIP pixels per degree.
    pub pixels_per_degree: f32,
    /// Path of each directory relative to the viewer, absolute only by opt-in. Empty in blind
    /// mode, where a path could reveal which side is which (the paths live in
    /// the blind key and reach the decisions through `unblind`).
    pub dirs: Vec<String>,
    /// The image sets, sorted by name.
    pub sets: Vec<ViewSet>,
    /// Run-level attribution against the reference, absent from blind views.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub perf_diff: Vec<ViewPerf>,
}

/// Run-level evidence for one viewer directory against the reference.
#[derive(Debug, Clone, Serialize)]
pub struct ViewPerf {
    /// Candidate directory index.
    pub run: usize,
    /// Attribution evidence.
    pub diff: Option<crate::perf::PerfDiff>,
    /// Malformed performance error entries.
    pub errors: Vec<crate::perf::PerfError>,
    /// Image and performance evidence.
    pub combined_verdict: String,
}

/// A human verdict on one image set.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// Promote the capture to baseline.
    Accept,
    /// Do not promote.
    Reject,
    /// Not decided; the renderer needs more work.
    NeedsWork,
}

/// A region of interest in image pixels.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Roi {
    /// Left edge.
    pub x: u32,
    /// Top edge.
    pub y: u32,
    /// Width.
    pub w: u32,
    /// Height.
    pub h: u32,
}

/// The judge's entry for one image set.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetDecision {
    /// Relative image path of the set.
    pub name: String,
    /// accept / reject / needs-work, if decided.
    #[serde(default)]
    pub decision: Option<Verdict>,
    /// Pairwise judging: the true label of the preferred pane.
    #[serde(default)]
    pub chosen_label: Option<String>,
    /// Pairwise judging: "no visible difference".
    #[serde(default)]
    pub no_difference: bool,
    /// Free-text note.
    #[serde(default)]
    pub note: String,
    /// The ROI that was drawn, if any.
    #[serde(default)]
    pub roi: Option<Roi>,
    /// When the entry was last edited, Unix milliseconds.
    #[serde(default)]
    pub timestamp_ms: u64,
    /// Pairwise judging: the recorded directory of the preferred pane (set by
    /// the viewer, or by `unblind` for a blind view). `approve` refuses a
    /// decision whose chosen directory is not the capture directory.
    #[serde(default)]
    pub chosen_dir: Option<String>,
    /// SHA-256 (hex) of the image of each directory when the set was judged,
    /// in directory order; `null` where the directory had no such image.
    #[serde(default)]
    pub sha256: Vec<Option<String>>,
    /// Answers recorded by `saccade decide` (an agent, a bounded-decision
    /// model or a person). New answers never change [`SetDecision::decision`].
    /// Historical finals require a new canonical review before approval.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proposals: Vec<Proposal>,
}

/// One answer to a decision-request question, as recorded by `saccade decide`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Proposal {
    /// The question type: `accept`, `triage`, `cause`, `ask_human` or `mask_suggest`.
    pub question: String,
    /// For `mask_suggest`: the 1-based hotspot the answer is about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hotspot: Option<u32>,
    /// The answer, one of the question's allowed answers.
    pub answer: String,
    /// The model's confidence in `[0, 1]`; `None` for a person.
    #[serde(default)]
    pub prob: Option<f64>,
    /// Who answered: `jev`, `openai-decisions`, a model name or `human`.
    pub source: String,
    /// A second confidence figure when the provider reports one besides the
    /// probability of the chosen answer (Jev's `confidence`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    /// When it was recorded, Unix milliseconds.
    #[serde(default)]
    pub timestamp_ms: u64,
    /// The `request_hash` of the decision-request item it answered.
    #[serde(default)]
    pub request_hash: String,
    /// Free-text note from the answerer.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// True for every new answer: source labels never grant authority.
    pub proposed: bool,
    /// Historical promotion flag, retained for readers; new answers set false.
    #[serde(default)]
    pub promoted: bool,
}

/// The `saccade-decisions.v1.json` file.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decisions {
    /// Always [`DECISIONS_SCHEMA`].
    pub schema: String,
    /// The shuffle seed of the view the decisions were made in.
    pub seed: u64,
    /// The view's directory labels.
    pub labels: Vec<String>,
    /// Whether the judging was blind.
    #[serde(default)]
    pub blind: bool,
    /// Input paths relative to the decisions document, absolute only by opt-in. Empty for a blind
    /// view until `unblind` fills it from the key.
    #[serde(default)]
    pub dirs: Vec<String>,
    /// One entry per image set.
    pub sets: Vec<SetDecision>,
}

/// The mapping from a blind view's neutral labels (`P1`, `P2`, ...) to the true
/// directory labels, per image set (every set shuffles its own panes). Never
/// embedded in the page.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlindKey {
    /// Always [`BLIND_KEY_SCHEMA`].
    pub schema: String,
    /// The page's random token (`ViewModel::seed`); must match the decisions file.
    pub seed: u64,
    /// The seed the panes were shuffled with (`--seed` reproduces the view).
    #[serde(default)]
    pub shuffle_seed: u64,
    /// True labels, in directory order.
    pub labels: Vec<String>,
    /// Input paths relative to the viewer directory, absolute only by opt-in.
    #[serde(default)]
    pub dirs: Vec<String>,
    /// Viewer directory relative to the key file, anchoring the recorded dirs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_dir: Option<String>,
    /// Per image set, the true label of each pane as displayed: entry `n - 1`
    /// is the label behind the set's neutral label `P<n>`.
    #[serde(default)]
    pub sets: BTreeMap<String, Vec<String>>,
}

/// Reads and validates a blind key file.
pub fn read_blind_key(path: &Path) -> Result<BlindKey> {
    let text = std::fs::read_to_string(path)
        .map_err(io_err(format!("reading blind key {}", path.display())))?;
    let key: BlindKey = serde_json::from_str(&text)?;
    if key.schema != BLIND_KEY_SCHEMA {
        return Err(Error::Config(format!(
            "{} has schema {:?}, expected {BLIND_KEY_SCHEMA:?}",
            path.display(),
            key.schema
        )));
    }
    Ok(key)
}

/// Replaces the neutral labels of a decisions file exported from a blind view
/// with the true labels from its key.
///
/// Fails when the seeds differ, the label counts differ, or a `chosen_label`
/// is not one of the decisions' own labels.
pub fn unblind(decisions: &Decisions, key: &BlindKey) -> Result<Decisions> {
    if decisions.seed != key.seed {
        return Err(Error::Config(format!(
            "seed mismatch: decisions {} vs key {}",
            decisions.seed, key.seed
        )));
    }
    if decisions.labels.len() != key.labels.len() {
        return Err(Error::Config(format!(
            "{} labels in the decisions, {} in the key",
            decisions.labels.len(),
            key.labels.len()
        )));
    }
    let mut out = decisions.clone();
    out.labels = key.labels.clone();
    out.dirs = key.dirs.clone();
    // The result carries the true labels: it is no longer a blind file.
    out.blind = false;
    for set in &mut out.sets {
        if let Some(neutral) = &set.chosen_label {
            let idx = decisions
                .labels
                .iter()
                .position(|l| l == neutral)
                .ok_or_else(|| {
                    Error::Config(format!("{:?}: unknown label {neutral:?}", set.name))
                })?;
            let per_set = key.sets.get(&set.name).ok_or_else(|| {
                Error::Config(format!("the blind key has no entry for {:?}", set.name))
            })?;
            set.chosen_label = per_set.get(idx).cloned();
            set.chosen_dir = set
                .chosen_label
                .as_ref()
                .and_then(|l| key.labels.iter().position(|k| k == l))
                .and_then(|i| key.dirs.get(i).cloned());
        }
        // A blind page lists a set's panes in its shuffled order; the hashes
        // go back to directory order, which is how `approve` reads them.
        if let Some(per_set) = key.sets.get(&set.name).filter(|_| !set.sha256.is_empty()) {
            let mut by_dir = vec![None; key.labels.len()];
            for (shown, label) in per_set.iter().enumerate() {
                if let Some(dir_idx) = key.labels.iter().position(|l| l == label) {
                    by_dir[dir_idx] = set.sha256.get(shown).cloned().flatten();
                }
            }
            set.sha256 = by_dir;
        }
    }
    Ok(out)
}

impl Decisions {
    /// Names of the sets whose verdict is [`Verdict::Accept`].
    pub fn accepted(&self) -> Vec<String> {
        self.sets
            .iter()
            .filter(|s| s.decision == Some(Verdict::Accept))
            .map(|s| s.name.clone())
            .collect()
    }
}

/// Reads and validates a decisions file.
pub fn read_decisions(path: &Path) -> Result<Decisions> {
    let text = std::fs::read_to_string(path)
        .map_err(io_err(format!("reading decisions file {}", path.display())))?;
    let decisions: Decisions = serde_json::from_str(&text)?;
    if decisions.schema != DECISIONS_SCHEMA {
        return Err(Error::Config(format!(
            "{} has schema {:?}, expected {DECISIONS_SCHEMA:?}",
            path.display(),
            decisions.schema
        )));
    }
    Ok(decisions)
}

/// True when `name` is a non-empty relative path made only of normal
/// components, so joining it onto a directory cannot escape that directory.
pub fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && Path::new(name)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}

/// A fresh 64-bit value from the operating system's randomness (the standard
/// library seeds every `RandomState` from it).
fn random_u64() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u8(0);
    h.finish()
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Deterministic pane order for one image set: a Fisher-Yates shuffle of
/// `0..n` seeded by `seed` and the set name.
pub fn shuffled_order(seed: u64, name: &str, n: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..n).collect();
    let mut state = seed ^ fnv1a(name.as_bytes());
    for i in (1..n).rev() {
        let j = (splitmix64(&mut state) % (i as u64 + 1)) as usize;
        order.swap(i, j);
    }
    order
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub(crate) fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for k in 0..4 {
            if k <= chunk.len() {
                out.push(char::from(B64[((n >> (18 - 6 * k)) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Copies a pane's file into the view and returns the path the page shows. An
/// HDR file is copied as `<stem>.orig.<ext>` and shown as a tone-mapped
/// `<stem>.png` (exposure 0).
fn pane_files(
    src: &Path,
    hdr: Option<&crate::hdr::HdrImage>,
    (out_dir, name, stem): (&Path, &str, &str),
    tm: crate::hdr::Tonemapper,
) -> Result<String> {
    let Some(hdr) = hdr else {
        return copy_into_report(src, out_dir, name, stem);
    };
    copy_into_report(src, out_dir, name, &format!("{stem}.orig"))?;
    let rel = format!("images/{name}.d/{stem}.png");
    let dest = out_dir.join(&rel);
    crate::hdr::display_image(hdr, tm)
        .save(&dest)
        .map_err(|source| Error::Encode {
            path: dest.clone(),
            source,
        })?;
    Ok(rel)
}

/// FLIP between a pane and the reference: HDR-FLIP when both are HDR, an
/// error when only one is, LDR-FLIP otherwise.
fn compare_pane(
    img: &image::RgbaImage,
    reference: &image::RgbaImage,
    hdr: Option<&crate::hdr::HdrImage>,
    ref_hdr: Option<&crate::hdr::HdrImage>,
    opts: &CompareOptions,
) -> Result<crate::compare::Comparison> {
    match (hdr, ref_hdr) {
        (Some(h), Some(r)) => crate::hdr::compare_hdr(h, r, opts).map(|(cmp, _)| cmp),
        (None, None) => compare_rgba(img, reference, opts),
        _ => Err(Error::HdrMismatch(
            "one of the panes is HDR and the other is LDR".into(),
        )),
    }
}

/// Longest side, in pixels, of the inspector pixel data of one pane.
const INSPECTOR_MAX_SIDE: u32 = 2048;

/// Integer downsampling factor that brings the longest side to
/// [`INSPECTOR_MAX_SIDE`] or less.
fn inspector_scale(w: u32, h: u32) -> u32 {
    w.max(h).div_ceil(INSPECTOR_MAX_SIDE).max(1)
}

/// Box-averages `channels` interleaved planes of `w`x`h` samples by `f`.
fn box_down(src: &[f32], channels: usize, (w, h): (u32, u32), f: u32) -> (Vec<f32>, u32, u32) {
    let (ow, oh) = (w.div_ceil(f), h.div_ceil(f));
    let mut out = Vec::with_capacity(ow as usize * oh as usize * channels);
    for oy in 0..oh {
        for ox in 0..ow {
            let (x1, y1) = (((ox + 1) * f).min(w), ((oy + 1) * f).min(h));
            let n = ((x1 - ox * f) * (y1 - oy * f)) as f32;
            for c in 0..channels {
                let mut sum = 0.0f32;
                for y in oy * f..y1 {
                    for x in ox * f..x1 {
                        sum += src[(y as usize * w as usize + x as usize) * channels + c];
                    }
                }
                out.push(sum / n);
            }
        }
    }
    (out, ow, oh)
}

fn png_data_uri(
    bytes: &[u8],
    (w, h): (u32, u32),
    color: image::ExtendedColorType,
) -> Result<String> {
    use image::ImageEncoder;
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new_with_quality(
        &mut png,
        image::codecs::png::CompressionType::Best,
        image::codecs::png::FilterType::Adaptive,
    )
    .write_image(bytes, w, h, color)
    .map_err(|source| Error::Encode {
        path: PathBuf::from("<inspector data>"),
        source,
    })?;
    Ok(format!("data:image/png;base64,{}", base64(&png)))
}

/// Lossless PNG data URI of a pane's RGB, box-downsampled by `f`.
fn rgb_data_uri(img: &image::RgbaImage, f: u32) -> Result<String> {
    let (w, h) = img.dimensions();
    if f == 1 {
        let rgb: Vec<u8> = img
            .as_raw()
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect();
        return png_data_uri(&rgb, (w, h), image::ExtendedColorType::Rgb8);
    }
    let rgb: Vec<f32> = img
        .as_raw()
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| [f32::from(p[0]), f32::from(p[1]), f32::from(p[2])])
        .collect();
    let (down, ow, oh) = box_down(&rgb, 3, (w, h), f);
    let bytes: Vec<u8> = down.iter().map(|v| v.round() as u8).collect();
    png_data_uri(&bytes, (ow, oh), image::ExtendedColorType::Rgb8)
}

/// Quantised (8-bit grayscale) FLIP error map as a lossless PNG data URI,
/// box-downsampled by `f`.
fn flip_data_uri(error_map: &[f32], (w, h): (u32, u32), f: u32) -> Result<String> {
    let (down, ow, oh) = if f == 1 {
        (error_map.to_vec(), w, h)
    } else {
        box_down(error_map, 1, (w, h), f)
    };
    let gray: Vec<u8> = down
        .iter()
        .map(|&v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
        .collect();
    png_data_uri(&gray, (ow, oh), image::ExtendedColorType::L8)
}

/// A pane error as shown on the page; blind pages must not leak directory paths.
fn pane_error(e: &Error, blind: bool) -> String {
    if blind && matches!(e, Error::Decode { .. }) {
        "image could not be decoded".to_string()
    } else {
        e.to_string()
    }
}

fn default_labels(dirs: &[PathBuf]) -> Vec<String> {
    let mut labels: Vec<String> = Vec::new();
    for (i, d) in dirs.iter().enumerate() {
        let base = d.file_name().map_or_else(
            || format!("dir{}", i + 1),
            |n| n.to_string_lossy().into_owned(),
        );
        let mut label = base.clone();
        let mut n = 2;
        while labels.contains(&label) {
            label = format!("{base}-{n}");
            n += 1;
        }
        labels.push(label);
    }
    labels
}

fn resolve_reference(
    reference: Option<&str>,
    dirs: &[PathBuf],
    labels: &[String],
) -> Result<usize> {
    let Some(r) = reference else { return Ok(0) };
    if let Some(i) = labels.iter().position(|l| l == r) {
        return Ok(i);
    }
    let want = crate::paths::canonicalize(r).ok();
    dirs.iter()
        .position(|d| d == Path::new(r) || want.as_ref() == Some(d))
        .ok_or_else(|| Error::Config(format!("--reference {r:?} is not one of the directories")))
}

/// Resolves the config regions that match `name` against a `width` x `height`
/// reference image.
fn preset_rois(
    regions: &[crate::regions::RegionSpec],
    name: &str,
    width: u32,
    height: u32,
) -> Vec<ViewPreset> {
    regions
        .iter()
        .filter(|r| {
            r.glob
                .as_deref()
                .is_none_or(|g| crate::config::compile_glob(g).is_ok_and(|m| m.is_match(name)))
        })
        .filter_map(|r| {
            let [x, y, w, h] = crate::regions::resolve_rect(r.rect, width, height)?;
            Some(ViewPreset {
                name: r.name.clone(),
                x,
                y,
                w,
                h,
            })
        })
        .collect()
}

/// One side of a pair as `diagnose_pane` reads it.
struct Side<'a> {
    /// Decoded 8-bit image (the display image for an HDR file).
    img: &'a image::RgbaImage,
    /// Float decode of an HDR file.
    hdr: Option<&'a crate::hdr::HdrImage>,
    /// The source file.
    file: &'a Path,
    /// The directory the file came from (its sidecar lives there).
    #[cfg(feature = "graphics")]
    dir: &'a Path,
}

/// Where a pane's diagnostics images go.
struct DiagTarget<'a> {
    /// The view directory.
    out_dir: &'a Path,
    /// The set name.
    name: &'a str,
    /// The pane's directory index.
    index: usize,
}

/// Runs the diagnostics engine for one pane against the reference and records
/// its findings, the signed-difference image and the non-finite mask. A failure
/// only adds a warning: the pane stays usable.
fn diagnose_pane(
    pane: &mut ViewPane,
    cmp: &crate::compare::Comparison,
    (reference, capture): (&Side<'_>, &Side<'_>),
    ctx: (&CompareOptions, &ViewOptions, &crate::meta::MetaChecker),
    target: &DiagTarget<'_>,
) {
    use crate::diagnostics::{DiagOut, DiagnoseRequest, Pixels, diagnose};
    let (flip, opts, _meta) = ctx;
    fn pixels<'a>(s: &Side<'a>) -> Pixels<'a> {
        match s.hdr {
            Some(h) => Pixels::Hdr(h),
            None => Pixels::Ldr(s.img),
        }
    }
    let diag_name = format!("{}.d/pane{}", target.name, target.index);
    let req = DiagnoseRequest {
        baseline: pixels(reference),
        capture: pixels(capture),
        comparison: cmp,
        flip,
        bit_identical: Some(crate::compare::native_samples_identical(
            reference.file,
            capture.file,
        )),
        baseline_properties: None,
        capture_properties: pane.properties,
        hotspots: &pane.hotspots,
        hotspot_options: crate::hotspots::HotspotOptions::default(),
        config: &opts.diagnostics,
        capture_path: Some(capture.file),
        out: Some(DiagOut {
            report_dir: target.out_dir,
            name: &diag_name,
        }),
    };
    match diagnose(&req) {
        Ok(out) => {
            #[cfg(feature = "graphics")]
            let mut out = out;
            #[cfg(feature = "graphics")]
            {
                (out.diagnostics.perf, out.diagnostics.perf_not_comparable) =
                    crate::diagnostics::perf_pairs(
                        _meta,
                        &opts.diagnostics,
                        reference.dir,
                        capture.dir,
                        target.name,
                    );
            }
            pane.signed_diff = out.signed_diff;
            pane.nonfinite_mask = out.nonfinite_mask;
            pane.diagnostics = Some(out.diagnostics);
        }
        Err(e) => pane.warnings.push(format!("diagnostics failed: {e}")),
    }
}

/// Builds the viewer in `out_dir` (`index.html` plus `images/`) and returns
/// the embedded model.
///
/// Only directory and write failures return `Err`; per-image problems become
/// a pane `error`.
pub fn build_view(dirs: &[PathBuf], out_dir: &Path, opts: &ViewOptions) -> Result<ViewModel> {
    if !(MIN_DIRS..=MAX_DIRS).contains(&dirs.len()) {
        return Err(Error::Config(format!(
            "view takes {MIN_DIRS} to {MAX_DIRS} directories, got {}",
            dirs.len()
        )));
    }
    let dirs: Vec<PathBuf> = dirs.iter().map(|d| crate::run::normalise_path(d)).collect();
    let dirs = dirs.as_slice();
    let labels = match &opts.labels {
        Some(l) if l.len() != dirs.len() => {
            return Err(Error::Config(format!(
                "{} labels for {} directories",
                l.len(),
                dirs.len()
            )));
        }
        Some(l) => l.clone(),
        None => default_labels(dirs),
    };
    for (i, l) in labels.iter().enumerate() {
        if l.is_empty() || labels[..i].contains(l) {
            return Err(Error::Config(format!(
                "labels must be non-empty and unique (got {l:?})"
            )));
        }
    }
    crate::compare::check_ppd(opts.pixels_per_degree)?;
    opts.diagnostics.validate()?;
    opts.perf.validate()?;
    #[cfg(not(feature = "graphics"))]
    if opts.perf != crate::perf::PerfOptions::default() {
        return Err(Error::FeatureUnavailable {
            feature: "graphics",
        });
    }
    let meta = opts.meta.checker()?;
    let reference = resolve_reference(opts.reference.as_deref(), dirs, &labels)?;
    // Blind pages carry neutral labels only; the true ones go to the key file.
    let shown_labels: Vec<String> = if opts.blind {
        (1..=dirs.len()).map(|i| format!("P{i}")).collect()
    } else {
        labels.clone()
    };

    for g in &opts.entries {
        crate::config::compile_glob(g)?;
    }
    let mut maps: Vec<BTreeMap<String, PathBuf>> = Vec::with_capacity(dirs.len());
    for d in dirs {
        let found = collect_images(d)?;
        if let Some((name, why)) = found.problems.iter().next() {
            return Err(Error::Config(format!("{}: {name}: {why}", d.display())));
        }
        maps.push(
            found
                .files
                .into_iter()
                .filter(|(name, _)| crate::paths::matches_entries(&opts.entries, name))
                .collect(),
        );
    }
    let mut names: Vec<&String> = maps.iter().flat_map(BTreeMap::keys).collect();
    names.sort();
    names.dedup();

    let input_dirs: Vec<&Path> = dirs.iter().map(PathBuf::as_path).collect();
    crate::run::guard_output_dir(out_dir, &input_dirs, &[VIEW_MARKER_FILE])?;
    std::fs::create_dir_all(out_dir).map_err(io_err(format!("creating {}", out_dir.display())))?;
    let marker_path = out_dir.join(VIEW_MARKER_FILE);
    std::fs::write(
        &marker_path,
        format!(
            "{{\"schema\":\"{VIEW_SCHEMA}\",\"tool_version\":\"{}\"}}\n",
            env!("CARGO_PKG_VERSION")
        ),
    )
    .map_err(io_err(format!("writing {}", marker_path.display())))?;
    let abs_dirs: Vec<String> = dirs
        .iter()
        .map(|d| crate::paths::record(d, out_dir, opts.record_absolute_paths))
        .collect();
    let generated_at_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    // JSON numbers are doubles in the browser: keep the seed exactly representable.
    let shuffle_seed = opts.seed.unwrap_or_else(|| {
        if opts.blind {
            random_u64() & MAX_SEED
        } else {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos() as u64 & MAX_SEED)
        }
    });
    if shuffle_seed > MAX_SEED {
        return Err(Error::Config(format!("--seed must be at most {MAX_SEED}")));
    }
    // A blind page must not carry what lets its shuffle be undone: it embeds
    // an unrelated random token instead of the seed.
    let seed = if opts.blind {
        random_u64() & MAX_SEED
    } else {
        shuffle_seed
    };
    let mut key_sets: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let compare_opts = CompareOptions {
        pixels_per_degree: opts.pixels_per_degree,
        hdr: opts.hdr,
    };

    // Image sets are independent: build them in parallel, kept in name order.
    let indexed: Vec<(usize, &String)> = names.into_iter().enumerate().collect();
    let built: Vec<(ViewSet, Option<Vec<String>>)> = indexed
        .into_par_iter()
        .map(
            |(set_idx, name)| -> Result<(ViewSet, Option<Vec<String>>)> {
                let sources: Vec<Option<&PathBuf>> = maps.iter().map(|m| m.get(name)).collect();
                let decoded: Vec<Option<std::result::Result<image::RgbaImage, Error>>> =
                    sources.iter().map(|s| s.map(|p| decode(p))).collect();
                let ref_img = match decoded.get(reference) {
                    Some(Some(Ok(img))) => Some(img),
                    _ => None,
                };
                // HDR sources are compared with HDR-FLIP and shown as tone-mapped PNGs.
                let hdr_imgs: Vec<Option<crate::hdr::HdrImage>> = sources
                    .iter()
                    .map(|s| {
                        s.filter(|p| crate::hdr::is_hdr_path(p))
                            .and_then(|p| crate::hdr::decode_hdr(p).ok())
                    })
                    .collect();
                let ref_hdr = hdr_imgs.get(reference).and_then(Option::as_ref);

                let mut panes = Vec::with_capacity(dirs.len());
                let mut pixel_uris: Vec<Option<String>> = Vec::with_capacity(dirs.len());
                let mut flip_uris: Vec<Option<String>> = Vec::with_capacity(dirs.len());
                // Blind: this set's panes are laid out in a shuffled directory order, so
                // the position of a pane says nothing about its directory.
                let dir_order: Vec<usize> = if opts.blind {
                    shuffled_order(shuffle_seed, name, dirs.len())
                } else {
                    (0..dirs.len()).collect()
                };
                let key_entry: Option<Vec<String>> = opts
                    .blind
                    .then(|| dir_order.iter().map(|&i| labels[i].clone()).collect());
                for &i in &dir_order {
                    let src = &sources[i];
                    let mut pane = ViewPane {
                        path: None,
                        width: 0,
                        height: 0,
                        heatmap: None,
                        inspector_scale: 1,
                        properties: None,
                        warnings: Vec::new(),
                        metrics: None,
                        hotspots: Vec::new(),
                        error: None,
                        meta_diff: Vec::new(),
                        meta_error: None,
                        sha256: None,
                        diagnostics: None,
                        signed_diff: None,
                        nonfinite_mask: None,
                    };
                    let Some(src) = src else {
                        pixel_uris.push(None);
                        flip_uris.push(None);
                        panes.push(pane);
                        continue;
                    };
                    let stem = if opts.blind {
                        format!("p_{:016x}", random_u64())
                    } else {
                        format!("pane{i}")
                    };
                    let shown = pane_files(
                        src,
                        hdr_imgs.get(i).and_then(Option::as_ref),
                        (out_dir, name, &stem),
                        opts.hdr.tonemapper,
                    )?;
                    // The inspector reads what the page shows: the decoded file, or the
                    // tone-mapped PNG of an HDR file.
                    let shown_img = if hdr_imgs.get(i).is_some_and(Option::is_some) {
                        image::open(out_dir.join(&shown)).ok().map(|d| d.to_rgba8())
                    } else {
                        match &decoded[i] {
                            Some(Ok(img)) => Some(img.clone()),
                            _ => None,
                        }
                    };
                    pane.path = Some(shown);
                    pane.sha256 = crate::run::sha256_file(src).ok();
                    let mut flip_uri = None;
                    if i != reference
                        && !opts.blind
                        && sources.get(reference).is_some_and(Option::is_some)
                    {
                        match meta.compare(&dirs[reference], &dirs[i], name) {
                            Ok((diff, _)) => pane.meta_diff = diff,
                            Err(e) => pane.meta_error = Some(e),
                        }
                    }
                    match &decoded[i] {
                        Some(Ok(img)) => {
                            (pane.width, pane.height) = img.dimensions();
                            let props = match hdr_imgs.get(i).and_then(Option::as_ref) {
                                Some(h) => crate::hdr::validate_hdr(h),
                                None => crate::properties::validate(&crate::compare::flatten_over(
                                    img, 0,
                                )),
                            };
                            pane.warnings = image_warnings(&props);
                            pane.properties = Some(props);
                            // Blind views embed no FLIP data: it would single out the
                            // reference directory.
                            if i != reference && !opts.blind {
                                match ref_img {
                                    Some(r) => match compare_pane(
                                        img,
                                        r,
                                        hdr_imgs.get(i).and_then(Option::as_ref),
                                        ref_hdr,
                                        &compare_opts,
                                    ) {
                                        Ok(cmp) => {
                                            let rel = format!("images/{name}.d/heatmap{i}.png");
                                            let dest = out_dir.join(&rel);
                                            cmp.heatmap_rgb().save(&dest).map_err(|source| {
                                                Error::Encode { path: dest, source }
                                            })?;
                                            pane.heatmap = Some(rel);
                                            flip_uri = Some(flip_data_uri(
                                                &cmp.error_map,
                                                (cmp.metrics.width, cmp.metrics.height),
                                                inspector_scale(
                                                    cmp.metrics.width,
                                                    cmp.metrics.height,
                                                ),
                                            )?);
                                            pane.hotspots = crate::hotspots::find_hotspots(
                                                &cmp.error_map,
                                                None,
                                                cmp.metrics.width,
                                                cmp.metrics.height,
                                                &crate::hotspots::HotspotOptions::default(),
                                            );
                                            if let Some(ref_src) = sources[reference]
                                                .filter(|_| opts.diagnostics.enabled)
                                            {
                                                diagnose_pane(
                                                    &mut pane,
                                                    &cmp,
                                                    (
                                                        &Side {
                                                            img: r,
                                                            hdr: ref_hdr,
                                                            file: ref_src,
                                                            #[cfg(feature = "graphics")]
                                                            dir: &dirs[reference],
                                                        },
                                                        &Side {
                                                            img,
                                                            hdr: hdr_imgs
                                                                .get(i)
                                                                .and_then(Option::as_ref),
                                                            file: src,
                                                            #[cfg(feature = "graphics")]
                                                            dir: &dirs[i],
                                                        },
                                                    ),
                                                    (&compare_opts, opts, &meta),
                                                    &DiagTarget {
                                                        out_dir,
                                                        name,
                                                        index: i,
                                                    },
                                                );
                                            }
                                            pane.metrics = Some(cmp.metrics);
                                        }
                                        Err(e) => pane.error = Some(pane_error(&e, opts.blind)),
                                    },
                                    None => {
                                        pane.error =
                                            Some("reference image is missing or unreadable".into());
                                    }
                                }
                            }
                        }
                        Some(Err(e)) => pane.error = Some(pane_error(e, opts.blind)),
                        None => {}
                    }
                    pane.inspector_scale = inspector_scale(pane.width, pane.height);
                    pixel_uris.push(match &shown_img {
                        Some(img) => Some(rgb_data_uri(img, pane.inspector_scale)?),
                        None => None,
                    });
                    flip_uris.push(flip_uri);
                    panes.push(pane);
                }

                let px_rel = format!("images/_px/{set_idx}.js");
                let px_path = out_dir.join(&px_rel);
                if let Some(parent) = px_path.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(io_err(format!("creating {}", parent.display())))?;
                }
                let px_json = serde_json::to_string(&serde_json::json!({
                    "scale": panes.iter().map(|p| p.inspector_scale).collect::<Vec<_>>(),
                    "rgb": pixel_uris,
                    "flip": flip_uris,
                }))?;
                let px_js = format!(
                    "(window.__saccadePx=window.__saccadePx||{{}})[{set_idx}]={px_json};\n"
                );
                std::fs::write(&px_path, px_js)
                    .map_err(io_err(format!("writing {}", px_path.display())))?;

                let order: Vec<usize> = (0..dirs.len()).collect();
                let ref_pane = if opts.blind {
                    panes.iter().find(|p| p.width > 0)
                } else {
                    panes.get(reference)
                };
                let presets = ref_pane
                    .map(|p| preset_rois(&opts.regions, name, p.width, p.height))
                    .unwrap_or_default();
                let (status, worst_flip) = if opts.blind {
                    (None, None)
                } else {
                    let (st, w) = triage(&panes, reference);
                    (Some(st), w)
                };
                Ok((
                    ViewSet {
                        name: name.clone(),
                        order,
                        panes,
                        pixels_script: px_rel,
                        status,
                        worst_flip,
                        presets,
                    },
                    key_entry,
                ))
            },
        )
        .collect::<Result<Vec<_>>>()?;
    let mut sets = Vec::with_capacity(built.len());
    for (set, key_entry) in built {
        if let Some(labels) = key_entry {
            key_sets.insert(set.name.clone(), labels);
        }
        sets.push(set);
    }

    let id = {
        let mut key = format!("{generated_at_unix}|{seed}|{}", shown_labels.join(","));
        for s in &sets {
            key.push('|');
            key.push_str(&s.name);
        }
        format!("{:016x}", fnv1a(key.as_bytes()))
    };
    let mut perf_diff = Vec::new();
    if !opts.blind {
        for (i, _dir) in dirs.iter().enumerate().filter(|(i, _)| *i != reference) {
            #[cfg(feature = "graphics")]
            let (diff, mut errors) = crate::perf::pair(&dirs[reference], _dir, &opts.perf)?;
            #[cfg(not(feature = "graphics"))]
            let (diff, mut errors): (
                Option<crate::perf::PerfDiff>,
                Vec<crate::perf::PerfError>,
            ) = (None, Vec::new());
            for error in &mut errors {
                if !opts.record_absolute_paths {
                    error.path = crate::paths::record(Path::new(&error.path), out_dir, false);
                }
            }
            let identical = !sets.is_empty()
                && sets.iter().all(|s| {
                    s.panes
                        .get(i)
                        .is_some_and(|p| p.error.is_none() && p.path.is_some())
                        && s.panes
                            .get(reference)
                            .is_some_and(|p| p.error.is_none() && p.path.is_some())
                        && crate::compare::native_samples_identical(
                            &dirs[reference].join(&s.name),
                            &dirs[i].join(&s.name),
                        )
                });
            let image = if identical {
                "image bit-identical"
            } else {
                "image changed or not comparable"
            };
            if diff.is_some() || !errors.is_empty() {
                perf_diff.push(ViewPerf {
                    run: i,
                    combined_verdict: diff.as_ref().map_or_else(
                        || format!("{image} · performance unavailable"),
                        |d| format!("{image} · {}", d.verdict()),
                    ),
                    diff,
                    errors,
                });
            }
        }
    }
    let mut model = ViewModel {
        perf_diff,
        schema: VIEW_SCHEMA.to_string(),
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        generated_at_unix,
        id,
        labels: shown_labels,
        reference: if opts.blind { dirs.len() } else { reference },
        blind: opts.blind,
        seed,
        pixels_per_degree: opts.pixels_per_degree,
        dirs: if opts.blind {
            Vec::new()
        } else {
            abs_dirs.clone()
        },
        sets,
    };
    if !opts.record_absolute_paths {
        for set in &mut model.sets {
            for pane in &mut set.panes {
                for error in [&mut pane.error, &mut pane.meta_error]
                    .into_iter()
                    .flatten()
                {
                    for dir in dirs {
                        *error = error.replace(
                            &crate::run::normalise_path(dir).display().to_string(),
                            &crate::paths::record(dir, out_dir, false),
                        );
                    }
                }
            }
        }
    }
    write_view_html(&model, out_dir)?;
    if opts.blind {
        let key = BlindKey {
            schema: BLIND_KEY_SCHEMA.to_string(),
            seed,
            shuffle_seed,
            labels,
            dirs: abs_dirs,
            view_dir: Some(crate::paths::record(
                out_dir,
                opts.key_out
                    .as_deref()
                    .and_then(Path::parent)
                    .unwrap_or(out_dir),
                opts.record_absolute_paths,
            )),
            sets: key_sets,
        };
        let key_path = opts
            .key_out
            .clone()
            .unwrap_or_else(|| out_dir.join(BLIND_KEY_FILE));
        if let Some(parent) = key_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)
                .map_err(io_err(format!("creating {}", parent.display())))?;
        }
        std::fs::write(&key_path, serde_json::to_string_pretty(&key)?)
            .map_err(io_err(format!("writing {}", key_path.display())))?;
    }
    Ok(model)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn write_png(dir: &Path, name: &str, rgb: [u8; 3]) {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        image::RgbImage::from_pixel(8, 8, image::Rgb(rgb))
            .save(path)
            .unwrap();
    }

    fn three_dirs(root: &Path) -> Vec<PathBuf> {
        let dirs: Vec<PathBuf> = ["a", "b", "c"].iter().map(|d| root.join(d)).collect();
        for d in &dirs {
            write_png(d, "x.png", [10, 10, 10]);
        }
        write_png(&dirs[1], "sub/y.png", [200, 20, 20]);
        write_png(&dirs[2], "x.png", [60, 10, 10]);
        dirs
    }

    #[test]
    fn pairs_n_ways_with_labels_and_reference() {
        let tmp = tempfile::tempdir().unwrap();
        let dirs = three_dirs(tmp.path());
        let opts = ViewOptions {
            labels: Some(vec!["old".into(), "mid".into(), "new".into()]),
            reference: Some("mid".into()),
            ..ViewOptions::default()
        };
        let out = tmp.path().join("view");
        let m = build_view(&dirs, &out, &opts).unwrap();
        assert_eq!(m.labels, ["old", "mid", "new"]);
        assert_eq!(m.reference, 1);
        let names: Vec<_> = m.sets.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["sub/y.png", "x.png"]);
        let x = &m.sets[1];
        assert!(x.panes.iter().all(|p| p.path.is_some()));
        // Reference pane has no comparison; the others do.
        assert!(x.panes[1].metrics.is_none());
        assert!(x.panes[0].metrics.is_some() && x.panes[2].metrics.is_some());
        // Hotspots come only with a comparison; the uniform shift covers the frame.
        assert!(x.panes[1].hotspots.is_empty());
        assert_eq!(x.panes[2].hotspots[0].rect_px, [0, 0, 8, 8]);
        assert!(out.join("images/x.png.d/heatmap2.png").is_file());
        // y.png exists only in the reference directory.
        let y = &m.sets[0];
        assert!(y.panes[0].path.is_none() && y.panes[1].path.is_some());
        assert!(y.panes.iter().all(|p| p.metrics.is_none()));
        assert!(build_view(&dirs[..1], &out, &ViewOptions::default()).is_err());
    }

    #[test]
    fn sets_carry_triage_data_and_blind_hides_it() {
        let tmp = tempfile::tempdir().unwrap();
        let dirs = three_dirs(tmp.path());
        let m = build_view(&dirs, &tmp.path().join("v"), &ViewOptions::default()).unwrap();
        let x = m.sets.iter().find(|s| s.name == "x.png").unwrap();
        let y = m.sets.iter().find(|s| s.name == "sub/y.png").unwrap();
        assert_eq!(x.status, Some(SetStatus::Changed));
        assert!(x.worst_flip.is_some_and(|w| w > 0.0));
        assert_eq!(y.status, Some(SetStatus::Missing));
        let blind = ViewOptions {
            blind: true,
            ..ViewOptions::default()
        };
        let mb = build_view(&dirs, &tmp.path().join("vb"), &blind).unwrap();
        assert!(
            mb.sets
                .iter()
                .all(|s| s.status.is_none() && s.worst_flip.is_none())
        );
    }

    #[test]
    fn inspector_data_is_capped_lossless_and_kept_out_of_the_page() {
        let tmp = tempfile::tempdir().unwrap();
        let (a, b) = (tmp.path().join("a"), tmp.path().join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let wide = |shift: u32| {
            image::RgbImage::from_fn(4200, 40, |x, y| {
                image::Rgb([(x / 17 + shift) as u8, y as u8, 90])
            })
        };
        wide(0).save(a.join("w.png")).unwrap();
        wide(9).save(b.join("w.png")).unwrap();
        let out = tmp.path().join("v");
        let m = build_view(&[a, b], &out, &ViewOptions::default()).unwrap();
        assert_eq!(m.sets[0].panes[1].inspector_scale, 3);
        let html = std::fs::read_to_string(out.join("index.html")).unwrap();
        assert!(
            !html.contains("data:image"),
            "pixel data must load lazily, not ride in the page"
        );
        let px = std::fs::read_to_string(out.join(&m.sets[0].pixels_script)).unwrap();
        assert!(px.contains("\"scale\":[3,3]") && px.contains("data:image/png;base64,"));
        // Lossless: the stored RGB is exactly the box average of 3x3 source blocks.
        let json = px
            .split_once("]=")
            .unwrap()
            .1
            .trim_end()
            .trim_end_matches(';');
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        let uri = v["rgb"][0]
            .as_str()
            .unwrap()
            .strip_prefix("data:image/png;base64,")
            .unwrap();
        let bytes = decode_b64(uri);
        let img = image::load_from_memory(&bytes).unwrap().to_rgb8();
        assert_eq!(img.dimensions(), (1400, 14));
        let src = wide(0);
        let want: u32 = (0..3).map(|d| u32::from(src.get_pixel(3 + d, 0)[0])).sum();
        assert_eq!(
            u32::from(img.get_pixel(1, 0)[0]),
            (want as f32 / 3.0).round() as u32
        );
    }

    fn decode_b64(s: &str) -> Vec<u8> {
        let val = |c: u8| B64.iter().position(|&b| b == c).map(|p| p as u32);
        let mut out = Vec::new();
        for chunk in s.as_bytes().chunks(4) {
            let n = chunk
                .iter()
                .filter(|&&c| c != b'=')
                .fold(0u32, |a, &c| (a << 6) | val(c).unwrap());
            let pad = chunk.iter().filter(|&&c| c == b'=').count();
            let n = n << (6 * pad);
            for k in 0..(3 - pad) {
                out.push((n >> (16 - 8 * k)) as u8);
            }
        }
        out
    }

    #[test]
    fn blind_order_is_a_deterministic_seeded_permutation() {
        let a = shuffled_order(7, "x.png", 6);
        assert_eq!(a, shuffled_order(7, "x.png", 6));
        let mut sorted = a.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, [0, 1, 2, 3, 4, 5]);
        let differs = (0..32).any(|s| shuffled_order(s, "x.png", 6) != a)
            && (0..32).any(|s| shuffled_order(s, "x.png", 6) != (0..6).collect::<Vec<_>>());
        assert!(differs, "seed must change the order");
    }

    #[test]
    fn decisions_round_trip_and_accepted_names() {
        let d = Decisions {
            schema: DECISIONS_SCHEMA.into(),
            seed: 42,
            labels: vec!["a".into(), "b".into()],
            blind: true,
            dirs: Vec::new(),
            sets: vec![
                SetDecision {
                    name: "x.png".into(),
                    decision: Some(Verdict::Accept),
                    chosen_label: Some("b".into()),
                    no_difference: false,
                    note: "fine".into(),
                    roi: Some(Roi {
                        x: 1,
                        y: 2,
                        w: 3,
                        h: 4,
                    }),
                    timestamp_ms: 9,
                    chosen_dir: None,
                    sha256: Vec::new(),
                    proposals: Vec::new(),
                },
                SetDecision {
                    name: "y.png".into(),
                    decision: Some(Verdict::NeedsWork),
                    chosen_label: None,
                    no_difference: true,
                    note: String::new(),
                    roi: None,
                    timestamp_ms: 10,
                    chosen_dir: None,
                    sha256: Vec::new(),
                    proposals: Vec::new(),
                },
            ],
        };
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("d.json");
        std::fs::write(&path, serde_json::to_string(&d).unwrap()).unwrap();
        let back = read_decisions(&path).unwrap();
        assert_eq!(back, d);
        assert_eq!(back.accepted(), ["x.png"]);
        std::fs::write(&path, r#"{"schema":"nope","seed":1,"labels":[],"sets":[]}"#).unwrap();
        assert!(read_decisions(&path).is_err());
    }

    #[test]
    fn html_is_self_contained() {
        let tmp = tempfile::tempdir().unwrap();
        let dirs = three_dirs(tmp.path());
        let out = tmp.path().join("view");
        build_view(&dirs, &out, &ViewOptions::default()).unwrap();
        let html = std::fs::read_to_string(out.join("index.html")).unwrap();
        for needle in [
            "http://",
            "https://",
            "src=\"//",
            "href=\"//",
            "@import",
            "url(//",
        ] {
            assert!(!html.contains(needle), "external reference {needle:?}");
        }
        assert!(html.contains("saccade-view.v1"));
        assert!(!html.contains("__SACCADE_"), "unreplaced placeholder");
    }

    #[test]
    fn blind_page_hides_true_labels_and_unblind_restores_them() {
        let tmp = tempfile::tempdir().unwrap();
        let dirs = three_dirs(tmp.path());
        let opts = ViewOptions {
            labels: Some(vec![
                "zz_parent_label".into(),
                "zz_candidate_label".into(),
                "zz_other_label".into(),
            ]),
            blind: true,
            seed: Some(5),
            ..ViewOptions::default()
        };
        let out = tmp.path().join("view");
        let m = build_view(&dirs, &out, &opts).unwrap();
        assert_eq!(m.labels, ["P1", "P2", "P3"]);
        let html = std::fs::read_to_string(out.join("index.html")).unwrap();
        for secret in ["zz_parent_label", "zz_candidate_label", "zz_other_label"] {
            assert!(!html.contains(secret), "page leaks {secret:?}");
        }
        let key = read_blind_key(&out.join(BLIND_KEY_FILE)).unwrap();
        assert_eq!(
            key.labels,
            ["zz_parent_label", "zz_candidate_label", "zz_other_label"]
        );
        assert_eq!(key.shuffle_seed, 5);
        assert_ne!(m.seed, 5, "the page carries a token, not the shuffle seed");
        let decisions = Decisions {
            schema: DECISIONS_SCHEMA.into(),
            seed: m.seed,
            labels: m.labels.clone(),
            blind: true,
            dirs: Vec::new(),
            sets: vec![SetDecision {
                name: "x.png".into(),
                decision: None,
                chosen_label: Some("P2".into()),
                no_difference: false,
                note: String::new(),
                roi: None,
                timestamp_ms: 0,
                chosen_dir: None,
                sha256: Vec::new(),
                proposals: Vec::new(),
            }],
        };
        let back = unblind(&decisions, &key).unwrap();
        // Every set shuffles its own panes: P2 is whichever directory the key
        // lists second for this set.
        let second = key.sets["x.png"][1].clone();
        assert_eq!(back.sets[0].chosen_label.as_deref(), Some(second.as_str()));
        assert_eq!(back.labels, key.labels);
        assert!(!back.blind, "an unblinded file is not blind");
        assert_eq!(back.dirs, key.dirs);
        let second_dir = key.labels.iter().position(|l| *l == second);
        assert_eq!(
            back.sets[0].chosen_dir.as_deref(),
            second_dir.and_then(|i| key.dirs.get(i)).map(String::as_str)
        );
        let wrong = BlindKey {
            view_dir: None,
            seed: key.seed + 1,
            ..key
        };
        assert!(unblind(&decisions, &wrong).is_err());
    }
}
